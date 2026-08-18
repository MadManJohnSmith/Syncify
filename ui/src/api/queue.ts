/**
 * Queue API
 * 
 * Tauri commands for download queue management.
 */

import { invokeCommand } from './tauri';
import type {
    QueueItem,
    QueueStats,
    WorkerStatus,
    DownloadItem,
    PreflightBatchResponse,
    BatchEnqueueResult,
    TrackPreflightResult,
    PreflightSummaryCounts,
    DownloadPreflightStatus,
} from './types';

export type {
    PreflightBatchResponse,
    BatchEnqueueResult,
    TrackPreflightResult,
    PreflightSummaryCounts,
    DownloadPreflightStatus,
};

/**
 * Normalize quality labels to DB-valid values.
 * DB CHECK constraint accepts: 'hires', 'lossless', 'high', 'any', or NULL.
 */
const QUALITY_MAP: Record<string, string> = {
    'HI_RES_LOSSLESS': 'hires',
    'HI_RES': 'hires',
    '24-96': 'hires',
    '24-192': 'hires',
    'LOSSLESS': 'lossless',
    '16-44': 'lossless',
    'HIGH': 'high',
    'ANY': 'any',
    // Pass through already-valid values
    'hires': 'hires',
    'lossless': 'lossless',
    'high': 'high',
    'any': 'any',
};

function normalizeQuality(q?: string | null): string | undefined {
    if (!q) return undefined;
    return QUALITY_MAP[q] || QUALITY_MAP[q.toUpperCase()] || 'hires';
}

/**
 * Add a track to the download queue with source identity locking
 */
export async function addToQueue(params: {
    trackId: number;
    priority?: number;
    qualityPreference?: string;
    serviceId?: number;
    serviceName?: string;
    serviceTrackId?: string;
    serviceAlbumId?: string;
    targetTitle?: string;
    targetArtist?: string;
    targetAlbum?: string;
    targetIsrc?: string;
    smartStudioOrigin?: boolean;
    allowFallback?: boolean;
    // Legacy compatibility fields
    service?: string;
    quality?: string;
}): Promise<number> {
    return invokeCommand<number>('add_to_queue', {
        trackId: params.trackId,
        priority: params.priority,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        serviceId: params.serviceId,
        serviceName: params.serviceName || params.service || undefined,
        serviceTrackId: params.serviceTrackId || undefined,
        serviceAlbumId: params.serviceAlbumId || undefined,
        targetTitle: params.targetTitle || undefined,
        targetArtist: params.targetArtist || undefined,
        targetAlbum: params.targetAlbum || undefined,
        targetIsrc: params.targetIsrc || undefined,
        smartStudioOrigin: params.smartStudioOrigin,
        allowFallback: params.allowFallback ?? true,
    });
}

/**
 * Force redownload tracks (clears from downloads and queue, then re-queues)
 */
export async function forceRedownloadTracks(
    trackIds: number[],
    priority?: number,
    qualityPreference?: string
): Promise<number> {
    return invokeCommand<number>('force_redownload_tracks', {
        trackIds,
        priority,
        qualityPreference: qualityPreference ? normalizeQuality(qualityPreference) : undefined,
    });
}

/**
 * Enqueue a track for download (canonical command)
 */
export async function enqueueDownload(params: {
    trackId: number;
    priority?: number;
    qualityPreference?: string;
    quality?: string;
    serviceId?: number;
    serviceName?: string;
    service?: string;
    serviceTrackId?: string;
    serviceAlbumId?: string;
    targetTitle?: string;
    targetArtist?: string;
    targetAlbum?: string;
    targetIsrc?: string;
    smartStudioOrigin?: boolean;
    allowFallback?: boolean;
    outputDir?: string | null;
} | number, legacyPriority?: number, legacyQuality?: string): Promise<number> {
    if (typeof params === 'number') {
        return invokeCommand<number>('enqueue_download', {
            trackId: params,
            priority: legacyPriority,
            qualityPreference: normalizeQuality(legacyQuality),
            allowFallback: true,
        });
    }
    return invokeCommand<number>('enqueue_download', {
        trackId: params.trackId,
        priority: params.priority,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        quality: normalizeQuality(params.quality || params.qualityPreference),
        serviceId: params.serviceId,
        serviceName: params.serviceName || params.service,
        service: params.service || params.serviceName,
        serviceTrackId: params.serviceTrackId,
        serviceAlbumId: params.serviceAlbumId,
        targetTitle: params.targetTitle,
        targetArtist: params.targetArtist,
        targetAlbum: params.targetAlbum,
        targetIsrc: params.targetIsrc,
        smartStudioOrigin: params.smartStudioOrigin,
        allowFallback: params.allowFallback ?? true,
        outputDir: params.outputDir,
    });
}

