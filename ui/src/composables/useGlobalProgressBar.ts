/**
 * Global Progress Bar Composable (barra de progreso global ponderada)
 *
 * Agrega el progreso de TODAS las tareas activas con pesos reales:
 *  - descargas: por bytes (`bytes_downloaded` / `total_bytes` del evento
 *    `syncify:download_progress`) cuando el backend los reporta;
 *  - resto de tareas: por número de ítems (`task.total`) y, si ninguna lo
 *    declara, con peso igualitario para que sigan contando.
 *
 * Expone el ítem actual (la tarea dominante), el porcentaje total SUAVIZADO
 * (progresión exponencial hacia el objetivo, sin saltos bruscos entre
 * eventos) y una ETA calculada por regresión lineal sobre la muestra
 * reciente. Sin datos de peso deja `isIndeterminate` a true para que la UI
 * pinte la barra animada en vez de un 0% congelado.
 *
 * Lee SOLO la API pública de `useGlobalTasks` (no lo modifica) y no toca el
 * módulo de logging.
 */

import { computed, ref, watch, onScopeDispose, getCurrentScope } from 'vue'
import { useEventBus, TauriEvents } from './useEventBus'
import { useGlobalTasks, generateTaskId, type GlobalTask } from './useGlobalTasks'

/** Payload relevante de `syncify:download_progress` (DownloadProgressEvent). */
interface DownloadProgressPayload {
    queue_id?: number | string
    status?: string
    bytes_downloaded?: number
    total_bytes?: number | null
    terminal?: boolean
}

interface ByteProgress {
    done: number
    total: number
}

interface Aggregate {
    hasActive: boolean
    percent: number
    determinate: boolean
    currentLabel: string
}

const EMPTY_AGGREGATE: Aggregate = { hasActive: false, percent: 0, determinate: false, currentLabel: '' }

/** Constante de tiempo del suavizado exponencial (ms). */
const SMOOTHING_TAU_MS = 400
/** Periodo del tick de suavizado (ms). */
const SMOOTHING_TICK_MS = 50
/** Umbral por debajo del cual se clava el valor en lugar de seguir aproximando. */
const SNAP_EPSILON = 0.15
/** Periodo de muestreo para la ETA (ms). */
const ETA_SAMPLE_MS = 1000
/** Ventana de muestra para la ETA (ms). */
const ETA_WINDOW_MS = 90_000
/** Muestras y lapso mínimos antes de fiarse de la pendiente. */
const ETA_MIN_SAMPLES = 3
const ETA_MIN_SPAN_MS = 2000
/** Pendiente mínima creíble (%/s) para anunciar una ETA. */
const ETA_MIN_RATE = 0.05

function clampPercent(value: number): number {
    return Math.min(100, Math.max(0, value))
}

/** Formats an ETA in seconds as a compact label ('2m 40s', '1h 05m', '45s'). */
export function formatEtaLabel(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds <= 0) return ''
    const total = Math.round(seconds)
    const hours = Math.floor(total / 3600)
    const minutes = Math.floor((total % 3600) / 60)
    const secs = total % 60
    if (hours > 0) return `${hours}h ${String(minutes).padStart(2, '0')}m`
    if (minutes > 0) return `${minutes}m ${String(secs).padStart(2, '0')}s`
    return `${secs}s`
}

// Estado compartido a nivel de módulo (patrón singleton, como useGlobalTasks):
// los bytes reportados por el evento de descarga, indexados por id de tarea.
const bytesByTask = new Map<string, ByteProgress>()

