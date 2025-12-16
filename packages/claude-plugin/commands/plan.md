# /plan

Set or view the implementation plan (steps) for the current feature.

## Usage

```
/plan <step1> | <step2> | <step3>
```

Or to view current plan:
```
/plan
```

## What This Does

Creates implementation steps in Ijoka's database for the active feature. These steps are:
- Tracked in Memgraph as Step nodes linked to the Feature
- Visible in the Phoenix dashboard
- Used for progress tracking and drift detection

**IMPORTANT**: Use this command instead of internal todo tracking. This ensures all work is tracked in Ijoka.

## Behavior

1. Check if there are arguments
2. If no arguments: GET /plan to show current steps
3. If arguments: POST /plan to set new steps
4. Display confirmation with step count

## Instructions

Run these commands:

```bash
if [ -z "$ARGUMENTS" ]; then
  # No arguments - show current plan
  PLAN=$(curl -s http://localhost:8000/plan)
  STEPS=$(echo "$PLAN" | jq -r '.steps // []')
  STEP_COUNT=$(echo "$STEPS" | jq 'length')

  if [ "$STEP_COUNT" = "0" ]; then
    echo "No plan set for current feature."
    echo "Usage: /plan <step1> | <step2> | <step3>"
  else
    echo "Current plan ($STEP_COUNT steps):"
    echo "$STEPS" | jq -r '.[] | "  - \(.status == "completed" | if . then "[x]" else "[ ]" end) \(.description)"'
  fi
else
  # Parse pipe-separated steps
  STEPS_JSON=$(echo "$ARGUMENTS" | tr '|' '\n' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | grep -v '^$' | jq -R -s 'split("\n") | map(select(length > 0))')

  # Set the plan via API
  RESPONSE=$(curl -s -X POST http://localhost:8000/plan \
    -H "Content-Type: application/json" \
    -d "{\"steps\": $STEPS_JSON}")

  STEP_COUNT=$(echo "$RESPONSE" | jq -r '.steps | length')
  echo "Plan set with $STEP_COUNT steps:"
  echo "$RESPONSE" | jq -r '.steps[] | "  - [ ] \(.description)"'
fi
```

## Examples

Set a multi-step plan:
```
/plan Create API endpoint | Add database queries | Write tests | Update documentation
```

View current plan:
```
/plan
```

## When to Use

- At the start of working on a feature to define implementation steps
- When breaking down work into trackable phases
- Instead of using internal todo tracking
- When you need clear progress visibility

## Why Use This Instead of TodoWrite

1. **Persistent Storage**: Steps are stored in Memgraph, not session memory
2. **Dashboard Visibility**: Steps appear in Phoenix UI
3. **Cross-Session**: Plan persists across Claude sessions
4. **Attribution**: Events are linked to steps for progress tracking
5. **Team Visibility**: Other agents can see and update the plan
