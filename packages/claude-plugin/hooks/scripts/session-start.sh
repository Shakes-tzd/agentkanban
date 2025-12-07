#!/bin/bash
# AgentKanban Session Start Hook
# Loads feature_list.json context and notifies the sync server

set -e

# Read hook input from stdin
INPUT=$(cat)

# Extract session info
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"

# Sync server URL (AgentKanban desktop app)
SYNC_SERVER="${AGENTKANBAN_SERVER:-http://127.0.0.1:4000}"

# Notify sync server of session start
notify_server() {
    curl -s -X POST "${SYNC_SERVER}/sessions/start" \
        -H "Content-Type: application/json" \
        -d "{
            \"sessionId\": \"${SESSION_ID}\",
            \"sourceAgent\": \"claude-code\",
            \"projectDir\": \"${PROJECT_DIR}\"
        }" --max-time 2 2>/dev/null || true
}

# Output JSON response using jq for proper escaping
output_response() {
    local context="$1"
    jq -c -n --arg ctx "$context" '{hookSpecificOutput: {hookEventName: "SessionStart", additionalContext: $ctx}}'
}

# Load feature list if it exists
load_features() {
    local feature_file="${PROJECT_DIR}/feature_list.json"

    if [ -f "$feature_file" ]; then
        # Count features
        local total=$(jq 'length' "$feature_file")
        local completed=$(jq '[.[] | select(.passes == true)] | length' "$feature_file")
        local percentage=0

        if [ "$total" -gt 0 ]; then
            percentage=$((completed * 100 / total))
        fi

        # Check for active feature (inProgress: true)
        local active_feature=$(jq -r '[.[] | select(.inProgress == true)][0].description // empty' "$feature_file")

        # Get next incomplete feature
        local next_feature=$(jq -r '[.[] | select(.passes == false and .inProgress != true)][0].description // "None"' "$feature_file")

        if [ -n "$active_feature" ]; then
            # Get completion criteria info
            local criteria_type=$(jq -r '[.[] | select(.inProgress == true)][0].completionCriteria.type // "manual"' "$feature_file")
            local work_count=$(jq '[.[] | select(.inProgress == true)][0].workCount // 0' "$feature_file")

            # Active feature exists - show it with auto-completion info
            local context="## Active Feature

**Currently Working On:** ${active_feature}

**Progress:** ${completed}/${total} features complete (${percentage}%)

**Auto-Completion:** ${criteria_type} | Work count: ${work_count}

All tool calls will be linked to this feature. Features auto-complete when criteria are met (build passes, tests pass, or work count threshold reached).

---

**Switching features:** The system auto-detects when you're working on a different feature and switches automatically based on AI classification.

---"
            output_response "$context"
        else
            # No active feature - auto-create will handle it
            local feature_summary=$(jq -r 'to_entries | .[:10] | .[] | "[\(.key)] \(if .value.passes then "✅" else "⬜" end) \(.value.description | .[0:60])"' "$feature_file" 2>/dev/null | head -10)

            local context="## No Active Feature

**Progress:** ${completed}/${total} features complete (${percentage}%)

**Features:**
${feature_summary}

**Auto-Mode Active:** When you start working, the system will:
1. Auto-match your work to an existing feature (AI classification)
2. Auto-create a new feature if no match found
3. Auto-complete features when completion criteria are met

**Manual Commands (optional):**
- \`/next-feature\` - Manually select next feature
- \`/complete-feature\` - Force complete active feature"
            output_response "$context"
        fi
    else
        # No feature list - suggest creating one
        output_response "No feature_list.json found in this project. Consider creating one with /init-project command for structured task management."
    fi
}

# Run in background to not block session start
# Redirect all output to /dev/null to prevent contaminating JSON stdout
notify_server >/dev/null 2>&1 &

# Load features and output context
load_features
