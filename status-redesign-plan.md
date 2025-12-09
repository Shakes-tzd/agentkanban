# Feature Status Redesign Plan

## Problem Analysis

### Current State
```
FEATURE STATUS DISTRIBUTION
  pending: 21 (100%)
  in_progress: 0
  complete: 0

STATUS EVENTS (Temporal Pattern)
  (no StatusEvent nodes found)
```

**Root Cause**: The status management is a remnant of the `feature_list.json` era where:
- Only ONE feature could be "in_progress" at a time (to prevent drift)
- Status was explicitly set via agent prompts or MCP tools
- The single-feature model enforced serialized work

**New Reality with Graph DB**:
- Activities are linked to features automatically
- Multiple features CAN be worked on in parallel (no file conflicts)
- Status should be DERIVED from activity, not just explicitly set
- But nothing triggers the pending → in_progress transition

---

## Design Philosophy (from temporal_design.md)

### Status as Events, Not Mutable State

```
OLD: Mutate in place (history lost)
  MATCH (f:Feature) SET f.status = 'in_progress'

NEW: Status events (history IS the structure)
  CREATE (f)<-[:CHANGED_STATUS]-(e:StatusEvent {
      from: 'pending',
      to: 'in_progress',
      at: timestamp(),
      by: 'activity:event-123',
      session: 'session-456'
  })
```

### Query Current Status
```cypher
// Get current status (latest StatusEvent)
MATCH (f:Feature {id: $id})<-[:CHANGED_STATUS]-(se:StatusEvent)
RETURN se.to_status as status
ORDER BY se.at DESC
LIMIT 1
```

### Query Status History
```cypher
// Full status timeline
MATCH (f:Feature {id: $id})<-[:CHANGED_STATUS]-(se:StatusEvent)
RETURN se.from_status, se.to_status, se.at, se.by
ORDER BY se.at ASC
```

---

## Proposed Architecture

### 1. StatusEvent Node Schema

```cypher
(:StatusEvent {
    id: string,           // UUID
    from_status: string,  // 'pending', 'in_progress', 'blocked', 'complete'
    to_status: string,
    at: datetime,
    by: string,           // What triggered: 'activity:evt-123', 'mcp:ijoka_start_feature', 'auto:first_activity'
    session_id: string,   // Optional: which session triggered this
    reason: string        // Optional: human-readable reason
})

// Relationship
(:StatusEvent)-[:CHANGED_STATUS]->(:Feature)
```

### 2. Status Transition Rules

| Trigger | From | To | `by` Field |
|---------|------|-----|------------|
| First activity linked to feature | pending | in_progress | `auto:first_activity` |
| `ijoka_start_feature` MCP call | pending | in_progress | `mcp:ijoka_start_feature` |
| `ijoka_complete_feature` MCP call | in_progress | complete | `mcp:ijoka_complete_feature` |
| `ijoka_block_feature` MCP call | in_progress | blocked | `mcp:ijoka_block_feature` |
| Auto-complete criteria met | in_progress | complete | `auto:build_passed`, `auto:test_passed`, etc. |
| No activity for 24h (optional) | in_progress | stale | `auto:inactivity` |

### 3. Parallel Work Support

**Remove the single-active-feature constraint:**

```cypher
// OLD: Only one feature in_progress
MATCH (f:Feature {status: 'in_progress'}) RETURN f LIMIT 1

// NEW: Multiple features can be in_progress
MATCH (f:Feature)
WHERE f.status = 'in_progress'
OR EXISTS {
    MATCH (f)<-[:LINKED_TO]-(e:Event)
    WHERE e.timestamp > datetime() - duration('PT4H')
}
RETURN f
```

**Session-Scoped Activity View:**
```cypher
// What's this session working on?
MATCH (s:Session {id: $sessionId})-[:TRIGGERED]->(e:Event)-[:LINKED_TO]->(f:Feature)
RETURN DISTINCT f
```

### 4. Activity-Inferred Status (Hybrid Approach)

Features have TWO status indicators:
1. **Explicit Status**: The `status` property (or latest StatusEvent)
2. **Activity Status**: Derived from recent events

```typescript
interface FeatureWithActivity {
    id: string;
    description: string;

    // Explicit status
    status: 'pending' | 'in_progress' | 'blocked' | 'complete';

    // Activity-derived
    lastActivityAt: Date | null;
    activityStatus: 'idle' | 'active' | 'stale';  // Based on recency
    eventCount: number;
    workCount: number;
}
```

**Activity Status Rules:**
- `active`: Has events in last 4 hours
- `stale`: Has events but none in last 24 hours
- `idle`: No events ever

---

## Implementation Plan

### Phase 1: Add StatusEvent Infrastructure

**1.1 Create StatusEvent in graph_db_helper.py**
```python
def create_status_event(
    feature_id: str,
    from_status: str,
    to_status: str,
    triggered_by: str,
    session_id: str = None,
    reason: str = None
) -> dict:
    """Create a StatusEvent and link to feature."""
    event_id = str(uuid.uuid4())
    now = datetime.now(timezone.utc).isoformat()

    run_write_query("""
        MATCH (f:Feature {id: $featureId})
        CREATE (se:StatusEvent {
            id: $eventId,
            from_status: $fromStatus,
            to_status: $toStatus,
            at: datetime($now),
            by: $triggeredBy,
            session_id: $sessionId,
            reason: $reason
        })-[:CHANGED_STATUS]->(f)
        SET f.status = $toStatus,
            f.updated_at = datetime($now)
    """, {...})
```

