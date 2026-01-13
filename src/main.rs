mod bottlenecks;
mod flamegraph;
mod github;
mod issues;
mod metrics;
mod parser;
mod report;
mod timeline;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "aist")]
#[command(about = "AI Session Tracker - Find bottlenecks in AI-assisted development")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze sessions and show metrics
    Analyze {
        /// Analyze only sessions for a specific project path
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Show detailed output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Detect and display bottlenecks
    Bottlenecks {
        /// Analyze only sessions for a specific project path
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Number of bottlenecks to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Generate a summary report
    Report {
        /// Report period: day, week, month, all
        #[arg(short, long, default_value = "week")]
        period: String,

        /// Output format: text, json
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show timeline for a specific session
    Timeline {
        /// Session ID or "latest" for most recent
        #[arg(default_value = "latest")]
        session: String,

        /// Project path to filter sessions
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// List all sessions
    List {
        /// Number of sessions to show
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Filter by project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },

    /// Generate a flamegraph-style SVG visualization
    Flame {
        /// Output file path
        #[arg(short, long, default_value = "session-flamegraph.svg")]
        output: PathBuf,

        /// Filter by project path
        #[arg(short, long)]
        project: Option<PathBuf>,

        /// Group by: session (default) or project
        #[arg(short, long, default_value = "session")]
        group_by: String,
    },

    /// Sync GitHub PRs and cache PR→Issue→Branch mappings
    Sync {
        /// GitHub repository owner (auto-detected from git remote if not specified)
        #[arg(long)]
        owner: Option<String>,

        /// GitHub repository name (auto-detected from git remote if not specified)
        #[arg(long)]
        repo: Option<String>,
    },

    /// List GitHub issues with time metrics
    Issues {
        /// Filter by project path
        #[arg(short, long)]
        project: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { project, verbose } => {
            analyze_command(project, verbose);
        }
        Commands::Bottlenecks { project, limit } => {
            bottlenecks_command(project, limit);
        }
        Commands::Report { period, format } => {
            report_command(&period, &format);
        }
        Commands::Timeline { session, project } => {
            timeline_command(&session, project);
        }
        Commands::List { limit, project } => {
            list_command(limit, project);
        }
        Commands::Flame {
            output,
            project,
            group_by,
        } => {
            flame_command(output, project, &group_by);
        }
        Commands::Sync { owner, repo } => {
            sync_command(owner.as_deref(), repo.as_deref());
        }
        Commands::Issues { project } => {
            issues_command(project);
        }
    }
}

fn analyze_command(project: Option<PathBuf>, verbose: bool) {
    let sessions = parser::load_sessions(project.as_deref());

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    let aggregated = metrics::aggregate_metrics(&sessions);

    // Header
    println!("{}", "SESSION ANALYSIS".bold());
    println!("{}", "\u{2550}".repeat(16));
    println!();

    // Summary line
    println!(
        "Sessions: {} | Total time: {}",
        aggregated.session_count.to_string().bold(),
        metrics::format_duration(aggregated.total_duration_minutes).bold()
    );
    println!();

    // Tool usage section
    println!("{}", "TOOL USAGE".bold());
    println!("{}", "\u{2500}".repeat(10));

    // Sort tools by count (descending)
    let mut tool_list: Vec<_> = aggregated.tool_counts.iter().collect();
    tool_list.sort_by(|a, b| b.1.cmp(a.1));

    for (tool, count) in tool_list
        .iter()
        .take(if verbose { tool_list.len() } else { 10 })
    {
        let percentage = if aggregated.total_tool_calls > 0 {
            (**count as f64 / aggregated.total_tool_calls as f64 * 100.0) as usize
        } else {
            0
        };
        println!(
            "{:<12} {:>6} ({:>2}%)",
            tool,
            metrics::format_number(**count),
            percentage
        );
    }

    if !verbose && tool_list.len() > 10 {
        println!(
            "{}",
            format!(
                "... and {} more (use --verbose to see all)",
                tool_list.len() - 10
            )
            .dimmed()
        );
    }
    println!();

    // By project section
    println!("{}", "BY PROJECT".bold());
    println!("{}", "\u{2500}".repeat(10));

    // Sort projects by duration (descending)
    let mut project_list: Vec<_> = aggregated.by_project.iter().collect();
    project_list.sort_by(|a, b| {
        b.1.total_duration_minutes
            .partial_cmp(&a.1.total_duration_minutes)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (project_name, proj_metrics) in
        project_list
            .iter()
            .take(if verbose { project_list.len() } else { 10 })
    {
        println!(
            "{:<20} {:>2} sessions, {:>6}",
            if project_name.len() > 18 {
                format!("{}...", &project_name[..15])
            } else {
                (*project_name).clone()
            },
            proj_metrics.session_count,
            metrics::format_duration(proj_metrics.total_duration_minutes)
        );
    }

    if !verbose && project_list.len() > 10 {
        println!(
            "{}",
            format!(
                "... and {} more (use --verbose to see all)",
                project_list.len() - 10
            )
            .dimmed()
        );
    }
    println!();

    // Errors section
    println!("{}", "ERRORS".bold());
    println!("{}", "\u{2500}".repeat(6));
    println!(
        "Total: {} errors detected",
        metrics::format_number(aggregated.total_errors)
    );
}

fn bottlenecks_command(project: Option<PathBuf>, limit: usize) {
    let sessions = parser::load_sessions(project.as_deref());

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    let detected = bottlenecks::detect_all(&sessions);
    bottlenecks::print_bottlenecks(&detected, limit);
}

fn report_command(period: &str, format: &str) {
    let sessions = parser::load_sessions(None);

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    let report_data = report::generate_report(&sessions, period);

    match format {
        "json" => report::print_json_report(&report_data),
        _ => report::print_text_report(&report_data),
    }
}

fn timeline_command(session_id: &str, project: Option<PathBuf>) {
    let sessions = parser::load_sessions(project.as_deref());

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    let session = if session_id == "latest" {
        timeline::get_latest_session(&sessions)
    } else {
        timeline::find_session_by_id(&sessions, session_id)
    };

    match session {
        Some(s) => timeline::print_timeline(s),
        None => {
            println!(
                "{}: No session found matching '{}'",
                "Error".red(),
                session_id
            );
            println!("{}", "Use 'aist list' to see available sessions.".dimmed());
        }
    }
}

fn list_command(limit: usize, project: Option<PathBuf>) {
    let sessions = parser::load_sessions(project.as_deref());

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    // Sort by end time, most recent first
    let mut sessions = sessions;
    sessions.sort_by(|a, b| b.end_time.cmp(&a.end_time));

    println!(
        "{}\n",
        format!("RECENT SESSIONS (showing {})", limit.min(sessions.len())).bold()
    );

    println!(
        "{:<12} {:<40} {:<15} {:>10}",
        "SESSION".dimmed(),
        "PROJECT".dimmed(),
        "BRANCH".dimmed(),
        "DURATION".dimmed()
    );
    println!("{}", "─".repeat(80).dimmed());

    for session in sessions.iter().take(limit) {
        let project_display = session.project.replace(
            &dirs::home_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            "~",
        );

        let project_short = if project_display.len() > 38 {
            format!("...{}", &project_display[project_display.len() - 35..])
        } else {
            project_display
        };

        let branch = session
            .git_branch
            .as_deref()
            .unwrap_or("-")
            .chars()
            .take(13)
            .collect::<String>();

        let duration = match (session.start_time, session.end_time) {
            (Some(start), Some(end)) => {
                let mins = (end - start).num_minutes();
                if mins >= 60 {
                    format!("{}h {}m", mins / 60, mins % 60)
                } else {
                    format!("{}m", mins)
                }
            }
            _ => "-".to_string(),
        };

        let session_short: String = session.session_id.chars().take(10).collect();

        println!(
            "{:<12} {:<40} {:<15} {:>10}",
            session_short, project_short, branch, duration
        );
    }

    println!(
        "\n{} total sessions found",
        sessions.len().to_string().bold()
    );
}

fn flame_command(output: PathBuf, project: Option<PathBuf>, group_by: &str) {
    let sessions = parser::load_sessions(project.as_deref());

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    let result = match group_by {
        "project" => flamegraph::generate_svg_by_project(&sessions, &output),
        _ => flamegraph::generate_svg(&sessions, &output),
    };

    match result {
        Ok(()) => {
            println!("{} Generated flamegraph: {}", "✓".green(), output.display());
            println!(
                "{}",
                "Open in browser to view interactive visualization".dimmed()
            );
        }
        Err(e) => {
            println!("{}: Failed to generate flamegraph: {}", "Error".red(), e);
        }
    }
}

fn sync_command(owner: Option<&str>, repo: Option<&str>) {
    match github::sync(owner, repo) {
        Ok(()) => {
            println!("{}", "Sync complete!".green().bold());
        }
        Err(e) => {
            println!("{}: {}", "Error".red(), e);
        }
    }
}

fn issues_command(project: Option<PathBuf>) {
    let sessions = parser::load_sessions(project.as_deref());

    if sessions.is_empty() {
        println!("{}", "No sessions found.".yellow());
        return;
    }

    issues::list_issues(&sessions);
}
