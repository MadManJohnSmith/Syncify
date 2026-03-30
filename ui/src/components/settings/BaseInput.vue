<template>
  <div>
    <label v-if="label" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{{ label }}</label>
    <input 
      :type="type || 'text'" 
      :value="modelValue" 
      :placeholder="placeholder"
      @input="handleInput($event, false)"
      @change="handleInput($event, true)"
      class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none transition-all"
    />
    <p v-if="subtitle" class="mt-1 text-xs text-text-secondary">{{ subtitle }}</p>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  label?: string
  modelValue: any
  placeholder?: string
  subtitle?: string
  type?: string
}>()

const emit = defineEmits(['update:modelValue', 'change'])

const handleInput = (event: Event, isChange: boolean) => {
  const val = (event.target as HTMLInputElement).value
  const parsedVal = props.type === 'number' ? Number(val) : val
  if (isChange) {
    emit('change', parsedVal)
  } else {
    emit('update:modelValue', parsedVal)
  }
}
</script>
