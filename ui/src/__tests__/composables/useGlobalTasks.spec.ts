import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
    useGlobalTasks,
    resetGlobalTasks,
    formatServiceName,
    parseServiceAndPhase,
    generateTaskId
} from '@/composables/useGlobalTasks'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'

describe('S128A: Global Tasks & Sync Progress Suite', () => {
    let eventBus: ReturnType<typeof useEventBus>

    beforeEach(() => {
        resetGlobalTasks()
        eventBus = useEventBus()
        eventBus.offAll()
    })

    afterEach(() => {
        resetGlobalTasks()
        eventBus.offAll()
    })

    it('formatServiceName and parseServiceAndPhase normalize compound service names correctly', () => {
        expect(formatServiceName('spotify')).toBe('Spotify')
        expect(formatServiceName('qobuz')).toBe('Qobuz')
        expect(formatServiceName('tidal')).toBe('Tidal')
        expect(formatServiceName('deezer')).toBe('Deezer')
        expect(formatServiceName('apple_music')).toBe('Apple Music')
        expect(formatServiceName('apple')).toBe('Apple Music')

        const parsed1 = parseServiceAndPhase('tidal_albums')
        expect(parsed1.rootService).toBe('tidal')
        expect(parsed1.phase).toBe('Albums')
        expect(parsed1.formattedName).toBe('Tidal')

        const parsed2 = parseServiceAndPhase('qobuz_playlists')
        expect(parsed2.rootService).toBe('qobuz')
        expect(parsed2.phase).toBe('Playlists')

        const parsed3 = parseServiceAndPhase('spotify_enrichment')
        expect(parsed3.rootService).toBe('spotify')
        expect(parsed3.phase).toBe('Enrichment')

        const parsed4 = parseServiceAndPhase('spotify', 'Favorite Tracks')
        expect(parsed4.rootService).toBe('spotify')
        expect(parsed4.phase).toBe('Favorite Tracks')
    })

    it('1. Evento started crea Active Task', async () => {
        const { activeTasks, hasActiveTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        expect(hasActiveTasks.value).toBe(false)
        expect(activeTasks.value.length).toBe(0)

        // Emit started event for Tidal
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'tidal',
            status: 'started',
            current: 0,
            total: 150,
            message: 'Starting Tidal sync...',
            phase: 'Favorites',
        })

        expect(hasActiveTasks.value).toBe(true)
        expect(activeTasks.value.length).toBe(1)

        const task = activeTasks.value[0]
        expect(task.id).toBe('sync-tidal')
        expect(task.type).toBe('sync')
        expect(task.name).toBe('Syncing Tidal')
        expect(task.status).toBe('running')
        expect(task.progress).toBe(0)
        expect(task.current).toBe(0)
        expect(task.total).toBe(150)
        expect(task.service).toBe('Tidal')
        expect(task.phase).toBe('Favorites')
    })

    it('2. Evento por fase actualiza barra global (overallProgress)', async () => {
        const { activeTasks, overallProgress, initEventListeners } = useGlobalTasks()
        initEventListeners()

        // Phase 1: Favorites started
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'tidal',
            status: 'started',
            current: 0,
            total: 100,
            message: 'Fetching favorites...',
        })
        expect(overallProgress.value).toBe(0)

        // Phase 1: Progress halfway
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'tidal',
            status: 'progress',
            current: 50,
            total: 100,
            message: 'Fetching favorites (50/100)',
            imported: 45,
            favorites: 45,
        })

        expect(overallProgress.value).toBe(50)
        expect(activeTasks.value[0].importedCount).toBe(45)
        expect(activeTasks.value[0].favoriteCount).toBe(45)

        // Phase 2: Albums
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'tidal_albums',
            status: 'progress',
            current: 10,
            total: 20,
            message: 'Importing albums (10/20)',
            imported: 55,
        })

        expect(overallProgress.value).toBe(50)
        expect(activeTasks.value[0].id).toBe('sync-tidal')
        expect(activeTasks.value[0].phase).toBe('Albums')
        expect(activeTasks.value[0].importedCount).toBe(55)

        // Phase 3: Playlists 80%
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'tidal_playlists',
            status: 'progress',
            current: 8,
            total: 10,
            message: 'Importing playlists (8/10)',
        })

        expect(overallProgress.value).toBe(80)
        expect(activeTasks.value[0].phase).toBe('Playlists')
    })

    it('3. Evento completed finaliza correctamente y actualiza métricas', async () => {
        const { tasks, activeTasks, hasActiveTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
            service: 'qobuz',
            status: 'started',
            current: 0,
            total: 80,
        })
        expect(hasActiveTasks.value).toBe(true)

        // Emit completed event
        await eventBus.emit(TauriEvents.SYNC_COMPLETE, {
            service: 'qobuz',
            imported: 75,
            favorites: 40,
            message: 'Sync completed for Qobuz: 75 tracks imported',
            success: true,
        })

        const qobuzTask = tasks.value.get('sync-qobuz')
        expect(qobuzTask).toBeDefined()
        expect(qobuzTask!.status).toBe('completed')
        expect(qobuzTask!.progress).toBe(100)
        expect(qobuzTask!.importedCount).toBe(75)
        expect(qobuzTask!.favoriteCount).toBe(40)
        expect(qobuzTask!.description).toBe('Sync completed for Qobuz: 75 tracks imported')
        expect(hasActiveTasks.value).toBe(false)
    })

    it('4. Evento failed finaliza sin spinner', async () => {
        const { tasks, activeTasks, hasActiveTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'spotify',
            status: 'started',
            current: 0,
            total: 100,
        })
        expect(hasActiveTasks.value).toBe(true)

        // Emit failed event
        await eventBus.emit(TauriEvents.IMPORT_FAILED, {
            service: 'spotify',
            error: 'Network connection lost',
            requires_auth: false,
        })

        expect(hasActiveTasks.value).toBe(false)
        const spotifyTask = tasks.value.get('sync-spotify')
        expect(spotifyTask).toBeDefined()
        expect(spotifyTask!.status).toBe('failed')
        expect(spotifyTask!.error).toBe('Network connection lost')
    })

    it('5. Evento requires_auth marca status como requires_auth', async () => {
        const { tasks, activeTasks, hasActiveTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'qobuz',
            status: 'started',
            current: 0,
            total: 50,
        })
        expect(hasActiveTasks.value).toBe(true)

        // Emit progress with requires_auth
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'qobuz',
            status: 'failed',
            requires_auth: true,
            error: 'RequiresAuth: User authentication required (401)',
        })

        expect(hasActiveTasks.value).toBe(false)
        const qobuzTask = tasks.value.get('sync-qobuz')
        expect(qobuzTask).toBeDefined()
        expect(qobuzTask!.status).toBe('requires_auth')
        expect(qobuzTask!.requiresAuth).toBe(true)
        expect(qobuzTask!.error).toContain('RequiresAuth')
    })

    it('6. Múltiples servicios simultáneos no pisan su progreso', async () => {
        const { activeTasks, overallProgress, initEventListeners } = useGlobalTasks()
        initEventListeners()

        // Tidal at 20%
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'tidal',
            status: 'progress',
            current: 20,
            total: 100,
            phase: 'Favorites',
            imported: 20,
        })

        // Qobuz at 60%
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'qobuz',
            status: 'progress',
            current: 30,
            total: 50,
            phase: 'Albums',
            imported: 30,
        })

        expect(activeTasks.value.length).toBe(2)

        const tidalTask = activeTasks.value.find(t => t.id === 'sync-tidal')
        const qobuzTask = activeTasks.value.find(t => t.id === 'sync-qobuz')

        expect(tidalTask).toBeDefined()
        expect(tidalTask!.progress).toBe(20)
        expect(tidalTask!.service).toBe('Tidal')
        expect(tidalTask!.phase).toBe('Favorites')

        expect(qobuzTask).toBeDefined()
        expect(qobuzTask!.progress).toBe(60)
        expect(qobuzTask!.service).toBe('Qobuz')
        expect(qobuzTask!.phase).toBe('Albums')

        // overallProgress = average(20, 60) = 40
        expect(overallProgress.value).toBe(40)

        // Update Tidal subphase without touching Qobuz
        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'tidal_playlists',
            status: 'progress',
            current: 80,
            total: 100,
        })

        expect(activeTasks.value.find(t => t.id === 'sync-tidal')!.progress).toBe(80)
        expect(activeTasks.value.find(t => t.id === 'sync-qobuz')!.progress).toBe(60)
        expect(overallProgress.value).toBe(70) // (80 + 60)/2 = 70
    })

    it('7. Recarga de vista no deja listeners duplicados', async () => {
        const { activeTasks, initEventListeners } = useGlobalTasks()

        // Initialize multiple times (simulating re-renders/mounts)
        initEventListeners()
        initEventListeners()
        initEventListeners()

        let emittedCalls = 0
        const handler = () => { emittedCalls++ }
        await eventBus.on(TauriEvents.IMPORT_PROGRESS, handler)

        await eventBus.emit(TauriEvents.IMPORT_PROGRESS, {
            service: 'deezer',
            status: 'progress',
            current: 10,
            total: 20,
        })

        expect(emittedCalls).toBe(1)
        expect(activeTasks.value.length).toBe(1)
        expect(activeTasks.value[0].id).toBe('sync-deezer')
        expect(activeTasks.value[0].progress).toBe(50)
    })
})
