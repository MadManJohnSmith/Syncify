<template>
  <div class="empty-state flex flex-col items-center justify-center py-16 px-8 text-center">
    <!-- Icon -->
    <div class="w-20 h-20 rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center mb-6">
      <span class="material-symbols-outlined text-4xl text-gray-400">{{ icon }}</span>
    </div>
    
    <!-- Heading -->
    <h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">{{ title }}</h3>
    
    <!-- Message -->
    <p class="text-gray-500 max-w-md mb-6">{{ message }}</p>
    
    <!-- Primary Action -->
    <button 
      v-if="actionLabel"
      @click="$emit('action')" 
      class="px-6 py-2.5 bg-primary hover:bg-primary-hover text-white font-medium rounded-xl flex items-center gap-2"
    >
      <span v-if="actionIcon" class="material-symbols-outlined text-lg">{{ actionIcon }}</span>
      {{ actionLabel }}
    </button>
    
    <!-- Custom Actions Slot -->
    <slot name="actions"></slot>
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  icon?: string
  title: string
  message: string
  actionLabel?: string
  actionIcon?: string
}>(), {
  icon: 'inbox'
})

defineEmits(['action'])
</script>

<style scoped>
</style>
