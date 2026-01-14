use crate::cost::calculate_cost;
use crate::flamegraph::{extract_spans, ActivityType};
use crate::github::{load_current_repo_cache, PrMapping, RepoCache};
use crate::parser::Session;
use chrono::{DateTime, Local, Utc};
use colored::Colorize;
use std::collections::HashMap;

/// Time metrics for a single GitHub PR
#[derive(Debug, Clone)]
#[allow(dead_code)] // branch and merged_at used in PR detail view
pub struct PrMetrics {
    pub pr_number: u32,
    pub title: String,
    pub branch: String,
    pub total_minutes: f64,
    pub session_count: usize,
    pub merged_at: Option<String>,
    pub closed_issues: Vec<u32>,
    pub cost: f64,
}

/// Calculate time spent per PR by matching sessions to PR branches
pub fn calculate_pr_metrics(sessions: &[Session], cache: &RepoCache) -> Vec<PrMetrics> {
    // Build branch -> PR mapping
    let branch_to_pr: HashMap<&str, &PrMapping> = cache
        .prs
        .iter()
        .map(|pr| (pr.branch.as_str(), pr))
        .collect();

    // Build PR -> (minutes, session_count, input_tokens, output_tokens)
    let mut pr_metrics: HashMap<u32, (f64, usize, u64, u64)> = HashMap::new();

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

        // Calculate session duration
        let duration_minutes = match (session.start_time, session.end_time) {
            (Some(start), Some(end)) => (end - start).num_minutes() as f64,
            _ => 0.0,
        };

        // Add time and tokens to this PR
        let entry = pr_metrics.entry(pr.pr_number).or_insert((0.0, 0, 0, 0));
        entry.0 += duration_minutes;
        entry.1 += 1;
        entry.2 += session.token_input;
        entry.3 += session.token_output;
    }

    // Convert to Vec with PR info
    let mut metrics: Vec<PrMetrics> = pr_metrics
        .into_iter()
        .filter_map(
            |(pr_number, (total_minutes, session_count, input_tokens, output_tokens))| {
                // Find the PR to get its metadata
                cache
                    .prs
                    .iter()
                    .find(|p| p.pr_number == pr_number)
                    .map(|pr| PrMetrics {
                        pr_number,
                        title: pr.title.clone(),
                        branch: pr.branch.clone(),
                        total_minutes,
                        session_count,
                        merged_at: pr.merged_at.clone(),
                        closed_issues: pr.closed_issues.clone(),
                        cost: calculate_cost(input_tokens, output_tokens),
                    })
            },
        )
        .collect();

    // Sort by total time descending
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