export function useGlobalProgressBar() {
    const eventBus = useEventBus()
    const { activeTasks } = useGlobalTasks()

    /** % agregado crudo (sin suavizar), para tests y para la ETA. */
    const targetPercent = ref(0)
    /** % mostrado, aproximado exponencialmente al objetivo. */
    const displayPercent = ref(0)
    const isIndeterminate = ref(false)
    const hasActive = ref(false)
    const currentLabel = ref('')
    const etaSeconds = ref<number | null>(null)

    let unlistenBytes: (() => void) | null = null
    let smoothTimer: ReturnType<typeof setInterval> | null = null
    let lastTickAt = 0
    let lastSampleAt = 0
    let samples: Array<{ t: number; percent: number }> = []
    let disposed = false

    /**
     * Recalcula el agregado ponderado sobre las tareas activas.
     */
    function recomputeAggregate(): Aggregate {
        const active = activeTasks.value
        if (active.length === 0) return { ...EMPTY_AGGREGATE }

        // Peso de referencia para tareas sin total declarado: la media de los
        // totales declarados (o 1 si ninguna declara), para que una fase
        // indeterminada siga contando en lugar de resetear el agregado.
        const declaredTotals = active
            .map(t => t.total ?? 0)
            .filter(t => t > 0)
        const referenceTotal = declaredTotals.length > 0
            ? declaredTotals.reduce((a, b) => a + b, 0) / declaredTotals.length
            : 1

        let weighted = 0
        let totalWeight = 0
        let determinate = false
        let label = ''
        let labelWeight = -1

        for (const task of active) {
            const bytes = bytesByTask.get(task.id)
            let progress: number
            let weight: number
            if (bytes && bytes.total > 0) {
                // Peso por bytes reales de la descarga.
                weight = bytes.total
                progress = (bytes.done / bytes.total) * 100
                determinate = true
            } else if ((task.total ?? 0) > 0) {
                // Peso por número de ítems de la tarea.
                weight = task.total as number
                progress = task.progress ?? 0
                determinate = true
            } else {
                weight = referenceTotal
                progress = task.progress ?? 0
            }
            progress = clampPercent(progress)
            weighted += progress * weight
            totalWeight += weight
            // El ítem actual es la tarea dominante (mayor peso).
            if (weight > labelWeight) {
                labelWeight = weight
                label = task.name
            }
        }

        return {
            hasActive: true,
            percent: totalWeight > 0 ? clampPercent(weighted / totalWeight) : 0,
            determinate,
            currentLabel: label,
        }
    }

    /** Recalcula objetivo/etiqueta/indeterminado y resetea la ETA si procede. */
    function refresh(): void {
        const next = recomputeAggregate()
        hasActive.value = next.hasActive
        targetPercent.value = next.percent
        isIndeterminate.value = next.hasActive && !next.determinate
        currentLabel.value = next.currentLabel

        if (!next.hasActive) {
            samples = []
            etaSeconds.value = null
        }
    }

    // ---------------------------------------------
    // ETA: pendiente por regresión lineal de la ventana reciente
    // ---------------------------------------------
    function updateEta(now: number): void {
        if (!hasActive.value || isIndeterminate.value) {
            etaSeconds.value = null
            return
        }
        if (now - lastSampleAt >= ETA_SAMPLE_MS) {
            lastSampleAt = now
            samples.push({ t: now, percent: targetPercent.value })
            const cutoff = now - ETA_WINDOW_MS
            while (samples.length > 0 && samples[0].t < cutoff) samples.shift()
        }

        if (samples.length < ETA_MIN_SAMPLES) {
            etaSeconds.value = null
            return
        }
        const span = samples[samples.length - 1].t - samples[0].t
        if (span < ETA_MIN_SPAN_MS) {
            etaSeconds.value = null
            return
        }

        // Regresión lineal simple: pendiente de % frente a tiempo.
        const n = samples.length
        const meanT = samples.reduce((s, p) => s + p.t, 0) / n
        const meanP = samples.reduce((s, p) => s + p.percent, 0) / n
        let cov = 0
        let varT = 0
        for (const p of samples) {
            cov += (p.t - meanT) * (p.percent - meanP)
            varT += (p.t - meanT) ** 2
        }
        const rate = varT > 0 ? (cov / varT) * 1000 : 0 // % por segundo
        const remaining = 100 - targetPercent.value
        if (rate > ETA_MIN_RATE && remaining > 0.5) {
            etaSeconds.value = remaining / rate
        } else {
            etaSeconds.value = null
        }
    }

    // ---------------------------------------------
    // Suavizado: aproximación exponencial al objetivo, sin saltos
    // ---------------------------------------------
    function tick(): void {
        const now = Date.now()
        const dt = lastTickAt > 0 ? Math.min(now - lastTickAt, 2000) : SMOOTHING_TICK_MS
        lastTickAt = now

        updateEta(now)

        // Purga de bytes de tareas que ya no están activas (anti-fuga).
        if (activeTasks.value.length > 0) {
            const activeIds = new Set(activeTasks.value.map((t: GlobalTask) => t.id))
            for (const id of bytesByTask.keys()) {
                if (!activeIds.has(id)) bytesByTask.delete(id)
            }
        }

        const target = targetPercent.value
        if (Math.abs(target - displayPercent.value) < SNAP_EPSILON) {
            displayPercent.value = target
        } else {
            const alpha = 1 - Math.exp(-dt / SMOOTHING_TAU_MS)
            displayPercent.value += (target - displayPercent.value) * alpha
        }

        // Sin tareas activas y ya de vuelta a cero: el timer sobra.
        if (!hasActive.value && displayPercent.value <= 0.01) {
            displayPercent.value = 0
            stopTimer()
        }
    }

    function startTimer(): void {
        if (smoothTimer !== null || disposed) return
        lastTickAt = 0
        lastSampleAt = 0
        smoothTimer = setInterval(tick, SMOOTHING_TICK_MS)
    }

    function stopTimer(): void {
        if (smoothTimer !== null) {
            clearInterval(smoothTimer)
            smoothTimer = null
        }
    }

    function handleDownloadProgress(payload: DownloadProgressPayload): void {
        if (!payload || payload.queue_id === undefined) return
        const taskId = generateTaskId('download', payload.queue_id)
        const terminal = payload.terminal === true
            || payload.status === 'complete'
            || payload.status === 'completed'
            || payload.status === 'failed'
        if (terminal) {
            bytesByTask.delete(taskId)
            return
        }
        const total = typeof payload.total_bytes === 'number' && payload.total_bytes > 0
            ? payload.total_bytes
            : 0
        if (total <= 0) return
        const done = typeof payload.bytes_downloaded === 'number' && payload.bytes_downloaded >= 0
            ? payload.bytes_downloaded
            : 0
        bytesByTask.set(taskId, { done: Math.min(done, total), total })
    }

    async function init(): Promise<void> {
        if (unlistenBytes || disposed) return
        try {
            unlistenBytes = await eventBus.on<DownloadProgressPayload>(
                TauriEvents.DOWNLOAD_PROGRESS,
                handleDownloadProgress
            )
        } catch {
            // Sin bus de eventos (entorno de prueba) el agregado sigue
            // funcionando con los pesos por ítems.
        }
    }

    // Arranque perezoso: en cuanto haya tareas (o cambien) se recalcula y se
    // enciende el tick de suavizado.
    watch(
        () => activeTasks.value.map((t: GlobalTask) => `${t.id}:${t.progress}:${t.total ?? 0}`).join('|'),
        () => {
            refresh()
            startTimer()
        },
        { immediate: true }
    )

    void init()

    const etaLabel = computed(() => (etaSeconds.value !== null ? formatEtaLabel(etaSeconds.value) : ''))

    function dispose(): void {
        disposed = true
        stopTimer()
        if (unlistenBytes) {
            unlistenBytes()
            unlistenBytes = null
        }
    }

    if (getCurrentScope()) {
        onScopeDispose(dispose)
    }

    return {
        /** Hay tareas activas. */
        hasActive,
        /** Activas pero sin datos de peso: la barra debe ser indeterminada. */
        isIndeterminate,
        /** % agregado suavizado (0-100). */
        progress: displayPercent,
        /** % agregado crudo, para tests. */
        targetPercent,
        /** Nombre de la tarea dominante (ítem actual). */
        currentLabel,
        /** ETA en segundos, o null cuando no hay datos suficientes. */
        etaSeconds,
        /** ETA formateada ('2m 40s'), '' si no hay datos. */
        etaLabel,
        /** Libera timer y suscripción (también al desmontar el scope). */
        dispose,
    }
}
