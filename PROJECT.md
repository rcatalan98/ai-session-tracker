# PROJECT.md

> Implementation plan and task tracking

---

## MVP Scope

**Goal:** Analyze Claude Code transcripts to find bottlenecks in AI-assisted development.

**Success criteria:**
1. Run `aist bottlenecks` and see actual bottlenecks from real sessions
2. Run `aist report --week` and see efficiency metrics
3. Data matches what we found in Python research script

---

## Implementation Plan

### Phase 1: Core Parser (Issues #1-2)

Build the foundation for reading Claude Code transcripts.

| Issue | Title | Scope |
|-------|-------|-------|
| #1 | Scaffold Rust project with CLI | Cargo.toml, main.rs, clap setup |
| #2 | Parse Claude Code JSONL transcripts | parser.rs - read sessions, extract messages |

### Phase 2: Metrics & Analysis (Issues #3-4)

Calculate useful metrics from parsed data.

| Issue | Title | Scope |
|-------|-------|-------|
| #3 | Extract session metrics | metrics.rs - duration, tool counts, timing |
| #4 | Implement bottleneck detection | bottlenecks.rs - error loops, spirals, thrashing |

### Phase 3: Output & Reports (Issues #5-6)

Display results to the user.

| Issue | Title | Scope |
|-------|-------|-------|
| #5 | Session timeline view | timeline.rs - visual timeline of session |
| #6 | Weekly report generation | report.rs - summary stats, efficiency score |

---

## Issue Details

### Issue #1: Scaffold Rust project with CLI

**Scope:** Project setup and CLI skeleton

**Files to create:**
- `Cargo.toml` - dependencies (clap, serde, chrono, etc.)
- `src/main.rs` - CLI entry point with subcommands
- `src/lib.rs` - module declarations

**Acceptance criteria:**
- [ ] `cargo build` succeeds
- [ ] `aist --help` shows all commands
- [ ] `aist analyze` prints "Not implemented yet"
- [ ] `aist bottlenecks` prints "Not implemented yet"
- [ ] `aist report` prints "Not implemented yet"
- [ ] `aist timeline` prints "Not implemented yet"
- [ ] `aist list` prints "Not implemented yet"

**CLI structure:**
```
aist analyze [--project PATH] [--verbose]
aist bottlenecks [--project PATH] [--limit N]
aist report [--period day|week|month] [--format text|json]
aist timeline [SESSION_ID] [--project PATH]
aist list [--limit N] [--project PATH]
```

---

### Issue #2: Parse Claude Code JSONL transcripts

**Scope:** Read and parse session files from ~/.claude/projects/

**Files to create:**
- `src/parser.rs` - transcript parsing logic

**Data structures:**
```rust
pub struct Session {
    pub session_id: String,
    pub project: String,
    pub git_branch: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub messages: Vec<Message>,
}

pub struct Message {
    pub msg_type: MessageType,  // User, Assistant, System, Summary
    pub timestamp: Option<DateTime<Utc>>,
    pub content: MessageContent,
}

pub struct ToolCall {
    pub name: String,
    pub input: serde_json::Value,
}

pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}
```

