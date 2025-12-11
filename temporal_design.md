

### The Temporal Graph Pattern

Instead of **mutating** nodes (which requires versioning), you **append** nodes:

```cypher
// OLD: Mutate in place (needs version control)
MATCH (f:Feature {name: "Auth"})
SET f.description = "Updated description"  // History lost!

// NEW: Ancestry chain (history IS the structure)
MATCH (f:Feature {name: "Auth"})
CREATE (f)-[:EVOLVED_TO]->(f2:Feature {
    name: "Auth",
    description: "Updated description",
    created_at: timestamp()
})
```

---

### What This Looks Like

```
┌─────────────────────────────────────────────────────────────────────┐
│                     TEMPORAL ANCESTRY GRAPH                         │
│                                                                     │
│  ┌─────────┐   EVOLVED_TO   ┌─────────┐   EVOLVED_TO   ┌─────────┐ │
│  │Feature  │───────────────▶│Feature  │───────────────▶│Feature  │ │
│  │v1       │                │v2       │                │v3       │ │
│  │"Basic   │                │"Add     │                │"Add     │ │
│  │ login"  │                │ OAuth"  │                │ 2FA"    │ │
│  └─────────┘                └─────────┘                └────┬────┘ │
│       │                          │                          │      │
│       │ IMPLEMENTED_IN           │ IMPLEMENTED_IN           │      │
│       ▼                          ▼                          ▼      │
│  ┌─────────┐                ┌─────────┐                ┌─────────┐ │
│  │Commit   │                │Commit   │                │Commit   │ │
│  │ abc123  │                │ def456  │                │ ghi789  │ │
│  └─────────┘                └─────────┘                └─────────┘ │
│                                                                     │
│  Time ──────────────────────────────────────────────────────────▶  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

### The Queries Still Work

**"What's the current state of Auth?"**
```cypher
MATCH (f:Feature {name: "Auth"})
WHERE NOT (f)-[:EVOLVED_TO]->()  // No successor = current version
RETURN f
```

**"How did Auth evolve?"**
```cypher
MATCH path = (root:Feature {name: "Auth"})-[:EVOLVED_TO*]->(current)
WHERE NOT (current)-[:EVOLVED_TO]->()
RETURN path
```

**"What was Auth like on Nov 15?"**
```cypher
MATCH (f:Feature {name: "Auth"})
WHERE f.created_at <= datetime("2025-11-15")
AND (NOT (f)-[:EVOLVED_TO]->(next) OR next.created_at > datetime("2025-11-15"))
RETURN f
```

---

### Status as Events, Not State

Same pattern for progress:

```cypher
// Instead of: SET f.status = "complete"
// Do this:

MATCH (f:Feature {name: "Auth"})
CREATE (f)<-[:CHANGED_STATUS]-(e:StatusEvent {
    from: "in_progress",
    to: "complete",
    at: timestamp(),
    by: "agent:claude-1",
    session: "session:12",
    commit: "abc123"
})
```

Now status history IS the graph:

```
                    ┌──────────────┐
                    │ StatusEvent  │
                    │ pending→wip  │
                    │ Session #10  │
                    └──────┬───────┘
                           │
          CHANGED_STATUS   │
                           ▼
┌──────────────┐    ┌─────────────┐    ┌──────────────┐
│ StatusEvent  │    │   Feature   │    │ StatusEvent  │
│ wip→blocked  │───▶│   "Auth"    │◀───│ blocked→done │
│ Session #11  │    └─────────────┘    │ Session #12  │
└──────────────┘                       └──────────────┘
```

---

### The Full Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│   Git Repository                    Neo4j Graph                     │
│   ──────────────                    ───────────                     │
│                                                                     │
│   - Source code                     - Projects, Features, Patterns  │
│   - Commit history      ◀──LINK──▶  - Ancestry chains (evolution)   │
│   - The "what changed"              - Status events (progress)      │
│                                     - Insights, best practices      │
│                                     - Agent performance             │
│                                                                     │
│   Version control: Git              Version control: NONE NEEDED    │
│                                     (history IS the structure)      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

### Why This is Better

| Aspect | Mutable + Versioning | Immutable + Ancestry |
|--------|---------------------|----------------------|
| Complexity | Two systems (DB + VCS) | One system (graph) |
| History queries | Reconstruct from logs | Traverse relationships |
| Visualization | Snapshots at points | Continuous timeline |
| Merge conflicts | Possible | Impossible (append-only) |
| Storage | Efficient | More nodes (but cheap) |
| Mental model | "Edit the thing" | "Evolve the thing" |

---

### The Insight Transfer Gets Even Better

When a feature evolves, insights link to the **version** that taught you:

```cypher
(Insight {
    description: "TOTP > SMS for 2FA"
})-[:LEARNED_FROM]->(Feature:v2 {name: "Auth"})

