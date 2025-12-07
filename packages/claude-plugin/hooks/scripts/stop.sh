#!/bin/bash
# AgentKanban Stop Hook
# Reports session end and feature status (auto-completion handled by PostToolUse)

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
FEATURE_FILE="${PROJECT_DIR}/feature_list.json"

# Read hook input from stdin
INPUT=$(cat)

# Extract stop reason and session info
STOP_REASON=$(echo "$INPUT" | jq -r '.stop_hook_input.stop_reason // "unknown"')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')

# Notify server of session stop
SYNC_SERVER="${AGENTKANBAN_SERVER:-http://127.0.0.1:4000}"
notify_server() {
    curl -s -X POST "${SYNC_SERVER}/sessions/end" \
        -H "Content-Type: application/json" \
        -d "{
            \"sessionId\": \"${SESSION_ID}\",
            \"sourceAgent\": \"claude-code\",
            \"projectDir\": \"${PROJECT_DIR}\",
            \"stopReason\": \"${STOP_REASON}\"
        }" --max-time 2 2>/dev/null || true
}

# Output JSON response
output_response() {
    local context="$1"
    jq -c -n --arg ctx "$context" '{hookSpecificOutput: {hookEventName: "Stop", additionalContext: $ctx}}'
}

# Report feature progress (informational only - completion is automatic)
report_progress() {
    if [ ! -f "$FEATURE_FILE" ]; then
        echo '{"hookSpecificOutput": {"hookEventName": "Stop"}}'
        return
    fi

    # Get progress stats
    local total=$(jq 'length' "$FEATURE_FILE")
    local completed=$(jq '[.[] | select(.passes == true)] | length' "$FEATURE_FILE")
    local percentage=0
    if [ "$total" -gt 0 ]; then
        percentage=$((completed * 100 / total))
    fi

    # Get active feature
    local active=$(jq -r '[.[] | select(.inProgress == true)][0].description // empty' "$FEATURE_FILE" | head -c 60)

    if [ -n "$active" ]; then
        # Active feature exists - show status
        local work_count=$(jq '[.[] | select(.inProgress == true)][0].workCount // 0' "$FEATURE_FILE")
        local criteria_type=$(jq -r '[.[] | select(.inProgress == true)][0].completionCriteria.type // "manual"' "$FEATURE_FILE")

        local context="**Session ended** | Progress: ${completed}/${total} (${percentage}%)
**Active:** ${active}
**Work count:** ${work_count} | **Completion:** ${criteria_type}"
        output_response "$context"
    else
        # No active feature
        output_response "**Session ended** | Progress: ${completed}/${total} (${percentage}%)"
    fi
}

# Notify server in background
notify_server >/dev/null 2>&1 &

# Report progress for end_turn stops
case "$STOP_REASON" in
    "end_turn")
        report_progress
        ;;
    *)
        echo '{"hookSpecificOutput": {"hookEventName": "Stop"}}'
        ;;
esac
