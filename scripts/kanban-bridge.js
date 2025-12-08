const { AgentKanbanClient } = require('../packages/agent-kanban-client/dist/index.js');
const fs = require('fs');
const path = require('path');

// Simple persistent storage for session ID
const SESSION_FILE = path.join(__dirname, '.current_session');

async function main() {
    const args = process.argv.slice(2);
    const command = args[0];
    const client = new AgentKanbanClient('http://127.0.0.1:4000');
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
            console.log('Fetching features from AgentKanban...');
            const features = await client.getFeatures(projectDir);

            console.log('\n--- Active Features ---');
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
            break;

        case 'log':
            // log <toolName>
            if (fs.existsSync(SESSION_FILE)) {
                const sId = fs.readFileSync(SESSION_FILE, 'utf8');
                const tool = args[1] || 'unknown_tool';
                await client.sendEvent({
                    eventType: 'ToolUse',
                    sourceAgent: 'antigravity',
                    sessionId: sId,
                    projectDir,
                    toolName: tool
                });
                console.log(`Logged tool use: ${tool}`);
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
            console.log('Usage: node kanban-bridge.js [start|log <tool>|end]');
    }
}

main().catch(console.error);
