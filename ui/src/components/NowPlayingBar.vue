<template>
  <!-- S194 residual + fase 1: barra global del player con cola visible,
       modos persistentes y volumen/mute. -->
  <Transition name="player-slide">
    <div
      v-if="current"
      class="fixed bottom-8 left-0 right-0 z-[150] h-16 bg-shell border-t border-line flex items-center gap-2 px-4"
      :style="{ boxShadow: 'var(--glow)' }"
    >
      <!-- Línea-aura superior: firma cromática del tema (gradiente en
           camaleón/tinta, invisible en consola). 2px, discreta. -->
      <div aria-hidden="true" class="absolute top-0 left-0 right-0 h-[2px] pointer-events-none" :style="{ background: 'var(--aura)' }"></div>
      <!-- Error de reproducción (fallo de avance de cola, archivo ilegible…):
           role=alert implica aria-live=assertive para que los lectores de
           pantalla lo anuncien. Flota sobre la barra sin romper su altura. -->
      <p
        v-if="error"
        data-testid="player-error"
        role="alert"
        class="absolute -top-9 left-4 max-w-[70ch] truncate rounded border border-error/40 bg-shell px-3 py-1.5 text-xs text-error shadow-lg"
      >{{ error }}</p>
      <TrackCover
        :src="current.coverUrl"
        :item-id="current.id"
        :alt="current.album ?? current.title"
        size-class="w-10 h-10"
      />

      <div class="min-w-0 w-44 shrink-0">
        <p class="text-sm text-ink truncate">{{ current.title }}</p>
        <p class="text-xs text-text-secondary truncate">{{ current.artist }}</p>
      </div>

      <!-- Transporte -->
      <button data-testid="player-previous" @click="previous" class="p-2 rounded-full bg-surface hover:bg-elevated transition-colors" title="Anterior" aria-label="Pista anterior">
        <span class="material-symbols-outlined text-ink">skip_previous</span>
      </button>
      <button @click="toggle" class="p-2 rounded-full bg-surface hover:bg-elevated transition-colors" :title="isPlaying ? 'Pausa' : 'Reproducir'">
        <span class="material-symbols-outlined text-ink">{{ isPlaying ? 'pause' : 'play_arrow' }}</span>
      </button>
      <button data-testid="player-next" @click="next" class="p-2 rounded-full bg-surface hover:bg-elevated transition-colors" title="Siguiente" aria-label="Pista siguiente">
        <span class="material-symbols-outlined text-ink">skip_next</span>
      </button>

      <span class="text-xs text-text-secondary font-mono shrink-0 tabular-nums">{{ formatTime(positionSec) }}</span>
      <!-- Progreso: pista visual con relleno var(--prog) (gradiente camaleón,
           sólido en el resto de temas). El range nativo va encima, transparente,
           y conserva el seek con ratón, teclado y lectores de pantalla. -->
      <div class="relative flex-1 h-4 flex items-center">
        <div aria-hidden="true" class="absolute inset-x-0 top-1/2 -translate-y-1/2 h-1 rounded-full bg-surface overflow-hidden">
          <div class="h-full rounded-full" :style="{ width: seekPercent + '%', background: 'var(--prog)' }"></div>
        </div>
        <input
          data-testid="player-seek"
          type="range"
          min="0"
          :max="durationSec || 0"
          step="0.1"
          :value="positionSec"
          @input="seek(Number(($event.target as HTMLInputElement).value))"
          class="player-seek relative z-10 w-full h-4"
          :class="!durationSec ? 'cursor-default' : 'cursor-pointer'"
          :disabled="!durationSec"
          aria-label="Posición de la pista"
          :aria-valuetext="`${formatTime(positionSec)} de ${formatTime(durationSec)}`"
        >
      </div>
      <span class="text-xs text-text-secondary font-mono shrink-0 tabular-nums">{{ formatTime(durationSec) }}</span>

      <!-- Modos persistentes -->
      <button
        data-testid="player-shuffle"
        @click="toggleShuffle"
        class="p-2 rounded-full hover:bg-elevated transition-colors"
        :class="shuffle ? 'text-accent' : 'text-ink-2'"
        :aria-pressed="shuffle"
        title="Aleatorio"
      >
        <span class="material-symbols-outlined">shuffle</span>
      </button>
      <button
        data-testid="player-repeat"
        @click="cycleRepeat"
        class="p-2 rounded-full hover:bg-elevated transition-colors"
        :class="repeat !== 'off' ? 'text-accent' : 'text-ink-2'"
        :aria-pressed="repeat !== 'off'"
        :title="repeatTitle"
      >
        <span class="material-symbols-outlined">{{ repeat === 'one' ? 'repeat_one' : 'repeat' }}</span>
      </button>

      <!-- Volumen / mute independientes -->
      <button
        data-testid="player-mute"
        @click="setMuted(!muted)"
        class="p-2 rounded-full hover:bg-elevated transition-colors"
        :title="muted ? 'Activar sonido' : 'Silenciar'"
        :aria-pressed="muted"
      >
        <span class="material-symbols-outlined text-ink-2">{{ volumeIcon }}</span>
      </button>
      <input
        data-testid="player-volume"
        type="range"
        min="0"
        max="1"
        step="0.01"
        :value="volume"
        @input="setVolume(Number(($event.target as HTMLInputElement).value))"
        class="w-16 accent-accent h-1 cursor-pointer"
        aria-label="Volumen"
      >

      <!-- Cola de reproducción -->
      <button
        data-testid="player-queue-toggle"
        @click="queueOpen = !queueOpen"
        class="relative p-2 rounded-full hover:bg-elevated transition-colors"
        :title="queueOpen ? 'Cerrar cola' : 'Ver cola'"
        :aria-expanded="queueOpen"
      >
        <span class="material-symbols-outlined text-ink-2">queue_music</span>
        <span
          v-if="queue.length"
          data-testid="player-queue-count"
          class="absolute -top-0.5 -right-0.5 min-w-[16px] h-4 px-0.5 rounded-full bg-accent text-on-accent text-[10px] leading-4 text-center tabular-nums"
        >{{ queue.length }}</span>
      </button>

      <button @click="stop" class="p-2 rounded-full hover:bg-elevated transition-colors" title="Detener">
        <span class="material-symbols-outlined text-ink-2">stop</span>
      </button>
    </div>
  </Transition>

  <!-- Panel de cola: reordenación por drag & drop y por botones accesibles -->
  <Transition name="player-slide">
    <div
      v-if="queueOpen && current"
      data-testid="player-queue-panel"
      role="region"
      aria-label="Cola de reproducción"
      class="fixed bottom-24 right-4 z-[150] w-96 max-h-96 bg-shell border border-line rounded-lg shadow-2xl flex flex-col overflow-hidden"
    >
      <div class="flex items-center justify-between px-3 py-2 border-b border-line shrink-0">
        <p class="text-xs font-semibold text-ink">Cola de reproducción</p>
        <button
          data-testid="player-queue-clear"
          @click="clearQueue"
          :disabled="!queue.length"
          class="text-xs text-ink-2 hover:text-ink disabled:opacity-40 disabled:hover:text-ink-2 transition-colors"
        >Vaciar</button>
      </div>
      <p v-if="!queue.length" data-testid="player-queue-empty" class="px-3 py-6 text-center text-xs text-text-secondary">La cola está vacía</p>
      <ul v-else class="overflow-y-auto flex-1 py-1">
        <li
          v-for="(entry, index) in queue"
          :key="entry.entryId"
          data-testid="player-queue-item"
          :draggable="true"
          class="group flex items-center gap-2 px-3 py-1.5 hover:bg-elevated"
          @dragstart="onDragStart(entry.entryId)"
          @dragover.prevent
          @drop="onDrop(index)"
        >
          <span class="text-[10px] text-ink-3 w-4 text-right tabular-nums shrink-0">{{ index + 1 }}</span>
          <TrackCover
            :src="entry.track?.coverUrl ?? null"
            :item-id="entry.trackId"
            :alt="entryTitle(entry)"
            size-class="w-8 h-8 shrink-0"
          />
          <div class="min-w-0 flex-1">
            <p class="text-xs text-ink truncate">{{ entryTitle(entry) }}</p>
            <p class="text-[10px] text-text-secondary truncate">{{ entry.track?.artist ?? 'Pendiente de validar' }}</p>
          </div>
          <div class="flex items-center opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity shrink-0">
            <button
              :data-testid="`queue-up-${index}`"
              :disabled="index === 0"
              :aria-label="`Subir ${entryTitle(entry)}`"
              @click="moveEntry(entry.entryId, index - 1)"
              class="p-0.5 text-ink-2 hover:text-ink disabled:opacity-30"
            ><span class="material-symbols-outlined text-sm">keyboard_arrow_up</span></button>
            <button
              :data-testid="`queue-down-${index}`"
              :disabled="index === queue.length - 1"
              :aria-label="`Bajar ${entryTitle(entry)}`"
              @click="moveEntry(entry.entryId, index + 1)"
              class="p-0.5 text-ink-2 hover:text-ink disabled:opacity-30"
            ><span class="material-symbols-outlined text-sm">keyboard_arrow_down</span></button>
            <button
              :data-testid="`queue-remove-${index}`"
              :aria-label="`Quitar ${entryTitle(entry)}`"
              @click="removeEntry(entry.entryId)"
              class="p-0.5 text-ink-2 hover:text-error"
            ><span class="material-symbols-outlined text-sm">close</span></button>
          </div>
        </li>
      </ul>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, toValue } from 'vue'
