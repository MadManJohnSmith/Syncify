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
    status: 'running' | 'paused' | 'completed' | 'failed'
    progress: number // 0-100
    current?: number
    total?: number
    service?: string
    startedAt: number
    error?: string
}

// Global state (singleton pattern)
const tasks = ref<Map<string, GlobalTask>>(new Map())
const initialized = ref(false)

/**
 * Generate unique task ID (case-insensitive for service names)
 */
function generateTaskId(type: string, subId?: string | number): string {
    const normalizedSub = typeof subId === 'string' ? subId.toLowerCase() : subId
    return `${type}-${normalizedSub || Date.now()}`
}

/**
 * Composable for global task management
 */
export function useGlobalTasks() {
    const { on } = useEventBus()

    // Computed
    const activeTasks = computed(() =>
        Array.from(tasks.value.values()).filter(t => t.status === 'running' || t.status === 'paused')
    )

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
        tasks.value.set(task.id, fullTask)
        return task.id
    }

    function updateTask(id: string, updates: Partial<GlobalTask>): void {
        const task = tasks.value.get(id)
        if (task) {
            tasks.value.set(id, { ...task, ...updates })
        }
    }

    function updateTaskProgress(id: string, progress: number, current?: number, total?: number): void {
        const task = tasks.value.get(id)
        if (task) {
            tasks.value.set(id, {
                ...task,
                progress,
                ...(current !== undefined && { current }),
                ...(total !== undefined && { total })
            })
        }
    }

    function completeTask(id: string, success: boolean = true, error?: string): void {
        const task = tasks.value.get(id)
        if (task) {
            tasks.value.set(id, {
                ...task,
                status: success ? 'completed' : 'failed',
                progress: success ? 100 : task.progress,
                error
            })
            // Auto-remove completed tasks after 3 seconds
            if (success) {
                setTimeout(() => removeTask(id), 3000)
            }
        }
    }

    function removeTask(id: string): void {
        tasks.value.delete(id)
    }

    function pauseTask(id: string): void {
        updateTask(id, { status: 'paused' })
    }

    function resumeTask(id: string): void {
        updateTask(id, { status: 'running' })
    }

    function clearCompleted(): void {
        for (const [id, task] of tasks.value) {
            if (task.status === 'completed') {
                tasks.value.delete(id)
            }
        }
    }

    function clearFailed(): void {
        for (const [id, task] of tasks.value) {
            if (task.status === 'failed') {
                tasks.value.delete(id)
            }
        }
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

    function startSyncTask(serviceName: string): string {
        return addTask({
            id: generateTaskId('sync', serviceName),
            type: 'sync',
            name: `Syncing ${serviceName}`,
            description: `Importing library from ${serviceName}`,
            status: 'running',
            progress: 0,
            service: serviceName
        })
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

    // Initialize event listeners (only once)
    function initEventListeners(): void {
        if (initialized.value) return
        initialized.value = true

        // Listen for unified download progress events from backend
        // Backend emits 'syncify:download_progress' for started, complete, and failed states
        on(TauriEvents.DOWNLOAD_PROGRESS, (payload: any) => {
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

            if (status === 'complete') {
                completeTask(taskId, true)
            } else if (status === 'failed') {
                // fix: error message might be in 'message' or 'error' depending on backend
                const errMsg = error || message || 'Download failed';
                completeTask(taskId, false, errMsg)
            } else {
                // status is 'started' or 'downloading' or 'progress'
                updateTaskProgress(taskId, progress_percent || 0)

                // Update description if available to show status message
                if (message && tasks.value.has(taskId)) {
                    const task = tasks.value.get(taskId)
                    if (task) {
                        // Keep the artist - title format but maybe append status? 
                        // Actually let's just keep the static description for now to avoid flickering
                        // unless it was just initialized with generic text
                    }
                }
            }
        })

        on(TauriEvents.IMPORT_PROGRESS, (payload: any) => {
            const { service, current, total } = payload
            const taskId = generateTaskId('sync', service)
            const progress = total > 0 ? Math.round((current / total) * 100) : 0

            if (!tasks.value.has(taskId)) {
                startSyncTask(service)
            }
            updateTaskProgress(taskId, progress, current, total)
        })

        // Background enrichment events
        on(TauriEvents.ENRICHMENT_STATUS, (payload: any) => {
            const { type, status, pending, enriched, processed, message, nextRunIn } = payload
            const taskId = generateTaskId('metadata', type)

            // Get friendly name for enrichment type
            const typeNames: Record<string, string> = {
                musicbrainz: 'MusicBrainz',
                spotify: 'Spotify Audio Features',
                lastfm: 'Last.fm Genre',
                idle: 'Background Enrichment'
            }
            const typeName = typeNames[type] || type

            if (status === 'running') {
                // Create or update task as running
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
                // Mark as complete and auto-remove
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
                // Mark as failed
                if (tasks.value.has(taskId)) {
                    completeTask(taskId, false, message)
                } else {
                    // Create and immediately fail it
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
                // Idle state - remove any existing task
                removeTask(taskId)
            }
        })

        on(TauriEvents.IMPORT_COMPLETE, (payload: any) => {
            const { service } = payload
            const taskId = generateTaskId('sync', service)
            if (tasks.value.has(taskId)) {
                completeTask(taskId, true)
            }
        })
    }

    return {
        // State (readonly)
        tasks: readonly(tasks),
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
        startImportTask,
        startScanTask,
        generateTaskId,

        // Init
        initEventListeners
    }
}
