/**
 * Metadata API
 * 
 * Tauri commands for metadata enrichment and management.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asString, asRecord, pick, pickArray, pickNumber } from './normalize';
import type {
    LibraryTrack,
    MetadataMatch,
    MetadataStats,
    EnrichmentMode,
    EnrichmentJobSummary,
    EnrichmentPreview,
    CatalogIdentityAuditReport,
    CatalogAnomalyItem,
    CatalogRepairPlan,
    CatalogRepairPlanItem,
    CatalogRepairExecutionReport,
    OperationRecoveryDetail,
    RecoveryAuditSummary,
    ConcurrencyStatsSummary,
    RepairHistoryRecord,
} from './types';
import {
    createConcurrencyGuard,
    createLatestAsyncCaller,
    createAsyncQueue,
    type ConcurrencyGuard,
    type ConcurrencyGuardOptions,
    type LatestAsyncCaller,
    type LatestAsyncResult,
} from './concurrency';
import {
    executeWithRecovery,
    isRetryableError,
    createOperationRecoveryTracker,
    type RetryOptions,
    type TrackedOperation,
    type OperationRecoveryState,
} from './resilience';

// ==============================================
// TYPES
// ==============================================

export interface TrackMetadata {
    id: number;
    title: string;
    artistName: string | null;
    albumName: string | null;
    trackNumber: number | null;
    discNumber: number | null;
    isrc: string | null;
    musicbrainzId: string | null;
    genre: string | null;
    releaseYear: number | null;
    bpm: number | null;
    musicalKey: string | null;
    explicit: boolean | null;
    durationMs: number | null;
    filePath: string | null;
}

export interface TrackTags {
    title: string;
    artist: string;
    album: string;
    album_artist?: string | null;
    composer?: string | null;
    genre?: string | null;
    style?: string | null;
    mood?: string | null;
    grouping?: string | null;
    language?: string | null;
    copyright?: string | null;
    label?: string | null;
    catalog_number?: string | null;
    isrc?: string | null;
    release_year?: string | null;
    comment?: string | null;
    track_number?: number | null;
    track_total?: number | null;
    disc_number?: number | null;
    disc_total?: number | null;
    bpm?: number | null;
    initial_key?: string | null;
}

export interface TrackTagsSnapshot {
    track_id: number;
    file_path: string;
    file_format: string;
    all_tags: Record<string, string[]>;
    has_cover: boolean;
    cover_mime?: string | null;
}

export interface TagVerification {
    file_exists: boolean;
    flac_valid: boolean;
    tags_match: boolean;
    cover_present: boolean;
    cover_size_bytes?: number | null;
    cover_mime?: string | null;
    cover_width?: number | null;
    cover_height?: number | null;
    lyrics_present: boolean;
    synced_lyrics_present: boolean;
    unsynced_lyrics_present: boolean;
    bpm_present: boolean;
    duration_sec?: number | null;
    mismatches?: Array<[string, string, string]>;
}

// ==============================================
// METADATA MANAGEMENT
// ==============================================

/**
 * Get overall metadata completeness statistics
 *
 * The Rust struct serializes camelCase (`totalTracks`, `withIsrc`, ...);
 * both spellings are accepted and normalized to the canonical snake_case contract.
 */
export async function getMetadataStats(): Promise<MetadataStats> {
    const raw = await invokeCommand<unknown>('get_metadata_stats');
    return {
        total_tracks: pickNumber(raw, ['total_tracks', 'totalTracks']),
        with_isrc: pickNumber(raw, ['with_isrc', 'withIsrc']),
        with_musicbrainz_id: pickNumber(raw, ['with_musicbrainz_id', 'withMusicbrainzId', 'withMusicBrainzId']),
        with_album: pickNumber(raw, ['with_album', 'withAlbum']),
        with_year: pickNumber(raw, ['with_year', 'withYear']),
        with_genre: pickNumber(raw, ['with_genre', 'withGenre']),
        with_art: pickNumber(raw, ['with_art', 'withArt']),
        average_completeness: pickNumber(raw, ['average_completeness', 'averageCompleteness']),
    };
}

/**
 * Get tracks that are missing core metadata
 */
