/**
 * Global Tasks Composable
 * 
 * Shared state for tracking all running tasks (downloads, syncs, imports, scans).
 * Used by header progress bar and status dropdown.
 */

import { ref, computed, readonly } from 'vue'
import { useEventBus, TauriEvents } from './useEventBus'

// Types
export interface GlobalTask {
    id: string
    type: 'download' | 'sync' | 'import' | 'scan' | 'metadata' | 'lyrics'
    name: string
    description?: string
    status: 'running' | 'paused' | 'completed' | 'failed' | 'requires_auth'
    progress: number // 0-100
    current?: number
    total?: number
    service?: string
    phase?: string
    importedCount?: number
    favoriteCount?: number
    startedAt: number
    error?: string
    requiresAuth?: boolean
}

// Global state (singleton pattern)
const tasks = ref<Map<string, GlobalTask>>(new Map())
const initialized = ref(false)
let unlistenFns: Array<() => void> = []

/**
 * Format a service ID into a friendly display name
 */
export function formatServiceName(serviceStr: string): string {
    const s = (serviceStr || '').trim().toLowerCase()
    if (s === 'spotify') return 'Spotify'
    if (s === 'qobuz') return 'Qobuz'
    if (s === 'tidal') return 'Tidal'
    if (s === 'deezer') return 'Deezer'
    if (s === 'soundcloud') return 'SoundCloud'
    if (s === 'apple' || s === 'apple_music' || s === 'applemusic') return 'Apple Music'
    if (s === 'musicbrainz') return 'MusicBrainz'
    if (s === 'lastfm') return 'Last.fm'
    return s.charAt(0).toUpperCase() + s.slice(1)
}

/**
 * Parse service name and sub-phase from compound strings (e.g. 'tidal_albums' -> root: 'tidal', phase: 'Albums')
 */
export function parseServiceAndPhase(
    serviceStr: string,
    explicitPhase?: string
): { rawService: string; rootService: string; phase: string; formattedName: string } {
    const raw = (serviceStr || '').trim().toLowerCase()
    let root = raw
    let phase = explicitPhase || ''

    const knownServices = ['spotify', 'qobuz', 'tidal', 'deezer', 'soundcloud', 'apple_music', 'apple']
    
    for (const s of knownServices) {
        if (raw === s) {
            root = s === 'apple' ? 'apple_music' : s
            break
        }
        if (raw.startsWith(s + '_')) {
            root = s === 'apple' ? 'apple_music' : s
            if (!phase) {
                phase = raw.slice(s.length + 1)
            }
            break
        }
    }

    // Capitalize phase nicely for display e.g. "playlists" -> "Playlists", "favorite_albums" -> "Favorite Albums"
    const formattedPhase = phase
        ? phase.split('_').map(w => w.charAt(0).toUpperCase() + w.slice(1)).join(' ')
        : 'Library'

    return {
        rawService: raw,
        rootService: root,
        phase: formattedPhase,
        formattedName: formatServiceName(root),
    }
}

/**
 * Generate unique task ID (case-insensitive for service names and maps subphases to parent sync task)
 */
export function generateTaskId(type: string, subId?: string | number): string {
    if (type === 'sync' && typeof subId === 'string') {
        const { rootService } = parseServiceAndPhase(subId)
        return `sync-${rootService}`
    }
    const normalizedSub = typeof subId === 'string' ? subId.toLowerCase() : subId
    return `${type}-${normalizedSub || Date.now()}`
}

/**
 * Reset all global tasks state (primarily for tests)
 */
export function resetGlobalTasks(): void {
    tasks.value = new Map()
    unlistenFns.forEach(fn => fn())
    unlistenFns = []
    initialized.value = false
}

/**
 * Composable for global task management
 */
