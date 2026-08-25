/**
 * Lyrics API
 * 
 * Tauri commands for lyrics fetching and management.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asRecord } from './normalize';
import type { Lyrics, LyricsSearchResult } from './types';

/**
 * Normalize batch-operation counters so missing fields default to 0.
 */
function normalizeBatchCounts(raw: unknown): { fetched: number; failed: number; skipped: number } {
    const rec = asRecord(raw);
    return {
        fetched: asNumber(rec?.fetched),
        failed: asNumber(rec?.failed),
        skipped: asNumber(rec?.skipped),
    };
}

// ==============================================
// LYRICS QUERIES
// ==============================================

/**
 * Get lyrics for a specific track
 */
export async function getLyrics(trackId: number): Promise<Lyrics | null> {
    return invokeCommand<Lyrics | null>('get_lyrics', { trackId });
}

/**
 * Get all lyrics (for batch processing)
 */
export async function getAllLyrics(options?: {
    limit?: number;
    offset?: number;
    format?: 'ttml' | 'lrc' | 'plain';
}): Promise<Lyrics[]> {
    const raw = await invokeCommand<unknown>('get_all_lyrics', options || {});
    return asArray<Lyrics>(raw);
}

/**
 * Search for lyrics online
 */
export async function searchLyrics(params: {
    title: string;
    artist: string;
    album?: string;
    durationMs?: number;
}): Promise<LyricsSearchResult[]> {
    const raw = await invokeCommand<unknown>('search_lyrics', params);
    return asArray<LyricsSearchResult>(raw);
}

// ==============================================
// LYRICS FETCHING
// ==============================================

/**
 * Fetch lyrics for a track from online sources and save to database
 */
export async function fetchLyrics(trackId: number): Promise<Lyrics | null> {
    return invokeCommand<Lyrics | null>('fetch_and_save_lyrics', { trackId });
}

/**
 * Batch fetch missing lyrics
 */
export async function batchFetchLyrics(trackIds: number[]): Promise<{
    fetched: number;
    failed: number;
    skipped: number;
}> {
    return normalizeBatchCounts(await invokeCommand<unknown>('batch_fetch_lyrics', { trackIds }));
}

/**
 * Batch fetch lyrics with real-time progress events
 * Listen to 'lyrics-fetch-progress' event for updates
 */
export async function batchFetchLyricsWithProgress(trackIds: number[]): Promise<{
    fetched: number;
    failed: number;
    skipped: number;
}> {
    return normalizeBatchCounts(await invokeCommand<unknown>('batch_fetch_lyrics_with_progress', { trackIds }));
}

/**
 * Fetch all missing lyrics (auto-detect)
 */
export async function fetchMissingLyrics(): Promise<{
    fetched: number;
    failed: number;
    skipped: number;
}> {
    return normalizeBatchCounts(await invokeCommand<unknown>('fetch_missing_lyrics'));
}

// ==============================================
// LYRICS MANAGEMENT
// ==============================================

/**
 * Save/update lyrics for a track
 */
export async function saveLyrics(params: {
    trackId: number;
    format: 'ttml' | 'lrc' | 'plain';
    content: string;
    syncLevel?: 'syllable' | 'word' | 'line' | 'none';
    source?: string;
    language?: string;
}): Promise<Lyrics> {
    return invokeCommand<Lyrics>('save_lyrics', {
        params: {
            trackId: params.trackId,
            format: params.format,
            content: params.content,
            syncLevel: params.syncLevel,
            source: params.source,
            language: params.language,
        }
    });
}

/**
 * Delete lyrics for a track
 */
export async function deleteLyrics(trackId: number, format?: string): Promise<void> {
    return invokeCommand<void>('delete_lyrics', { trackId, format });
}

/**
 * S192: associate an external lyrics file (.lrc / .txt) with a track.
 * Format is detected server-side from content; persisted as source=manual_import.
 */
export async function importLyricsFile(trackId: number, filePath: string): Promise<Lyrics> {
    return invokeCommand<Lyrics>('import_lyrics_file', { trackId, filePath });
}

/**
 * Embed lyrics into audio file
 */
export async function embedLyrics(trackId: number): Promise<boolean> {
    return invokeCommand<boolean>('embed_lyrics', { trackId });
}

/**
 * Batch embed lyrics into files
 */
export async function batchEmbedLyrics(trackIds: number[]): Promise<{
    embedded: number;
    failed: number;
    skipped: number;
}> {
    const raw = await invokeCommand<unknown>('batch_embed_lyrics', { trackIds });
    return {
        embedded: asNumber(asRecord(raw)?.embedded),
        failed: asNumber(asRecord(raw)?.failed),
        skipped: asNumber(asRecord(raw)?.skipped),
    };
}

// ==============================================
// LYRICS STATS
// ==============================================

/**
 * Get lyrics coverage statistics
 */
export async function getLyricsStats(): Promise<{
    total_tracks: number;
    with_lyrics: number;
    synced_lyrics: number;
    embedded_lyrics: number;
}> {
    const raw = await invokeCommand<unknown>('get_lyrics_stats');
    return {
        total_tracks: asNumber(asRecord(raw)?.total_tracks),
        with_lyrics: asNumber(asRecord(raw)?.with_lyrics),
        synced_lyrics: asNumber(asRecord(raw)?.synced_lyrics),
        embedded_lyrics: asNumber(asRecord(raw)?.embedded_lyrics),
    };
}

// Export as namespace
export const lyricsApi = {
    getLyrics,
    getAllLyrics,
    searchLyrics,
    fetchLyrics,
    batchFetchLyrics,
    batchFetchLyricsWithProgress,
    fetchMissingLyrics,
    saveLyrics,
    importLyricsFile,
    deleteLyrics,
    embedLyrics,
    batchEmbedLyrics,
    getLyricsStats,
};
