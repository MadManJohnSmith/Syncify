// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
// ==============================================
// SPRINT 6: MIGRATION TYPES
// ==============================================

export interface MigrationJob {
    id: string;
    source_service: string;
    destination_service: string;
    source_account_id: number | null;
    destination_account_id: number | null;
    source_playlist_ids: string | null;
    /** JSON array of requested collection names; null keeps meaning the legacy track-favorites flow (0113). */
    collections: string | null;
    options: string;
    status: string;
    total_items: number;
    completed_items: number;
    failed_items: number;
    skipped_items: number;
    started_at: string | null;
    completed_at: string | null;
    error_message: string | null;
    created_at: string;
}

export interface MigrationItem {
    id: number;
    job_id: string;
    source_track_id: string;
    source_track_title: string;
    source_track_artist: string;
    source_track_album: string | null;
    source_playlist_id: string | null;
    source_playlist_name: string | null;
    /** Collection the item belongs to (0113): 'track' | 'album' | 'artist'. */
    kind: string;
    destination_track_id: string | null;
    match_confidence: number | null;
    match_method: string | null;
    status: string;
    error_message: string | null;
    processed_at: string | null;
    created_at: string;
}

export interface MigrationTemplate {
    id: number;
    name: string;
    description: string | null;
    source_service: string;
    destination_service: string;
    options: string;
    created_at: string;
    updated_at: string;
}

export interface MigrationOptions {
    match_threshold: number;
    skip_unmatched: boolean;
    create_playlists: boolean;
    merge_existing: boolean;
    download_matched: boolean;
}

/** Collections the migration flow tracks (M00 capability matrix). */
export type MigrationCollection = 'tracks' | 'albums' | 'artists' | 'playlists';

/** One matrix cell: whether a destination declares a collection. */
export interface MigrationCollectionSupport {
    collection: MigrationCollection;
    supported: boolean;
}

/** One matrix row: a destination and the support of every collection. */
export interface MigrationDestinationCapability {
    service: string;
    collections: MigrationCollectionSupport[];
}

export interface MigrationPreviewResult {
    total_tracks: number;
    matched_tracks: number;
    unmatched_tracks: number;
    playlists: PlaylistPreview[];
}

export interface PlaylistPreview {
    id: string;
    name: string;
    track_count: number;
    matched_count: number;
}

export interface MigrationProgress {
    job_id: string;
    current_item: number;
    total_items: number;
    current_track: string;
    status: string;
    completed_count: number;
    failed_count: number;
    skipped_count: number;
    /** Completion percentage: current_item / total_items * 100 */
    percent: number;
    /** Estimated throughput in items per minute */
    speed: number;
    /** Estimated time remaining, derived from the observed throughput */
    eta: string;
    /** Human readable description of the operation being performed */
    current_action: string;
    /** Collection kind of the item being processed (0113); absent outside per-item events. */
    current_kind?: string;
    /** Batch that produced this event (M06); absent outside per-batch events. */
    batch_index?: number;
    batch_total?: number;
    /** Outcomes of this batch alone: created now, already present, failed, skipped. */
    batch_created?: number;
    batch_preexisting?: number;
    batch_failed?: number;
    batch_skipped?: number;
}

export interface DestinationTrackMatch {
    track_id: string;
    title: string;
    artist: string;
    album: string | null;
    duration_ms: number;
    quality: string | null;
    confidence: number;
}

// #197 (SB02): `incremental_sync` retirado; el sync siempre es recorrido
// completo (spec #64).
export interface ImportPreferences {
    service_name: string;
    favorite_tracks: boolean;
    favorite_albums: boolean;
    favorite_artists: boolean;
    playlists: boolean;
    purchases: boolean;
    library_history: boolean;
    include_appearances: boolean;
}

