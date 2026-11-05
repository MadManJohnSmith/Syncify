<script setup lang="ts">
import { computed, ref, type Component } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { ProtoNote, ProtoVariant } from './types'

/**
 * Anfitrión común de los prototipos de tab: marca de PROTOTIPO, barra de
 * variantes, panel «¿Cómo se calcula?» y sincronización con `?variant=`.
 *
 * Cada tab sigue decidiendo sus propias variantes; esto solo evita repetir
 * el andamiaje. Se borra con el resto de `__prototype__`.
 */

const props = defineProps<{
  tab: string
  subtitle: string
  variants: readonly ProtoVariant[]
  components: Record<string, Component>
  notes: readonly ProtoNote[]
  defaultVariant?: string
}>()

const route = useRoute()
const router = useRouter()
const notesOpen = ref(false)

const ids = computed(() => props.variants.map(variant => variant.id))
const current = computed(() => {
  const query = route.query.variant
  if (typeof query === 'string' && ids.value.includes(query)) return query
  return props.defaultVariant ?? ids.value[0]
})
const currentComponent = computed(() => props.components[current.value])

function select(id: string) {
  router.replace({ query: { ...route.query, variant: id } })
}
</script>

<template>
  <div class="relative flex h-full flex-col overflow-hidden bg-base">
    <div class="flex shrink-0 items-center justify-between border-b border-line bg-surface/40 px-6 py-1.5 text-[10px] leading-none text-ink-3">
      <span class="flex items-center gap-2">
        <RouterLink
          to="/__prototype"
          class="rounded px-1.5 py-0.5 transition hover:bg-elevated hover:text-ink"
          title="Volver al índice de tabs"
        >
          ← Todos
        </RouterLink>
        <span class="rounded bg-warn/15 px-1.5 py-0.5 font-semibold tracking-wider text-warn">PROTOTIPO</span>
        <span>{{ tab }} · {{ variants.length }} variantes · {{ subtitle }}</span>
      </span>
      <button class="rounded px-2 py-0.5 transition hover:bg-elevated hover:text-ink" @click="notesOpen = !notesOpen">
        ¿Cómo se calcula?
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-hidden">
      <component :is="currentComponent" />
    </div>

    <Transition name="proto-fade">
      <aside
        v-if="notesOpen"
        class="absolute right-0 top-8 z-30 flex h-[calc(100%-4rem)] w-[420px] flex-col border-l border-line bg-shell shadow-2xl"
      >
        <header class="flex items-center justify-between border-b border-line px-4 py-3">
          <div>
            <h2 class="text-sm font-semibold text-ink">Cómo se calcula</h2>
            <p class="text-[11px] text-ink-3">Misma tabla en las {{ variants.length }} variantes</p>
          </div>
          <button class="rounded p-1 text-ink-3 transition hover:bg-elevated hover:text-ink" @click="notesOpen = false">
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </header>
        <div class="min-h-0 flex-1 overflow-y-auto px-4 py-3">
          <ul class="space-y-2.5">
            <li v-for="note in notes" :key="note.label" class="rounded-lg border border-line/60 bg-surface/50 px-3 py-2">
              <div class="flex items-baseline justify-between gap-2">
                <span class="text-[12px] font-medium text-ink">{{ note.label }}</span>
                <span class="font-mono text-[13px] text-accent">{{ note.value }}</span>
              </div>
              <p class="mt-1 font-mono text-[10.5px] text-ink-2">{{ note.formula }}</p>
              <p class="mt-1 text-[10.5px] leading-snug text-ink-3">{{ note.caveat }}</p>
            </li>
          </ul>
        </div>
      </aside>
    </Transition>

    <nav class="pointer-events-none absolute bottom-5 left-1/2 z-40 w-full max-w-3xl -translate-x-1/2 px-4">
      <div class="pointer-events-auto flex items-stretch gap-1 rounded-2xl border border-line-strong bg-shell/95 p-1.5 shadow-2xl backdrop-blur">
        <button
          v-for="variant in variants"
          :key="variant.id"
          class="flex flex-1 flex-col items-start gap-0.5 rounded-xl px-3 py-1.5 text-left transition"
          :class="current === variant.id ? 'bg-accent text-on-accent shadow' : 'text-ink-2 hover:bg-elevated hover:text-ink'"
          :title="variant.hint"
          @click="select(variant.id)"
        >
          <span class="text-[11.5px] font-semibold leading-tight">{{ variant.label }}</span>
          <span class="hidden text-[10px] leading-tight opacity-70 lg:block">{{ variant.hint }}</span>
        </button>
      </div>
    </nav>
  </div>
</template>

<style scoped>
.proto-fade-enter-active,
.proto-fade-leave-active {
  transition: opacity 160ms ease, transform 160ms ease;
}
.proto-fade-enter-from,
.proto-fade-leave-to {
  opacity: 0;
  transform: translateX(12px);
}
</style>
