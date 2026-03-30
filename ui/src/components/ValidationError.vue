<template>
  <div class="validation-error-wrapper">
    <!-- Input with validation -->
    <div class="relative">
      <slot></slot>
      
      <!-- Error Icon -->
      <span 
        v-if="hasError" 
        class="absolute right-3 top-1/2 -translate-y-1/2 material-symbols-outlined text-red-500 text-lg"
      >
        error
      </span>
    </div>
    
    <!-- Error Message -->
    <Transition name="slide-down">
      <div v-if="hasError" class="validation-error mt-1.5">
        <p class="text-sm text-red-500 flex items-center gap-1">
          <span class="material-symbols-outlined text-sm">error</span>
          {{ error }}
        </p>
        <p v-if="hint" class="text-xs text-gray-400 mt-0.5">{{ hint }}</p>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  error?: string
  hint?: string
}>()

const hasError = computed(() => !!props.error)
</script>

<style scoped>
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.2s ease;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-5px);
}

/* Shake animation for invalid inputs */
:deep(.invalid) {
  animation: shake 0.4s ease;
  border-color: #ef4444 !important;
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  20%, 60% { transform: translateX(-5px); }
  40%, 80% { transform: translateX(5px); }
}
</style>
