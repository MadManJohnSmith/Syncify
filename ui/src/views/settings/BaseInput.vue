<template>
  <div>
    <label v-if="label" class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">{{ label }}</label>
    <input 
      :type="type || 'text'" 
      :value="modelValue || defaultValue" 
      :placeholder="placeholder"
      @input="handleInput"
      @change="handleChange"
      class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none transition-all"
    />
    <p v-if="subtitle" class="mt-1 text-xs text-text-secondary">{{ subtitle }}</p>
  </div>
</template>

<script setup lang="ts">
const props = defineProps({
  label: String,
  modelValue: [String, Number],
  defaultValue: [String, Number],
  placeholder: String,
  subtitle: String,
  type: String
})

const emit = defineEmits(['update:modelValue', 'change'])

const handleInput = (event: Event) => {
  const input = event.target as HTMLInputElement
  const val = props.type === 'number' ? Number(input.value) : input.value
  emit('update:modelValue', val)
}

const handleChange = (event: Event) => {
  const input = event.target as HTMLInputElement
  const val = props.type === 'number' ? Number(input.value) : input.value
  emit('change', val)
}
</script>
