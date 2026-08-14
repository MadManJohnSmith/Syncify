/**
 * Queue API
 * 
 * Tauri commands for download queue management.
 */

import { invokeCommand } from './tauri';
import type { QueueItem, QueueStats, WorkerStatus, DownloadItem } from './types';

/**
 * Add a track to the download queue
 */
export async function addToQueue(params: {
    trackId: number;
    service: string;
    quality: string;
    priority?: number;
}): Promise<number> {
    return invokeCommand<number>('add_to_queue', params);
}

/**
 * Add multiple tracks to the download queue
 */
export async function addBatchToQueue(params: {
    trackIds: number[];
    service: string;
    quality: string;
    priority?: number;
}): Promise<number[]> {
    return invokeCommand<number[]>('add_batch_to_queue', params);
}

/**
 * Get all items in the queue
 */
export async function getQueue(statuses?: string[]): Promise<QueueItem[]> {
    return invokeCommand<QueueItem[]>('get_queue', { statuses });
}

/**
 * Get queue statistics
 */
export async function getQueueStats(): Promise<QueueStats> {
    return invokeCommand<QueueStats>('get_queue_stats');
}

/**
 * Get download queue (legacy)
 */
export async function getDownloadQueue(): Promise<DownloadItem[]> {
    return invokeCommand<DownloadItem[]>('get_download_queue');
}

/**
 * Get failed downloads
 */
export async function getFailedDownloads(): Promise<DownloadItem[]> {
    return invokeCommand<DownloadItem[]>('get_failed_downloads');
}

/**
 * Update queue item priority
 */
export async function updatePriority(id: number, priority: number): Promise<void> {
    return invokeCommand<void>('update_queue_priority', { queueId: id, priority });
}


/**
 * Cancel a queue item
 */
export async function cancelItem(id: number): Promise<void> {
    return invokeCommand<void>('cancel_queue_item', { queueId: id });
}


/**
 * Retry a failed queue item
 */
export async function retryItem(id: number): Promise<void> {
    return invokeCommand<void>('retry_queue_item', { queueId: id });
}


/**
 * Retry all failed items
 */
export async function retryAllFailed(): Promise<string> {
    return invokeCommand<string>('retry_failed_downloads');
}

/**
 * Clear all failed downloads
 */
export async function clearAllFailed(): Promise<string> {
    return invokeCommand<string>('clear_failed_downloads');
}

/**
 * Clear queue items by status
 */
export async function clearQueue(status?: string): Promise<number> {
    return invokeCommand<number>('clear_queue', { status });
}

/**
 * Remove specific item from queue
 */
export async function removeFromQueue(id: number): Promise<void> {
    return invokeCommand<void>('remove_from_queue', { queueId: id });
}


/**
 * Restore interrupted downloads
 */
export async function restoreInterrupted(): Promise<number> {
    return invokeCommand<number>('restore_interrupted_downloads');
}

// ==============================================
// WORKER CONTROL
// ==============================================

/**
 * Get worker status
 */
export async function getWorkerStatus(): Promise<WorkerStatus> {
    return invokeCommand<WorkerStatus>('get_worker_status');
}

/**
 * Pause all downloads
 */
export async function pauseDownloads(): Promise<void> {
    return invokeCommand<void>('pause_downloads');
}

/**
 * Resume all downloads
 */
export async function resumeDownloads(): Promise<void> {
    return invokeCommand<void>('resume_downloads');
}

/**
 * Set max concurrent downloads
 */
/**
 * Download a single track directly from Tidal with full pipeline
 */
export async function downloadTidalSingleTrack(params: {
    trackIdOrQuery: string;
    quality?: string;
    outputDir?: string;
    allowFallback?: boolean;
}): Promise<import('./types').TidalSingleTrackResponse> {
    return invokeCommand<import('./types').TidalSingleTrackResponse>('download_tidal_single_track', {
        trackIdOrQuery: params.trackIdOrQuery,
        quality: params.quality,
        outputDir: params.outputDir,
        allowFallback: params.allowFallback,
    });
}

// Export as namespace
export const queueApi = {
    addToQueue,
    addBatchToQueue,
    getQueue,
    getQueueStats,
    getDownloadQueue,
    getFailedDownloads,
    updatePriority,
    cancelItem,
    retryItem,
    retryAllFailed,
    clearAllFailed,
    clearQueue,
    removeFromQueue,
    restoreInterrupted,
    getWorkerStatus,
    pauseDownloads,
    resumeDownloads,
    setMaxConcurrent,
    downloadTidalSingleTrack,
};

