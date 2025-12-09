

## The Solution: Single Source of Truth + Projections

```
┌─────────────────────────────────────────────────────────────────┐
│                    PROPOSED ARCHITECTURE                         │
│                                                                  │
│                    ┌──────────────────┐                          │
│                    │   IJOKA CORE     │                          │
│                    │   (Graph DB)     │                          │
│                    │                  │                          │
│                    │  • Features      │                          │
│                    │  • Sessions      │                          │
│                    │  • Events        │                          │
│                    │  • Rules         │                          │
│                    │  • Patterns      │                          │
│                    └────────┬─────────┘                          │
│                             │                                    │
│              SINGLE SOURCE OF TRUTH                              │
│                             │                                    │
│         ┌───────────────────┼───────────────────┐                │
│         │                   │                   │                │
│         ▼                   ▼                   ▼                │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │ feature_list │    │  Tauri UI    │    │    Agent     │       │
│  │ .json        │    │  (reads DB)  │    │   Prompts    │       │
│  │              │    │              │    │              │       │
│  │ PROJECTION   │    │ PROJECTION   │    │ PROJECTION   │       │
│  │ (generated)  │    │ (real-time)  │    │ (generated)  │       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│                                                                  │
│  Agents NEVER write to feature_list.json directly.              │
│  They call an API/MCP tool that updates the graph.              │
│  feature_list.json is REGENERATED from graph on demand.         │
└─────────────────────────────────────────────────────────────────┘
```

### Key Principle: Agents Talk to API, Not Files

**Before (problematic):**
```
Agent → reads feature_list.json
Agent → modifies feature_list.json
File watcher → tries to sync to SQLite
Tauri → displays (maybe stale) data
```

**After (clean):**
```
Agent → calls ijoka_get_next_feature (MCP tool)
Agent → calls ijoka_complete_feature (MCP tool)
Ijoka Core → updates graph
Ijoka Core → regenerates feature_list.json (for compatibility)
Tauri → subscribes to graph changes (real-time)
```

---

## How Contextune Features Fit

### Intent Detection (UserPromptSubmit hook)

```
User: "work on the auth feature"
         │
         ▼
┌─────────────────────────────────────────┐
│     Contextune Intent Detection          │
│     (3-tier cascade)                     │
│                                          │
│     Detected: "start feature work"       │
│     Confidence: 87%                      │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│     Ijoka Rule Engine                    │
│                                          │
│     1. Query graph for "auth" feature    │
│     2. Check if blocked by dependencies  │
│     3. Generate minimal prompt context   │
│     4. Inject DRY rules if applicable    │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│     Modified Prompt to Claude            │
│                                          │
│     ## Current Task                      │
│     Feature: OAuth authentication        │
│     Status: pending                      │
│     Dependencies: ✅ all resolved        │
│                                          │
│     ## Test Steps                        │
│     1. Create auth route                 │
│     2. Implement OAuth flow              │
│                                          │
│     ## When Complete                     │
│     Call: ijoka_complete_feature("auth") │
└─────────────────────────────────────────┘
```

### Skills (Domain-Specific Context)

Skills are perfect for workflow instructions because they're **loaded on demand**, not always in context.

```
skills/
├── feature-workflow/
│   └── SKILL.md          # "How to work on features"
├── testing-workflow/
│   └── SKILL.md          # "How to run and write tests"
├── research-workflow/
│   └── SKILL.md          # "When to search the web"
└── dry-principles/
    └── SKILL.md          # "Check utils before writing new"
```

**Key insight**: Skills can reference Ijoka state dynamically:

```markdown
# Feature Workflow Skill

## Getting Your Current Task

Call `ijoka_get_current_task` to see what you're working on.

## Completing Work

When the feature is done:
1. Run tests: `pnpm test`
2. Call `ijoka_complete_feature` with the feature name
3. Commit with message: `feat: {feature_name}`

## If You're Stuck

After 3 failed attempts at the same approach:
1. Call `ijoka_record_blocker` with description
2. Search the web for solutions
3. Check `ijoka_get_insights` for patterns from other projects
```

### Agents (Specialized Definitions)