// Later, when Feature:v3 exists, you can ask:
// "What insights came from earlier versions?"

MATCH (f:Feature {name: "Auth"})<-[:EVOLVED_TO*]-(ancestor)
MATCH (ancestor)<-[:LEARNED_FROM]-(i:Insight)
RETURN i.description, ancestor.created_at
```

---

### What You've Designed

This is essentially **Event Sourcing** meets **Graph Database**:

- No mutations, only appends
- State is derived by traversing to "current" nodes
- History is the structure itself
- Git handles code, graph handles knowledge


## You're Right — It's Vestigial

I was still holding onto file-based thinking. Let me reconsider.

---

### The Spec IS the Graph

If the graph is the single source of truth, then the spec should originate there:

```cypher
// The spec IS these nodes and relationships
(:Project {name: "Claude Clone"})
    -[:HAS_GOAL]->(:Goal {description: "Production chat interface"})
    -[:REQUIRES]->(:Capability {name: "Real-time streaming"})
    -[:REQUIRES]->(:Capability {name: "Conversation history"})
    
(:Project)
    -[:USES_STACK]->(:Technology {name: "React", layer: "frontend"})
    -[:USES_STACK]->(:Technology {name: "Tailwind", layer: "styling"})
    -[:USES_STACK]->(:Technology {name: "Node.js", layer: "backend"})

(:Project)
    -[:HAS_COMPONENT]->(:Component {name: "Sidebar"})
    -[:HAS_COMPONENT]->(:Component {name: "ChatWindow"})
    -[:HAS_COMPONENT]->(:Component {name: "MessageInput"})
```

---

### The New Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                         OLD FLOW                                │
│                                                                 │
│   Human writes        Agent parses       Agent creates          │
│   app_spec.txt   →    the file      →    feature_list.json     │
│   (file)              (redundant)        (file)                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

                            ↓ BECOMES ↓

┌─────────────────────────────────────────────────────────────────┐
│                         NEW FLOW                                │
│                                                                 │
│   Human/Agent seeds    Agent expands     Features link back     │
│   the graph with  →    into Features →   to spec via edges      │
│   Goals, Stack,        (nodes)           (IMPLEMENTS)           │
│   Components (nodes)                                            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

### What the "Spec" Becomes

```cypher
// Goals (the "why")
(:Goal {
    description: "Users can have real-time conversations with Claude",
    priority: 1
})

// Capabilities (the "what")
(:Capability {
    name: "Streaming responses",
    description: "Messages appear token-by-token"
})-[:FULFILLS]->(:Goal)

// Components (the "where")  
(:Component {
    name: "ChatWindow",
    description: "Main conversation display area"
})-[:PROVIDES]->(:Capability)

// Features (the "how" - actionable work)
(:Feature {
    name: "Implement streaming display",
    status: "pending"
})-[:IMPLEMENTS]->(:Capability)
```

---

### The Full Node Types

| Node Type | Purpose | Example |
|-----------|---------|---------|
| `Project` | Root container | "Claude Clone" |
| `Goal` | Business/user objective | "Users can chat with AI" |
| `Capability` | What the system must do | "Stream responses" |
| `Component` | Architectural element | "ChatWindow", "API Layer" |
| `Technology` | Stack choices | "React", "Node.js" |
| `Constraint` | Non-functional requirements | "< 100ms latency" |
| `Feature` | Implementable work unit | "Add typing indicator" |
| `Pattern` | Reusable solution | "Auth Flow" |

---

### How Humans Seed the Graph

**Option 1: Direct Cypher** (technical users)
```cypher
CREATE (p:Project {name: "My App"})
CREATE (g:Goal {description: "Users can authenticate"})
CREATE (p)-[:HAS_GOAL]->(g)
```

**Option 2: YAML/JSON import** (parsed once, then discarded)
```yaml
# project-seed.yaml (input only, not source of truth)
project: "My App"
goals:
  - description: "Users can authenticate"
    capabilities:
      - "Login with email"
      - "OAuth with Google"