**Acceptance criteria:**
- [ ] Find all JSONL files in ~/.claude/projects/
- [ ] Parse each line as JSON
- [ ] Extract session_id, timestamps, git_branch
- [ ] Extract tool_use from assistant messages
- [ ] Extract tool_result from user messages
- [ ] Handle malformed lines gracefully (skip, don't crash)
- [ ] `aist list` shows real sessions from disk

---

### Issue #3: Extract session metrics

**Scope:** Calculate metrics from parsed sessions

**Files to create:**
- `src/metrics.rs` - metric calculations

**Metrics to calculate:**
```rust
pub struct SessionMetrics {
    pub duration_minutes: f64,
    pub tool_counts: HashMap<String, usize>,
    pub total_tool_calls: usize,
    pub error_count: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub files_read: HashSet<String>,
    pub files_edited: HashSet<String>,
    pub subagent_count: usize,
}

pub struct AggregatedMetrics {
    pub session_count: usize,
    pub total_duration_minutes: f64,
    pub total_tool_calls: usize,
    pub total_errors: usize,
    pub tool_counts: HashMap<String, usize>,
    pub by_project: HashMap<String, ProjectMetrics>,
}
```

**Acceptance criteria:**
- [ ] Calculate duration from first to last timestamp
- [ ] Count tool usage by type
- [ ] Count errors (is_error: true or error patterns in content)
- [ ] Extract file paths from Read/Edit/Write tools
- [ ] Count subagent sessions
- [ ] `aist analyze` shows real metrics

---

### Issue #4: Implement bottleneck detection

**Scope:** Detect patterns that indicate wasted time

**Files to create:**
- `src/bottlenecks.rs` - pattern detection

**Patterns to detect:**

1. **Error Loop** - Same tool fails 3+ times in a row
```rust
pub struct ErrorLoop {
    pub tool_name: String,
    pub failure_count: usize,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub error_samples: Vec<String>,
}
```

2. **Exploration Spiral** - >10 Read/Grep calls with 0 Edit in 10+ minutes
```rust
pub struct ExplorationSpiral {
    pub read_count: usize,
    pub duration_minutes: f64,
    pub files_searched: Vec<String>,
}
```

3. **Edit Thrashing** - Same file edited 5+ times
```rust
pub struct EditThrashing {
    pub file_path: String,
    pub edit_count: usize,
    pub duration_minutes: f64,
}
```

4. **Long Gap** - >5 minutes between messages
```rust
pub struct LongGap {
    pub gap_minutes: f64,
    pub before_message: String,
    pub after_message: String,
}
```

**Acceptance criteria:**
- [ ] Detect error loops with 3+ failures
- [ ] Detect exploration spirals (high read, no edit)
- [ ] Detect edit thrashing (same file 5+ times)
- [ ] Detect long gaps (>5 min)
- [ ] `aist bottlenecks` shows real bottlenecks
- [ ] Output includes session ID, project, time, and suggestion

---

### Issue #5: Session timeline view

**Scope:** Visual timeline of a single session

**Files to create:**
- `src/timeline.rs` - timeline rendering

**Output format:**
```
SESSION: abc123
Project: ~/personal_projects/ai-editor
Branch: feature/auth
Duration: 45 minutes

TIMELINE
────────
14:00:00  ▶ Session start
14:00:05  📖 Read CLAUDE.md
14:00:12  📖 Read src/main.rs
14:00:30  🔍 Grep "auth"
14:01:15  📖 Read src/auth.rs
14:02:00  ✏️  Edit src/auth.rs
14:03:30  🖥️  Bash: cargo build
14:03:45  ❌ Error: missing import
14:04:00  ✏️  Edit src/auth.rs (fix import)
14:04:30  🖥️  Bash: cargo build
14:04:45  ✅ Success
14:05:00  🖥️  Bash: cargo test
14:05:30  ✅ All tests pass
14:06:00  ⏹ Session end

SUMMARY
───────
Tool calls: 12 (Read: 4, Edit: 2, Bash: 3, Grep: 1)
Errors: 1 (resolved)
Files touched: 2
```

**Acceptance criteria:**
- [ ] Show timeline with timestamps
- [ ] Use icons for different tool types
- [ ] Mark errors clearly
- [ ] Show summary at end
- [ ] `aist timeline` shows latest session
- [ ] `aist timeline <id>` shows specific session

---

### Issue #6: Weekly report generation

**Scope:** Summary report with efficiency metrics

**Files to create:**
- `src/report.rs` - report generation

**Output format:**
```
AI SESSION REPORT: Week 3, 2026
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Sessions: 12 | Time: 8.5 hours | Efficiency: 73%

TIME BREAKDOWN
──────────────
Productive:  ████████████████░░░░ 73% (6.2h)
Error loops: ████░░░░░░░░░░░░░░░░ 15% (1.3h)
Exploration: ███░░░░░░░░░░░░░░░░░ 12% (1.0h)

TOP BOTTLENECKS
───────────────
1. Bash errors (42 failures) → Missing dependencies
2. Edit conflicts (18 retries) → Complex refactoring
3. Long searches (3 sessions) → Unclear requirements

BY PROJECT
──────────
ai-editor:        5 sessions, 4.2h, 68% efficiency
booking-platform: 4 sessions, 2.8h, 81% efficiency

RECOMMENDATIONS
───────────────
□ Add cargo to PATH in ai-editor environment
□ Break down large refactoring tasks
```

**Acceptance criteria:**
- [ ] Filter sessions by period (day/week/month)
- [ ] Calculate efficiency score
- [ ] Show time breakdown
- [ ] List top bottlenecks
- [ ] Group by project
- [ ] Generate actionable recommendations
- [ ] Support JSON output format

---

## Open Questions

1. **Efficiency calculation** - How do we define "wasted time"?
   - Current idea: Error loops + exploration spirals + long gaps
   - Efficiency = (Total - Wasted) / Total

2. **Session boundaries** - What counts as one "session"?
   - Current idea: One JSONL file = one session
   - Could also split by long gaps (>30 min)

3. **Error detection** - What counts as an error?
   - `is_error: true` in tool_result
   - Keywords: "error", "failed", "not found", "permission denied"
   - Exit codes != 0 in Bash

---

## Not Building (Out of Scope)

- Manual start/stop commands
- GitHub integration
- Database storage
- Web dashboard
- Issue tracking integration
- Cost calculation (API costs)

---

*Last updated: 2026-01-13*
