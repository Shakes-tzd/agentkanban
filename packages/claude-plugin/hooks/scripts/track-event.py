#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.9"
# dependencies = []
# ///
"""
AgentKanban Event Tracker
Unified script for tracking tool calls, stops, and subagent events.
Links events to the active feature in feature_list.json.
"""

import json
import os
import sys
from pathlib import Path
from urllib.request import urlopen, Request
from urllib.error import URLError

SYNC_SERVER = os.environ.get("AGENTKANBAN_SERVER", "http://127.0.0.1:4000")


def load_features(project_dir: str) -> list[dict] | None:
    """Load feature_list.json."""
    feature_file = Path(project_dir) / "feature_list.json"
    if not feature_file.exists():
        return None
    try:
        return json.loads(feature_file.read_text())
    except (json.JSONDecodeError, IOError):
        return None


def save_features(project_dir: str, features: list[dict]) -> bool:
    """Save feature_list.json."""
    feature_file = Path(project_dir) / "feature_list.json"
    try:
        feature_file.write_text(json.dumps(features, indent=2))
        return True
    except IOError:
        return False


def get_active_feature(project_dir: str) -> dict | None:
    """Get the currently active feature (inProgress: true).

    Returns feature with ID in format 'project_dir:index' to match database storage.
    """
    features = load_features(project_dir)
    if not features:
        return None

    for index, feature in enumerate(features):
        if feature.get("inProgress"):
            # ID format must match database: project_dir:index
            feature_id = f"{project_dir}:{index}"
            return {
                "id": feature_id,
                "index": index,
                "description": feature.get("description"),
                "category": feature.get("category", "functional"),
                "completionCriteria": feature.get("completionCriteria"),
                "workCount": feature.get("workCount", 0)
            }

    return None


def check_completion_criteria(
    feature: dict,
    tool_name: str,
    tool_input: dict,
    tool_result: dict
) -> tuple[bool, str]:
    """Check if a tool call satisfies the feature's completion criteria."""
    import re

    criteria = feature.get("completionCriteria", {})
    criteria_type = criteria.get("type", "manual")

    # Check if tool result indicates success
    is_error = tool_result.get("is_error", False)
    if is_error:
        return False, ""

    # Check based on criteria type
    if criteria_type == "build":
        if tool_name == "Bash":
            cmd = tool_input.get("command", "").lower()
            pattern = criteria.get("command_pattern", "")
            if pattern:
                if re.search(pattern, cmd, re.IGNORECASE):
                    return True, "Build passed"
            elif any(kw in cmd for kw in ["build", "compile", "cargo build", "pnpm build", "npm run build"]):
                return True, "Build passed"

    elif criteria_type == "test":
        if tool_name == "Bash":
            cmd = tool_input.get("command", "").lower()
            if any(kw in cmd for kw in ["test", "pytest", "jest", "vitest", "cargo test"]):
                return True, "Tests passed"

    elif criteria_type == "lint":
        if tool_name == "Bash":
            cmd = tool_input.get("command", "").lower()
            if any(kw in cmd for kw in ["lint", "eslint", "prettier", "clippy"]):
                return True, "Lint passed"

    elif criteria_type == "work_count":
        # Check if work count threshold reached (handled separately)
        pass

    elif criteria_type == "any_success":
        if tool_name in {"Edit", "Write", "Bash"}:
            return True, "Work completed"

    return False, ""


def maybe_auto_complete(
    project_dir: str,
    tool_name: str,
    tool_input: dict,
    tool_result: dict
) -> str | None:
    """Check if the active feature should be auto-completed. Returns status message."""
    features = load_features(project_dir)
    if not features:
        return None

    # Find active feature
    active_idx = None
    active_feature = None
    for i, f in enumerate(features):
        if f.get("inProgress"):
            active_idx = i
            active_feature = f
            break

    if active_idx is None or active_feature is None:
        return None

    # Skip if already complete
    if active_feature.get("passes"):
        return None

    # Check if this is a "work" tool (not read-only)
    is_work_tool = tool_name in {"Edit", "Write", "Bash", "Task"}
    is_error = tool_result.get("is_error", False)

    # Increment work count for successful work tools
    if is_work_tool and not is_error:
        work_count = active_feature.get("workCount", 0) + 1
        features[active_idx]["workCount"] = work_count

        # Check work_count completion criteria
        criteria = active_feature.get("completionCriteria", {})
        if criteria.get("type") == "work_count":
            threshold = criteria.get("count", 3)
            if work_count >= threshold:
                features[active_idx]["passes"] = True
                features[active_idx]["inProgress"] = False
                activate_next_feature(features)
                save_features(project_dir, features)
                return f"Auto-completed (work count: {work_count})"

    # Check other completion criteria
    is_complete, reason = check_completion_criteria(
        active_feature, tool_name, tool_input, tool_result
    )

    if is_complete:
        features[active_idx]["passes"] = True
        features[active_idx]["inProgress"] = False
        activate_next_feature(features)
        save_features(project_dir, features)
        return f"Auto-completed: {reason}"

    # Save updated work count even if not complete
    if is_work_tool and not is_error:
        save_features(project_dir, features)

    return None


