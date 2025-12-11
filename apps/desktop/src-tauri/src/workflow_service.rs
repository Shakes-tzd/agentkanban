use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct WorkflowService;

impl WorkflowService {
    pub fn new() -> Self {
        Self
    }

    pub fn install_antigravity_integration(project_dir: &str) -> Result<()> {
        let root_path = Path::new(project_dir);
        let workflows_dir = root_path.join(".agent/workflows");
        let scripts_dir = root_path.join("scripts");

        // Ensure directories exist
        fs::create_dir_all(&workflows_dir)?;
        fs::create_dir_all(&scripts_dir)?;

        // Write kanban-bridge.js
        let bridge_path = scripts_dir.join("kanban-bridge.js");
        fs::write(&bridge_path, Self::get_bridge_script_content())?;
        tracing::info!("Created {:?}", bridge_path);

        // Write workflow
        let workflow_path = workflows_dir.join("kanban_session.md");
        fs::write(&workflow_path, Self::get_workflow_content())?;
        tracing::info!("Created {:?}", workflow_path);

        Ok(())
    }

    fn get_bridge_script_content() -> &'static str {
        r#"const { IjokaClient } = require('@ijoka/client');
const fs = require('fs');
const path = require('path');

// Simple persistent storage for session ID
const SESSION_FILE = path.join(__dirname, '.current_session');

async function main() {
    const args = process.argv.slice(2);
    const command = args[0];
    // Default to localhost:4000
    const client = new IjokaClient('http://127.0.0.1:4000');
    const projectDir = process.cwd();

    switch (command) {
        case 'start':
            const sessionId = `antigravity-${Date.now()}`;
            console.log(`Starting session ${sessionId}...`);
            await client.startSession({
                sessionId,
                sourceAgent: 'antigravity',
                projectDir
            });
            fs.writeFileSync(SESSION_FILE, sessionId);
            console.log('Session started.');
            break;

        case 'list':
            console.log('Fetching features from Ijoka...');
            try {
                const features = await client.getFeatures(projectDir);

                console.log('\n--- Active Features ---');
                // Active = not passed
                // We could also filter by "in progress" vs "todo"
                const active = features.filter(f => !f.passes);

                if (active.length === 0) {
                    console.log('No active features found.');
                } else {
                    active.forEach(f => {
                        const status = f.inProgress ? '[IN PROGRESS]' : '[TODO]';
                        console.log(`${status} ${f.description}`);
                    });
                }
                console.log('-----------------------\n');
            } catch (err) {
                console.error('Error fetching features:', err.message);
            }
            break;

        case 'end':
            if (fs.existsSync(SESSION_FILE)) {
                const sId = fs.readFileSync(SESSION_FILE, 'utf8');
                console.log(`Ending session ${sId}...`);
                await client.endSession({
                    sessionId: sId,
                    sourceAgent: 'antigravity',
                    projectDir
                });
                fs.unlinkSync(SESSION_FILE);
                console.log('Session ended.');
            } else {
                console.log('No active session found.');
            }
            break;

        default:
            console.log('Usage: node ijoka-bridge.js [start|list|end]');
    }
}

main().catch(console.error);
"#
    }

    fn get_workflow_content() -> &'static str {
        r#"---
description: Start a tracked work session in Ijoka
---

This workflow helps you synchronize your work with the Ijoka dashboard.

1. **Setup (First Time Only)**
   Ensure dependencies are installed.
   ```bash
   npm install @ijoka/client
   ```

2. **Check Active Features**
   View the current feature list to decide what to work on.
   ```bash
   // turbo
   node scripts/ijoka-bridge.js list
   ```

3. **Start Session**
   Register a new session with the Ijoka server.
   ```bash
   // turbo
   node scripts/ijoka-bridge.js start
   ```

4. **Perform Work**
   - Implement the selected feature.
   - When finished, ask the user to mark the feature as passed in Ijoka.

5. **End Session**
   When finished, close the session.
   ```bash
   // turbo
   node scripts/ijoka-bridge.js end
   ```
"#
    }
}
