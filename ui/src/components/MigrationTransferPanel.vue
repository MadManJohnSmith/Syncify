<template>
  <div class="transfer-panel">
    <!-- Not Started / Ready State -->
    <div v-if="!started" class="text-center py-12">
      <div class="w-24 h-24 mx-auto rounded-full bg-primary/10 flex items-center justify-center mb-6">
        <span class="material-symbols-outlined text-5xl text-primary">cloud_sync</span>
      </div>
      <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">{{ readyTitle }}</h2>
      <p class="text-text-secondary mb-4 max-w-md mx-auto">{{ readyDescription }}</p>
      <p v-if="readyHint" class="text-xs text-text-secondary mb-6">{{ readyHint }}</p>

      <!-- Start failure (backend refused the migration) -->
      <div v-if="error" data-testid="transfer-error" class="mb-6 mx-auto max-w-md rounded-lg border border-error/30 bg-error/5 p-4 text-sm text-error">
        {{ error }}
      </div>

      <button @click="$emit('start')" data-testid="start-transfer-btn" :disabled="starting" class="px-8 py-4 bg-primary hover:bg-primary-hover text-white rounded-xl text-lg font-semibold transition-colors shadow-lg shadow-primary/30 disabled:opacity-50 disabled:cursor-not-allowed">
        <span class="flex items-center gap-3">
          <span class="material-symbols-outlined text-[24px]">play_arrow</span>
          {{ startLabel }}
        </span>
      </button>
    </div>

    <!-- Transfer In Progress -->
    <div v-else-if="!complete" class="transfer-progress" data-testid="transfer-progress">
      <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Transferring...</h2>
      <p class="text-text-secondary mb-6">Please keep this window open until complete</p>

      <!-- Large Progress Bar -->
      <div class="mb-6">
        <div class="flex items-center justify-between mb-2">
          <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{{ progress.current }} / {{ progress.total }} tracks</span>
          <span class="text-sm font-bold text-primary">{{ progress.percent }}%</span>
        </div>
        <div class="h-4 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div class="h-full bg-gradient-to-r from-primary to-blue-400 rounded-full transition-all duration-300 relative" :style="{ width: progress.percent + '%' }">
            <div class="absolute inset-0 bg-white/20 animate-shine"></div>
          </div>
        </div>
        <div class="flex items-center justify-between mt-2 text-xs text-text-secondary">
          <span>~{{ progress.eta }} remaining</span>
          <span>{{ progress.speed }} tracks/min</span>
        </div>
      </div>

      <!-- Current Action -->
      <div class="flex items-center gap-3 mb-6 px-4 py-3 bg-primary/5 border border-primary/20 rounded-lg">
        <span class="material-symbols-outlined text-primary animate-spin text-xl">sync</span>
        <span class="text-sm text-gray-700 dark:text-gray-300">{{ progress.currentAction }}</span>
      </div>

      <!-- Activity Log (real entries, from migration-progress events) -->
      <div class="activity-log mb-6">
        <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-2">Activity Log</h4>
        <div v-if="activityLog.length === 0" class="text-xs text-text-secondary py-2">Waiting for progress updates from the migration engine...</div>
        <div v-else class="bg-gray-900 rounded-lg p-3 h-[160px] overflow-y-auto custom-scrollbar font-mono text-xs">
          <div
            v-for="log in activityLog"
            :key="log.id"
            class="log-entry flex items-start gap-2 py-1"
          >
            <span class="text-gray-500 shrink-0">{{ log.time }}</span>
            <span :class="log.status === 'ok' ? 'text-green-400' : log.status === 'failed' ? 'text-red-400' : 'text-amber-400'">{{ log.status === 'ok' ? '✓' : log.status === 'failed' ? '✗' : '−' }}</span>
            <span class="text-gray-300">{{ log.message }}</span>
          </div>
        </div>
      </div>

      <!-- Status Summary -->
      <div class="grid grid-cols-3 gap-4 mb-6">
        <div class="text-center">
          <p class="text-2xl font-bold text-success">{{ progress.transferred }}</p>
          <p class="text-xs text-text-secondary">Transferred</p>
        </div>
        <div class="text-center">
          <p class="text-2xl font-bold text-error">{{ progress.failed }}</p>
          <p class="text-xs text-text-secondary">Failed</p>
        </div>
        <div class="text-center">
          <p class="text-2xl font-bold text-gray-400">{{ progress.skipped }}</p>
          <p class="text-xs text-text-secondary">Skipped</p>
        </div>
      </div>

      <!-- Cancel Button -->
      <div class="text-center">
        <button @click="$emit('cancel')" class="px-5 py-2.5 bg-error/10 border border-error/30 hover:bg-error/20 text-error rounded-lg text-sm font-medium transition-colors">
          Cancel Transfer
        </button>
      </div>
    </div>

    <!-- Transfer Complete -->
    <div v-else class="transfer-complete text-center py-8 relative" data-testid="transfer-complete">
      <!-- Confetti Container -->
      <div class="confetti-container absolute inset-0 pointer-events-none overflow-hidden">
        <div v-for="i in 20" :key="i" :class="['confetti', `confetti-${i}`]"></div>
      </div>

      <div class="w-24 h-24 mx-auto rounded-full bg-success/10 flex items-center justify-center mb-6 relative">
        <span class="material-symbols-outlined text-5xl text-success">celebration</span>
      </div>
      <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Transfer complete! 🎉</h2>
      <p class="text-text-secondary mb-8">{{ progress.failed > 0 ? 'The migration finished, but some tracks could not be transferred' : 'Your music has been migrated successfully' }}</p>

      <!-- Summary (real final counts) -->
      <div class="bg-gray-50 dark:bg-surface-highlight/50 rounded-xl p-6 mb-8 text-left max-w-md mx-auto">
        <div class="flex items-center justify-between py-2 border-b border-gray-200 dark:border-border-dark">
          <span class="text-sm text-gray-700 dark:text-gray-300">Successfully transferred</span>
          <span class="text-sm font-bold text-success" data-testid="complete-transferred">{{ progress.transferred }} tracks</span>
        </div>
        <div class="flex items-center justify-between py-2 border-b border-gray-200 dark:border-border-dark">
          <span class="text-sm text-gray-700 dark:text-gray-300">Failed</span>
          <button v-if="progress.failed > 0" @click="$emit('view-details', true)" data-testid="view-failed-details" class="text-sm font-bold text-error hover:underline">{{ progress.failed }} tracks (view details)</button>
          <span v-else class="text-sm font-bold text-success">0 tracks</span>
        </div>
        <div class="flex items-center justify-between py-2">
          <span class="text-sm text-gray-700 dark:text-gray-300">Skipped</span>
          <span class="text-sm font-bold text-gray-500">{{ progress.skipped }} tracks</span>
        </div>
      </div>

      <!-- Action Buttons -->
      <div class="flex items-center justify-center gap-4">
        <button @click="$emit('view-details', false)" class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
          View Failed Tracks
        </button>
        <button @click="$emit('reset')" class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
          New Migration
        </button>
        <button @click="$emit('reset')" class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
          Done
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Shared transfer screen for both assistant modes (4.2): the transfer wizard
 * and the service → service mode render the same real progress, activity log
 * and completion summary, driven entirely by migration-progress events.
 */
