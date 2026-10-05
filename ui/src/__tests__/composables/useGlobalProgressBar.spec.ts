/**
 * useGlobalProgressBar.spec.ts
 * Regresión de la barra de progreso global ponderada (carril navegación):
 *  - peso por bytes cuando el evento de descarga los reporta;
 *  - peso por ítems (task.total) en su defecto e indeterminada sin datos;
 *  - suavizado sin saltos bruscos;
 *  - ETA con progreso sostenido y limpieza al terminar.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { nextTick } from 'vue'
import { useGlobalProgressBar } from '@/composables/useGlobalProgressBar'
import { useGlobalTasks, resetGlobalTasks } from '@/composables/useGlobalTasks'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { resetMocks } from '../setup'

describe('useGlobalProgressBar (barra global ponderada)', () => {
    let eventBus: ReturnType<typeof useEventBus>
    let dispose: () => void

    beforeEach(() => {
        resetMocks()
        resetGlobalTasks()
        vi.useFakeTimers()
        eventBus = useEventBus()
        eventBus.offAll()
        dispose = () => {}
    })

    afterEach(() => {
        dispose()
        resetGlobalTasks()
        eventBus.offAll()
        vi.useRealTimers()
    })

    it('agrega por bytes cuando syncify:download_progress los reporta', async () => {
        const gp = useGlobalProgressBar()
        dispose = gp.dispose
        const { activeTasks, initEventListeners } = useGlobalTasks()
        initEventListeners()

        await eventBus.emit(TauriEvents.DOWNLOAD_PROGRESS, {
            queue_id: 1,
            status: 'downloading',
            title: 'Song One',
            progress_percent: 25,
            bytes_downloaded: 25_000_000,
            total_bytes: 100_000_000,
        })
        await eventBus.emit(TauriEvents.DOWNLOAD_PROGRESS, {
            queue_id: 2,
            status: 'downloading',
            title: 'Song Two',
            progress_percent: 0,
            bytes_downloaded: 0,
            total_bytes: 50_000_000,
        })
        await nextTick()

        expect(activeTasks.value.length).toBe(2)
        expect(gp.hasActive.value).toBe(true)
        expect(gp.isIndeterminate.value).toBe(false)
        // (25% × 100 MB + 0% × 50 MB) / 150 MB = 16.67%, NO 12.5% (media plana)
        expect(gp.targetPercent.value).toBeCloseTo(100 / 6, 1)
        // La tarea dominante (más bytes) nombra el ítem actual
        expect(gp.currentLabel.value).toContain('Song One')
    })

    it('agrega por ítems cuando no hay bytes y marca indeterminada sin datos de peso', async () => {
        const gp = useGlobalProgressBar()
        dispose = gp.dispose
        const g = useGlobalTasks()

        // Sync con total declarado: peso por ítems
        g.startSyncTask('tidal')
        g.updateSyncProgress('tidal', { current: 40, total: 100 })
        await nextTick()

        expect(gp.hasActive.value).toBe(true)
        expect(gp.isIndeterminate.value).toBe(false)
        expect(gp.targetPercent.value).toBe(40)
        expect(gp.currentLabel.value).toContain('Tidal')

        // Solo una tarea sin total ni bytes: indeterminada, no 0% congelado
        g.startScanTask('/music/library')
        await nextTick()

        // El sync sigue dominando el agregado (peso 100)…
        expect(gp.isIndeterminate.value).toBe(false)

        // …pero si la única tarea activa no declara nada, es indeterminada
        resetGlobalTasks()
        const g2 = useGlobalTasks()
        g2.startScanTask('/music/library')
        await nextTick()
        expect(gp.hasActive.value).toBe(true)
        expect(gp.isIndeterminate.value).toBe(true)
        expect(gp.targetPercent.value).toBe(0)
    })

    it('suaviza el avance sin saltos bruscos y converge al objetivo', async () => {
        const gp = useGlobalProgressBar()
        dispose = gp.dispose
        const g = useGlobalTasks()

        g.startSyncTask('qobuz')
        g.updateSyncProgress('qobuz', { current: 0, total: 100 })
        await nextTick()
        expect(gp.progress.value).toBe(0)

        g.updateSyncProgress('qobuz', { current: 100, total: 100 })
        await nextTick()
        expect(gp.targetPercent.value).toBe(100)

        // Un tick de 50 ms NO salta al 100%: solo se aproxima
        vi.advanceTimersByTime(50)
        const afterOneTick = gp.progress.value
        expect(afterOneTick).toBeGreaterThan(0)
        expect(afterOneTick).toBeLessThan(100)

        // Sin saltos hacia atrás: la serie es monótona creciente hacia 100
        let previous = afterOneTick
        for (let i = 0; i < 60; i++) {
            vi.advanceTimersByTime(50)
            expect(gp.progress.value).toBeGreaterThanOrEqual(previous)
            previous = gp.progress.value
        }
        // …y converge clavado en el objetivo
        expect(gp.progress.value).toBe(100)
    })

    it('anuncia ETA con progreso sostenido y la limpia al terminar', async () => {
        const gp = useGlobalProgressBar()
        dispose = gp.dispose
        const g = useGlobalTasks()
        g.startSyncTask('spotify')
        await nextTick()

        // Sin muestra suficiente: sin ETA
        vi.advanceTimersByTime(200)
        expect(gp.etaSeconds.value).toBeNull()

        // Progreso sostenido ~10%/s durante 5 segundos
        for (let i = 1; i <= 5; i++) {
            g.updateSyncProgress('spotify', { current: i * 10, total: 100 })
            await nextTick()
            vi.advanceTimersByTime(1000)
        }
        expect(gp.targetPercent.value).toBe(50)
        expect(gp.etaSeconds.value).not.toBeNull()
        expect(gp.etaLabel.value).not.toBe('')

        // Al terminar: sin tareas activas, sin ETA
        g.completeSyncTask('spotify', true)
        vi.advanceTimersByTime(3500) // auto-remove a los 3s + easing
        await nextTick()
        expect(gp.hasActive.value).toBe(false)
        expect(gp.etaSeconds.value).toBeNull()
        expect(gp.etaLabel.value).toBe('')
    })
})
