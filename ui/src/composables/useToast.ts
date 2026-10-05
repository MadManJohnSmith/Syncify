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

// Toast default durations. `error` antes valía 0 y por eso ningún error se
// iba solo: se quedaba en pantalla hasta que el usuario lo cerraba a mano
// (R16). Se le da un plazo más largo que al resto porque exige leerlo.
const defaultDurations: Record<string, number> = {
    success: 3000,
    error: 8000,
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
    markAsRead(id)
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
        // Un download fallido también se retira solo: antes `duration = 0`
        // dejaba el aviso clavado en pantalla para siempre.
        toast.autoDismiss = true
        toast.duration = success ? 3000 : defaultDurations.error
        toast.createdAt = Date.now()
        startTimer(id, toast.duration)
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

// ============================================================
// Diálogos internos (R13): sustituyen a alert()/confirm()/message()
// nativos de plugin-dialog, que se abrían fuera de la ventana de la app.
export type DialogVariant = 'info' | 'warning' | 'danger'
export type DialogKind = 'alert' | 'confirm' | 'prompt'

export interface DialogRequest {
    id: string
    kind: DialogKind
    variant: DialogVariant
    title: string
    message?: string
    placeholder?: string
    defaultValue: string
    confirmLabel: string
    cancelLabel: string
}

export interface DialogOptions {
    title?: string
    message?: string
    variant?: DialogVariant
    confirmLabel?: string
    cancelLabel?: string
    placeholder?: string
    defaultValue?: string
}

const dialogQueue = ref<DialogRequest[]>([])
const dialogValue = ref('')
const dialogResolvers = new Map<string, (value: string | null) => void>()

const activeDialog = computed<DialogRequest | null>(() => dialogQueue.value[0] ?? null)

function openDialog(kind: DialogKind, message: string, options: DialogOptions = {}): Promise<string | null> {
    const id = generateId()
    const body = options.message ?? message
    const request: DialogRequest = {
        id,
        kind,
        variant: options.variant ?? (kind === 'confirm' ? 'warning' : 'info'),
        title: options.title ?? body,
        message: options.title ? body : undefined,
        placeholder: options.placeholder,
        defaultValue: options.defaultValue ?? '',
        confirmLabel: options.confirmLabel ?? (kind === 'alert' ? 'OK' : 'Confirm'),
        cancelLabel: options.cancelLabel ?? 'Cancel'
    }

    dialogQueue.value = [...dialogQueue.value, request]
    dialogValue.value = request.defaultValue

    return new Promise<string | null>((resolve) => {
        dialogResolvers.set(id, resolve)
    })
}

function resolveDialog(value: string | null) {
    const current = dialogQueue.value[0]
    if (!current) return

    dialogQueue.value = dialogQueue.value.slice(1)
    dialogResolvers.get(current.id)?.(value)
    dialogResolvers.delete(current.id)
    dialogValue.value = dialogQueue.value[0]?.defaultValue ?? ''
}

function alertDialog(message: string, options?: DialogOptions): Promise<void> {
    return openDialog('alert', message, options).then(() => undefined)
}

function confirmDialog(message: string, options?: DialogOptions): Promise<boolean> {
    return openDialog('confirm', message, options).then(value => value === 'confirm')
}

function promptDialog(message: string, options?: DialogOptions): Promise<string | null> {
    return openDialog('prompt', message, options).then(value => (value === null ? null : value))
}

export function useDialog() {
    return {
        dialogQueue: readonly(dialogQueue),
        dialogValue,
        activeDialog,
        openDialog,
        resolveDialog,
        alert: alertDialog,
        confirm: confirmDialog,
        prompt: promptDialog
    }
}

// Se exportan sueltos para que migrar un fichero sea cambiar el import:
// `import { confirm } from '@tauri-apps/plugin-dialog'` ->
// `import { confirm } from '@/composables/useToast'`.
export const alert = alertDialog
export const confirm = confirmDialog
export const prompt = promptDialog

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
        clearAllHistory,

        // Avisos con respuesta: el mismo canal que los toasts, sin salir de la app.
        alert: alertDialog,
        confirm: confirmDialog,
        prompt: promptDialog
    }
}

export { pauseToast, resumeToast, dismissToast }
export type { Toast, ToastAction }

