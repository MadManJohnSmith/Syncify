/**
 * Library API
 * 
 * Tauri commands for library management and local scanning.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asString, asBoolean, asRecord, pick, pickArray, pickNumber, optionalNumber } from './normalize';
import type { LibraryTrack, LibraryPage, LibraryStats, Playlist, SearchResult, AlbumDetail, ArtistDetail, TopArtist, TopGenre, QualityBucket, TrackSourceAvailability, TrackPreflightResult } from './types';

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
 * Normalize a paginated library response so missing fields can never crash rendering.
 */
function normalizeLibraryPage(raw: unknown, offset?: number, limit?: number): LibraryPage {
    const rec = asRecord(raw);
    return {
        tracks: asArray<LibraryTrack>(rec?.tracks),
        total: asNumber(rec?.total),
        offset: asNumber(rec?.offset, offset ?? 0),
        limit: asNumber(rec?.limit, limit ?? 0),
        has_more: rec?.has_more === true,
    };
}

/**
 * Get tracks in the library (paginated)
 */
export async function getLibrary(offset?: number, limit?: number): Promise<LibraryPage> {
    return normalizeLibraryPage(await invokeCommand<unknown>('get_library', { offset, limit }), offset, limit);
}

/**
 * Get favorite tracks in the library (paginated)
 */
export async function getFavoriteTracks(offset?: number, limit?: number): Promise<LibraryPage> {
    return normalizeLibraryPage(await invokeCommand<unknown>('get_favorite_tracks', { offset, limit }), offset, limit);
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
    return normalizeLibraryPage(await invokeCommand<unknown>('get_duplicate_tracks', { offset, limit }), offset, limit);
}

/**
 * Get library statistics
 */
export async function getLibraryStats(): Promise<LibraryStats> {
    const raw = await invokeCommand<unknown>('get_library_stats');
    return {
        total_tracks: pickNumber(raw, ['total_tracks', 'totalTracks']),
        total_artists: pickNumber(raw, ['total_artists', 'totalArtists']),
        total_albums: pickNumber(raw, ['total_albums', 'totalAlbums']),
        total_downloads: pickNumber(raw, ['total_downloads', 'totalDownloads']),
        queued_downloads: pickNumber(raw, ['queued_downloads', 'queuedDownloads']),
        active_downloads: pickNumber(raw, ['active_downloads', 'activeDownloads']),
        library_entries: pickNumber(raw, ['library_entries', 'libraryEntries']),
        playlists: pickNumber(raw, ['playlists']),
        services_with_data: pickNumber(raw, ['services_with_data', 'servicesWithData']),
    };
}

/**
 * Search tracks using FTS5 (paginated)
 */
export async function searchTracks(
    query: string,
    offset?: number,
    limit?: number
): Promise<SearchResult> {
    return normalizeLibraryPage(await invokeCommand<unknown>('search_tracks', { query, offset, limit }), offset, limit);
}

/**
 * Get all playlists
 */
export async function getPlaylists(): Promise<Playlist[]> {
    const raw = await invokeCommand<unknown>('get_playlists');
    return asArray<Playlist>(raw);
}

/**
 * Get tracks in a playlist (paginated)
 */
export async function getPlaylistTracks(
    playlistId: number,
    offset?: number,
    limit?: number
): Promise<LibraryPage> {
    return normalizeLibraryPage(
        await invokeCommand<unknown>('get_local_playlist_tracks', {
            playlistId: playlistId,
            offset,
            limit,
        }),
        offset,
        limit
    );
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
    const id = await invokeCommand<unknown>('create_playlist', { accountId, name, description });
    return typeof id === 'number' ? id : 0;
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
    const removed = await invokeCommand<unknown>('bulk_remove_tracks', { trackIds });
    return typeof removed === 'number' ? removed : 0;
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
    const raw = await invokeCommand<unknown>('get_top_artists', { limit });
    return asArray<TopArtist>(raw);
}


