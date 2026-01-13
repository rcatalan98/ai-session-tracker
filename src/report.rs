use crate::bottlenecks::{self, Bottleneck};
use crate::metrics::{self, format_duration, ProjectMetrics};
use crate::parser::Session;
use chrono::{Datelike, Utc};
use colored::Colorize;
use serde::Serialize;
use std::collections::HashMap;

/// Report data structure for JSON output
#[derive(Debug, Serialize)]
pub struct Report {
    pub period: String,
    pub week_number: u32,
    pub year: i32,
    pub session_count: usize,
    pub total_hours: f64,
    pub efficiency_percent: f64,
    pub time_breakdown: TimeBreakdown,
    pub top_bottlenecks: Vec<BottleneckSummary>,
    pub by_project: Vec<ProjectReport>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TimeBreakdown {
    pub productive_minutes: f64,
    pub error_loop_minutes: f64,
    pub exploration_minutes: f64,
    pub edit_thrashing_minutes: f64,
    pub long_gap_minutes: f64,
}

#[derive(Debug, Serialize)]
pub struct BottleneckSummary {
    pub bottleneck_type: String,
    pub count: usize,
    pub total_minutes: f64,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectReport {
    pub name: String,
    pub session_count: usize,
    pub hours: f64,
    pub efficiency_percent: f64,
}

/// Generate a report for the given sessions
pub fn generate_report(sessions: &[Session], period: &str) -> Report {
    let filtered = metrics::filter_by_period(sessions, period);
    let aggregated = metrics::aggregate_metrics(&filtered);
    let bottlenecks = bottlenecks::detect_all(&filtered);

    let now = Utc::now();
    let week_number = now.iso_week().week();
    let year = now.year();

    // Calculate time breakdown
    let time_breakdown = calculate_time_breakdown(&bottlenecks, aggregated.total_duration_minutes);

    // Calculate efficiency
    let wasted_time = time_breakdown.error_loop_minutes
        + time_breakdown.exploration_minutes
        + time_breakdown.edit_thrashing_minutes
        + time_breakdown.long_gap_minutes;

    let efficiency_percent = if aggregated.total_duration_minutes > 0.0 {
        ((aggregated.total_duration_minutes - wasted_time) / aggregated.total_duration_minutes
            * 100.0)
            .max(0.0)
            .min(100.0)
    } else {
        100.0
    };

    // Summarize bottlenecks by type
    let top_bottlenecks = summarize_bottlenecks(&bottlenecks);

    // Calculate per-project efficiency
    let by_project = calculate_project_reports(&filtered, &aggregated.by_project);

    // Generate recommendations
    let recommendations = generate_recommendations(&bottlenecks);

    Report {
        period: period.to_string(),
        week_number,
        year,
        session_count: filtered.len(),
        total_hours: aggregated.total_duration_minutes / 60.0,
        efficiency_percent,
        time_breakdown,
        top_bottlenecks,
        by_project,
        recommendations,
    }
}

fn calculate_time_breakdown(bottlenecks: &[Bottleneck], total_minutes: f64) -> TimeBreakdown {
    let mut error_loop_minutes = 0.0;
    let mut exploration_minutes = 0.0;
    let mut edit_thrashing_minutes = 0.0;
    let mut long_gap_minutes = 0.0;

    for b in bottlenecks {
        match b {
            Bottleneck::ErrorLoop(e) => error_loop_minutes += e.duration_minutes,
            Bottleneck::ExplorationSpiral(e) => exploration_minutes += e.duration_minutes,
            Bottleneck::EditThrashing(e) => edit_thrashing_minutes += e.duration_minutes,
            Bottleneck::LongGap(g) => long_gap_minutes += g.gap_minutes,
        }
    }

    // Cap wasted time at total time
    let total_wasted =
        error_loop_minutes + exploration_minutes + edit_thrashing_minutes + long_gap_minutes;
    let scale = if total_wasted > total_minutes && total_wasted > 0.0 {
        total_minutes / total_wasted
    } else {
        1.0
    };

    let productive_minutes = (total_minutes
        - (error_loop_minutes + exploration_minutes + edit_thrashing_minutes + long_gap_minutes)
            * scale)
        .max(0.0);

    TimeBreakdown {
        productive_minutes,
        error_loop_minutes: error_loop_minutes * scale,
        exploration_minutes: exploration_minutes * scale,
        edit_thrashing_minutes: edit_thrashing_minutes * scale,
        long_gap_minutes: long_gap_minutes * scale,
    }
}

fn summarize_bottlenecks(bottlenecks: &[Bottleneck]) -> Vec<BottleneckSummary> {
    let mut by_type: HashMap<&str, (usize, f64)> = HashMap::new();

    for b in bottlenecks {
        let (type_name, minutes) = match b {
            Bottleneck::ErrorLoop(e) => ("Error loops", e.duration_minutes),
            Bottleneck::ExplorationSpiral(e) => ("Exploration spirals", e.duration_minutes),
            Bottleneck::EditThrashing(e) => ("Edit thrashing", e.duration_minutes),
            Bottleneck::LongGap(g) => ("Long gaps", g.gap_minutes),
        };
        let entry = by_type.entry(type_name).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += minutes;
    }

    let mut summaries: Vec<BottleneckSummary> = by_type
        .into_iter()
        .map(|(type_name, (count, total_minutes))| {
            let description = match type_name {
                "Error loops" => format!("{} consecutive failures", count),
                "Exploration spirals" => format!("{} search sessions without edits", count),
                "Edit thrashing" => format!("{} files edited repeatedly", count),
                "Long gaps" => format!("{} pauses over 5 minutes", count),
                _ => format!("{} occurrences", count),
            };
            BottleneckSummary {
                bottleneck_type: type_name.to_string(),
                count,
                total_minutes,
                description,
            }
        })
        .collect();

    // Sort by total time wasted
    summaries.sort_by(|a, b| {
        b.total_minutes
            .partial_cmp(&a.total_minutes)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    summaries.into_iter().take(5).collect()
}

fn calculate_project_reports(
    sessions: &[Session],
    project_metrics: &HashMap<String, ProjectMetrics>,
) -> Vec<ProjectReport> {
    let mut reports: Vec<ProjectReport> = Vec::new();

    for (name, metrics) in project_metrics {
        // Get sessions for this project
        let project_sessions: Vec<_> = sessions
            .iter()
            .filter(|s| extract_project_name(&s.project) == *name)
            .cloned()
            .collect();

        // Calculate project-specific bottlenecks
        let project_bottlenecks = bottlenecks::detect_all(&project_sessions);
        let wasted: f64 = project_bottlenecks.iter().map(|b| b.wasted_minutes()).sum();

        let efficiency = if metrics.total_duration_minutes > 0.0 {
            ((metrics.total_duration_minutes - wasted.min(metrics.total_duration_minutes))
                / metrics.total_duration_minutes
                * 100.0)
                .max(0.0)
        } else {
            100.0
        };

        reports.push(ProjectReport {
            name: name.clone(),
            session_count: metrics.session_count,
            hours: metrics.total_duration_minutes / 60.0,
            efficiency_percent: efficiency,
        });
    }

    // Sort by hours descending
    reports.sort_by(|a, b| {
        b.hours
            .partial_cmp(&a.hours)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    reports
}

fn generate_recommendations(bottlenecks: &[Bottleneck]) -> Vec<String> {
    let mut recommendations = Vec::new();
    let mut has_error_loops = false;
    let mut has_exploration = false;
    let mut has_thrashing = false;
    let mut has_gaps = false;

    for b in bottlenecks {
        match b {
            Bottleneck::ErrorLoop(_) => has_error_loops = true,
            Bottleneck::ExplorationSpiral(_) => has_exploration = true,
            Bottleneck::EditThrashing(_) => has_thrashing = true,
            Bottleneck::LongGap(_) => has_gaps = true,
        }
    }

    if has_error_loops {
        recommendations.push("Check PATH and dependencies for failing tools".to_string());
    }
    if has_exploration {
        recommendations.push("Add better context to CLAUDE.md to reduce search time".to_string());
    }
    if has_thrashing {
        recommendations.push("Break down complex changes into smaller, focused tasks".to_string());
    }
    if has_gaps {
        recommendations.push("Review blocked sessions - unclear requirements?".to_string());
    }

    if recommendations.is_empty() {
        recommendations.push("No significant bottlenecks detected - keep it up!".to_string());
    }

    recommendations
}

fn extract_project_name(project_path: &str) -> String {
    project_path
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("unknown")
        .to_string()
}

/// Print report in text format
pub fn print_text_report(report: &Report) {
    let period_display = match report.period.as_str() {
        "day" => "Today".to_string(),
        "week" => format!("Week {}, {}", report.week_number, report.year),
        "month" => "Last 30 days".to_string(),
        "all" => "All time".to_string(),
        _ => report.period.clone(),
    };

    // Header
    println!(
        "{}",
        format!("AI SESSION REPORT: {}", period_display).bold()
    );
    println!("{}", "━".repeat(50));
    println!();

    // Summary line
    println!(
        "Sessions: {} | Time: {} | Efficiency: {}",
        report.session_count.to_string().bold(),
        format!("{:.1}h", report.total_hours).bold(),
        format!("{:.0}%", report.efficiency_percent)
            .color(efficiency_color(report.efficiency_percent))
            .bold()
    );
    println!();

    // Time breakdown with ASCII bar chart
    println!("{}", "TIME BREAKDOWN".bold());
    println!("{}", "─".repeat(40));

    let total = report.time_breakdown.productive_minutes
        + report.time_breakdown.error_loop_minutes
        + report.time_breakdown.exploration_minutes
        + report.time_breakdown.edit_thrashing_minutes
        + report.time_breakdown.long_gap_minutes;

    if total > 0.0 {
        print_bar(
            "Productive",
            report.time_breakdown.productive_minutes,
            total,
            "green",
        );
        if report.time_breakdown.error_loop_minutes > 0.0 {
            print_bar(
                "Error loops",
                report.time_breakdown.error_loop_minutes,
                total,
                "red",
            );
        }
        if report.time_breakdown.exploration_minutes > 0.0 {
            print_bar(
                "Exploration",
                report.time_breakdown.exploration_minutes,
                total,
                "yellow",
            );
        }
        if report.time_breakdown.edit_thrashing_minutes > 0.0 {
            print_bar(
                "Edit thrash",
                report.time_breakdown.edit_thrashing_minutes,
                total,
                "magenta",
            );
        }
        if report.time_breakdown.long_gap_minutes > 0.0 {
            print_bar(
                "Long gaps",
                report.time_breakdown.long_gap_minutes,
                total,
                "blue",
            );
        }
    } else {
        println!("{}", "No time data available".dimmed());
    }
    println!();

    // Top bottlenecks
    if !report.top_bottlenecks.is_empty() {
        println!("{}", "TOP BOTTLENECKS".bold());
        println!("{}", "─".repeat(40));

        for (i, b) in report.top_bottlenecks.iter().enumerate() {
            println!(
                "{}. {} ({}) - {}",
                i + 1,
                b.bottleneck_type.yellow(),
                format_duration(b.total_minutes),
                b.description.dimmed()
            );
        }
        println!();
    }

    // By project
    if !report.by_project.is_empty() {
        println!("{}", "BY PROJECT".bold());
        println!("{}", "─".repeat(40));

        for p in report.by_project.iter().take(5) {
            let name_display = if p.name.len() > 20 {
                format!("{}...", &p.name[..17])
            } else {
                p.name.clone()
            };
            println!(
                "{:<20} {:>2} sessions, {:>5}, {}",
                name_display,
                p.session_count,
                format!("{:.1}h", p.hours),
                format!("{:.0}% eff", p.efficiency_percent)
                    .color(efficiency_color(p.efficiency_percent))
            );
        }

        if report.by_project.len() > 5 {
            println!(
                "{}",
                format!("... and {} more projects", report.by_project.len() - 5).dimmed()
            );
        }
        println!();
    }

    // Recommendations
    println!("{}", "RECOMMENDATIONS".bold());
    println!("{}", "─".repeat(40));
    for rec in &report.recommendations {
        println!("{} {}", "→".cyan(), rec);
    }
}

fn print_bar(label: &str, value: f64, total: f64, color: &str) {
    let percent = (value / total * 100.0) as usize;
    let bar_width = 20;
    let filled = (percent * bar_width / 100).min(bar_width);
    let empty = bar_width - filled;

    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    let colored_bar = match color {
        "green" => bar.green(),
        "red" => bar.red(),
        "yellow" => bar.yellow(),
        "magenta" => bar.magenta(),
        "blue" => bar.blue(),
        _ => bar.normal(),
    };

    println!(
        "{:<12} {} {:>3}% ({})",
        label,
        colored_bar,
        percent,
        format_duration(value)
    );
}

fn efficiency_color(percent: f64) -> colored::Color {
    if percent >= 80.0 {
        colored::Color::Green
    } else if percent >= 60.0 {
        colored::Color::Yellow
    } else {
        colored::Color::Red
    }
}

/// Print report as JSON
pub fn print_json_report(report: &Report) {
    match serde_json::to_string_pretty(report) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializing report: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Message, MessageType};
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
                    timestamp: Some(end),
                    tool_calls: vec![],
                    tool_results: vec![],
                },
            ],
        }
    }

