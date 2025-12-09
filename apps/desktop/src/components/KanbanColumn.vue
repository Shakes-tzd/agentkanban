<script setup lang="ts">
import { computed } from 'vue'

interface Feature {
  id: string
  projectDir: string
  description: string
  category: string
  passes: boolean
  inProgress: boolean
  agent?: string
  steps?: string[]
  updatedAt: string
  // Agent-managed state
  confidence?: number
  model?: string
  isStreaming?: boolean
  retryCount?: number
  tokenCost?: number
  hasError?: boolean
  lastAgentUpdate?: string
  // Human override state
  manualPriority?: string
  humanOverrideUntil?: string
}

const props = defineProps<{
  title: string
  columnId: string
  features: Feature[]
  color: string
  collapsed: boolean
}>()

const emit = defineEmits<{
  'feature-click': [feature: Feature]
  'expand': []
  'collapse': []
  'feature-drop': [featureId: string, targetColumn: string]
}>()

// Calculate aggregate column state based on features
const columnState = computed(() => {
  const activeFeatures = props.features.filter(f => f.inProgress)
  if (activeFeatures.some(f => f.hasError)) return 'error'
  if (activeFeatures.some(f => f.isStreaming)) return 'active'
  if (activeFeatures.length > 0) return 'working'
  return 'idle'
})

// Mini indicators for collapsed view
const statusDots = computed(() => {
  return props.features.slice(0, 5).map(f => {
    if (f.hasError) return 'error'
    if (f.isStreaming) return 'streaming'
    if (f.inProgress) return 'working'
    if (f.passes) return 'done'
    return 'idle'
  })
})

const categoryColors: Record<string, string> = {
  functional: '#60a5fa',
  ui: '#a78bfa',
  security: '#f87171',
  performance: '#fbbf24',
  documentation: '#4ade80',
  testing: '#fb923c',
  infrastructure: '#64748b',
  refactoring: '#e879f9',
}

function getCategoryColor(category: string): string {
  return categoryColors[category] || '#888'
}

