#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.9"
# dependencies = ["neo4j>=5.0"]
# ///
"""
Ijoka Session End Hook (SQLite Version)

Records session end in database.
"""

import json
import os
import sys
from pathlib import Path

# Import shared database helper
sys.path.insert(0, str(Path(__file__).parent))
import graph_db_helper as db_helper


def main():
    try:
        hook_input = json.load(sys.stdin)
    except json.JSONDecodeError:
        hook_input = {}

    session_id = hook_input.get("session_id") or os.environ.get("CLAUDE_SESSION_ID", "unknown")
    project_dir = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())

    # End session in database
    db_helper.end_session(session_id)

    # Record session end event
    db_helper.insert_event(
        event_type="SessionEnd",
        source_agent="claude-code",
        session_id=session_id,
        project_dir=project_dir,
        payload={"action": "session_ended"}
    )

    # Output response
    print(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "SessionEnd"
        }
    }))


if __name__ == "__main__":
    main()