import { usePlayer, type PlaybackQueueEntry } from '../composables/usePlayer'
import TrackCover from './TrackCover.vue'

const {
  current, isPlaying, positionSec, durationSec, error, toggle, stop, seek,
  queue, next, previous,
  shuffle, toggleShuffle, repeat, cycleRepeat,
  volume, muted, setVolume, setMuted,
  clearQueue, removeEntry, moveEntry,
  restoreFromPersistence,
} = usePlayer()

const queueOpen = ref(false)
let dragEntryId: string | null = null

// Restauración perezosa del estado persistido: al montar la barra global (una
// sola vez por arranque, con guard interno). Importar usePlayer no restaura ni
// reproduce nada; la restauración queda pausada.
onMounted(() => { void restoreFromPersistence() })

const repeatTitle = computed(() => {
  const r = toValue(repeat)
  return r === 'one' ? 'Repetir una pista (activo)'
    : r === 'all' ? 'Repetir todo (activo)'
      : 'Repetir: desactivado'
})

// Porcentaje reproducido para el relleno visual de la barra (var(--prog)).
// toValue por la misma razón que arriba: los specs inyectan valores crudos.
const seekPercent = computed(() => {
  const total = toValue(durationSec)
  const pos = toValue(positionSec)
  if (!Number.isFinite(total) || total <= 0) return 0
  return Math.min(100, Math.max(0, (pos / total) * 100))
})

