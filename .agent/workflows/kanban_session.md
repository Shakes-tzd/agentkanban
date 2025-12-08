---
description: Start a tracked work session in AgentKanban
---

This workflow helps you synchronize your work with the AgentKanban dashboard.

1. **Check Active Features**
   View the current feature list to decide what to work on.
   ```bash
   // turbo
   node scripts/kanban-bridge.js list
   ```

2. **Start Session**
   Register a new session with the AgentKanban server.
   ```bash
   // turbo
   node scripts/kanban-bridge.js start
   ```

3. **Perform Work**
   - Implement the selected feature.
   - You can optionally log major steps using: `node scripts/kanban-bridge.js log "tool_name"`

4. **Update Feature Status**
   If you completed a feature, ask the user to update `feature_list.json` to mark it as passed.

5. **End Session**
   When finished, close the session.
   ```bash
   // turbo
   node scripts/kanban-bridge.js end
   ```
