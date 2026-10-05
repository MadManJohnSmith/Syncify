/**
 * R18 / audit 41: serialized sync counters and global progress weighted by the
 * real size of each task.
 *
 * `SyncProgressEvent` serializes `imported_tracks_total` and
 * `favorite_tracks_total` (src-tauri/src/commands/types.rs:137-138), so reading
 * only `imported`/`favorites` left the counters undefined forever.
 */
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { useGlobalTasks, resetGlobalTasks } from '@/composables/useGlobalTasks'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { resetMocks } from '../setup'

describe('R18/41: serialized counters and weighted global progress', () => {
    let eventBus: ReturnType<typeof useEventBus>

    beforeEach(() => {
        resetMocks()
        resetGlobalTasks()
        eventBus = useEventBus()
        eventBus.offAll()
    })

    afterEach(() => {
        resetGlobalTasks()
        eventBus.offAll()
    })

    it('lee imported_tracks_total y favorite_tracks_total de sync-progress', async () => {
        const { activeTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
            service: 'tidal',
            status: 'running',
            phase: 'Tracks',
            current: 40,
            total: 100,
            message: 'Importing favourites',
            imported_tracks_total: 137,
            favorite_tracks_total: 12,
        })

        const task = activeTasks.value[0]
        expect(task.importedCount).toBe(137)
        expect(task.favoriteCount).toBe(12)
        expect(task.description).toContain('137 imported')
        expect(task.description).toContain('12 favorites')
    })

    it('sigue aceptando los contadores ya renombrados de import-complete', async () => {
        const { allTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
            service: 'qobuz',
            status: 'running',
            current: 0,
            total: 10,
        })
        await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
            service: 'qobuz',
            status: 'completed',
            imported: 9,
            favorites: 4,
            message: 'Sync completed',
        })

        expect(allTasks.value[0].status).toBe('completed')
        expect(allTasks.value[0].importedCount).toBe(9)
        expect(allTasks.value[0].favoriteCount).toBe(4)
    })

    it('lee los contadores serializados al cerrar la sincronización', async () => {
        const { allTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
            service: 'spotify',
            status: 'running',
            current: 10,
            total: 100,
            imported_tracks_total: 10,
        })

        await eventBus.emit(TauriEvents.SYNC_COMPLETE, {
            service: 'spotify',
            status: 'completed',
            imported_tracks_total: 342,
            favorite_tracks_total: 27,
            message: 'Sync completed',
        })

        const task = allTasks.value.find(t => t.service === 'Spotify')
        expect(task?.status).toBe('completed')
        expect(task?.importedCount).toBe(342)
        expect(task?.favoriteCount).toBe(27)
    })

    it('aggregateProgress pondera por el volumen y no por la media de porcentajes', () => {
        const { addTask, aggregateProgress, overallProgress, hasDeterminateProgress, resetGlobalTasks: reset } =
            useGlobalTasks()
        reset()

        addTask({
            id: 'download-1',
            type: 'download',
            name: 'Downloading album',
            status: 'running',
            progress: 100,
            current: 10,
            total: 10,
        })
        addTask({
            id: 'download-2',
            type: 'download',
            name: 'Downloading singles',
            status: 'running',
            progress: 10,
            current: 100,
            total: 1000,
        })

        // Media simple: (100 + 10) / 2 = 55. Ponderado por volumen: 11.
        expect(overallProgress.value).toBe(55)
        expect(aggregateProgress.value).toBe(11)
        expect(hasDeterminateProgress.value).toBe(true)
    })

    it('aggregateProgress marca progreso indeterminado cuando ninguna tarea declara total', () => {
        const { addTask, aggregateProgress, hasDeterminateProgress, resetGlobalTasks: reset } = useGlobalTasks()
        reset()

        addTask({
            id: 'metadata-1',
            type: 'metadata',
            name: 'MusicBrainz',
            status: 'running',
            progress: 40,
        })

        expect(hasDeterminateProgress.value).toBe(false)
        expect(aggregateProgress.value).toBe(40)
    })

    it('una tarea indeterminada pesa como la media de las tareas con total declarado', () => {
        const { addTask, aggregateProgress, resetGlobalTasks: reset } = useGlobalTasks()
        reset()

        addTask({
            id: 'download-1',
            type: 'download',
            name: 'Downloading album',
            status: 'running',
            progress: 50,
            current: 500,
            total: 1000,
        })
        expect(aggregateProgress.value).toBe(50)

        // The phase with no total joins with the mean weight of the declared ones
        // (1000), so new work is reflected without resetting the bar to zero.
        addTask({
            id: 'lyrics-1',
            type: 'lyrics',
            name: 'Fetching lyrics',
            status: 'running',
            progress: 0,
        })

        expect(aggregateProgress.value).toBe(25)
    })
})