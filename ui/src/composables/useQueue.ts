/**
 * Queue Composable
 * 
 * State management for download queue and worker.
 */

import { ref, computed } from 'vue';
import { queueApi } from '@/api/queue';
import { useEventBus, TauriEvents } from './useEventBus';
import type { QueueItem, QueueStats, WorkerStatus, ProgressEvent } from '@/api/types';

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
        queue.value.filter(q => q.status === 'completed')
    );

    const failedItems = computed(() =>
        queue.value.filter(q => q.status === 'failed')
    );

    const isWorkerPaused = computed(() =>
        workerStatus.value?.is_paused ?? false
    );

    const hasActiveDownloads = computed(() =>
        activeDownloads.value.length > 0
    );

    // Actions
    async function fetchQueue(statuses?: string[]): Promise<void> {
        loading.value = true;
        error.value = null;

        try {
            queue.value = await queueApi.getQueue(statuses);
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
        service: string,
        quality: string,
        priority?: number
    ): Promise<number[]> {
        const ids = await queueApi.addBatchToQueue({
            trackIds,
            service,
            quality,
            priority,
        });

        // Refresh queue
        await fetchQueue();
        await fetchStats();

        return ids;
    }

    async function cancelItem(id: number): Promise<void> {
        await queueApi.cancelItem(id);
        await fetchQueue();
        await fetchStats();
    }

    async function retryItem(id: number): Promise<void> {
        await queueApi.retryItem(id);
        await fetchQueue();
        await fetchStats();
    }

    async function retryAllFailed(): Promise<number> {
        const result = await queueApi.retryAllFailed();
        await fetchQueue();
        await fetchStats();
        // Extract number from message like "Requeued 5 failed downloads"
        const match = result.match(/(\d+)/);
        return match ? parseInt(match[1], 10) : 0;
    }

    async function clearCompleted(): Promise<number> {
        const count = await queueApi.clearQueue('completed');
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
        await queueApi.setMaxConcurrent(count);
        await fetchWorkerStatus();
    }

    // Handle progress event
    function handleProgressEvent(event: ProgressEvent): void {
        if (event.operation !== 'download') return;

        const item = queue.value.find(q => q.id === parseInt(event.id));
        if (item) {
            item.progress_percent = event.percentage;

            if (event.status === 'completed') {
                item.status = 'completed';
            } else if (event.status === 'failed') {
                item.status = 'failed';
                item.error_message = event.message || 'Download failed';
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
        initialize,
    };
}
