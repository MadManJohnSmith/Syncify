<template>
  <div class="mb-2">
    <div class="flex justify-between text-xs mb-1">
      <div>
         <span class="text-gray-700 dark:text-gray-300 font-medium block">{{ label }}</span>
         <span v-if="subtitle" class="text-text-secondary scale-90 origin-top-left block">{{ subtitle }}</span>
      </div>
      <span class="text-primary font-bold">{{ modelValue }}</span>
    </div>
    <input 
      type="range" 
      class="w-full h-1.5 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary mt-1" 
      min="0" max="10" step="1" 
      :value="modelValue"
      @input="$emit('update:modelValue', Number(($event.target as HTMLInputElement).value))"
    >
  </div>
</template>

<script setup lang="ts">
defineProps<{
  label: string
  modelValue: number
  subtitle?: string
}>()
defineEmits(['update:modelValue'])
</script>
