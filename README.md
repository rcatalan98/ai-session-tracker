# AI Session Tracker

> Find bottlenecks in AI-assisted development

A CLI tool that analyzes Claude Code session transcripts to identify what's slowing down your AI-assisted workflow.

## The Problem

When working with AI coding assistants, you can't see:
- Where does the AI get stuck?
- What patterns cause sessions to fail?
- How much time is wasted on error loops?
- Which types of tasks work well vs poorly?

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
```

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
make setup    # First-time setup (installs hooks)
make dev      # Build and run
make test     # Run tests
make lint     # Check style
```

## How It Works

Claude Code already saves session transcripts to `~/.claude/projects/`. Each message has timestamps, tool calls, and results. `aist` parses these files and detects patterns that indicate wasted time.

No manual start/stop. No database. Just file analysis.

## License

MIT
