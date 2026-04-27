/**
 * Library API
 * 
 * Tauri commands for library management and local scanning.
 */

import { invokeCommand } from './tauri';
import type { LibraryTrack, LibraryPage, LibraryStats, Playlist, SearchResult, AlbumDetail, ArtistDetail, TopArtist, TopGenre, QualityBucket } from './types';

// ==============================================
// SCAN TYPES
// ==============================================

export interface ScanResult {
    success: boolean;
    data?: {
        directory: string;
        total_files: number;
        tracks: ScannedTrackInfo[];
        errors?: { file: string; error: string }[];
    };
    error?: string;
}

export interface ScannedTrackInfo {
    file_path: string;
    file_name: string;
    file_size: number;
    format: string;
    title?: string;
    artist?: string;
    album?: string;
    album_artist?: string;
    track_number?: number;
    disc_number?: number;
    year?: number;
    genre?: string;
    duration_seconds?: number;
    bitrate?: number;
    sample_rate?: number;
    channels?: number;
    has_cover_art: boolean;
}

/**
 * Get tracks in the library (paginated)
 */
export async function getLibrary(offset?: number, limit?: number): Promise<LibraryPage> {
    return invokeCommand<LibraryPage>('get_library', { offset, limit });
}

/**
 * Get duplicate tracks (by Title + Primary Artist) (paginated)
 */
export async function getDuplicateTracks(offset?: number, limit?: number): Promise<LibraryPage> {
    return invokeCommand<LibraryPage>('get_duplicate_tracks', { offset, limit });
}

/**
 * Get library statistics
 */
export async function getLibraryStats(): Promise<LibraryStats> {
    return invokeCommand<LibraryStats>('get_library_stats');
}

/**
 * Search tracks using FTS5 (paginated)
 */
export async function searchTracks(
    query: string,
    offset?: number,
    limit?: number
): Promise<SearchResult> {
    return invokeCommand<SearchResult>('search_tracks', { query, offset, limit });
}

/**
 * Get all playlists
 */
export async function getPlaylists(): Promise<Playlist[]> {
    return invokeCommand<Playlist[]>('get_playlists');
}

/**
 * Get tracks in a playlist (paginated)
 */
export async function getPlaylistTracks(
    playlistId: number,
    offset?: number,
    limit?: number
): Promise<LibraryPage> {
    return invokeCommand<LibraryPage>('get_local_playlist_tracks', {
        playlistId: playlistId,
        offset,
        limit,
    });
}

/**
 * Add tracks to a playlist
 * @param playlistId - The ID of the playlist to add tracks to
 * @param trackIds - Array of track IDs to add
 * @returns Success message with count of tracks added
 */
export async function addToPlaylist(playlistId: number, trackIds: number[]): Promise<string> {
    return invokeCommand<string>('add_to_playlist', { playlistId, trackIds });
}

/**
 * Create a new local playlist
 * @param accountId - The account ID to associate the playlist with
 * @param name - Name of the new playlist
 * @param description - Optional description
 * @returns The ID of the newly created playlist
 */
export async function createPlaylist(
    accountId: number,
    name: string,
    description?: string
): Promise<number> {
    return invokeCommand<number>('create_playlist', { accountId, name, description });
}

/**
 * Queue tracks for download
 */
export async function queueDownloads(trackIds: number[]): Promise<string> {
    return invokeCommand<string>('queue_downloads', { trackIds });
}

// ==============================================
// LOCAL LIBRARY SCAN COMMANDS
// ==============================================

/**
 * Scan a directory for audio files
 */
export async function scanLocalLibrary(
    directory: string,
    options?: {
        recursive?: boolean;
        limit?: number;
    }
): Promise<ScanResult> {
    return invokeCommand<ScanResult>('scan_local_library', {
        directory,
        recursive: options?.recursive ?? true,
        limit: options?.limit ?? null,
    });
}

/**
 * Scan a directory for audio files with progress events
 */
export async function scanLocalLibraryWithProgress(
    directory: string,
    options?: {
        recursive?: boolean;
    }
): Promise<ScanResult> {
    return invokeCommand<ScanResult>('scan_local_library_with_progress', {
        directory,
        recursive: options?.recursive ?? true,
    });
}

/**
 * Remove a single track from the library
 */
export async function removeTrack(trackId: number): Promise<void> {
    return invokeCommand<void>('remove_track', { trackId });
}

/**
 * Bulk remove tracks from the library
 */
export async function bulkRemoveTracks(trackIds: number[]): Promise<number> {
    return invokeCommand<number>('bulk_remove_tracks', { trackIds });
}

/**
 * Toggle favorite status of a track
 * @returns The new favorite state (true = favorited)
 */
export async function toggleFavorite(trackId: number): Promise<boolean> {
    return invokeCommand<boolean>('toggle_favorite', { trackId });
}

/**
 * Open the file explorer and reveal the track's file
 */
export async function showInFolder(trackId: number): Promise<void> {
    return invokeCommand<void>('show_in_folder', { trackId });
}

/**
 * Get metadata for a single audio file
 */
export async function getLocalTrackMetadata(filePath: string): Promise<ScanResult> {
    return invokeCommand<ScanResult>('get_local_track_metadata', { filePath });
}

/**
 * Get album detail with tracks
 */
export async function getAlbum(albumId: number): Promise<AlbumDetail> {
    return invokeCommand<AlbumDetail>('get_album', {
        albumId: albumId,
    });
}

/**
 * Get artist detail with albums and top tracks
 */
export async function getArtist(artistId: number): Promise<ArtistDetail> {
    return invokeCommand<ArtistDetail>('get_artist', { artistId: artistId });
}

/**
 * Get top artists by track count
 */
export async function getTopArtists(limit: number = 5): Promise<TopArtist[]> {
    return invokeCommand<TopArtist[]>('get_top_artists', { limit });
}


/**
 * Get audio quality distribution from downloads
 */
export async function getAudioQualityDistribution(): Promise<QualityBucket[]> {
    return invokeCommand<QualityBucket[]>('get_audio_quality_distribution');
}

// Export as namespace
export const libraryApi = {
    getLibrary,
    getDuplicateTracks,
    getLibraryStats,
    searchTracks,
    getPlaylists,
    getPlaylistTracks,
    addToPlaylist,
    createPlaylist,
    queueDownloads,
    scanLocalLibrary,
    scanLocalLibraryWithProgress,
    getLocalTrackMetadata,
    removeTrack,
    bulkRemoveTracks,
    toggleFavorite,
    showInFolder,
    getAlbum,
    getArtist,
    getTopArtists,
    getAudioQualityDistribution,
};

