<template>
  <!-- @deprecated Zombie duplicate. Use @/components/settings/BaseInput.vue instead. -->
  <BaseInput
    :label="props.label"
    :model-value="props.modelValue"
    :placeholder="props.placeholder"
    :subtitle="props.subtitle"
    :type="props.type"
    @update:model-value="$emit('update:modelValue', $event)"
    @change="$emit('change', $event)"
  />
</template>

<script setup lang="ts">
/**
 * @deprecated Zombie duplicate component. Use @/components/settings/BaseInput.vue instead.
 */
import BaseInput from '@/components/settings/BaseInput.vue'

const props = withDefaults(
  defineProps<{
    label?: string
    modelValue?: any
    placeholder?: string
    subtitle?: string
    type?: string
  }>(),
  {
    modelValue: '',
  }
)

defineEmits<{
  (e: 'update:modelValue', value: any): void
  (e: 'change', value: any): void
}>()
</script>
