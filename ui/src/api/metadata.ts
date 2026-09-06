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
} from './types';

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
export async function getRepairHistory(limit: number = 100, offset: number = 0): Promise<import('./types').RepairHistoryRecord[]> {
    const raw = await invokeCommand<unknown>('get_repair_history', { limit, offset });
    return asArray<import('./types').RepairHistoryRecord>(raw);
}

/**
 * S165: Read-only forensic audit across 16 categories of catalog consistency
 */
export async function auditCatalogIdentity(): Promise<import('./types').CatalogIdentityAuditReport> {
    return invokeCommand<import('./types').CatalogIdentityAuditReport>('audit_catalog_identity');
}

/**
 * S165: Generate a non-mutating Dry-Run plan for catalog identity repair
 */
export async function planCatalogIdentityRepair(): Promise<import('./types').CatalogRepairPlan> {
    return invokeCommand<import('./types').CatalogRepairPlan>('plan_catalog_identity_repair');
}

/**
 * S165: Apply catalog repair plan with explicit confirmation, SHA-256 backup, and append-only audit trail
 */
export async function applyCatalogIdentityRepair(
    plan: import('./types').CatalogRepairPlan,
    confirmed: boolean
): Promise<import('./types').CatalogRepairExecutionReport> {
    return invokeCommand<import('./types').CatalogRepairExecutionReport>('apply_catalog_identity_repair', { plan, confirmed });
}

/**
 * S167: Query aggregate post-crash recovery audit summary and details
 */
export async function getRecoveryAuditSummary(): Promise<import('./types').RecoveryAuditSummary> {
    return invokeCommand<import('./types').RecoveryAuditSummary>('get_recovery_audit_summary');
}

/**
 * S167: Trigger manual/startup post-crash reconciliation
 */
export async function triggerStartupReconciliation(): Promise<import('./types').RecoveryAuditSummary> {
    return invokeCommand<import('./types').RecoveryAuditSummary>('trigger_startup_reconciliation');
}

/**
 * S168: Get concurrency statistics summary
 */
export async function getConcurrencyStatsSummary(): Promise<import('./types').ConcurrencyStatsSummary> {
    return invokeCommand<import('./types').ConcurrencyStatsSummary>('get_concurrency_stats_summary');
}

/**
 * S168: Get active redacted concurrency lock hashes
 */
export async function getActiveConcurrencyLocks(): Promise<string[]> {
    return invokeCommand<string[]>('get_active_concurrency_locks');
}

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
};


