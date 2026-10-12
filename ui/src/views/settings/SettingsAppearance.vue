<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <p class="text-sm text-text-secondary">
      El tema se aplica al instante y queda guardado para las próximas sesiones (no pasa por «Save Changes»).
    </p>

    <!-- Temas estándar del sistema (G-02: default Tinta) -->
    <section class="grid grid-cols-1 md:grid-cols-2 gap-4" data-testid="appearance-themes">
      <article
        v-for="theme in standardThemes"
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

    <!-- Temas por servicio musical (AJ-05) - Colocados debajo y separados -->
    <div class="pt-6 border-t border-line space-y-6">
      <div class="flex flex-col gap-1">
        <h2 class="text-base font-semibold text-ink">Temas por servicio</h2>
        <p class="text-xs text-ink-2">
          Paletas de colores diseñadas a partir de la identidad de cada servicio de streaming musical disponible.
        </p>
      </div>

      <!-- Conmutador para tintado dinámico por carátula (AJ-06) -->
      <div class="flex items-center justify-between p-4 rounded-xl bg-surface border border-line">
        <div class="space-y-0.5 pr-4">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-accent text-[20px]">palette</span>
            <span class="text-sm font-medium text-ink">Color dinámico por carátula</span>
          </div>
          <p class="text-xs text-ink-2">
            Permite que los temas de servicio adapten dinámicamente sus tonos de acento al color de la portada en reproducción.
          </p>
        </div>
        <button
          type="button"
          role="switch"
          data-testid="toggle-dynamic-cover"
          :aria-checked="dynamicCoverEnabled"
          :class="[
            'relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-accent focus:ring-offset-2',
            dynamicCoverEnabled ? 'bg-accent' : 'bg-surface-2 border-line'
          ]"
          @click="setDynamicCover(!dynamicCoverEnabled)"
        >
          <span
            :class="[
              'pointer-events-none inline-block h-5 w-5 transform rounded-full shadow-sm ring-0 transition duration-200 ease-in-out',
              dynamicCoverEnabled ? 'translate-x-5 bg-on-accent' : 'translate-x-0 bg-ink-2'
            ]"
          />
        </button>
      </div>

      <!-- Cuadrícula de temas por servicio -->
      <section class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4" data-testid="appearance-service-themes">
        <article
          v-for="theme in serviceThemes"
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

          <!-- Mini-paleta del servicio -->
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
        </article>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useTheme } from '@/composables/useTheme'
import { tintIntensity, setTintIntensity } from '@/composables/useTint'

const PALETTE_TOKENS = [
  '--bg-base',
  '--surface-1',
  '--surface-2',
  '--accent',
  '--text-1',
  '--text-2',
]

const {
  themes,
  standardThemes,
  serviceThemes,
  current,
  setTheme,
  dynamicCoverEnabled,
  setDynamicCover,
} = useTheme()

const tintPercent = computed(() => Math.round(tintIntensity.value * 100))

function onTintInput(event: Event) {
  const value = Number((event.target as HTMLInputElement).value) / 100
  setTintIntensity(value)
}
</script>