export async function getTracksNeedingMetadata(limit: number = 100): Promise<LibraryTrack[]> {
    const raw = await invokeCommand<unknown>('get_tracks_needing_metadata', { limit });
    return asArray<LibraryTrack>(raw);
}

/**
 * Update metadata for a specific track
 */
export async function updateTrackMetadata(trackId: number, metadata: Partial<{
    title: string;
    albumName: string;
    artistName: string;
    trackNumber: number;
    discNumber: number;
    isrc: string;
    explicit: boolean;
    genre: string;
    year: number;
    bpm: number;
    musicalKey: string;
    mbTrackId: string;
    label: string;
}>): Promise<LibraryTrack> {
    return invokeCommand<LibraryTrack>('update_track_metadata', {
        trackId,
        metadata
    });
}

// ==============================================
// METADATA ENRICHMENT
// ==============================================

/**
 * Enrich metadata for a single track
 */
export async function enrichMetadata(trackId: number): Promise<{
    success: boolean;
    updatedFields: string[];
    error?: string;
}> {
    const raw = await invokeCommand<unknown>('enrich_metadata', { trackId });
    return {
        success: pick(raw, ['success']) === true,
        updatedFields: pickArray<string>(raw, ['updatedFields', 'updated_fields']),
        error: typeof pick(raw, ['error']) === 'string' ? (pick(raw, ['error']) as string) : undefined,
    };
}

/**
 * Batch enrich metadata for multiple tracks
 */
export async function batchEnrichMetadata(trackIds: number[]): Promise<{
    enriched: number;
    failed: number;
    skipped: number;
}> {
    const raw = await invokeCommand<unknown>('batch_enrich_metadata', { tracks: trackIds });
    const payload = (raw && typeof raw === 'object' && 'data' in raw && (raw as { data: unknown }).data)
        ? (raw as { data: unknown }).data
        : raw;
    return {
        enriched: pickNumber(payload, ['enriched']),
        failed: pickNumber(payload, ['failed']),
        skipped: pickNumber(payload, ['skipped']),
    };
}

/**
 * Enrich all tracks needing metadata across the entire library.
 * Invokes native `start_library_enrichment` with mode 'incomplete_only'.
 */
export async function enrichAllNeeding(): Promise<{
    total: number;
    enriched: number;
    failed: number;
    jobSummary?: EnrichmentJobSummary;
}> {
    const raw = await invokeCommand<unknown>('start_library_enrichment', {
        mode: 'incomplete_only',
    });
    return {
        total: pickNumber(raw, ['totalTracks', 'total_tracks', 'total']),
        enriched: pickNumber(raw, ['modifiedTracks', 'modified_tracks', 'enriched']),
        failed: pickNumber(raw, ['failedTracks', 'failed_tracks', 'failed']),
        jobSummary: (raw && typeof raw === 'object' && ('jobId' in raw || 'job_id' in raw))
            ? (raw as EnrichmentJobSummary)
            : undefined,
    };
}

/**
 * Start incremental library enrichment with an explicit mode and optional track list.
 */
export async function startLibraryEnrichment(
    mode: EnrichmentMode = 'incomplete_only',
    trackIds?: number[]
): Promise<EnrichmentJobSummary> {
    return invokeCommand<EnrichmentJobSummary>('start_library_enrichment', {
        mode,
        trackIds,
    });
}

/**
 * Preview library enrichment to count eligible tracks before execution.
 */
export async function previewLibraryEnrichment(
    mode: EnrichmentMode = 'incomplete_only',
    trackIds?: number[]
): Promise<EnrichmentPreview> {
    return invokeCommand<EnrichmentPreview>('preview_library_enrichment', {
        mode,
        trackIds,
    });
}

/**
 * Cancel currently running library enrichment job.
 */
export async function cancelLibraryEnrichment(): Promise<boolean> {
    return invokeCommand<boolean>('cancel_library_enrichment');
}

/**
 * Query current status of background library enrichment job.
 */
export async function getLibraryEnrichmentStatus(): Promise<EnrichmentJobSummary | null> {
    return invokeCommand<EnrichmentJobSummary | null>('get_library_enrichment_status');
}

/**
 * Backfill album artwork via MusicBrainz ISRC → release-group → Cover Art Archive.
 * Only albums whose cover is currently empty are touched; every CAA URL is
 * HEAD-verified before being persisted.
 */
