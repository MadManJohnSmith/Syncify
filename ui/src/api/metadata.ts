/**
 * Metadata API
 * 
 * Tauri commands for metadata enrichment and management.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asString, asRecord, pick, pickArray, pickNumber } from './normalize';
import type { LibraryTrack, MetadataMatch, MetadataStats } from './types';

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
    return {
        enriched: pickNumber(raw, ['enriched']),
        failed: pickNumber(raw, ['failed']),
        skipped: pickNumber(raw, ['skipped']),
    };
}

/**
 * Enrich all tracks needing metadata
 */
export async function enrichAllNeeding(): Promise<{
    total: number;
    enriched: number;
    failed: number;
}> {
    const raw = await invokeCommand<unknown>('enrich_all_needing_metadata');
    return {
        total: pickNumber(raw, ['total']),
        enriched: pickNumber(raw, ['enriched']),
        failed: pickNumber(raw, ['failed']),
    };
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
 * Auto-match tracks against MusicBrainz
 */
export async function autoMatchMusicBrainz(trackIds: number[]): Promise<{
    matched: number;
    failed: number;
    noMatch: number;
}> {
    const raw = await invokeCommand<unknown>('auto_match_musicbrainz', { trackIds });
    return {
        matched: pickNumber(raw, ['matched']),
        failed: pickNumber(raw, ['failed']),
        noMatch: pickNumber(raw, ['noMatch', 'no_match']),
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
// METADATA EDITING
// ==============================================

/**
 * Write metadata to audio file
 */
export async function writeMetadataToFile(trackId: number): Promise<boolean> {
    return invokeCommand<boolean>('write_metadata_to_file', { trackId });
}

/**
 * Read metadata from audio file
 */
export async function readMetadataFromFile(filePath: string): Promise<Partial<LibraryTrack>> {
    return invokeCommand<Partial<LibraryTrack>>('read_metadata_from_file', { filePath });
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
    fetchMissingCoverArt,
    matchMusicBrainz,
    applyMusicBrainzMatch,
    autoMatchMusicBrainz,
    checkFingerprintAvailable,
    identifyAudio,
    findAudioDuplicates,
    updateTrackMetadata,
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