/**
 * Run preflight analysis on a batch of tracks without downloading audio (S138A)
 */
export async function preflightDownloadBatch(params: {
    trackIds: number[];
    serviceName?: string;
    qualityPreference?: string;
    strictQuality?: boolean;
    allowFallback?: boolean;
    // Legacy compatibility fields
    service?: string;
    quality?: string;
}): Promise<PreflightBatchResponse> {
    return invokeCommand<PreflightBatchResponse>('preflight_download_batch', {
        trackIds: params.trackIds,
        serviceName: params.serviceName || params.service || undefined,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        strictQuality: params.strictQuality ?? false,
        allowFallback: params.allowFallback ?? true,
    });
}

/**
 * Enqueue ONLY eligible tracks evaluated by preflight (ReadyExactSource and ReadyFallbackExactIdentity)
 */
export async function enqueueEligibleBatch(params: {
    trackIds: number[];
    priority?: number;
    qualityPreference?: string;
    serviceName?: string;
    strictQuality?: boolean;
    allowFallback?: boolean;
    smartStudioOrigin?: boolean;
    // Legacy compatibility fields
    service?: string;
    quality?: string;
}): Promise<BatchEnqueueResult> {
    return invokeCommand<BatchEnqueueResult>('enqueue_eligible_batch', {
        trackIds: params.trackIds,
        priority: params.priority,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        serviceName: params.serviceName || params.service || undefined,
        strictQuality: params.strictQuality ?? false,
        allowFallback: params.allowFallback ?? true,
        smartStudioOrigin: params.smartStudioOrigin,
    });
}

/**
 * Add multiple tracks to the download queue using safe preflight
 */
export async function addBatchToQueue(params: {
    trackIds: number[];
    serviceName?: string;
    qualityPreference?: string;
    priority?: number;
    smartStudioOrigin?: boolean;
    allowFallback?: boolean;
    // Legacy compatibility fields
    service?: string;
    quality?: string;
}): Promise<{ added: number; skipped: number; summary?: PreflightSummaryCounts }> {
    return invokeCommand<{ added: number; skipped: number; summary?: PreflightSummaryCounts }>('add_batch_to_queue', {
        trackIds: params.trackIds,
        priority: params.priority,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        serviceName: params.serviceName || params.service || undefined,
        smartStudioOrigin: params.smartStudioOrigin,
        allowFallback: params.allowFallback ?? true,
    });
}

/**
 * Get all items in the queue
 */