// ==============================================
// FAVORITES (contracts verified against src-tauri/src/commands/favorites.rs)
// ==============================================

export interface FavoriteTrackItem {
    id: number;
    service_track_id?: string;
    title?: string;
    artist?: string;
    album?: string | null;
    isrc?: string | null;
    cover_art_url?: string | null;
    service?: string;
    favorited_at?: string | null;
}

export interface FavoriteAlbumItem {
    id: number;
    service_album_id?: string;
    title?: string;
    artist?: string;
    upc?: string | null;
    cover_art_url?: string | null;
    service?: string;
    total_tracks?: number | null;
    release_date?: string | null;
    favorited_at?: string | null;
}

export interface FavoriteArtistItem {
    id: number;
    service_artist_id?: string;
    name?: string;
    image_url?: string | null;
    service?: string;
    favorited_at?: string | null;
}

export interface FavoritesSyncResult {
    service?: string;
    item_type?: string;
    total_found?: number;
    imported?: number;
    cached?: number;
    message?: string;
}

export interface PushFavoriteResponse {
    service?: string;
    item_type?: string;
    service_item_id?: string;
    is_favorite?: boolean;
    status?: string;
    message?: string;
}

/**
 * Get favorite tracks with optional service filter
 */
export async function getFavoritesTracks(service?: string, offset?: number, limit?: number): Promise<FavoriteTrackItem[]> {
    const raw = await invokeCommand<unknown>('get_favorites_tracks', { service, offset, limit });
    return asArray<FavoriteTrackItem>(raw);
}

/**
 * Get favorite albums with optional service filter
 */
export async function getFavoritesAlbums(service?: string, offset?: number, limit?: number): Promise<FavoriteAlbumItem[]> {
    const raw = await invokeCommand<unknown>('get_favorites_albums', { service, offset, limit });
    return asArray<FavoriteAlbumItem>(raw);
}

/**
 * Get favorite artists with optional service filter
 */
export async function getFavoritesArtists(service?: string, offset?: number, limit?: number): Promise<FavoriteArtistItem[]> {
    const raw = await invokeCommand<unknown>('get_favorites_artists', { service, offset, limit });
    return asArray<FavoriteArtistItem>(raw);
}

/**
 * Sync favorites from a service
 */
export async function syncFavorites(service: string, favType?: string): Promise<FavoritesSyncResult> {
    const raw = await invokeCommand<unknown>('sync_favorites', { service, favType });
    return {
        service: asString(pick(raw, ['service'])),
        item_type: asString(pick(raw, ['item_type', 'itemType'])),
        total_found: pickNumber(raw, ['total_found', 'totalFound']),
        imported: pickNumber(raw, ['imported']),
        cached: pickNumber(raw, ['cached']),
        message: asString(pick(raw, ['message'])),
    };
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
): Promise<PushFavoriteResponse> {
    const raw = await invokeCommand<unknown>('push_favorite_to_service', {
        service,
        itemType,
        serviceItemId,
        isFavorite,
    });
    return {
        service: asString(pick(raw, ['service'])),
        item_type: asString(pick(raw, ['item_type', 'itemType'])),
        service_item_id: asString(pick(raw, ['service_item_id', 'serviceItemId'])),
        is_favorite: pick(raw, ['is_favorite', 'isFavorite']) === true,
        status: asString(pick(raw, ['status'])),
        message: asString(pick(raw, ['message'])),
    };
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
    const retried = await invokeCommand<unknown>('retry_failed', { queueId });
    return typeof retried === 'number' ? retried : 0;
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
    const cleared = await invokeCommand<unknown>('clear_completed', { status });
    return typeof cleared === 'number' ? cleared : 0;
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
    const raw = await invokeCommand<unknown>('get_enrichment_status');
    return {
        is_paused: pick(raw, ['is_paused', 'isPaused']) === true,
        active_jobs: pickNumber(raw, ['active_jobs', 'activeJobs']),
        pending_count: pickNumber(raw, ['pending_count', 'pendingCount']),
        completed_count: pickNumber(raw, ['completed_count', 'completedCount']),
        failed_count: pickNumber(raw, ['failed_count', 'failedCount']),
    };
}