export interface ServiceSyncResult {
    service: string;
    account_id?: number | null;
    accountId?: number | null;
    success: boolean;
    message: string;
    imported_tracks_total?: number;
    importedTracksTotal?: number;
    favorite_tracks_total?: number;
    favoriteTracksTotal?: number;
    favorite_albums_total?: number;
    favoriteAlbumsTotal?: number;
    favorite_artists_total?: number;
    favoriteArtistsTotal?: number;
    playlists_total?: number;
    playlistsTotal?: number;
    purchases_total?: number;
    purchasesTotal?: number;
    skipped_tracks_total?: number;
    skippedTracksTotal?: number;
    albums_total?: number;
    albumsTotal?: number;
    metadata_enriched?: number;
    metadataEnriched?: number;
    metadata_partial?: number;
    metadataPartial?: number;
    availability_unknown?: number;
    availabilityUnknown?: number;
    availability_checked?: number;
    availabilityChecked?: number;
    tracks_processed?: number;
    tracksProcessed?: number;
    tracks_changed_unique?: number;
    tracksChangedUnique?: number;
    tracks_new_global?: number;
    tracksNewGlobal?: number;
    sources_new_for_service?: number;
    sourcesNewForService?: number;
    library_entries_new_for_account?: number;
    libraryEntriesNewForAccount?: number;
    tracks_already_present?: number;
    tracksAlreadyPresent?: number;
    favorites_seen?: number;
    favoritesSeen?: number;
    albums_seen?: number;
    albumsSeen?: number;
    playlists_seen?: number;
    playlistsSeen?: number;
    tracks_expanded?: number;
    tracksExpanded?: number;
    tracks_expansion_failed?: number;
    tracksExpansionFailed?: number;
    errors: string[];
}

export interface ServiceAuthStatus {
    service: string;
    account_id?: number | null;
    accountId?: number | null;
    is_active: boolean;
    status: 'connected_valid' | 'requires_auth' | 'expired' | 'missing' | 'error';
    is_authenticated: boolean;
    isAuthenticated?: boolean;
    credentials_valid?: boolean;
    credentialsValid?: boolean;
    credentials_expired?: boolean;
    credentialsExpired?: boolean;
    credentials_invalid?: boolean;
    credentialsInvalid?: boolean;
    sync_available?: boolean;
    syncAvailable?: boolean;
    download_entitled?: boolean;
    downloadEntitled?: boolean;
    download_auth_failed?: boolean;
    downloadAuthFailed?: boolean;
    display_name?: string | null;
    displayName?: string | null;
    email?: string | null;
    error_message?: string | null;
    errorMessage?: string | null;
    last_auth_error?: string | null;
    lastAuthError?: string | null;
    last_auth_error_at?: string | null;
    lastAuthErrorAt?: string | null;
    last_checked?: string | null;
    lastChecked?: string | null;
}

export interface ServiceNotification {
    service: string;
    account_id?: number | null;
    accountId?: number | null;
    operation: 'sync' | 'download';
    kind: 'auth' | 'entitlement' | 'rate_limit' | 'network' | 'quality' | 'expansion';
    severity: 'info' | 'warning' | 'error';
    dedupe_key: string;
    dedupeKey?: string;
    message: string;
    occurred_at: string;
    occurredAt?: string;
    resolved_at?: string | null;
    resolvedAt?: string | null;
}

export interface DownloadRepairDryRunItem {
    download_id: number;
    old_track_id: number;
    new_track_id: number;
    old_path: string;
    new_path: string;
    old_title: string;
    new_title: string;
    old_artist: string;
    new_artist: string;
    old_album: string;
    new_album: string;
    old_hash: string | null;
    expected_hash_after: string | null;
    flac_operation: string;
    lrc_operation: string;
    cover_operation: string;
    downloads_update: string;
    ghost_cleanup: string;
    rollback_plan: string;
    planned_action: string;
    confidence: number;
    provenance: string;
    no_redownload_confirmed: boolean;
}

export interface RepairHistoryRecord {
    id: number;
    repair_id: string;
    timestamp: string;
    download_id?: number | null;
    old_track_id?: number | null;
    new_track_id?: number | null;
    old_path: string;
    new_path: string;
    input_file_hash: string;
    output_file_hash?: string | null;
    audio_payload_hash_before?: string | null;
    audio_payload_hash_after?: string | null;
    baseline_validation: string;
    actions: string[];
    rollback_state?: string | null;
    provenance: string;
    result: string;
    details_json?: string | null;
}

