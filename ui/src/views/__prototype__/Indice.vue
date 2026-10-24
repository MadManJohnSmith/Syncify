<script setup lang="ts">
import { PROTO_TABS, TOTAL_VARIANTS } from './manifest'
</script>

<template>
  <div class="min-h-screen bg-base text-ink">
    <header class="border-b border-line px-10 pb-6 pt-10">
      <div class="mx-auto max-w-5xl">
        <div class="flex items-center gap-2">
          <span class="rounded bg-warn/15 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-widest text-warn">Prototipo</span>
          <span class="text-[9.5px] uppercase tracking-widest text-ink-3">sin persistencia</span>
        </div>
        <h1 class="mt-3 text-2xl font-semibold tracking-tight">Alternativas de interfaz</h1>
        <p class="mt-1 max-w-2xl text-[12.5px] leading-relaxed text-ink-2">
          {{ PROTO_TABS.length }} tabs · {{ TOTAL_VARIANTS }} variantes. Elige un tab y salta entre sus
          versiones con la barra inferior; el botón «¿Cómo se calcula?» explica de dónde sale cada cifra.
        </p>
        <p class="mt-2 max-w-2xl text-[11px] leading-relaxed text-ink-3">
          Nada de lo que hay aquí toca la base de datos ni el backend: son maquetas en memoria.
          Cuando elijas, esta carpeta se borra.
        </p>
      </div>
    </header>

    <main class="mx-auto max-w-5xl px-10 py-8">
      <div class="grid gap-4 md:grid-cols-2">
        <section
          v-for="tab in PROTO_TABS"
          :key="tab.id"
          class="rounded-xl border border-line bg-surface/50 p-4"
        >
          <div class="flex items-baseline justify-between gap-3">
            <h2 class="text-[14px] font-medium text-ink">{{ tab.label }}</h2>
            <span class="text-[9.5px] tabular-nums text-ink-3">{{ tab.variants.length }}</span>
          </div>
          <p class="mt-0.5 text-[11px] text-ink-3">{{ tab.blurb }}</p>

          <ul class="mt-3 space-y-1">
            <li v-for="variant in tab.variants" :key="variant.id">
              <RouterLink
                :to="{ path: tab.route, query: { variant: variant.id } }"
                class="group flex items-baseline gap-2 rounded-lg px-2 py-1.5 transition hover:bg-elevated"
              >
                <span class="w-[110px] shrink-0 text-[11.5px] font-medium text-ink group-hover:text-accent">{{ variant.label }}</span>
                <span class="min-w-0 flex-1 text-[10.5px] leading-snug text-ink-3">{{ variant.hint }}</span>
                <span class="material-symbols-outlined shrink-0 text-[14px] text-ink-3 opacity-0 transition group-hover:opacity-100">arrow_forward</span>
              </RouterLink>
            </li>
          </ul>
        </section>
      </div>

      <footer class="mt-8 rounded-xl border border-line bg-surface/30 px-4 py-3">
        <p class="text-[11px] leading-relaxed text-ink-2">
          <span class="font-medium text-ink">Cómo leer esto:</span> cada variante es una forma
          distinta de contar lo mismo —qué se ve primero, qué se calla y qué se explica—. Aquí solo
          está el tab de accounts: cinco lecturas de las conexiones por servicio, con datos
          simulados en memoria.
        </p>
      </footer>
    </main>
  </div>
</template>
