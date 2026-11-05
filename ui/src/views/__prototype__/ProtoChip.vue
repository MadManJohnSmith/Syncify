<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: 'ok' | 'warn' | 'danger' | 'neutral' | 'accent' | 'info'
    icon?: string
    dot?: boolean
  }>(),
  { tone: 'neutral', icon: '', dot: false },
)
</script>

<template>
  <span
    class="inline-flex items-center gap-1 rounded-full px-2 py-[3px] text-[11px] font-medium leading-none whitespace-nowrap"
    :class="{
      'bg-ok/12 text-ok': tone === 'ok',
      'bg-warn/12 text-warn': tone === 'warn',
      'bg-error/12 text-error': tone === 'danger',
      'bg-elevated text-ink-2': tone === 'neutral',
      'bg-accent/12 text-accent': tone === 'accent',
      'bg-info/12 text-info': tone === 'info',
    }"
  >
    <span v-if="dot" class="h-1.5 w-1.5 rounded-full bg-current" />
    <span v-if="icon" class="material-symbols-outlined text-[13px] leading-none">{{ icon }}</span>
    <slot />
  </span>
</template>
