---
name: ijoka-development-process
description: |
  CRITICAL: Ijoka is installed. This skill defines the complete software development
  process for this project. Use this skill for ALL work - every coding session, every
  task, every feature. Ijoka tracks all activity, attributes work to features, and
  provides cross-session continuity. You MUST follow this process.
---

# Ijoka Development Process

**Ijoka is the software development process for this project.** When Ijoka is installed, you MUST follow this process for ALL work. Every activity is tracked, attributed to work items, and persisted across sessions.

See `IJOKA_POLICY.md` in the plugin directory for complete policy documentation.

## Core Principles

1. **Everything is tracked** - All tool calls are captured by hooks and stored in the graph database
2. **Work must be attributed** - Every activity belongs to a work item (feature, bug, spike, subtask)
3. **API is the interface** - Use the REST API for all Ijoka operations
4. **Parallel development supported** - Multiple features can be in progress (WIP limit: 3)
5. **Finish over start** - Complete existing work before starting new work
6. **Single source of truth** - Memgraph graph database is authoritative

## Hook System

```
SessionStart ──► Records session, provides context, auto-starts API
PreToolUse ────► Smart feature matching, activity attribution
PostToolUse ───► Tracks ALL tool calls, links to features, syncs TodoWrite
UserPromptSubmit► Captures queries, classifies features
SessionEnd ────► Parses transcript, generates summary
```

## Work Item Types

| Type | Purpose | Command |
|------|---------|---------|
| **Feature** | New functionality | `/add-feature "desc"` |
| **Bug** | Defect fix | `/bug "desc"` |
| **Spike** | Research/investigation | `/spike "desc"` |
| **Subtask** | Child of feature | `/subtask "desc"` |

## MANDATORY: Before Any Work

**You MUST have at least one active work item before doing any coding work.**

```bash
# 1. Check project status
curl -s http://localhost:8000/status | jq

# 2. If no active feature, start one
curl -s -X POST http://localhost:8000/features/{id}/start

# 3. Or create a new work item
/add-feature "Description"
/bug "Bug description"
/spike "Research topic"
```

## Parallel Development

**Ijoka supports multiple features in progress simultaneously (WIP limit: 3).**

### How Attribution Works

When multiple features are active, PostToolUse hook scores activity:

| Scenario | Attribution |
|----------|-------------|
| Files related to Feature A | → Feature A |
| Files related to Feature B | → Feature B |
| Ambiguous files | → Most recently active |
| No active features | → "Session Work" (unattributed) |

### Best Practices

1. **Keep features focused** - Smaller scope = better attribution
2. **Use subtasks** - Decompose large features
3. **Separate concerns** - Different features, different files
4. **Check WIP** - Don't exceed 3 in-progress features
5. **Finish first** - Complete existing work before starting new

## REST API (Primary Interface)

**AI agents MUST use the REST API for ALL Ijoka operations.**

### Core Endpoints (http://localhost:8000)

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/status` | **START HERE** - Project status, active features |
| GET | `/features` | List all work items |
| POST | `/features` | Create work item |
| POST | `/features/{id}/start` | Start working on feature |
| POST | `/features/{id}/complete` | Mark complete |
| GET | `/plan` | Get steps for active feature |
| POST | `/plan` | Set steps |

### Work Item Hierarchy

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/features/{id}/link/{parent_id}` | Link as child |
| GET | `/features/{id}/tree` | Get hierarchy tree |
| GET | `/features/{id}/children` | Get children |

## Task/Step Tracking

**USE IJOKA FOR ALL TASK TRACKING - NOT INTERNAL TODOS**

### Set Steps
```
/plan Step 1 | Step 2 | Step 3
```

Or via API:
```bash
curl -X POST http://localhost:8000/plan \
  -H "Content-Type: application/json" \
  -d '{"steps": ["Step 1", "Step 2", "Step 3"]}'
```

### Why NOT TodoWrite?
- Session-scoped (lost when session ends)
- Not visible in dashboard
- No cross-session continuity
- Events can't be attributed to steps

**Note:** If you use TodoWrite, it's synced to Steps via hook - but prefer the API.

## Session Workflow

### At Session Start
1. Hooks provide context automatically
2. **Check status**: `curl -s http://localhost:8000/status`
3. **Verify WIP**: Ensure not exceeding limit
4. **Review plan** if exists: `curl -s http://localhost:8000/plan`

### During Work
1. Ensure correct feature is active for current work
2. Set a plan with steps for complex work
3. All tool calls are automatically tracked
4. Switch features with `/set-feature {id}` when needed
5. Commit code regularly

### Before Completing
1. Verify requirements met
2. Run tests if applicable
3. Complete: `curl -X POST http://localhost:8000/features/{id}/complete`
4. Commit final changes

## Pull Order (Priority)

When deciding what to work on:

1. **Blocked work resolution** - Unblock first
2. **Bugs** - Fix defects before new features
3. **Features** - New value delivery
4. **Spikes** - Research when needed
5. **Subtasks** - Decomposed work

## Slash Commands

| Command | Purpose |
|---------|---------|
| `/add-feature` | Create feature |
| `/bug` | Create bug |
| `/spike` | Create research item |
| `/subtask` | Create child of current feature |
| `/plan` | Set/view implementation steps |
| `/set-feature` | Switch active feature |
| `/complete-feature` | Mark complete |
| `/feature-status` | Show progress |
| `/next-feature` | Start next pending |

## Critical Rules

1. **Have active work items** - All coding must be attributed
2. **Respect WIP limits** - Max 3 features in progress
3. **Use the API** - Never bypass it
4. **Use Ijoka for tasks** - `/plan`, not internal todos
5. **Finish over start** - Complete work before starting new
6. **Commit frequently** - Commits linked to features
7. **Leave code working** - Sessions can end anytime

## Anti-Patterns (Avoid)

- Starting work without active feature
- Exceeding WIP limits
- Using TodoWrite instead of `/plan`
- Ignoring blocked work
- Everything marked urgent
- Spikes without documented output