export async function fetchMissingCoverArt(limit: number = 100): Promise<{
    checked: number;
    updated: number;
    skipped: number;
    failed: number;
}> {
    const raw = await invokeCommand<unknown>('fetch_missing_cover_art', { limit });
    return {
        checked: pickNumber(raw, ['checked']),
        updated: pickNumber(raw, ['updated']),
        skipped: pickNumber(raw, ['skipped']),
        failed: pickNumber(raw, ['failed']),
    };
}

// ==============================================
// MUSICBRAINZ MATCHING
// ==============================================

/**
 * Match a track against MusicBrainz
 */
export async function matchMusicBrainz(params: {
    title: string;
    artist: string;
    album?: string;
    durationMs?: number;
    isrc?: string;
}): Promise<MetadataMatch[]> {
    const raw = await invokeCommand<unknown>('match_musicbrainz', { params });
    return asArray<MetadataMatch>(raw);
}

/**
 * Apply a MusicBrainz match to a track
 */
export async function applyMusicBrainzMatch(trackId: number, recordingId: string): Promise<boolean> {
    return invokeCommand<boolean>('apply_musicbrainz_match', {
        trackId,
        recordingId
    });
}

/**
 * Auto-match tracks against MusicBrainz.
 *
 * Redirects the phantom `auto_match_musicbrainz` to canonical native commands:
 * - When specific `trackIds` are supplied: invokes `start_library_enrichment` with selection mode.
 * - Otherwise: triggers batch MusicBrainz lookup via `enrich_metadata_musicbrainz`.
 */
export async function autoMatchMusicBrainz(trackIds?: number[]): Promise<{
    matched: number;
    failed: number;
    noMatch: number;
}> {
    if (trackIds && trackIds.length > 0) {
        try {
            const raw = await invokeCommand<unknown>('start_library_enrichment', {
                mode: 'selection',
                trackIds,
            });
            const matched = pickNumber(raw, ['modifiedTracks', 'modified_tracks', 'matched']);
            const failed = pickNumber(raw, ['failedTracks', 'failed_tracks', 'failed']);
            const total = pickNumber(raw, ['totalTracks', 'total_tracks', 'total']) || trackIds.length;
            return {
                matched,
                failed,
                noMatch: Math.max(0, total - (matched + failed)),
            };
        } catch {
            // Fall back to batch ISRC lookup below if incremental worker cannot start
        }
    }

    const raw = await invokeCommand<unknown>('enrich_metadata_musicbrainz', {
        limit: trackIds && trackIds.length > 0 ? trackIds.length : undefined,
    });
    const total = pickNumber(raw, ['total', 'totalTracks']);
    const matched = pickNumber(raw, ['enriched', 'matched']);
    const failed = pickNumber(raw, ['failed']);
    return {
        matched,
        failed,
        noMatch: Math.max(0, total - (matched + failed)),
    };
}

/**
 * Batch enrich track metadata using MusicBrainz ISRC lookups.
 */
export async function enrichMetadataMusicBrainz(limit?: number): Promise<{
    total: number;
    enriched: number;
    failed: number;
}> {
    const raw = await invokeCommand<unknown>('enrich_metadata_musicbrainz', { limit });
    return {
        total: pickNumber(raw, ['total']),
        enriched: pickNumber(raw, ['enriched']),
        failed: pickNumber(raw, ['failed']),
    };
}

// ==============================================
// FINGERPRINTING
// ==============================================

/**
 * Check if fingerprinting is available
 */
export async function checkFingerprintAvailable(): Promise<boolean> {
    return invokeCommand<boolean>('check_fingerprint_available');
}

/**
 * Identify audio by fingerprint (AcoustID)
 */
export async function identifyAudio(filePath: string): Promise<MetadataMatch[]> {
    const raw = await invokeCommand<unknown>('identify_audio', { filePath });
    return asArray<MetadataMatch>(raw);
}

/**
 * Find duplicate audio files
 */
export async function findAudioDuplicates(): Promise<{
    groups: Array<{ fingerprint: string; tracks: LibraryTrack[] }>;
    totalDuplicates: number;
}> {
    const raw = await invokeCommand<unknown>('find_audio_duplicates');
    return {
        groups: asArray<{ fingerprint: string; tracks: LibraryTrack[] }>(pick(raw, ['groups'])),
        totalDuplicates: pickNumber(raw, ['totalDuplicates', 'total_duplicates']),
    };
}