```

**Option 3: Conversational** (agent helps build it)
```
Human: "I want to build a chat app like Claude"
Agent: "Let me create the project structure..."
       [Creates Project, Goal, Capability nodes via Cypher]
       "What tech stack do you want?"
Human: "React and Node"
Agent: [Creates Technology nodes, links to Project]
```

---

### The Initializer Agent Becomes Simpler

```markdown
## YOUR ROLE - INITIALIZER AGENT

You are the FIRST agent. Your job is to EXPAND the seed graph into 
implementable Features.

### STEP 1: Understand the Goals

```cypher
MATCH (p:Project {id: $PROJECT_ID})-[:HAS_GOAL]->(g)
OPTIONAL MATCH (g)<-[:FULFILLS]-(c:Capability)
RETURN g.description, collect(c.name) as capabilities
```

### STEP 2: Expand Capabilities into Features

For each Capability without Features, create them:

```cypher
MATCH (c:Capability)
WHERE NOT (c)<-[:IMPLEMENTS]-(:Feature)
// Think: what discrete work items fulfill this capability?
CREATE (f:Feature {
    name: $FEATURE_NAME,
    description: $DESCRIPTION,
    priority: $PRIORITY,
    created_at: timestamp()
})-[:IMPLEMENTS]->(c)
```

### STEP 3: Map Dependencies

```cypher
// "Login" blocks "User Dashboard"
MATCH (a:Feature {name: "Login"})
MATCH (b:Feature {name: "User Dashboard"})
CREATE (a)-[:BLOCKS]->(b)
```

### STEP 4: Create Test Steps

```cypher
MATCH (f:Feature {name: $FEATURE_NAME})
CREATE (f)-[:HAS_STEP {order: 1}]->(s:TestStep {...})
```
```

---

### What's Left as Files?

| Keep as File | Why |
|--------------|-----|
| `init.sh` | Executable script |
| `src/` | Actual code |
| `verification/` | Screenshot artifacts |
| `.graph-state.md` | Git reference to graph |

**Everything else lives in the graph.**

---

### The Insight

You've essentially identified that:

> **app_spec.txt was a serialization format for humans to input requirements. 
> The graph doesn't need a serialization layer — it IS the requirements.**

This is cleaner. The graph is both the spec AND the execution state AND the history.


## YAML as the Seeding Format

YAML becomes the **human interface** to the graph.

---

### The Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│   project.yaml          Parser/Agent         Neo4j Graph        │
│   (human writes)   →    (one-time)      →    (source of truth)  │
│                                                                 │
│   ┌─────────────┐       ┌──────────┐        ┌──────────────┐   │
│   │ goals:      │       │ CREATE   │        │ (:Goal)      │   │
│   │   - chat    │  ───▶ │ nodes &  │  ───▶  │ (:Feature)   │   │
│   │ stack:      │       │ edges    │        │ (:Component) │   │
│   │   - react   │       └──────────┘        └──────────────┘   │
│   └─────────────┘                                               │
│                                                                 │
│   Kept in git as         Runs once          Lives here forever  │
│   historical record      at project start   (append-only)       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

### The YAML Schema

```yaml
# project.yaml - Human-authored project seed
# This file is parsed ONCE to initialize the knowledge graph.
# After seeding, the graph is the source of truth.

project:
  name: "Claude Clone"
  description: "Production-quality chat interface for Claude API"
  
