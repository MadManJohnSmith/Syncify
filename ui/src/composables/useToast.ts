import { ref, readonly, computed } from 'vue'

// Types
export interface ToastAction {
    label: string
    primary?: boolean
    /** Router path the notification center offers when the popup is gone. */
    route?: string
    /** Payload a caller may need to run the action (queue_id, track_id). */
    meta?: Record<string, unknown>
    kind?: 'navigate' | 'retry' | 'resolve'
    handler: () => void | Promise<void>
}

export type PopupCategory = 'download' | 'enrichment' | 'sync' | 'system' | 'backup'

// NT-03. The preferences are REAL: `TraySettings` (ui/src/api/tray.ts) is
// persisted in the backend KV store and is the same table the worker already
// consults via `tray::notification_category_enabled` (src-tauri/src/tray.rs:368)
// before emitting per-track download events. So muting a category here stops
// both the emission and the popup — no duplicate localStorage copy.
//
// Mapping (frontend category -> backend knob), keeping one source of truth:
//   download   -> notify_download_complete (routine completions)
//   sync       -> notify_sync_complete
//   enrichment -> notify_download_complete (metadata work rides the same knob)
//   backup     -> notify_updates
//   system     -> always true (the master `notifications_enabled` switch is
//                 checked by the owner; per-category it must never be silent)
const popupPreferenceKey = 'syncify:popup-preferences:v1'
const defaultPopupPreferences: Record<PopupCategory, boolean> = {
    download: true,
    enrichment: false,
    sync: false,
    system: true,
    backup: false
}
const popupPreferences = ref<Record<PopupCategory, boolean>>({ ...defaultPopupPreferences })
let popupPreferencesLoaded = false

function loadPopupPreferences() {
    if (popupPreferencesLoaded) return
    popupPreferencesLoaded = true
    try {
        const saved = JSON.parse(localStorage.getItem(popupPreferenceKey) || '{}') as Record<string, unknown>
        for (const category of Object.keys(defaultPopupPreferences) as PopupCategory[]) {
            if (typeof saved[category] === 'boolean') popupPreferences.value[category] = saved[category]
        }
    } catch { /* Storage is optional (tests/private mode). */ }
    // Mirror the real backend setting when it has ever been loaded, so a mute
    // applied in Settings is honoured here without a second store.
    syncFromTraySettings(traySettingsCache)
}

/** Cached backend preference, refreshed by `applyTrayPreferences`. */
let traySettingsCache: { notify_download_complete: boolean; notify_sync_complete: boolean; notify_updates: boolean } | null = null

function syncFromTraySettings(settings: { notify_download_complete: boolean; notify_sync_complete: boolean; notify_updates: boolean } | null) {
    if (!settings) return
    popupPreferences.value = {
        ...popupPreferences.value,
        download: settings.notify_download_complete,
        sync: settings.notify_sync_complete,
    }
}

/**
 * Called by whoever owns the tray settings (App.vue's load cycle or the
 * Settings view) to keep the popup taxonomy in sync with the backend.
 */
function applyTrayPreferences(settings: { notify_download_complete: boolean; notify_sync_complete: boolean; notify_updates: boolean } | null) {
    traySettingsCache = settings
    loadPopupPreferences()
    syncFromTraySettings(settings)
}

function setPopupPreference(category: PopupCategory, enabled: boolean) {
    loadPopupPreferences()
    // Writing a category that the backend owns must not paper over it: persist
    // the request so `update_tray_settings` can be applied to the real store.
    popupPreferences.value = { ...popupPreferences.value, [category]: enabled }
    try { localStorage.setItem(popupPreferenceKey, JSON.stringify(popupPreferences.value)) } catch { /* optional */ }
}

function shouldPopup(category: PopupCategory): boolean {
    loadPopupPreferences()
    return popupPreferences.value[category]
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
    /** Aggregation key (NT-02); toasts sharing it are merged by counting. */
    batchKey?: string
    /** How many real events the aggregated toast stands for. */
    batchCount?: number
}

/**
 * Popup description for an aggregated batch: the first event's message stays
 * visible and the counter is appended, so a single toast reports "n tracks"
 * without dropping what happened to any of them (the history has the detail).
 */
function batchDescription(description: string | undefined, count: number): string | undefined {
    if (count <= 1) return description
    return description ? `${description} (+${count} tracks)` : `${count} tracks`
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
    /** Action that outlives the popup (NT-01). */
    actionLabel?: string
    actionRoute?: string
    actionKind?: 'navigate' | 'retry' | 'resolve'
    actionMeta?: Record<string, unknown>
    /** Batch toast this entry was folded into, when aggregated (NT-02). */
    toastId?: string
    /** Kept in memory so the center can invoke retry as well as navigation. */
    action?: ToastAction
}

const history = ref<HistoryNotification[]>([])
const showHistoryPanel = ref(false)

/** Notification-center retention: the panel never grows without bound. */
const MAX_HISTORY = 50

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

function describeAction(action: ToastAction | undefined) {
    if (!action) return {}
    return {
        actionLabel: action.label,
        actionRoute: action.route,
        actionKind: action.kind ?? (action.route ? 'navigate' : 'retry'),
        actionMeta: action.meta,
        action
    }
}

