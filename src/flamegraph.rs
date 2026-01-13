use crate::parser::{MessageType, Session};
use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Activity type for coloring
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivityType {
    Productive, // Edit, Write - making changes
    Reading,    // Read, Grep, Glob - exploring
    Executing,  // Bash commands
    Error,      // Failed operations
    Gap,        // Long pauses
    Thinking,   // Time between user message and response
}

impl ActivityType {
    fn color(&self) -> &'static str {
        match self {
            ActivityType::Productive => "#4ade80", // green
            ActivityType::Reading => "#facc15",    // yellow
            ActivityType::Executing => "#60a5fa",  // blue
            ActivityType::Error => "#f87171",      // red
            ActivityType::Gap => "#9ca3af",        // gray
            ActivityType::Thinking => "#c4b5fd",   // purple
        }
    }

    fn label(&self) -> &'static str {
        match self {
            ActivityType::Productive => "Productive",
            ActivityType::Reading => "Reading/Search",
            ActivityType::Executing => "Executing",
            ActivityType::Error => "Error",
            ActivityType::Gap => "Gap/Pause",
            ActivityType::Thinking => "Thinking",
        }
    }
}

/// A time span with an activity type
#[derive(Debug, Clone)]
pub struct TimeSpan {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub activity: ActivityType,
    pub label: String,
}

/// Extract time spans from a session
pub fn extract_spans(session: &Session) -> Vec<TimeSpan> {
    let mut spans = Vec::new();
    let mut prev_time: Option<DateTime<Utc>> = None;
    let mut current_activity: Option<(DateTime<Utc>, ActivityType, String)> = None;

    for msg in &session.messages {
        let Some(ts) = msg.timestamp else { continue };

        // Check for gaps (>2 min between messages)
        if let Some(prev) = prev_time {
            let gap_secs = (ts - prev).num_seconds();
            if gap_secs > 120 {
                // Close any current activity
                if let Some((start, activity, label)) = current_activity.take() {
                    spans.push(TimeSpan {
                        start,
                        end: prev,
                        activity,
                        label,
                    });
                }
                // Add gap span
                spans.push(TimeSpan {
                    start: prev,
                    end: ts,
                    activity: ActivityType::Gap,
                    label: format!("{:.0}m pause", gap_secs as f64 / 60.0),
                });
            }
        }

        // Determine activity type from message
        let (activity, label) = if msg.msg_type == MessageType::Assistant {
            // Check tool calls
            let mut has_edit = false;
            let mut has_read = false;
            let mut has_bash = false;
            let mut tool_names: Vec<String> = Vec::new();

            for tc in &msg.tool_calls {
                tool_names.push(tc.name.clone());
                match tc.name.as_str() {
                    "Edit" | "Write" | "NotebookEdit" => has_edit = true,
                    "Read" | "Grep" | "Glob" => has_read = true,
                    "Bash" => has_bash = true,
                    _ => {}
                }
            }

            let label = if tool_names.is_empty() {
                "Thinking".to_string()
            } else if tool_names.len() <= 3 {
                tool_names.join(", ")
            } else {
                format!(
                    "{} + {} more",
                    tool_names[..2].join(", "),
                    tool_names.len() - 2
                )
            };

            if has_edit {
                (ActivityType::Productive, label)
            } else if has_bash {
                (ActivityType::Executing, label)
            } else if has_read {
                (ActivityType::Reading, label)
            } else {
                (ActivityType::Thinking, label)
            }
        } else if msg.msg_type == MessageType::User {
            // Check for errors in tool results
            let has_error = msg.tool_results.iter().any(|r| r.is_error);
            if has_error {
                (ActivityType::Error, "Error".to_string())
            } else {
                (ActivityType::Thinking, "User input".to_string())
            }
        } else {
            (ActivityType::Thinking, "System".to_string())
        };

        // Close previous activity if type changed
        if let Some((start, prev_activity, prev_label)) = &current_activity {
            if *prev_activity != activity {
                spans.push(TimeSpan {
                    start: *start,
                    end: ts,
                    activity: *prev_activity,
                    label: prev_label.clone(),
                });
                current_activity = Some((ts, activity, label));
            }
        } else {
            current_activity = Some((ts, activity, label));
        }

        prev_time = Some(ts);
    }

    // Close final activity
    if let Some((start, activity, label)) = current_activity {
        if let Some(end) = session.end_time {
            spans.push(TimeSpan {
                start,
                end,
                activity,
                label,
            });
        }
    }

    spans
}

