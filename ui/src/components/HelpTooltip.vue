<template>
  <div class="help-tooltip-wrapper relative inline-flex items-center">
    <slot></slot>
    <button 
      @mouseenter="showTooltip = true"
      @mouseleave="showTooltip = false"
      @focus="showTooltip = true"
      @blur="showTooltip = false"
      class="help-tooltip-trigger ml-1 p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors cursor-help"
      type="button"
      aria-label="Help"
    >
      <span class="material-symbols-outlined text-[16px]">help</span>
    </button>
    
    <Transition name="tooltip">
      <div 
        v-if="showTooltip"
        class="help-tooltip absolute z-50"
        :class="[positionClasses]"
      >
        <div class="bg-gray-900 text-white text-xs px-3 py-2 rounded-lg shadow-lg max-w-[250px]">
          <p>{{ text }}</p>
          <a v-if="learnMore" :href="learnMore" class="text-primary-light hover:underline mt-1 block">Learn more →</a>
        </div>
        <!-- Arrow -->
        <div 
          class="tooltip-arrow absolute w-2 h-2 bg-gray-900 transform rotate-45"
          :class="arrowClasses"
        ></div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const props = defineProps<{
  text: string
  position?: 'top' | 'bottom' | 'left' | 'right'
  learnMore?: string
}>()

const showTooltip = ref(false)

const positionClasses = computed(() => {
  switch (props.position) {
    case 'bottom':
      return 'top-full left-1/2 -translate-x-1/2 mt-2'
    case 'left':
      return 'right-full top-1/2 -translate-y-1/2 mr-2'
    case 'right':
      return 'left-full top-1/2 -translate-y-1/2 ml-2'
    case 'top':
    default:
      return 'bottom-full left-1/2 -translate-x-1/2 mb-2'
  }
})

const arrowClasses = computed(() => {
  switch (props.position) {
    case 'bottom':
      return '-top-1 left-1/2 -translate-x-1/2'
    case 'left':
      return 'top-1/2 -right-1 -translate-y-1/2'
    case 'right':
      return 'top-1/2 -left-1 -translate-y-1/2'
    case 'top':
    default:
      return '-bottom-1 left-1/2 -translate-x-1/2'
  }
})
</script>

<style scoped>
.tooltip-enter-active,
.tooltip-leave-active {
  transition: all 0.1s ease;
}

.tooltip-enter-from,
.tooltip-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