def activate_next_feature(features: list[dict]) -> int | None:
    """Activate the next incomplete feature. Returns its index."""
    for i, f in enumerate(features):
        if not f.get("passes") and not f.get("inProgress"):
            features[i]["inProgress"] = True
            return i
    return None


def send_event(event_data: dict) -> bool:
    """Send event to AgentKanban server."""
    try:
        data = json.dumps(event_data).encode()
        req = Request(
            f"{SYNC_SERVER}/events",
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST"
        )
        with urlopen(req, timeout=2) as resp:
            return resp.status == 200
    except (URLError, TimeoutError, OSError):
        return False


def extract_file_paths(tool_input: dict) -> list[str]:
    """Extract file paths from tool input."""
    paths = []

    # Direct file_path
    if "file_path" in tool_input:
        paths.append(tool_input["file_path"])

    # Glob pattern
    if "pattern" in tool_input:
        paths.append(f"glob:{tool_input['pattern']}")

    # Bash command - extract paths heuristically
    if "command" in tool_input:
        cmd = tool_input["command"]
        # Just note the command type, don't parse all paths
        paths.append(f"bash:{cmd[:50]}...")

    return paths


def handle_post_tool_use(hook_input: dict, project_dir: str, session_id: str):
    """Handle PostToolUse events - track all tool calls."""
    tool_name = hook_input.get("tool_name", "unknown")
    tool_input = hook_input.get("tool_input", {})
    tool_result = hook_input.get("tool_result", {})

    # Skip tracking the tracking script itself
    if "track-event.py" in str(tool_input):
        return

    # Get active feature
    active_feature = get_active_feature(project_dir)

    # Build detailed payload based on tool type
    payload = {
        "filePaths": extract_file_paths(tool_input),
        "inputSummary": summarize_input(tool_name, tool_input),
        "success": not tool_result.get("is_error", False)
    }

    # Add tool-specific details
    if tool_name == "Edit":
        payload["oldString"] = (tool_input.get("old_string", "")[:200] + "...") if len(tool_input.get("old_string", "")) > 200 else tool_input.get("old_string", "")
        payload["newString"] = (tool_input.get("new_string", "")[:200] + "...") if len(tool_input.get("new_string", "")) > 200 else tool_input.get("new_string", "")
        payload["filePath"] = tool_input.get("file_path", "")
    elif tool_name == "Bash":
        payload["command"] = tool_input.get("command", "")[:500]
        payload["description"] = tool_input.get("description", "")
        # Include output preview if available
        output = tool_result.get("output", "")
        if output:
            payload["outputPreview"] = (output[:300] + "...") if len(output) > 300 else output
    elif tool_name == "Read":
        payload["filePath"] = tool_input.get("file_path", "")
        payload["offset"] = tool_input.get("offset")
        payload["limit"] = tool_input.get("limit")
    elif tool_name == "Write":
        payload["filePath"] = tool_input.get("file_path", "")
        content = tool_input.get("content", "")
        payload["contentPreview"] = (content[:200] + "...") if len(content) > 200 else content
    elif tool_name == "Grep":
        payload["pattern"] = tool_input.get("pattern", "")
        payload["path"] = tool_input.get("path", "")
        payload["glob"] = tool_input.get("glob", "")
    elif tool_name == "Glob":
        payload["pattern"] = tool_input.get("pattern", "")
        payload["path"] = tool_input.get("path", "")

    # Build event
    event = {
        "eventType": "ToolCall",
        "sourceAgent": "claude-code",
        "sessionId": session_id,
        "projectDir": project_dir,
        "toolName": tool_name,
        "payload": payload
    }

    if active_feature:
        event["featureId"] = active_feature["id"]
        event["payload"]["featureCategory"] = active_feature["category"]
        event["payload"]["featureDescription"] = active_feature["description"]

    send_event(event)

    # Check for auto-completion after tracking the event
    completion_status = maybe_auto_complete(project_dir, tool_name, tool_input, tool_result)
    if completion_status:
        # Add completion info to event payload for observability
        completion_event = {
            "eventType": "FeatureCompleted",
            "sourceAgent": "claude-code",
            "sessionId": session_id,
            "projectDir": project_dir,
            "payload": {
                "completionStatus": completion_status,
                "triggeredBy": tool_name
            }
        }
        if active_feature:
            completion_event["featureId"] = active_feature["id"]
            completion_event["payload"]["featureDescription"] = active_feature["description"]
        send_event(completion_event)


