/**
 * Lyrics API
 * 
 * Tauri commands for lyrics fetching and management.
 */

import { invokeCommand } from './tauri';
import type { Lyrics, LyricsSearchResult } from './types';

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
    return invokeCommand<Lyrics[]>('get_all_lyrics', options || {});
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
    return invokeCommand<LyricsSearchResult[]>('search_lyrics', params);
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
    return invokeCommand<{ fetched: number; failed: number; skipped: number }>('batch_fetch_lyrics', { trackIds });
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
    return invokeCommand<{ fetched: number; failed: number; skipped: number }>('batch_fetch_lyrics_with_progress', { trackIds });
}

/**
 * Fetch all missing lyrics (auto-detect)
 */
export async function fetchMissingLyrics(): Promise<{
    fetched: number;
    failed: number;
    skipped: number;
}> {
    return invokeCommand<{ fetched: number; failed: number; skipped: number }>('fetch_missing_lyrics');
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
    return invokeCommand<{ embedded: number; failed: number; skipped: number }>('batch_embed_lyrics', { trackIds });
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
    return invokeCommand<{
        total_tracks: number;
        with_lyrics: number;
        synced_lyrics: number;
        embedded_lyrics: number;
    }>('get_lyrics_stats');
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
    deleteLyrics,
    embedLyrics,
    batchEmbedLyrics,
    getLyricsStats,
};
