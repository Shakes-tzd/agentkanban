#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""
AgentKanban Session Start Hook (SQLite Version)

Records session start in database and provides feature context to Claude.
Imports features from feature_list.json if not already in database.
"""

import json
import os
import sys
from pathlib import Path

# Import shared database helper
sys.path.insert(0, str(Path(__file__).parent))
import db_helper


def output_response(context: str) -> None:
    """Output JSON response with context."""
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": context
        }
    }))


def main():
    try:
        hook_input = json.load(sys.stdin)
    except json.JSONDecodeError:
        hook_input = {}

    session_id = hook_input.get("session_id") or os.environ.get("CLAUDE_SESSION_ID", "unknown")
    project_dir = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())

    # Record session start in database
    db_helper.start_session(session_id, "claude-code", project_dir)

    # Record session start event
    db_helper.insert_event(
        event_type="SessionStart",
        source_agent="claude-code",
        session_id=session_id,
        project_dir=project_dir,
        payload={"action": "session_started"}
    )

    # Get features from database
    features = db_helper.get_features(project_dir)

    # If no features in database, try to import from JSON (backward compatibility)
    if not features:
        feature_file = Path(project_dir) / "feature_list.json"
        if feature_file.exists():
            try:
                json_features = json.loads(feature_file.read_text())
                db_helper.sync_features_from_json(project_dir, json_features)
                features = db_helper.get_features(project_dir)
            except (json.JSONDecodeError, IOError):
                pass

    if not features:
        output_response("No feature_list.json found in this project. Consider creating one with /init-project command for structured task management.")
        return

    # Calculate stats
    total = len(features)
    completed = sum(1 for f in features if f.get("passes"))
    percentage = int(completed * 100 / total) if total > 0 else 0

    # Find active feature
    active_feature = None
    for f in features:
        if f.get("inProgress"):
            active_feature = f
            break

    if active_feature:
        # Active feature exists - show it with auto-completion info
        criteria_type = "manual"
        if active_feature.get("completionCriteria"):
            criteria_type = active_feature["completionCriteria"].get("type", "manual")
        work_count = active_feature.get("workCount", 0)

        context = f"""## Active Feature

**Currently Working On:** {active_feature['description']}

**Progress:** {completed}/{total} features complete ({percentage}%)

**Auto-Completion:** {criteria_type} | Work count: {work_count}

All tool calls will be linked to this feature. Features auto-complete when criteria are met (build passes, tests pass, or work count threshold reached).

---

**Switching features:** The system auto-detects when you're working on a different feature and switches automatically based on AI classification.

---"""
        output_response(context)
    else:
        # No active feature - show summary
        feature_lines = []
        for i, f in enumerate(features[:10]):
            status = "x" if f.get("passes") else " "
            desc = f.get("description", "")[:60]
            feature_lines.append(f"[{i}] [{status}] {desc}")

        feature_summary = "\n".join(feature_lines)

        context = f"""## No Active Feature

**Progress:** {completed}/{total} features complete ({percentage}%)

**Features:**
{feature_summary}

**Auto-Mode Active:** When you start working, the system will:
1. Auto-match your work to an existing feature (AI classification)
2. Auto-create a new feature if no match found
3. Auto-complete features when completion criteria are met

**Manual Commands (optional):**
- `/next-feature` - Manually select next feature
- `/complete-feature` - Force complete active feature"""
        output_response(context)


if __name__ == "__main__":
    main()