export interface DownloadFavoritesResult {
    total_candidates: number;
    enqueued: number;
    already_downloaded: number;
    already_queued: number;
    unresolved_sources?: number;
    stale_sources?: number;
    ambiguous_sources?: number;
    ready_exact?: number;
    ready_fallback?: number;
    no_download_provider?: number;
    is_preflight?: boolean;
    estimated_size_mb?: number;
    message: string;
}

/**
 * Download favorites matching service and item type filters with optional batch limit and dry_run preflight
 */
export async function downloadFavorites(
    service?: string,
    itemType?: string,
    qualityPreference?: string,
    priority?: number,
    limit?: number,
    dryRun?: boolean,
): Promise<DownloadFavoritesResult> {
    console.debug('[library.ts] invoke download_favorites payload:', {
        service,
        itemType,
        qualityPreference,
        priority,
        limit,
        dryRun,
    });
    return invokeCommand<DownloadFavoritesResult>('download_favorites', {
        service,
        itemType,
        qualityPreference,
        priority,
        limit,
        dryRun,
    }).then(normalizeDownloadFavoritesResult);
}

function normalizeDownloadFavoritesResult(raw: unknown): DownloadFavoritesResult {
    return {
        total_candidates: pickNumber(raw, ['total_candidates', 'totalCandidates']),
        enqueued: pickNumber(raw, ['enqueued']),
        already_downloaded: pickNumber(raw, ['already_downloaded', 'alreadyDownloaded']),
        already_queued: pickNumber(raw, ['already_queued', 'alreadyQueued']),
        unresolved_sources: optionalNumber(pick(raw, ['unresolved_sources', 'unresolvedSources'])),
        stale_sources: optionalNumber(pick(raw, ['stale_sources', 'staleSources'])),
        ambiguous_sources: optionalNumber(pick(raw, ['ambiguous_sources', 'ambiguousSources'])),
        ready_exact: optionalNumber(pick(raw, ['ready_exact', 'readyExact'])),
        ready_fallback: optionalNumber(pick(raw, ['ready_fallback', 'readyFallback'])),
        no_download_provider: optionalNumber(pick(raw, ['no_download_provider', 'noDownloadProvider'])),
        is_preflight: pick(raw, ['is_preflight', 'isPreflight']) === true ? true : undefined,
        estimated_size_mb: optionalNumber(pick(raw, ['estimated_size_mb', 'estimatedSizeMb'])),
        message: asString(pick(raw, ['message'])),
    };
}

export interface QueueAuditReport {
    total_items: number;
    ready_count: number;
    source_locked_count: number;
    legacy_unresolved_count: number;
    stale_source_count: number;
    ambiguous_source_count: number;
    source_identity_missing_count: number;
    completed_count: number;
    failed_count: number;
    downloading_count: number;
}