/// Format cost as USD
fn format_cost(cost: f64) -> String {
    if cost < 0.01 {
        format!("${:.4}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// List all PRs with time metrics
pub fn list_prs(sessions: &[Session]) {
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

    let metrics = calculate_pr_metrics(sessions, &cache);

    if metrics.is_empty() {
        println!("{}", "No PRs found with matching sessions.".yellow());
        println!(
            "{}",
            "Tip: Make sure PR branches match session git branches.".dimmed()
        );
        return;
    }

    // Calculate totals
    let total_time: f64 = metrics.iter().map(|m| m.total_minutes).sum();
    let total_sessions: usize = metrics.iter().map(|m| m.session_count).sum();
    let total_cost: f64 = metrics.iter().map(|m| m.cost).sum();

    // Header
    println!("{}", "PRS BY TIME".bold());
    println!("{}", "═".repeat(90));
    println!(
        "{} PRs | {} sessions | {} total | {} cost\n",
        metrics.len().to_string().bold(),
        total_sessions.to_string().bold(),
        format_duration(total_time).bold(),
        format_cost(total_cost).green().bold()
    );

    // Column headers
    println!(
        "{:<8} {:<38} {:>10} {:>8} {:>10} {:>8}",
        "PR".dimmed(),
        "TITLE".dimmed(),
        "TIME".dimmed(),
        "SESSIONS".dimmed(),
        "COST".dimmed(),
        "ISSUES".dimmed()
    );
    println!("{}", "─".repeat(90).dimmed());

    // List PRs
    for m in &metrics {
        let title_display = if m.title.len() > 36 {
            format!("{}...", &m.title[..33])
        } else {
            m.title.clone()
        };

        let issues_str = if m.closed_issues.is_empty() {
            "-".to_string()
        } else {
            m.closed_issues
                .iter()
                .map(|i| format!("#{}", i))
                .collect::<Vec<_>>()
                .join(",")
        };

        let issues_display = if issues_str.len() > 8 {
            format!("{}+", m.closed_issues.len())
        } else {
            issues_str
        };

        println!(
            "#{:<7} {:<38} {:>10} {:>8} {:>10} {:>8}",
            m.pr_number,
            title_display,
            format_duration(m.total_minutes),
            m.session_count,
            format_cost(m.cost),
            issues_display
        );
    }

    println!("{}", "─".repeat(90).dimmed());
    println!(
        "{:<8} {:<38} {:>10} {:>8} {:>10}",
        "TOTAL".bold(),
        "",
        format_duration(total_time).bold(),
        total_sessions.to_string().bold(),
        format_cost(total_cost).green().bold()
    );
}

/// Session info for a specific PR
#[derive(Debug)]
struct PrSession<'a> {
    session: &'a Session,
    duration_minutes: f64,
}

/// Show detailed metrics for a specific PR
pub fn show_pr_detail(pr_number: u32, sessions: &[Session]) {
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

    // Find the PR
    let pr = cache.prs.iter().find(|p| p.pr_number == pr_number);

    let pr = match pr {
        Some(p) => p,
        None => {
            println!(
                "{}: PR #{} not found in synced PRs.",
                "Error".red(),
                pr_number
            );
            println!("{}", "Tip: Run `aist sync` to update PR cache.".dimmed());
            return;
        }
    };

    // Find sessions matching this PR's branch
    let mut pr_sessions: Vec<PrSession> = sessions
        .iter()
        .filter(|s| s.git_branch.as_deref() == Some(&pr.branch))
        .map(|s| {
            let duration = match (s.start_time, s.end_time) {
                (Some(start), Some(end)) => (end - start).num_minutes() as f64,
                _ => 0.0,
            };
            PrSession {
                session: s,
                duration_minutes: duration,
            }
        })
        .collect();

    // Sort by start time
    pr_sessions.sort_by(|a, b| a.session.start_time.cmp(&b.session.start_time));

    // Calculate totals
    let total_time: f64 = pr_sessions.iter().map(|s| s.duration_minutes).sum();
    let session_count = pr_sessions.len();
    let total_input: u64 = pr_sessions.iter().map(|s| s.session.token_input).sum();
    let total_output: u64 = pr_sessions.iter().map(|s| s.session.token_output).sum();
    let total_cost = calculate_cost(total_input, total_output);

    // Determine status
    let status = if pr.merged_at.is_some() {
        "Merged".green()
    } else {
        "Open".yellow()
    };

    // Print header
    println!("{}", format!("PR #{}", pr_number).bold());
    println!("{}", "═".repeat(70));
    println!();

    // PR metadata
    println!("{}: {}", "Title".dimmed(), pr.title);
    println!("{}: {}", "Status".dimmed(), status);
    println!("{}: {}", "Branch".dimmed(), pr.branch);

    // Closed issues
    if !pr.closed_issues.is_empty() {
        let issues_str = pr
            .closed_issues
            .iter()
            .map(|i| format!("#{}", i))
            .collect::<Vec<_>>()
            .join(", ");
        println!("{}: {}", "Closes".dimmed(), issues_str);
    }

    println!(
        "{}: {}",
        "Total time".dimmed(),
        format_duration(total_time).bold()
    );
    println!("{}: {}", "Sessions".dimmed(), session_count);
    println!(
        "{}: {}",
        "Cost".dimmed(),
        format_cost(total_cost).green().bold()
    );
    println!();

    if pr_sessions.is_empty() {
        println!(
            "{}",
            "No sessions found matching this PR's branch.".yellow()
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

    for pr_session in &pr_sessions {
        let session = pr_session.session;
        let session_short: String = session.session_id.chars().take(18).collect();
        let duration_str = format_duration(pr_session.duration_minutes);
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
    print_activity_breakdown(&pr_sessions);
}

/// Format timestamp for display
fn format_timestamp(ts: &DateTime<Utc>) -> String {
    let local: DateTime<Local> = ts.with_timezone(&Local);
    local.format("%Y-%m-%d %H:%M").to_string()
}

/// Print time breakdown by activity type
fn print_activity_breakdown(pr_sessions: &[PrSession]) {
    println!("{}", "ACTIVITY BREAKDOWN".bold());
    println!("{}", "─".repeat(70).dimmed());

    // Collect all spans from all sessions
    let mut time_by_activity: HashMap<ActivityType, f64> = HashMap::new();
    let mut total_span_time = 0.0;

    for pr_session in pr_sessions {
        let spans = extract_spans(pr_session.session);
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
        let bar_width = (percentage / 2).clamp(1, 30);
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
            token_input: 0,
            token_output: 0,
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
    fn test_calculate_pr_metrics_basic() {
        let sessions = vec![
            make_session("s1", Some("feature/auth"), 30),
            make_session("s2", Some("feature/auth"), 45),
            make_session("s3", Some("fix/bug"), 20),
        ];

        let cache = make_cache(vec![
            PrMapping {
                pr_number: 10,
                title: "Add authentication".to_string(),
                branch: "feature/auth".to_string(),
                closed_issues: vec![1, 2],
                merged_at: Some("2026-01-01".to_string()),
            },
            PrMapping {
                pr_number: 11,
                title: "Fix bug".to_string(),
                branch: "fix/bug".to_string(),
                closed_issues: vec![3],
                merged_at: None,
            },
        ]);

        let metrics = calculate_pr_metrics(&sessions, &cache);

        assert_eq!(metrics.len(), 2);
        // Sorted by time descending, PR 10 has 75 mins
        assert_eq!(metrics[0].pr_number, 10);
        assert_eq!(metrics[0].total_minutes, 75.0);
        assert_eq!(metrics[0].session_count, 2);
        assert_eq!(metrics[0].closed_issues, vec![1, 2]);
        assert_eq!(metrics[0].cost, 0.0); // No tokens in test sessions

        assert_eq!(metrics[1].pr_number, 11);
        assert_eq!(metrics[1].total_minutes, 20.0);
        assert_eq!(metrics[1].session_count, 1);
        assert_eq!(metrics[1].cost, 0.0);
    }

    #[test]
    fn test_calculate_pr_metrics_no_branch() {
        let sessions = vec![make_session("s1", None, 30)];

        let cache = make_cache(vec![PrMapping {
            pr_number: 10,
            title: "PR".to_string(),
            branch: "feature/x".to_string(),
            closed_issues: vec![1],
            merged_at: None,
        }]);

        let metrics = calculate_pr_metrics(&sessions, &cache);
        assert!(metrics.is_empty());
    }

    #[test]
    fn test_calculate_pr_metrics_no_matching_pr() {
        let sessions = vec![make_session("s1", Some("unrelated-branch"), 30)];

        let cache = make_cache(vec![PrMapping {
            pr_number: 10,
            title: "PR".to_string(),
            branch: "feature/x".to_string(),
            closed_issues: vec![1],
            merged_at: None,
        }]);

        let metrics = calculate_pr_metrics(&sessions, &cache);
        assert!(metrics.is_empty());
    }

    #[test]
    fn test_calculate_pr_metrics_pr_without_issues() {
        // PRs without linked issues should still be tracked
        let sessions = vec![make_session("s1", Some("feature/x"), 30)];

        let cache = make_cache(vec![PrMapping {
            pr_number: 10,
            title: "PR without issues".to_string(),
            branch: "feature/x".to_string(),
            closed_issues: vec![], // No linked issues
            merged_at: None,
        }]);

        let metrics = calculate_pr_metrics(&sessions, &cache);
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].pr_number, 10);
        assert_eq!(metrics[0].total_minutes, 30.0);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30.0), "30m");
        assert_eq!(format_duration(60.0), "1h 0m");
        assert_eq!(format_duration(90.0), "1h 30m");
        assert_eq!(format_duration(125.0), "2h 5m");
    }

    #[test]
    fn test_pr_session_duration() {
        let session = make_session("test-session", Some("feature/pr-5"), 45);
        let pr_session = PrSession {
            session: &session,
            duration_minutes: 45.0,
        };
        assert_eq!(pr_session.duration_minutes, 45.0);
        assert_eq!(
            pr_session.session.git_branch,
            Some("feature/pr-5".to_string())
        );
    }
}
