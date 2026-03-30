<template>
  <div class="notification-system">
    <!-- Toast Container -->
    <div class="toast-container fixed top-4 right-4 z-[100] flex flex-col gap-3 pointer-events-none">
      <TransitionGroup name="toast">
        <div 
          v-for="toast in visibleToasts" 
          :key="toast.id"
          :class="[
            'toast pointer-events-auto w-80 rounded-lg shadow-xl overflow-hidden',
            `toast-${toast.type}`
          ]"
          @mouseenter="pauseTimer(toast.id)"
          @mouseleave="resumeTimer(toast.id)"
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
    
    <!-- Notification Bell (for history panel) -->
    <div class="notification-bell relative">
      <button 
        @click="showHistoryPanel = !showHistoryPanel"
        class="p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors relative"
      >
        <span class="material-symbols-outlined text-gray-600 dark:text-gray-300">notifications</span>
        <span 
          v-if="unreadCount > 0" 
          class="absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] bg-error text-white text-[10px] font-bold rounded-full flex items-center justify-center px-1"
        >
          {{ unreadCount > 99 ? '99+' : unreadCount }}
        </span>
      </button>
      
      <!-- History Panel -->
      <Transition name="dropdown">
        <div 
          v-if="showHistoryPanel" 
          class="notification-history absolute top-full right-0 mt-2 w-96 bg-white dark:bg-surface-dark rounded-xl shadow-2xl border border-gray-200 dark:border-border-dark overflow-hidden z-50"
        >
          <!-- Header -->
          <div class="px-4 py-3 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
            <h4 class="font-semibold text-gray-900 dark:text-white">Notifications</h4>
            <div class="flex gap-1">
              <button 
                :class="[
                  'px-3 py-1 rounded-lg text-xs font-medium transition-colors',
                  historyFilter === 'all' ? 'bg-primary/10 text-primary' : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight'
                ]"
                @click="historyFilter = 'all'"
              >
                All
              </button>
              <button 
                :class="[
                  'px-3 py-1 rounded-lg text-xs font-medium transition-colors',
                  historyFilter === 'unread' ? 'bg-primary/10 text-primary' : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight'
                ]"
                @click="historyFilter = 'unread'"
              >
                Unread
              </button>
            </div>
          </div>
          
          <!-- Notifications List -->
          <div class="max-h-96 overflow-y-auto custom-scrollbar">
            <div v-if="filteredHistory.length === 0" class="p-8 text-center">
              <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-2 block">notifications_off</span>
              <p class="text-sm text-gray-500">No notifications</p>
            </div>
            
            <div 
              v-for="notification in filteredHistory" 
              :key="notification.id"
              :class="[
                'px-4 py-3 border-b border-gray-100 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors cursor-pointer',
                !notification.read && 'bg-primary/5'
              ]"
              @click="markAsRead(notification.id)"
            >
              <div class="flex items-start gap-3">
                <div :class="[
                  'w-8 h-8 rounded-lg flex items-center justify-center shrink-0',
                  notification.type === 'success' ? 'bg-success/20 text-success' :
                  notification.type === 'error' ? 'bg-error/20 text-error' :
                  notification.type === 'warning' ? 'bg-amber-500/20 text-amber-500' :
                  'bg-blue-500/20 text-blue-500'
                ]">
                  <span class="material-symbols-outlined text-[18px]">
                    {{ notification.type === 'success' ? 'check_circle' : 
                       notification.type === 'error' ? 'error' : 
                       notification.type === 'warning' ? 'warning' : 'info' }}
                  </span>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm text-gray-900 dark:text-white font-medium">{{ notification.title }}</p>
                  <p v-if="notification.description" class="text-xs text-gray-500 mt-0.5 truncate">{{ notification.description }}</p>
                  <p class="text-[10px] text-gray-400 mt-1">{{ notification.timestamp }}</p>
                </div>
                <div v-if="!notification.read" class="w-2 h-2 bg-primary rounded-full shrink-0 mt-2"></div>
              </div>
            </div>
          </div>
          
          <!-- Footer -->
          <div class="px-4 py-3 border-t border-gray-200 dark:border-border-dark flex items-center justify-between">
            <button @click="clearAllHistory" class="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
              Clear All
            </button>
            <button @click="markAllAsRead" class="text-xs text-primary hover:underline">
              Mark all as read
            </button>
          </div>
        </div>
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

// Types
interface ToastAction {
  label: string
  primary?: boolean
  handler: () => void
}

interface Toast {
  id: string
  type: 'success' | 'error' | 'warning' | 'info' | 'progress'
  title: string
  description?: string
  actions?: ToastAction[]
  autoDismiss: boolean
  duration: number
  progress?: number
  timeRemaining?: string
  createdAt: number
  paused?: boolean
  timerRemaining?: number
}

interface HistoryNotification {
  id: string
  type: string
  title: string
  description?: string
  timestamp: string
  read: boolean
}

// State
const toasts = ref<Toast[]>([])
const history = ref<HistoryNotification[]>([])
const showHistoryPanel = ref(false)
const historyFilter = ref<'all' | 'unread'>('all')
const timers = ref<Map<string, ReturnType<typeof setTimeout>>>(new Map())

// Computed
const visibleToasts = computed(() => toasts.value.slice(0, 5))
const unreadCount = computed(() => history.value.filter(n => !n.read).length)
const filteredHistory = computed(() => {
  if (historyFilter.value === 'unread') {
    return history.value.filter(n => !n.read)
  }
  return history.value
})

// Toast default durations
const defaultDurations: Record<string, number> = {
  success: 3000,
  error: 0, // Never auto-dismiss
  warning: 5000,
  info: 4000,
  progress: 0 // Never auto-dismiss until complete
}