**1.2 Create StatusEvent in MCP server db.ts**
```typescript
export async function createStatusEvent(
    featureId: string,
    fromStatus: FeatureStatus,
    toStatus: FeatureStatus,
    triggeredBy: string,
    sessionId?: string,
    reason?: string
): Promise<void> { ... }
```

### Phase 2: Auto-Transition on First Activity

**2.1 Modify activity linking in track-event.py**
```python
def link_event_to_feature(event_id: str, feature_id: str, project_dir: str):
    """Link event to feature and auto-transition if needed."""

    # Get current feature status
    feature = get_feature_by_id(feature_id)

    # Auto-transition pending → in_progress on first activity
    if feature['status'] == 'pending':
        create_status_event(
            feature_id=feature_id,
            from_status='pending',
            to_status='in_progress',
            triggered_by=f'auto:first_activity:{event_id}',
            reason='Automatically started when first activity was linked'
        )

    # Link the event
    run_write_query("""
        MATCH (e:Event {id: $eventId})
        MATCH (f:Feature {id: $featureId})
        MERGE (e)-[:LINKED_TO]->(f)
    """, {...})
```

**2.2 Update MCP server handlers**

In `ijoka_start_feature`:
```typescript
// Create StatusEvent for explicit start
await db.createStatusEvent(
    feature.id,
    'pending',
    'in_progress',
    'mcp:ijoka_start_feature',
    undefined,
    `Started by agent: ${agent}`
);
```

### Phase 3: Fix Existing Data

**3.1 Migration script to create StatusEvents for existing features**
```cypher
// For features with activities but still pending, create StatusEvents
MATCH (f:Feature {status: 'pending'})
WHERE EXISTS {
    MATCH (e:Event)-[:BELONGS_TO]->(f)
}
WITH f, datetime() as now
CREATE (se:StatusEvent {
    id: randomUUID(),
    from_status: 'pending',
    to_status: 'in_progress',
    at: now,
    by: 'migration:backfill',
    reason: 'Backfilled based on existing activity'
})-[:CHANGED_STATUS]->(f)
SET f.status = 'in_progress', f.updated_at = now
```

**3.2 Re-classify activities to proper features**

This requires analyzing the activity content and matching to feature descriptions:
```python
def reclassify_activities():
    """Re-analyze unlinked or mislinked activities."""

    # Get all events linked to Session Work that shouldn't be
    events = get_events_for_session_work()

    for event in events:
        # Analyze event content
        if not is_meta_tool(event['tool_name']):
            # Find best matching feature
            best_feature = classify_event_to_feature(event)
            if best_feature:
                relink_event(event['id'], best_feature['id'])
```

### Phase 4: Update UI to Show Activity Status

**4.1 Add activity indicators to feature cards**
```vue
<template>
  <div class="feature-card" :class="activityStatusClass">
    <div class="status-badge">{{ feature.status }}</div>
    <div class="activity-indicator" v-if="feature.lastActivityAt">
      <span class="pulse" v-if="isActive"></span>
      {{ timeAgo(feature.lastActivityAt) }}
    </div>
  </div>
</template>
```

**4.2 Update kanban columns**
- "To Do" = `status: pending` AND no recent activity
- "In Progress" = `status: in_progress` OR has recent activity
- "Done" = `status: complete`
- Consider a "Stale" indicator for features with old activity

---

## Immediate Actions

### 1. Fix Current Feature Statuses (NOW)

```cypher
// Update features that have the "Claude plugin installs and hooks work"
// as the only active feature (legacy from feature_list.json)

// First, find features with actual implementation work
MATCH (f:Feature)-[:BELONGS_TO]->(p:Project {path: '/Users/shakes/DevProjects/ijoka'})
WHERE f.description CONTAINS 'Phase 1'
   OR f.description CONTAINS 'Phase 2'
   OR f.description CONTAINS 'MCP'
   OR f.description CONTAINS 'graph'
WITH f
SET f.status = 'in_progress', f.updated_at = datetime()
RETURN f.description
```

### 2. Create StatusEvent Nodes for Audit Trail

```cypher
// Backfill StatusEvents for all features currently in_progress
MATCH (f:Feature {status: 'in_progress'})
WHERE NOT EXISTS {
    MATCH (se:StatusEvent)-[:CHANGED_STATUS]->(f)
    WHERE se.to_status = 'in_progress'
}
CREATE (se:StatusEvent {
    id: randomUUID(),
    from_status: 'pending',
    to_status: 'in_progress',
    at: datetime(),
    by: 'migration:initial_backfill'
})-[:CHANGED_STATUS]->(f)
```

### 3. Implement Auto-Transition in Hook Scripts

Modify `track-event.py` to auto-transition features when activities are linked.

---

## Summary

| Aspect | Old (feature_list.json) | New (Graph DB) |
|--------|------------------------|----------------|
| Status storage | Mutable `status` field | StatusEvent chain + derived |
| Parallel work | Forbidden (drift risk) | Allowed (no file conflicts) |
| Status change | Explicit only | Auto on first activity + explicit |
| History | None | Full audit trail |
| Active feature | Single (`inProgress: true`) | Multiple (activity-based) |

This redesign:
1. **Preserves backward compatibility** (status field still exists)
2. **Adds temporal audit trail** (StatusEvent nodes)
3. **Enables parallel work** (multiple in_progress)
4. **Auto-infers status** from activity presence
5. **Matches the temporal_design.md philosophy** (events, not mutations)
