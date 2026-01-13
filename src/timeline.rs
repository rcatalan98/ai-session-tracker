use crate::parser::Session;
use chrono::{DateTime, Local, Utc};
use colored::Colorize;
use std::collections::HashMap;

/// Print a visual timeline for a session
pub fn print_timeline(session: &Session) {
    print_session_header(session);
    print_timeline_events(session);
    print_summary(session);
}

/// Print session header with metadata
fn print_session_header(session: &Session) {
    let session_short: String = session.session_id.chars().take(10).collect();

    // Replace home dir with ~ for display
    let project_display = session.project.replace(
        &dirs::home_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        "~",
    );

    let branch = session.git_branch.as_deref().unwrap_or("unknown");

    let duration = match (session.start_time, session.end_time) {
        (Some(start), Some(end)) => {
            let mins = (end - start).num_minutes();
            if mins >= 60 {
                format!("{} hours {} minutes", mins / 60, mins % 60)
            } else {
                format!("{} minutes", mins)
            }
        }
        _ => "unknown".to_string(),
    };

    println!("{}: {}", "SESSION".bold(), session_short);
    println!("{}: {}", "Project".dimmed(), project_display);
    println!("{}: {}", "Branch".dimmed(), branch);
    println!("{}: {}", "Duration".dimmed(), duration);
    println!();
}

/// Get icon for a tool type
fn get_tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "Read" => "\u{1F4D6}",                  // book
        "Edit" | "Write" => "\u{270F}\u{FE0F}", // pencil
        "Bash" => "\u{1F5A5}\u{FE0F}",          // desktop computer
        "Grep" | "Glob" => "\u{1F50D}",         // magnifying glass
        "Task" => "\u{1F916}",                  // robot
        _ => "\u{2022}",                        // bullet
    }
}

/// Format timestamp for display
fn format_timestamp(ts: &DateTime<Utc>) -> String {
    let local: DateTime<Local> = ts.with_timezone(&Local);
    local.format("%H:%M:%S").to_string()
}

/// Extract a short description for a tool call
fn get_tool_description(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                let short_path = shorten_path(path);
                format!("Read {}", short_path)
            } else {
                "Read file".to_string()
            }
        }
        "Edit" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                let short_path = shorten_path(path);
                format!("Edit {}", short_path)
            } else {
                "Edit file".to_string()
            }
        }
        "Write" => {
            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                let short_path = shorten_path(path);
                format!("Write {}", short_path)
            } else {
                "Write file".to_string()
            }
        }
        "Bash" => {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                let short_cmd = if cmd.len() > 40 {
                    format!("{}...", &cmd[..37])
                } else {
                    cmd.to_string()
                };
                format!("Bash: {}", short_cmd)
            } else {
                "Bash command".to_string()
            }
        }
        "Grep" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                let short_pattern = if pattern.len() > 30 {
                    format!("{}...", &pattern[..27])
                } else {
                    pattern.to_string()
                };
                format!("Grep \"{}\"", short_pattern)
            } else {
                "Grep search".to_string()
            }
        }
        "Glob" => {
            if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                format!("Glob \"{}\"", pattern)
            } else {
                "Glob search".to_string()
            }
        }
        "Task" => "Task (subagent)".to_string(),
        _ => tool_name.to_string(),
    }
}

/// Shorten a file path for display
fn shorten_path(path: &str) -> String {
    // Replace home dir with ~
    let shortened = path.replace(
        &dirs::home_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        "~",
    );

    // If still too long, show just the filename
    if shortened.len() > 50 {
        if let Some(filename) = std::path::Path::new(path).file_name() {
            return filename.to_string_lossy().to_string();
        }
    }

    shortened
}

/// A timeline event for display
struct TimelineEvent {
    timestamp: DateTime<Utc>,
    icon: &'static str,
    description: String,
    is_error: bool,
    has_success: bool,
}

/// Print the timeline events
fn print_timeline_events(session: &Session) {
    println!("{}", "TIMELINE".bold());
    println!("{}", "\u{2500}".repeat(60).dimmed());

    let mut events: Vec<TimelineEvent> = Vec::new();

    // Add session start
    if let Some(start) = session.start_time {
        events.push(TimelineEvent {
            timestamp: start,
            icon: "\u{25B6}",
            description: "Session start".to_string(),
            is_error: false,
            has_success: false,
        });
    }

    // Collect tool calls and results
    for message in &session.messages {
        let ts = match message.timestamp {
            Some(t) => t,
            None => continue,
        };

        // Add tool calls
        for tool_call in &message.tool_calls {
            events.push(TimelineEvent {
                timestamp: ts,
                icon: get_tool_icon(&tool_call.name),
                description: get_tool_description(&tool_call.name, &tool_call.input),
                is_error: false,
                has_success: false,
            });
        }

        // Add tool results (especially errors)
        for tool_result in &message.tool_results {
            if tool_result.is_error {
                // Extract a short error message
                let error_msg = if tool_result.content.len() > 50 {
                    format!("{}...", &tool_result.content[..47])
                } else {
                    tool_result.content.clone()
                };
                events.push(TimelineEvent {
                    timestamp: ts,
                    icon: "\u{274C}",
                    description: format!("Error: {}", error_msg),
                    is_error: true,
                    has_success: false,
                });
            }
        }
    }

    // Add session end
    if let Some(end) = session.end_time {
        events.push(TimelineEvent {
            timestamp: end,
            icon: "\u{23F9}",
            description: "Session end".to_string(),
            is_error: false,
            has_success: false,
        });
    }

    // Sort events by timestamp
    events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    // Mark successful bash commands (those not followed by errors)
    mark_successful_bash_commands(&mut events);

    // Print events
    for event in &events {
        let ts_str = format_timestamp(&event.timestamp);
        let icon = event.icon;

        let desc = if event.is_error {
            event.description.red().to_string()
        } else if event.has_success {
            format!("{} {}", event.description, "\u{2705}".green())
        } else {
            event.description.clone()
        };

        println!("{}  {} {}", ts_str.dimmed(), icon, desc);
    }

    println!();
}