// ==============================================
// METADATA EDITING & TAGS IPC
// ==============================================

/**
 * Write edited tags directly to audio file (FLAC Vorbis comments).
 * Delegates to native `write_track_tags` command and returns a roundtrip TagVerification report.
 */
export async function writeTrackTags(trackId: number, tags: TrackTags): Promise<TagVerification> {
    return invokeCommand<TagVerification>('write_track_tags', {
        trackId,
        tags,
        metadata: tags,
    });
}

/**
 * Read raw tag snapshot directly from local audio file.
 * Returns a TrackTagsSnapshot containing all tags, file format, and cover art info.
 */
export async function readTrackTags(trackId: number): Promise<TrackTagsSnapshot> {
    return invokeCommand<TrackTagsSnapshot>('read_track_tags', { trackId });
}

/**
 * Compatibility wrapper for writing tags to file.
 * Preserves compatibility with existing callers while delegating to `write_track_tags`.
 * @deprecated Use `writeTrackTags(trackId, tags)` instead.
 */
export async function writeMetadataToFile(trackId: number, tags?: TrackTags): Promise<boolean> {
    const payload: TrackTags = tags ?? {
        title: '',
        artist: '',
        album: '',
    };
    const res = await writeTrackTags(trackId, payload);
    return res.tags_match ?? res.flac_valid ?? true;
}

/**
 * Compatibility wrapper for reading tags from file.
 * Preserves compatibility with existing callers while delegating to `read_track_tags`.
 * @deprecated Use `readTrackTags(trackId)` instead.
 */
export async function readMetadataFromFile(trackIdOrPath: number | string): Promise<TrackTagsSnapshot | Partial<LibraryTrack>> {
    if (typeof trackIdOrPath === 'number') {
        return readTrackTags(trackIdOrPath);
    }
    const parsed = parseInt(trackIdOrPath, 10);
    if (!Number.isNaN(parsed)) {
        return readTrackTags(parsed);
    }
    return invokeCommand<Partial<LibraryTrack>>('read_track_tags', { trackId: 0, filePath: trackIdOrPath });
}

/**
 * S158: Compute rich dry-run repair audit items for corrupt Tidal downloads (read-only)
 */
export async function getTidalRepairDryRun(): Promise<import('./types').DownloadRepairDryRunItem[]> {
    const raw = await invokeCommand<unknown>('get_tidal_repair_dry_run');
    return asArray<import('./types').DownloadRepairDryRunItem>(raw);
}

/**
 * S163: Query persistent, append-only historical audit records of applied repairs (read-only)
 */
export async function getRepairHistory(limit: number = 100, offset: number = 0): Promise<RepairHistoryRecord[]> {
    const safeLimit = Math.max(0, limit);
    const safeOffset = Math.max(0, offset);
    const raw = await invokeCommand<unknown>('get_repair_history', { limit: safeLimit, offset: safeOffset });
    return asArray<RepairHistoryRecord>(raw).map((record) => {
        const rec = asRecord(record);
        return {
            id: pickNumber(rec, ['id']),
            repair_id: asString(pick(rec, ['repair_id', 'repairId']), ''),
            timestamp: asString(pick(rec, ['timestamp']), ''),
            download_id: typeof pick(rec, ['download_id', 'downloadId']) === 'number'
                ? (pick(rec, ['download_id', 'downloadId']) as number)
                : null,
            old_track_id: typeof pick(rec, ['old_track_id', 'oldTrackId']) === 'number'
                ? (pick(rec, ['old_track_id', 'oldTrackId']) as number)
                : null,
            new_track_id: typeof pick(rec, ['new_track_id', 'newTrackId']) === 'number'
                ? (pick(rec, ['new_track_id', 'newTrackId']) as number)
                : null,
            old_path: asString(pick(rec, ['old_path', 'oldPath']), ''),
            new_path: asString(pick(rec, ['new_path', 'newPath']), ''),
            input_file_hash: asString(pick(rec, ['input_file_hash', 'inputFileHash']), ''),
            output_file_hash: pick(rec, ['output_file_hash', 'outputFileHash'])
                ? asString(pick(rec, ['output_file_hash', 'outputFileHash']))
                : null,
            audio_payload_hash_before: pick(rec, ['audio_payload_hash_before', 'audioPayloadHashBefore'])
                ? asString(pick(rec, ['audio_payload_hash_before', 'audioPayloadHashBefore']))
                : null,
            audio_payload_hash_after: pick(rec, ['audio_payload_hash_after', 'audioPayloadHashAfter'])
                ? asString(pick(rec, ['audio_payload_hash_after', 'audioPayloadHashAfter']))
                : null,
            baseline_validation: asString(pick(rec, ['baseline_validation', 'baselineValidation']), 'Valid'),
            actions: asArray<string>(pick(rec, ['actions'])),
            rollback_state: pick(rec, ['rollback_state', 'rollbackState'])
                ? asString(pick(rec, ['rollback_state', 'rollbackState']))
                : null,
            provenance: asString(pick(rec, ['provenance']), 'CatalogIdentityRepair'),
            result: asString(pick(rec, ['result']), 'success'),
            details_json: pick(rec, ['details_json', 'detailsJson'])
                ? asString(pick(rec, ['details_json', 'detailsJson']))
                : null,
        };
    });
}