export function useGlobalTasks() {
    const eventBus = useEventBus()

    // Computed
    const activeTasks = computed(() =>
        Array.from(tasks.value.values()).filter(t => t.status === 'running' || t.status === 'paused')
    )

    const allTasks = computed(() => Array.from(tasks.value.values()))

    const hasActiveTasks = computed(() => activeTasks.value.length > 0)

    const activeTaskCount = computed(() => activeTasks.value.length)

    const overallProgress = computed(() => {
        const active = activeTasks.value
        if (active.length === 0) return 0
        const totalProgress = active.reduce((sum, task) => sum + task.progress, 0)
        return Math.round(totalProgress / active.length)
    })

    const isAnyDownloading = computed(() =>
        activeTasks.value.some(t => t.type === 'download' && t.status === 'running')
    )

    const downloadingCount = computed(() =>
        activeTasks.value.filter(t => t.type === 'download' && t.status === 'running').length
    )

    // Actions
    function addTask(task: Omit<GlobalTask, 'startedAt'>): string {
        const fullTask: GlobalTask = {
            ...task,
            startedAt: Date.now()
        }
        const next = new Map(tasks.value)
        next.set(task.id, fullTask)
        tasks.value = next
        return task.id
    }

    function updateTask(id: string, updates: Partial<GlobalTask>): void {
        const task = tasks.value.get(id)
        if (task) {
            const next = new Map(tasks.value)
            next.set(id, { ...task, ...updates })
            tasks.value = next
        }
    }

    function updateTaskProgress(
        id: string,
        progress: number,
        current?: number,
        total?: number,
        description?: string
    ): void {
        const task = tasks.value.get(id)
        if (task) {
            const next = new Map(tasks.value)
            next.set(id, {
                ...task,
                progress: Math.min(100, Math.max(0, progress)),
                ...(current !== undefined && { current }),
                ...(total !== undefined && { total }),
                ...(description !== undefined && { description }),
            })
            tasks.value = next
        }
    }

    function completeTask(
        id: string,
        success: boolean = true,
        error?: string,
        options?: {
            requiresAuth?: boolean
            imported?: number
            favorites?: number
            message?: string
        }
    ): void {
        const task = tasks.value.get(id)
        if (task) {
            const isAuth = options?.requiresAuth || (error && (
                error.includes('RequiresAuth') ||
                error.includes('401') ||
                error.includes('authentication required') ||
                error.includes('credentials invalid') ||
                error.includes('Session expired')
            ))

            const status: GlobalTask['status'] = isAuth
                ? 'requires_auth'
                : (success ? 'completed' : 'failed')

            const next = new Map(tasks.value)
            next.set(id, {
                ...task,
                status,
                progress: success ? 100 : task.progress,
                error,
                requiresAuth: !!isAuth,
                ...(options?.imported !== undefined && { importedCount: options.imported }),
                ...(options?.favorites !== undefined && { favoriteCount: options.favorites }),
                ...(options?.message && { description: options.message }),
            })
            tasks.value = next

            // Auto-remove successful tasks after 3 seconds
            if (success) {
                setTimeout(() => {
                    // Only remove if it hasn't been re-started
                    const current = tasks.value.get(id)
                    if (current && current.status === 'completed') {
                        removeTask(id)
                    }
                }, 3000)
            }
        }
    }

    function removeTask(id: string): void {
        const next = new Map(tasks.value)
        next.delete(id)
        tasks.value = next
    }

    function pauseTask(id: string): void {
        updateTask(id, { status: 'paused' })
    }

    function resumeTask(id: string): void {
        updateTask(id, { status: 'running' })
    }

    function clearCompleted(): void {
        const next = new Map(tasks.value)
        for (const [id, task] of next) {
            if (task.status === 'completed') {
                next.delete(id)
            }
        }
        tasks.value = next
    }

    function clearFailed(): void {
        const next = new Map(tasks.value)
        for (const [id, task] of next) {
            if (task.status === 'failed' || task.status === 'requires_auth') {
                next.delete(id)
            }
        }
        tasks.value = next
    }

    // Helper to create common task types
    function startDownloadTask(trackName: string, queueId: number, service?: string): string {
        return addTask({
            id: generateTaskId('download', queueId),
            type: 'download',
            name: `Downloading ${trackName}`,
            description: trackName,
            status: 'running',
            progress: 0,
            service
        })
    }

    function startSyncTask(serviceName: string, initialPhase?: string): string {
        const { rootService, phase, formattedName } = parseServiceAndPhase(serviceName, initialPhase)
        const taskId = generateTaskId('sync', rootService)

        const existing = tasks.value.get(taskId)
        if (existing) {
            updateTask(taskId, {
                status: 'running',
                progress: 0,
                current: 0,
                total: 0,
                phase: initialPhase || existing.phase || 'Initializing',
                description: `Syncing ${formattedName}...`,
                error: undefined,
                requiresAuth: false,
            })
            return taskId
        }

        return addTask({
            id: taskId,
            type: 'sync',
            name: `Syncing ${formattedName}`,
            description: `Importing library from ${formattedName}`,
            status: 'running',
            progress: 0,
            current: 0,
            total: 0,
            service: formattedName,
            phase: initialPhase || 'Initializing',
        })
    }

    function updateSyncProgress(
        serviceName: string,
        data: {
            phase?: string
            current?: number
            total?: number
            progress?: number
            message?: string
            imported?: number
            favorites?: number
        }
    ): void {
        const { rootService, phase: parsedPhase, formattedName } = parseServiceAndPhase(serviceName, data.phase)
        const taskId = generateTaskId('sync', rootService)

        const current = data.current
        const total = data.total
        const progress = total && total > 0 && current !== undefined
            ? Math.min(100, Math.round((current / total) * 100))
            : (data.progress !== undefined ? data.progress : 0)

        const phase = data.phase || parsedPhase

        let description = data.message || ''
        if (!description && phase) {
            description = `Phase: ${phase}`
            if (current !== undefined && total !== undefined && total > 0) {
                description += ` (${current}/${total})`
            }
        }
        if (data.imported !== undefined || data.favorites !== undefined) {
            const counts: string[] = []
            if (data.imported !== undefined) counts.push(`${data.imported} imported`)
            if (data.favorites !== undefined) counts.push(`${data.favorites} favorites`)
            if (counts.length > 0) {
                description = description ? `${description} • ${counts.join(', ')}` : counts.join(', ')
            }
        }

        if (!tasks.value.has(taskId)) {
            addTask({
                id: taskId,
                type: 'sync',
                name: `Syncing ${formattedName}`,
                description: description || `Importing library from ${formattedName}`,
                status: 'running',
                progress,
                current,
                total,
                service: formattedName,
                phase,
                importedCount: data.imported,
                favoriteCount: data.favorites,
            })
        } else {
            const existing = tasks.value.get(taskId)!
            const next = new Map(tasks.value)
            next.set(taskId, {
                ...existing,
                status: 'running',
                progress: Math.min(100, Math.max(0, progress || existing.progress)),
                ...(current !== undefined && { current }),
                ...(total !== undefined && { total }),
                ...(phase && { phase }),
                ...(data.imported !== undefined && { importedCount: data.imported }),
                ...(data.favorites !== undefined && { favoriteCount: data.favorites }),
                description: description || existing.description,
            })
            tasks.value = next
        }
    }

    function completeSyncTask(
        serviceName: string,
        success: boolean = true,
        options?: {
            imported?: number
            favorites?: number
            message?: string
            error?: string
            requiresAuth?: boolean
        }
    ): void {
        const { rootService } = parseServiceAndPhase(serviceName)
        const taskId = generateTaskId('sync', rootService)
        completeTask(taskId, success, options?.error, options)
    }

    function failSyncTask(serviceName: string, error: string, requiresAuth?: boolean): void {
        const { rootService } = parseServiceAndPhase(serviceName)
        const taskId = generateTaskId('sync', rootService)
        completeTask(taskId, false, error, { requiresAuth })
    }

    function startImportTask(source: string, total?: number): string {
        return addTask({
            id: generateTaskId('import', source),
            type: 'import',
            name: `Importing from ${source}`,
            status: 'running',
            progress: 0,
            current: 0,
            total
        })
    }

    function startScanTask(path: string): string {
        return addTask({
            id: generateTaskId('scan', Date.now()),
            type: 'scan',
            name: 'Scanning local files',
            description: path,
            status: 'running',
            progress: 0
        })
    }

    // Structured Event Handlers
    function handleSyncProgressEvent(payload: any) {
        if (!payload || !payload.service) return
        const { status, error, requires_auth, current, total, progress, message, phase, imported, favorites } = payload

        if (status === 'failed' || status === 'error' || requires_auth || status === 'stale_source' || status === 'rejected_quality') {
            failSyncTask(payload.service, error || message || 'Sync failed', !!requires_auth)
            return
        }

        if (status === 'completed' || status === 'complete') {
            completeSyncTask(payload.service, true, {
                imported,
                favorites,
                message: message || 'Sync completed'
            })
            return
        }

        updateSyncProgress(payload.service, {
            phase,
            current,
            total,
            progress,
            message,
            imported,
            favorites,
        })
    }

    function handleSyncCompleteEvent(payload: any) {
        if (!payload || !payload.service) return
        completeSyncTask(payload.service, payload.success !== false, {
            imported: payload.imported,
            favorites: payload.favorites,
            message: payload.message || `Successfully synced ${formatServiceName(payload.service)}`
        })
    }

    function handleSyncFailedEvent(payload: any) {
        if (!payload || !payload.service) return
        failSyncTask(
            payload.service,
            payload.error || payload.message || 'Sync failed',
            !!payload.requires_auth
        )
    }

    // Initialize event listeners (idempotent)
    function initEventListeners(): void {
        if (initialized.value) return
        initialized.value = true

        // Clean any stale listeners first
        unlistenFns.forEach(fn => fn())
        unlistenFns = []

        // 1. Download progress events
        eventBus.on(TauriEvents.DOWNLOAD_PROGRESS, (payload: any) => {
            if (!payload) return
            const { queue_id, status, progress_percent, title, artist, message, error } = payload
            const taskId = generateTaskId('download', queue_id)
            const taskName = title ? `Downloading ${title}` : 'Downloading track'
            const taskDesc = artist ? `${artist} - ${title}` : (message || title)

            if (status === 'started' || !tasks.value.has(taskId)) {
                if (!tasks.value.has(taskId)) {
                    addTask({
                        id: taskId,
                        type: 'download',
                        name: taskName,
                        description: taskDesc,
                        status: 'running',
                        progress: progress_percent || 0,
                        current: progress_percent || 0,
                        total: 100
                    })
                }
            }

            if (status === 'complete' || status === 'completed') {
                completeTask(taskId, true)
            } else if (status === 'failed' || status === 'stale_source' || status === 'error' || status === 'rejected_quality') {
                const errMsg = error || message || 'Download failed'
                completeTask(taskId, false, errMsg)
            } else {
                updateTaskProgress(taskId, progress_percent || 0, undefined, undefined, taskDesc)
            }
        }).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        // 2. Import & Sync progress events
        eventBus.on(TauriEvents.IMPORT_PROGRESS, handleSyncProgressEvent).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        eventBus.on(TauriEvents.SYNC_PROGRESS, handleSyncProgressEvent).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        // 3. Import & Sync complete events
        eventBus.on(TauriEvents.IMPORT_COMPLETE, handleSyncCompleteEvent).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        eventBus.on(TauriEvents.SYNC_COMPLETE, handleSyncCompleteEvent).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        // 4. Import & Sync failed events
        eventBus.on(TauriEvents.IMPORT_FAILED, handleSyncFailedEvent).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        eventBus.on(TauriEvents.SYNC_FAILED, handleSyncFailedEvent).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        // 5. Auth session expired event
        eventBus.on(TauriEvents.AUTH_SESSION_EXPIRED, (payload: any) => {
            if (payload?.service) {
                failSyncTask(payload.service, payload.error || 'Authentication required', true)
            }
        }).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })

        // 6. Background enrichment events
        eventBus.on(TauriEvents.ENRICHMENT_STATUS, (payload: any) => {
            if (!payload) return
            const { type, status, pending, enriched, processed, message } = payload
            const taskId = generateTaskId('metadata', type)

            const typeNames: Record<string, string> = {
                musicbrainz: 'MusicBrainz',
                spotify: 'Spotify Audio Features',
                lastfm: 'Last.fm Genre',
                idle: 'Background Enrichment'
            }
            const typeName = typeNames[type] || type

            if (status === 'running') {
                if (!tasks.value.has(taskId)) {
                    addTask({
                        id: taskId,
                        type: 'metadata',
                        name: `${typeName}`,
                        description: message,
                        status: 'running',
                        progress: 0,
                        total: pending || 100
                    })
                } else {
                    updateTask(taskId, { description: message })
                }
            } else if (status === 'completed') {
                if (tasks.value.has(taskId)) {
                    const progress = enriched && processed ? Math.round((enriched / processed) * 100) : 100
                    updateTask(taskId, {
                        progress,
                        description: message,
                        current: enriched,
                        total: processed
                    })
                    completeTask(taskId, true)
                }
            } else if (status === 'error') {
                if (tasks.value.has(taskId)) {
                    completeTask(taskId, false, message)
                } else {
                    addTask({
                        id: taskId,
                        type: 'metadata',
                        name: typeName,
                        description: message,
                        status: 'failed',
                        progress: 0,
                        error: message
                    })
                }
            } else if (status === 'waiting') {
                removeTask(taskId)
            }
        }).then(unlisten => {
            if (unlisten) unlistenFns.push(unlisten)
        })
    }

    return {
        // State (readonly)
        tasks: readonly(tasks),
        allTasks,
        activeTasks,
        hasActiveTasks,
        activeTaskCount,
        overallProgress,
        isAnyDownloading,
        downloadingCount,

        // Actions
        addTask,
        updateTask,
        updateTaskProgress,
        completeTask,
        removeTask,
        pauseTask,
        resumeTask,
        clearCompleted,
        clearFailed,

        // Helpers
        startDownloadTask,
        startSyncTask,
        updateSyncProgress,
        completeSyncTask,
        failSyncTask,
        startImportTask,
        startScanTask,
        generateTaskId,

        // Init & Reset
        initEventListeners,
        resetGlobalTasks,
    }
}