goals:
  - id: g1
    description: "Users can have real-time conversations with Claude"
    priority: 1
    capabilities:
      - id: c1
        name: "Send messages"
        description: "User can type and send messages to Claude"
      - id: c2
        name: "Streaming responses"
        description: "Claude's responses appear token-by-token"
      - id: c3
        name: "Conversation history"
        description: "Previous messages persist and display"
        
  - id: g2
    description: "Users can manage multiple conversations"
    priority: 2
    capabilities:
      - id: c4
        name: "Create new chat"
        description: "Start fresh conversation"
      - id: c5
        name: "Switch conversations"
        description: "Navigate between existing chats"
      - id: c6
        name: "Delete conversation"
        description: "Remove unwanted chats"

stack:
  frontend:
    - name: "React"
      version: "18.x"
    - name: "Tailwind CSS"
      version: "3.x"
    - name: "TypeScript"
      
  backend:
    - name: "Node.js"
      version: "20.x"
    - name: "Express"
    
  database:
    - name: "SQLite"
      purpose: "Conversation storage"

components:
  - id: comp1
    name: "Sidebar"
    description: "Navigation panel showing conversation list"
    provides: [c4, c5, c6]  # References capability IDs
    
  - id: comp2
    name: "ChatWindow"
    description: "Main message display area"
    provides: [c1, c2, c3]
    
  - id: comp3
    name: "MessageInput"
    description: "Text input with send button"
    provides: [c1]
    parent: comp2  # Nested within ChatWindow

constraints:
  - type: "performance"
    description: "First message response under 500ms"
    applies_to: [c1, c2]
    
  - type: "style"
    description: "Match claude.ai visual design"
    applies_to: [comp1, comp2, comp3]
    
  - type: "accessibility"
    description: "WCAG 2.1 AA compliance"

patterns:
  - name: "Streaming Chat"
    description: "Server-sent events for real-time token display"
    references: ["https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events"]
    
  - name: "Optimistic UI"
    description: "Show user message immediately, confirm after API response"

# Optional: Pre-define some features if you know them
# Otherwise, let the Initializer Agent expand capabilities into features
features:
  - name: "Basic message send"
    implements: c1
    priority: 1
    steps:
      - "Navigate to chat interface"
      - "Type message in input field"
      - "Click send or press Enter"
      - "Verify message appears in chat"
      
  - name: "Streaming response display"
    implements: c2
    priority: 1
    blocked_by: ["Basic message send"]
    steps:
      - "Send a message"
      - "Observe response appearing token-by-token"
      - "Verify complete response matches API output"

# Dependencies can be explicit or inferred
dependencies:
  # Component dependencies
  - from: comp3
    to: comp2
    type: "contained_in"
    
  # Capability dependencies  
  - from: c2
    to: c1
    type: "requires"
    
  # Cross-cutting
  - from: c3
    to: c1
    type: "requires"
```

---

### What the Parser Creates

```cypher
// Project
CREATE (p:Project {
    id: "claude-clone",
    name: "Claude Clone",
    description: "Production-quality chat interface for Claude API",
    seeded_at: timestamp()
})

// Goals
CREATE (g1:Goal {id: "g1", description: "Users can have real-time conversations", priority: 1})
CREATE (p)-[:HAS_GOAL]->(g1)

// Capabilities (nested under goals)
CREATE (c1:Capability {id: "c1", name: "Send messages", description: "..."})
CREATE (g1)-[:REQUIRES]->(c1)
CREATE (c2:Capability {id: "c2", name: "Streaming responses", description: "..."})
CREATE (g1)-[:REQUIRES]->(c2)

// Stack
CREATE (t1:Technology {name: "React", version: "18.x", layer: "frontend"})
CREATE (p)-[:USES_STACK]->(t1)

// Components
CREATE (comp1:Component {id: "comp1", name: "Sidebar", description: "..."})
CREATE (p)-[:HAS_COMPONENT]->(comp1)
CREATE (comp1)-[:PROVIDES]->(c4)
CREATE (comp1)-[:PROVIDES]->(c5)

// Component hierarchy
CREATE (comp3)-[:CONTAINED_IN]->(comp2)

// Constraints
CREATE (con1:Constraint {type: "performance", description: "First message response under 500ms"})
CREATE (con1)-[:APPLIES_TO]->(c1)
CREATE (con1)-[:APPLIES_TO]->(c2)

