<template>
  <div>
    <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{{ label }}</label>
    <div class="flex gap-2">
      <div class="flex-1 px-3 py-2 bg-gray-50 dark:bg-[#121b29]/50 border border-gray-300 dark:border-gray-600 rounded-lg text-sm text-gray-600 dark:text-gray-400 font-mono flex items-center overflow-hidden">
        {{ defaultPath }}
      </div>
      <button class="px-4 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
        Browse...
      </button>
       <button v-if="hasReset" class="px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg transition-colors" title="Reset to Default">
        <span class="material-symbols-outlined text-[20px]">restart_alt</span>
      </button>
    </div>
    <p v-if="subtitle" class="mt-1 text-xs text-text-secondary">{{ subtitle }}</p>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  label: string
  defaultPath: string
  subtitle?: string
  hasReset?: boolean
}>()
</script>
