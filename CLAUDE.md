# CLAUDE.md

> Single source of truth for AI assistants working on this project.

---

## Project Overview

**What:** CLI tool that analyzes Claude Code session transcripts to find bottlenecks in AI-assisted development.

**Current phase:** Phase 2 (GitHub Integration)

**In scope:**
- Parse Claude Code transcripts from `~/.claude/projects/`
- Detect bottleneck patterns (error loops, exploration spirals, edit thrashing)
- Generate session timelines and flamegraph visualizations
- Generate weekly efficiency reports
- GitHub integration (track time per issue via PR→Branch→Session linking)

**Out of scope:**
- Manual start/stop tracking (transcripts already have timestamps)
- Database storage (files only)
- Web dashboard

---

## Quick Start

```bash
make setup          # First-time setup
make dev            # Run in development
make test           # Run tests
```

---

## Tech Stack

| Component | Choice | Notes |
|-----------|--------|-------|
| Language | Rust | Single binary, fast, cross-platform |
| CLI | clap v4 | Derive macros |
| JSON | serde + serde_json | Parse JSONL transcripts |
| Time | chrono | Timestamp handling |
| Output | colored | Terminal colors |

---

## File Structure

```
ai-session-tracker/
├── CLAUDE.md              # This file
├── Makefile               # Commands
├── Cargo.toml
├── .github/workflows/     # CI configuration
│   └── ci.yml
├── .githooks/             # Git hooks (installed via make setup)
│   ├── pre-commit         # cargo fmt --check
│   └── pre-push           # fmt, clippy, build, test
├── .ralph/                # Ralph autonomous agent config
│   ├── prompt.md
│   ├── prd.json
│   └── progress.txt
├── src/
│   ├── main.rs            # CLI entry point
│   ├── parser.rs          # Parse Claude JSONL transcripts
│   ├── metrics.rs         # Calculate metrics
│   ├── bottlenecks.rs     # Detect bottleneck patterns
│   ├── timeline.rs        # Session timeline view
│   ├── report.rs          # Generate reports
│   ├── flamegraph.rs      # SVG flamegraph visualization
│   ├── github.rs          # GitHub API (PR sync, caching)
│   ├── issues.rs          # Issue-level time tracking
│   ├── prs.rs             # PR-level time tracking
│   └── export.rs          # HTML report generation
└── product_research/      # Research scripts and findings
```

---

## Commands

```bash
# Development
make dev              # Build and run
make check            # Fast type check
make smoke            # Fast validation (~10s)

# Build & Test
make test             # Run tests
make lint             # Check style (fmt + clippy)
make fmt              # Format code
make build            # Release build

# CLI Usage (after build)
aist analyze          # Analyze all sessions
aist bottlenecks      # Show top bottlenecks
aist report --week    # Weekly summary
aist timeline         # Show latest session timeline
aist list             # List recent sessions
aist flame            # Generate flamegraph SVG
aist flame --group-by project  # Group by project
aist flame --group-by pr       # Group by PR

# GitHub Integration
aist sync             # Fetch merged PRs, cache mappings
aist prs              # List time per PR
aist pr <N>           # Detailed breakdown for PR #N
aist issues           # List time per issue
aist issue <N>        # Detailed breakdown for issue #N

# HTML Reports
aist export           # Generate HTML report for current repo
aist export --period week      # Filter by time period
aist export -o report.html     # Custom output path
```

---

## Code Patterns

### Philosophy

1. **Simplicity over performance** — Only optimize when benchmarks prove it
2. **Less code is better** — Deleted code is debugged code
3. **YAGNI** — Build what you need today, not what you might need tomorrow
4. **Few abstractions** — Repeat code 2-3x before abstracting

### Data Sources

Claude Code stores transcripts at:
```
~/.claude/projects/{project-path-encoded}/{session-id}.jsonl
~/.claude/projects/{project-path-encoded}/{session-id}/subagents/agent-{id}.jsonl
```

Each JSONL line is a message with:
```json
{
  "type": "user|assistant|system|summary|file-history-snapshot",
  "timestamp": "2026-01-11T19:56:15.359Z",
  "sessionId": "uuid",
  "gitBranch": "main",
  "cwd": "/path/to/project",
  "message": { "role": "...", "content": [...] }
}
```

Tool calls are in assistant messages:
```json
{
  "type": "tool_use",
  "name": "Bash|Read|Edit|Write|Grep|Glob|Task|...",
  "input": { ... }
}
```

Tool results are in user messages:
```json
{
  "type": "tool_result",
  "tool_use_id": "...",
  "content": "...",
  "is_error": false
}
```

### Bottleneck Patterns to Detect

| Pattern | Description | Detection |
|---------|-------------|-----------|
| Error Loop | Same tool fails multiple times | 3+ consecutive failures of same tool |
| Exploration Spiral | Lots of reading, no editing | >10 Read/Grep with 0 Edit in 10+ min |
| Edit Thrashing | Same file edited repeatedly | Same file edited 5+ times |
| Long Gaps | Session stalls | >5 min between messages |
| Subagent Overhead | Spawning without results | Task calls with minimal output |

---

## Git Conventions

- **Branch:** `feature/issue-{N}-description` or `fix/issue-{N}-description`
- **Commit:** `type(scope): subject`
- **Never commit directly to main**
- **One issue = One branch = One PR**

---

## When Stuck

1. Pick the simpler option
2. Ask, don't guess
3. If complex, there's a simpler way

---

## What NOT to Do

- Don't add database storage (files are fine for this scale)
- Don't add web UI (CLI is sufficient)
- Don't optimize prematurely
- Don't add features "while you're there"

---

## Features

| Command | Description | Status |
|---------|-------------|--------|
| `aist analyze` | Show session metrics | ✓ |
| `aist bottlenecks` | Detect and display bottleneck patterns | ✓ |
| `aist report --week` | Weekly efficiency report | ✓ |
| `aist timeline` | Visual timeline of session | ✓ |
| `aist list` | List recent sessions | ✓ |
| `aist flame` | Flamegraph SVG visualization | ✓ |
| `aist sync` | Sync GitHub PRs and cache mappings | ✓ |
| `aist prs` | List time spent per PR | ✓ |
| `aist pr <N>` | Detailed breakdown for specific PR | ✓ |
| `aist issues` | List time spent per GitHub issue | ✓ |
| `aist issue <N>` | Detailed breakdown for specific issue | ✓ |
| `aist export` | Generate HTML report with flamegraph | ✓ |

---

## GitHub Integration Architecture

Sessions are linked to GitHub issues via PR branch names:

```
PR #12 "Add auth" → closes #4 → branch: feature/issue-4-auth
                                    ↓
Session (gitBranch: "feature/issue-4-auth") → linked to Issue #4
```

**Data flow:**
1. `aist sync` fetches merged PRs via `gh pr list --json`
2. Parses "Closes #N", "Fixes #N", "Resolves #N" from PR bodies
3. Caches to `~/.config/aist/repos/{owner}-{repo}.json`
4. `aist issues` matches sessions by `gitBranch` field

---

## Key Metrics

| Metric | Why It Matters |
|--------|----------------|
| Error loops | Tool fails → retry → fails = wasted time |
| Exploration ratio | High Read/Grep with low Edit = stuck searching |
| Time gaps | Long pauses (>5min) = blocked or confused |
| Retry patterns | Same file edited 3+ times = struggling |
| Session duration | Total time from first to last message |
| Efficiency | (Total time - wasted time) / Total time |

---

*Last updated: 2026-01-14*
