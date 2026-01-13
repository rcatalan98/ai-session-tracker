use crate::flamegraph::{extract_spans, ActivityType};
use crate::github::{load_current_repo_cache, PrMapping, RepoCache};
use crate::parser::Session;
use chrono::{DateTime, Local, Utc};
use colored::Colorize;
use std::collections::HashMap;

/// Time metrics for a single GitHub issue
#[derive(Debug, Clone)]
#[allow(dead_code)] // branch field used in US-003 issue detail
pub struct IssueMetrics {
    pub issue_number: u32,
    pub title: String,
    pub branch: String,
    pub total_minutes: f64,
    pub session_count: usize,
}

/// Calculate time spent per issue by matching sessions to PR branches
pub fn calculate_issue_metrics(sessions: &[Session], cache: &RepoCache) -> Vec<IssueMetrics> {
    // Build branch -> PR mapping (a branch can only have one PR)
    let branch_to_pr: HashMap<&str, &PrMapping> = cache
        .prs
        .iter()
        .map(|pr| (pr.branch.as_str(), pr))
        .collect();

    // Build issue -> (title, branch, minutes, session_count)
    let mut issue_metrics: HashMap<u32, (String, String, f64, usize)> = HashMap::new();

    for session in sessions {
        let branch = match &session.git_branch {
            Some(b) => b.as_str(),
            None => continue,
        };

        // Find the PR for this branch
        let pr = match branch_to_pr.get(branch) {
            Some(pr) => pr,
            None => continue,
        };

        // Skip PRs with no linked issues
        if pr.closed_issues.is_empty() {
            continue;
        }

        // Calculate session duration
        let duration_minutes = match (session.start_time, session.end_time) {
            (Some(start), Some(end)) => (end - start).num_minutes() as f64,
            _ => 0.0,
        };

        // Add time to each linked issue
        for &issue_num in &pr.closed_issues {
            let entry = issue_metrics
                .entry(issue_num)
                .or_insert_with(|| (pr.title.clone(), pr.branch.clone(), 0.0, 0));
            entry.2 += duration_minutes;
            entry.3 += 1;
        }
    }

    // Convert to Vec and sort by total time descending
    let mut metrics: Vec<IssueMetrics> = issue_metrics
        .into_iter()
        .map(
            |(issue_number, (title, branch, total_minutes, session_count))| IssueMetrics {
                issue_number,
                title,
                branch,
                total_minutes,
                session_count,
            },
        )
        .collect();

    metrics.sort_by(|a, b| {
        b.total_minutes
            .partial_cmp(&a.total_minutes)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    metrics
}

/// Format duration in minutes to human-readable string
fn format_duration(minutes: f64) -> String {
    if minutes >= 60.0 {
        let hours = (minutes / 60.0).floor();
        let mins = (minutes % 60.0).round();
        format!("{}h {}m", hours as u32, mins as u32)
    } else {
        format!("{}m", minutes.round() as u32)
    }
}

/// List all issues with time metrics
pub fn list_issues(sessions: &[Session]) {
    // Load GitHub cache
    let cache = match load_current_repo_cache() {
        Some(c) => c,
        None => {
            println!(
                "{}: No GitHub cache found. Run `aist sync` first.",
                "Error".red()
            );
            return;
        }
    };

    let metrics = calculate_issue_metrics(sessions, &cache);

    if metrics.is_empty() {
        println!("{}", "No issues found with matching sessions.".yellow());
        println!(
            "{}",
            "Tip: Make sure PR branches match session git branches.".dimmed()
        );
        return;
    }

    // Calculate totals
    let total_time: f64 = metrics.iter().map(|m| m.total_minutes).sum();
    let total_sessions: usize = metrics.iter().map(|m| m.session_count).sum();

    // Header
    println!("{}", "ISSUES BY TIME".bold());
    println!("{}", "═".repeat(70));
    println!(
        "{} issues | {} sessions | {} total\n",
        metrics.len().to_string().bold(),
        total_sessions.to_string().bold(),
        format_duration(total_time).bold()
    );

    // Column headers
    println!(
        "{:<8} {:<40} {:>10} {:>10}",
        "ISSUE".dimmed(),
        "TITLE".dimmed(),
        "TIME".dimmed(),
        "SESSIONS".dimmed()
    );
    println!("{}", "─".repeat(70).dimmed());

    // List issues
    for m in &metrics {
        let title_display = if m.title.len() > 38 {
            format!("{}...", &m.title[..35])
        } else {
            m.title.clone()
        };

        println!(
            "#{:<7} {:<40} {:>10} {:>10}",
            m.issue_number,
            title_display,
            format_duration(m.total_minutes),
            m.session_count
        );
    }

    println!("{}", "─".repeat(70).dimmed());
    println!(
        "{:<8} {:<40} {:>10} {:>10}",
        "TOTAL".bold(),
        "",
        format_duration(total_time).bold(),
        total_sessions.to_string().bold()
    );
}

/// Session info for a specific issue
#[derive(Debug)]
struct IssueSession<'a> {
    session: &'a Session,
    duration_minutes: f64,
}

/// Show detailed metrics for a specific issue
pub fn show_issue_detail(issue_number: u32, sessions: &[Session]) {
    // Load GitHub cache
    let cache = match load_current_repo_cache() {
        Some(c) => c,
        None => {
            println!(
                "{}: No GitHub cache found. Run `aist sync` first.",
                "Error".red()
            );
            return;
        }
    };

    // Find the PR that closes this issue
    let pr = cache
        .prs
        .iter()
        .find(|p| p.closed_issues.contains(&issue_number));

    let pr = match pr {
        Some(p) => p,
        None => {
            println!(
                "{}: Issue #{} not found in synced PRs.",
                "Error".red(),
                issue_number
            );
            println!("{}", "Tip: Run `aist sync` to update PR cache.".dimmed());
            return;
        }
    };

    // Find sessions matching this PR's branch
    let mut issue_sessions: Vec<IssueSession> = sessions
        .iter()
        .filter(|s| s.git_branch.as_deref() == Some(&pr.branch))
        .map(|s| {
            let duration = match (s.start_time, s.end_time) {
                (Some(start), Some(end)) => (end - start).num_minutes() as f64,
                _ => 0.0,
            };
            IssueSession {
                session: s,
                duration_minutes: duration,
            }
        })
        .collect();

    // Sort by start time
    issue_sessions.sort_by(|a, b| a.session.start_time.cmp(&b.session.start_time));

    // Calculate totals
    let total_time: f64 = issue_sessions.iter().map(|s| s.duration_minutes).sum();
    let session_count = issue_sessions.len();

    // Determine status
    let status = if pr.merged_at.is_some() {
        "Merged".green()
    } else {
        "Open".yellow()
    };

    // Print header
    println!("{}", format!("ISSUE #{}", issue_number).bold());
    println!("{}", "═".repeat(70));
    println!();

    // Issue metadata
    println!("{}: {}", "Title".dimmed(), pr.title);
    println!("{}: {}", "Status".dimmed(), status);
    println!("{}: {}", "Branch".dimmed(), pr.branch);
    println!(
        "{}: {}",
        "Total time".dimmed(),
        format_duration(total_time).bold()
    );
    println!("{}: {}", "Sessions".dimmed(), session_count);
    println!();

    if issue_sessions.is_empty() {
        println!(
            "{}",
            "No sessions found matching this issue's branch.".yellow()
        );
        return;
    }

    // Session list
    println!("{}", "SESSIONS".bold());
    println!("{}", "─".repeat(70).dimmed());
    println!(
        "{:<20} {:<12} {:>10} {:>26}",
        "SESSION".dimmed(),
        "".dimmed(),
        "DURATION".dimmed(),
        "TIMESTAMP".dimmed()
    );
    println!("{}", "─".repeat(70).dimmed());

    for issue_session in &issue_sessions {
        let session = issue_session.session;
        let session_short: String = session.session_id.chars().take(18).collect();
        let duration_str = format_duration(issue_session.duration_minutes);
        let timestamp_str = session
            .start_time
            .map(|t| format_timestamp(&t))
            .unwrap_or_else(|| "-".to_string());

        println!(
            "{:<20} {:<12} {:>10} {:>26}",
            session_short, "", duration_str, timestamp_str
        );
    }

    println!("{}", "─".repeat(70).dimmed());
    println!();

    // Activity breakdown
    print_activity_breakdown(&issue_sessions);
}

/// Format timestamp for display
fn format_timestamp(ts: &DateTime<Utc>) -> String {
    let local: DateTime<Local> = ts.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M").to_string()
}

/// Print time breakdown by activity type
fn print_activity_breakdown(issue_sessions: &[IssueSession]) {
    println!("{}", "ACTIVITY BREAKDOWN".bold());
    println!("{}", "─".repeat(70).dimmed());

    // Collect all spans from all sessions
    let mut time_by_activity: HashMap<ActivityType, f64> = HashMap::new();
    let mut total_span_time = 0.0;

    for issue_session in issue_sessions {
        let spans = extract_spans(issue_session.session);
        for span in spans {
            let duration_mins = (span.end - span.start).num_seconds() as f64 / 60.0;
            *time_by_activity.entry(span.activity).or_insert(0.0) += duration_mins;
            total_span_time += duration_mins;
        }
    }

    if total_span_time == 0.0 {
        println!("{}", "No activity data available.".yellow());
        return;
    }

    // Sort by time descending
    let mut activities: Vec<_> = time_by_activity.into_iter().collect();
    activities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Print each activity with a simple bar
    for (activity, minutes) in &activities {
        let percentage = (*minutes / total_span_time * 100.0) as usize;
        let bar_width = (percentage / 2).clamp(1, 30); // scale to ~30 chars max
        let bar: String = "█".repeat(bar_width);

        let activity_name = match activity {
            ActivityType::Productive => "Productive",
            ActivityType::Reading => "Reading/Search",
            ActivityType::Executing => "Executing",
            ActivityType::Error => "Error",
            ActivityType::Gap => "Gap/Pause",
            ActivityType::Thinking => "Thinking",
        };

        let colored_bar = match activity {
            ActivityType::Productive => bar.green(),
            ActivityType::Reading => bar.yellow(),
            ActivityType::Executing => bar.blue(),
            ActivityType::Error => bar.red(),
            ActivityType::Gap => bar.dimmed(),
            ActivityType::Thinking => bar.purple(),
        };

        println!(
            "{:<14} {} {:>6} ({:>2}%)",
            activity_name,
            colored_bar,
            format_duration(*minutes),
            percentage
        );
    }
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

    fn make_cache(prs: Vec<PrMapping>) -> RepoCache {
        RepoCache {
            owner: "test".to_string(),
            repo: "repo".to_string(),
            prs,
            synced_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_calculate_issue_metrics_basic() {
        let sessions = vec![
            make_session("s1", Some("feature/issue-1"), 30),
            make_session("s2", Some("feature/issue-1"), 45),
            make_session("s3", Some("fix/issue-2"), 20),
        ];

        let cache = make_cache(vec![
            PrMapping {
                pr_number: 10,
                title: "Feature PR".to_string(),
                branch: "feature/issue-1".to_string(),
                closed_issues: vec![1],
                merged_at: None,
            },
            PrMapping {
                pr_number: 11,
                title: "Fix PR".to_string(),
                branch: "fix/issue-2".to_string(),
                closed_issues: vec![2],
                merged_at: None,
            },
        ]);

        let metrics = calculate_issue_metrics(&sessions, &cache);

        assert_eq!(metrics.len(), 2);
        // Sorted by time descending, issue 1 has 75 mins
        assert_eq!(metrics[0].issue_number, 1);
        assert_eq!(metrics[0].total_minutes, 75.0);
        assert_eq!(metrics[0].session_count, 2);

        assert_eq!(metrics[1].issue_number, 2);
        assert_eq!(metrics[1].total_minutes, 20.0);
        assert_eq!(metrics[1].session_count, 1);
    }

    #[test]
    fn test_calculate_issue_metrics_no_branch() {
        let sessions = vec![make_session("s1", None, 30)];

        let cache = make_cache(vec![PrMapping {
            pr_number: 10,
            title: "PR".to_string(),
            branch: "feature/x".to_string(),
            closed_issues: vec![1],
            merged_at: None,
        }]);

        let metrics = calculate_issue_metrics(&sessions, &cache);
        assert!(metrics.is_empty());
    }

    #[test]
    fn test_calculate_issue_metrics_no_matching_pr() {
        let sessions = vec![make_session("s1", Some("unrelated-branch"), 30)];

        let cache = make_cache(vec![PrMapping {
            pr_number: 10,
            title: "PR".to_string(),
            branch: "feature/x".to_string(),
            closed_issues: vec![1],
            merged_at: None,
        }]);

        let metrics = calculate_issue_metrics(&sessions, &cache);
        assert!(metrics.is_empty());
    }

    #[test]
    fn test_calculate_issue_metrics_pr_no_issues() {
        let sessions = vec![make_session("s1", Some("feature/x"), 30)];

        let cache = make_cache(vec![PrMapping {
            pr_number: 10,
            title: "PR without issue link".to_string(),
            branch: "feature/x".to_string(),
            closed_issues: vec![], // No linked issues
            merged_at: None,
        }]);

        let metrics = calculate_issue_metrics(&sessions, &cache);
        assert!(metrics.is_empty());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30m");
        assert_eq!(format_duration(60.0), "1h 0m");
        assert_eq!(format_duration(90.0), "1h 30m");
        assert_eq!(format_duration(125.0), "2h 5m");
    }

    #[test]
    fn test_format_timestamp() {
        let ts = Utc.with_ymd_and_hms(2026, 1, 15, 14, 30, 0).unwrap();
        let formatted = format_timestamp(&ts);
        // Should contain the date and time components
        assert!(formatted.contains("2026"));
        assert!(formatted.contains("01"));
        assert!(formatted.contains("15"));
    }

    #[test]
    fn test_issue_session_duration() {
        let session = make_session("test-session", Some("feature/issue-5"), 45);
        let issue_session = IssueSession {
            session: &session,
            duration_minutes: 45.0,
        };
        assert_eq!(issue_session.duration_minutes, 45.0);
        assert_eq!(
            issue_session.session.git_branch,
            Some("feature/issue-5".to_string())
        );
    }
}
