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
      <span class="inline-flex items-center gap-1.5 text-sm font-medium text-gray-900 dark:text-white">
        <span>{{ title }}</span>
        <span
          v-if="tooltip"
          class="relative group/toggle-tip inline-flex items-center pointer-events-auto"
          :title="tooltip"
        >
          <span
            class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 group-hover/toggle-tip:bg-primary/20 group-hover/toggle-tip:text-primary text-[10px] font-bold cursor-help transition-colors"
          >?</span>
          <span
            role="tooltip"
            class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/toggle-tip:block z-40 w-64 p-2 text-xs font-normal normal-case text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none"
          >
            {{ tooltip }}
          </span>
        </span>
      </span>
      <span v-if="subtitle" class="block text-xs text-text-secondary mt-0.5">{{ subtitle }}</span>
    </span>
    <span
      aria-hidden="true"
      class="relative inline-block w-10 h-5 align-middle select-none transition duration-200 ease-in pointer-events-none shrink-0"
    >
      <span
        :class="[
          'absolute block w-5 h-5 rounded-full border-4 transition-all duration-200 ease-in',
          checked ? 'right-0 bg-on-accent border-primary' : 'left-0 bg-white border-gray-300 dark:border-gray-700'
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
    tooltip?: string
    checked: boolean
    disabled?: boolean
    testId?: string
  }>(),
  {
    subtitle: '',
    tooltip: '',
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