<template>
  <div class="splash-screen fixed inset-0 z-[500] bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900 flex flex-col items-center justify-center select-none">
    <!-- Logo -->
    <div class="logo-container mb-8">
      <div class="w-28 h-28 rounded-3xl bg-gradient-to-br from-primary to-primary-600 flex items-center justify-center shadow-2xl shadow-primary/30">
        <span class="material-symbols-outlined text-white text-6xl">music_note</span>
      </div>
    </div>
    
    <!-- App Name -->
    <div class="text-center">
      <h1 class="text-4xl font-bold text-white mb-2">Syncify</h1>
      <p class="text-gray-400 text-lg">Your Unified Music Library</p>
    </div>
    
    <!-- Error State -->
    <div v-if="error" class="mt-8 text-center max-w-md px-6 flex flex-col items-center" data-testid="splash-error">
      <div class="flex items-center gap-2 text-rose-400 bg-rose-500/10 border border-rose-500/20 px-4 py-2.5 rounded-xl mb-4 text-sm font-medium">
        <span class="material-symbols-outlined text-xl shrink-0">error</span>
        <span class="text-left">{{ error }}</span>
      </div>
      <button 
        @click="emit('retry')"
        type="button"
        data-testid="splash-retry-btn"
        class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-xl text-sm font-semibold transition-all shadow-lg shadow-primary/20 flex items-center gap-2 cursor-pointer"
      >
        <span class="material-symbols-outlined text-lg">refresh</span>
        <span>Retry Initialization</span>
      </button>
    </div>

    <!-- Normal Loading Indicator -->
    <div v-else class="mt-12 w-64 text-center" data-testid="splash-loading">
      <!-- Progress Bar -->
      <div class="h-1.5 bg-gray-700/60 rounded-full overflow-hidden mb-3">
        <div 
          class="h-full bg-primary rounded-full transition-all duration-300 ease-out"
          :style="{ width: displayProgress + '%' }"
        ></div>
      </div>
      
      <!-- Status Text -->
      <p class="text-sm text-gray-400 font-medium" data-testid="splash-status">{{ displayStatusText }}</p>
    </div>
    
    <!-- Version -->
    <p class="absolute bottom-6 text-xs text-gray-600">v2.1.0</p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  error?: string | null
  statusText?: string
  progress?: number
}>(), {
  error: null,
  statusText: '',
  progress: 0,
})

const emit = defineEmits<{
  (e: 'ready'): void
  (e: 'complete'): void
  (e: 'retry'): void
}>()

const displayProgress = computed(() => {
  if (typeof props.progress === 'number' && props.progress >= 0) {
    return Math.min(100, Math.max(0, props.progress))
  }
  return 0
})

const displayStatusText = computed(() => {
  return props.statusText || 'Loading...'
})

function hide() {
  emit('ready')
  emit('complete')
}

function appReady() {
  hide()
}

defineExpose({ appReady, hide, displayProgress, displayStatusText })
</script>

<style scoped>
/* Logo pulse animation */
.logo-container {
  animation: pulse 2s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { transform: scale(1); }
  50% { transform: scale(1.02); }
}
</style>