/**
 * Normalizes a single catalog anomaly item from backend payloads
 */
export function normalizeCatalogAnomalyItem(raw: unknown): CatalogAnomalyItem {
    const rec = asRecord(raw);
    const category = asString(pick(rec, ['category', 'anomaly_category', 'anomalyCategory']), 'UnknownAnomaly');
    const entity_type = asString(pick(rec, ['entity_type', 'entityType']), 'unknown');
    const entity_id = typeof pick(rec, ['entity_id', 'entityId']) === 'number'
        ? (pick(rec, ['entity_id', 'entityId']) as number)
        : null;
    const service_id = typeof pick(rec, ['service_id', 'serviceId']) === 'number'
        ? (pick(rec, ['service_id', 'serviceId']) as number)
        : null;
    const service_track_id = pick(rec, ['service_track_id', 'serviceTrackId'])
        ? asString(pick(rec, ['service_track_id', 'serviceTrackId']))
        : null;
    const message = asString(pick(rec, ['message', 'current_state', 'currentState']), '');
    const suggested_action = asString(
        pick(rec, ['suggested_action', 'suggestedAction', 'proposed_state', 'proposedState']),
        ''
    );

    return {
        category,
        entity_type,
        entity_id,
        service_id,
        service_track_id,
        message,
        suggested_action,
    };
}

/**
 * Normalizes a catalog identity audit report
 */
