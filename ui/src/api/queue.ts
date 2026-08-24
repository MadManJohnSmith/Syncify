/**
 * Queue API
 * 
 * Tauri commands for download queue management.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asRecord, pick, pickNumber, pickArray, optionalNumber, optionalBoolean } from './normalize';
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
    const queued = await invokeCommand<unknown>('add_to_queue', {
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
    return typeof queued === 'number' ? queued : 0;
}

/**
 * Force redownload tracks (clears from downloads and queue, then re-queues)
 */
export async function forceRedownloadTracks(
    trackIds: number[],
    priority?: number,
    qualityPreference?: string
): Promise<number> {
    const queued = await invokeCommand<unknown>('force_redownload_tracks', {
        trackIds,
        priority,
        qualityPreference: qualityPreference ? normalizeQuality(qualityPreference) : undefined,
    });
    return typeof queued === 'number' ? queued : 0;
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
        const legacyQueued = await invokeCommand<unknown>('enqueue_download', {
            trackId: params,
            priority: legacyPriority,
            qualityPreference: normalizeQuality(legacyQuality),
            allowFallback: true,
        });
        return typeof legacyQueued === 'number' ? legacyQueued : 0;
    }
    const queued = await invokeCommand<unknown>('enqueue_download', {
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
    return typeof queued === 'number' ? queued : 0;
}

function normalizePreflightSummary(raw: unknown): PreflightSummaryCounts {
    return {
        requested_total: pickNumber(raw, ['requested_total', 'requestedTotal']),
        eligible_total: pickNumber(raw, ['eligible_total', 'eligibleTotal']),
        ready_exact: pickNumber(raw, ['ready_exact', 'readyExact']),
        ready_fallback: pickNumber(raw, ['ready_fallback', 'readyFallback']),
        already_downloaded: pickNumber(raw, ['already_downloaded', 'alreadyDownloaded']),
        already_queued: pickNumber(raw, ['already_queued', 'alreadyQueued']),
        no_download_provider: pickNumber(raw, ['no_download_provider', 'noDownloadProvider']),
        ambiguous_source: pickNumber(raw, ['ambiguous_source', 'ambiguousSource']),
        rejected_quality: pickNumber(raw, ['rejected_quality', 'rejectedQuality']),
        stale_source: pickNumber(raw, ['stale_source', 'staleSource']),
        requires_auth: pickNumber(raw, ['requires_auth', 'requiresAuth']),
        network_retryable: pickNumber(raw, ['network_retryable', 'networkRetryable']),
    };
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
    const raw = await invokeCommand<unknown>('preflight_download_batch', {
        trackIds: params.trackIds,
        serviceName: params.serviceName || params.service || undefined,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        strictQuality: params.strictQuality ?? false,
        allowFallback: params.allowFallback ?? true,
    });
    return {
        summary: normalizePreflightSummary(pick(raw, ['summary'])),
        tracks: pickArray<TrackPreflightResult>(raw, ['tracks']),
        estimated_size_mb: pickNumber(raw, ['estimated_size_mb', 'estimatedSizeMb']),
    };
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
    const raw = await invokeCommand<unknown>('enqueue_eligible_batch', {
        trackIds: params.trackIds,
        priority: params.priority,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        serviceName: params.serviceName || params.service || undefined,
        strictQuality: params.strictQuality ?? false,
        allowFallback: params.allowFallback ?? true,
        smartStudioOrigin: params.smartStudioOrigin,
    });
    return {
        submitted: pickNumber(raw, ['submitted']),
        added: pickNumber(raw, ['added']),
        enqueued: pickNumber(raw, ['enqueued']),
        deduplicated: pickNumber(raw, ['deduplicated']),
        skipped: pickNumber(raw, ['skipped']),
        summary: normalizePreflightSummary(pick(raw, ['summary'])),
        tracks: pickArray<TrackPreflightResult>(raw, ['tracks']),
    };
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
    const raw = await invokeCommand<unknown>('add_batch_to_queue', {
        trackIds: params.trackIds,
        priority: params.priority,
        qualityPreference: normalizeQuality(params.qualityPreference || params.quality),
        serviceName: params.serviceName || params.service || undefined,
        smartStudioOrigin: params.smartStudioOrigin,
        allowFallback: params.allowFallback ?? true,
    });
    const summary = pick(raw, ['summary']);
    return {
        added: pickNumber(raw, ['added']),
        skipped: pickNumber(raw, ['skipped']),
        summary: asRecord(summary) ? normalizePreflightSummary(summary) : undefined,
    };
}

/**
 * Get all items in the queue
 */
export async function getQueue(statuses?: string[] | string, limit?: number): Promise<QueueItem[]> {
    const statusFilter = Array.isArray(statuses) ? (statuses.length > 0 ? statuses[0] : undefined) : statuses;
    const raw = await invokeCommand<unknown>('get_queue', { statusFilter, statuses, limit });
    // Array identity is preserved intentionally: consumers mutate fetched queue
    // items (live progress updates) across polling refreshes.
    return asArray<QueueItem>(raw);
}

/**
 * Normalize queue statistics so every rendered counter has a safe numeric default.
 */
function normalizeQueueStats(raw: unknown): QueueStats {
    return {
        total: pickNumber(raw, ['total']),
        queued: pickNumber(raw, ['queued']),
        downloading: pickNumber(raw, ['downloading']),
        // Canonical backend key is 'completed'; older payloads used 'complete'.
        completed: pickNumber(raw, ['completed', 'complete']),
        failed: pickNumber(raw, ['failed']),
        paused: pickNumber(raw, ['paused']),
        cancelled: optionalNumber(pick(raw, ['cancelled'])),
        submitted: optionalNumber(pick(raw, ['submitted'])),
        active: optionalNumber(pick(raw, ['active'])),
        skipped: optionalNumber(pick(raw, ['skipped'])),
        deduplicated: optionalNumber(pick(raw, ['deduplicated'])),
        physical_files: optionalNumber(pick(raw, ['physical_files', 'physicalFiles'])),
        downloads_count: optionalNumber(pick(raw, ['downloads_count', 'downloadsCount'])),
        success_rate: optionalNumber(pick(raw, ['success_rate', 'successRate'])),
        audio_count: optionalNumber(pick(raw, ['audio_count', 'audioCount'])),
        lrc_count: optionalNumber(pick(raw, ['lrc_count', 'lrcCount'])),
        cover_count: optionalNumber(pick(raw, ['cover_count', 'coverCount'])),
        booklet_count: optionalNumber(pick(raw, ['booklet_count', 'bookletCount'])),
    };
}

/**
 * Get queue statistics
 */
export async function getQueueStats(): Promise<QueueStats> {
    return normalizeQueueStats(await invokeCommand<unknown>('get_queue_stats'));
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
    const cleared = await invokeCommand<unknown>('clear_queue', { status });
    return typeof cleared === 'number' ? cleared : 0;
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
    const restored = await invokeCommand<unknown>('restore_interrupted_downloads');
    return typeof restored === 'number' ? restored : 0;
}

// ==============================================
// WORKER CONTROL
// ==============================================

/**
 * Get worker status
 */
export async function getWorkerStatus(): Promise<WorkerStatus> {
    const raw = await invokeCommand<unknown>('get_worker_status');
    return {
        running: pick(raw, ['running']) === true,
        paused: pick(raw, ['paused']) === true,
        active_downloads: pickNumber(raw, ['active_downloads', 'activeDownloads']),
        max_concurrent: pickNumber(raw, ['max_concurrent', 'maxConcurrent']),
        is_running: optionalBoolean(pick(raw, ['is_running', 'isRunning'])),
        is_paused: optionalBoolean(pick(raw, ['is_paused', 'isPaused'])),
        current_downloads: optionalNumber(pick(raw, ['current_downloads', 'currentDownloads'])),
        total_processed: optionalNumber(pick(raw, ['total_processed', 'totalProcessed'])),
        total_failed: optionalNumber(pick(raw, ['total_failed', 'totalFailed'])),
    };
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
  | 'auth'                // Authentication required (401/403)
  | 'requires_auth'       // Alias for auth
  | 'entitlement'         // Subscription / entitlement restricted
  | 'quality'             // Quality preference rejected
  | 'rejected_quality'    // Alias for quality
  | 'unavailable'         // Stale source / 404
  | 'stale_source'        // Alias for unavailable
  | 'validation'          // Audio validation failed
  | 'tagging'             // Tagging / embedding error
  | 'filesystem'          // Filesystem / storage / IO error
  | 'network'             // Network retry exhausted
  | 'cancelled'           // Cancelled by user
  | 'ambiguous_source'    // Ambiguous candidate match
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

  // 1. Authentication (401, 403, unauthorized, token expired, forbidden, login required)
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
    text.includes('auth failed') ||
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

  // 2. Entitlement (subscription tier, geo-restricted, country restriction)
  if (
    text.includes('entitlement') ||
    text.includes('subscription') ||
    text.includes('premium required') ||
    text.includes('tier restricted') ||
    text.includes('geo-restricted') ||
    text.includes('country restricted') ||
    text.includes('region restricted') ||
    text.includes('licensing restricted')
  ) {
    return {
      reason: 'entitlement',
      label: 'Entitlement restricted',
      description: 'Track is not permitted under the current subscription tier or region.',
      badgeClass: 'bg-amber-500/10 text-amber-500 border border-amber-500/30',
      icon: 'verified_user',
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

  // 4. Stale Source / 404 / Unavailable (StaleSource, 404, not found, resource missing, deleted from catalog)
  if (
    text.includes('stalesource') ||
    text.includes('stale_source') ||
    text.includes('unavailable') ||
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

  // 5. Audio Validation Failed
  if (
    text.includes('validation') ||
    text.includes('corrupted') ||
    text.includes('invalid audio') ||
    text.includes('invalid flac') ||
    text.includes('flac header') ||
    text.includes('decode error') ||
    text.includes('audio header')
  ) {
    return {
      reason: 'validation',
      label: 'Audio validation failed',
      description: 'Downloaded audio file failed integrity or header validation check.',
      badgeClass: 'bg-red-500/10 text-red-500 border border-red-500/30',
      icon: 'waveform',
      isRetryableOriginal: true,
      canUseFallback: true,
      requiresAuth: false,
    };
  }

  // 6. Tagging / Metadata Embedding Failed
  if (
    text.includes('tagging') ||
    text.includes('mutagen') ||
    text.includes('id3') ||
    text.includes('vorbis') ||
    text.includes('metadata embed') ||
    text.includes('embed artwork')
  ) {
    return {
      reason: 'tagging',
      label: 'Tagging failed',
      description: 'Failed to write ID3/FLAC metadata tags or artwork to the file.',
      badgeClass: 'bg-indigo-500/10 text-indigo-500 border border-indigo-500/30',
      icon: 'label_off',
      isRetryableOriginal: true,
      canUseFallback: false,
      requiresAuth: false,
    };
  }

  // 7. Filesystem / Storage / Promotion Failed
  if (
    text.includes('filesystem') ||
    text.includes('permission denied') ||
    text.includes('disk full') ||
    text.includes('io error') ||
    text.includes('moving to library') ||
    text.includes('promotion failed') ||
    text.includes('cannot move') ||
    text.includes('path not found')
  ) {
    return {
      reason: 'filesystem',
      label: 'Filesystem error',
      description: 'Failed writing or moving audio file into destination library directory.',
      badgeClass: 'bg-amber-600/10 text-amber-600 border border-amber-600/30',
      icon: 'folder_off',
      isRetryableOriginal: true,
      canUseFallback: false,
      requiresAuth: false,
    };
  }

  // 8. Cancelled by user
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

  // 9. Ambiguous Source (multiple candidate tracks, mismatch, or loose title/artist only fallback)
  if (
    text.includes('ambiguoussource') ||
    text.includes('ambiguous_source') ||
    text.includes('multiple matches') ||
    text.includes('ambiguous') ||
    text.includes('sourceidentitymissing') ||
    text.includes('identityconflict')
  ) {
    return {
      reason: 'ambiguous_source',
      label: 'Ambiguous source',
      description:
        'Cannot download automatically: Tidal fallback matched title/artist only without edition identity (ISRC/MBID/AcoustID). Choose source manually or enrich metadata.',
      badgeClass: 'bg-yellow-500/10 text-yellow-500 border border-yellow-500/30',
      icon: 'alt_route',
      isRetryableOriginal: false,
      canUseFallback: false,
      requiresAuth: false,
    };
  }

  // 10. Network retry exhausted / transient stream errors
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
      canUseFallback: false,
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