const volumeIcon = computed(() => {
  const mv = toValue(muted)
  const vv = toValue(volume)
  return mv ? 'volume_off'
    : vv === 0 ? 'volume_mute'
      : vv < 0.5 ? 'volume_down'
        : 'volume_up'
})

function formatTime(sec: number): string {
  if (!Number.isFinite(sec) || sec <= 0) return '0:00'
  const m = Math.floor(sec / 60)
  const s = Math.floor(sec % 60)
  return `${m}:${s.toString().padStart(2, '0')}`
}

function entryTitle(entry: PlaybackQueueEntry): string {
  return entry.track?.title ?? `Pista #${entry.trackId}`
}

function onDragStart(entryId: string): void {
  dragEntryId = entryId
}

function onDrop(index: number): void {
  if (dragEntryId) moveEntry(dragEntryId, index)
  dragEntryId = null
}
</script>

<style scoped>
.player-slide-enter-active,
.player-slide-leave-active {
  transition: transform 0.25s ease, opacity 0.25s ease;
}
.player-slide-enter-from,
.player-slide-leave-to {
  transform: translateY(100%);
  opacity: 0;
}

/* Slider de progreso transparente: la pista visible es el div inferior
   (relleno var(--prog)); el pulgar nativo solo aparece al hover/focus. */
.player-seek {
  -webkit-appearance: none;
  appearance: none;
  background: transparent;
  outline: none;
}
.player-seek::-webkit-slider-runnable-track {
  height: 4px;
  background: transparent;
}
.player-seek::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 10px;
  height: 10px;
  margin-top: -3px; /* centra el pulgar de 10px sobre la pista de 4px */
  border: none;
  border-radius: 9999px;
  background: var(--text-1);
  box-shadow: var(--shadow-card);
  opacity: 0;
  transition: opacity 0.15s ease;
}
.player-seek:hover::-webkit-slider-thumb,
.player-seek:focus-visible::-webkit-slider-thumb {
  opacity: 1;
}
.player-seek:disabled::-webkit-slider-thumb {
  opacity: 0;
}
.player-seek::-moz-range-track {
  height: 4px;
  background: transparent;
}
.player-seek::-moz-range-thumb {
  width: 10px;
  height: 10px;
  border: none;
  border-radius: 9999px;
  background: var(--text-1);
  box-shadow: var(--shadow-card);
  opacity: 0;
  transition: opacity 0.15s ease;
}
.player-seek:hover::-moz-range-thumb,
.player-seek:focus-visible::-moz-range-thumb {
  opacity: 1;
}
.player-seek:disabled::-moz-range-thumb {
  opacity: 0;
}
</style>
