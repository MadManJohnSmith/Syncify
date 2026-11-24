// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
// ==============================================
// DOWNLOAD TYPES
// ==============================================

export interface DownloadItem {
    id: number;
    title: string;
    artist_name: string;
    status: 'queued' | 'downloading' | 'completed' | 'failed' | 'cancelled' | 'paused';
    progress_percent: number;
}

export interface QueueItem {
    id: number;
    track_id: number;
    title: string | null;
    artist: string | null;
    service_id?: number | null;
    service_name?: string | null;
    service_track_id?: string | null;
    service_album_id?: string | null;
    target_title?: string | null;
    target_artist?: string | null;
    target_album?: string | null;
    target_isrc?: string | null;
    quality_preference?: string | null;
    service?: string;
    quality?: string;
    status: string;
    priority: number;
    progress_percent: number;
    /** D00/D04: account that served this row, when resolved. */
    resolved_account_id?: number | null;
    /** Optional display label returned alongside the resolved account. */
    resolved_account_name?: string | null;
    error_message: string | null;
    last_error?: string | null;
    retry_count?: number;
    effective_service?: string | null;
    original_service?: string | null;
    allow_fallback?: boolean;
    created_at: string;
    started_at: string | null;
    completed_at: string | null;
    requested_quality?: string | null;
    effective_quality?: string | null;
    requested_format?: string | null;
    effective_format?: string | null;
    quality_decision?: string | null;
    provider_fallback_used?: number | null;
    quality_fallback_used?: number | null;
    decision_reason?: string | null;
}

export interface QueueStats {
    total: number;
    queued: number;
    downloading: number;
    completed: number;
    failed: number;
    paused: number;
    cancelled?: number;
    submitted?: number;
    active?: number;
    skipped?: number;
    deduplicated?: number;
    physical_files?: number;
    downloads_count?: number;
    success_rate?: number;
    audio_count?: number;
    lrc_count?: number;
    cover_count?: number;
    booklet_count?: number;
}

// ==============================================
// DOWNLOAD PREFLIGHT & SAFE BATCH TYPES (S138A)
// ==============================================

export type DownloadPreflightStatus =
    | 'ReadyExactSource'
    | 'ReadyFallbackExactIdentity'
    | 'StaleSource'
    | 'RequiresAuth'
    | 'RejectedQuality'
    | 'AmbiguousSource'
    | 'NoDownloadProvider'
    | 'NetworkRetryable'
    | 'AlreadyDownloaded'
    | 'AlreadyQueued';

export interface TrackPreflightResult {
    track_id: number;
    title: string;
    artist?: string | null;
    album?: string | null;
    status: DownloadPreflightStatus;
    is_eligible: boolean;
    resolved_service_id?: number | null;
    resolved_service_name?: string | null;
    resolved_service_track_id?: string | null;
    resolved_quality?: string | null;
    reason: string;
    match_method?: string | null;
}

export interface PreflightSummaryCounts {
    requested_total: number;
    eligible_total: number;
    ready_exact: number;
    ready_fallback: number;
    already_downloaded: number;
    already_queued: number;
    no_download_provider: number;
    ambiguous_source: number;
    rejected_quality: number;
    stale_source: number;
    requires_auth: number;
    network_retryable: number;
}

export interface PreflightBatchResponse {
    summary: PreflightSummaryCounts;
    tracks: TrackPreflightResult[];
    estimated_size_mb: number;
}

export interface BatchEnqueueResult {
    submitted: number;
    added: number;
    enqueued: number;
    deduplicated: number;
    skipped: number;
    summary: PreflightSummaryCounts;
    tracks: TrackPreflightResult[];
}

export interface WorkerStatus {
    running: boolean;
    paused: boolean;
    active_downloads: number;
    max_concurrent: number;
    is_running?: boolean;
    is_paused?: boolean;
    current_downloads?: number;
    total_processed?: number;
    total_failed?: number;
}

export interface PipelineResolvedTrack {
    track_id: string;
    title: string;
    artist: string;
    album: string;
    isrc?: string;
    requested_quality: string;
    obtained_quality?: string;
    format_id_requested?: string;
    format_id_obtained?: string;
    quality_class?: string;
    codec?: string;
    container?: string;
    final_path?: string;
    size_bytes?: number;
    bit_depth?: number;
    sample_rate?: number;
}

export interface PipelineProgressEvent {
    target: string;
    provider: string;
    step: string;
    step_number: number;
    total_steps: number;
    message?: string;
    error?: string;
    resolved_track?: PipelineResolvedTrack;
}

export interface TidalSingleTrackRequest {
    track_id_or_query: string;
    requested_quality?: string;
    output_dir?: string;
    allow_lossy_fallback?: boolean;
}

export interface TidalSingleTrackResponse {
    track_id: string;
    title: string;
    artist: string;
    album: string;
    isrc?: string;
    requested_quality: string;
    obtained_quality: string;
    format_id_requested?: string;
    format_id_obtained?: string;
    quality_class?: string;
    codec: string;
    container: string;
    extension: string;
    final_path: string;
    size_bytes: number;
    bit_depth: number;
    sample_rate: number;
    is_fallback: boolean;
    download_result: string;
    rejection_reason?: string;
    audio_validation: string;
    flac_validation: string;
    tagging_result: string;
    enrichment_result: string;
    cover_result: string;
}
