<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger' | 'text'
    size?: 'sm' | 'md'
    icon?: string
    active?: boolean
  }>(),
  { variant: 'secondary', size: 'md', icon: '', active: false },
)
</script>

<template>
  <button
    type="button"
    class="inline-flex items-center justify-center gap-1.5 rounded-lg font-medium transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50"
    :class="[
      size === 'sm' ? 'h-8 px-2.5 text-xs' : 'h-9 px-3.5 text-[13px]',
      variant === 'primary' && 'bg-accent text-on-accent hover:bg-accent-hover',
      variant === 'secondary' && 'bg-elevated text-ink hover:bg-accent/10',
      variant === 'ghost' && 'border border-line bg-transparent text-ink-2 hover:bg-surface hover:text-ink',
      variant === 'danger' && 'text-error hover:bg-error/10',
      variant === 'text' && 'px-1 text-ink-3 hover:text-ink',
      active && 'ring-1 ring-accent/60 bg-accent/10 text-accent',
    ]"
  >
    <span v-if="icon" class="material-symbols-outlined text-[17px] leading-none">{{ icon }}</span>
    <slot />
  </button>
</template>
