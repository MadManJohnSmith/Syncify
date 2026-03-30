<template>
  <div class="image-loader relative overflow-hidden" :class="containerClass">
    <!-- Placeholder -->
    <Transition name="fade">
      <div 
        v-if="!loaded || error"
        class="absolute inset-0 bg-gray-200 dark:bg-gray-700 flex items-center justify-center"
      >
        <span class="material-symbols-outlined text-gray-400" :class="iconClass">
          {{ error ? 'broken_image' : 'music_note' }}
        </span>
      </div>
    </Transition>
    
    <!-- Actual Image -->
    <img 
      v-if="src && !error"
      :src="src"
      :alt="alt"
      @load="onLoad"
      @error="onError"
      class="w-full h-full object-cover transition-opacity duration-300"
      :class="{ 'opacity-0': !loaded }"
    >
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const props = withDefaults(defineProps<{
  src?: string
  alt?: string
  size?: 'sm' | 'md' | 'lg' | 'xl'
  rounded?: 'none' | 'sm' | 'md' | 'lg' | 'full'
}>(), {
  alt: 'Album art',
  size: 'md',
  rounded: 'lg'
})

const loaded = ref(false)
const error = ref(false)

const containerClass = computed(() => {
  const classes = []
  
  switch (props.size) {
    case 'sm': classes.push('w-10 h-10'); break
    case 'md': classes.push('w-12 h-12'); break
    case 'lg': classes.push('w-16 h-16'); break
    case 'xl': classes.push('w-48 h-48'); break
  }
  
  switch (props.rounded) {
    case 'none': break
    case 'sm': classes.push('rounded'); break
    case 'md': classes.push('rounded-lg'); break
    case 'lg': classes.push('rounded-xl'); break
    case 'full': classes.push('rounded-full'); break
  }
  
  return classes.join(' ')
})

const iconClass = computed(() => {
  switch (props.size) {
    case 'sm': return 'text-lg'
    case 'md': return 'text-xl'
    case 'lg': return 'text-2xl'
    case 'xl': return 'text-4xl'
    default: return 'text-xl'
  }
})

function onLoad() {
  loaded.value = true
}

function onError() {
  error.value = true
}
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
