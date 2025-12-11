
# Ijoka Implementation Kickoff

## Context

I'm renaming my project from **AgentKanban** to **Ijoka** (Zulu for "yoke" - yoking AI agents together for coordinated work). This is a comprehensive architectural upgrade, not just a rename.

## Implementation Plan

The full implementation plan is at:
```
/Users/shakes/DevProjects/ijoka/ijoka-implementation-plan.yaml
```

Read this file first to understand the full scope. Key architectural decisions:

1. **Eliminate feature_list.json entirely** - Graph DB becomes single source of truth
2. **Memgraph as source of truth** - SQLite becomes read-only cache for Tauri UI
3. **MCP server for agent communication** - Universal interface for Claude Code, Gemini, Codex
4. **Event sourcing** - Append-only events, computed state
5. **Contextune absorption** - All functionality merged into Ijoka plugin with unified branding

## Phase 0: Rename (Start Here)

### Step 1: Rename GitHub Repository

Use GitHub CLI to rename the repository:

```bash
# Rename the repo from agentkanban to ijoka
gh repo rename ijoka

# Update the remote URL locally
git remote set-url origin git@github.com:$(gh repo view --json owner -q .owner.login)/ijoka.git

# Verify
git remote -v
```

### Step 2: Execute Rename Tasks

Follow Phase 0 in the implementation plan YAML. Key renames:

| Current | New |
|---------|-----|
| `agentkanban` | `ijoka` |
| `AgentKanban` | `Ijoka` |
| `~/.agentkanban/` | `~/.ijoka/` |
| `agentkanban.db` | `ijoka.db` |
| `@agentkanban/*` | `@ijoka/*` |

Files to update:
- `package.json` (all)
- `Cargo.toml`
- `tauri.conf.json`
- All Vue components with branding
- All Python scripts with imports
- README.md
- Any hardcoded paths

### Step 3: Verify Rename

```bash
# Should return NO matches
grep -r "AgentKanban" --include='*.rs' --include='*.vue' --include='*.ts' --include='*.py' --include='*.json' --include='*.toml' .
grep -r "agentkanban" --include='*.rs' --include='*.vue' --include='*.ts' --include='*.py' --include='*.json' --include='*.toml' .

# Build should succeed
pnpm build

# App should launch with Ijoka branding
pnpm dev
```

## Constraints

- **Do NOT touch feature_list.json logic yet** - That's Phase 5
- **Focus only on renaming in Phase 0** - No architectural changes
- **Commit incrementally** - One commit per major file group
- **Run tests after each change** - Ensure nothing breaks

## After Phase 0

Move to Phase 1 (Graph Database Setup) following the YAML plan. The critical path is:

```
Phase 0 (Rename) → Phase 1 (Graph DB) → Phase 5 (Eliminate JSON) → Phase 2 (MCP Server)
```

## Start

Begin by:
1. Reading the implementation plan YAML
2. Renaming the GitHub repo with `gh repo rename ijoka`
3. Executing the Phase 0 rename tasks systematically

Let's go!
