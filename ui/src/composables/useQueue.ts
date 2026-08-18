/**
 * Queue Composable
 * 
 * State management for download queue and worker.
 */

import { ref, computed } from 'vue';
import { queueApi } from '@/api/queue';
import { useEventBus, TauriEvents } from './useEventBus';
import type { QueueItem, QueueStats, WorkerStatus, ProgressEvent } from '@/api/types';

const PROGRESS_THROTTLE_MS = 250; // Max 4 updates/sec per track

/**
 * Composable for download queue state management
 */
export function useQueue() {
    // State
    const queue = ref<QueueItem[]>([]);
    const stats = ref<QueueStats | null>(null);
    const workerStatus = ref<WorkerStatus | null>(null);
    const loading = ref(false);
    const error = ref<Error | null>(null);

    // Track timestamps for throttling progress events
    const lastProgressTimestamp = new Map<number | string, number>();

    // Event bus for real-time updates
    const { on } = useEventBus();

    // Telemetry state
    const throughputKbps = ref<number>(0);
    const artifactCounters = ref<{ audio: number; lrc: number; covers: number; booklets: number }>({
        audio: 0,
        lrc: 0,
        covers: 0,
        booklets: 0,
    });

    // Rolling progress samples for throughput calculation
    const progressSamples: { time: number; bytes: number }[] = [];
    const prevItemProgress = new Map<number | string, number>();

    // Computed
    const activeDownloads = computed(() =>
        queue.value.filter(q => q.status === 'downloading')
    );

    const queuedItems = computed(() =>
        queue.value.filter(q => q.status === 'queued')
    );

    const completedItems = computed(() =>
        queue.value.filter(q => q.status === 'completed' || q.status === 'complete')
    );

    const failedItems = computed(() =>
        queue.value.filter(q => q.status === 'failed')
    );

    // Reconciled counts
    const submittedCount = computed(() => stats.value?.submitted ?? queue.value.length);
    const queuedCount = computed(() => stats.value?.queued ?? queuedItems.value.length);
    const activeCount = computed(() => stats.value?.active ?? stats.value?.downloading ?? activeDownloads.value.length);
    const completedCount = computed(() => stats.value?.completed ?? completedItems.value.length);
    const failedCount = computed(() => stats.value?.failed ?? failedItems.value.length);
    const skippedCount = computed(() => stats.value?.skipped ?? 0);
    const deduplicatedCount = computed(() => stats.value?.deduplicated ?? 0);
    const physicalFilesCount = computed(() => stats.value?.physical_files ?? stats.value?.downloads_count ?? completedItems.value.length);

    const isWorkerPaused = computed(() =>
        workerStatus.value?.paused ?? workerStatus.value?.is_paused ?? false
    );

    const maxConcurrent = computed(() =>
        workerStatus.value?.max_concurrent ?? 3
    );

    const hasActiveDownloads = computed(() =>
        activeDownloads.value.length > 0
    );

    const successRate = computed<number>(() => {
        if (stats.value && typeof (stats.value as any).success_rate === 'number') {
            return Math.round((stats.value as any).success_rate * 10) / 10;
        }
        const finished = completedItems.value.length + failedItems.value.length;
        if (finished === 0) return 100.0;
        return Math.round((completedItems.value.length / finished) * 1000) / 10;
    });

    const formattedThroughput = computed<string>(() => {
        const kbps = throughputKbps.value;
        if (kbps <= 0 || activeDownloads.value.length === 0) return '0 KB/s';
        if (kbps >= 1024) {
            return `${(kbps / 1024).toFixed(1)} MB/s`;
        }
        return `${Math.round(kbps)} KB/s`;
    });

    const etaSeconds = computed<number | null>(() => {
        const activeCount = activeDownloads.value.length;
        const queuedCount = queuedItems.value.length;
        if (activeCount === 0 && queuedCount === 0) return 0;
        if (isWorkerPaused.value) return null;

        const avgTrackBytes = 25 * 1024 * 1024; // ~25MB average FLAC track
        const remainingActivePercent = activeDownloads.value.reduce((acc, item) => acc + (100 - (item.progress_percent || 0)), 0);
        const totalRemainingBytes = (queuedCount * avgTrackBytes) + ((remainingActivePercent / 100) * avgTrackBytes);

        const currentSpeedBytesPerSec = throughputKbps.value > 0 
            ? throughputKbps.value * 1024 
            : (activeCount > 0 ? 1.5 * 1024 * 1024 : 0);

        if (currentSpeedBytesPerSec <= 0) return null;

        const est = Math.ceil(totalRemainingBytes / currentSpeedBytesPerSec);
        return Math.max(1, est);
    });

    const formattedEta = computed<string>(() => {
        const s = etaSeconds.value;
        if (s === 0) return 'Completed';
        if (s === null) return activeDownloads.value.length > 0 ? 'Calculating...' : '--';
        if (s < 60) return `${s}s`;
        const mins = Math.floor(s / 60);
        const secs = s % 60;
        if (mins < 60) {
            return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`;
        }
        const hours = Math.floor(mins / 60);
        const remMins = mins % 60;
        return `${hours}h ${remMins}m`;
    });

    // Actions
    async function fetchQueue(statuses?: string[] | string, limit?: number): Promise<void> {
        loading.value = true;
        error.value = null;

        try {
            queue.value = await queueApi.getQueue(statuses, limit);
        } catch (e) {
            error.value = e instanceof Error ? e : new Error(String(e));
        } finally {
            loading.value = false;
        }
    }

    async function fetchStats(): Promise<void> {
        try {
            const res = await queueApi.getQueueStats();
            stats.value = res;
            if (res) {
                const r = res as any;
                if (typeof r.audio_count === 'number') artifactCounters.value.audio = r.audio_count;
                else if (typeof r.completed === 'number') artifactCounters.value.audio = r.completed;
                if (typeof r.lrc_count === 'number') artifactCounters.value.lrc = r.lrc_count;
                else if (typeof r.completed === 'number') artifactCounters.value.lrc = r.completed;
                if (typeof r.cover_count === 'number') artifactCounters.value.covers = r.cover_count;
                else if (typeof r.completed === 'number') artifactCounters.value.covers = r.completed;
                if (typeof r.booklet_count === 'number') artifactCounters.value.booklets = r.booklet_count;
            }
        } catch (e) {
            console.error('Failed to fetch queue stats:', e);
        }
    }

    async function fetchWorkerStatus(): Promise<void> {
        try {
            workerStatus.value = await queueApi.getWorkerStatus();
        } catch (e) {
            console.error('Failed to fetch worker status:', e);
        }
    }

    async function addToQueue(
        trackId: number,
        service: string,
        quality: string,
        priority?: number
    ): Promise<number> {
        const id = await queueApi.addToQueue({
            trackId,
            service,
            quality,
            priority,
        });

        // Refresh queue
        await fetchQueue();
        await fetchStats();

        return id;
    }

    async function addBatchToQueue(
        trackIds: number[],
        service?: string,
        quality?: string,
        priority?: number
    ): Promise<{ added: number; skipped: number }> {
        const res = await queueApi.addBatchToQueue({
            trackIds,
            service,
            quality,
            priority,
        });

        // Refresh queue
        await fetchQueue();
        await fetchStats();

        return res;
    }

    async function cancelItem(id: number): Promise<void> {
        await queueApi.cancelItem(id);
        lastProgressTimestamp.delete(id);
        prevItemProgress.delete(id);
        await fetchQueue();
        await fetchStats();
    }

    async function retryItem(id: number): Promise<void> {
        await queueApi.retryItem(id);
        lastProgressTimestamp.delete(id);
        prevItemProgress.delete(id);
        await fetchQueue();
        await fetchStats();
    }

    async function retryAllFailed(): Promise<number> {
        const result = await queueApi.retryAllFailed();
        lastProgressTimestamp.clear();
        prevItemProgress.clear();
        await fetchQueue();
        await fetchStats();
        // Extract number from message like "Requeued 5 failed downloads"
        const match = String(result).match(/(\d+)/);
        return match ? parseInt(match[1], 10) : 0;
    }

    async function clearCompleted(): Promise<number> {
        const count = await queueApi.clearQueue('complete');
        await fetchQueue();
        await fetchStats();
        return count;
    }

    async function clearFailed(): Promise<number> {
        const count = await queueApi.clearQueue('failed');
        await fetchQueue();
        await fetchStats();
        return count;
    }

    async function pauseDownloads(): Promise<void> {
        await queueApi.pauseDownloads();
        throughputKbps.value = 0;
        await fetchWorkerStatus();
    }

    async function resumeDownloads(): Promise<void> {
        await queueApi.resumeDownloads();
        await fetchWorkerStatus();
    }

    async function setMaxConcurrent(count: number): Promise<void> {
        const clamped = Math.max(1, Math.min(5, count));
        await queueApi.setMaxConcurrent(clamped);
        if (workerStatus.value) {
            workerStatus.value.max_concurrent = clamped;
        }
        await fetchWorkerStatus();
    }

    /**
     * Handle progress event with strict 4 updates/sec per track throttling and live speed calculation
     */
    function handleProgressEvent(event: any): void {
        if (!event) return;
        
        // Normalize event data (supports ProgressEvent and DownloadProgressEvent)
        const queueId = event.queue_id ? parseInt(String(event.queue_id), 10) : (event.id ? parseInt(String(event.id), 10) : undefined);
        if (queueId === undefined || isNaN(queueId)) return;

        const percentage = typeof event.progress_percent === 'number' ? event.progress_percent : (typeof event.percentage === 'number' ? event.percentage : 0);
        const status = event.status || 'downloading';
        const isTerminal = status === 'completed' || status === 'complete' || status === 'failed' || percentage >= 100;
        const isInitial = status === 'started' || percentage === 0;

        const now = Date.now();
        const lastTime = lastProgressTimestamp.get(queueId) || 0;

        // Calculate delta progress for throughput calculation
        const prevPerc = prevItemProgress.get(queueId) ?? 0;
        const deltaPerc = Math.max(0, percentage - prevPerc);
        prevItemProgress.set(queueId, percentage);

        const estTrackBytes = 25 * 1024 * 1024; // 25MB
        const deltaBytes = event.bytes_downloaded 
            ? (event.bytes_downloaded - (event.prev_bytes || 0)) 
            : (deltaPerc / 100) * estTrackBytes;

        if (deltaBytes > 0) {
            progressSamples.push({ time: now, bytes: deltaBytes });
        }

        // Prune samples older than 3.5 seconds
        const cutoff = now - 3500;
        while (progressSamples.length > 0 && progressSamples[0].time < cutoff) {
            progressSamples.shift();
        }

        // Calculate instant throughput in KB/s
        if (progressSamples.length > 1) {
            const durationSec = Math.max(0.5, (now - progressSamples[0].time) / 1000);
            const totalBytesInWindow = progressSamples.reduce((sum, s) => sum + s.bytes, 0);
            const instantKbps = (totalBytesInWindow / durationSec) / 1024;
            throughputKbps.value = Math.round(
                throughputKbps.value === 0 ? instantKbps : (throughputKbps.value * 0.65 + instantKbps * 0.35)
            );
        } else if (activeDownloads.value.length === 0) {
            throughputKbps.value = 0;
        }

        // Apply throttle for intermediate progress events (max 4 per sec = 250ms)
        if (!isTerminal && !isInitial && now - lastTime < PROGRESS_THROTTLE_MS) {
            return;
        }

        lastProgressTimestamp.set(queueId, now);

        if (isTerminal) {
            lastProgressTimestamp.delete(queueId);
            prevItemProgress.delete(queueId);
        }

        const item = queue.value.find(q => q.id === queueId);
        if (item) {
            item.progress_percent = percentage;

            if (status === 'completed' || status === 'complete') {
                item.status = 'complete';
                item.completed_at = new Date().toISOString();
                // Increment live artifact counters
                artifactCounters.value.audio += 1;
                artifactCounters.value.lrc += 1;
                artifactCounters.value.covers += 1;
                if (item.target_album && item.target_album.includes('Edition')) {
                    artifactCounters.value.booklets += 1;
                }
            } else if (status === 'failed') {
                item.status = 'failed';
                item.error_message = event.message || event.error || 'Download failed';
            } else if (status === 'started' || status === 'downloading') {
                item.status = 'downloading';
            }
        }
    }

    // Initialize
    async function initialize(): Promise<void> {
        await Promise.all([
            fetchQueue(),
            fetchStats(),
            fetchWorkerStatus(),
        ]);

        // Subscribe to progress events
        on<ProgressEvent>(TauriEvents.DOWNLOAD_PROGRESS, handleProgressEvent);
    }

    return {
        // State
        queue,
        stats,
        workerStatus,
        loading,
        error,
        throughputKbps,
        artifactCounters,

        // Computed
        activeDownloads,
        queuedItems,
        completedItems,
        failedItems,
        submittedCount,
        queuedCount,
        activeCount,
        completedCount,
        failedCount,
        skippedCount,
        deduplicatedCount,
        physicalFilesCount,
        isWorkerPaused,
        maxConcurrent,
        hasActiveDownloads,
        successRate,
        formattedThroughput,
        etaSeconds,
        formattedEta,

        // Actions
        fetchQueue,
        fetchStats,
        fetchWorkerStatus,
        addToQueue,
        addBatchToQueue,
        cancelItem,
        retryItem,
        retryAllFailed,
        clearCompleted,
        clearFailed,
        pauseDownloads,
        resumeDownloads,
        setMaxConcurrent,
        handleProgressEvent,
        initialize,
    };
}


