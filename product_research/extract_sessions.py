#!/usr/bin/env python3
"""
Extract metrics from Claude Code session transcripts.

Usage:
    python extract_sessions.py > sessions.jsonl
    python extract_sessions.py --summary
"""

import json
import os
import sys
from pathlib import Path
from datetime import datetime
from collections import defaultdict
from typing import Optional
import re

CLAUDE_DIR = Path.home() / ".claude" / "projects"


def parse_timestamp(ts: str) -> Optional[datetime]:
    """Parse ISO timestamp."""
    if not ts:
        return None
    try:
        return datetime.fromisoformat(ts.replace("Z", "+00:00"))
    except:
        return None


def extract_session(jsonl_path: Path) -> dict:
    """Extract metrics from a single session JSONL file."""
    messages = []
    with open(jsonl_path, "r") as f:
        for line in f:
            try:
                messages.append(json.loads(line))
            except json.JSONDecodeError:
                continue

    if not messages:
        return None

    # Basic info
    session_id = None
    project = None
    git_branch = None
    timestamps = []

    # Tool usage
    tool_counts = defaultdict(int)
    tool_errors = defaultdict(int)
    error_samples = []

    # Message counts
    user_messages = 0
    assistant_messages = 0
    system_messages = 0

    # Files
    files_read = set()
    files_edited = set()

    # Subagents
    subagent_count = 0

    for msg in messages:
        msg_type = msg.get("type")

        # Extract session info
        if not session_id and msg.get("sessionId"):
            session_id = msg["sessionId"]
        if not project and msg.get("cwd"):
            project = msg["cwd"]
        if not git_branch and msg.get("gitBranch"):
            git_branch = msg["gitBranch"]
        if msg.get("timestamp"):
            ts = parse_timestamp(msg["timestamp"])
            if ts:
                timestamps.append(ts)

        # Count message types
        if msg_type == "user":
            user_messages += 1

            # Check for tool results (including errors)
            content = msg.get("message", {}).get("content", [])
            if isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get("type") == "tool_result":
                        is_error = item.get("is_error", False)
                        result_content = item.get("content", "")
                        if is_error or (isinstance(result_content, str) and
                                        re.search(r"error|failed|exception|not found",
                                                  result_content, re.I)):
                            error_samples.append(result_content[:200] if isinstance(result_content, str) else str(result_content)[:200])

        elif msg_type == "assistant":
            assistant_messages += 1

            # Extract tool calls
            content = msg.get("message", {}).get("content", [])
            if isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get("type") == "tool_use":
                        tool_name = item.get("name", "unknown")
                        tool_counts[tool_name] += 1

                        # Extract file paths from tool inputs
                        tool_input = item.get("input", {})
                        if isinstance(tool_input, dict):
                            file_path = tool_input.get("file_path") or tool_input.get("path")
                            if file_path:
                                if tool_name == "Read":
                                    files_read.add(file_path)
                                elif tool_name in ("Edit", "Write"):
                                    files_edited.add(file_path)

        elif msg_type == "system":
            system_messages += 1

    # Count subagents
    subagent_dir = jsonl_path.parent / "subagents"
    if subagent_dir.exists():
        subagent_count = len(list(subagent_dir.glob("*.jsonl")))

    # Calculate duration
    duration_minutes = None
    start_time = None
    end_time = None
    if timestamps:
        timestamps.sort()
        start_time = timestamps[0].isoformat()
        end_time = timestamps[-1].isoformat()
        duration_minutes = (timestamps[-1] - timestamps[0]).total_seconds() / 60

    return {
        "session_id": session_id,
        "jsonl_path": str(jsonl_path),
        "project": project,
        "git_branch": git_branch,
        "start_time": start_time,
        "end_time": end_time,
        "duration_minutes": round(duration_minutes, 1) if duration_minutes else None,
        "tool_counts": dict(tool_counts),
        "total_tool_calls": sum(tool_counts.values()),
        "errors": {
            "count": len(error_samples),
            "samples": error_samples[:5]  # First 5 errors
        },
        "messages": {
            "user": user_messages,
            "assistant": assistant_messages,
            "system": system_messages,
            "total": user_messages + assistant_messages + system_messages
        },
        "files": {
            "read": len(files_read),
            "edited": len(files_edited),
            "read_list": sorted(files_read)[:20],  # First 20
            "edited_list": sorted(files_edited)
        },
        "subagent_count": subagent_count
    }


