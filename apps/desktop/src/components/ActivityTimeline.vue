<script setup lang="ts">
import { computed, ref } from 'vue'
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

interface SessionGroup {
  sessionId: string
  sourceAgent: string
  projectDir: string
  startTime: string
  endTime: string | null
  events: AgentEvent[]
  isActive: boolean
}

const props = defineProps<{
  events: AgentEvent[]
}>()

// Track collapsed sessions (collapsed by default for ended sessions)
const collapsedSessions = ref<Set<string>>(new Set())

// Session is considered stale after 15 minutes of inactivity
const STALE_SESSION_MINUTES = 15

// Group events by sessionId
const sessionGroups = computed<SessionGroup[]>(() => {
  const groups = new Map<string, SessionGroup>()
  const now = new Date()

  // Sort events by time (oldest first for grouping)
  const sortedEvents = [...props.events].sort((a, b) =>
    new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
  )

  for (const event of sortedEvents) {
    const sessionId = event.sessionId || 'unknown'

    if (!groups.has(sessionId)) {
      groups.set(sessionId, {
        sessionId,
        sourceAgent: event.sourceAgent,
        projectDir: event.projectDir,
        startTime: event.createdAt,
        endTime: null,
        events: [],
        isActive: true
      })
    }

    const group = groups.get(sessionId)!
    group.events.push(event)

    // Update end time and status
    if (event.eventType === 'SessionEnd') {
      group.endTime = event.createdAt
      group.isActive = false
    }

    // Track latest activity time
    const eventTime = new Date(event.createdAt)
    if (eventTime > new Date(group.startTime)) {
      if (!group.endTime || group.isActive) {
        // Update "last activity" for active sessions
        group.endTime = event.createdAt
      }
    }
  }

  // Mark stale sessions as inactive (no activity for 30+ minutes)
  // Also reverse events so latest appears at top
  for (const group of groups.values()) {
    // Reverse events order: latest activity at top
    group.events.reverse()

    if (group.isActive) {
      const lastActivity = group.endTime ? new Date(group.endTime) : new Date(group.startTime)
      const minutesSinceActivity = (now.getTime() - lastActivity.getTime()) / (1000 * 60)

      if (minutesSinceActivity > STALE_SESSION_MINUTES) {
        group.isActive = false
      }
    }
  }

  // Identify internal/subagent sessions (e.g., feature classifier calls)
  const isInternalSession = (group: SessionGroup): boolean => {
    if (group.events.length <= 2) {
      return group.events.some(e => {
        if (e.eventType === 'UserQuery' && e.payload) {
          try {
            const payload = JSON.parse(e.payload)
            const prompt = payload.prompt || ''
            return prompt.includes('feature classifier') ||
                   prompt.includes('You are a feature classifier')
          } catch { return false }
        }
        return false
      })
    }
    return false
  }

  // Separate internal sessions and combine them into one group
  const allGroups = Array.from(groups.values())
  const regularSessions = allGroups.filter(g => !isInternalSession(g))
  const internalSessions = allGroups.filter(g => isInternalSession(g))

  // If there are internal sessions, combine them into a single "Subagents" group
  if (internalSessions.length > 0) {
    const combinedInternal: SessionGroup = {
      sessionId: 'internal-subagents',
      sourceAgent: 'subagents',
      projectDir: internalSessions[0].projectDir,
      startTime: internalSessions.reduce((min, s) =>
        new Date(s.startTime) < new Date(min) ? s.startTime : min,
        internalSessions[0].startTime
      ),
      endTime: internalSessions.reduce((max, s) => {
        const sEnd = s.endTime || s.startTime
        return new Date(sEnd) > new Date(max) ? sEnd : max
      }, internalSessions[0].endTime || internalSessions[0].startTime),
      events: internalSessions.flatMap(s => s.events),
      isActive: internalSessions.some(s => s.isActive)
    }
    // Sort combined events by time (latest first)
    combinedInternal.events.sort((a, b) =>
      new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    )
    regularSessions.push(combinedInternal)
  }

  // Sort: active first, then by most recent activity
  return regularSessions.sort((a, b) => {
      // Active sessions first
      if (a.isActive !== b.isActive) {
        return a.isActive ? -1 : 1
      }
      // Then sort by most recent activity
      const aTime = a.endTime ? new Date(a.endTime) : new Date(a.startTime)
      const bTime = b.endTime ? new Date(b.endTime) : new Date(b.startTime)
      return bTime.getTime() - aTime.getTime()
    })
})

function toggleSession(sessionId: string) {
  if (collapsedSessions.value.has(sessionId)) {
    collapsedSessions.value.delete(sessionId)
  } else {
    collapsedSessions.value.add(sessionId)
  }
}

