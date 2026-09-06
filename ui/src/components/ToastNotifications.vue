<template>
  <div class="toast-system pointer-events-none fixed top-4 right-4 z-[100] flex flex-col gap-3">
    <!-- Toast Container -->
    <TransitionGroup name="toast">
      <div 
        v-for="toast in visibleToasts" 
        :key="toast.id"
        :class="[
          'toast pointer-events-auto w-80 rounded-lg shadow-xl overflow-hidden',
          `toast-${toast.type}`
        ]"
        @mouseenter="pauseToast(toast.id)"
        @mouseleave="resumeToast(toast.id)"
        @click="handleToastClick(toast, $event)"
      >
        <div class="flex items-start gap-3 p-3">
          <!-- Icon -->
          <div class="toast-icon shrink-0 mt-0.5">
            <span v-if="toast.type === 'success'" class="material-symbols-outlined text-white">check_circle</span>
            <span v-else-if="toast.type === 'error'" class="material-symbols-outlined text-white">error</span>
            <span v-else-if="toast.type === 'warning'" class="material-symbols-outlined text-white">warning</span>
            <span v-else-if="toast.type === 'info'" class="material-symbols-outlined text-white">info</span>
            <span v-else-if="toast.type === 'progress'" class="material-symbols-outlined text-white animate-spin">sync</span>
          </div>
          
          <!-- Message -->
          <div class="toast-message flex-1 min-w-0">
            <p class="text-sm font-semibold text-white">{{ toast.title }}</p>
            <p v-if="toast.description" class="text-xs text-white/70 mt-0.5">{{ toast.description }}</p>
            
            <!-- Progress Bar (for progress type) -->
            <div v-if="toast.type === 'progress' && toast.progress !== undefined" class="mt-2">
              <div class="toast-progress h-1.5 bg-white/20 rounded-full overflow-hidden">
                <div 
                  class="h-full bg-white rounded-full transition-all duration-300"
                  :style="{ width: toast.progress + '%' }"
                ></div>
              </div>
              <div class="flex justify-between mt-1">
                <span class="text-[10px] text-white/60">{{ toast.progress }}%</span>
                <span v-if="toast.timeRemaining" class="text-[10px] text-white/60">{{ toast.timeRemaining }}</span>
              </div>
            </div>
          </div>
          
          <!-- Close Button -->
          <button 
            @click.stop="dismissToast(toast.id)" 
            class="shrink-0 p-1 hover:bg-white/20 rounded transition-colors"
          >
            <span class="material-symbols-outlined text-white/70 text-[18px]">close</span>
          </button>
        </div>
        
        <!-- Action Buttons -->
        <div v-if="toast.actions && toast.actions.length > 0" class="toast-actions px-3 pb-3 flex gap-2">
          <button 
            v-for="action in toast.actions" 
            :key="action.label"
            @click.stop="handleAction(toast, action)"
            :class="[
              'px-3 py-1.5 rounded text-xs font-medium transition-colors',
              action.primary 
                ? 'bg-white text-gray-900 hover:bg-gray-100' 
                : 'border border-white/30 text-white hover:bg-white/10'
            ]"
          >
            {{ action.label }}
          </button>
        </div>
        
        <!-- Auto-dismiss Progress Bar -->
        <div 
          v-if="toast.autoDismiss && toast.duration && toast.type !== 'progress'"
          class="h-0.5 bg-white/30"
        >
          <div 
            class="h-full bg-white transition-all ease-linear"
            :style="{ 
              width: getTimerProgress(toast) + '%',
              transitionDuration: toast.paused ? '0ms' : '100ms'
            }"
          ></div>
        </div>
      </div>
    </TransitionGroup>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useToast, type Toast, type ToastAction } from '@/composables/useToast'

const { toasts, dismiss, pauseToast, resumeToast } = useToast()

const visibleToasts = computed(() => toasts.value.slice(0, 5))

function dismissToast(id: string) {
  dismiss(id)
}

function getTimerProgress(toast: { autoDismiss?: boolean; duration?: number; paused?: boolean; timerRemaining?: number; createdAt: number }): number {
  if (!toast.autoDismiss || !toast.duration) return 100
  if (toast.paused && toast.timerRemaining !== undefined) {
    return Math.max(0, (toast.timerRemaining / toast.duration) * 100)
  }
  const elapsed = Date.now() - toast.createdAt
  return Math.max(0, 100 - (elapsed / toast.duration) * 100)
}

function handleToastClick(toast: { id: string }, event: MouseEvent) {
  if (!(event.target as HTMLElement).closest('button')) {
    dismissToast(toast.id)
  }
}

function handleAction(toast: { id: string }, action: { handler: () => void }) {
  action.handler()
  dismissToast(toast.id)
}
</script>

<style scoped>
/* Toast Types */
.toast-success {
  background: linear-gradient(135deg, #10b981, #059669);
}

.toast-error {
  background: linear-gradient(135deg, #ef4444, #dc2626);
}

.toast-warning {
  background: linear-gradient(135deg, #f59e0b, #d97706);
}

.toast-info {
  background: linear-gradient(135deg, #3b82f6, #2563eb);
}

.toast-progress {
  background: linear-gradient(135deg, #6366f1, #4f46e5);
}

/* Toast Animations */
.toast-enter-active,
.toast-leave-active {
  transition: all 0.2s ease;
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(100%);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(100%);
}

.toast-move {
  transition: transform 0.15s ease;
}

/* Spin Animation */
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 1s linear infinite;
}
</style>
