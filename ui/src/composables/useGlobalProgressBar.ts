/**
 * Global Progress Bar Composable (weighted global progress bar)
 *
 * Aggregates the progress of ALL active tasks with real weights:
 *  - downloads: by bytes (`bytes_downloaded` / `total_bytes` from the
 *    `syncify:download_progress` event) when the backend reports them;
 *  - other tasks: by item count (`task.total`) and, if none declares one,
 *    with an equal weight so they still count.
 *
 * Exposes the current item (the dominant task), the SMOOTHED total percentage
 * (exponential progression toward the target, no abrupt jumps between
 * events) and an ETA computed by linear regression over the recent sample.
 * Without weight data it leaves `isIndeterminate` true so the UI
 * paints the animated bar instead of a frozen 0%.
 *
 * Reads ONLY the public API of `useGlobalTasks` (it does not modify it) and
 * does not touch the logging module.
 */

import { computed, ref, watch, onScopeDispose, getCurrentScope } from 'vue'
import { useEventBus, TauriEvents } from './useEventBus'
import { useGlobalTasks, generateTaskId, type GlobalTask } from './useGlobalTasks'

/** Relevant payload of `syncify:download_progress` (DownloadProgressEvent). */
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

/** Time constant of the exponential smoothing (ms). */
const SMOOTHING_TAU_MS = 400
/** Smoothing tick period (ms). */
const SMOOTHING_TICK_MS = 50
/** Threshold below which the value snaps instead of continuing to approach. */
const SNAP_EPSILON = 0.15
/** Sampling period for the ETA (ms). */
const ETA_SAMPLE_MS = 1000
/** Sampling window for the ETA (ms). */
const ETA_WINDOW_MS = 90_000
/** Minimum samples and span before trusting the slope. */
const ETA_MIN_SAMPLES = 3
const ETA_MIN_SPAN_MS = 2000
/** Minimum credible slope (%/s) to announce an ETA. */
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

// Module-level shared state (singleton pattern, like useGlobalTasks):
// the bytes reported by the download event, indexed by task id.
const bytesByTask = new Map<string, ByteProgress>()

export function useGlobalProgressBar() {
    const eventBus = useEventBus()
    const { activeTasks } = useGlobalTasks()

    /** Raw aggregated % (unsmoothed), for tests and for the ETA. */
    const targetPercent = ref(0)
    /** Displayed %, exponentially approaching the target. */
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
     * Recomputes the weighted aggregate over the active tasks.
     */
    function recomputeAggregate(): Aggregate {
        const active = activeTasks.value
        if (active.length === 0) return { ...EMPTY_AGGREGATE }

        // Reference weight for tasks without a declared total: the average of
        // the declared totals (or 1 if none declares one), so an
        // indeterminate phase still counts instead of resetting the aggregate.
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
                // Weight by the download's real bytes.
                weight = bytes.total
                progress = (bytes.done / bytes.total) * 100
                determinate = true
            } else if ((task.total ?? 0) > 0) {
                // Weight by the task's item count.
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
            // The current item is the dominant task (highest weight).
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

    /** Recomputes target/label/indeterminate and resets the ETA when appropriate. */
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
    // ETA: slope via linear regression over the recent window
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

        // Simple linear regression: slope of % over time.
        const n = samples.length
        const meanT = samples.reduce((s, p) => s + p.t, 0) / n
        const meanP = samples.reduce((s, p) => s + p.percent, 0) / n
        let cov = 0
        let varT = 0
        for (const p of samples) {
            cov += (p.t - meanT) * (p.percent - meanP)
            varT += (p.t - meanT) ** 2
        }
        const rate = varT > 0 ? (cov / varT) * 1000 : 0 // % per second
        const remaining = 100 - targetPercent.value
        if (rate > ETA_MIN_RATE && remaining > 0.5) {
            etaSeconds.value = remaining / rate
        } else {
            etaSeconds.value = null
        }
    }

    // ---------------------------------------------
    // Smoothing: exponential approach to the target, without jumps
    // ---------------------------------------------
    function tick(): void {
        const now = Date.now()
        const dt = lastTickAt > 0 ? Math.min(now - lastTickAt, 2000) : SMOOTHING_TICK_MS
        lastTickAt = now

        updateEta(now)

        // Purge bytes of tasks that are no longer active (leak guard).
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

        // No active tasks and already back at zero: the timer is unnecessary.
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
            // Without an event bus (test environment) the aggregate keeps
            // working with the item-count weights.
        }
    }

    // Lazy start: as soon as there are tasks (or they change) it recomputes and
    // turns on the smoothing tick.
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
        /** Whether there are active tasks. */
        hasActive,
        /** Active but without weight data: the bar must be indeterminate. */
        isIndeterminate,
        /** Smoothed aggregated % (0-100). */
        progress: displayPercent,
        /** Raw aggregated %, for tests. */
        targetPercent,
        /** Name of the dominant task (current item). */
        currentLabel,
        /** ETA in seconds, or null when there is not enough data. */
        etaSeconds,
        /** Formatted ETA ('2m 40s'), '' when there is no data. */
        etaLabel,
        /** Releases timer and subscription (also when the scope unmounts). */
        dispose,
    }
}