export function normalizeCatalogAuditReport(raw: unknown): CatalogIdentityAuditReport {
    const rec = asRecord(raw);
    const rawDetails = pick(rec, ['details']);
    const details = asArray(rawDetails)
        .map(normalizeCatalogAnomalyItem)
        .filter((d) => d.category !== '');

    const duplicate_service_sources_count = pickNumber(rec, ['duplicate_service_sources_count', 'duplicateServiceSourcesCount']);
    const conflicting_isrc_count = pickNumber(rec, ['conflicting_isrc_count', 'conflictingIsrcCount']);
    const ghost_tracks_count = pickNumber(rec, ['ghost_tracks_count', 'ghostTracksCount']);
    const ghost_albums_count = pickNumber(rec, ['ghost_albums_count', 'ghostAlbumsCount']);
    const ghost_artists_count = pickNumber(rec, ['ghost_artists_count', 'ghostArtistsCount']);
    const downloads_without_canonical_track_count = pickNumber(rec, ['downloads_without_canonical_track_count', 'downloadsWithoutCanonicalTrackCount']);
    const canonical_tracks_without_valid_source_count = pickNumber(rec, ['canonical_tracks_without_valid_source_count', 'canonicalTracksWithoutValidSourceCount']);
    const placeholder_metadata_count = pickNumber(rec, ['placeholder_metadata_count', 'placeholderMetadataCount']);
    const ambiguous_editions_count = pickNumber(rec, ['ambiguous_editions_count', 'ambiguousEditionsCount']);
    const orphan_playlist_links_count = pickNumber(rec, ['orphan_playlist_links_count', 'orphanPlaylistLinksCount']);
    const physical_path_mismatches_count = pickNumber(rec, ['physical_path_mismatches_count', 'physicalPathMismatchesCount']);
    const metadata_provenance_conflicts_count = pickNumber(rec, ['metadata_provenance_conflicts_count', 'metadataProvenanceConflictsCount']);
    const invalid_filenames_count = pickNumber(rec, ['invalid_filenames_count', 'invalidFilenamesCount']);
    const invalid_taggings_count = pickNumber(rec, ['invalid_taggings_count', 'invalidTaggingsCount']);
    const sidecar_mismatches_count = pickNumber(rec, ['sidecar_mismatches_count', 'sidecarMismatchesCount']);
    const staging_residuals_count = pickNumber(rec, ['staging_residuals_count', 'stagingResidualsCount']);

    const sumCounters = duplicate_service_sources_count +
        conflicting_isrc_count +
        ghost_tracks_count +
        ghost_albums_count +
        ghost_artists_count +
        downloads_without_canonical_track_count +
        canonical_tracks_without_valid_source_count +
        placeholder_metadata_count +
        ambiguous_editions_count +
        orphan_playlist_links_count +
        physical_path_mismatches_count +
        metadata_provenance_conflicts_count +
        invalid_filenames_count +
        invalid_taggings_count +
        sidecar_mismatches_count +
        staging_residuals_count;

    const total_anomalies = pickNumber(rec, ['total_anomalies', 'totalAnomalies'], sumCounters || details.length);

    return {
        audit_timestamp: asString(pick(rec, ['audit_timestamp', 'auditTimestamp']), new Date().toISOString()),
        duplicate_service_sources_count,
        conflicting_isrc_count,
        ghost_tracks_count,
        ghost_albums_count,
        ghost_artists_count,
        downloads_without_canonical_track_count,
        canonical_tracks_without_valid_source_count,
        placeholder_metadata_count,
        ambiguous_editions_count,
        orphan_playlist_links_count,
        physical_path_mismatches_count,
        metadata_provenance_conflicts_count,
        invalid_filenames_count,
        invalid_taggings_count,
        sidecar_mismatches_count,
        staging_residuals_count,
        total_anomalies,
        details,
    };
}

/**
 * S165: Read-only forensic audit across 16 categories of catalog consistency
 */
export async function auditCatalogIdentity(): Promise<CatalogIdentityAuditReport> {
    const raw = await invokeCommand<unknown>('audit_catalog_identity');
    return normalizeCatalogAuditReport(raw);
}

/**
 * Normalizes catalog repair plan items
 */
export function normalizeRepairPlanItem(raw: unknown): CatalogRepairPlanItem {
    const rec = asRecord(raw);
    return {
        anomaly_category: asString(pick(rec, ['anomaly_category', 'anomalyCategory', 'category']), 'UnknownAnomaly'),
        entity_type: asString(pick(rec, ['entity_type', 'entityType']), 'unknown'),
        entity_id: typeof pick(rec, ['entity_id', 'entityId']) === 'number'
            ? (pick(rec, ['entity_id', 'entityId']) as number)
            : null,
        current_state: asString(pick(rec, ['current_state', 'currentState']), ''),
        proposed_state: asString(pick(rec, ['proposed_state', 'proposedState']), ''),
        requires_fs_mutation: pick(rec, ['requires_fs_mutation', 'requiresFsMutation']) === true,
        file_path: pick(rec, ['file_path', 'filePath']) ? asString(pick(rec, ['file_path', 'filePath'])) : null,
    };
}

/**
 * Normalizes catalog repair plan
 */
export function normalizeCatalogRepairPlan(raw: unknown): CatalogRepairPlan {
    const rec = asRecord(raw);
    return {
        plan_id: asString(pick(rec, ['plan_id', 'planId']), ''),
        created_at: asString(pick(rec, ['created_at', 'createdAt']), new Date().toISOString()),
        items_to_repair: asArray(pick(rec, ['items_to_repair', 'itemsToRepair'])).map(normalizeRepairPlanItem),
        requires_confirmation: pick(rec, ['requires_confirmation', 'requiresConfirmation']) !== false,
    };
}

