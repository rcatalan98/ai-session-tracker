use crate::parser::{Message, MessageType, Session};
use chrono::{DateTime, Utc};
use colored::Colorize;
use std::collections::HashMap;

/// A detected bottleneck in a session
#[derive(Debug, Clone)]
pub enum Bottleneck {
    ErrorLoop(ErrorLoop),
    ExplorationSpiral(ExplorationSpiral),
    EditThrashing(EditThrashing),
    LongGap(LongGap),
}

/// Same tool fails 3+ times consecutively
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in report generation
pub struct ErrorLoop {
    pub session_id: String,
    pub project: String,
    pub tool_name: String,
    pub failure_count: usize,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_minutes: f64,
    pub error_samples: Vec<String>,
    pub preceding_prompt: Option<String>,
}

/// >10 Read/Grep calls with 0 Edit in 10+ minutes
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in report generation
pub struct ExplorationSpiral {
    pub session_id: String,
    pub project: String,
    pub read_count: usize,
    pub grep_count: usize,
    pub duration_minutes: f64,
    pub start_time: Option<DateTime<Utc>>,
    pub files_searched: Vec<String>,
    pub preceding_prompt: Option<String>,
}

/// Same file edited 5+ times in a session
#[derive(Debug, Clone)]
pub struct EditThrashing {
    pub session_id: String,
    pub project: String,
    pub file_path: String,
    pub edit_count: usize,
    pub duration_minutes: f64,
    pub preceding_prompt: Option<String>,
}

/// >5 minutes between consecutive messages
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in report generation
pub struct LongGap {
    pub session_id: String,
    pub project: String,
    pub gap_minutes: f64,
    pub before_timestamp: Option<DateTime<Utc>>,
    pub after_timestamp: Option<DateTime<Utc>>,
    pub preceding_prompt: Option<String>,
}

#[allow(dead_code)] // Methods will be used in report generation
impl Bottleneck {
    pub fn wasted_minutes(&self) -> f64 {
        match self {
            Bottleneck::ErrorLoop(e) => e.duration_minutes,
            Bottleneck::ExplorationSpiral(e) => e.duration_minutes,
            Bottleneck::EditThrashing(e) => e.duration_minutes,
            Bottleneck::LongGap(g) => g.gap_minutes,
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Bottleneck::ErrorLoop(e) => &e.session_id,
            Bottleneck::ExplorationSpiral(e) => &e.session_id,
            Bottleneck::EditThrashing(e) => &e.session_id,
            Bottleneck::LongGap(g) => &g.session_id,
        }
    }

    pub fn project(&self) -> &str {
        match self {
            Bottleneck::ErrorLoop(e) => &e.project,
            Bottleneck::ExplorationSpiral(e) => &e.project,
            Bottleneck::EditThrashing(e) => &e.project,
            Bottleneck::LongGap(g) => &g.project,
        }
    }

    pub fn preceding_prompt(&self) -> Option<&str> {
        match self {
            Bottleneck::ErrorLoop(e) => e.preceding_prompt.as_deref(),
            Bottleneck::ExplorationSpiral(e) => e.preceding_prompt.as_deref(),
            Bottleneck::EditThrashing(e) => e.preceding_prompt.as_deref(),
            Bottleneck::LongGap(g) => g.preceding_prompt.as_deref(),
        }
    }
}

/// Truncate text to max chars, adding "..." if truncated
fn truncate_prompt(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        format!("{}...", &text[..max_chars])
    }
}

/// Find the last user message with text content before the given message index
fn find_preceding_prompt(messages: &[Message], before_index: usize) -> Option<String> {
    for i in (0..before_index).rev() {
        if messages[i].msg_type == MessageType::User {
            if let Some(text) = &messages[i].text_content {
                if !text.is_empty() {
                    return Some(truncate_prompt(text, 200));
                }
            }
        }
    }
    None
}