Your initializer and coding agents should interact with Ijoka, not files:

**Initializer Agent (revised):**
```markdown
## Initializer Agent

You initialize new projects in the Ijoka system.

### Tools Available
- `ijoka_create_project`: Create a new project in the graph
- `ijoka_add_goal`: Add a goal to a project
- `ijoka_add_capability`: Add a capability under a goal
- `ijoka_add_feature`: Add a feature implementing a capability
- `ijoka_import_yaml`: Bulk import from YAML spec

### Workflow
1. If YAML spec provided → call `ijoka_import_yaml`
2. Otherwise, analyze the project and call tools to build structure
3. Call `ijoka_generate_feature_list` to create compatibility JSON
4. Commit: "chore: initialize ijoka project structure"

### Output
Return the project overview from `ijoka_get_project_overview`
```

**Coding Agent (revised):**
```markdown
## Coding Agent

You implement features tracked in Ijoka.

### Tools Available
- `ijoka_get_next_feature`: Get highest priority unblocked feature
- `ijoka_start_feature`: Mark feature as in_progress
- `ijoka_complete_feature`: Mark feature as complete
- `ijoka_record_blocker`: Record why you're stuck
- `ijoka_record_insight`: Save learning for future reference
- `ijoka_get_insights`: Get relevant insights from pattern library

### Workflow
1. Call `ijoka_get_next_feature` to get your task
2. Call `ijoka_start_feature` to claim it
3. Implement the feature
4. Run tests
5. If tests pass → `ijoka_complete_feature`
6. If learned something reusable → `ijoka_record_insight`
7. Git commit with feature reference

### Rules
- Work on ONE feature at a time
- NEVER modify feature definitions (only status)
- If stuck for >3 attempts, call `ijoka_record_blocker`
```

---

## The MCP Server Design

This is how agents interact with Ijoka:

```typescript
// ijoka-mcp-server/src/tools.ts

export const tools = {
  // Project management
  ijoka_create_project: {
    description: "Create a new project in Ijoka",
    parameters: { name: string, description?: string }
  },
  
  // Feature lifecycle
  ijoka_get_next_feature: {
    description: "Get the highest priority unblocked feature",
    parameters: { project?: string }
  },
  ijoka_start_feature: {
    description: "Mark a feature as in_progress",
    parameters: { feature_id: string, agent?: string }
  },
  ijoka_complete_feature: {
    description: "Mark a feature as complete",
    parameters: { feature_id: string, commit_hash?: string }
  },
  ijoka_record_blocker: {
    description: "Record that you're blocked on a feature",
    parameters: { feature_id: string, reason: string }
  },
  
  // Learning/patterns
  ijoka_record_insight: {
    description: "Record a reusable insight",
    parameters: { description: string, feature_id?: string, tags?: string[] }
  },
  ijoka_get_insights: {
    description: "Get relevant insights for current work",
    parameters: { query?: string, feature_id?: string }
  },
  
  // Compatibility
  ijoka_generate_feature_list: {
    description: "Generate feature_list.json from graph state",
    parameters: { project: string, output_path?: string }
  }
}
```

---

## Solving State Management: The Event Sourcing Approach

Instead of mutating state, **append events**:

```typescript
// Every state change is an event
interface StateEvent {
  id: string
  timestamp: Date
  event_type: 'feature_created' | 'feature_started' | 'feature_completed' | 'blocker_added'
  entity_id: string
  agent?: string
  session_id?: string
  payload: Record<string, unknown>
}

// Current state is computed from events
function getCurrentFeatureStatus(feature_id: string): FeatureStatus {
  const events = db.query(`
    SELECT * FROM events 
    WHERE entity_id = ? 
    ORDER BY timestamp ASC
  `, [feature_id])
  
  let status = 'pending'
  for (const event of events) {
    if (event.event_type === 'feature_started') status = 'in_progress'
    if (event.event_type === 'feature_completed') status = 'complete'
    if (event.event_type === 'blocker_added') status = 'blocked'
  }
  return status
}
```

