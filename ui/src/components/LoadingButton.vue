<template>
  <button 
    :disabled="loading || disabled"
    :class="[
      'loading-button inline-flex items-center justify-center gap-2 transition-all',
      loading && 'opacity-80 cursor-not-allowed'
    ]"
    v-bind="$attrs"
  >
    <!-- Spinner -->
    <Transition name="fade" mode="out-in">
      <span v-if="loading" class="inline-spinner">
        <svg class="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-linecap="round" opacity="0.25"></circle>
          <path d="M12 2a10 10 0 0 1 10 10" stroke="currentColor" stroke-width="3" stroke-linecap="round"></path>
        </svg>
      </span>
      <span v-else-if="$slots.icon">
        <slot name="icon"></slot>
      </span>
    </Transition>
    
    <!-- Text -->
    <span>{{ loading ? loadingText : '' }}<slot v-if="!loading"></slot></span>
  </button>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  loading?: boolean
  loadingText?: string
  disabled?: boolean
}>(), {
  loading: false,
  loadingText: 'Loading...',
  disabled: false
})
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 0.8s linear infinite;
}
</style>
