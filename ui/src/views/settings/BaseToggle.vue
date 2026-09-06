<template>
  <!-- @deprecated Zombie duplicate. Use @/components/settings/BaseToggle.vue instead. -->
  <BaseToggle
    :title="props.title"
    :subtitle="props.subtitle"
    :checked="props.checked"
    @click="$emit('click')"
  />
</template>

<script setup lang="ts">
/**
 * @deprecated Zombie duplicate component. Use @/components/settings/BaseToggle.vue instead.
 */
import BaseToggle from '@/components/settings/BaseToggle.vue'

const props = withDefaults(
  defineProps<{
    title?: string
    subtitle?: string
    checked?: boolean
  }>(),
  {
    title: '',
    checked: false,
  }
)

defineEmits<{
  (e: 'click'): void
}>()
</script>
