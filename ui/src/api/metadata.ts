/**
 * Metadata API
 * 
 * Tauri commands for metadata enrichment and management.
 */

import { invokeCommand } from './tauri';
import type { LibraryTrack, MetadataMatch, MetadataStats } from './types';

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
    getTracksNeedingMetadata
};