function addToast(options: Partial<Toast> & { title: string; type: Toast['type'] }): string {
    const duration = options.duration ?? defaultDurations[options.type] ?? 4000

    // NT-02: popups that scale with the number of tracks (download completion,
    // per-track failure) share a batchKey and are merged into one toast whose
    // description ends in "(n tracks)", while the history keeps every
    // individual entry. Time-based: each new event restarts the window, so the
    // popup stays visible while events keep arriving and disappears one window
    // after the last one.
    if (options.batchKey && options.type !== 'progress') {
        const existing = toasts.value.find(t => t.batchKey === options.batchKey)
        const id = existing?.id ?? generateId()
        if (existing) {
            clearTimer(existing.id)
            existing.batchCount = (existing.batchCount ?? 1) + 1
            existing.title = options.title
            existing.type = options.type
            // The AppNotification carries queue_id for the concrete Retry
            // action; the second ServiceNotification does not. Do not replace
            // that richer action with a generic one while merging the popup.
            if (options.actions?.some(a => a.meta?.queue_id !== undefined) || !existing.actions) {
                existing.actions = options.actions
            }
            existing.description = batchDescription(options.description, existing.batchCount)
            existing.createdAt = Date.now()
            existing.timerRemaining = duration
            if (duration > 0) {
                startTimer(id, duration)
            }
        } else {
            const toast: Toast = {
                id,
                type: options.type,
                title: options.title,
                description: batchDescription(options.description, 1),
                actions: options.actions,
                autoDismiss: duration > 0,
                duration,
                progress: options.progress,
                timeRemaining: options.timeRemaining,
                createdAt: Date.now(),
                paused: false,
                timerRemaining: duration,
                batchKey: options.batchKey,
                batchCount: 1
            }
            toasts.value.unshift(toast)
            if (toast.autoDismiss && duration > 0) {
                startTimer(id, duration)
            }
        }

        // Add to persistent notification history. Every aggregated event is
        // kept individually, marked read when the single popup is dismissed.
        history.value.unshift({
            id: generateId(),
            type: options.type,
            title: options.title,
            description: options.description,
            timestamp: formatNow(),
            read: false,
            toastId: id,
            ...describeAction(options.actions?.[0])
        })
        if (history.value.length > MAX_HISTORY) {
            history.value.length = MAX_HISTORY
        }
        if (toasts.value.length > 5) {
            const oldest = toasts.value[toasts.value.length - 1]
            dismissToast(oldest.id)
        }
        return id
    }

    const id = generateId()
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
        read: false,
        ...describeAction(options.actions?.[0])
    })
    if (history.value.length > MAX_HISTORY) {
        history.value.length = MAX_HISTORY
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

/** History-only entry: no popup, no timer, no key collision (NT-02/NT-03). */
function logToHistory(entry: {
    type: Toast['type']
    title: string
    description?: string
    action?: ToastAction
}): void {
    history.value.unshift({
        id: generateId(),
        type: entry.type,
        title: entry.title,
        description: entry.description,
        timestamp: formatNow(),
        read: false,
        ...describeAction(entry.action)
    })
    if (history.value.length > MAX_HISTORY) {
        history.value.length = MAX_HISTORY
    }
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
    if (item) {
        item.read = true
        return
    }
    // `id` can also be the batch toast a set of aggregated entries belongs to
    // (NT-02): dismissing the popup reads all of them at once.
    for (const entry of history.value) {
        if (entry.toastId === id) entry.read = true
    }
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
        /** Re-run the action of a history entry (NT-01: clickable history). */
        runHistoryAction: (id: string): boolean => {
            const entry = history.value.find(h => h.id === id && h.action)
            if (!entry) return false
            entry.read = true
            void entry.action?.handler()
            return true
        },

        /**
         * NT-03: one toast per category/operation instead of one per event.
         * `category` gates the popup against the user's preference; the
         * `batchKey` merges everything sharing it (counted), so a 100-track
         * download produces one popup while the history keeps all 100 entries.
         */
        notify: (options: {
            type: Toast['type']
            title: string
            description?: string
            category: PopupCategory
            batchKey: string
            /** Never suppressed, whatever the preference says. */
            critical?: boolean
            /** Route to history only, e.g. the second Rust failure channel. */
            popup?: boolean
            actions?: ToastAction[]
        }): string | null => {
            if (options.popup === false || (options.critical !== true && !shouldPopup(options.category))) {
                // Suppressed from the popup surface only: the notification center
                // keeps the entry and its action, so nothing is lost.
                logToHistory({
                    type: options.type === 'progress' ? 'info' : options.type,
                    title: options.title,
                    description: options.description,
                    action: options.actions?.[0]
                })
                return null
            }
            return addToast({
                type: options.type,
                title: options.title,
                description: options.description,
                actions: options.actions,
                batchKey: options.batchKey
            })
        },

        popupPreferences: readonly(popupPreferences),
        setPopupPreference,
        isPopupEnabled: (category: PopupCategory) => shouldPopup(category),
        /** Feed the real backend `TraySettings` into the taxonomy. */
        applyTrayPreferences,

        // Avisos con respuesta: el mismo canal que los toasts, sin salir de la app.
        alert: alertDialog,
        confirm: confirmDialog,
        prompt: promptDialog
    }
}

export { pauseToast, resumeToast, dismissToast }
export type { Toast }