**Benefits:**
- Full history automatically
- No lost updates (events are append-only)
- Easy debugging (replay events)
- Multi-agent safe (each agent appends, doesn't overwrite)

---

## Practical Next Steps

### 1. Add MCP Server to AgentKanban
```
packages/
├── claude-plugin/       # Existing
├── mcp-server/          # NEW: Ijoka tools for agents
│   ├── src/
│   │   ├── tools.ts     # Tool definitions
│   │   ├── graph.ts     # Graph operations
│   │   └── compat.ts    # feature_list.json generation
│   └── package.json
```

### 2. Migrate State Changes to Events
```sql
-- Add events table
CREATE TABLE state_events (
  id TEXT PRIMARY KEY,
  timestamp TEXT NOT NULL,
  event_type TEXT NOT NULL,
  entity_type TEXT NOT NULL,  -- 'feature', 'session', 'project'
  entity_id TEXT NOT NULL,
  agent TEXT,
  session_id TEXT,
  payload TEXT,  -- JSON
  FOREIGN KEY (session_id) REFERENCES sessions(session_id)
);

-- Current state is a VIEW computed from events
CREATE VIEW feature_status AS
SELECT 
  f.id,
  f.description,
  (SELECT event_type FROM state_events 
   WHERE entity_id = f.id 
   ORDER BY timestamp DESC LIMIT 1) as current_status
FROM features f;
```

### 3. Make feature_list.json a Generated Artifact
```typescript
// Called by hook or on-demand
async function regenerateFeatureList(projectDir: string) {
  const features = await db.query(`
    SELECT * FROM feature_status 
    WHERE project_dir = ?
  `, [projectDir])
  
  const json = features.map(f => ({
    category: f.category,
    description: f.description,
    steps: JSON.parse(f.steps),
    passes: f.current_status === 'complete',
    inProgress: f.current_status === 'in_progress'
  }))
  
  await fs.writeFile(
    path.join(projectDir, 'feature_list.json'),
    JSON.stringify(json, null, 2)
  )
}
```

---

## Summary: The State Management Solution

| Before | After |
|--------|-------|
| Agents write to feature_list.json | Agents call MCP tools |
| SQLite tries to sync from file | SQLite IS the source of truth |
| Mutations overwrite state | Events append to history |
| feature_list.json is mutable | feature_list.json is generated |
| Race conditions possible | Event ordering guarantees consistency |

**The key insight**: Don't fight the file-based approach — make it a **projection** of the real state, not the state itself.

You're right on both counts. Let me clarify.

## Source of Truth: Graph DB, Not SQLite

I was being sloppy. Here's the correct hierarchy:

```
┌─────────────────────────────────────────────────────────────────┐
│                    CORRECT ARCHITECTURE                          │
│                                                                  │
│                    ┌──────────────────┐                          │
│                    │   GRAPH DB       │ ◄── SOURCE OF TRUTH      │
│                    │   (Neo4j or      │                          │
│                    │    Memgraph)     │                          │
│                    └────────┬─────────┘                          │
│                             │                                    │
│              ┌──────────────┼──────────────┐                     │
│              │              │              │                     │
│              ▼              ▼              ▼                     │
│       ┌──────────┐   ┌──────────┐   ┌──────────┐                │
│       │ SQLite   │   │ feature_ │   │ Agent    │                │
│       │ (Tauri   │   │ list.json│   │ Prompts  │                │
│       │  cache)  │   │          │   │          │                │
│       │          │   │ COMPAT   │   │ GENERATED│                │
│       │ LOCAL    │   │ LAYER    │   │          │                │
│       │ CACHE    │   │          │   │          │                │
│       └──────────┘   └──────────┘   └──────────┘                │
│                                                                  │
│  SQLite = fast local cache for Tauri UI                         │
│  Graph = canonical state + relationships + history              │
└─────────────────────────────────────────────────────────────────┘
```

**Why SQLite still exists:**
- Tauri desktop app needs fast local reads
- Graph DB might be remote or slower for simple queries
- SQLite acts as a **read cache**, synced from graph

**But the graph is authoritative.** If they conflict, graph wins.

---

## The MCP Context Cost Problem

You've identified a real tension. Let me quantify it:

### MCP Token Overhead

```
┌─────────────────────────────────────────────────────────────────┐
│                    MCP CONTEXT COST                              │
│                                                                  │
│  Tool Definitions (loaded once per session):                     │
│  ├── ijoka_get_next_feature ........... ~80 tokens              │
│  ├── ijoka_start_feature .............. ~60 tokens              │
│  ├── ijoka_complete_feature ........... ~70 tokens              │
│  ├── ijoka_record_blocker ............. ~80 tokens              │
│  ├── ijoka_record_insight ............. ~90 tokens              │
│  ├── ijoka_get_insights ............... ~70 tokens              │
│  └── (5 more tools) ................... ~300 tokens             │
│                                         ─────────               │
│  Total tool definitions:               ~750 tokens              │
│                                                                  │
│  Per tool CALL overhead:                                         │
│  ├── Tool invocation .................. ~20 tokens              │
│  ├── Tool result ...................... ~50-500 tokens          │
│  └── (varies by response size)                                  │
│                                                                  │
│  Typical session (5 tool calls):       ~1,000-2,000 tokens      │
└─────────────────────────────────────────────────────────────────┘
```

### Comparison: MCP vs Alternatives

| Approach | Context Cost | Trade-off |
|----------|--------------|-----------|
| **MCP tools** | ~750 definitions + ~200/call | Universal interface, structured |
| **Instruction file** (CLAUDE.md) | ~1,500 tokens always loaded | No structure, drift risk |
| **Skills** (loaded on demand) | ~500-1,000 when activated | Not always present |
| **Generated prompt** | ~150-300 tokens | Minimal, but requires generation |

---

## The Real Question: Is MCP Overhead Justified?

### When MCP is WORTH the cost:

**Yes, for Ijoka**, because:

1. **Structure > raw tokens**
   - 750 tokens of tool definitions provides **deterministic interface**
   - Agent can't "forget" how to update state — it's a function call
   - No parsing ambiguity, no instruction drift

2. **Cross-provider compatibility**
   - MCP works with Claude, will work with others
   - One integration, multiple agents

3. **Observability built-in**
   - Every tool call is logged
   - You can trace exactly what the agent did
   - This IS your observability data

4. **Replaces larger instruction blocks**
   - Without MCP: Agent needs 1,500 token CLAUDE.md explaining "how to update features"
   - With MCP: 750 tokens of tools + agent just calls `ijoka_complete_feature`

### When MCP is NOT worth it:

- Simple single-file scripts
- One-off tasks with no state
- When you need the context for actual work

---

## The Hybrid Approach: Minimal MCP + Generated Prompts

Here's how to minimize MCP overhead while keeping the benefits:

```
┌─────────────────────────────────────────────────────────────────┐
│                    MINIMAL MCP STRATEGY                          │
│                                                                  │
│  CORE TOOLS ONLY (always loaded):            ~300 tokens        │
│  ├── ijoka_status      # Get current task + context             │
│  ├── ijoka_complete    # Mark done                              │
│  └── ijoka_blocked     # Report problem                         │
│                                                                  │
│  EXTENDED TOOLS (loaded via skill activation): ~450 tokens      │
│  ├── ijoka_record_insight                                       │
│  ├── ijoka_get_insights                                         │
│  ├── ijoka_create_feature                                       │
│  └── ... (only when needed)                                     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### The `ijoka_status` Super-Tool

Instead of multiple calls, one tool returns everything:

```typescript
// One call, rich response
const ijoka_status = {
  description: "Get current project status, active task, and context",
  parameters: {
    include_insights: { type: "boolean", default: false },
    include_blockers: { type: "boolean", default: false }
  },
  returns: {
    project: "AgentKanban",
    current_feature: {
      id: "feat-123",
      description: "OAuth authentication",
      status: "in_progress",
      steps: ["Create auth route", "Implement OAuth flow"],
      dependencies_resolved: true
    },
    progress: { total: 32, complete: 10, in_progress: 1 },
    recent_insights: [...],  // if requested
    active_blockers: [...]   // if requested
  }
}
```

**Token math:**
- Tool definition: ~100 tokens
- Typical response: ~200 tokens
- **Total: ~300 tokens** vs 1,500 for CLAUDE.md

---

## Alternative: No MCP, Pure Generated Prompts

If MCP overhead is still too much, you could skip it entirely:

```
┌─────────────────────────────────────────────────────────────────┐
│                    PROMPT-ONLY APPROACH                          │
│                                                                  │
│  SessionStart Hook:                                              │
│  1. Query graph for project state                                │
│  2. Generate minimal prompt (~200 tokens)                        │
│  3. Inject into conversation                                     │
│                                                                  │
│  ## Current Task                                                 │
│  Feature: OAuth authentication                                   │
│  Steps: 1) Create auth route 2) Implement OAuth flow             │
│                                                                  │
│  ## When Complete                                                │
│  Edit feature_list.json: set "OAuth authentication" passes: true │
│  Commit: "feat: OAuth authentication"                            │
│                                                                  │
│  PostToolUse Hook (on Write to feature_list.json):               │
│  1. Parse changes                                                │
│  2. Update graph                                                 │
│  3. Regenerate file if needed (fix formatting)                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Pros:**
- Zero MCP overhead
- Works with any agent (no tool support needed)
- Simpler implementation