export interface CatalogAnomalyItem {
    category: string;
    entity_type: string;
    entity_id?: number | null;
    service_id?: number | null;
    service_track_id?: string | null;
    message: string;
    suggested_action: string;
}

export interface CatalogIdentityAuditReport {
    audit_timestamp: string;
    duplicate_service_sources_count: number;
    conflicting_isrc_count: number;
    ghost_tracks_count: number;
    ghost_albums_count: number;
    ghost_artists_count: number;
    downloads_without_canonical_track_count: number;
    canonical_tracks_without_valid_source_count: number;
    placeholder_metadata_count: number;
    ambiguous_editions_count: number;
    orphan_playlist_links_count: number;
    physical_path_mismatches_count: number;
    metadata_provenance_conflicts_count: number;
    invalid_filenames_count: number;
    invalid_taggings_count: number;
    sidecar_mismatches_count: number;
    staging_residuals_count: number;
    total_anomalies: number;
    details: CatalogAnomalyItem[];
}

export interface CatalogRepairPlanItem {
    anomaly_category: string;
    entity_type: string;
    entity_id?: number | null;
    current_state: string;
    proposed_state: string;
    requires_fs_mutation: boolean;
    file_path?: string | null;
}

export interface CatalogRepairPlan {
    plan_id: string;
    created_at: string;
    items_to_repair: CatalogRepairPlanItem[];
    requires_confirmation: boolean;
}

export interface CatalogRepairExecutionReport {
    plan_id: string;
    executed_at: string;
    items_attempted: number;
    items_succeeded: number;
    items_failed: number;
    db_backup_path?: string | null;
    db_backup_sha256?: string | null;
    errors: string[];
}
/** S143B/S159: snapshot of file + audio payload + sidecar LRC hashes taken during the dry-run */
export interface DisambiguationRepairBaseline {
    file_path: string;
    input_sha256: string;
    input_size: number;
    input_modified_at: number;
    audio_content_hash?: string | null;
    lrc_path?: string | null;
    lrc_sha256?: string | null;
    lrc_size?: number | null;
    lrc_modified_at?: number | null;
}
/** S143B/S159: before/after hash audit captured around an applied rename */
export interface DisambiguationRepairOutputHashes {
    file_hash_before: string;
    file_hash_after?: string | null;
    audio_content_hash_before?: string | null;
    audio_content_hash_after?: string | null;
    lrc_hash_before?: string | null;
    lrc_hash_after?: string | null;
}
export interface DisambiguationRepairItem {
    track_id: number;
    isrc?: string | null;
    current_audio_path: string;
    target_audio_path: string;
    current_lrc_path?: string | null;
    target_lrc_path?: string | null;
    source_title: string;
    display_title: string;
    file_disambiguator: string;
    sha256_before: string;
    status: string;
    baseline?: DisambiguationRepairBaseline | null;
    output_hashes?: DisambiguationRepairOutputHashes | null;
    applied_actions: string[];
    rollback_state?: string | null;
}
export interface DisambiguationRepairReport {
    dry_run: boolean;
    items: DisambiguationRepairItem[];
    total_candidates: number;
    total_renamed: number;
    total_skipped: number;
    errors: string[];
    applied_actions: string[];
    rollback_state?: string | null;
}

export interface OperationRecoveryDetail {
    operation_id: string;
    operation_type: string;
    previous_status: string;
    new_status: string;
    phase: string;
    action_taken: string;
    message: string;
    ui_label: string;
    error_taxonomy?: string | null;
}

export interface RecoveryAuditSummary {
    total_journal_scanned: number;
    active_operations_found: number;
    recovered_count: number;
    interrupted_retryable_count: number;
    failed_terminal_count: number;
    cleaned_staging_files: number;
    details: OperationRecoveryDetail[];
}

export interface ConcurrencyStatsSummary {
    total_acquisitions: number;
    contended_acquisitions: number;
    timeouts: number;
    active_locks_count: number;
    max_wait_duration_ms: number;
    max_held_duration_ms: number;
}