/**
 * S165: Generate a non-mutating Dry-Run plan for catalog identity repair
 */
export async function planCatalogIdentityRepair(): Promise<CatalogRepairPlan> {
    const raw = await invokeCommand<unknown>('plan_catalog_identity_repair');
    return normalizeCatalogRepairPlan(raw);
}

/**
 * Normalizes catalog repair execution report
 */
export function normalizeCatalogRepairExecutionReport(raw: unknown, fallbackPlanId: string = ''): CatalogRepairExecutionReport {
    const rec = asRecord(raw);
    return {
        plan_id: asString(pick(rec, ['plan_id', 'planId']), fallbackPlanId),
        executed_at: asString(pick(rec, ['executed_at', 'executedAt']), new Date().toISOString()),
        items_attempted: pickNumber(rec, ['items_attempted', 'itemsAttempted']),
        items_succeeded: pickNumber(rec, ['items_succeeded', 'itemsSucceeded']),
        items_failed: pickNumber(rec, ['items_failed', 'itemsFailed']),
        db_backup_path: pick(rec, ['db_backup_path', 'dbBackupPath']) ? asString(pick(rec, ['db_backup_path', 'dbBackupPath'])) : null,
        db_backup_sha256: pick(rec, ['db_backup_sha256', 'dbBackupSha256']) ? asString(pick(rec, ['db_backup_sha256', 'dbBackupSha256'])) : null,
        errors: pickArray<string>(rec, ['errors']),
    };
}

/**
 * S165: Apply catalog repair plan with explicit confirmation, SHA-256 backup, and append-only audit trail
 */
export async function applyCatalogIdentityRepair(
    plan: CatalogRepairPlan,
    confirmed: boolean
): Promise<CatalogRepairExecutionReport> {
    if (confirmed !== true) {
        throw new Error('applyCatalogIdentityRepair: execution requires explicit confirmation (confirmed: true)');
    }
    if (!plan || typeof plan !== 'object' || !plan.plan_id || typeof plan.plan_id !== 'string' || plan.plan_id.trim() === '') {
        throw new Error('applyCatalogIdentityRepair: invalid repair plan, non-empty plan_id is required');
    }

    const raw = await invokeCommand<unknown>('apply_catalog_identity_repair', { plan, confirmed });
    return normalizeCatalogRepairExecutionReport(raw, plan.plan_id);
}

/**
 * Normalizes a single operation recovery detail item
 */
export function normalizeRecoveryDetail(raw: unknown): OperationRecoveryDetail {
    const rec = asRecord(raw);
    const new_status = asString(pick(rec, ['new_status', 'newStatus', 'status']), 'unknown');
    const defaultLabel = new_status === 'recovered'
        ? 'Recovered after restart'
        : new_status === 'interrupted'
            ? 'Interrupted — retry available'
            : new_status === 'failed_terminal'
                ? 'Failed terminal — user action required'
                : 'Status pending';

    return {
        operation_id: asString(pick(rec, ['operation_id', 'operationId', 'id']), ''),
        operation_type: asString(pick(rec, ['operation_type', 'operationType']), 'unknown'),
        previous_status: asString(pick(rec, ['previous_status', 'previousStatus']), 'unknown'),
        new_status,
        phase: asString(pick(rec, ['phase']), 'unknown'),
        action_taken: asString(pick(rec, ['action_taken', 'actionTaken']), ''),
        message: asString(pick(rec, ['message']), ''),
        ui_label: asString(pick(rec, ['ui_label', 'uiLabel']), defaultLabel),
        error_taxonomy: pick(rec, ['error_taxonomy', 'errorTaxonomy']) ? asString(pick(rec, ['error_taxonomy', 'errorTaxonomy'])) : null,
    };
}

/**
 * Normalizes a post-crash recovery audit summary
 */
export function normalizeRecoveryAuditSummary(raw: unknown): RecoveryAuditSummary {
    const rec = asRecord(raw);
    const details = asArray(pick(rec, ['details'])).map(normalizeRecoveryDetail);
    return {
        total_journal_scanned: pickNumber(rec, ['total_journal_scanned', 'totalJournalScanned']),
        active_operations_found: pickNumber(rec, ['active_operations_found', 'activeOperationsFound']),
        recovered_count: pickNumber(rec, ['recovered_count', 'recoveredCount']),
        interrupted_retryable_count: pickNumber(rec, ['interrupted_retryable_count', 'interruptedRetryableCount']),
        failed_terminal_count: pickNumber(rec, ['failed_terminal_count', 'failedTerminalCount']),
        cleaned_staging_files: pickNumber(rec, ['cleaned_staging_files', 'cleanedStagingFiles']),
        details,
    };
}