export async function auditDownloadQueue(): Promise<QueueAuditReport> {
    const raw = await invokeCommand<unknown>('audit_download_queue');
    return {
        total_items: pickNumber(raw, ['total_items', 'totalItems']),
        ready_count: pickNumber(raw, ['ready_count', 'readyCount']),
        source_locked_count: pickNumber(raw, ['source_locked_count', 'sourceLockedCount']),
        legacy_unresolved_count: pickNumber(raw, ['legacy_unresolved_count', 'legacyUnresolvedCount']),
        stale_source_count: pickNumber(raw, ['stale_source_count', 'staleSourceCount']),
        ambiguous_source_count: pickNumber(raw, ['ambiguous_source_count', 'ambiguousSourceCount']),
        source_identity_missing_count: pickNumber(raw, ['source_identity_missing_count', 'sourceIdentityMissingCount']),
        completed_count: pickNumber(raw, ['completed_count', 'completedCount']),
        failed_count: pickNumber(raw, ['failed_count', 'failedCount']),
        downloading_count: pickNumber(raw, ['downloading_count', 'downloadingCount']),
    };
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
    const raw = await invokeCommand<unknown>('run_integrity_audit', { downloadDir });
    return {
        total_tracks_scanned: pickNumber(raw, ['total_tracks_scanned', 'totalTracksScanned']),
        verified_files: pickNumber(raw, ['verified_files', 'verifiedFiles']),
        missing_files: pickArray<string>(raw, ['missing_files', 'missingFiles']),
        orphan_files: pickArray<string>(raw, ['orphan_files', 'orphanFiles']),
        corrupt_or_zero_byte_files: pickArray<string>(raw, ['corrupt_or_zero_byte_files', 'corruptOrZeroByteFiles']),
        abandoned_staging_files: pickArray<string>(raw, ['abandoned_staging_files', 'abandonedStagingFiles']),
        database_inconsistencies: pickArray<string>(raw, ['database_inconsistencies', 'databaseInconsistencies']),
        is_healthy: pick(raw, ['is_healthy', 'isHealthy']) === true,
        timestamp: asString(pick(raw, ['timestamp'])),
    };
}

/**
 * Repair detected library integrity issues
 */
export async function repairIntegrityIssues(stagingFilesToPurge?: string[]): Promise<IntegrityRepairResult> {
    const raw = await invokeCommand<unknown>('repair_integrity_issues', { stagingFilesToPurge });
    return {
        purged_staging_files: pickNumber(raw, ['purged_staging_files', 'purgedStagingFiles']),
        cleaned_database_entries: pickNumber(raw, ['cleaned_database_entries', 'cleanedDatabaseEntries']),
        message: asString(pick(raw, ['message'])),
    };
}

export type ReconciliationScope =
    | { type: 'all' }
    | { type: 'selected_download_ids'; value: number[] }
    | { type: 'selected_root'; value: string };

export type MissingFilePolicy = 'report_only' | 'mark_missing' | 'delete_record';
export type OrphanPolicy = 'report_only' | 'relink_if_exact_identity' | 'ignore';
export type StagingPolicy = 'report_only' | 'purge_safe_residuals';

export interface ReconciliationOptions {
    dryRun?: boolean;
    scope?: ReconciliationScope;
    missingFilePolicy?: MissingFilePolicy;
    orphanPolicy?: OrphanPolicy;
    stagingPolicy?: StagingPolicy;
    confirmDelete?: boolean;
    baseFolderOverride?: string;
}

export interface ReconciliationActionItem {
    actionType: string;
    target: string;
    details: string;
    trackId?: number;
    downloadId?: number;
    service?: string;
    executed: boolean;
}

export interface ReconciliationStats {
    totalDownloadRecords: number;
    physicalAudioFiles: number;
    missingFileRecords: number;
    orphanFilesCount: number;
    stagingResidualsCount: number;
}

export interface LibraryReconciliationReport {
    reportId: string;
    timestamp: string;
    dryRun: boolean;
    scope: ReconciliationScope;
    missingPolicy: MissingFilePolicy;
    orphanPolicy: OrphanPolicy;
    stagingPolicy: StagingPolicy;
    backupId?: string;
    backupPath?: string;
    backupSha256?: string;
    purgedMissing: number;
    relinkedOrphans: number;
    cleanedStagingResiduals: number;
    verifiedTotal: number;
    orphanFiles: string[];
    missingFiles: string[];
    ambiguousOrphans: string[];
    plannedActions: ReconciliationActionItem[];
    executedActions: ReconciliationActionItem[];
    failures: string[];
    beforeStats: ReconciliationStats;
    afterStats: ReconciliationStats;
}

/**
 * Reconciles physical audio files on disk with the runtime `downloads` SQLite table
 */
