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

    const isWorkerPaused = computed(() =>
        workerStatus.value?.paused ?? workerStatus.value?.is_paused ?? false
    );

    const maxConcurrent = computed(() =>
        workerStatus.value?.max_concurrent ?? 3
    );

    const hasActiveDownloads = computed(() =>
        activeDownloads.value.length > 0
    );

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
            stats.value = await queueApi.getQueueStats();
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
        await fetchQueue();
        await fetchStats();
    }

    async function retryItem(id: number): Promise<void> {
        await queueApi.retryItem(id);
        lastProgressTimestamp.delete(id);
        await fetchQueue();
        await fetchStats();
    }

    async function retryAllFailed(): Promise<number> {
        const result = await queueApi.retryAllFailed();
        lastProgressTimestamp.clear();
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
     * Handle progress event with strict 4 updates/sec per track throttling
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

        // Apply throttle for intermediate progress events (max 4 per sec = 250ms)
        if (!isTerminal && !isInitial && now - lastTime < PROGRESS_THROTTLE_MS) {
            return;
        }

        lastProgressTimestamp.set(queueId, now);

        if (isTerminal) {
            lastProgressTimestamp.delete(queueId);
        }

        const item = queue.value.find(q => q.id === queueId);
        if (item) {
            item.progress_percent = percentage;

            if (status === 'completed' || status === 'complete') {
                item.status = 'complete';
                item.completed_at = new Date().toISOString();
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

        // Computed
        activeDownloads,
        queuedItems,
        completedItems,
        failedItems,
        isWorkerPaused,
        maxConcurrent,
        hasActiveDownloads,

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

