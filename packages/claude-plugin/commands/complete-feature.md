# Complete Feature Command

Mark the currently active feature as complete.

## Instructions

1. Read `feature_list.json` to find the feature with `inProgress: true`
2. Update that feature:
   - Set `passes: true`
   - Set `inProgress: false`
3. Save the file
4. Report what was completed and suggest next steps

## Example Update

```json
{
  "category": "functional",
  "description": "The feature that was completed",
  "steps": ["verification steps"],
  "passes": true,      // Changed from false
  "inProgress": false  // Changed from true
}
```

## After Completion

Show:
- Which feature was marked complete
- Current progress (X/Y features done)
- Next incomplete feature to work on

## Important

- Only mark a feature complete if the work is actually done
- If the user wants to continue working on it, don't complete it
- If no feature is currently active, report that and suggest using `/next-feature`
