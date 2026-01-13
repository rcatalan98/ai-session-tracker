# Ralph Agent Instructions for ai-session-tracker

You are an autonomous coding agent working on **ai-session-tracker**, a Rust CLI tool that analyzes Claude Code sessions to find bottlenecks in AI-assisted development.

## Before Starting

1. Read `.ralph/prd.json` for current task state
2. Read `.ralph/progress.txt` for learnings from previous iterations
3. Read `CLAUDE.md` for project patterns and conventions

## Your Task

1. Check you're on the correct branch from PRD `branchName`. If not, create it from main.
2. Pick the **highest priority** user story where `passes: false`
3. Implement that single user story
4. Run quality checks (see below)
5. If checks pass:
   - Commit ALL changes: `feat: [Story ID] - [Story Title]`
   - Update `.ralph/prd.json`: set `passes: true` for completed story
   - Append progress to `.ralph/progress.txt`

## Quality Commands (MUST PASS)

```bash
# All of these must pass before committing
cargo fmt --check    # Format check
cargo clippy         # Linter
cargo build          # Compile
cargo test           # Tests
```

If any command fails, fix the issue before committing.

## Project Structure

```
ai-session-tracker/
├── src/
│   ├── main.rs        # CLI entry point (clap)
│   ├── parser.rs      # Parse Claude JSONL transcripts
│   ├── metrics.rs     # Calculate metrics
│   ├── bottlenecks.rs # Detect bottleneck patterns
│   ├── timeline.rs    # Session timeline view
│   ├── report.rs      # Weekly report generation
│   ├── flamegraph.rs  # SVG visualization
│   ├── github.rs      # GitHub API integration (NEW)
│   └── issues.rs      # Issue-level metrics (NEW)
├── Cargo.toml
├── CLAUDE.md          # Project conventions
└── PROJECT.md         # Implementation plan
```

## Conventions (from CLAUDE.md)

### Rust
- Use clap derive macros for CLI
- Use serde for JSON parsing
- Use colored for terminal output
- Error handling: Return `Result` or print errors directly
- New commands: Add to `Commands` enum in main.rs

### External Commands
- Use `gh` CLI for GitHub API (already authenticated)
- Parse output with serde_json

### Git
- Commit format: `feat: [US-XXX] - Story title`
- One story = one commit
- Always run quality checks before committing
- Co-author line: `Co-Authored-By: Claude <noreply@anthropic.com>`

## Progress Report Format

APPEND to `.ralph/progress.txt` (never replace):

```
## [Date/Time] - [Story ID]: [Title]
- What was implemented
- Files changed: [list files]
- **Learnings for future iterations:**
  - Patterns discovered
  - Gotchas encountered
  - Useful context
---
```

## Consolidate Patterns

If you discover a **reusable pattern**, add it to the `## Codebase Patterns` section at the TOP of progress.txt:

```
## Codebase Patterns
- Pattern: [description]
- Gotcha: [description]
```

Only add patterns that are general and reusable, not story-specific details.

## Stop Condition

After completing a story, check if ALL stories have `passes: true`.

If ALL complete: output `<promise>COMPLETE</promise>`
If more remain: end normally (next iteration continues)

## Important Rules

- Work on ONE story per iteration
- Commit only when ALL quality checks pass
- Keep changes focused and minimal
- Follow existing patterns from CLAUDE.md
- Don't add features not in the current story
- Don't refactor unrelated code
- Read the `notes` field in each story for hints
