# /set-feature

Explicitly set the active feature for activity tracking, including reopening completed features.

## When to Use

- When starting work that relates to a specific feature
- When the user's request relates to an existing feature (even if completed)
- When you need to reopen a completed feature for additional work
- When creating follow-up work for a completed feature

## Usage

```
/set-feature <description or index>
```

Examples:
- `/set-feature User authentication` - Match by description
- `/set-feature 3` - Set feature at index 3
- `/set-feature security:0` - First security feature

## What This Command Does

1. Reads `feature_list.json`
2. Finds the matching feature
3. If the feature is already complete (`passes: true`):
   - Prompts: "This feature is marked complete. Would you like to:"
     - A) Reopen it (set `passes: false`, `inProgress: true`)
     - B) Create a follow-up feature
4. Sets `inProgress: true` on the target feature
5. Clears `inProgress` from any other features
6. Confirms the active feature

## Smart Feature Detection

Before working on any task, Claude should:

1. **Analyze the user's request** - What are they asking for?
2. **Check feature_list.json** - Does this relate to an existing feature?
3. **Match by keywords** - Look for overlapping terms in descriptions
4. **Consider completed features** - If the request relates to a "done" feature, it may need reopening

## Instructions for Claude

When the user runs this command or when you identify that work relates to a specific feature:

1. Read `feature_list.json` and list all features with their status
2. Match the input to a feature by:
   - Exact index number
   - Partial description match
   - Category prefix (e.g., "security:0")
3. If no match found, suggest similar features or offer to create new
4. If matching a completed feature:
   ```
   This feature is marked complete:
   - "User authentication with OAuth" ✅

   Options:
   A) Reopen for additional work
   B) Create follow-up: "Fix/enhance: User authentication with OAuth"
   ```
5. Update feature_list.json:
   - Clear `inProgress` from all features
   - Set `inProgress: true` on target feature
   - If reopening: set `passes: false`
6. Confirm: "Now tracking activity for: [feature description]"

## Example Flow

```
User: "Fix the bug in the login form"

Claude thinks:
- User is asking to fix something in login
- Check features... "User authentication" exists and is complete
- This work relates to that feature
- Should run /set-feature to track properly

Claude: "This relates to the 'User authentication' feature which is marked complete.
Should I:
A) Reopen it for this bug fix
B) Create a new 'Bug fix: Login form issue' feature"

User: "A"

Claude: [Updates feature_list.json to reopen]
Claude: "Now tracking activity for: User authentication with OAuth"
[Proceeds to fix the bug, all tool calls are linked to this feature]
```

## Proactive Feature Management

Claude should proactively manage features by:

1. **At session start**: Check if there's an active feature
2. **Before any work**: Identify which feature the work relates to
3. **For new work**: Either find matching feature or create new one
4. **For fixes/enhancements**: Use /set-feature to properly attribute work

This ensures ALL activity is properly linked to features in AgentKanban.
