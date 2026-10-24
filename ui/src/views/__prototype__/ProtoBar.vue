<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    value: number | null
    tone?: 'accent' | 'ok' | 'warn' | 'danger' | 'neutral'
    height?: 'sm' | 'md'
    emptyLabel?: string
  }>(),
  { tone: 'accent', height: 'md', emptyLabel: 'Sin datos' },
)

const pct = computed(() => (props.value == null ? 0 : Math.max(0, Math.min(100, props.value * 100))))

const color = computed(() => {
  if (props.value == null) return 'bg-elevated'
  switch (props.tone) {
    case 'ok':
      return 'bg-ok'
    case 'warn':
      return 'bg-warn'
    case 'danger':
      return 'bg-error'
    case 'neutral':
      return 'bg-ink-3'
    default:
      return 'bg-accent'
  }
})
</script>

<template>
  <div
    class="relative w-full overflow-hidden rounded-full bg-elevated/80"
    :class="height === 'sm' ? 'h-1' : 'h-1.5'"
    role="progressbar"
    :aria-valuenow="value == null ? undefined : Math.round(pct)"
    aria-valuemin="0"
    aria-valuemax="100"
  >
    <div v-if="value != null" class="h-full rounded-full transition-[width] duration-500" :class="color" :style="{ width: `${pct}%` }" />
    <div v-else class="h-full w-full opacity-60" :title="emptyLabel" />
  </div>
</template>
