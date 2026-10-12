<template>
  <div class="mb-2">
    <div class="flex justify-between text-xs mb-1">
      <div>
        <div class="flex items-center gap-1.5">
          <span class="text-gray-700 dark:text-gray-300 font-medium block">{{ label }}</span>
          <span
            v-if="tooltip"
            class="relative group/slider-tip inline-flex items-center"
            :title="tooltip"
          >
            <span
              class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 group-hover/slider-tip:bg-primary/20 group-hover/slider-tip:text-primary text-[10px] font-bold cursor-help transition-colors"
            >?</span>
            <span
              role="tooltip"
              class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/slider-tip:block z-40 w-64 p-2 text-xs font-normal normal-case text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none"
            >
              {{ tooltip }}
            </span>
          </span>
        </div>
        <span v-if="subtitle" class="text-text-secondary scale-90 origin-top-left block">{{ subtitle }}</span>
      </div>
      <span class="text-primary font-bold">{{ modelValue }}</span>
    </div>
    <input
      type="range"
      class="w-full h-1.5 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary mt-1"
      :min="min ?? 0"
      :max="max ?? 10"
      :step="step ?? 1"
      :value="modelValue"
      @input="$emit('update:modelValue', Number(($event.target as HTMLInputElement).value))"
    >
  </div>
</template>

<script setup lang="ts">
withDefaults(
  defineProps<{
    label: string
    modelValue: number
    subtitle?: string
    tooltip?: string
    min?: number
    max?: number
    step?: number
  }>(),
  {
    subtitle: '',
    tooltip: '',
    min: 0,
    max: 10,
    step: 1,
  }
)
defineEmits(['update:modelValue'])
</script>
