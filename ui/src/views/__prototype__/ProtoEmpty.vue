<script setup lang="ts">
/**
 * Estado vacío compartido.
 *
 * La distinción que importa no es estética sino semántica: **vacío por filtro**
 * (hay datos, no pasan) frente a **vacío de verdad** (no hay nada todavía). El
 * primero ofrece quitar el filtro; el segundo, crear. Antes había siete
 * redacciones distintas y ninguna distinguía los dos casos.
 */
const props = withDefaults(
  defineProps<{
    title: string
    hint?: string
    kind?: 'filtered' | 'empty'
    actionLabel?: string
    compact?: boolean
  }>(),
  { kind: 'filtered', compact: false },
)

defineEmits<{ action: [] }>()

const icon = () => (props.kind === 'filtered' ? 'filter_alt_off' : 'inbox')
</script>

<template>
  <div
    class="flex flex-col items-center justify-center text-center"
    :class="compact ? 'gap-1 py-6' : 'gap-2 py-14'"
  >
    <span class="material-symbols-outlined text-ink-3" :class="compact ? 'text-[18px]' : 'text-[28px]'">{{ icon() }}</span>
    <p class="text-ink-2" :class="compact ? 'text-[10.5px]' : 'text-[12.5px]'">{{ title }}</p>
    <p v-if="hint" class="max-w-xs text-ink-3" :class="compact ? 'text-[9.5px]' : 'text-[10.5px]'">{{ hint }}</p>
    <button
      v-if="actionLabel"
      class="mt-0.5 text-accent transition hover:underline"
      :class="compact ? 'text-[10px]' : 'text-[11.5px]'"
      @click="$emit('action')"
    >
      {{ actionLabel }}
    </button>
  </div>
</template>
