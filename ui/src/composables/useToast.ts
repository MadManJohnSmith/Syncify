import { ref, readonly, computed } from 'vue'

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

// Global state
const toasts = ref<Toast[]>([])
const timers = new Map<string, ReturnType<typeof setTimeout>>()
const timerStarts = new Map<string, number>()

export interface HistoryNotification {
    id: string
    type: 'success' | 'error' | 'warning' | 'info' | 'progress'
    title: string
    description?: string
    timestamp: string
    read: boolean
}

const history = ref<HistoryNotification[]>([])
const showHistoryPanel = ref(false)

function formatNow(): string {
    const now = new Date()
    return now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

// Toast default durations
const defaultDurations: Record<string, number> = {
    success: 3000,
    error: 0, // Never auto-dismiss
    warning: 5000,
    info: 4000,
    progress: 0 // Never auto-dismiss until complete
}

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

    // Add to persistent notification history
    history.value.unshift({
        id,
        type: options.type,
        title: options.title,
        description: options.description,
        timestamp: formatNow(),
        read: false
    })
    if (history.value.length > 50) {
        history.value.pop()
    }

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
    clearTimer(id)
    timerStarts.set(id, Date.now())
    const timer = setTimeout(() => {
        timerStarts.delete(id)
        dismissToast(id)
    }, duration)
    timers.set(id, timer)
}

function clearTimer(id: string) {
    const timer = timers.get(id)
    if (timer) {
        clearTimeout(timer)
        timers.delete(id)
    }
    timerStarts.delete(id)
}

function pauseToast(id: string) {
    const toast = toasts.value.find(t => t.id === id)
    if (!toast || !toast.autoDismiss || toast.paused) return

    const timer = timers.get(id)
    if (timer) {
        clearTimeout(timer)
        timers.delete(id)
    }

    const startTime = timerStarts.get(id)
    if (startTime !== undefined) {
        const elapsed = Date.now() - startTime
        const currentRemaining = toast.timerRemaining ?? toast.duration
        toast.timerRemaining = Math.max(0, currentRemaining - elapsed)
        timerStarts.delete(id)
    }
    toast.paused = true
}

function resumeToast(id: string) {
    const toast = toasts.value.find(t => t.id === id)
    if (!toast || !toast.autoDismiss || !toast.paused) return

    toast.paused = false
    const remaining = toast.timerRemaining ?? toast.duration
    if (remaining > 0) {
        toast.createdAt = Date.now() - (toast.duration - remaining)
        startTimer(id, remaining)
    } else {
        dismissToast(id)
    }
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

function markAsRead(id: string) {
    const item = history.value.find(h => h.id === id)
    if (item) item.read = true
}

function markAllAsRead() {
    history.value.forEach(h => h.read = true)
}

function clearAllHistory() {
    history.value = []
}

// Composable
export function useToast() {
    const unreadCount = computed(() => history.value.filter(n => !n.read).length)

    return {
        toasts: readonly(toasts),
        history: readonly(history),
        unreadCount,
        showHistoryPanel,

        success: (title: string, description?: string) =>
            addToast({ type: 'success', title, description }),

        error: (title: string, description?: string, actions?: ToastAction[]) =>
            addToast({ type: 'error', title, description, actions }),

        warning: (title: string, description?: string) =>
            addToast({ type: 'warning', title, description }),

        info: (title: string, description?: string) =>
            addToast({ type: 'info', title, description }),

        progress: (title: string, progress: number = 0) =>
            addToast({ type: 'progress', title, progress }),

        updateProgress,
        completeProgress,
        dismiss: dismissToast,
        pauseToast,
        resumeToast,
        markAsRead,
        markAllAsRead,
        clearAllHistory
    }
}

export { pauseToast, resumeToast, dismissToast }
export type { Toast, ToastAction }

