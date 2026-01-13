# Research Plan: Validating AI Session Tracker Value

## Objective

Before building any tooling, prove that Claude Code transcript data can answer:
**"What are the bottlenecks when building with AI agents?"**

## Phase 1: Data Extraction Script (1-2 days)

Build a simple Python/jq script to extract metrics from existing Claude Code sessions.

### Metrics to Extract

From each session in `~/.claude/projects/*/`:

```python
{
    "session_id": "uuid",
    "project": "/path/to/project",
    "git_branch": "feature/xyz",
    "start_time": "2026-01-11T19:56:15Z",
    "end_time": "2026-01-11T20:30:00Z",
    "duration_minutes": 34,

    # Tool usage
    "tool_counts": {
        "Read": 23,
        "Edit": 21,
        "Bash": 33,
        "Write": 13,
        "Glob": 5,
        "Grep": 2,
        "Task": 9  # subagents
    },

    # Error analysis
    "errors": {
        "total": 5,
        "by_tool": {"Bash": 3, "Edit": 2},
        "samples": ["command not found: xyz", "file not found"]
    },

    # Retry patterns
    "retries": {
        "same_tool_consecutive": 3,  # Tool called multiple times in a row
        "edit_after_error": 2        # Edit after a failed bash
    },

    # Subagent usage
    "subagents": {
        "count": 3,
        "total_messages": 150
    },

    # Message patterns
    "user_messages": 12,
    "assistant_messages": 45,
    "user_rejections": 2,  # "no", "wrong", "that's not right"

    # Files
    "unique_files_read": 15,
    "unique_files_edited": 4
}
```

### Output

Generate `analysis/sessions.jsonl` with one record per session.

---

## Phase 2: Bottleneck Hypotheses (2-3 days)

Using the extracted data, test these hypotheses:

### Hypothesis 1: Error Loops are Time Sinks

**Question**: How much time is spent in error-retry cycles?

**Analysis**:
```sql
-- Pseudo-query
SELECT
    session_id,
    errors.total,
    duration_minutes,
    errors.total / duration_minutes as error_rate
FROM sessions
ORDER BY error_rate DESC
```

**Expected insight**: "Sessions with >5 errors take 2x longer on average"

---

### Hypothesis 2: Exploration Before Action Correlates with Success

**Question**: Do sessions that Read more before Edit succeed more?

**Analysis**:
```python
read_before_first_edit = count_reads_before_first_edit(session)
# Correlate with session "success" (needs manual labeling for now)
```

**Expected insight**: "Sessions with >=3 reads before first edit have 40% fewer errors"

---

### Hypothesis 3: Subagent Overuse is a Bottleneck

**Question**: Are subagents being used effectively or causing overhead?

**Analysis**:
```python
subagent_ratio = subagent_messages / total_messages
# Compare sessions with high vs low subagent usage
```

**Expected insight**: "Sessions with >50% subagent messages take 30% longer"

---

### Hypothesis 4: Certain Task Types Fail More

**Question**: What types of tasks (based on prompt keywords) fail most?

**Analysis**:
```python
task_type = classify_prompt(["refactor", "fix", "add", "test", "deploy"])
# Compare error rates and durations by task type
```

**Expected insight**: "Deployment tasks have 60% error rate vs 20% for refactors"

---

### Hypothesis 5: Large Codebases = More Exploration Time

**Question**: Does codebase size affect the Read/Edit ratio?

**Analysis**:
```python
files_in_project = count_files(project_path)
read_edit_ratio = tool_counts["Read"] / tool_counts["Edit"]
# Correlation analysis
```

**Expected insight**: "Projects with >100 files have 3x higher Read/Edit ratio"

---

## Phase 3: Manual Session Labeling (1-2 days)

To validate hypotheses, manually label 20-30 sessions with:

| Field | Values | Source |
|-------|--------|--------|
| outcome | success / partial / failure | Your memory |
| task_type | feature / bugfix / refactor / docs / deploy | Prompt analysis |
| blockers | unclear_requirements / missing_context / api_error / etc | Review transcript |
| estimated_difficulty | easy / medium / hard | Your judgment |

Store in `analysis/labeled_sessions.json`.

---

## Phase 4: Analysis & Insights (1-2 days)

Using DuckDB or pandas, answer:

1. **What predicts session failure?**
   - Error count? Task type? Read/Edit ratio?

2. **What are the top 3 time sinks?**
   - Error loops? Excessive exploration? Subagent overhead?

3. **What patterns correlate with fast, successful sessions?**
   - Specific tool sequences? Branch naming? Time of day?

### Deliverable

A report (`analysis/findings.md`) with:
- Top 5 bottlenecks with supporting data
- Recommendations for improving AI agent workflows
- Decision: Build tracker or not?

---

## Phase 5: Decision Gate

After Phase 4, answer:

1. **Did we find actionable bottlenecks?**
   - If yes: Build the tracker with focus on those metrics
   - If no: The problem might not be data visibility

2. **What metrics actually matter?**
   - Cut vanity metrics (tool call counts)
   - Keep predictive metrics (error patterns, retry loops)

3. **Is manual start/stop needed?**
   - If transcripts have enough context: No
   - If we need issue linkage + outcome: Yes (but enforce via AI agent instructions)

---

## Implementation Order

```
Week 1:
├── Day 1-2: Build extraction script
├── Day 3-4: Run on all existing sessions
└── Day 5: Manual labeling of 20 sessions

Week 2:
├── Day 1-2: Hypothesis testing with real data
├── Day 3: Write findings report
└── Day 4: Decision gate - build or not?
```

---

## Success Criteria

The research succeeds if we can answer:

1. ✅ "These 3 patterns indicate a session will fail"
2. ✅ "These 3 bottlenecks account for X% of wasted time"
3. ✅ "Tracking these specific metrics will reduce bottlenecks"

The research fails if:

1. ❌ All sessions look the same in the data
2. ❌ Bottlenecks aren't detectable from transcript data
3. ❌ The problems are human-side (bad prompts) not agent-side