/**
 * S167: Query aggregate post-crash recovery audit summary and details
 */
export async function getRecoveryAuditSummary(): Promise<RecoveryAuditSummary> {
    const raw = await invokeCommand<unknown>('get_recovery_audit_summary');
    return normalizeRecoveryAuditSummary(raw);
}

/**
 * S167: Trigger manual/startup post-crash reconciliation
 */
export async function triggerStartupReconciliation(): Promise<RecoveryAuditSummary> {
    const raw = await invokeCommand<unknown>('trigger_startup_reconciliation');
    return normalizeRecoveryAuditSummary(raw);
}

/**
 * Normalizes concurrency statistics summary
 */
export function normalizeConcurrencyStatsSummary(raw: unknown): ConcurrencyStatsSummary {
    const rec = asRecord(raw);
    return {
        total_acquisitions: pickNumber(rec, ['total_acquisitions', 'totalAcquisitions']),
        contended_acquisitions: pickNumber(rec, ['contended_acquisitions', 'contendedAcquisitions']),
        timeouts: pickNumber(rec, ['timeouts']),
        active_locks_count: pickNumber(rec, ['active_locks_count', 'activeLocksCount']),
        max_wait_duration_ms: pickNumber(rec, ['max_wait_duration_ms', 'maxWaitDurationMs']),
        max_held_duration_ms: pickNumber(rec, ['max_held_duration_ms', 'maxHeldDurationMs']),
    };
}

/**
 * S168: Get concurrency statistics summary
 */
export async function getConcurrencyStatsSummary(): Promise<ConcurrencyStatsSummary> {
    const raw = await invokeCommand<unknown>('get_concurrency_stats_summary');
    return normalizeConcurrencyStatsSummary(raw);
}

/**
 * S168: Get active redacted concurrency lock hashes
 */
export async function getActiveConcurrencyLocks(): Promise<string[]> {
    const raw = await invokeCommand<unknown>('get_active_concurrency_locks');
    return asArray<string>(raw).filter((item): item is string => typeof item === 'string' && item.trim().length > 0);
}

// Re-export concurrency and resilience tools
export {
    createConcurrencyGuard,
    createLatestAsyncCaller,
    createAsyncQueue,
    executeWithRecovery,
    isRetryableError,
    createOperationRecoveryTracker,
    type ConcurrencyGuard,
    type ConcurrencyGuardOptions,
    type LatestAsyncCaller,
    type LatestAsyncResult,
    type RetryOptions,
    type TrackedOperation,
    type OperationRecoveryState,
};

// Export as namespace
export const metadataApi = {
    enrichMetadata,
    batchEnrichMetadata,
    enrichAllNeeding,
    startLibraryEnrichment,
    previewLibraryEnrichment,
    cancelLibraryEnrichment,
    getLibraryEnrichmentStatus,
    fetchMissingCoverArt,
    matchMusicBrainz,
    applyMusicBrainzMatch,
    autoMatchMusicBrainz,
    enrichMetadataMusicBrainz,
    checkFingerprintAvailable,
    identifyAudio,
    findAudioDuplicates,
    updateTrackMetadata,
    writeTrackTags,
    readTrackTags,
    writeMetadataToFile,
    readMetadataFromFile,
    getMetadataStats,
    getTracksNeedingMetadata,
    getTidalRepairDryRun,
    getRepairHistory,
    auditCatalogIdentity,
    planCatalogIdentityRepair,
    applyCatalogIdentityRepair,
    getRecoveryAuditSummary,
    triggerStartupReconciliation,
    getConcurrencyStatsSummary,
    getActiveConcurrencyLocks,
    createConcurrencyGuard,
    createLatestAsyncCaller,
    createAsyncQueue,
    executeWithRecovery,
    isRetryableError,
    createOperationRecoveryTracker,
};


