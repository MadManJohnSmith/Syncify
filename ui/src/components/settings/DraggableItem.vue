<template>
  <div
    :draggable="true"
    :data-testid="`draggable-item-${index}`"
    :class="[
      'flex items-center gap-4 p-3 rounded-lg border bg-white dark:bg-surface-dark transition-colors group',
      isDragging
        ? 'opacity-40 border-primary'
        : 'border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight/30',
      isDropTarget ? 'ring-2 ring-primary' : ''
    ]"
    @dragstart="handleDragStart"
    @dragend="handleDragEnd"
    @dragover.prevent="handleDragOver"
    @drop.prevent="handleDrop"
  >
    <div
      class="text-gray-400 group-hover:text-gray-600 dark:group-hover:text-gray-300 cursor-grab active:cursor-grabbing"
      :data-testid="`drag-handle-${index}`"
    >
      <span class="material-symbols-outlined">drag_indicator</span>
    </div>
    <div class="h-6 w-6 rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center text-xs font-medium text-gray-600 dark:text-gray-400">{{ index }}</div>
    <span class="flex-1 min-w-0">
      <span class="block text-sm font-medium text-gray-900 dark:text-white">{{ text }}</span>
      <span v-if="subtitle" class="block text-xs text-text-secondary capitalize">{{ subtitle }}</span>
    </span>
    <label v-if="autoImport !== undefined" class="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400 cursor-pointer" @click.stop>
      <input
        type="checkbox"
        :checked="autoImport"
        :aria-label="`Auto-import for ${text}`"
        @change="$emit('toggle-auto-import')"
        class="w-3.5 h-3.5 rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-gray-300 dark:border-gray-600"
      >
      <span>Auto-import</span>
    </label>
    <div class="flex items-center gap-2">
      <slot name="actions" />
      <div class="flex flex-col gap-0.5 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
        <button
          type="button"
          :disabled="index === 1"
          :aria-label="`Move ${text} up`"
          class="text-gray-400 hover:text-primary disabled:opacity-30 disabled:hover:text-gray-400"
          @click.stop="$emit('move-up')"
        >
          <span class="material-symbols-outlined text-[16px]">keyboard_arrow_up</span>
        </button>
        <button
          type="button"
          :disabled="index === total"
          :aria-label="`Move ${text} down`"
          class="text-gray-400 hover:text-primary disabled:opacity-30 disabled:hover:text-gray-400"
          @click.stop="$emit('move-down')"
        >
          <span class="material-symbols-outlined text-[16px]">keyboard_arrow_down</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'

const props = withDefaults(
  defineProps<{
    text: string
    index: number
    total?: number
    subtitle?: string
    autoImport?: boolean
  }>(),
  { total: 0 }
)

const emit = defineEmits([
  'move-up',
  'move-down',
  'toggle-auto-import',
  'drag-start',
  'reorder',
])

const isDragging = ref(false)
const isDropTarget = ref(false)

// Callers that do not pass `total` still need the last row to disable "down",
// so fall back to "at least this row" rather than leaving the button live.
const total = computed(() => props.total || props.index)

function handleDragStart(event: DragEvent) {
  isDragging.value = true
  event.dataTransfer?.setData('text/plain', String(props.index))
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
  emit('drag-start', props.index - 1)
}

function handleDragOver() {
  isDropTarget.value = true
}

function handleDrop() {
  isDropTarget.value = false
  emit('reorder', props.index - 1)
}

function handleDragEnd() {
  isDragging.value = false
  isDropTarget.value = false
}
</script>