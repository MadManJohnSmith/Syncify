<template>
  <div 
    ref="scrollContainer"
    class="infinite-scroll overflow-y-auto"
    @scroll="handleScroll"
  >
    <!-- Content -->
    <slot></slot>
    
    <!-- Loading Indicator -->
    <Transition name="fade">
      <div v-if="loading" class="py-4 flex items-center justify-center gap-2">
        <LoadingSpinner size="sm" color="gray" />
        <span class="text-sm text-gray-400">Loading more...</span>
      </div>
    </Transition>
    
    <!-- End Indicator -->
    <Transition name="fade">
      <div v-if="finished && !loading" class="py-4 text-center">
        <span class="text-sm text-gray-400">{{ finishedText }}</span>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import LoadingSpinner from './LoadingSpinner.vue'

const props = withDefaults(defineProps<{
  loading?: boolean
  finished?: boolean
  threshold?: number
  finishedText?: string
}>(), {
  loading: false,
  finished: false,
  threshold: 100,
  finishedText: 'No more items to load'
})

const emit = defineEmits(['load-more'])

const scrollContainer = ref<HTMLElement | null>(null)

function handleScroll() {
  if (props.loading || props.finished) return
  
  const container = scrollContainer.value
  if (!container) return
  
  const { scrollTop, scrollHeight, clientHeight } = container
  const distanceFromBottom = scrollHeight - scrollTop - clientHeight
  
  if (distanceFromBottom <= props.threshold) {
    emit('load-more')
  }
}

// Also trigger on resize in case content becomes shorter
onMounted(() => {
  window.addEventListener('resize', handleScroll)
})

onUnmounted(() => {
  window.removeEventListener('resize', handleScroll)
})
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