function isCollapsed(sessionId: string): boolean {
  return collapsedSessions.value.has(sessionId)
}

function formatDuration(startTime: string, endTime: string | null): string {
  const start = new Date(startTime)
  const end = endTime ? new Date(endTime) : new Date()
  const diffMs = Math.abs(end.getTime() - start.getTime())

  const totalSeconds = Math.floor(diffMs / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60

  if (hours > 0) {
    return `${hours}h ${minutes}m`
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`
  }
  return `${seconds}s`
}

function getShortSessionId(sessionId: string): string {
  // Show last 8 chars of session ID
  if (sessionId.length > 12) {
    return '...' + sessionId.slice(-8)
  }
  return sessionId
}

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
      <span class="session-count">{{ sessionGroups.length }} sessions</span>
    </div>

    <div class="timeline-content">
      <!-- Session groups -->
      <div
        v-for="session in sessionGroups"
        :key="session.sessionId"
        class="session-group"
        :class="{ 'session-active': session.isActive }"
      >
        <!-- Session header (clickable to expand/collapse) -->
        <div
          class="session-header"
          @click="toggleSession(session.sessionId)"
        >
          <!-- Row 1: Agent + Status -->
          <div class="session-row-primary">
            <div class="session-agent-info">
              <ToolIcon
                :name="isCollapsed(session.sessionId) ? 'chevron-right' : 'chevron-down'"
                :size="12"
                class="collapse-icon"
              />
              <span
                class="agent-dot"
                :style="{ background: getAgentColor(session.sourceAgent) }"
              ></span>
              <span class="session-agent">{{ session.sourceAgent }}</span>
            </div>
            <span v-if="session.isActive" class="session-status active">active</span>
            <span v-else class="session-status ended">ended</span>
          </div>
          <!-- Row 2: Stats -->
          <div class="session-row-secondary">
            <span class="session-id">{{ getShortSessionId(session.sessionId) }}</span>
            <div class="session-meta">
              <span class="session-stats">{{ session.events.length }}</span>
              <span class="session-duration">{{ formatDuration(session.startTime, session.endTime) }}</span>
            </div>
          </div>
        </div>

        <!-- Session events (collapsible) -->
        <div v-show="!isCollapsed(session.sessionId)" class="session-events">
          <div
            v-for="event in session.events"
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

              <!-- Secondary: Minimal metadata -->
              <div class="meta-row">
                <span v-if="event.featureId" class="feature-link">
                  {{ event.featureId.split(':').pop() }}
                </span>
                <span class="event-type-badge">{{ getEventTypeBadge(event) }}</span>
              </div>
            </div>
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
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.timeline-header h2 {
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.session-count {
  font-size: 0.75rem;
  color: var(--text-muted);
}

/* Session group styles */
.session-group {
  border-bottom: 1px solid var(--border-color);
}

.session-group:last-child {
  border-bottom: none;
}

.session-group.session-active {
  background: rgba(96, 165, 250, 0.03);
}

.session-header {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 12px;
  cursor: pointer;
  background: var(--bg-secondary);
  transition: background 0.15s;
}

.session-header:hover {
  background: var(--bg-tertiary);
}

/* Row 1: Agent + Status */
.session-row-primary {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
}

.session-agent-info {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.collapse-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.session-agent {
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--text-primary);
  white-space: nowrap;
}

/* Row 2: ID + Stats */
.session-row-secondary {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding-left: 18px;
}

.session-id {
  font-size: 0.65rem;
  color: var(--text-muted);
  font-family: monospace;
}

.session-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}

.session-stats {
  font-size: 0.65rem;
  color: var(--text-muted);
}

.session-stats::after {
  content: ' events';
}

.session-duration {
  font-size: 0.65rem;
  color: var(--text-secondary);
  font-family: monospace;
}

.session-status {
  font-size: 0.6rem;
  padding: 2px 6px;
  border-radius: 4px;
  text-transform: uppercase;
  font-weight: 600;
  letter-spacing: 0.02em;
  flex-shrink: 0;
}

.session-status.active {
  background: rgba(96, 165, 250, 0.2);
  color: #60a5fa;
}

.session-status.ended {
  background: var(--bg-tertiary);
  color: var(--text-muted);
}

.session-events {
  padding-left: 8px;
  border-left: 2px solid var(--border-color);
  margin-left: 16px;
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

.feature-link {
  font-size: 0.65rem;
  color: var(--accent-blue);
  background: rgba(96, 165, 250, 0.1);
  padding: 1px 5px;
  border-radius: 3px;
}

.event-type-badge {
  font-size: 0.6rem;
  color: var(--text-muted);
  text-transform: uppercase;
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
