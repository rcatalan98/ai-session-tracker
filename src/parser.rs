use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A parsed Claude Code session
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in later issues
pub struct Session {
    pub session_id: String,
    pub project: String,
    pub jsonl_path: PathBuf,
    pub git_branch: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub messages: Vec<Message>,
    /// Total input tokens consumed in this session
    pub token_input: u64,
    /// Total output tokens consumed in this session
    pub token_output: u64,
}

/// A message in a session
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in later issues
pub struct Message {
    pub msg_type: MessageType,
    pub timestamp: Option<DateTime<Utc>>,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolResult>,
    pub text_content: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    User,
    Assistant,
    System,
    Summary,
    FileHistorySnapshot,
    Unknown,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in later issues
pub struct ToolCall {
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields will be used in later issues
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

/// Raw JSON structure from Claude Code transcripts
#[derive(Debug, Deserialize)]
struct RawMessage {
    #[serde(rename = "type")]
    msg_type: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    cwd: Option<String>,
    message: Option<RawMessageContent>,
}

#[derive(Debug, Deserialize)]
struct RawMessageContent {
    content: Option<serde_json::Value>,
    usage: Option<RawUsage>,
}

/// Token usage data from Claude API responses
#[derive(Debug, Deserialize)]
struct RawUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    #[allow(dead_code)]
    cache_read_input_tokens: Option<u64>, // Usually free, not counted
}

/// Get the Claude projects directory
fn claude_projects_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("projects"))
}

/// Find all session JSONL files
fn find_session_files(filter_project: Option<&Path>) -> Vec<PathBuf> {
    let projects_dir = match claude_projects_dir() {
        Some(dir) if dir.exists() => dir,
        _ => return vec![],
    };

    let mut files = vec![];

    for entry in WalkDir::new(&projects_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip if not a JSONL file
        if path.extension().map(|e| e != "jsonl").unwrap_or(true) {
            continue;
        }

        // Skip subagent files
        if path.to_string_lossy().contains("/subagents/") {
            continue;
        }

        // Apply project filter if specified
        if let Some(filter) = filter_project {
            let filter_str = filter.to_string_lossy();
            let path_str = path.to_string_lossy();

            // The project path is encoded in the directory name
            // e.g., ~/.claude/projects/-Users-rj-personal-projects-ai-editor/
            let encoded_filter = filter_str.replace('/', "-");
            if !path_str.contains(&encoded_filter) && !path_str.contains(&*filter_str) {
                continue;
            }
        }

        files.push(path.to_path_buf());
    }

    files
}

