use crate::parser::{MessageType, Session};
use chrono::{Duration, Utc};
use std::collections::{HashMap, HashSet};

/// Metrics for a single session
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in later issues
pub struct SessionMetrics {
    pub duration_minutes: f64,
    pub tool_counts: HashMap<String, usize>,
    pub total_tool_calls: usize,
    pub error_count: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub files_read: HashSet<String>,
    pub files_edited: HashSet<String>,
}

/// Metrics for a project
#[derive(Debug, Clone, Default)]
pub struct ProjectMetrics {
    pub session_count: usize,
    pub total_duration_minutes: f64,
    pub total_tool_calls: usize,
    pub total_errors: usize,
}

/// Aggregated metrics across multiple sessions
#[derive(Debug, Clone)]
pub struct AggregatedMetrics {
    pub session_count: usize,
    pub total_duration_minutes: f64,
    pub total_tool_calls: usize,
    pub total_errors: usize,
    pub tool_counts: HashMap<String, usize>,
    pub by_project: HashMap<String, ProjectMetrics>,
}

/// Calculate metrics for a single session
pub fn calculate_session_metrics(session: &Session) -> SessionMetrics {
    let mut tool_counts: HashMap<String, usize> = HashMap::new();
    let mut total_tool_calls = 0;
    let mut error_count = 0;
    let mut user_messages = 0;
    let mut assistant_messages = 0;
    let mut files_read: HashSet<String> = HashSet::new();
    let mut files_edited: HashSet<String> = HashSet::new();

    for message in &session.messages {
        // Count message types
        match message.msg_type {
            MessageType::User => user_messages += 1,
            MessageType::Assistant => assistant_messages += 1,
            _ => {}
        }

        // Count tool calls
        for tool_call in &message.tool_calls {
            *tool_counts.entry(tool_call.name.clone()).or_insert(0) += 1;
            total_tool_calls += 1;

            // Track files read
            if tool_call.name == "Read" {
                if let Some(path) = tool_call.input.get("file_path").and_then(|v| v.as_str()) {
                    files_read.insert(path.to_string());
                }
            }

            // Track files edited
            if tool_call.name == "Edit" || tool_call.name == "Write" {
                if let Some(path) = tool_call.input.get("file_path").and_then(|v| v.as_str()) {
                    files_edited.insert(path.to_string());
                }
            }
        }

        // Count errors
        for tool_result in &message.tool_results {
            if tool_result.is_error {
                error_count += 1;
            }
        }
    }

    // Calculate duration
    let duration_minutes = match (session.start_time, session.end_time) {
        (Some(start), Some(end)) => (end - start).num_seconds() as f64 / 60.0,
        _ => 0.0,
    };

    SessionMetrics {
        duration_minutes,
        tool_counts,
        total_tool_calls,
        error_count,
        user_messages,
        assistant_messages,
        files_read,
        files_edited,
    }
}

/// Aggregate metrics across multiple sessions
pub fn aggregate_metrics(sessions: &[Session]) -> AggregatedMetrics {
    let mut total_duration_minutes = 0.0;
    let mut total_tool_calls = 0;
    let mut total_errors = 0;
    let mut tool_counts: HashMap<String, usize> = HashMap::new();
    let mut by_project: HashMap<String, ProjectMetrics> = HashMap::new();

    for session in sessions {
        let metrics = calculate_session_metrics(session);

        total_duration_minutes += metrics.duration_minutes;
        total_tool_calls += metrics.total_tool_calls;
        total_errors += metrics.error_count;

        // Aggregate tool counts
        for (tool, count) in &metrics.tool_counts {
            *tool_counts.entry(tool.clone()).or_insert(0) += count;
        }

        // Aggregate by project
        let project_name = extract_project_name(&session.project);
        let project_metrics = by_project.entry(project_name).or_default();
        project_metrics.session_count += 1;
        project_metrics.total_duration_minutes += metrics.duration_minutes;
        project_metrics.total_tool_calls += metrics.total_tool_calls;
        project_metrics.total_errors += metrics.error_count;
    }

    AggregatedMetrics {
        session_count: sessions.len(),
        total_duration_minutes,
        total_tool_calls,
        total_errors,
        tool_counts,
        by_project,
    }
}

