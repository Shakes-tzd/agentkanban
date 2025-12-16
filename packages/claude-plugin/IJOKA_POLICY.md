# Ijoka Development Policy

## 1. Purpose

This document defines how work enters, flows through, and exits the Ijoka system.
The goal is to optimize flow, enable parallel development, and maintain proper attribution while reducing chaos.

**Core Principle:** We limit work-in-progress, finish fast, and track everything.

---

## 2. Work Item Types

All work must be classified as one of the following:

| Type | Purpose | Command | Notes |
|------|---------|---------|-------|
| **Feature** | New functionality or enhancement | `/add-feature` | Normal flow item |
| **Bug** | Fix incorrect behavior | `/bug` | Must include repro context |
| **Spike** | Reduce uncertainty, research | `/spike` | Time-boxed, ends in decision |
| **Subtask** | Decompose large features | `/subtask` | Child of a feature |
| **Epic** | Track large initiatives | (manual) | Container only, never flows |

**Epics do not move across the board.** They are containers for features/bugs/spikes.

---

## 3. Work Item Hierarchy

```
Epic (container)
├── Feature
│   ├── Subtask
│   └── Subtask
├── Feature
├── Bug
└── Spike
```

Use `CHILD_OF` relationships to maintain hierarchy:
- Subtasks are children of Features
- Features/Bugs/Spikes can be children of Epics

---

## 4. Board Structure

### Statuses

```
pending → in_progress → complete
              ↓
          blocked (optional)
```

### WIP Limits

| Status | Limit | Rationale |
|--------|-------|-----------|
| `in_progress` | 3 | Focus on finishing over starting |
| `blocked` | 2 | Resolve blockers quickly |

**If WIP is full, help finish existing work instead of starting new work.**

---

## 5. Parallel Development

**Ijoka supports multiple features in progress simultaneously.**

### How It Works

1. **Multiple active features** - Up to WIP limit can have `status: in_progress`
2. **Smart attribution** - PostToolUse hook scores activity to determine best feature match based on:
   - File paths being modified
   - Keywords in tool input
   - Recent activity patterns
3. **Explicit switching** - Use `/set-feature {id}` to change primary focus

### Attribution Rules

When multiple features are active:

| Scenario | Attribution |
|----------|-------------|
| Working in files related to Feature A | → Feature A |
| Working in files related to Feature B | → Feature B |
| Ambiguous or shared files | → Most recently active feature |
| No active features | → "Session Work" pseudo-feature |

### Best Practices for Parallel Work

1. **Keep features focused** - Smaller scope = better attribution
2. **Use subtasks** - Decompose large features for clarity
3. **Separate concerns** - Different features should touch different files
4. **Check status regularly** - `curl http://localhost:8000/status`

---

## 6. Entry Policies

### Backlog (pending)

A work item may enter pending only if:
- Problem/goal is clearly stated
- Correct work type is assigned
- Acceptance criteria or exit condition exists

### Ready to Start

A work item may move to `in_progress` only if:
- WIP limit allows
- Scope is actionable
- Dependencies are known or resolved

---

## 7. Pull Policies

Work is **pulled, never pushed**.

### Pull Order (Priority)

1. **Blocked work resolution** - Unblock first
2. **Bugs** - Fix defects before new features
3. **Features/Stories** - New value delivery
4. **Spikes** - Research when needed
5. **Subtasks** - Decomposed work

---

## 8. Work-Type-Specific Policies

### Bugs

- Must include context about the incorrect behavior
- Severity affects priority, not WIP rules
- Critical bugs may bypass backlog but not WIP limits

### Spikes

- Time-boxed (recommend max 1-3 sessions)
- Exit criteria:
  - Decision made
  - Findings documented (use `/insights`)
  - Follow-up work items created OR work killed

### Subtasks

- Created via `/subtask` from active feature
- Automatically linked as child
- Inherits category from parent
- Can have independent plans/steps

### Epics

- Used for tracking and reporting only
- Broken into features, bugs, or spikes
- Never placed in `in_progress`
- Progress calculated from children

---

## 9. Completion Policies

A work item may move to `complete` only if:
- Acceptance criteria met
- No known regressions introduced
- Code committed
- Tests pass (if applicable)

### Auto-Completion

Ijoka can auto-complete features based on criteria:
- `work_count` - After N tool calls
- `build` - After successful build command
- `test` - After successful test command
- `manual` - Explicit completion only (default)

---

## 10. Task/Step Tracking

### Use Ijoka Steps, Not Internal Todos

When planning work, use:
```
/plan Step 1 | Step 2 | Step 3
```

Or via API:
```bash
curl -X POST http://localhost:8000/plan \
  -H "Content-Type: application/json" \
  -d '{"steps": ["Step 1", "Step 2"]}'
```

### Why Steps Matter

- Persist across sessions
- Visible in dashboard
- Events attributed to steps
- Enable drift detection
- Track progress percentage

---

## 11. Metrics We Track

We optimize for **flow**, not utilization.

| Metric | Purpose |
|--------|---------|
| Cycle time | How long from start to complete |
| Lead time | How long from created to complete |
| WIP count | Current work in progress |
| Throughput | Features completed per period |
| Event count | Activity level per feature |
| Drift score | Alignment with current step |

Metrics are for learning and improvement, not performance evaluation.

---

## 12. Session Continuity

### At Session Start
- Previous session context is provided
- Active features are shown
- Plan progress is displayed

### During Session
- All tool calls are tracked
- Activity attributed to features
- Commits linked to features

### At Session End
- Transcript parsed for insights
- Session summary generated
- Next session has full context

---

## 13. Anti-Patterns (What We Avoid)

- Epics flowing across the board
- Unlimited "In Progress" features
- Everything marked urgent
- Spikes with no documented output
- Work with no active feature (goes to Session Work)
- Starting new work when blocked work exists
- Ignoring WIP limits

---

## 14. Continuous Improvement

- Policies are reviewed and updated as needed
- Use `/spike` for process experiments
- Record learnings with `POST /insights`
- Adapt based on metrics

---

## 15. One-Line Principle

**We track everything, limit work-in-progress, and finish fast.**
