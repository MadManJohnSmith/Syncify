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

  <!-- Diálogos internos: sustituyen a los alert/confirm/message nativos -->
  <Teleport to="body">
    <Transition name="dialog-overlay">
      <div
        v-if="activeDialog"
        class="dialog-overlay fixed inset-0 z-[300] flex items-center justify-center p-4"
        @click.self="cancelDialog"
        @keydown.esc="cancelDialog"
      >
        <div
          class="dialog-card w-full max-w-md rounded-2xl shadow-2xl overflow-hidden bg-surface text-ink"
          role="alertdialog"
          aria-modal="true"
          :aria-label="activeDialog.title"
        >
          <div class="flex items-start gap-3 p-5">
            <span
              class="material-symbols-outlined text-2xl shrink-0"
              :class="dialogIconClass"
              aria-hidden="true"
            >{{ dialogIcon }}</span>
            <div class="min-w-0 flex-1">
              <p class="text-base font-semibold">{{ activeDialog.title }}</p>
              <p v-if="activeDialog.message" class="text-sm text-ink-2 mt-1 whitespace-pre-line">
                {{ activeDialog.message }}
              </p>
            </div>
          </div>

          <div v-if="activeDialog.kind === 'prompt'" class="px-5 pb-4">
            <input
              ref="dialogInput"
              v-model="dialogValue"
              type="text"
              :placeholder="activeDialog.placeholder"
              class="w-full px-3 py-2 bg-elevated rounded-lg text-ink focus:outline-none focus:ring-2 focus:ring-accent/50"
              @keydown.enter.prevent="acceptDialog"
            >
          </div>

          <div class="px-5 py-4 border-t border-line flex justify-end gap-3">
            <button
              v-if="activeDialog.kind !== 'alert'"
              class="dialog-cancel px-4 py-2 rounded-lg text-sm font-medium border border-line-strong hover:bg-elevated transition-colors"
              @click="cancelDialog"
            >
              {{ activeDialog.cancelLabel }}
            </button>
            <button
              class="dialog-confirm px-4 py-2 rounded-lg text-sm font-medium text-white transition-colors"
              :class="dialogConfirmClass"
              @click="acceptDialog"
            >
              {{ activeDialog.confirmLabel }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useToast, useDialog, type Toast, type ToastAction } from '@/composables/useToast'

const { toasts, dismiss, pauseToast, resumeToast } = useToast()
const { activeDialog, dialogValue, resolveDialog } = useDialog()

const visibleToasts = computed(() => toasts.value.slice(0, 5))

const dialogInput = ref<HTMLInputElement | null>(null)

const dialogIcon = computed(() => {
    if (activeDialog.value?.variant === 'danger') return 'error'
    if (activeDialog.value?.variant === 'warning') return 'warning'
    return 'info'
})

const dialogIconClass = computed(() => {
    if (activeDialog.value?.variant === 'danger') return 'text-error'
    if (activeDialog.value?.variant === 'warning') return 'text-warn'
    return 'text-accent'
})

const dialogConfirmClass = computed(() => {
    if (activeDialog.value?.variant === 'danger') return 'bg-error hover:bg-error/80'
    return 'bg-accent hover:opacity-90'
})

watch(activeDialog, (dialog) => {
    if (!dialog) return
    nextTick(() => dialogInput.value?.focus())
})

function acceptDialog() {
    const dialog = activeDialog.value
    if (!dialog) return
    resolveDialog(dialog.kind === 'prompt' ? dialogValue.value : 'confirm')
}

function cancelDialog() {
    resolveDialog(null)
}

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
/* Toast Types: color semántico + un paso más oscuro al final del degradado
   (equivalente al -500 → -600 anterior, ahora en tokens de tema). */
.toast-success {
  background: linear-gradient(135deg, var(--ok), color-mix(in srgb, var(--ok) 70%, black));
}

.toast-error {
  background: linear-gradient(135deg, var(--error), color-mix(in srgb, var(--error) 80%, black));
}

.toast-warning {
  background: linear-gradient(135deg, var(--warn), color-mix(in srgb, var(--warn) 80%, black));
}

.toast-info {
  background: linear-gradient(135deg, var(--info), color-mix(in srgb, var(--info) 80%, black));
}

.toast-progress {
  background: linear-gradient(135deg, var(--accent), color-mix(in srgb, var(--accent) 80%, black));
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

/* Dialogs */
.dialog-overlay {
  background: rgba(0, 0, 0, 0.55);
}

.dialog-overlay-enter-active,
.dialog-overlay-leave-active {
  transition: opacity 0.15s ease;
}

.dialog-overlay-enter-active .dialog-card,
.dialog-overlay-leave-active .dialog-card {
  transition: transform 0.15s ease, opacity 0.15s ease;
}

.dialog-overlay-enter-from,
.dialog-overlay-leave-to {
  opacity: 0;
}

.dialog-overlay-enter-from .dialog-card,
.dialog-overlay-leave-to .dialog-card {
  transform: scale(0.96) translateY(8px);
  opacity: 0;
}
</style>
