<template>
  <div class="error-state flex flex-col items-center justify-center py-16 px-8 text-center">
    <!-- Icon -->
    <div :class="[
      'w-20 h-20 rounded-full flex items-center justify-center mb-6',
      type === 'error' ? 'bg-red-500/10' : type === 'warning' ? 'bg-amber-500/10' : 'bg-gray-500/10'
    ]">
      <span :class="[
        'material-symbols-outlined text-4xl',
        type === 'error' ? 'text-red-500' : type === 'warning' ? 'text-amber-500' : 'text-gray-400'
      ]">
        {{ iconName }}
      </span>
    </div>
    
    <!-- Heading -->
    <h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">{{ title }}</h3>
    
    <!-- Message -->
    <p class="text-gray-500 max-w-md mb-6">{{ message }}</p>
    
    <!-- Actions -->
    <div class="flex flex-wrap items-center justify-center gap-3">
      <button 
        v-if="showRetry"
        @click="$emit('retry')" 
        class="px-6 py-2.5 bg-primary hover:bg-primary-hover text-white font-medium rounded-xl flex items-center gap-2"
      >
        <span class="material-symbols-outlined text-lg">refresh</span>
        Retry
      </button>
      
      <button 
        v-if="showViewLogs"
        @click="$emit('view-logs')" 
        class="px-6 py-2.5 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-xl flex items-center gap-2"
      >
        <span class="material-symbols-outlined text-lg">description</span>
        View Logs
      </button>
      
      <slot name="actions"></slot>
    </div>
    
    <!-- Last Attempt -->
    <p v-if="lastAttempt" class="text-xs text-gray-400 mt-4">Last tried: {{ lastAttempt }}</p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  type?: 'error' | 'warning' | 'empty'
  icon?: string
  title: string
  message: string
  showRetry?: boolean
  showViewLogs?: boolean
  lastAttempt?: string
}>(), {
  type: 'error',
  showRetry: true,
  showViewLogs: false
})

const emit = defineEmits(['retry', 'view-logs'])

const iconName = computed(() => {
  if (props.icon) return props.icon
  switch (props.type) {
    case 'error': return 'error'
    case 'warning': return 'warning'
    case 'empty': return 'inbox'
    default: return 'error'
  }
})
</script>

<style scoped>
</style>
