<script setup lang="ts">
import ToolIcon from './icons/ToolIcon.vue'

interface AgentEvent {
  id: number
  eventType: string
  sourceAgent: string
  sessionId: string
  projectDir: string
  toolName?: string
  payload?: string
  featureId?: string
  createdAt: string
}

interface ParsedPayload {
  command?: string
  filePath?: string
  pattern?: string
  inputSummary?: string
  preview?: string
  description?: string
  prompt?: string
  messageType?: string
  reason?: string
  taskDescription?: string
  subagentType?: string
  [key: string]: unknown
}

defineProps<{
  events: AgentEvent[]
}>()

const emit = defineEmits<{
  'event-click': [event: AgentEvent]
}>()

// Tool name to icon mapping
const toolIconNames: Record<string, string> = {
  Bash: 'terminal',
  BashOutput: 'terminal-output',
  Read: 'file',
  Write: 'file-plus',
  Edit: 'file-edit',
  Grep: 'search',
  Glob: 'folder-search',
  Task: 'bot',
  TodoWrite: 'check-square',
  TodoRead: 'list',
  WebFetch: 'globe',
  WebSearch: 'search-globe',
}

// Event type to icon mapping (fallback)
const eventIconNames: Record<string, string> = {
  SessionStart: 'rocket',
  SessionEnd: 'flag',
  ToolCall: 'wrench',
  ToolUse: 'wrench',
  FeatureStarted: 'file-edit',
  FeatureCompleted: 'check-square',
  Error: 'x-circle',
  TranscriptUpdated: 'message',
  UserQuery: 'user',
  AgentStop: 'stop',
  SubagentStop: 'cpu',
}

const agentColors: Record<string, string> = {
  'claude-code': '#60a5fa',
  'codex-cli': '#4ade80',
  'gemini-cli': '#fbbf24',
  'hook': '#a78bfa',
  'file-watch': '#64748b',
  'unknown': '#888',
}

function parsePayload(payload?: string): ParsedPayload | null {
  if (!payload) return null
  try {
    return JSON.parse(payload)
  } catch {
    return null
  }
}

function getIconName(event: AgentEvent): string {
  // Prefer tool-specific icon
  if (event.toolName && toolIconNames[event.toolName]) {
    return toolIconNames[event.toolName]
  }
  // Fall back to event type icon
  return eventIconNames[event.eventType] || 'wrench'
}

function getAgentColor(agent: string): string {
  return agentColors[agent] || '#888'
}

function formatTime(dateStr: string): string {
  const date = new Date(dateStr)
  return date.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
  })
}

function getDescriptiveTitle(event: AgentEvent): string {
  const payload = parsePayload(event.payload)

  // UserQuery - show prompt preview
  if (event.eventType === 'UserQuery') {
    const prompt = payload?.prompt || payload?.preview || ''
    if (prompt) {
      return truncate(prompt, 50)
    }
    return 'User Query'
  }

  // Session events
  if (event.eventType === 'SessionStart') return 'Session Started'
  if (event.eventType === 'SessionEnd') return 'Session Ended'
  if (event.eventType === 'AgentStop') {
    const reason = payload?.reason || 'completed'
    return `Agent Stopped (${reason})`
  }
  if (event.eventType === 'SubagentStop') {
    const task = payload?.taskDescription || payload?.subagentType || 'task'
    return `Subagent: ${truncate(task, 40)}`
  }

  // TranscriptUpdated - use messageType or tool info
  if (event.eventType === 'TranscriptUpdated') {
    const msgType = payload?.messageType || event.toolName
    if (msgType === 'tool_result' || event.toolName === 'ToolResult') {
      return 'Tool Result'
    }
    if (msgType === 'tool_use') {
      return 'Tool Use'
    }
    return msgType || 'Transcript Update'
  }

  // ToolCall - use tool name and context
  if (event.eventType === 'ToolCall' && event.toolName) {
    return getToolTitle(event.toolName, payload)
  }

  // Fallback
  return event.toolName || event.eventType
}

function getToolTitle(toolName: string, payload: ParsedPayload | null): string {
  switch (toolName) {
    case 'Bash': {
      const cmd = payload?.command || ''
      if (cmd) {
        // Extract just the command name and first arg
        const parts = cmd.trim().split(/\s+/)
        const cmdName = parts[0]
        const preview = parts.slice(0, 3).join(' ')
        return `$ ${truncate(preview, 40)}`
      }
      return 'Run Command'
    }
    case 'BashOutput':
      return 'Check Background Output'
    case 'Read': {
      const file = payload?.filePath || ''
      if (file) {
        return `Read ${getFileName(file)}`
      }
      return 'Read File'
    }
    case 'Write': {
      const file = payload?.filePath || ''
      if (file) {
        return `Write ${getFileName(file)}`
      }
      return 'Write File'
    }
    case 'Edit': {
      const file = payload?.filePath || ''
      if (file) {
        return `Edit ${getFileName(file)}`
      }
      return 'Edit File'
    }
    case 'Grep': {
      const pattern = payload?.pattern || ''
      if (pattern) {
        return `Search: ${truncate(pattern, 30)}`
      }
      return 'Search Code'
    }
    case 'Glob': {
      const pattern = payload?.pattern || ''
      if (pattern) {
        return `Find: ${truncate(pattern, 30)}`
      }
      return 'Find Files'
    }
    case 'Task': {
      const desc = payload?.taskDescription || payload?.description || ''
      if (desc) {
        return `Task: ${truncate(desc, 35)}`
      }
      return 'Run Task'
    }
    case 'TodoWrite':
      return 'Update Todos'
    case 'TodoRead':
      return 'Read Todos'
    case 'WebFetch':
      return 'Fetch Web Page'
    case 'WebSearch':
      return 'Web Search'
    default:
      return toolName
  }
}