def handle_stop(hook_input: dict, project_dir: str, session_id: str):
    """Handle Stop events - agent finished."""
    stop_hook_input = hook_input.get("stop_hook_input", {})

    event = {
        "eventType": "AgentStop",
        "sourceAgent": "claude-code",
        "sessionId": session_id,
        "projectDir": project_dir,
        "payload": {
            "reason": stop_hook_input.get("stop_reason", "unknown"),
            "lastMessage": (stop_hook_input.get("last_assistant_message", "") or "")[:200]
        }
    }

    send_event(event)


def handle_subagent_stop(hook_input: dict, project_dir: str, session_id: str):
    """Handle SubagentStop events - Task tool finished."""
    tool_input = hook_input.get("tool_input", {})
    tool_result = hook_input.get("tool_result", {})

    event = {
        "eventType": "SubagentStop",
        "sourceAgent": "claude-code",
        "sessionId": session_id,
        "projectDir": project_dir,
        "toolName": "Task",
        "payload": {
            "taskDescription": tool_input.get("description", ""),
            "subagentType": tool_input.get("subagent_type", ""),
            "success": not tool_result.get("is_error", False),
            "resultSummary": (str(tool_result.get("output", ""))[:200] if tool_result else "")
        }
    }

    active_feature = get_active_feature(project_dir)
    if active_feature:
        event["featureId"] = active_feature["id"]
        event["payload"]["featureDescription"] = active_feature["description"]

    send_event(event)


def handle_user_prompt_submit(hook_input: dict, project_dir: str, session_id: str):
    """Handle UserPromptSubmit events - capture user queries for observability."""
    # Extract user prompt from hook input
    user_prompt = hook_input.get("user_prompt", "")
    if not user_prompt:
        # Try alternative field names
        user_prompt = hook_input.get("prompt", "") or hook_input.get("message", "")

    event = {
        "eventType": "UserQuery",
        "sourceAgent": "claude-code",
        "sessionId": session_id,
        "projectDir": project_dir,
        "payload": {
            "prompt": user_prompt[:1000],  # Truncate long prompts
            "promptLength": len(user_prompt),
            "preview": user_prompt[:200] if user_prompt else ""
        }
    }

    # Get active feature
    active_feature = get_active_feature(project_dir)
    if active_feature:
        event["featureId"] = active_feature["id"]
        event["payload"]["featureDescription"] = active_feature["description"]

    send_event(event)


def summarize_input(tool_name: str, tool_input: dict) -> str:
    """Create a brief summary of the tool input."""
    if tool_name == "Read":
        return f"Read: {tool_input.get('file_path', 'unknown')}"
    elif tool_name == "Write":
        return f"Write: {tool_input.get('file_path', 'unknown')}"
    elif tool_name == "Edit":
        return f"Edit: {tool_input.get('file_path', 'unknown')}"
    elif tool_name == "Bash":
        cmd = tool_input.get("command", "")
        return f"Bash: {cmd[:60]}..." if len(cmd) > 60 else f"Bash: {cmd}"
    elif tool_name == "Glob":
        return f"Glob: {tool_input.get('pattern', 'unknown')}"
    elif tool_name == "Grep":
        return f"Grep: {tool_input.get('pattern', 'unknown')}"
    elif tool_name == "Task":
        return f"Task: {tool_input.get('description', 'unknown')}"
    else:
        return f"{tool_name}: {str(tool_input)[:60]}"


def main():
    # Get hook type from environment (set by wrapper or hooks.json)
    hook_type = os.environ.get("AGENTKANBAN_HOOK_TYPE", "PostToolUse")

    # Read hook input from stdin
    try:
        hook_input = json.load(sys.stdin)
    except json.JSONDecodeError:
        print(json.dumps({"hookSpecificOutput": {"hookEventName": hook_type}}))
        return

    # Get session_id from hook input or environment
    session_id = hook_input.get("session_id") or os.environ.get("CLAUDE_SESSION_ID", "unknown")

    # Get project directory from environment (Claude Code sets this)
    project_dir = os.environ.get("CLAUDE_PROJECT_DIR", "")
    if not project_dir:
        # Fallback: try to detect from tool input file paths
        tool_input = hook_input.get("tool_input", {})
        file_path = tool_input.get("file_path", "")
        if file_path:
            path = Path(file_path)
            for parent in [path] + list(path.parents):
                if (parent / "feature_list.json").exists():
                    project_dir = str(parent)
                    break

    if not project_dir:
        project_dir = os.getcwd()

    # Route to appropriate handler
    if hook_type == "PostToolUse":
        handle_post_tool_use(hook_input, project_dir, session_id)
    elif hook_type == "Stop":
        handle_stop(hook_input, project_dir, session_id)
    elif hook_type == "SubagentStop":
        handle_subagent_stop(hook_input, project_dir, session_id)
    elif hook_type == "UserPromptSubmit":
        handle_user_prompt_submit(hook_input, project_dir, session_id)

    # Always continue - include hookEventName in hookSpecificOutput
    print(json.dumps({"hookSpecificOutput": {"hookEventName": hook_type}}))


if __name__ == "__main__":
    main()
