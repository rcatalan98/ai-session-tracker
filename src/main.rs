mod parser;
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
    }
}

fn analyze_command(_project: Option<PathBuf>, _verbose: bool) {
    println!("{}", "Not implemented yet".yellow());
}

fn bottlenecks_command(_project: Option<PathBuf>, _limit: usize) {
    println!("{}", "Not implemented yet".yellow());
}

fn report_command(_period: &str, _format: &str) {
    println!("{}", "Not implemented yet".yellow());
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