import type { PropType } from 'vue'

/** One activity-log row derived from a migration-progress event. */
export interface MigrationActivityEntry {
  id: string
  time: string
  status: 'ok' | 'failed' | 'skipped'
  message: string
}

/** Display shape of the real progress state (computed from the event payload). */
export interface MigrationProgressDisplay {
  current: number
  total: number
  percent: number
  transferred: number
  failed: number
  skipped: number
  currentAction: string
  eta: string
  speed: string
}

defineProps({
  started: { type: Boolean, required: true },
  complete: { type: Boolean, required: true },
  starting: { type: Boolean, required: true },
  error: { type: String as PropType<string | null>, default: null },
  readyTitle: { type: String, default: 'Ready to transfer' },
  readyDescription: { type: String, required: true },
  readyHint: { type: String, default: '' },
  startLabel: { type: String, default: 'Start Transfer' },
  progress: { type: Object as PropType<MigrationProgressDisplay>, required: true },
  activityLog: { type: Array as PropType<MigrationActivityEntry[]>, required: true },
})

defineEmits<{
  (e: 'start'): void
  (e: 'cancel'): void
  (e: 'view-details', expandFailed: boolean): void
  (e: 'reset'): void
}>()
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(128, 128, 128, 0.3);
  border-radius: 4px;
}

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(128, 128, 128, 0.5);
}

/* Shine animation for progress bar */
@keyframes shine {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(200%); }
}

.animate-shine {
  animation: shine 2s ease-in-out infinite;
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
}

/* Confetti animation */
.confetti-container {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.confetti {
  position: absolute;
  width: 10px;
  height: 10px;
  top: -10px;
  animation: confetti-fall 3s ease-out forwards;
}

@keyframes confetti-fall {
  0% {
    transform: translateY(0) rotate(0deg);
    opacity: 1;
  }
  100% {
    transform: translateY(400px) rotate(720deg);
    opacity: 0;
  }
}

.confetti-1 { left: 5%; background: #3b82f6; animation-delay: 0s; }
.confetti-2 { left: 10%; background: #10b981; animation-delay: 0.1s; }
.confetti-3 { left: 15%; background: #f59e0b; animation-delay: 0.2s; }
.confetti-4 { left: 20%; background: #ef4444; animation-delay: 0.3s; }
.confetti-5 { left: 25%; background: #8b5cf6; animation-delay: 0.4s; }
.confetti-6 { left: 30%; background: #ec4899; animation-delay: 0.5s; }
.confetti-7 { left: 35%; background: #3b82f6; animation-delay: 0.1s; }
.confetti-8 { left: 40%; background: #10b981; animation-delay: 0.2s; }
.confetti-9 { left: 45%; background: #f59e0b; animation-delay: 0.3s; }
.confetti-10 { left: 50%; background: #ef4444; animation-delay: 0.4s; }
.confetti-11 { left: 55%; background: #8b5cf6; animation-delay: 0s; }
.confetti-12 { left: 60%; background: #ec4899; animation-delay: 0.1s; }
.confetti-13 { left: 65%; background: #3b82f6; animation-delay: 0.2s; }
.confetti-14 { left: 70%; background: #10b981; animation-delay: 0.3s; }
.confetti-15 { left: 75%; background: #f59e0b; animation-delay: 0.4s; }
.confetti-16 { left: 80%; background: #ef4444; animation-delay: 0.5s; }
.confetti-17 { left: 85%; background: #8b5cf6; animation-delay: 0s; }
.confetti-18 { left: 90%; background: #ef4444; animation-delay: 0.1s; }
.confetti-19 { left: 92%; background: #3b82f6; animation-delay: 0.2s; }
.confetti-20 { left: 95%; background: #10b981; animation-delay: 0.3s; }
</style>