export async function reconcileLibraryPhysicalState(
    options?: ReconciliationOptions
): Promise<LibraryReconciliationReport> {
    return invokeCommand<LibraryReconciliationReport>('reconcile_library_physical_state', { options });
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
    const raw = await invokeCommand<unknown>('export_library', { outputPath });
    return {
        file_path: asString(pick(raw, ['file_path', 'filePath'])),
        tracks_count: pickNumber(raw, ['tracks_count', 'tracksCount']),
        albums_count: pickNumber(raw, ['albums_count', 'albumsCount']),
        artists_count: pickNumber(raw, ['artists_count', 'artistsCount']),
        playlists_count: pickNumber(raw, ['playlists_count', 'playlistsCount']),
        file_size_bytes: pickNumber(raw, ['file_size_bytes', 'fileSizeBytes']),
        checksum: asString(pick(raw, ['checksum'])),
    };
}

/**
 * Import and restore library from a backup JSON file
 */
export async function importLibrary(filePath: string, ignoreChecksumError?: boolean): Promise<ImportLibraryResult> {
    const raw = await invokeCommand<unknown>('import_library', { filePath, ignoreChecksumError });
    return {
        tracks_imported: pickNumber(raw, ['tracks_imported', 'tracksImported']),
        albums_imported: pickNumber(raw, ['albums_imported', 'albumsImported']),
        artists_imported: pickNumber(raw, ['artists_imported', 'artistsImported']),
        playlists_imported: pickNumber(raw, ['playlists_imported', 'playlistsImported']),
        favorites_restored: pickNumber(raw, ['favorites_restored', 'favoritesRestored']),
        message: asString(pick(raw, ['message'])),
    };
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
    const raw = await invokeCommand<unknown>('search_library', { params });
    return {
        query: asString(pick(raw, ['query'])),
        tracks: asArray<SearchResultTrack>(pick(raw, ['tracks'])).filter((t) => asRecord(t) !== null),
        albums: asArray<SearchResultAlbum>(pick(raw, ['albums'])).filter((a) => asRecord(a) !== null),
        artists: asArray<SearchResultArtist>(pick(raw, ['artists'])).filter((a) => asRecord(a) !== null),
        playlists: asArray<SearchResultPlaylist>(pick(raw, ['playlists'])).filter((p) => asRecord(p) !== null),
        total_tracks: pickNumber(raw, ['total_tracks', 'totalTracks']),
        total_albums: pickNumber(raw, ['total_albums', 'totalAlbums']),
        total_artists: pickNumber(raw, ['total_artists', 'totalArtists']),
        total_playlists: pickNumber(raw, ['total_playlists', 'totalPlaylists']),
    };
}

export async function getTopGenres(limit: number = 10): Promise<TopGenre[]> {
    const raw = await invokeCommand<unknown>('get_top_genres', { limit });
    return asArray<TopGenre>(raw);
}

export async function getAudioQualityDistribution(): Promise<QualityBucket[]> {
    const raw = await invokeCommand<unknown>('get_audio_quality_distribution');
    return asArray<QualityBucket>(raw);
}

/**
 * Fetch detailed per-provider source availability for a track
 */
export async function getTrackSourcesAvailability(trackId: number): Promise<TrackSourceAvailability[]> {
    const raw = await invokeCommand<unknown>('get_track_sources_availability', { trackId });
    return asArray<TrackSourceAvailability>(raw);
}

/**
 * Perform non-destructive availability check for a track
 */
export async function checkTrackAvailability(trackId: number, service?: string): Promise<TrackSourceAvailability[]> {
    const raw = await invokeCommand<unknown>('check_track_availability', { trackId, service });
    return asArray<TrackSourceAvailability>(raw);
}

/**
 * Perform non-destructive availability check for a batch of tracks
 */
export async function checkTracksAvailability(trackIds: number[]): Promise<Record<number, TrackSourceAvailability[]>> {
    const raw = await invokeCommand<unknown>('check_tracks_availability', { trackIds });
    const rec = asRecord(raw);
    if (!rec) return {};
    const result: Record<number, TrackSourceAvailability[]> = {};
    for (const [key, value] of Object.entries(rec)) {
        const trackId = Number(key);
        if (!Number.isFinite(trackId)) continue;
        result[trackId] = asArray<TrackSourceAvailability>(value).filter((s) => asRecord(s) !== null);
    }
    return result;
}