function truncate(str: string, maxLen: number): string {
  if (str.length <= maxLen) return str
  return str.slice(0, maxLen - 1) + '…'
}

function getFileName(path: string): string {
  const parts = path.split('/')
  return parts[parts.length - 1] || path
}

function getEventTypeBadge(event: AgentEvent): string {
  // Simplify event type for badge display
  switch (event.eventType) {
    case 'ToolCall':
      return 'call'
    case 'TranscriptUpdated':
      return 'result'
    case 'UserQuery':
      return 'query'
    case 'SessionStart':
      return 'start'
    case 'SessionEnd':
      return 'end'
    case 'AgentStop':
      return 'stop'
    case 'SubagentStop':
      return 'agent'
    default:
      return event.eventType.toLowerCase()
  }
}

function getProjectName(projectDir: string): string {
  if (!projectDir) return ''
  const parts = projectDir.split('/')
  return parts[parts.length - 1] || projectDir
}

function getSuccessStatus(event: AgentEvent): boolean | null {
  const payload = parsePayload(event.payload)
  if (payload?.success !== undefined) {
    return payload.success as boolean
  }
  return null
}
</script>

<template>
  <div class="activity-timeline">
    <div class="timeline-header">
      <h2>Activity</h2>
    </div>

    <div class="timeline-content">
      <div
        v-for="event in events"
        :key="event.id"
        class="timeline-item"
        :class="{
          'status-success': getSuccessStatus(event) === true,
          'status-error': getSuccessStatus(event) === false
        }"
        @click="emit('event-click', event)"
      >
        <div class="timeline-icon">
          <ToolIcon :name="getIconName(event)" :size="16" />
        </div>

        <div class="timeline-body">
          <!-- Primary: Event title -->
          <div class="title-row">
            <p class="event-title">
              {{ getDescriptiveTitle(event) }}
            </p>
            <span class="event-time">{{ formatTime(event.createdAt) }}</span>
          </div>

          <!-- Secondary: Minimal metadata - just project + colored agent dot -->
          <div class="meta-row">
            <span v-if="event.projectDir" class="project-name">
              {{ getProjectName(event.projectDir) }}
            </span>
            <span
              class="agent-dot"
              :style="{ background: getAgentColor(event.sourceAgent) }"
              :title="event.sourceAgent"
            ></span>
          </div>
        </div>
      </div>

      <div v-if="events.length === 0" class="empty-timeline">
        <p>No activity yet</p>
        <p class="hint">Events will appear here as agents work</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.activity-timeline {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.timeline-header {
  padding: 16px;
  border-bottom: 1px solid var(--border-color);
}

.timeline-header h2 {
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.timeline-content {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
}

.timeline-item {
  display: flex;
  gap: 12px;
  padding: 12px 16px;
  transition: background 0.2s;
  cursor: pointer;
  border-left: 3px solid transparent;
}

.timeline-item:hover {
  background: var(--bg-tertiary);
}

.timeline-item:active {
  background: var(--card-bg);
}

.timeline-item.status-success {
  border-left-color: var(--accent-green);
}

.timeline-item.status-error {
  border-left-color: #f87171;
}

.timeline-icon {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  background: var(--bg-tertiary);
  border-radius: 8px;
  color: var(--text-secondary);
}

.timeline-item.status-success .timeline-icon {
  color: var(--accent-green);
  background: rgba(74, 222, 128, 0.1);
}

.timeline-item.status-error .timeline-icon {
  color: #f87171;
  background: rgba(248, 113, 113, 0.1);
}

.timeline-body {
  flex: 1;
  min-width: 0;
}

/* Primary row: Title + Time */
.title-row {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 8px;
  margin-bottom: 4px;
}

.event-title {
  font-size: 0.9rem;
  color: var(--text-primary);
  line-height: 1.4;
  font-weight: 500;
  flex: 1;
  min-width: 0;
}

.event-time {
  font-size: 0.7rem;
  color: var(--text-muted);
  flex-shrink: 0;
}

/* Secondary row: Minimal metadata */
.meta-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.project-name {
  font-size: 0.7rem;
  color: var(--text-muted);
}

.agent-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.empty-timeline {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--text-muted);
}

.empty-timeline p {
  font-size: 0.85rem;
}

.empty-timeline .hint {
  font-size: 0.75rem;
  margin-top: 4px;
}
</style>