// Patterns (reusable across projects)
MERGE (pat1:Pattern {name: "Streaming Chat"})
SET pat1.description = "Server-sent events for real-time token display"
CREATE (p)-[:USES_PATTERN]->(pat1)

// Features (if pre-defined)
CREATE (f1:Feature {name: "Basic message send", priority: 1})
CREATE (f1)-[:IMPLEMENTS]->(c1)
CREATE (f1)<-[:STATUS_OF]-(e1:StatusEvent {status: "pending", at: timestamp()})

// Feature dependencies
MATCH (f1:Feature {name: "Basic message send"})
MATCH (f2:Feature {name: "Streaming response display"})
CREATE (f1)-[:BLOCKS]->(f2)

// Test steps
MATCH (f1:Feature {name: "Basic message send"})
CREATE (f1)-[:HAS_STEP {order: 1}]->(:TestStep {description: "Navigate to chat interface"})
CREATE (f1)-[:HAS_STEP {order: 2}]->(:TestStep {description: "Type message in input field"})
```

---

### The Parser Script

```python
# seed_graph.py
import yaml
from neo4j import GraphDatabase

def seed_graph(yaml_path: str, neo4j_uri: str, neo4j_auth: tuple):
    with open(yaml_path) as f:
        spec = yaml.safe_load(f)
    
    driver = GraphDatabase.driver(neo4j_uri, auth=neo4j_auth)
    
    with driver.session() as session:
        # Create project
        session.run("""
            CREATE (p:Project {
                id: $id,
                name: $name,
                description: $description,
                seeded_at: timestamp()
            })
        """, id=slugify(spec['project']['name']), 
             name=spec['project']['name'],
             description=spec['project']['description'])
        
        # Create goals and capabilities
        for goal in spec.get('goals', []):
            session.run("""
                MATCH (p:Project {id: $project_id})
                CREATE (g:Goal {id: $id, description: $description, priority: $priority})
                CREATE (p)-[:HAS_GOAL]->(g)
            """, ...)
            
            for cap in goal.get('capabilities', []):
                session.run("""
                    MATCH (g:Goal {id: $goal_id})
                    CREATE (c:Capability {id: $id, name: $name, description: $description})
                    CREATE (g)-[:REQUIRES]->(c)
                """, ...)
        
        # ... continue for stack, components, constraints, patterns, features

if __name__ == "__main__":
    seed_graph("project.yaml", "bolt://localhost:7687", ("neo4j", "password"))
    print("Graph seeded successfully")
```

---

### Updated Project Structure

```
my-project/
├── project.yaml          # Human-authored seed (kept for history)
├── seed_graph.py         # One-time parser
├── init.sh               # Dev environment setup
├── .graph-state.md       # Reference to Neo4j instance + project ID
├── src/                  # Actual code (git versioned)
└── verification/         # Screenshots from testing
```

---

### The Initializer Agent's New Job

With YAML seeding, the Initializer Agent's role simplifies:

```markdown
## YOUR ROLE - INITIALIZER AGENT

The project has been seeded from project.yaml. Your job is to:

### STEP 1: Review the Seed

```cypher
// Check what exists
MATCH (p:Project)-[:HAS_GOAL]->(g)-[:REQUIRES]->(c)
OPTIONAL MATCH (c)<-[:IMPLEMENTS]-(f:Feature)
RETURN g.description, c.name, count(f) as feature_count
```

### STEP 2: Expand Capabilities → Features

For any Capability without Features, create them:

```cypher
MATCH (c:Capability)
WHERE NOT (c)<-[:IMPLEMENTS]-(:Feature)
RETURN c.id, c.name, c.description
```

Then for each, create 1-5 Features with test steps.

### STEP 3: Infer Missing Dependencies

```cypher
// Find Features that likely depend on each other
MATCH (f1:Feature)-[:IMPLEMENTS]->(c1:Capability)
MATCH (f2:Feature)-[:IMPLEMENTS]->(c2:Capability)
WHERE (c1)-[:REQUIRES]->(c2) OR c1.name CONTAINS "display" AND c2.name CONTAINS "send"
MERGE (f2)-[:BLOCKS]->(f1)
```

