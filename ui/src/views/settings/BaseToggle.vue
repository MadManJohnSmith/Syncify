<template>
  <div class="flex items-center justify-between py-1 cursor-pointer" @click="$emit('click')">
    <div class="pr-8 pointer-events-none">
      <span class="block text-sm font-medium text-gray-900 dark:text-white">{{ title }}</span>
      <span v-if="subtitle" class="block text-xs text-text-secondary mt-0.5">{{ subtitle }}</span>
    </div>
    <div class="relative inline-block w-10 align-middle select-none transition duration-200 ease-in pointer-events-none">
      <input type="checkbox" :checked="checked" class="toggle-checkbox absolute block w-5 h-5 rounded-full bg-white border-4 appearance-none checked:right-0 checked:border-primary"/>
      <label class="toggle-label block overflow-hidden h-5 rounded-full bg-gray-300 dark:bg-gray-700 checked:bg-primary"></label>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string
  subtitle?: string
  checked: boolean
}>()
defineEmits(['click'])
</script>

<style scoped>
.toggle-checkbox:checked {
  right: 0;
  border-color: #3c83f6;
}
.toggle-checkbox {
  right: 20px;
  transition: all 0.2s ease-in;
}
</style>
