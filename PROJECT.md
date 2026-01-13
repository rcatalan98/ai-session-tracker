# PROJECT.md

> Implementation plan and task tracking

---

## Project Status

**Phase 1 (MVP): COMPLETE**
- Session parsing, metrics, bottlenecks, timeline, report, flamegraph

**Phase 2 (GitHub Integration): COMPLETE**
- `aist sync` - Fetch PR→Issue→Branch mappings from GitHub
- `aist issues` - List time spent per issue
- `aist issue <N>` - Detailed breakdown per issue
- `aist flame --group-by issue` - Flamegraph grouped by issue

---

## Vision

Answer: *"How long did it take to build Feature X?"*

```
GitHub Issue #4
      ↓ (closed by)
PR #10
      ↓ (branch)
"feature/issue-4-bottlenecks"
      ↓ (Claude sessions on this branch)
Time: 2h 15m across 3 sessions
```

---

## Completed Features

| Command | Description |
|---------|-------------|
| `aist analyze` | Session metrics (tool counts, errors, by project) |
| `aist bottlenecks` | Detect error loops, exploration spirals, edit thrashing, gaps |
| `aist report` | Weekly efficiency report with recommendations |
| `aist timeline` | Visual timeline of session activities |
| `aist list` | List recent sessions |
| `aist flame` | SVG flamegraph (by session, project, or issue) |
| `aist sync` | Fetch PR→Issue→Branch mappings from GitHub |
| `aist issues` | List time spent per GitHub issue |
| `aist issue <N>` | Detailed breakdown for specific issue |

---

## Phase 2: GitHub Integration (COMPLETE)

### Architecture

```
aist sync
    ↓
Fetch merged PRs via `gh` CLI
    ↓
Extract: PR → Branch → Closed Issues
    ↓
Cache in ~/.config/aist/repos/{owner}-{repo}.json
    ↓
Link to Claude sessions by matching gitBranch
    ↓
aist issues / aist issue <N>
```

### Issues

| Issue | Title | Status |
|-------|-------|--------|
| #13 | GitHub sync: fetch PR→Issue mappings | ✓ Closed |
| #14 | Issue metrics: show time per GitHub issue | ✓ Closed |
| #15 | Flamegraph by issue | ✓ Closed |

### Commands

```bash
aist sync                    # Fetch PR data from GitHub
aist issues                  # List issues with time metrics
aist issue 4                 # Details for issue #4
aist flame --group-by issue  # Flamegraph by issue
```

### Output Examples

**`aist issues`**
```
ISSUES BY TIME
══════════════════════════════════════════════════
#4  bottleneck detection      2h 15m   3 sessions
#6  weekly report             1h 45m   2 sessions
#3  metrics extraction          45m   1 session

Total: 8h 30m across 12 issues
```

**`aist issue 4`**
```
ISSUE #4: bottleneck detection
══════════════════════════════════════════════════
Status: Closed (PR #10)
Branch: feature/issue-4-bottlenecks
Time:   2h 15m across 3 sessions

SESSIONS
────────────────────────────────────────
1. 2026-01-13 15:34 - 16:45 (1h 11m)
2. 2026-01-13 17:00 - 17:32 (32m)
3. 2026-01-13 17:45 - 18:17 (32m)

TIME BREAKDOWN
────────────────────────────────────────
Productive:  ████████████████░░░░ 78%
Reading:     ██░░░░░░░░░░░░░░░░░░ 12%
Errors:      █░░░░░░░░░░░░░░░░░░░  5%
Gaps:        █░░░░░░░░░░░░░░░░░░░  5%
```

---

## Future Ideas (Not Planned Yet)

- **Estimates vs Actuals**: If issues have time labels, compare estimated vs actual
- **Velocity tracking**: Issues completed per week trend
- **Team rollup**: Aggregate across multiple contributors
- **Cost tracking**: Estimate API costs per issue (tokens used)
- **CI integration**: Track build/test time as part of issue time

---

## Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Language | Rust | Fast, single binary, cross-platform |
| GitHub API | `gh` CLI | No OAuth setup, uses existing auth |
| Storage | JSON files | Simple, no database needed |
| Linking | PR → Branch → Session | Reliable, doesn't depend on branch naming |
| CI | GitHub Actions | fmt, clippy, build, test on every push/PR |
| Git hooks | pre-commit, pre-push | Local validation before pushing |

---

## Open Questions

1. **Multiple PRs per issue** - Sum time from all PRs?
2. **PRs closing multiple issues** - Split time evenly or attribute to all?
3. **Sessions spanning multiple branches** - Rare, but how to handle?
4. **Stale cache** - How often to re-sync? Auto-sync on `aist issues`?

---

*Last updated: 2026-01-14*