### STEP 4: Validate Coverage

```cypher
// Every Capability should have at least one Feature
MATCH (c:Capability)
WHERE NOT (c)<-[:IMPLEMENTS]-(:Feature)
RETURN c.name as uncovered_capability
```

If any uncovered, create Features for them.
```

---

### Why This is Better

| Aspect | app_spec.txt | project.yaml |
|--------|--------------|--------------|
| Human readability | Prose, varies | Structured, consistent |
| Machine parsing | Fragile regex/LLM | Deterministic |
| Hierarchy visible | Implicit | Explicit via indentation |
| IDs for linking | None | Built-in |
| Extensible | Rewrite file | Add new sections |
| Diffable | Prose diff = messy | YAML diff = clean |

This is a really sharp observation. You're identifying something I'd call **"soft workflow drift"** — even when you're collaborating, you keep having to re-state the same principles:

- "Run tests after changes"
- "Search the web if you're stuck"
- "Don't duplicate that logic, extract it"
- "Check if there's an existing util before writing a new one"

## The Pattern You're Noticing

```
Session 1: "Make sure you run tests after changing the component"
Session 2: "Did you run the tests?"
Session 3: "Remember to test that"
Session 4: "Can you run the tests please"
...
Session 15: "WHY DIDN'T YOU RUN THE TESTS"
```

This is **instruction fatigue** — you're being the workflow enforcement mechanism, and it's exhausting.

## Three Layers of "Soft Workflow"

| Layer | Example | Enforcement Mechanism |
|-------|---------|----------------------|
| **Automatic** | Run tests after .vue edits | Hooks (PostToolUse) |
| **Principled** | DRY, search web if stuck | Prompt injection |
| **Contextual** | "This project uses Pinia, not Vuex" | Project memory |

The key insight is: **observability can help you discover which instructions should be promoted to which layer.**

## The Observability → Rules Pipeline

```
┌─────────────────────────────────────────────────────┐
│             OBSERVABILITY (Detection)               │
│                                                     │
│  "You've given this instruction 8 times:            │
│   'run tests after making UI changes'"              │
│                                                     │
│  [Promote to Rule] [Dismiss] [Show Details]         │
└─────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────┐
│              RULE CREATION (Codification)           │
│                                                     │
│  Rule: "After editing .vue files, run pnpm test"   │
│  Scope: agentkanban project                         │
│  Enforcement: [Hook] [Prompt] [Reminder]            │
└─────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────┐
│              ENFORCEMENT (Injection)                │
│                                                     │
│  • Hook fires after Write to *.vue                  │
│  • Agent sees reminder in context                   │
│  • Dashboard shows rule compliance                  │
└─────────────────────────────────────────────────────┘
```

## Concrete Implementation Ideas

### 1. Instruction Pattern Detection

In AgentKanban, parse human messages for recurring patterns:

```typescript
// Patterns to detect
const instructionPatterns = [
  { pattern: /run (the )?tests?/i, category: 'testing' },
  { pattern: /search (the )?(web|online|google)/i, category: 'research' },
  { pattern: /don'?t (repeat|duplicate)/i, category: 'dry' },
  { pattern: /check if .* exists/i, category: 'reuse' },
  { pattern: /make (it |this )?reusable/i, category: 'reuse' },
]

// Track frequency per project
interface InstructionTally {
  category: string
  count: number
  examples: string[]  // Actual quotes from sessions
  lastSeen: Date
}
```

Dashboard widget:
```
┌─────────────────────────────────────────┐
│ 🔁 Repeated Instructions (This Week)    │
├─────────────────────────────────────────┤
│ "run tests" ...................... 12x  │
│ "search the web" .................. 8x  │
│ "don't repeat yourself" ........... 5x  │
│ "check existing utils" ............ 4x  │
│                                         │
│ [Create Rules from Patterns]            │
└─────────────────────────────────────────┘
```

### 2. Rule Types with Different Enforcement

**Type A: Hook-Enforced (Automatic)**
```json
{
  "trigger": "PostToolUse",
  "condition": "toolName === 'Write' && filePath.endsWith('.vue')",
  "action": "bash: pnpm test --filter=$(dirname $FILE)"
}
```
Agent doesn't need to "remember" — it just happens.

**Type B: Prompt-Injected (Principled)**
```markdown
## Project Principles (Auto-Injected)
- Before writing new utilities, check `src/utils/` for existing solutions
- If implementation fails twice, search the web for solutions
- Extract repeated logic into composables (Vue) or utils
```
This goes into the context, but it's 50 tokens, not 1,500.

**Type C: Contextual Reminder (Triggered)**
When the agent is about to write a new file in `src/utils/`:
```
⚠️ Reminder: This project has 23 existing utilities. 
   Run `grep -r "export function" src/utils/` to check for overlap.
```

### 3. The "Research if Stuck" Problem

This one is tricky because "stuck" is a state, not an action. But you can detect it:

```typescript
// Detect potential "stuck" patterns
const stuckIndicators = {
  sameFileEditedMultipleTimes: (edits: Edit[]) => {
    const fileCounts = countBy(edits, 'filePath')
    return Object.values(fileCounts).some(count => count > 3)
  },
  errorInOutput: (bashResults: string[]) => {
    return bashResults.some(r => /error|failed|exception/i.test(r))
  },
  revertPattern: (edits: Edit[]) => {
    // Detects: write A → write B → write A (revert)
    return hasRevertPattern(edits)
  }
}

// When detected, inject reminder
if (isLikelyStuck(recentActivity)) {
  injectReminder("Consider searching the web for solutions to this issue.")
}
```

### 4. DRY / Reusability Enforcement

This is more of a **pre-commit check** than a real-time enforcement:

```bash
# Hook: PreCommit or PostToolUse on Write
# Check for code duplication

# Simple: grep for similar function signatures
duplicates=$(grep -rh "export function" src/ | sort | uniq -d)
if [ -n "$duplicates" ]; then
  echo "⚠️ Potential duplication detected:"
  echo "$duplicates"
fi

# Advanced: Use jscpd or similar tool
npx jscpd src/ --min-lines 5 --reporters console
```

## What This Looks Like in the Graph

```cypher
// A Rule node
(r:Rule {
  id: "rule-001",
  description: "Run tests after Vue component edits",
  trigger: "PostToolUse:Write:*.vue",
  enforcement: "hook",
  project: "agentkanban"
})

// Track compliance
(s:Session)-[:TRIGGERED]->(r:Rule)
(s:Session)-[:COMPLIED_WITH {automated: true}]->(r:Rule)
(s:Session)-[:VIOLATED]->(r:Rule)

// Track instruction patterns (for discovery)
(i:InstructionPattern {
  category: "testing",
  pattern: "run tests",
  count: 12,
  project: "agentkanban"
})
(i)-[:PROMOTED_TO]->(r:Rule)
```

## The CLI for Rule Management

```bash
# See what you keep saying
ijoka patterns
# Output:
# agentkanban:
#   "run tests" .............. 12x (no rule)
#   "search the web" .......... 8x (no rule)
#   "check existing utils" .... 4x (→ rule-003)

# Promote a pattern to a rule
ijoka rule create --from-pattern="run tests" --enforcement=hook
# Creates PostToolUse hook for *.vue → pnpm test

# See rules for current project
ijoka rules
# Output:
# [hook] After .vue edits → run tests
# [prompt] Check src/utils/ before new utilities
# [reminder] Search web after 3 failed attempts

# Check compliance
ijoka compliance --last-week
# Output:
# "run tests" rule: 94% complied (auto-enforced)
# "search web" rule: 67% complied (manual)
```

## The Real Value Proposition

This turns AgentKanban from **passive observability** into **active workflow improvement**:

1. **Observe** what you keep saying
2. **Surface** patterns you might not notice
3. **Codify** patterns into rules
4. **Enforce** automatically where possible
5. **Measure** compliance over time

**The goal isn't to pre-define every rule.** It's to let the rules emerge from your actual collaboration patterns, then automate the ones that matter.