/// Parse a single JSONL file into a Session
fn parse_session_file(path: &Path) -> Option<Session> {
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    let mut session_id = String::new();
    let mut project = String::new();
    let mut git_branch = None;
    let mut messages = vec![];
    let mut timestamps: Vec<DateTime<Utc>> = vec![];
    let mut token_input: u64 = 0;
    let mut token_output: u64 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        if line.trim().is_empty() {
            continue;
        }

        let raw: RawMessage = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(_) => continue, // Skip malformed lines
        };

        // Extract session metadata from first valid message
        if session_id.is_empty() {
            if let Some(sid) = &raw.session_id {
                session_id = sid.clone();
            }
        }
        if project.is_empty() {
            if let Some(cwd) = &raw.cwd {
                project = cwd.clone();
            }
        }
        if git_branch.is_none() {
            git_branch = raw.git_branch.clone();
        }

        // Parse timestamp
        let timestamp = raw.timestamp.as_ref().and_then(|ts| {
            DateTime::parse_from_rfc3339(ts)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

        if let Some(ts) = timestamp {
            timestamps.push(ts);
        }

        // Parse message type
        let msg_type = match raw.msg_type.as_deref() {
            Some("user") => MessageType::User,
            Some("assistant") => MessageType::Assistant,
            Some("system") => MessageType::System,
            Some("summary") => MessageType::Summary,
            Some("file-history-snapshot") => MessageType::FileHistorySnapshot,
            _ => MessageType::Unknown,
        };

        // Parse tool calls, results, and text from message content
        let (tool_calls, tool_results, text_content) = parse_message_content(&raw.message);

        // Extract token usage from assistant messages
        if let Some(ref msg) = raw.message {
            if let Some(ref usage) = msg.usage {
                // input_tokens + cache_creation_input_tokens = billable input
                token_input += usage.input_tokens.unwrap_or(0);
                token_input += usage.cache_creation_input_tokens.unwrap_or(0);
                token_output += usage.output_tokens.unwrap_or(0);
            }
        }

        messages.push(Message {
            msg_type,
            timestamp,
            tool_calls,
            tool_results,
            text_content,
        });
    }

    // If we couldn't extract a session ID, use filename
    if session_id.is_empty() {
        session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
    }

    // Extract project from path if not found in messages
    if project.is_empty() {
        // Path like: ~/.claude/projects/-Users-rj-personal-projects-ai-editor/abc.jsonl
        if let Some(parent) = path.parent() {
            let dir_name = parent.file_name().unwrap_or_default().to_string_lossy();
            // Decode the path: -Users-rj-... -> /Users/rj/...
            project = dir_name.replace('-', "/");
            if project.starts_with('/') {
                // Already looks like a path
            } else {
                project = format!("/{}", project);
            }
        }
    }

    let start_time = timestamps.iter().min().cloned();
    let end_time = timestamps.iter().max().cloned();

    Some(Session {
        session_id,
        project,
        jsonl_path: path.to_path_buf(),
        git_branch,
        start_time,
        end_time,
        messages,
        token_input,
        token_output,
    })
}

/// Parse tool calls, results, and text content from message content
fn parse_message_content(
    content: &Option<RawMessageContent>,
) -> (Vec<ToolCall>, Vec<ToolResult>, Option<String>) {
    let mut tool_calls = vec![];
    let mut tool_results = vec![];
    let mut text_parts = vec![];

    let content = match content {
        Some(c) => c,
        None => return (tool_calls, tool_results, None),
    };

    let items = match &content.content {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return (tool_calls, tool_results, None),
    };

    for item in items {
        if let Some(obj) = item.as_object() {
            let item_type = obj.get("type").and_then(|v| v.as_str());

            match item_type {
                Some("text") => {
                    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
                Some("tool_use") => {
                    let name = obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let input = obj.get("input").cloned().unwrap_or(serde_json::Value::Null);
                    tool_calls.push(ToolCall { name, input });
                }
                Some("tool_result") => {
                    let tool_use_id = obj
                        .get("tool_use_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let content_val = obj.get("content");
                    let content_str = match content_val {
                        Some(serde_json::Value::String(s)) => s.clone(),
                        Some(v) => v.to_string(),
                        None => String::new(),
                    };
                    let is_error = obj
                        .get("is_error")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    tool_results.push(ToolResult {
                        tool_use_id,
                        content: content_str,
                        is_error,
                    });
                }
                _ => {}
            }
        }
    }

    let text_content = if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n"))
    };

    (tool_calls, tool_results, text_content)
}

/// Load all sessions, optionally filtered by project
pub fn load_sessions(filter_project: Option<&Path>) -> Vec<Session> {
    let files = find_session_files(filter_project);

    files
        .iter()
        .filter_map(|path| parse_session_file(path))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_parsing() {
        assert_eq!(
            match "user" {
                "user" => MessageType::User,
                _ => MessageType::Unknown,
            },
            MessageType::User
        );
    }

    #[test]
    fn test_load_sessions_returns_vec() {
        // Just verify it doesn't crash and returns a Vec
        let sessions = load_sessions(None);
        // sessions may be empty if ~/.claude doesn't exist, that's fine
        // This test just ensures the function runs without panicking
        let _ = sessions;
    }
}