def find_sessions():
    """Find all session JSONL files."""
    if not CLAUDE_DIR.exists():
        print(f"Claude directory not found: {CLAUDE_DIR}", file=sys.stderr)
        return []

    sessions = []
    for jsonl in CLAUDE_DIR.rglob("*.jsonl"):
        # Skip subagent files for main session list
        if "subagents" in str(jsonl):
            continue
        sessions.append(jsonl)

    return sessions


def print_summary(sessions):
    """Print summary statistics."""
    total_duration = sum(s.get("duration_minutes") or 0 for s in sessions)
    total_tools = sum(s.get("total_tool_calls") or 0 for s in sessions)
    total_errors = sum(s.get("errors", {}).get("count", 0) for s in sessions)

    # Aggregate tool counts
    all_tools = defaultdict(int)
    for s in sessions:
        for tool, count in s.get("tool_counts", {}).items():
            all_tools[tool] += count

    print("=" * 60)
    print("CLAUDE CODE SESSION ANALYSIS")
    print("=" * 60)
    print(f"\nSessions analyzed: {len(sessions)}")
    print(f"Total duration: {total_duration:.0f} minutes ({total_duration/60:.1f} hours)")
    print(f"Total tool calls: {total_tools}")
    print(f"Total errors detected: {total_errors}")

    print("\n--- Tool Usage ---")
    for tool, count in sorted(all_tools.items(), key=lambda x: -x[1]):
        print(f"  {tool}: {count}")

    print("\n--- Sessions by Project ---")
    by_project = defaultdict(list)
    for s in sessions:
        proj = s.get("project", "unknown")
        # Shorten project path
        proj = proj.replace(str(Path.home()), "~") if proj else "unknown"
        by_project[proj].append(s)

    for proj, proj_sessions in sorted(by_project.items(), key=lambda x: -len(x[1])):
        total_time = sum(s.get("duration_minutes") or 0 for s in proj_sessions)
        print(f"  {proj}: {len(proj_sessions)} sessions, {total_time:.0f} min")

    print("\n--- Longest Sessions ---")
    by_duration = sorted(sessions, key=lambda x: x.get("duration_minutes") or 0, reverse=True)
    for s in by_duration[:5]:
        proj = (s.get("project") or "").replace(str(Path.home()), "~")
        print(f"  {s.get('duration_minutes', 0):.0f} min - {proj}")

    print("\n--- Sessions with Most Errors ---")
    by_errors = sorted(sessions, key=lambda x: x.get("errors", {}).get("count", 0), reverse=True)
    for s in by_errors[:5]:
        if s.get("errors", {}).get("count", 0) == 0:
            break
        proj = (s.get("project") or "").replace(str(Path.home()), "~")
        print(f"  {s.get('errors', {}).get('count', 0)} errors - {proj}")
        for sample in s.get("errors", {}).get("samples", [])[:2]:
            print(f"    → {sample[:80]}...")


def main():
    sessions_paths = find_sessions()
    print(f"Found {len(sessions_paths)} session files", file=sys.stderr)

    sessions = []
    for path in sessions_paths:
        result = extract_session(path)
        if result:
            sessions.append(result)

    if "--summary" in sys.argv:
        print_summary(sessions)
    else:
        # Output as JSONL
        for s in sessions:
            print(json.dumps(s))


if __name__ == "__main__":
    main()