function formatTime(dateStr: string): string {
  const date = new Date(dateStr)
  const now = new Date()
  const diff = now.getTime() - date.getTime()

  if (diff < 60000) return 'just now'
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`
  return `${Math.floor(diff / 86400000)}d ago`
}

function formatTokens(cost?: number): string {
  if (!cost) return ''
  if (cost < 1000) return `${cost}`
  if (cost < 1000000) return `${(cost / 1000).toFixed(1)}k`
  return `${(cost / 1000000).toFixed(2)}M`
}

function hasHumanOverride(feature: Feature): boolean {
  if (!feature.humanOverrideUntil) return false
  return new Date(feature.humanOverrideUntil) > new Date()
}

// Drag and drop handlers
function onDragStart(e: DragEvent, feature: Feature) {
  if (e.dataTransfer) {
    e.dataTransfer.setData('text/plain', feature.id)
    e.dataTransfer.effectAllowed = 'move'
  }
}

function onDragOver(e: DragEvent) {
  e.preventDefault()
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'move'
  }
}

function onDrop(e: DragEvent) {
  e.preventDefault()
  const featureId = e.dataTransfer?.getData('text/plain')
  if (featureId) {
    emit('feature-drop', featureId, props.columnId)
  }
}
</script>

<template>
  <div
    class="kanban-column"
    :class="[
      `state-${columnState}`,
      { collapsed: collapsed }
    ]"
    @dragover="onDragOver"
    @drop="onDrop"
  >
    <!-- Collapsed view -->
    <div v-if="collapsed" class="collapsed-view" @click="emit('expand')">
      <div class="collapsed-header">
        <span class="collapsed-count">{{ features.length }}</span>
        <span class="collapsed-title">{{ title.replace(/^[^ ]+ /, '') }}</span>
      </div>
      <div class="status-dots">
        <span
          v-for="(status, i) in statusDots"
          :key="i"
          class="status-dot"
          :class="status"
        />
        <span v-if="features.length > 5" class="more-indicator">+{{ features.length - 5 }}</span>
      </div>
    </div>

    <!-- Expanded view -->
    <template v-else>
      <div class="column-header" :style="{ borderColor: color }">
        <div class="header-left">
          <span class="column-title">{{ title }}</span>
          <span class="column-count">{{ features.length }}</span>
        </div>
        <button class="collapse-btn" @click.stop="emit('collapse')" title="Collapse column">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
      </div>

      <div class="column-content">
        <div
          v-for="feature in features"
          :key="feature.id"
          class="feature-card"
          :class="{
            'priority-high': feature.manualPriority === 'high',
            'has-error': feature.hasError,
            'is-streaming': feature.isStreaming,
            'human-override': hasHumanOverride(feature),
            'agent-managed': !hasHumanOverride(feature)
          }"
          draggable="true"
          @dragstart="(e) => onDragStart(e, feature)"
          @click="emit('feature-click', feature)"
        >
          <!-- Override indicator -->
          <div v-if="hasHumanOverride(feature)" class="override-banner">
            Manual control
          </div>

          <div class="card-header">
            <span
              class="category-badge"
              :style="{ backgroundColor: getCategoryColor(feature.category) + '20', color: getCategoryColor(feature.category) }"
            >
              {{ feature.category }}
            </span>
            <span v-if="feature.model" class="model-badge">
              {{ feature.model }}
            </span>
            <span v-if="feature.tokenCost" class="token-cost">
              {{ formatTokens(feature.tokenCost) }} tok
            </span>
          </div>

          <div class="card-body">
            <span v-if="feature.isStreaming" class="streaming-dot" />
            <p class="card-description">{{ feature.description }}</p>
          </div>

          <div v-if="feature.steps?.length" class="card-steps">
            <span class="steps-count">{{ feature.steps.length }} steps</span>
          </div>

          <div class="card-footer">
            <div class="footer-left">
              <span v-if="feature.agent" class="agent-badge">
                {{ feature.agent }}
              </span>
              <span v-if="feature.confidence" class="confidence">
                {{ feature.confidence }}%
              </span>
            </div>
            <div class="footer-right">
              <span v-if="feature.retryCount && feature.retryCount > 2" class="loop-warning" title="Possible loop detected">
                {{ feature.retryCount }}
              </span>
              <span class="card-time">{{ formatTime(feature.updatedAt) }}</span>
            </div>
          </div>
        </div>

        <div v-if="features.length === 0" class="empty-column">
          <p>No features</p>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.kanban-column {
  display: flex;
  flex-direction: column;
  background: var(--bg-tertiary);
  border-radius: 8px;
  min-height: 0;
  max-height: calc(100vh - 140px);
  transition: all 0.3s ease;
  border-top: 3px solid var(--border-color);
}

/* Column state indicators */
.kanban-column.state-idle { border-top-color: var(--accent-blue); }
.kanban-column.state-working { border-top-color: var(--accent-yellow); }
.kanban-column.state-active { border-top-color: var(--accent-green); }
.kanban-column.state-error { border-top-color: var(--accent-red); }

/* Collapsed state */
.kanban-column.collapsed {
  cursor: pointer;
}

.collapsed-view {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 12px 8px;
  gap: 12px;
  height: 100%;
}

.collapsed-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}

.collapsed-count {
  font-size: 1.25rem;
  font-weight: 700;
  color: var(--text-primary);
}

.collapsed-title {
  font-size: 0.65rem;
  color: var(--text-muted);
  text-transform: uppercase;
  writing-mode: vertical-rl;
  text-orientation: mixed;
  letter-spacing: 0.05em;
}

.status-dots {
  display: flex;
  flex-direction: column;
  gap: 4px;
  align-items: center;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-muted);
}

.status-dot.idle { background: var(--text-muted); }
.status-dot.working { background: var(--accent-yellow); }
.status-dot.streaming { background: var(--accent-green); animation: pulse 1.5s ease-in-out infinite; }
.status-dot.error { background: var(--accent-red); }
.status-dot.done { background: var(--accent-green); opacity: 0.6; }

.more-indicator {
  font-size: 0.6rem;
  color: var(--text-muted);
}

/* Expanded view */
.column-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 2px solid;
  flex-shrink: 0;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.column-title {
  font-weight: 600;
  font-size: 0.9rem;
}

.column-count {
  background: var(--bg-secondary);
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 0.8rem;
  color: var(--text-secondary);
}

.collapse-btn {
  background: transparent;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.2s;
}

.collapse-btn:hover {
  background: var(--bg-secondary);
  color: var(--text-primary);
}

.column-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

/* Feature cards */
.feature-card {
  background: var(--card-bg);
  border-radius: 6px;
  padding: 12px;
  transition: all 0.2s;
  cursor: grab;
  border-left: 3px solid var(--accent-blue);
  position: relative;
}

.feature-card:hover {
  background: var(--card-hover);
  transform: translateY(-1px);
}

.feature-card:active {
  cursor: grabbing;
}

/* Card state modifiers */
.feature-card.human-override {
  border-left-color: var(--accent-purple);
}

.feature-card.agent-managed {
  border-left-color: var(--accent-blue);
}

.feature-card.priority-high {
  border: 2px solid gold;
  box-shadow: 0 0 12px rgba(255, 215, 0, 0.2);
}

.feature-card.has-error {
  border-left-color: var(--accent-red);
  background: rgba(248, 113, 113, 0.1);
}

.feature-card.is-streaming {
  border-left-color: var(--accent-green);
}

.override-banner {
  position: absolute;
  top: 0;
  right: 0;
  background: var(--accent-purple);
  color: white;
  font-size: 0.6rem;
  padding: 2px 6px;
  border-radius: 0 4px 0 4px;
  text-transform: uppercase;
}

.card-header {
  display: flex;
  gap: 6px;
  margin-bottom: 8px;
  flex-wrap: wrap;
  align-items: center;
}

.category-badge {
  font-size: 0.65rem;
  padding: 2px 6px;
  border-radius: 4px;
  font-weight: 500;
  text-transform: uppercase;
}

.model-badge {
  font-size: 0.6rem;
  color: var(--accent-blue);
  background: rgba(96, 165, 250, 0.15);
  padding: 2px 6px;
  border-radius: 4px;
}

.token-cost {
  font-size: 0.6rem;
  color: var(--text-muted);
  margin-left: auto;
}

.card-body {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.streaming-dot {
  width: 8px;
  height: 8px;
  min-width: 8px;
  background: var(--accent-green);
  border-radius: 50%;
  margin-top: 4px;
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.5; transform: scale(1.2); }
}

.card-description {
  font-size: 0.85rem;
  line-height: 1.4;
  color: var(--text-primary);
}

.card-steps {
  display: flex;
  align-items: center;
  gap: 4px;
  margin: 8px 0;
  font-size: 0.75rem;
  color: var(--text-secondary);
}

.steps-count {
  color: var(--accent-blue);
}

.card-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
}

.footer-left, .footer-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.agent-badge {
  font-size: 0.65rem;
  color: var(--text-secondary);
  background: var(--bg-secondary);
  padding: 2px 6px;
  border-radius: 4px;
}

.confidence {
  font-size: 0.7rem;
  color: var(--accent-green);
  font-weight: 600;
}

.loop-warning {
  font-size: 0.7rem;
  color: var(--accent-red);
  background: rgba(248, 113, 113, 0.2);
  padding: 2px 6px;
  border-radius: 4px;
  animation: shake 0.5s ease-in-out;
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  25% { transform: translateX(-2px); }
  75% { transform: translateX(2px); }
}

.card-time {
  font-size: 0.7rem;
  color: var(--text-muted);
}

.empty-column {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-muted);
  font-size: 0.85rem;
}
</style>
