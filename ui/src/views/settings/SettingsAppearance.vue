<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <p class="text-sm text-text-secondary">
      El tema se aplica al instante y queda guardado para las próximas sesiones (no pasa por «Save Changes»).
    </p>

    <section class="grid grid-cols-1 md:grid-cols-2 gap-4" data-testid="appearance-themes">
      <article
        v-for="theme in themes"
        :key="theme.id"
        :data-theme="theme.id"
        :data-testid="`theme-card-${theme.id}`"
        :class="[
          'p-4 rounded-xl bg-surface text-ink border transition-shadow',
          current === theme.id ? 'border-transparent ring-2 ring-accent' : 'border-line'
        ]"
      >
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0">
            <h3 class="text-sm font-semibold">{{ theme.name }}</h3>
            <p class="text-xs text-ink-2 mt-0.5">{{ theme.description }}</p>
          </div>
          <span
            v-if="current === theme.id"
            class="material-symbols-outlined text-accent text-[20px] shrink-0"
            title="Tema activo"
          >check_circle</span>
        </div>

        <!-- Mini-paleta: los chips leen los tokens reales de cada tema porque
             la propia tarjeta lleva su data-theme (cero hex duplicados aquí). -->
        <div class="flex gap-1.5 mt-3" aria-hidden="true">
          <span
            v-for="token in PALETTE_TOKENS"
            :key="token"
            class="w-6 h-6 rounded-md border border-line"
            :style="{ background: `var(${token})` }"
          ></span>
        </div>

        <button
          type="button"
          class="mt-3 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors disabled:cursor-default"
          :class="current === theme.id
            ? 'bg-accent-soft text-accent'
            : 'bg-accent text-on-accent hover:bg-accent-hover'"
          :disabled="current === theme.id"
          :aria-pressed="current === theme.id"
          @click="setTheme(theme.id)"
        >
          {{ current === theme.id ? 'Tema activo' : 'Usar este tema' }}
        </button>

        <!-- Control exclusivo de Camaleón: escala la saturación de sus tokens -->
        <label v-if="theme.id === 'camaleon'" class="block mt-4 text-xs text-ink-2">
          <span class="flex items-center justify-between">
            <span>Intensidad del tinte</span>
            <span class="font-mono tabular-nums" data-testid="tint-intensity-value">{{ tintPercent }}%</span>
          </span>
          <input
            data-testid="tint-intensity"
            type="range"
            min="0"
            max="100"
            step="1"
            :value="tintPercent"
            class="w-full mt-1 h-1 accent-primary cursor-pointer"
            aria-label="Intensidad del tinte de Camaleón"
            @input="onTintInput"
          >
        </label>
      </article>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useTheme } from '@/composables/useTheme'
import { tintIntensity, setTintIntensity } from '@/composables/useTint'

const { themes, current, setTheme } = useTheme()

// Mini-paleta representativa de cada tema: base, superficie, superficie
// elevada, acento y corazón.
const PALETTE_TOKENS = ['--bg-base', '--surface-1', '--surface-2', '--accent', '--heart'] as const

const tintPercent = computed(() => Math.round(tintIntensity.value * 100))

function onTintInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value)
  setTintIntensity(value / 100)
}
</script>
