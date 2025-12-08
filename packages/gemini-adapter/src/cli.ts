#!/usr/bin/env node

import { spawn } from 'child_process';
import { v4 as uuidv4 } from 'uuid';
import { AgentKanbanClient } from '@agentkanban/client';
import path from 'path';

// Parse args: node dist/cli.js [command] [args...]
const args = process.argv.slice(2);
const command = args[0] || 'echo'; // Default to echo for safety if no command
const commandArgs = args.slice(1);

async function main() {
    const client = new AgentKanbanClient();
    const sessionId = uuidv4();
    const projectDir = process.cwd();

    // 1. Notify start
    console.log(`[AgentKanban] Starting session ${sessionId}...`);
    await client.startSession({
        sessionId,
        sourceAgent: 'gemini-cli',
        projectDir
    });

    // 2. Spawn the actual tool
    console.log(`[AgentKanban] Running: ${command} ${commandArgs.join(' ')}`);

    const child = spawn(command, commandArgs, {
        stdio: 'inherit', // Pipe stdin/out/err directly
        shell: true       // Run in shell to support commands like 'echo'
    });

    // 3. Handle exit
    child.on('close', async (code) => {
        console.log(`[AgentKanban] Process exited with code ${code}`);

        // Notify end
        await client.endSession({
            sessionId,
            sourceAgent: 'gemini-cli',
            projectDir
        });

        process.exit(code || 0);
    });

    // Optional: Trap signals to ensure endSession is called if user Ctrl+C
    process.on('SIGINT', async () => {
        await client.endSession({
            sessionId,
            sourceAgent: 'gemini-cli',
            projectDir
        });
        process.exit(130);
    });
}

main().catch(console.error);
