<template>
  <!--
    Role "switch" on a real <button>: the browser gives Enter/Space activation
    and the assistive name for free, which a click-only <div> did not.
  -->
  <button
    type="button"
    role="switch"
    :aria-checked="checked"
    :aria-label="accessibleName"
    :disabled="disabled"
    :data-testid="testId"
    :class="[
      'w-full flex items-center justify-between py-1 text-left rounded-lg focus:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-1 dark:focus-visible:ring-offset-surface-dark',
      disabled ? 'opacity-60 cursor-not-allowed' : 'cursor-pointer'
    ]"
    @click="!disabled && $emit('click')"
  >
    <span class="pr-8 pointer-events-none">
      <span class="block text-sm font-medium text-gray-900 dark:text-white">{{ title }}</span>
      <span v-if="subtitle" class="block text-xs text-text-secondary mt-0.5">{{ subtitle }}</span>
    </span>
    <span
      aria-hidden="true"
      class="relative inline-block w-10 h-5 align-middle select-none transition duration-200 ease-in pointer-events-none shrink-0"
    >
      <span
        :class="[
          'absolute block w-5 h-5 rounded-full bg-white border-4 transition-all duration-200 ease-in',
          checked ? 'right-0 border-primary' : 'left-0 border-gray-300 dark:border-gray-700'
        ]"
      />
      <span
        :class="[
          'block overflow-hidden h-5 rounded-full transition-colors duration-200 ease-in',
          checked ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-700'
        ]"
      />
    </span>
  </button>
</template>
<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    title: string
    subtitle?: string
    checked: boolean
    disabled?: boolean
    testId?: string
  }>(),
  {
    subtitle: '',
    disabled: false,
    testId: 'settings-toggle',
  }
)

defineEmits(['click'])

// The visual switch carries no text of its own, so the whole row is folded into
// the accessible name; otherwise a screen reader announces only "switch".
const accessibleName = computed(() =>
  props.subtitle ? `${props.title}. ${props.subtitle}` : props.title
)
</script>