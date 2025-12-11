<script setup lang="ts">
import { computed, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import KanbanColumn from './KanbanColumn.vue'

interface Feature {
  id: string
  projectDir: string
  description: string
  category: string
  passes: boolean
  inProgress: boolean
  agent?: string
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

interface FeatureUpdate {
  passes?: boolean
  inProgress?: boolean
  agent?: string
  confidence?: number
  model?: string
  isStreaming?: boolean
  retryCount?: number
  tokenCost?: number
  hasError?: boolean
  manualPriority?: string
}

const props = defineProps<{
  todo: Feature[]
  inProgress: Feature[]
  done: Feature[]
}>()

const emit = defineEmits<{
  'feature-click': [feature: Feature]
  'feature-updated': []
}>()

// Track which columns are expanded (max 2)
const expandedColumns = ref<Set<string>>(new Set(['inProgress']))

const columns = computed(() => [
  { id: 'todo', title: 'To Do', features: props.todo, color: 'var(--accent-blue)' },
  { id: 'inProgress', title: 'In Progress', features: props.inProgress, color: 'var(--accent-yellow)' },
  { id: 'done', title: 'Done', features: props.done, color: 'var(--accent-green)' },
])

function isCollapsed(columnId: string): boolean {
  return !expandedColumns.value.has(columnId)
}

function expandColumn(columnId: string) {
  const expanded = new Set(expandedColumns.value)
  expanded.add(columnId)

  // Limit to 2 max
  if (expanded.size > 2) {
    // Remove the first one that isn't the one we just added
    for (const id of expanded) {
      if (id !== columnId) {
        expanded.delete(id)
        break
      }
    }
  }

  expandedColumns.value = expanded
}

function collapseColumn(columnId: string) {
  const expanded = new Set(expandedColumns.value)

  // Only collapse if there's at least one other expanded column
  if (expanded.size > 1) {
    expanded.delete(columnId)
    expandedColumns.value = expanded
  }
}

// Map column ID to feature state
function columnToState(columnId: string): { passes: boolean; inProgress: boolean } {
  switch (columnId) {
    case 'todo':
      return { passes: false, inProgress: false }
    case 'inProgress':
      return { passes: false, inProgress: true }
    case 'done':
      return { passes: true, inProgress: false }
    default:
      return { passes: false, inProgress: false }
  }
}

// Handle drag-drop (human override)
async function handleFeatureDrop(featureId: string, targetColumn: string) {
  const state = columnToState(targetColumn)

  const update: FeatureUpdate = {
    passes: state.passes,
    inProgress: state.inProgress,
  }

  try {
    await invoke('update_feature', {
      featureId,
      update,
      source: 'human', // This triggers the 5-minute override lock
    })

    // Notify parent to refresh data
    emit('feature-updated')
  } catch (e) {
    console.error('Failed to update feature:', e)
  }
}

// Keyboard navigation
function handleKeydown(e: KeyboardEvent) {
  const columnIds = ['todo', 'inProgress', 'done']
  const expandedList = Array.from(expandedColumns.value)

  if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
    e.preventDefault()

    // Find current focus and move
    const currentIndex = expandedList.length > 0
      ? columnIds.indexOf(expandedList[0])
      : 1

    const direction = e.key === 'ArrowLeft' ? -1 : 1
    const newIndex = Math.max(0, Math.min(2, currentIndex + direction))

    expandedColumns.value = new Set([columnIds[newIndex]])
  }
}

// Calculate flex value for each column based on expanded state
function getColumnClass(columnId: string): string {
  return isCollapsed(columnId) ? 'column-collapsed' : 'column-expanded'
}
</script>

<template>
  <div
    class="kanban-board"
    tabindex="0"
    @keydown="handleKeydown"
  >
    <KanbanColumn
      v-for="column in columns"
      :key="column.id"
      :class="getColumnClass(column.id)"
      :column-id="column.id"
      :title="column.title"
      :features="column.features"
      :color="column.color"
      :collapsed="isCollapsed(column.id)"
      @feature-click="(f) => emit('feature-click', f)"
      @expand="expandColumn(column.id)"
      @collapse="collapseColumn(column.id)"
      @feature-drop="handleFeatureDrop"
    />
  </div>
</template>

<style scoped>
.kanban-board {
  display: flex;
  gap: 12px;
  height: 100%;
  min-height: 0;
  outline: none;
}

.kanban-board:focus {
  outline: none;
}

/* Collapsed columns have fixed width */
.column-collapsed {
  flex: 0 0 60px;
}

/* Expanded columns share remaining space */
.column-expanded {
  flex: 1 1 0;
  min-width: 280px;
}

/* Responsive: on smaller screens, reduce gap */
@media (max-width: 900px) {
  .kanban-board {
    gap: 8px;
  }

  .column-expanded {
    min-width: 200px;
  }
}
</style>