/// Detect all bottlenecks in a set of sessions
pub fn detect_all(sessions: &[Session]) -> Vec<Bottleneck> {
    let mut bottlenecks = Vec::new();

    for session in sessions {
        bottlenecks.extend(detect_error_loops(session));
        bottlenecks.extend(detect_exploration_spirals(session));
        bottlenecks.extend(detect_edit_thrashing(session));
        bottlenecks.extend(detect_long_gaps(session));
    }

    // Sort by wasted time descending
    bottlenecks.sort_by(|a, b| {
        b.wasted_minutes()
            .partial_cmp(&a.wasted_minutes())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    bottlenecks
}

/// Detect error loops: same tool fails 3+ times consecutively
fn detect_error_loops(session: &Session) -> Vec<Bottleneck> {
    let mut bottlenecks = Vec::new();

    // Build a list of (tool_name, is_error, timestamp, msg_index) from tool results
    let mut tool_results: Vec<(String, bool, Option<DateTime<Utc>>, usize)> = Vec::new();

    // Track tool_use_id -> tool_name mapping
    let mut tool_id_to_name: HashMap<String, String> = HashMap::new();

    for (msg_idx, msg) in session.messages.iter().enumerate() {
        // Record tool calls
        if msg.msg_type == MessageType::Assistant {
            for tc in &msg.tool_calls {
                // We don't have tool_use_id in our struct, so we'll match by order
                tool_id_to_name.insert(tc.name.clone(), tc.name.clone());
            }
        }

        // Record tool results
        if msg.msg_type == MessageType::User {
            for tr in &msg.tool_results {
                let is_error = tr.is_error || is_error_content(&tr.content);
                // Try to find the tool name from the mapping, fallback to generic
                let tool_name = tool_id_to_name
                    .get(&tr.tool_use_id)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
                tool_results.push((tool_name, is_error, msg.timestamp, msg_idx));
            }
        }
    }

    // Find consecutive failures
    let mut i = 0;
    while i < tool_results.len() {
        if tool_results[i].1 {
            // Found an error
            let tool_name = &tool_results[i].0;
            let start_time = tool_results[i].2;
            let start_msg_idx = tool_results[i].3;
            let mut count = 1;
            let error_samples: Vec<String> = Vec::new();

            // Look ahead for consecutive errors of same tool (or any tool)
            let mut j = i + 1;
            let mut end_time = start_time;
            while j < tool_results.len() && tool_results[j].1 {
                count += 1;
                end_time = tool_results[j].2;
                j += 1;
            }

            if count >= 3 {
                let duration = match (start_time, end_time) {
                    (Some(s), Some(e)) => (e - s).num_seconds() as f64 / 60.0,
                    _ => 0.0,
                };

                let preceding_prompt = find_preceding_prompt(&session.messages, start_msg_idx);

                bottlenecks.push(Bottleneck::ErrorLoop(ErrorLoop {
                    session_id: session.session_id.clone(),
                    project: extract_project_name(&session.project),
                    tool_name: tool_name.clone(),
                    failure_count: count,
                    start_time,
                    end_time,
                    duration_minutes: duration.max(1.0), // At least 1 minute
                    error_samples,
                    preceding_prompt,
                }));
            }

            i = j;
        } else {
            i += 1;
        }
    }

    bottlenecks
}

/// Detect exploration spirals: lots of reading without editing
fn detect_exploration_spirals(session: &Session) -> Vec<Bottleneck> {
    let mut bottlenecks = Vec::new();

    // Sliding window approach: look for periods of high read/grep with no edit
    let mut read_count = 0;
    let mut grep_count = 0;
    let mut files_searched: Vec<String> = Vec::new();
    let mut window_start: Option<DateTime<Utc>> = None;
    let mut window_start_idx: usize = 0;
    let mut last_edit_time: Option<DateTime<Utc>> = None;

    for (msg_idx, msg) in session.messages.iter().enumerate() {
        if msg.msg_type == MessageType::Assistant {
            for tc in &msg.tool_calls {
                match tc.name.as_str() {
                    "Read" => {
                        if window_start.is_none() {
                            window_start = msg.timestamp;
                            window_start_idx = msg_idx;
                        }
                        read_count += 1;
                        if let Some(path) = tc.input.get("file_path").and_then(|v| v.as_str()) {
                            if !files_searched.contains(&path.to_string()) {
                                files_searched.push(path.to_string());
                            }
                        }
                    }
                    "Grep" | "Glob" => {
                        if window_start.is_none() {
                            window_start = msg.timestamp;
                            window_start_idx = msg_idx;
                        }
                        grep_count += 1;
                    }
                    "Edit" | "Write" => {
                        // Check if we had a spiral before this edit
                        if read_count + grep_count >= 10 {
                            if let Some(start) = window_start {
                                let end = last_edit_time.or(msg.timestamp).unwrap_or(start);
                                let duration = (end - start).num_seconds() as f64 / 60.0;

                                if duration >= 10.0 {
                                    let preceding_prompt =
                                        find_preceding_prompt(&session.messages, window_start_idx);
                                    bottlenecks.push(Bottleneck::ExplorationSpiral(
                                        ExplorationSpiral {
                                            session_id: session.session_id.clone(),
                                            project: extract_project_name(&session.project),
                                            read_count,
                                            grep_count,
                                            duration_minutes: duration,
                                            start_time: Some(start),
                                            files_searched: files_searched.clone(),
                                            preceding_prompt,
                                        },
                                    ));
                                }
                            }
                        }

                        // Reset counters
                        read_count = 0;
                        grep_count = 0;
                        files_searched.clear();
                        window_start = None;
                        last_edit_time = msg.timestamp;
                    }
                    _ => {}
                }
            }
        }
    }

    // Check for trailing spiral (session ended without edit)
    if read_count + grep_count >= 10 {
        if let (Some(start), Some(end)) = (window_start, session.end_time) {
            let duration = (end - start).num_seconds() as f64 / 60.0;
            if duration >= 10.0 {
                let preceding_prompt = find_preceding_prompt(&session.messages, window_start_idx);
                bottlenecks.push(Bottleneck::ExplorationSpiral(ExplorationSpiral {
                    session_id: session.session_id.clone(),
                    project: extract_project_name(&session.project),
                    read_count,
                    grep_count,
                    duration_minutes: duration,
                    start_time: Some(start),
                    files_searched,
                    preceding_prompt,
                }));
            }
        }
    }

    bottlenecks
}

/// Detect edit thrashing: same file edited 5+ times
fn detect_edit_thrashing(session: &Session) -> Vec<Bottleneck> {
    let mut bottlenecks = Vec::new();

    // Count edits per file: (count, first_edit, last_edit, first_msg_idx)
    #[allow(clippy::type_complexity)]
    let mut edit_counts: HashMap<
        String,
        (usize, Option<DateTime<Utc>>, Option<DateTime<Utc>>, usize),
    > = HashMap::new();

    for (msg_idx, msg) in session.messages.iter().enumerate() {
        if msg.msg_type == MessageType::Assistant {
            for tc in &msg.tool_calls {
                if tc.name == "Edit" || tc.name == "Write" {
                    if let Some(path) = tc.input.get("file_path").and_then(|v| v.as_str()) {
                        let entry = edit_counts.entry(path.to_string()).or_insert((
                            0,
                            msg.timestamp,
                            msg.timestamp,
                            msg_idx,
                        ));
                        entry.0 += 1;
                        entry.2 = msg.timestamp; // Update end time
                    }
                }
            }
        }
    }

    // Find files with 5+ edits
    for (file_path, (count, start, end, first_msg_idx)) in edit_counts {
        if count >= 5 {
            let duration = match (start, end) {
                (Some(s), Some(e)) => (e - s).num_seconds() as f64 / 60.0,
                _ => 0.0,
            };

            let preceding_prompt = find_preceding_prompt(&session.messages, first_msg_idx);

            bottlenecks.push(Bottleneck::EditThrashing(EditThrashing {
                session_id: session.session_id.clone(),
                project: extract_project_name(&session.project),
                file_path: shorten_path(&file_path),
                edit_count: count,
                duration_minutes: duration.max(1.0),
                preceding_prompt,
            }));
        }
    }

    bottlenecks
}

/// Detect long gaps: >5 minutes between consecutive messages
fn detect_long_gaps(session: &Session) -> Vec<Bottleneck> {
    let mut bottlenecks = Vec::new();

    let mut prev_timestamp: Option<DateTime<Utc>> = None;
    let mut prev_msg_idx: usize = 0;

    for (msg_idx, msg) in session.messages.iter().enumerate() {
        if let Some(ts) = msg.timestamp {
            if let Some(prev) = prev_timestamp {
                let gap_minutes = (ts - prev).num_seconds() as f64 / 60.0;

                if gap_minutes >= 5.0 {
                    // Find the user prompt before the gap
                    let preceding_prompt =
                        find_preceding_prompt(&session.messages, prev_msg_idx + 1);

                    bottlenecks.push(Bottleneck::LongGap(LongGap {
                        session_id: session.session_id.clone(),
                        project: extract_project_name(&session.project),
                        gap_minutes,
                        before_timestamp: Some(prev),
                        after_timestamp: Some(ts),
                        preceding_prompt,
                    }));
                }
            }
            prev_timestamp = Some(ts);
            prev_msg_idx = msg_idx;
        }
    }

    bottlenecks
}

/// Check if content looks like an error
fn is_error_content(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("error")
        || lower.contains("failed")
        || lower.contains("not found")
        || lower.contains("permission denied")
        || lower.contains("no such file")
        || lower.contains("command not found")
        || lower.contains("exit code")
}

/// Extract short project name from path
fn extract_project_name(path: &str) -> String {
    path.split('/')
        .rfind(|s| !s.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

/// Shorten file path for display
fn shorten_path(path: &str) -> String {
    let home = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();
    path.replace(&home, "~")
}

/// Print bottlenecks to terminal
pub fn print_bottlenecks(bottlenecks: &[Bottleneck], limit: usize, show_prompts: bool) {
    if bottlenecks.is_empty() {
        println!("{}", "No bottlenecks detected.".green());
        return;
    }

    let total_wasted: f64 = bottlenecks.iter().map(|b| b.wasted_minutes()).sum();

    println!("{}", "BOTTLENECKS DETECTED".bold());
    println!("{}", "═".repeat(60));
    println!(
        "Found {} bottlenecks | ~{:.0} minutes potentially wasted\n",
        bottlenecks.len().to_string().bold(),
        total_wasted
    );

    for (i, bottleneck) in bottlenecks.iter().take(limit).enumerate() {
        print_single_bottleneck(i + 1, bottleneck, show_prompts);
        println!();
    }

    if bottlenecks.len() > limit {
        println!(
            "... and {} more (use --limit to see more)",
            bottlenecks.len() - limit
        );
    }
}

fn print_single_bottleneck(num: usize, bottleneck: &Bottleneck, show_prompt: bool) {
    match bottleneck {
        Bottleneck::ErrorLoop(e) => {
            println!(
                "{}. {} {}",
                num,
                "ERROR LOOP".red().bold(),
                format!("(~{:.0} min wasted)", e.duration_minutes).dimmed()
            );
            println!("   {}", "─".repeat(50).dimmed());
            println!(
                "   Session: {} ({})",
                &e.session_id[..10.min(e.session_id.len())],
                e.project
            );
            println!(
                "   Pattern: {} failed {} times in a row",
                e.tool_name.yellow(),
                e.failure_count
            );
            println!(
                "   {}",
                "Suggestion: Check tool availability and inputs before running".cyan()
            );
        }
        Bottleneck::ExplorationSpiral(e) => {
            println!(
                "{}. {} {}",
                num,
                "EXPLORATION SPIRAL".yellow().bold(),
                format!("(~{:.0} min)", e.duration_minutes).dimmed()
            );
            println!("   {}", "─".repeat(50).dimmed());
            println!(
                "   Session: {} ({})",
                &e.session_id[..10.min(e.session_id.len())],
                e.project
            );
            println!(
                "   Pattern: {} Read + {} Grep calls with no Edit",
                e.read_count, e.grep_count
            );
            println!(
                "   Files searched: {}",
                e.files_searched.len().to_string().yellow()
            );
            println!(
                "   {}",
                "Suggestion: Provide better context upfront (CLAUDE.md, file hints)".cyan()
            );
        }
        Bottleneck::EditThrashing(e) => {
            println!(
                "{}. {} {}",
                num,
                "EDIT THRASHING".magenta().bold(),
                format!("(~{:.0} min)", e.duration_minutes).dimmed()
            );
            println!("   {}", "─".repeat(50).dimmed());
            println!(
                "   Session: {} ({})",
                &e.session_id[..10.min(e.session_id.len())],
                e.project
            );
            println!(
                "   Pattern: {} edited {} times",
                e.file_path.yellow(),
                e.edit_count
            );
            println!(
                "   {}",
                "Suggestion: Break down complex changes into smaller tasks".cyan()
            );
        }
        Bottleneck::LongGap(g) => {
            println!(
                "{}. {} {}",
                num,
                "LONG GAP".blue().bold(),
                format!("({:.0} min pause)", g.gap_minutes).dimmed()
            );
            println!("   {}", "─".repeat(50).dimmed());
            println!(
                "   Session: {} ({})",
                &g.session_id[..10.min(g.session_id.len())],
                g.project
            );
            println!(
                "   Pattern: {:.0} minute gap between actions",
                g.gap_minutes
            );
            println!(
                "   {}",
                "Suggestion: Review what caused the pause - unclear requirements?".cyan()
            );
        }
    }

    // Show preceding prompt if requested
    if show_prompt {
        if let Some(prompt) = bottleneck.preceding_prompt() {
            println!("   {}", "Prompt:".dimmed());
            println!("   {}", format!("\"{}\"", prompt).italic().dimmed());
        } else {
            println!("   {}", "Prompt: (no prompt found)".dimmed());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_error_content() {
        assert!(is_error_content("Error: file not found"));
        assert!(is_error_content("command failed with exit code 1"));
        assert!(is_error_content("Permission denied"));
        assert!(!is_error_content("Success!"));
        assert!(!is_error_content("File created"));
    }

    #[test]
    fn test_extract_project_name() {
        assert_eq!(extract_project_name("/Users/rj/projects/my-app"), "my-app");
        assert_eq!(extract_project_name("~/projects/foo"), "foo");
        assert_eq!(extract_project_name(""), "unknown");
    }

    #[test]
    fn test_shorten_path() {
        // Just verify it doesn't panic
        let result = shorten_path("/some/long/path/to/file.rs");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_bottleneck_wasted_minutes() {
        let error_loop = Bottleneck::ErrorLoop(ErrorLoop {
            session_id: "test".to_string(),
            project: "test".to_string(),
            tool_name: "Bash".to_string(),
            failure_count: 3,
            start_time: None,
            end_time: None,
            duration_minutes: 5.0,
            error_samples: vec![],
            preceding_prompt: Some("help me fix this bug".to_string()),
        });
        assert_eq!(error_loop.wasted_minutes(), 5.0);
        assert_eq!(error_loop.preceding_prompt(), Some("help me fix this bug"));
    }
}
