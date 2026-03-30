<template>
  <div>
    <label v-if="label" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{{ label }}</label>
    <div class="relative">
      <select 
        :value="modelValue"
        @change="handleChange"
        class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer"
      >
        <option v-for="opt in options" :key="typeof opt === 'string' ? opt : opt.value" :value="typeof opt === 'string' ? opt : opt.value">
          {{ typeof opt === 'string' ? opt : opt.label }}
        </option>
      </select>
      <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-gray-500">
        <span class="material-symbols-outlined text-[20px]">expand_more</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  label?: string
  options: (string | { label: string; value: any })[]
  modelValue: any
}>()

const emit = defineEmits(['update:modelValue', 'change'])

const handleChange = (event: Event) => {
  const value = (event.target as HTMLSelectElement).value
  emit('update:modelValue', value)
  emit('change', value)
}
</script>