export async function getQueue(statuses?: string[] | string, limit?: number): Promise<QueueItem[]> {
    const statusFilter = Array.isArray(statuses) ? (statuses.length > 0 ? statuses[0] : undefined) : statuses;
    return invokeCommand<QueueItem[]>('get_queue', { statusFilter, statuses, limit });
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
export async function setMaxConcurrent(count: number): Promise<void> {
    return invokeCommand<void>('set_max_concurrent_downloads', { max: count });
}

/**
 * Start worker (alias for resume)
 */
export async function startWorker(): Promise<void> {
    return invokeCommand<void>('start_worker');
}

/**
 * Resume worker (alias)
 */
export async function resumeWorker(): Promise<void> {
    return invokeCommand<void>('resume_worker');
}

/**
 * Pause worker (alias)
 */
export async function pauseWorker(): Promise<void> {
    return invokeCommand<void>('pause_worker');
}

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

// ==============================================
// FAILURE CLASSIFICATION & TELEMETRY
// ==============================================

export type FailureReason = 
  | 'network'            // Network retry exhausted
  | 'stale_source'        // Stale source / 404
  | 'requires_auth'       // Requires authentication (401/403)
  | 'rejected_quality'    // Rejected quality
  | 'cancelled'           // Cancelled
  | 'ambiguous_source'    // Ambiguous source
  | 'unknown';            // General / unknown error

export interface FailureInfo {
  reason: FailureReason;
  label: string;
  description: string;
  badgeClass: string;
  icon: string;
  isRetryableOriginal: boolean;
  canUseFallback: boolean;
  requiresAuth: boolean;
}

/**
 * Classify error message into standardized failure categories.
 */
export function classifyFailureReason(errorMessage?: string | null, lastError?: string | null): FailureInfo {
  const text = `${errorMessage || ''} ${lastError || ''}`.toLowerCase();

  // 1. Requires Authentication (401, 403, unauthorized, token expired, forbidden, login required)
  if (
    text.includes('requiresauth') ||
    text.includes('requires_auth') ||
    text.includes('unauthorized') ||
    text.includes('forbidden') ||
    text.includes('401') ||
    text.includes('403') ||
    text.includes('session expired') ||
    text.includes('invalid token') ||
    text.includes('login required') ||
    text.includes('auth error') ||
    text.includes('authentication required')
  ) {
    return {
      reason: 'requires_auth',
      label: 'Requires authentication',
      description: 'Account credentials expired or invalid. Please check your account settings.',
      badgeClass: 'bg-rose-500/10 text-rose-500 border border-rose-500/30',
      icon: 'lock',
      isRetryableOriginal: false,
      canUseFallback: false,
      requiresAuth: true,
    };
  }

  // 2. Stale Source / 404 (StaleSource, 404, not found, resource missing, deleted from catalog)
  if (
    text.includes('stalesource') ||
    text.includes('stale_source') ||
    text.includes('404') ||
    text.includes('not found') ||
    text.includes('track not found') ||
    text.includes('source missing') ||
    text.includes('resource not found') ||
    text.includes('sourceidentitymissing') ||
    text.includes('no source track')
  ) {
    return {
      reason: 'stale_source',
      label: 'Stale source / 404',
      description: 'The source track or stream URL is no longer available on this service.',
      badgeClass: 'bg-orange-500/10 text-orange-500 border border-orange-500/30',
      icon: 'link_off',
      isRetryableOriginal: false,
      canUseFallback: true,
      requiresAuth: false,
    };
  }

  // 3. Rejected Quality (quality mismatch, requested bitrate/format unavailable)
  if (
    text.includes('rejectedquality') ||
    text.includes('rejected_quality') ||
    (text.includes('quality') && (text.includes('reject') || text.includes('unavailable') || text.includes('not available') || text.includes('too low') || text.includes('mismatch')))
  ) {
    return {
      reason: 'rejected_quality',
      label: 'Rejected quality',
      description: 'Requested audio quality or format is unavailable for this track.',
      badgeClass: 'bg-purple-500/10 text-purple-500 border border-purple-500/30',
      icon: 'high_quality',
      isRetryableOriginal: false,
      canUseFallback: true,
      requiresAuth: false,
    };
  }

  // 4. Cancelled by user
  if (
    text.includes('cancelled') ||
    text.includes('canceled') ||
    text.includes('user cancelled') ||
    text.includes('aborted')
  ) {
    return {
      reason: 'cancelled',
      label: 'Cancelled',
      description: 'Download was cancelled by user.',
      badgeClass: 'bg-gray-500/10 text-gray-400 border border-gray-500/30',
      icon: 'cancel',
      isRetryableOriginal: true,
      canUseFallback: false,
      requiresAuth: false,
    };
  }

  // 5. Ambiguous Source (multiple candidate tracks, mismatch)
  if (
    text.includes('ambiguoussource') ||
    text.includes('ambiguous_source') ||
    text.includes('multiple matches') ||
    text.includes('ambiguous')
  ) {
    return {
      reason: 'ambiguous_source',
      label: 'Ambiguous source',
      description: 'Multiple matching tracks found; manual selection required.',
      badgeClass: 'bg-yellow-500/10 text-yellow-500 border border-yellow-500/30',
      icon: 'alt_route',
      isRetryableOriginal: false,
      canUseFallback: true,
      requiresAuth: false,
    };
  }

  // 6. Network retry exhausted / transient stream errors
  if (
    text.includes('network') ||
    text.includes('timeout') ||
    text.includes('timed out') ||
    text.includes('connection reset') ||
    text.includes('error decoding response body') ||
    text.includes('stream') ||
    text.includes('retry exhausted') ||
    text.includes('socket') ||
    text.includes('eof') ||
    text.includes('broken pipe') ||
    text.includes('http') ||
    text.includes('request failed') ||
    text.includes('502') ||
    text.includes('503') ||
    text.includes('504')
  ) {
    return {
      reason: 'network',
      label: 'Network retry exhausted',
      description: 'Transient network or CDN error. Connection was interrupted.',
      badgeClass: 'bg-blue-500/10 text-blue-500 border border-blue-500/30',
      icon: 'wifi_off',
      isRetryableOriginal: true,
      canUseFallback: false, // For network error: no automatic "try another service"
      requiresAuth: false,
    };
  }

  // Default unknown
  return {
    reason: 'unknown',
    label: errorMessage ? (errorMessage.length > 30 ? `${errorMessage.slice(0, 30)}...` : errorMessage) : 'Download failed',
    description: errorMessage || 'An unexpected error occurred during download.',
    badgeClass: 'bg-red-500/10 text-red-500 border border-red-500/30',
    icon: 'error',
    isRetryableOriginal: true,
    canUseFallback: true,
    requiresAuth: false,
  };
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
    startWorker,
    resumeWorker,
    pauseWorker,
    setMaxConcurrent,
    downloadTidalSingleTrack,
    forceRedownloadTracks,
    classifyFailureReason,
};


