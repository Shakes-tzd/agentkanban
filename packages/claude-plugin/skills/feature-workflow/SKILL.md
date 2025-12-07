# Feature Workflow Skill

This skill provides guidance for working with `feature_list.json` files following Anthropic's long-running agent pattern.

## When This Skill Activates

This skill should be used when:
- A `feature_list.json` file exists in the project
- User asks about project tasks or features
- User wants to track progress on a project
- Starting a new coding session in a project

## The Pattern

`feature_list.json` is a persistent task queue that survives across sessions. It solves the core challenge of long-running agents: maintaining context across multiple sessions.

### File Structure

```json
[
  {
    "category": "functional",
    "description": "Human-readable description of what the feature does",
    "steps": ["Step 1", "Step 2", "Step 3"],
    "passes": false
  }
]
```

### Fields

- **category**: Groups features (functional, ui, security, performance, documentation, testing, infrastructure, refactoring)
- **description**: What the feature does (human-readable)
- **steps**: How to verify it works (test script for agent)
- **passes**: `false` = not done, `true` = done

### Optional Fields

- **inProgress**: `true` if currently being worked on
- **agent**: Which agent is working on this feature

## CRITICAL: Activity Tracking

**All tool calls are linked to the active feature (inProgress: true) in AgentKanban.**

This means:
- If no feature is active, activities are NOT tracked
- If the wrong feature is active, activities are misattributed
- You MUST ensure the correct feature is active before doing any work

## Workflow

### At Session Start

1. Read `feature_list.json`
2. Check overall progress (X/Y complete)
3. Identify features where `passes: false`
4. Note any features with `inProgress: true`

### BEFORE Any Work (CRITICAL)

Before implementing anything, you MUST:

1. **Analyze the user's request** - What are they asking for?
2. **Check feature_list.json** - Does this relate to an existing feature?
3. **Match to a feature** - Find the most relevant feature by:
   - Keywords in descriptions
   - Category match
   - Related functionality
4. **Handle completed features** - If the work relates to a completed feature:
   - **Option A**: Reopen it (set `passes: false`, `inProgress: true`)
   - **Option B**: Create a follow-up feature
5. **Set the active feature** - Ensure `inProgress: true` is set on the correct feature

### Working on Completed Features

If a user asks to fix/enhance something related to a completed feature:

```
User: "Fix the login form validation"

Claude:
1. Checks features... "User authentication" is marked complete
2. This relates to that feature
3. Ask user:
   "This relates to 'User authentication' which is complete.
   Should I:
   A) Reopen it for this fix
   B) Create a new bug-fix feature"
4. Update feature_list.json accordingly
5. Proceed with the fix (now properly tracked)
```

### During Session

1. Ensure correct feature has `inProgress: true`
2. Implement the feature thoroughly
3. Test using the verification steps
4. When complete:
   - Set `passes: true`
   - Set `inProgress: false`
5. Commit the code changes

### Critical Rules

> "It is unacceptable to remove or edit tests because this could lead to missing or buggy functionality."

1. **ALWAYS set a feature active** before doing work
2. **Match work to features** - Don't just use whatever is inProgress
3. **Reopen or create follow-ups** for work on completed features
4. **NEVER remove features** from the list
5. **NEVER edit feature descriptions** or steps (except adding follow-ups)
6. **Work on ONE feature** at a time
7. **Complete fully** before marking as done
8. **Leave code in working state** at session end

## Why JSON Not Markdown?

> "After experimentation, we landed on using JSON for this, as the model is less likely to inappropriately change or overwrite JSON files compared to Markdown files."

JSON feels like data. Markdown feels editable. Claude is more careful with structured data.

## Commands Available

- `/init-project` - Create feature_list.json
- `/feature-status` - Show progress and next tasks
- `/next-feature` - Start the next incomplete feature

## Integration with AgentKanban

When AgentKanban is running:
- Features sync to the desktop kanban board
- Progress updates trigger notifications
- Activity is logged to the timeline
- Multiple agents can be coordinated

## Example Session Flow

```
Session Start:
→ Hook loads feature_list.json
→ "Progress: 5/12 features complete (42%)"
→ "Next: [security] Input validation"

Working:
→ Claude picks up "Input validation" feature
→ Sets inProgress: true
→ Implements validation middleware
→ Tests against verification steps
→ Sets passes: true, inProgress: false
→ Commits code

Session End:
→ Code is in working state
→ Progress: 6/12 (50%)
→ Next session can continue seamlessly
```
