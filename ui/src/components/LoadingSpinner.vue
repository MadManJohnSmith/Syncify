<template>
  <div class="loading-spinner inline-flex items-center justify-center" :class="sizeClass">
    <!-- Spinner SVG -->
    <svg :class="['animate-spin', colorClass]" viewBox="0 0 24 24" fill="none">
      <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-linecap="round" opacity="0.25"></circle>
      <path d="M12 2a10 10 0 0 1 10 10" stroke="currentColor" stroke-width="3" stroke-linecap="round"></path>
    </svg>
    
    <!-- Optional Text -->
    <span v-if="text" :class="['ml-2', textClass]">{{ text }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  size?: 'xs' | 'sm' | 'md' | 'lg' | 'xl'
  color?: 'primary' | 'white' | 'gray'
  text?: string
}>(), {
  size: 'md',
  color: 'primary'
})

const sizeClass = computed(() => {
  switch (props.size) {
    case 'xs': return 'w-3 h-3'
    case 'sm': return 'w-4 h-4'
    case 'md': return 'w-6 h-6'
    case 'lg': return 'w-8 h-8'
    case 'xl': return 'w-12 h-12'
    default: return 'w-6 h-6'
  }
})

const colorClass = computed(() => {
  switch (props.color) {
    case 'primary': return 'text-primary'
    case 'white': return 'text-white'
    case 'gray': return 'text-gray-400'
    default: return 'text-primary'
  }
})

const textClass = computed(() => {
  switch (props.size) {
    case 'xs': return 'text-xs'
    case 'sm': return 'text-sm'
    default: return 'text-sm'
  }
})
</script>

<style scoped>
@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 0.8s linear infinite;
}
</style>