/// Mark bash commands that complete successfully (not followed by error)
fn mark_successful_bash_commands(events: &mut [TimelineEvent]) {
    let len = events.len();
    for i in 0..len {
        if events[i].description.starts_with("Bash:") {
            // Check if next event is an error
            let next_is_error = if i + 1 < len {
                events[i + 1].is_error
            } else {
                false
            };

            if !next_is_error {
                events[i].has_success = true;
            }
        }
    }
}

/// Print summary statistics
fn print_summary(session: &Session) {
    println!("{}", "SUMMARY".bold());
    println!("{}", "\u{2500}".repeat(60).dimmed());

    // Count tool calls by type
    let mut tool_counts: HashMap<String, usize> = HashMap::new();
    let mut error_count = 0;
    let mut files_touched: std::collections::HashSet<String> = std::collections::HashSet::new();

    for message in &session.messages {
        for tool_call in &message.tool_calls {
            *tool_counts.entry(tool_call.name.clone()).or_insert(0) += 1;

            // Track files touched
            if let Some(path) = tool_call.input.get("file_path").and_then(|v| v.as_str()) {
                files_touched.insert(path.to_string());
            }
        }

        for tool_result in &message.tool_results {
            if tool_result.is_error {
                error_count += 1;
            }
        }
    }

    // Total tool calls
    let total_calls: usize = tool_counts.values().sum();

    // Format tool breakdown
    let mut breakdown_parts: Vec<String> = Vec::new();
    for (name, count) in &tool_counts {
        breakdown_parts.push(format!("{}: {}", name, count));
    }
    breakdown_parts.sort();
    let breakdown = breakdown_parts.join(", ");

    println!("{}: {} ({})", "Tool calls".dimmed(), total_calls, breakdown);

    // Errors
    let error_status = if error_count > 0 {
        format!("{} (check timeline for details)", error_count)
    } else {
        "0".to_string()
    };
    println!("{}: {}", "Errors".dimmed(), error_status);

    // Files touched
    println!("{}: {}", "Files touched".dimmed(), files_touched.len());
}

/// Find a session by ID (supports partial match)
pub fn find_session_by_id<'a>(sessions: &'a [Session], id: &str) -> Option<&'a Session> {
    // First try exact match
    if let Some(session) = sessions.iter().find(|s| s.session_id == id) {
        return Some(session);
    }

    // Then try partial match (starts with)
    sessions.iter().find(|s| s.session_id.starts_with(id))
}

/// Get the latest session
pub fn get_latest_session(sessions: &[Session]) -> Option<&Session> {
    sessions
        .iter()
        .filter(|s| s.end_time.is_some())
        .max_by_key(|s| s.end_time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_icon() {
        assert_eq!(get_tool_icon("Read"), "\u{1F4D6}");
        assert_eq!(get_tool_icon("Edit"), "\u{270F}\u{FE0F}");
        assert_eq!(get_tool_icon("Bash"), "\u{1F5A5}\u{FE0F}");
        assert_eq!(get_tool_icon("Grep"), "\u{1F50D}");
        assert_eq!(get_tool_icon("Task"), "\u{1F916}");
        assert_eq!(get_tool_icon("Unknown"), "\u{2022}");
    }

    #[test]
    fn test_shorten_path() {
        // Test that a short path stays as-is (minus home dir replacement)
        let short = shorten_path("/tmp/test.rs");
        assert!(short.len() <= 50 || short.contains("test.rs"));
    }

    #[test]
    fn test_get_tool_description() {
        let input = serde_json::json!({"file_path": "/tmp/test.rs"});
        let desc = get_tool_description("Read", &input);
        assert!(desc.contains("Read"));
        assert!(desc.contains("test.rs"));

        let bash_input = serde_json::json!({"command": "cargo build"});
        let bash_desc = get_tool_description("Bash", &bash_input);
        assert!(bash_desc.contains("Bash:"));
        assert!(bash_desc.contains("cargo build"));
    }

    #[test]
    fn test_find_session_by_id() {
        let sessions = vec![
            Session {
                session_id: "abc123def".to_string(),
                project: "/test".to_string(),
                jsonl_path: std::path::PathBuf::from("/test.jsonl"),
                git_branch: None,
                start_time: None,
                end_time: None,
                messages: vec![],
            },
            Session {
                session_id: "xyz789ghi".to_string(),
                project: "/test2".to_string(),
                jsonl_path: std::path::PathBuf::from("/test2.jsonl"),
                git_branch: None,
                start_time: None,
                end_time: None,
                messages: vec![],
            },
        ];

        // Exact match
        let found = find_session_by_id(&sessions, "abc123def");
        assert!(found.is_some());
        assert_eq!(found.unwrap().session_id, "abc123def");

        // Partial match
        let found_partial = find_session_by_id(&sessions, "abc");
        assert!(found_partial.is_some());
        assert_eq!(found_partial.unwrap().session_id, "abc123def");

        // No match
        let not_found = find_session_by_id(&sessions, "notfound");
        assert!(not_found.is_none());
    }
}