// ==============================================
// S176Q: ENQUEUE TRACKS & QUEUE RECONCILIATION
// ==============================================

export interface PreflightExclusion {
    track_id?: number;
    title?: string;
    artist?: string | null;
    status?: string;
    skip_reason?: string;
}

export interface QueueReconciliationSummary {
    selected: number;
    eligible: number;
    enqueued: number;
    already_downloaded: number;
    already_queued: number;
    no_download_provider: number;
    rejected_quality: number;
    requires_auth: number;
    stale_source: number;
    deduplicated: number;
    skipped: number;
}

export interface EnqueueResult {
    selected: number;
    eligible: number;
    excluded_preflight?: number;
    enqueued: number;
    skip_reasons?: string[];
    skipped?: number;
    deduplicated?: number;
    tracks?: TrackPreflightResult[];
    summary?: QueueReconciliationSummary;
}

/**
 * Raw wire contract of the `enqueue_tracks` command.
 *
 * The Rust struct serializes camelCase (`excludedPreflight` as a list of
 * exclusions, `skipReasons`), while legacy builds returned a bare count under
 * `excluded_preflight`. `enqueueTracks()` normalizes both shapes into a
 * complete `EnqueueResult`; this interface documents what the command may
 * actually emit so the union is explicit instead of `any`.
 */
export interface EnqueueTracksResponse {
    selected: number;
    eligible: number;
    enqueued: number;
    /** Bare count on legacy builds; full exclusion list on current builds. */
    excluded_preflight: number | PreflightExclusion[];
    skip_reasons?: string[];
    skipped?: number;
    deduplicated?: number;
    tracks?: TrackPreflightResult[];
    summary?: QueueReconciliationSummary;
}

export interface QueueReconciliationReport {
    selected: number;
    eligible: number;
    excluded_preflight: number;
    pending: number;
    active: number;
    completed: number;
    failed: number;
    skipped: number;
    exclusions: PreflightExclusion[];
    breakdown_by_reason: Record<string, number>;
}

/**
 * Map quality settings to canonical SQLite CHECK values ('hires', 'lossless', 'high', 'any')
 */
export function normalizeQualityPreference(quality?: string): string | undefined {
    if (!quality) return undefined;
    const s = quality.trim().toLowerCase();
    if (s === 'hi_res_lossless' || s === 'hires' || s === 'hi_res' || s === 'hi-res' || s === 'hires_lossless') {
        return 'hires';
    }
    if (s === 'lossless' || s === 'cd' || s === 'flac') {
        return 'lossless';
    }
    if (s === 'high' || s === '320' || s === '320kbps' || s === 'aac' || s === 'mp3') {
        return 'high';
    }
    if (s === 'any' || s === 'best' || s === 'auto') {
        return 'any';
    }
    return undefined;
}

/**
 * Enqueue selected tracks into download queue with zero silent exclusions (S176Q)
 */