/// Generate an SVG flamegraph for sessions
pub fn generate_svg(sessions: &[Session], output_path: &Path) -> std::io::Result<()> {
    let width = 1200;
    let row_height = 30;
    let margin = 40;
    let legend_height = 60;

    // Filter sessions with valid times and sort by start time
    let mut valid_sessions: Vec<_> = sessions
        .iter()
        .filter(|s| s.start_time.is_some() && s.end_time.is_some())
        .collect();
    valid_sessions.sort_by_key(|s| s.start_time);

    // Take most recent sessions that fit
    let max_sessions = 20;
    let sessions_to_show: Vec<_> = valid_sessions
        .into_iter()
        .rev()
        .take(max_sessions)
        .rev()
        .collect();

    if sessions_to_show.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No sessions with valid timestamps",
        ));
    }

    let height = margin * 2 + legend_height + (sessions_to_show.len() * row_height);

    let mut svg = String::new();

    // SVG header with styles
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">
<style>
  .session-label {{ font: 11px monospace; fill: #374151; }}
  .time-label {{ font: 10px monospace; fill: #6b7280; }}
  .legend-label {{ font: 12px sans-serif; fill: #374151; }}
  .title {{ font: bold 16px sans-serif; fill: #111827; }}
  rect.span {{ stroke: #fff; stroke-width: 1; }}
  rect.span:hover {{ stroke: #000; stroke-width: 2; opacity: 0.8; }}
</style>
<rect width="100%" height="100%" fill="{}"/>
"#,
        width, height, width, height, "#f9fafb"
    ));

    // Title
    svg.push_str(&format!(
        r#"<text x="{}" y="25" class="title">AI Session Flamegraph</text>"#,
        margin
    ));

    // Legend
    let legend_y = 45;
    let legend_items = [
        (ActivityType::Productive, 0),
        (ActivityType::Reading, 120),
        (ActivityType::Executing, 260),
        (ActivityType::Error, 380),
        (ActivityType::Gap, 470),
        (ActivityType::Thinking, 570),
    ];

    for (activity, x_offset) in legend_items {
        svg.push_str(&format!(
            r#"<rect x="{}" y="{}" width="14" height="14" fill="{}" rx="2"/>
<text x="{}" y="{}" class="legend-label">{}</text>"#,
            margin + x_offset,
            legend_y,
            activity.color(),
            margin + x_offset + 18,
            legend_y + 11,
            activity.label()
        ));
    }

    let chart_y_start = margin + legend_height;
    let chart_width = width - margin * 2 - 150; // Leave room for labels

    // Draw each session
    for (i, session) in sessions_to_show.iter().enumerate() {
        let y = chart_y_start + (i * row_height);
        let session_start = session.start_time.unwrap();
        let session_end = session.end_time.unwrap();
        let session_duration = (session_end - session_start).num_seconds() as f64;

        if session_duration <= 0.0 {
            continue;
        }

        // Session label
        let project_name = extract_project_name(&session.project);
        let duration_str = format_duration(session_duration / 60.0);
        let label = format!(
            "{} ({})",
            &session.session_id[..8.min(session.session_id.len())],
            project_name
        );

        svg.push_str(&format!(
            r#"<text x="{}" y="{}" class="session-label">{}</text>
<text x="{}" y="{}" class="time-label">{}</text>"#,
            margin,
            y + row_height / 2 + 4,
            label,
            width - margin - 50,
            y + row_height / 2 + 4,
            duration_str
        ));

        // Background for session row
        let bar_x = margin + 150;
        svg.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#e5e7eb\" rx=\"2\"/>",
            bar_x,
            y + 2,
            chart_width,
            row_height - 4
        ));

        // Draw spans
        let spans = extract_spans(session);
        for span in &spans {
            let span_start = (span.start - session_start).num_seconds() as f64;
            let span_end = (span.end - session_start).num_seconds() as f64;

            let x = bar_x + (span_start / session_duration * chart_width as f64) as usize;
            let w = ((span_end - span_start) / session_duration * chart_width as f64) as usize;

            if w < 1 {
                continue;
            }

            // Escape label for XML
            let escaped_label = span
                .label
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;");

            let duration_mins = (span.end - span.start).num_seconds() as f64 / 60.0;

            svg.push_str(&format!(
                r#"<rect class="span" x="{}" y="{}" width="{}" height="{}" fill="{}" rx="1">
<title>{}: {} ({:.1}m)</title>
</rect>"#,
                x,
                y + 2,
                w.max(1),
                row_height - 4,
                span.activity.color(),
                span.activity.label(),
                escaped_label,
                duration_mins
            ));
        }
    }

    svg.push_str("</svg>");

    // Write to file
    let mut file = File::create(output_path)?;
    file.write_all(svg.as_bytes())?;

    Ok(())
}

fn extract_project_name(project_path: &str) -> String {
    project_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("unknown")
        .to_string()
}

fn format_duration(minutes: f64) -> String {
    if minutes >= 60.0 {
        format!("{:.1}h", minutes / 60.0)
    } else {
        format!("{:.0}m", minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_colors() {
        assert!(ActivityType::Productive.color().starts_with('#'));
        assert!(ActivityType::Error.color().starts_with('#'));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30m");
        assert_eq!(format_duration(90.0), "1.5h");
    }

    #[test]
    fn test_extract_project_name() {
        assert_eq!(
            extract_project_name("/Users/test/projects/my-app"),
            "my-app"
        );
    }
}
