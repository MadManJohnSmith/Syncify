import { ref, readonly } from 'vue'

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
    timers.set(id, timer)
}

function clearTimer(id: string) {
    const timer = timers.get(id)
    if (timer) {
        clearTimeout(timer)
        timers.delete(id)
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

// Composable
export function useToast() {
    return {
        toasts: readonly(toasts),

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
        dismiss: dismissToast
    }
}

export type { Toast, ToastAction }
