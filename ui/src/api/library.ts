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
 * Get favorite tracks in the library (paginated)
 */
export async function getFavoriteTracks(offset?: number, limit?: number): Promise<LibraryPage> {
    return invokeCommand<LibraryPage>('get_favorite_tracks', { offset, limit });
}

/**
 * Toggle favorite status of a track
 */
export async function toggleTrackFavorite(trackId: number): Promise<boolean> {
    return invokeCommand<boolean>('toggle_favorite', { trackId });
}

/**
 * Set favorite status of a track explicitly
 */
export async function setTrackFavorite(trackId: number, isFavorite: boolean): Promise<boolean> {
    return invokeCommand<boolean>('set_track_favorite', { trackId, isFavorite });
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
 * Get favorite tracks with optional service filter
 */
export async function getFavoritesTracks(service?: string, offset?: number, limit?: number): Promise<any[]> {
    return invokeCommand<any[]>('get_favorites_tracks', { service, offset, limit });
}

/**
 * Get favorite albums with optional service filter
 */
export async function getFavoritesAlbums(service?: string, offset?: number, limit?: number): Promise<any[]> {
    return invokeCommand<any[]>('get_favorites_albums', { service, offset, limit });
}

/**
 * Get favorite artists with optional service filter
 */
export async function getFavoritesArtists(service?: string, offset?: number, limit?: number): Promise<any[]> {
    return invokeCommand<any[]>('get_favorites_artists', { service, offset, limit });
}

/**
 * Sync favorites from a service
 */
export async function syncFavorites(service: string, favType?: string): Promise<any> {
    return invokeCommand<any>('sync_favorites', { service, favType });
}

/**
 * Toggle favorite status of an album
 */
export async function toggleAlbumFavorite(albumId: number): Promise<boolean> {
    return invokeCommand<boolean>('toggle_album_favorite', { albumId });
}

/**
 * Toggle favorite status of an artist
 */
export async function toggleArtistFavorite(artistId: number): Promise<boolean> {
    return invokeCommand<boolean>('toggle_artist_favorite', { artistId });
}

/**
 * Push favorite update to external service
 */
export async function pushFavoriteToService(
    service: string,
    itemType: string,
    serviceItemId: string,
    isFavorite: boolean
): Promise<any> {
    return invokeCommand<any>('push_favorite_to_service', {
        service,
        itemType,
        serviceItemId,
        isFavorite,
    });
}

/**
 * Enqueue a track for download
 */
export async function enqueueDownload(trackId: number, priority?: number, qualityPreference?: string): Promise<number> {
    return invokeCommand<number>('enqueue_download', { trackId, priority, qualityPreference });
}

/**
 * Reorder download queue (drag-and-drop)
 */
export async function reorderQueue(queueIds: number[]): Promise<void> {
    return invokeCommand<void>('reorder_queue', { queueIds });
}

/**
 * Retry failed download(s)
 */
export async function retryFailed(queueId?: number): Promise<number> {
    return invokeCommand<number>('retry_failed', { queueId });
}

/**
 * Cancel a download
 */
export async function cancelDownload(queueId: number): Promise<void> {
    return invokeCommand<void>('cancel_download', { queueId });
}

/**
 * Clear completed downloads
 */
export async function clearCompleted(status?: string): Promise<number> {
    return invokeCommand<number>('clear_completed', { status });
}

export interface EnrichmentStatus {
    is_paused: boolean;
    active_jobs: number;
    pending_count: number;
    completed_count: number;
    failed_count: number;
}

/**
 * Start background enrichment worker
 */
export async function startEnrichmentWorker(): Promise<void> {
    return invokeCommand<void>('start_enrichment_worker');
}

/**
 * Pause background enrichment worker
 */
export async function pauseEnrichmentWorker(): Promise<void> {
    return invokeCommand<void>('pause_enrichment_worker');
}

/**
 * Resume background enrichment worker
 */
export async function resumeEnrichmentWorker(): Promise<void> {
    return invokeCommand<void>('resume_enrichment_worker');
}

/**
 * Get background enrichment status
 */
export async function getEnrichmentStatus(): Promise<EnrichmentStatus> {
    return invokeCommand<EnrichmentStatus>('get_enrichment_status');
}

export interface DownloadFavoritesResult {
    total_candidates: number;
    enqueued: number;
    already_downloaded: number;
    already_queued: number;
    message: string;
}

/**
 * Download favorites matching service and item type filters
 */
export async function downloadFavorites(
    service?: string,
    itemType?: string,
    qualityPreference?: string,
    priority?: number,
): Promise<DownloadFavoritesResult> {
    return invokeCommand<DownloadFavoritesResult>('download_favorites', {
        service,
        itemType,
        qualityPreference,
        priority,
    });
}

export interface IntegrityAuditReport {
    total_tracks_scanned: number;
    verified_files: number;
    missing_files: string[];
    orphan_files: string[];
    corrupt_or_zero_byte_files: string[];
    abandoned_staging_files: string[];
    database_inconsistencies: string[];
    is_healthy: boolean;
    timestamp: string;
}

export interface IntegrityRepairResult {
    purged_staging_files: number;
    cleaned_database_entries: number;
    message: string;
}

/**
 * Run a full physical and database library integrity audit
 */
export async function runIntegrityAudit(downloadDir?: string): Promise<IntegrityAuditReport> {
    return invokeCommand<IntegrityAuditReport>('run_integrity_audit', { downloadDir });
}

/**
 * Repair detected library integrity issues
 */
export async function repairIntegrityIssues(stagingFilesToPurge?: string[]): Promise<IntegrityRepairResult> {
    return invokeCommand<IntegrityRepairResult>('repair_integrity_issues', { stagingFilesToPurge });
}

export interface ExportLibraryResult {
    file_path: string;
    tracks_count: number;
    albums_count: number;
    artists_count: number;
    playlists_count: number;
    file_size_bytes: number;
    checksum: string;
}

export interface ImportLibraryResult {
    tracks_imported: number;
    albums_imported: number;
    artists_imported: number;
    playlists_imported: number;
    favorites_restored: number;
    message: string;
}

/**
 * Export library to a portable JSON backup file
 */
export async function exportLibrary(outputPath?: string): Promise<ExportLibraryResult> {
    return invokeCommand<ExportLibraryResult>('export_library', { outputPath });
}

/**
 * Import and restore library from a backup JSON file
 */
export async function importLibrary(filePath: string, ignoreChecksumError?: boolean): Promise<ImportLibraryResult> {
    return invokeCommand<ImportLibraryResult>('import_library', { filePath, ignoreChecksumError });
}

export interface SearchResultTrack {
    id: number;
    title: string;
    artist_name?: string;
    album_name?: string;
    album_id?: number;
    duration_ms?: number;
    isrc?: string;
    is_favorite: boolean;
    services?: string;
    quality?: string;
    download_status: string;
}

export interface SearchResultAlbum {
    id: number;
    title: string;
    artist_name?: string;
    release_year?: number;
    cover_art_url?: string;
    track_count: number;
    is_favorite: boolean;
}

export interface SearchResultArtist {
    id: number;
    name: string;
    is_favorite: boolean;
    track_count: number;
    album_count: number;
}

export interface SearchResultPlaylist {
    id: number;
    name: string;
    description?: string;
    track_count: number;
    service_name?: string;
}

export interface UnifiedSearchResult {
    query: string;
    tracks: SearchResultTrack[];
    albums: SearchResultAlbum[];
    artists: SearchResultArtist[];
    playlists: SearchResultPlaylist[];
    total_tracks: number;
    total_albums: number;
    total_artists: number;
    total_playlists: number;
}

export interface SearchLibraryParams {
    query: string;
    entity_type?: string;
    service?: string;
    only_favorites?: boolean;
    download_status?: string;
    offset?: number;
    limit?: number;
}

/**
 * High-performance unified search across library
 */
export async function searchLibrary(params: SearchLibraryParams): Promise<UnifiedSearchResult> {
    return invokeCommand<UnifiedSearchResult>('search_library', { params });
}

// Export as namespace
export const libraryApi = {
    getLibrary,
    getDuplicateTracks,
    getFavoriteTracks,
    getFavoritesTracks,
    getFavoritesAlbums,
    getFavoritesArtists,
    syncFavorites,
    pushFavoriteToService,
    downloadFavorites,
    runIntegrityAudit,
    repairIntegrityIssues,
    exportLibrary,
    importLibrary,
    searchLibrary,
    toggleAlbumFavorite,
    toggleArtistFavorite,
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
    toggleTrackFavorite,
    setTrackFavorite,
    showInFolder,
    getAlbum,
    getArtist,
    getTopArtists,
    getAudioQualityDistribution,
};