export async function enqueueTracks(
    trackIds: number[],
    priority?: number,
    qualityPreference?: string,
    serviceName?: string,
    strictQuality?: boolean,
    allowFallback?: boolean,
    smartStudioOrigin?: boolean,
    skipAlreadyDownloaded?: boolean
): Promise<EnqueueResult> {
    const canonicalQuality = normalizeQualityPreference(qualityPreference) ?? qualityPreference;
    const raw = await invokeCommand<EnqueueTracksResponse | null>('enqueue_tracks', {
        trackIds,
        priority,
        qualityPreference: canonicalQuality,
        serviceName,
        strictQuality,
        allowFallback,
        smartStudioOrigin,
        skipAlreadyDownloaded,
    });

    // Backend serializes camelCase (`excludedPreflight`, `skipReasons`);
    // older builds used snake_case or a bare numeric count. Accept all shapes.
    const rawExcluded = pick(raw, ['excludedPreflight', 'excluded_preflight']);
    let excludedCount = 0;
    let skipReasons: string[] = [];

    if (typeof rawExcluded === 'number') {
        excludedCount = rawExcluded;
    } else if (Array.isArray(rawExcluded)) {
        excludedCount = rawExcluded.length;
        skipReasons = rawExcluded
            .map((e) => (typeof e === 'string' ? e : asString(pick(e, ['skip_reason', 'skipReason']))))
            .filter((s: string) => s.length > 0);
    }

    const rawSkipReasons = pickArray<string>(raw, ['skipReasons', 'skip_reasons']);
    if (rawSkipReasons.length > 0) {
        skipReasons = rawSkipReasons.filter((s) => typeof s === 'string');
    }

    const selected = typeof raw?.selected === 'number' ? raw.selected : (trackIds?.length ?? 0);
    const eligible = typeof raw?.eligible === 'number' ? raw.eligible : Math.max(0, selected - excludedCount);
    const enqueued = typeof raw?.enqueued === 'number' ? raw.enqueued : 0;
    const summarySkipped = pickNumber(raw?.summary, ['skipped']);
    const summaryDeduplicated = pickNumber(raw?.summary, ['deduplicated']);
    const skipped = typeof raw?.skipped === 'number' ? raw.skipped : summarySkipped;
    const deduplicated = typeof raw?.deduplicated === 'number' ? raw.deduplicated : summaryDeduplicated;
    const tracks = asArray<TrackPreflightResult>(pick(raw, ['tracks']));
    const rawSummary = asRecord(pick(raw, ['summary']));
    const summary = rawSummary
        ? ({ ...rawSummary } as unknown as QueueReconciliationSummary)
        : undefined;

    return {
        selected,
        eligible,
        excluded_preflight: excludedCount,
        enqueued,
        skip_reasons: skipReasons,
        skipped,
        deduplicated,
        tracks,
        summary,
    };
}

/**
 * Reconcile queue state with preflight and runtime execution stats (S176Q)
 */
export async function reconcileQueue(
    selectedTrackIds?: number[]
): Promise<QueueReconciliationReport> {
    const raw = await invokeCommand<unknown>('reconcile_queue', {
        selectedTrackIds,
    });

    // Rust struct is camelCase (`excludedPreflight`, `breakdownByReason`).
    const breakdownRaw = asRecord(pick(raw, ['breakdown_by_reason', 'breakdownByReason']));
    const breakdown_by_reason: Record<string, number> = {};
    if (breakdownRaw) {
        for (const [reason, count] of Object.entries(breakdownRaw)) {
            if (typeof count === 'number' && Number.isFinite(count)) {
                breakdown_by_reason[reason] = count;
            }
        }
    }

    return {
        selected: pickNumber(raw, ['selected']),
        eligible: pickNumber(raw, ['eligible']),
        excluded_preflight: pickNumber(raw, ['excludedPreflight', 'excluded_preflight']),
        pending: pickNumber(raw, ['pending']),
        active: pickNumber(raw, ['active']),
        completed: pickNumber(raw, ['completed']),
        failed: pickNumber(raw, ['failed']),
        skipped: pickNumber(raw, ['skipped']),
        exclusions: pickArray<PreflightExclusion>(raw, ['exclusions']).map((e) => ({
            track_id: optionalNumber(pick(e, ['track_id', 'trackId'])),
            title: asString(pick(e, ['title'])),
            artist: (pick(e, ['artist']) as string | null | undefined) ?? null,
            status: asString(pick(e, ['status'])),
            skip_reason: asString(pick(e, ['skip_reason', 'skipReason'])),
        })),
        breakdown_by_reason,
    };
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
    reconcileLibraryPhysicalState,
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
    enqueueTracks,
    reconcileQueue,
    normalizeQualityPreference,
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
    getTopGenres,
    getAudioQualityDistribution,
    getTrackSourcesAvailability,
    checkTrackAvailability,
    checkTracksAvailability,
};





