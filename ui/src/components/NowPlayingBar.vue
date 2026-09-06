<template>
  <!-- S194 residual: minimal now-playing bar for local FLAC playback -->
  <Transition name="player-slide">
    <div v-if="current" class="fixed bottom-8 left-0 right-0 z-[150] h-16 bg-[#101723] border-t border-border-dark flex items-center gap-4 px-4 shadow-2xl">
      <div class="w-10 h-10 rounded-md bg-surface-dark shrink-0 overflow-hidden flex items-center justify-center">
        <img v-if="current.coverUrl" :src="current.coverUrl" :alt="current.album ?? current.title" class="w-full h-full object-cover">
        <span v-else class="material-symbols-outlined text-gray-500">music_note</span>
      </div>

      <div class="min-w-0 w-52 shrink-0">
        <p class="text-sm text-white truncate">{{ current.title }}</p>
        <p class="text-xs text-text-secondary truncate">{{ current.artist }}</p>
      </div>

      <button @click="toggle" class="p-2 rounded-full bg-white/5 hover:bg-white/10 transition-colors" :title="isPlaying ? 'Pausa' : 'Reproducir'">
        <span class="material-symbols-outlined text-white">{{ isPlaying ? 'pause' : 'play_arrow' }}</span>
      </button>

      <span class="text-xs text-text-secondary font-mono shrink-0 tabular-nums">{{ formatTime(positionSec) }}</span>
      <input
        type="range"
        min="0"
        :max="durationSec || 0"
        step="0.1"
        :value="positionSec"
        @input="seek(Number(($event.target as HTMLInputElement).value))"
        class="flex-1 accent-primary h-1 cursor-pointer"
        :disabled="!durationSec"
      >
      <span class="text-xs text-text-secondary font-mono shrink-0 tabular-nums">{{ formatTime(durationSec) }}</span>

      <button @click="stop" class="p-2 rounded-full hover:bg-white/10 transition-colors" title="Detener">
        <span class="material-symbols-outlined text-gray-400">stop</span>
      </button>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { usePlayer } from '../composables/usePlayer'

const { current, isPlaying, positionSec, durationSec, toggle, stop, seek } = usePlayer()

function formatTime(sec: number): string {
  if (!Number.isFinite(sec) || sec <= 0) return '0:00'
  const m = Math.floor(sec / 60)
  const s = Math.floor(sec % 60)
  return `${m}:${s.toString().padStart(2, '0')}`
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
</style>
