/**
 * Metadata API
 * 
 * Tauri commands for metadata enrichment and management.
 */

import { invokeCommand } from './tauri';
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
 */
export async function getMetadataStats(): Promise<MetadataStats> {
    return invokeCommand<MetadataStats>('get_metadata_stats');
}

/**
 * Get tracks that are missing core metadata
 */
export async function getTracksNeedingMetadata(limit: number = 100): Promise<LibraryTrack[]> {
    return invokeCommand<LibraryTrack[]>('get_tracks_needing_metadata', { limit });
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
    return invokeCommand<{
        success: boolean;
        updatedFields: string[];
        error?: string;
    }>('enrich_metadata', { trackId });
}

/**
 * Batch enrich metadata for multiple tracks
 */
export async function batchEnrichMetadata(trackIds: number[]): Promise<{
    enriched: number;
    failed: number;
    skipped: number;
}> {
    return invokeCommand<{
        enriched: number;
        failed: number;
        skipped: number;
    }>('batch_enrich_metadata', { tracks: trackIds });
}

/**
 * Enrich all tracks needing metadata
 */
export async function enrichAllNeeding(): Promise<{
    total: number;
    enriched: number;
    failed: number;
}> {
    return invokeCommand<{
        total: number;
        enriched: number;
        failed: number;
    }>('enrich_all_needing_metadata');
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
    return invokeCommand<MetadataMatch[]>('match_musicbrainz', { params });
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
    return invokeCommand<{
        matched: number;
        failed: number;
        noMatch: number;
    }>('auto_match_musicbrainz', { trackIds });
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
    return invokeCommand<MetadataMatch[]>('identify_audio', { filePath });
}

/**
 * Find duplicate audio files
 */
export async function findAudioDuplicates(): Promise<{
    groups: Array<{ fingerprint: string; tracks: LibraryTrack[] }>;
    totalDuplicates: number;
}> {
    return invokeCommand<{
        groups: Array<{ fingerprint: string; tracks: LibraryTrack[] }>;
        totalDuplicates: number;
    }>('find_audio_duplicates');
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
    return invokeCommand<import('./types').DownloadRepairDryRunItem[]>('get_tidal_repair_dry_run');
}

/**
 * S163: Query persistent, append-only historical audit records of applied repairs (read-only)
 */
export async function getRepairHistory(limit: number = 100, offset: number = 0): Promise<import('./types').RepairHistoryRecord[]> {
    return invokeCommand<import('./types').RepairHistoryRecord[]>('get_repair_history', { limit, offset });
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


