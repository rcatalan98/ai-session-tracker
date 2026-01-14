use crate::bottlenecks::{detect_all, Bottleneck};
use crate::flamegraph::{extract_spans, generate_svg_by_pr, ActivityType};
use crate::github::{load_cache, RepoCache};
use crate::parser::Session;
use crate::prs::calculate_pr_metrics;
use chrono::{DateTime, Duration, Local, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Filter sessions that belong to a specific GitHub repo
/// by matching session git_branch to PR branches from the cache
pub fn filter_sessions_by_repo(
    sessions: &[Session],
    owner: &str,
    repo: &str,
) -> (Vec<Session>, Option<RepoCache>) {
    let cache = match load_cache(owner, repo) {
        Some(c) => c,
        None => return (vec![], None),
    };

    // Build set of branch names from cached PRs
    let pr_branches: std::collections::HashSet<&str> =
        cache.prs.iter().map(|pr| pr.branch.as_str()).collect();

    // Filter sessions where git_branch matches a PR branch
    let filtered: Vec<Session> = sessions
        .iter()
        .filter(|s| {
            s.git_branch
                .as_ref()
                .map(|b| pr_branches.contains(b.as_str()))
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    (filtered, Some(cache))
}

/// Filter sessions by time period
pub fn filter_sessions_by_period(sessions: &[Session], period: &str) -> Vec<Session> {
    let now = Utc::now();
    let cutoff = match period {
        "day" => now - Duration::days(1),
        "week" => now - Duration::weeks(1),
        "month" => now - Duration::days(30),
        _ => return sessions.to_vec(), // "all"
    };

    sessions
        .iter()
        .filter(|s| s.start_time.map(|t| t > cutoff).unwrap_or(false))
        .cloned()
        .collect()
}

/// Generate an HTML report for the given sessions
pub fn generate_html_report(
    sessions: &[Session],
    cache: &RepoCache,
    output_path: &Path,
) -> Result<(), String> {
    let (start_date, end_date) = get_date_range(sessions);

    // Generate flamegraph SVG
    let flamegraph_svg = generate_flamegraph_svg(sessions);

    // Build HTML content
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>AI Session Report - {owner}/{repo}</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            line-height: 1.6;
            color: #333;
            max-width: 1200px;
            margin: 0 auto;
            padding: 2rem;
            background: #f9fafb;
        }}
        h1 {{ font-size: 2rem; color: #111; margin-bottom: 0.25rem; }}
        h2 {{ font-size: 1.25rem; color: #374151; margin: 2rem 0 1rem; border-bottom: 2px solid #e5e7eb; padding-bottom: 0.5rem; }}
        .subtitle {{ color: #6b7280; font-size: 1rem; margin-bottom: 0.5rem; }}
        .date-range {{ color: #9ca3af; font-size: 0.875rem; margin-bottom: 2rem; }}
        .card {{ background: white; border-radius: 8px; padding: 1.5rem; margin-bottom: 1.5rem; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }}
        .stats-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 1rem; }}
        .stat {{ text-align: center; padding: 1rem; }}
        .stat-value {{ font-size: 2rem; font-weight: bold; color: #111; }}
        .stat-label {{ font-size: 0.875rem; color: #6b7280; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 0.75rem 1rem; text-align: left; border-bottom: 1px solid #e5e7eb; }}
        th {{ background: #f9fafb; font-weight: 600; color: #374151; }}
        tr:hover {{ background: #f9fafb; }}
        .bar-container {{ width: 100%; background: #e5e7eb; border-radius: 4px; height: 8px; }}
        .bar {{ height: 100%; border-radius: 4px; }}
        .bar-productive {{ background: #4ade80; }}
        .bar-reading {{ background: #facc15; }}
        .bar-executing {{ background: #60a5fa; }}
        .bar-error {{ background: #f87171; }}
        .bar-gap {{ background: #9ca3af; }}
        .bar-thinking {{ background: #c4b5fd; }}
        .recommendation {{ padding: 0.75rem 1rem; margin: 0.5rem 0; background: #f0f9ff; border-left: 4px solid #3b82f6; border-radius: 0 4px 4px 0; }}
        .flamegraph-container {{ margin-top: 1rem; overflow-x: auto; }}
        .flamegraph-container svg {{ max-width: 100%; height: auto; }}
        .footer {{ margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #e5e7eb; color: #9ca3af; font-size: 0.875rem; text-align: center; }}
    </style>
</head>
<body>
    <h1>{owner}/{repo}</h1>
    <p class="subtitle">AI Session Report</p>
    <p class="date-range">{start_date} — {end_date}</p>

    {summary_section}

    {time_breakdown_section}

    {bottlenecks_section}

    {pr_breakdown_section}

    {recommendations_section}

    {flamegraph_section}

    <div class="footer">
        Generated by <strong>aist</strong> (AI Session Tracker) • {generated_at}
    </div>
</body>
</html>"#,
        owner = cache.owner,
        repo = cache.repo,
        start_date = start_date,
        end_date = end_date,
        summary_section = generate_summary_section(sessions),
        time_breakdown_section = generate_time_breakdown_section(sessions),
        bottlenecks_section = generate_bottlenecks_section(sessions),
        pr_breakdown_section = generate_pr_breakdown_section(sessions, cache),
        recommendations_section = generate_recommendations_section(sessions),
        flamegraph_section = generate_flamegraph_section(&flamegraph_svg),
        generated_at = Local::now().format("%Y-%m-%d %H:%M"),
    );

    fs::write(output_path, html).map_err(|e| format!("Failed to write HTML: {}", e))?;

    Ok(())
}

fn get_date_range(sessions: &[Session]) -> (String, String) {
    let min_time = sessions
        .iter()
        .filter_map(|s| s.start_time)
        .min()
        .unwrap_or_else(Utc::now);
    let max_time = sessions
        .iter()
        .filter_map(|s| s.end_time)
        .max()
        .unwrap_or_else(Utc::now);

    let format_date = |dt: DateTime<Utc>| -> String {
        let local: DateTime<Local> = dt.with_timezone(&Local);
        local.format("%B %d, %Y").to_string()
    };

    (format_date(min_time), format_date(max_time))
}

fn generate_summary_section(sessions: &[Session]) -> String {
    let total_sessions = sessions.len();
    let total_minutes: f64 = sessions
        .iter()
        .filter_map(|s| match (s.start_time, s.end_time) {
            (Some(start), Some(end)) => Some((end - start).num_minutes() as f64),
            _ => None,
        })
        .sum();

    // Calculate efficiency
    let mut productive_time = 0.0;
    let mut error_time = 0.0;
    let mut gap_time = 0.0;

    for session in sessions {
        for span in extract_spans(session) {
            let duration = (span.end - span.start).num_seconds() as f64 / 60.0;
            match span.activity {
                ActivityType::Error => error_time += duration,
                ActivityType::Gap => gap_time += duration,
                _ => productive_time += duration,
            }
        }
    }

    let total_span_time = productive_time + error_time + gap_time;
    let efficiency = if total_span_time > 0.0 {
        ((productive_time / total_span_time) * 100.0) as u32
    } else {
        0
    };

    format!(
        r#"<h2>Summary</h2>
    <div class="card">
        <div class="stats-grid">
            <div class="stat">
                <div class="stat-value">{sessions}</div>
                <div class="stat-label">Sessions</div>
            </div>
            <div class="stat">
                <div class="stat-value">{time}</div>
                <div class="stat-label">Total Time</div>
            </div>
            <div class="stat">
                <div class="stat-value">{efficiency}%</div>
                <div class="stat-label">Efficiency</div>
            </div>
        </div>
    </div>"#,
        sessions = total_sessions,
        time = format_duration(total_minutes),
        efficiency = efficiency,
    )
}

fn generate_time_breakdown_section(sessions: &[Session]) -> String {
    let mut time_by_activity: HashMap<ActivityType, f64> = HashMap::new();

    for session in sessions {
        for span in extract_spans(session) {
            let duration = (span.end - span.start).num_seconds() as f64 / 60.0;
            *time_by_activity.entry(span.activity).or_insert(0.0) += duration;
        }
    }

    let total_time: f64 = time_by_activity.values().sum();

    if total_time == 0.0 {
        return r#"<h2>Time Breakdown</h2><div class="card"><p>No activity data available.</p></div>"#.to_string();
    }

    let mut activities: Vec<_> = time_by_activity.into_iter().collect();
    activities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let rows: String = activities
        .iter()
        .map(|(activity, minutes)| {
            let percentage = (minutes / total_time * 100.0) as u32;
            let (name, bar_class) = match activity {
                ActivityType::Productive => ("Productive", "bar-productive"),
                ActivityType::Reading => ("Reading/Search", "bar-reading"),
                ActivityType::Executing => ("Executing", "bar-executing"),
                ActivityType::Error => ("Error", "bar-error"),
                ActivityType::Gap => ("Gap/Pause", "bar-gap"),
                ActivityType::Thinking => ("Thinking", "bar-thinking"),
            };
            format!(
                r#"<tr>
                <td>{name}</td>
                <td>{time}</td>
                <td>{percentage}%</td>
                <td><div class="bar-container"><div class="bar {bar_class}" style="width: {percentage}%"></div></div></td>
            </tr>"#,
                name = name,
                time = format_duration(*minutes),
                percentage = percentage,
                bar_class = bar_class,
            )
        })
        .collect();

    format!(
        r#"<h2>Time Breakdown</h2>
    <div class="card">
        <table>
            <thead>
                <tr><th>Activity</th><th>Time</th><th>%</th><th>Distribution</th></tr>
            </thead>
            <tbody>
                {rows}
            </tbody>
        </table>
    </div>"#,
        rows = rows
    )
}

fn generate_bottlenecks_section(sessions: &[Session]) -> String {
    let bottlenecks = detect_all(sessions);

    if bottlenecks.is_empty() {
        return r#"<h2>Bottlenecks</h2><div class="card"><p>No significant bottlenecks detected.</p></div>"#.to_string();
    }

    // Aggregate by type
    let mut by_type: HashMap<String, (usize, f64)> = HashMap::new();
    for b in &bottlenecks {
        let (type_name, duration) = match b {
            Bottleneck::ErrorLoop(e) => ("Error Loop", e.duration_minutes),
            Bottleneck::ExplorationSpiral(e) => ("Exploration Spiral", e.duration_minutes),
            Bottleneck::EditThrashing(e) => ("Edit Thrashing", e.duration_minutes),
            Bottleneck::LongGap(g) => ("Long Gap", g.gap_minutes),
        };
        let entry = by_type.entry(type_name.to_string()).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += duration;
    }

    let mut types: Vec<_> = by_type.into_iter().collect();
    types.sort_by(|a, b| {
        b.1 .1
            .partial_cmp(&a.1 .1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let rows: String = types
        .iter()
        .take(5)
        .map(|(type_name, (count, duration))| {
            format!(
                r#"<tr><td>{type_name}</td><td>{count}</td><td>{duration}</td></tr>"#,
                type_name = type_name,
                count = count,
                duration = format_duration(*duration),
            )
        })
        .collect();

    format!(
        r#"<h2>Bottlenecks</h2>
    <div class="card">
        <table>
            <thead>
                <tr><th>Type</th><th>Count</th><th>Time Lost</th></tr>
            </thead>
            <tbody>
                {rows}
            </tbody>
        </table>
    </div>"#,
        rows = rows
    )
}

fn generate_pr_breakdown_section(sessions: &[Session], cache: &RepoCache) -> String {
    let pr_metrics = calculate_pr_metrics(sessions, cache);

    if pr_metrics.is_empty() {
        return r#"<h2>PR Breakdown</h2><div class="card"><p>No PRs found with matching sessions.</p></div>"#.to_string();
    }

    let rows: String = pr_metrics
        .iter()
        .take(10)
        .map(|m| {
            let title_display = if m.title.len() > 50 {
                format!("{}...", &m.title[..47])
            } else {
                m.title.clone()
            };
            format!(
                r#"<tr><td>#{number}</td><td>{title}</td><td>{time}</td><td>{sessions}</td></tr>"#,
                number = m.pr_number,
                title = html_escape(&title_display),
                time = format_duration(m.total_minutes),
                sessions = m.session_count,
            )
        })
        .collect();

    format!(
        r#"<h2>PR Breakdown</h2>
    <div class="card">
        <table>
            <thead>
                <tr><th>PR</th><th>Title</th><th>Time</th><th>Sessions</th></tr>
            </thead>
            <tbody>
                {rows}
            </tbody>
        </table>
    </div>"#,
        rows = rows
    )
}

fn generate_recommendations_section(sessions: &[Session]) -> String {
    let bottlenecks = detect_all(sessions);

    let mut error_loops = 0;
    let mut exploration_spirals = 0;
    let mut edit_thrashing = 0;
    let mut long_gaps = 0;

    for b in &bottlenecks {
        match b {
            Bottleneck::ErrorLoop(_) => error_loops += 1,
            Bottleneck::ExplorationSpiral(_) => exploration_spirals += 1,
            Bottleneck::EditThrashing(_) => edit_thrashing += 1,
            Bottleneck::LongGap(_) => long_gaps += 1,
        }
    }

    let mut recommendations: Vec<String> = vec![];

    if error_loops > 2 {
        recommendations.push(format!(
            "<strong>{} error loops</strong> detected. Consider providing more context or examples when asking for help.",
            error_loops
        ));
    }

    if exploration_spirals > 2 {
        recommendations.push(format!(
            "<strong>{} exploration spirals</strong> found. Try giving the AI direct file paths instead of letting it search.",
            exploration_spirals
        ));
    }

    if edit_thrashing > 2 {
        recommendations.push(format!(
            "<strong>{} edit thrashing</strong> instances. Consider reviewing changes before committing to avoid repeated fixes.",
            edit_thrashing
        ));
    }

    if long_gaps > 3 {
        recommendations.push(format!(
            "<strong>{} long gaps</strong> detected. Break complex tasks into smaller chunks for better focus.",
            long_gaps
        ));
    }

    if recommendations.is_empty() {
        recommendations.push("No major issues detected. Keep up the good work!".to_string());
    }

    let items: String = recommendations
        .iter()
        .map(|r| format!(r#"<div class="recommendation">{}</div>"#, r))
        .collect();

    format!(
        r#"<h2>Recommendations</h2>
    <div class="card">
        {items}
    </div>"#,
        items = items
    )
}

fn generate_flamegraph_svg(sessions: &[Session]) -> Option<String> {
    let temp_svg = std::env::temp_dir().join("aist-flamegraph-temp.svg");

    if generate_svg_by_pr(sessions, &temp_svg).is_err() {
        return None;
    }

    let svg_content = fs::read_to_string(&temp_svg).ok();
    let _ = fs::remove_file(&temp_svg);

    svg_content
}

fn generate_flamegraph_section(svg: &Option<String>) -> String {
    match svg {
        Some(svg_content) => {
            format!(
                r#"<h2>Session Flamegraph</h2>
    <div class="card">
        <div class="flamegraph-container">
            {svg}
        </div>
    </div>"#,
                svg = svg_content
            )
        }
        None => r#"<h2>Session Flamegraph</h2>
    <div class="card">
        <p>Could not generate flamegraph (no PR data available).</p>
    </div>"#
            .to_string(),
    }
}

fn format_duration(minutes: f64) -> String {
    if minutes >= 60.0 {
        let hours = (minutes / 60.0).floor();
        let mins = (minutes % 60.0).round();
        format!("{}h {}m", hours as u32, mins as u32)
    } else {
        format!("{}m", minutes.round() as u32)
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::PrMapping;
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;

    fn make_session(id: &str, branch: Option<&str>, duration_mins: i64) -> Session {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let end = start + chrono::Duration::minutes(duration_mins);
        Session {
            session_id: id.to_string(),
            project: "/test/project".to_string(),
            jsonl_path: PathBuf::from("/test/session.jsonl"),
            git_branch: branch.map(|s| s.to_string()),
            start_time: Some(start),
            end_time: Some(end),
            messages: vec![],
        }
    }

    #[allow(dead_code)]
    fn make_cache(prs: Vec<PrMapping>) -> RepoCache {
        RepoCache {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            prs,
            synced_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30m");
        assert_eq!(format_duration(60.0), "1h 0m");
        assert_eq!(format_duration(90.0), "1h 30m");
    }

    #[test]
    fn test_filter_sessions_by_period_all() {
        let sessions = vec![
            make_session("s1", Some("main"), 30),
            make_session("s2", Some("feature"), 45),
        ];
        let filtered = filter_sessions_by_period(&sessions, "all");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }
}
