# AI Session Tracker

> Find bottlenecks in AI-assisted development

A CLI tool that analyzes Claude Code session transcripts to identify what's slowing down your AI-assisted workflow.

## The Problem

When working with AI coding assistants, you can't see:
- Where does the AI get stuck?
- What patterns cause sessions to fail?
- How much time is wasted on error loops?
- Which types of tasks work well vs poorly?
- How long does it take to build a feature?

## Solution

`aist` analyzes your existing Claude Code transcripts (no manual tracking needed) and detects:

| Pattern | What It Means |
|---------|--------------|
| **Error loops** | Tool fails → retry → fails again |
| **Exploration spirals** | Lots of reading, no editing |
| **Edit thrashing** | Same file edited repeatedly |
| **Long gaps** | Session stalls for >5 minutes |

## Usage

```bash
# Analyze all sessions
aist analyze

# Show top bottlenecks
aist bottlenecks

# Weekly efficiency report
aist report --week

# Session timeline
aist timeline

# List recent sessions
aist list

# Generate flamegraph visualization
aist flame                      # All sessions
aist flame --group-by project   # Group by project
aist flame --group-by issue     # Group by GitHub issue
```

### GitHub Integration

Track time spent per GitHub issue by linking PRs to Claude sessions:

```bash
# Sync merged PRs from GitHub (caches PR→Issue→Branch mappings)
aist sync

# List time spent per issue
aist issues

# Detailed breakdown for a specific issue
aist issue 4
```

**How it works:** Sessions are linked to issues via branch names. When you work on a branch like `feature/issue-4-auth`, and your PR says "Closes #4", `aist` connects all sessions on that branch to issue #4.

## Example Output

```
BOTTLENECK: Error Loop
━━━━━━━━━━━━━━━━━━━━━━
Session: abc123 (ai-editor, 45 min ago)
Pattern: Bash failed 4 times in a row
Time wasted: ~8 minutes
Suggestion: Check PATH or tool availability before running

BOTTLENECK: Exploration Spiral
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Session: def456 (booking-platform, 2 hours ago)
Pattern: 23 Read calls, 0 Edit calls over 12 minutes
Suggestion: Provide better context upfront (CLAUDE.md, file hints)
```

## Install

```bash
# From source
git clone https://github.com/rcatalan98/ai-session-tracker
cd ai-session-tracker
make install

# Verify
aist --help
```

## Development

```bash
make setup    # First-time setup (installs git hooks)
make dev      # Build and run
make test     # Run tests
make lint     # Check style (fmt + clippy)
make fmt      # Auto-format code
```

### Git Hooks

Installed via `make setup`:
- **pre-commit**: `cargo fmt --check`
- **pre-push**: fmt, clippy, build, test

### CI

GitHub Actions runs on every push/PR: format, clippy, build, test.

## How It Works

Claude Code already saves session transcripts to `~/.claude/projects/`. Each message has timestamps, tool calls, and results. `aist` parses these files and detects patterns that indicate wasted time.

No manual start/stop. No database. Just file analysis.

## License

MIT