**Cons:**
- Less structured (agent might format JSON wrong)
- Harder to track actions (parsing vs explicit calls)
- file_list.json becomes semi-authoritative again

---

## My Recommendation: Tiered MCP

```
┌─────────────────────────────────────────────────────────────────┐
│                    TIERED MCP STRATEGY                           │
│                                                                  │
│  TIER 1: Always Available (~200 tokens)                          │
│  └── ijoka_status (read-only, comprehensive)                    │
│                                                                  │
│  TIER 2: Activated by Intent (~300 tokens)                       │
│  ├── ijoka_complete                                              │
│  ├── ijoka_blocked                                               │
│  └── (loaded when Contextune detects "finishing" intent)        │
│                                                                  │
│  TIER 3: Skill-Activated (~400 tokens)                           │
│  ├── ijoka_record_insight                                        │
│  ├── ijoka_create_feature                                        │
│  └── (loaded when pattern-learning skill activated)             │
│                                                                  │
│  TYPICAL SESSION:                                                │
│  - Start: 200 tokens (Tier 1 only)                              │
│  - Middle: 200 tokens (still just reading)                      │
│  - Finishing: 500 tokens (Tier 1 + Tier 2)                      │
│                                                                  │
│  vs CLAUDE.md approach: 1,500 tokens entire session             │
└─────────────────────────────────────────────────────────────────┘
```