/// Filter sessions by time period
#[allow(dead_code)] // Will be used in report command
pub fn filter_by_period(sessions: &[Session], period: &str) -> Vec<Session> {
    let now = Utc::now();
    let cutoff = match period.to_lowercase().as_str() {
        "day" => now - Duration::days(1),
        "week" => now - Duration::weeks(1),
        "month" => now - Duration::days(30),
        _ => return sessions.to_vec(),
    };

    sessions
        .iter()
        .filter(|s| s.end_time.map(|t| t >= cutoff).unwrap_or(false))
        .cloned()
        .collect()
}

/// Extract a short project name from the full path
fn extract_project_name(project_path: &str) -> String {
    project_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("unknown")
        .to_string()
}

/// Format duration in hours and minutes
pub fn format_duration(minutes: f64) -> String {
    let hours = minutes / 60.0;
    if hours >= 1.0 {
        format!("{:.1}h", hours)
    } else {
        format!("{:.0}m", minutes)
    }
}

/// Format a number with thousands separators
pub fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Message, ToolCall, ToolResult};
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn create_test_session() -> Session {
        let start = Utc.with_ymd_and_hms(2026, 1, 13, 10, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 13, 11, 30, 0).unwrap();

        Session {
            session_id: "test-session".to_string(),
            project: "/Users/test/projects/my-project".to_string(),
            jsonl_path: PathBuf::from("/test/path.jsonl"),
            git_branch: Some("main".to_string()),
            start_time: Some(start),
            end_time: Some(end),
            messages: vec![
                Message {
                    msg_type: MessageType::User,
                    timestamp: Some(start),
                    tool_calls: vec![],
                    tool_results: vec![],
                },
                Message {
                    msg_type: MessageType::Assistant,
                    timestamp: Some(start),
                    tool_calls: vec![
                        ToolCall {
                            name: "Read".to_string(),
                            input: serde_json::json!({"file_path": "/test/file.rs"}),
                        },
                        ToolCall {
                            name: "Edit".to_string(),
                            input: serde_json::json!({"file_path": "/test/file.rs"}),
                        },
                    ],
                    tool_results: vec![],
                },
                Message {
                    msg_type: MessageType::User,
                    timestamp: Some(end),
                    tool_calls: vec![],
                    tool_results: vec![
                        ToolResult {
                            tool_use_id: "1".to_string(),
                            content: "success".to_string(),
                            is_error: false,
                        },
                        ToolResult {
                            tool_use_id: "2".to_string(),
                            content: "error".to_string(),
                            is_error: true,
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn test_calculate_session_metrics() {
        let session = create_test_session();
        let metrics = calculate_session_metrics(&session);

        assert_eq!(metrics.duration_minutes, 90.0);
        assert_eq!(metrics.total_tool_calls, 2);
        assert_eq!(metrics.error_count, 1);
        assert_eq!(metrics.user_messages, 2);
        assert_eq!(metrics.assistant_messages, 1);
        assert!(metrics.files_read.contains("/test/file.rs"));
        assert!(metrics.files_edited.contains("/test/file.rs"));
        assert_eq!(*metrics.tool_counts.get("Read").unwrap_or(&0), 1);
        assert_eq!(*metrics.tool_counts.get("Edit").unwrap_or(&0), 1);
    }

    #[test]
    fn test_aggregate_metrics() {
        let session1 = create_test_session();
        let mut session2 = create_test_session();
        session2.project = "/Users/test/projects/other-project".to_string();

        let metrics = aggregate_metrics(&[session1, session2]);

        assert_eq!(metrics.session_count, 2);
        assert_eq!(metrics.total_duration_minutes, 180.0);
        assert_eq!(metrics.total_tool_calls, 4);
        assert_eq!(metrics.total_errors, 2);
        assert_eq!(metrics.by_project.len(), 2);
    }

    #[test]
    fn test_extract_project_name() {
        assert_eq!(
            extract_project_name("/Users/test/projects/my-project"),
            "my-project"
        );
        assert_eq!(
            extract_project_name("/Users/test/projects/my-project/"),
            "my-project"
        );
        assert_eq!(extract_project_name("simple"), "simple");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30m");
        assert_eq!(format_duration(90.0), "1.5h");
        assert_eq!(format_duration(120.0), "2.0h");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(100), "100");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn test_filter_by_period_all() {
        let sessions = vec![create_test_session()];
        let filtered = filter_by_period(&sessions, "all");
        assert_eq!(filtered.len(), 1);
    }
}
