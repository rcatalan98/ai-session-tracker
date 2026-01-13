# AI Session Tracker

> Track, analyze, and improve AI-assisted development workflows

A CLI tool that helps developers understand and optimize their AI coding sessions by tracking time, correlating with git/GitHub, and generating actionable insights.

## The Problem

When working with AI coding assistants (Claude Code, Copilot, Cursor), we lack visibility into:
- How long does AI take to complete tasks?
- How much time is thinking/planning vs actual code output?
- Which types of tasks does AI handle well vs poorly?
- What causes AI sessions to fail or take longer?

## Solution

`aist` (AI Session Tracker) provides:

- **Session tracking** - Start/stop tracking with issue context
- **Claude Code parsing** - Extract detailed metrics from session transcripts
- **Git correlation** - Link sessions to commits made during that time
- **GitHub sync** - Push session data as issue comments or PR metadata
- **Reports** - Weekly/monthly summaries with actionable insights

## Quick Start

```bash
# Install (coming soon)
brew install ai-session-tracker

# Start tracking a session
aist start --issue 110 --description "Fix error handling"

# Work with your AI assistant...

# Stop and record outcome
aist stop --outcome success

# Generate a report
aist report --week
```

## Tech Stack

- **Language**: Rust (single binary, fast, cross-platform)
- **CLI Framework**: clap v4
- **Storage**: SQLite + JSONL hybrid
- **GitHub API**: octocrab
- **Templates**: handlebars

## Development

```bash
# Clone the repository
git clone https://github.com/YOUR_USERNAME/ai-session-tracker
cd ai-session-tracker

# Build
cargo build

# Run tests
cargo test

# Run the CLI
cargo run -- start --issue 123
```

## Project Status

This project is currently in the specification and planning phase. See the [spec document](./docs/spec.md) for the full design.

## License

MIT