// Methods
function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).substring(2)
}

function addToast(options: Partial<Toast> & { title: string; type: Toast['type'] }): string {
  const id = generateId()
  const duration = options.duration ?? defaultDurations[options.type] ?? 4000
  
  const toast: Toast = {
    id,
    type: options.type,
    title: options.title,
    description: options.description,
    actions: options.actions,
    autoDismiss: duration > 0,
    duration,
    progress: options.progress,
    timeRemaining: options.timeRemaining,
    createdAt: Date.now(),
    paused: false,
    timerRemaining: duration
  }
  
  toasts.value.unshift(toast)
  
  // Add to history
  addToHistory(toast)
  
  // Set auto-dismiss timer
  if (toast.autoDismiss && duration > 0) {
    startTimer(id, duration)
  }
  
  // Prune if more than 5 toasts
  if (toasts.value.length > 5) {
    const oldest = toasts.value[toasts.value.length - 1]
    dismissToast(oldest.id)
  }
  
  return id
}

function dismissToast(id: string) {
  clearTimer(id)
  toasts.value = toasts.value.filter(t => t.id !== id)
}

function startTimer(id: string, duration: number) {
  const timer = setTimeout(() => {
    dismissToast(id)
  }, duration)
  timers.value.set(id, timer)
}

function clearTimer(id: string) {
  const timer = timers.value.get(id)
  if (timer) {
    clearTimeout(timer)
    timers.value.delete(id)
  }
}

function pauseTimer(id: string) {
  const toast = toasts.value.find(t => t.id === id)
  if (toast && toast.autoDismiss) {
    toast.paused = true
    const timer = timers.value.get(id)
    if (timer) {
      clearTimeout(timer)
      toast.timerRemaining = Math.max(0, toast.duration - (Date.now() - toast.createdAt))
    }
  }
}

function resumeTimer(id: string) {
  const toast = toasts.value.find(t => t.id === id)
  if (toast && toast.autoDismiss && toast.paused) {
    toast.paused = false
    toast.createdAt = Date.now() - (toast.duration - (toast.timerRemaining || 0))
    startTimer(id, toast.timerRemaining || toast.duration)
  }
}

function getTimerProgress(toast: Toast): number {
  if (!toast.autoDismiss || !toast.duration) return 100
  const elapsed = Date.now() - toast.createdAt
  return Math.max(0, 100 - (elapsed / toast.duration) * 100)
}

function handleToastClick(toast: Toast, event: MouseEvent) {
  if (!(event.target as HTMLElement).closest('button')) {
    dismissToast(toast.id)
  }
}

function handleAction(toast: Toast, action: ToastAction) {
  action.handler()
  dismissToast(toast.id)
}

function updateProgress(id: string, progress: number, timeRemaining?: string) {
  const toast = toasts.value.find(t => t.id === id)
  if (toast) {
    toast.progress = progress
    if (timeRemaining) toast.timeRemaining = timeRemaining
  }
}

function completeProgress(id: string, success: boolean, message?: string) {
  const toast = toasts.value.find(t => t.id === id)
  if (toast) {
    toast.type = success ? 'success' : 'error'
    if (message) toast.title = message
    toast.progress = undefined
    toast.timeRemaining = undefined
    toast.autoDismiss = success
    toast.duration = success ? 3000 : 0
    toast.createdAt = Date.now()
    if (success) {
      startTimer(id, 3000)
    }
  }
}

// History Methods
function addToHistory(toast: Toast) {
  const notification: HistoryNotification = {
    id: toast.id,
    type: toast.type,
    title: toast.title,
    description: toast.description,
    timestamp: 'Just now',
    read: false
  }
  history.value.unshift(notification)
  
  // Limit to 50 items
  if (history.value.length > 50) {
    history.value = history.value.slice(0, 50)
  }
}

function markAsRead(id: string) {
  const notification = history.value.find(n => n.id === id)
  if (notification) {
    notification.read = true
  }
}

function markAllAsRead() {
  history.value.forEach(n => n.read = true)
}

function clearAllHistory() {
  history.value = []
}

// Click outside to close history panel
function handleClickOutside(event: MouseEvent) {
  const panel = document.querySelector('.notification-history')
  const bell = document.querySelector('.notification-bell button')
  if (panel && bell && !panel.contains(event.target as Node) && !bell.contains(event.target as Node)) {
    showHistoryPanel.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)
  
  // Demo notifications
  setTimeout(() => {
    addToast({ type: 'success', title: '15 tracks added to queue' })
  }, 1000)
  
  setTimeout(() => {
    addToast({ 
      type: 'info', 
      title: 'Sync completed', 
      description: '5 new tracks from Spotify' 
    })
  }, 2000)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
  timers.value.forEach(timer => clearTimeout(timer))
})

// Expose methods for external use
defineExpose({
  success: (title: string, description?: string) => addToast({ type: 'success', title, description }),
  error: (title: string, description?: string, actions?: ToastAction[]) => addToast({ type: 'error', title, description, actions }),
  warning: (title: string, description?: string) => addToast({ type: 'warning', title, description }),
  info: (title: string, description?: string) => addToast({ type: 'info', title, description }),
  progress: (title: string, progress: number = 0) => addToast({ type: 'progress', title, progress }),
  updateProgress,
  completeProgress,
  dismiss: dismissToast
})
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

/* Dropdown Animation */
.dropdown-enter-active,
.dropdown-leave-active {
  transition: all 0.2s ease;
}

.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

/* Custom Scrollbar */
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.2);
  border-radius: 3px;
}

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.3);
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
