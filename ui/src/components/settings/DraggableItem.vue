<template>
  <div class="flex items-center gap-4 p-3 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors cursor-grab active:cursor-grabbing group">
      <div class="text-gray-400 group-hover:text-gray-600 dark:group-hover:text-gray-300"><span class="material-symbols-outlined">drag_indicator</span></div>
      <div class="h-6 w-6 rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center text-xs font-medium text-gray-600 dark:text-gray-400">{{ index }}</div>
      <span class="text-sm font-medium text-gray-900 dark:text-white flex-1">{{ text }}</span>
      <label v-if="autoImport !== undefined" class="flex items-center gap-2 text-xs text-gray-600 dark:text-gray-400 cursor-pointer" @click.stop>
        <input type="checkbox" :checked="autoImport" @change="$emit('toggle-auto-import')" class="w-3.5 h-3.5 rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-gray-300 dark:border-gray-600">
        <span>Auto-import</span>
      </label>
      <div class="flex flex-col gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
         <button @click.stop="$emit('move-up')" class="text-gray-400 hover:text-primary"><span class="material-symbols-outlined text-[16px]">keyboard_arrow_up</span></button>
         <button @click.stop="$emit('move-down')" class="text-gray-400 hover:text-primary"><span class="material-symbols-outlined text-[16px]">keyboard_arrow_down</span></button>
      </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  text: string
  index: number
  autoImport?: boolean
}>()

defineEmits(['move-up', 'move-down', 'toggle-auto-import'])
</script>