    #[test]
    fn test_generate_report() {
        let sessions = vec![create_test_session()];
        let report = generate_report(&sessions, "all");

        assert_eq!(report.session_count, 1);
        assert!(report.total_hours > 0.0);
        assert!(report.efficiency_percent >= 0.0 && report.efficiency_percent <= 100.0);
    }

    #[test]
    fn test_generate_report_empty() {
        let sessions: Vec<Session> = vec![];
        let report = generate_report(&sessions, "all");

        assert_eq!(report.session_count, 0);
        assert_eq!(report.total_hours, 0.0);
        assert_eq!(report.efficiency_percent, 100.0);
    }

    #[test]
    fn test_extract_project_name() {
        assert_eq!(
            extract_project_name("/Users/test/projects/my-project"),
            "my-project"
        );
        assert_eq!(extract_project_name("simple"), "simple");
    }

    #[test]
    fn test_generate_recommendations_empty() {
        let bottlenecks: Vec<Bottleneck> = vec![];
        let recs = generate_recommendations(&bottlenecks);
        assert_eq!(recs.len(), 1);
        assert!(recs[0].contains("No significant bottlenecks"));
    }

    #[test]
    fn test_efficiency_color() {
        assert_eq!(efficiency_color(85.0), colored::Color::Green);
        assert_eq!(efficiency_color(70.0), colored::Color::Yellow);
        assert_eq!(efficiency_color(50.0), colored::Color::Red);
    }
}