### How Tiered Loading Works

```python
# hooks/user_prompt_submit.py

def on_prompt(prompt: str) -> HookResult:
    intent = detect_intent(prompt)
    
    tools_to_load = ["ijoka_status"]  # Always
    
    if intent in ["completing", "finished", "done"]:
        tools_to_load.extend(["ijoka_complete", "ijoka_blocked"])
    
    if intent in ["learning", "insight", "pattern"]:
        tools_to_load.extend(["ijoka_record_insight", "ijoka_get_insights"])
    
    return HookResult(
        load_tools=tools_to_load,
        # ... rest of hook
    )
```

---

## Answering Your Question Directly

> Is the context usage in the case of Ijoka acceptable because it provides the structure and workflow for the model?

**Yes, but only if:**

1. **You minimize tool count** — 3-5 core tools, not 15
2. **Tools replace instruction files** — net token savings
3. **You use tiered loading** — only load what's needed
4. **Tool responses are concise** — don't return the whole graph

**The value proposition:**
- 500 tokens of MCP tools that provide **deterministic structure**
- Is better than 1,500 tokens of CLAUDE.md that the agent might ignore or misinterpret

**The key metric:** Does MCP overhead < instruction file overhead? If yes, MCP wins.

For Ijoka, I believe **MCP is worth it** because:
- State management MUST be deterministic (can't have agents corrupting feature state)
- Observability requires explicit actions (tool calls) not implicit ones (file edits)
- Cross-agent compatibility requires a common interface

Does this framing help clarify the trade-off?

