/**
 * Library API
 * 
 * Tauri commands for library management and local scanning.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asString, asBoolean, asRecord, pick, pickArray, pickNumber, optionalNumber } from './normalize';
import type { LibraryTrack, LibraryPage, LibraryStats, Playlist, SearchResult, AlbumDetail, ArtistDetail, TopArtist, TopGenre, QualityBucket, TrackSourceAvailability, TrackPreflightResult } from './types';
import type {
    LibraryQuery,
    LibraryQueryCounts,
    LibraryQueryResult,
    LibraryAlbumQuery,
    LibraryAlbumRow,
    LibraryAlbumsResult,
    LibrarySelection,
    LibraryBatchAction,
    LibraryActionPreview,
    LibraryActionSummary,
} from './types';

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
/**
 * Consulta de biblioteca v2 (L01, issue #98): una fila por pista, filtros
 * AND/OR, contadores y orden EN SERVIDOR, paginación offset/limit. Los
 * contadores solo vienen en la primera página (`offset == 0`); al paginar
 * la vista conserva los de la primera.
 */
export async function queryLibrary(query: LibraryQuery): Promise<LibraryQueryResult> {
    const raw = asRecord(await invokeCommand<unknown>('query_library', { query }));
    const counts = asRecord(raw?.counts);
    const normalizedCounts: LibraryQueryCounts | null = counts
        ? {
              total: asNumber(counts.total),
              favorites: asNumber(counts.favorites),
              downloaded: asNumber(counts.downloaded),
              inPlaylists: asNumber(counts.inPlaylists, asNumber(counts.in_playlists)),
              albums: asNumber(counts.albums),
              byService: asArray(counts.byService ?? counts.by_service).map((s) => {
                  const rec = asRecord(s) ?? {};
                  return {
                      serviceId: asNumber(rec.serviceId ?? rec.service_id),
                      serviceName: asString(rec.serviceName ?? rec.service_name, ''),
                      count: asNumber(rec.count),
                  };
              }),
              byAccount: asArray(counts.byAccount ?? counts.by_account).map((a) => {
                  const rec = asRecord(a) ?? {};
                  return {
                      accountId: asNumber(rec.accountId ?? rec.account_id),
                      displayName: (rec.displayName ?? rec.display_name) === null || (rec.displayName ?? rec.display_name) === undefined ? null : asString(rec.displayName ?? rec.display_name, ''),
                      serviceName: asString(rec.serviceName ?? rec.service_name, ''),
                      count: asNumber(rec.count),
                  };
              }),
          }
        : null;
    return {
        ...normalizeLibraryPage(raw, query.offset, query.limit ?? undefined),
        counts: normalizedCounts,
    };
}

/**
 * Consulta de la pestaña de álbumes (L07, #104): una fila por álbum con
 * pistas vivas, cobertura, cuentas y servicios. Comparte texto, filtros y
 * paginación con `queryLibrary`, pero la agregación y el orden son de álbum.
 */
export async function queryLibraryAlbums(query: LibraryAlbumQuery): Promise<LibraryAlbumsResult> {
    const raw = asRecord(await invokeCommand<unknown>('query_library_albums', { query }));
    const nullableText = (value: unknown): string | null =>
        typeof value === 'string' ? value : null;
    const normalizeAlbum = (album: unknown): LibraryAlbumRow => {
        const rec = asRecord(album) ?? {};
        return {
            albumId: asNumber(rec.albumId ?? rec.album_id),
            title: asString(rec.title, ''),
            artistName: nullableText(rec.artistName ?? rec.artist_name),
            releaseYear: optionalNumber(rec.releaseYear ?? rec.release_year) ?? null,
            trackCount: asNumber(rec.trackCount ?? rec.track_count),
            downloadedCount: asNumber(rec.downloadedCount ?? rec.downloaded_count),
            coverage: optionalNumber(rec.coverage) ?? null,
            accounts: asArray(rec.accounts).map((account) => {
                const a = asRecord(account) ?? {};
                return {
                    accountId: asNumber(a.accountId ?? a.account_id),
                    displayName: nullableText(a.displayName ?? a.display_name),
                    serviceName: asString(a.serviceName ?? a.service_name, ''),
                };
            }),
            services: asArray(rec.services).map((service) => {
                const s = asRecord(service) ?? {};
                return {
                    serviceId: asNumber(s.serviceId ?? s.service_id),
                    serviceName: asString(s.serviceName ?? s.service_name, ''),
                };
            }),
            dateAdded: nullableText(rec.dateAdded ?? rec.date_added),
            coverArtUrl: nullableText(rec.coverArtUrl ?? rec.cover_art_url),
        };
    };
    return {
        albums: asArray(raw?.albums).map((album) => normalizeAlbum(album)),
        total: asNumber(raw?.total),
        offset: asNumber(raw?.offset, query.offset ?? 0),
        limit: asNumber(raw?.limit, query.limit ?? 0),
        hasMore: raw?.hasMore === true || raw?.has_more === true,
    };
}

/**
 * Vista previa de una acción por lotes (L05, #102): recuento elegible y
 * motivos de omisión agregados por acción. No escribe nada.
 */
export async function previewLibraryAction(
    selection: LibrarySelection,
    action: LibraryBatchAction
): Promise<LibraryActionPreview> {
    return invokeCommand<LibraryActionPreview>('preview_library_action', { selection, action });
}

/**
 * Ejecuta una acción por lotes. El servidor vuelve a resolver la selección y
 * recalcula la elegibilidad; `expectedTotal`/`expectedEligible` solo sirven
 * para que el resumen declare `recalculated` si el sync cambió la biblioteca.
 */
export async function runLibraryAction(
    selection: LibrarySelection,
    action: LibraryBatchAction,
    expectedTotal?: number | null,
    expectedEligible?: number | null
): Promise<LibraryActionSummary> {
    return invokeCommand<LibraryActionSummary>('run_library_action', {
        selection,
        action,
        expectedTotal: expectedTotal ?? null,
        expectedEligible: expectedEligible ?? null,
    });
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
 * Catálogo local MusicBrainz (mb_dump_*): tipos y wrappers de los comandos
 * `import_mb_dump` / `get_mb_dump_state` (issues #264 punto 1). Los campos
 * llegan con las claves snake_case de los structs Rust.
 */
export interface MbDumpImportCounts {
    recording: number;
    artist: number;
    artist_credit_name: number;
    isrc: number;
}

export interface MbDumpImportState {
    dump_path: string;
    dump_timestamp: string;
    schema_version: number;
    recording_rows: number;
    isrc_rows: number;
    artist_rows: number;
    artist_credit_name_rows: number;
    status: 'importing' | 'completed' | 'failed';
    error: string | null;
    imported_at: string | null;
}

export interface MbDumpProgress {
    table: string;
    rows: number;
    expected: number;
}

/**
 * Importa un mbdump legacy de MusicBrainz al catálogo local. Larga: resuelve
 * al terminar; el progreso llega por el evento MB_DUMP_PROGRESS.
 */
export async function importMbDump(dumpPath: string): Promise<MbDumpImportCounts> {
    return invokeCommand<MbDumpImportCounts>('import_mb_dump', { dumpPath });
}

/**
 * Último intento de importación del catálogo; null si nunca se importó.
 */
export async function getMbDumpImportState(): Promise<MbDumpImportState | null> {
    return invokeCommand<MbDumpImportState | null>('get_mb_dump_state');
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
export async function queueDownloads(trackIds: number[]): Promise<number> {
    return invokeCommand<number>('queue_downloads', { trackIds });
}

// ==============================================
// LOCAL LIBRARY SCAN COMMANDS
// ==============================================

export interface LocalSyncOptions {
    recursive?: boolean;
    autoResolveDuplicates?: boolean;
}

export interface LocalSyncReport {
    success: boolean;
    directory: string;
    total_scanned: number;
    imported: number;
    duplicates_matched: number;
    duplicates_skipped: number;
    errors: string[];
}

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
 *
 * FE-6: `watchForChanges` launches a backend auto-rescan watcher for the
 * directory; `skipSmallFiles` makes the scanner skip files under 1 MB.
 * Both mirror the checkboxes of the "Add Local Library Path" dialog.
 */
export async function scanLocalLibraryWithProgress(
    directory: string,
    options?: {
        recursive?: boolean;
        watchForChanges?: boolean;
        skipSmallFiles?: boolean;
    }
): Promise<ScanResult> {
    return invokeCommand<ScanResult>('scan_local_library_with_progress', {
        directory,
        recursive: options?.recursive ?? true,
        watchForChanges: options?.watchForChanges ?? false,
        skipSmallFiles: options?.skipSmallFiles ?? false,
    });
}

/**
 * Synchronize a local directory into the library as a first-class service.
 * F-01 & F-02: Performs filesystem scanning, duplicate detection, metadata scoring (SC-03),
 * physical audio relinking, and duplicate resolution.
 */
export async function syncLocalLibrary(
    directory: string,
    options?: LocalSyncOptions
): Promise<LocalSyncReport> {
    try {
        const report = await invokeCommand<LocalSyncReport>('sync_local_library_cmd', {
            directory,
            recursive: options?.recursive ?? true,
        });

        if (report?.success && options?.autoResolveDuplicates) {
            // This option modifies the WHOLE library, not just the imported
            // folder. It is kept as an explicit option, off by
            // default; it must not run silently on every import.
            try {
                await invokeCommand<unknown>('auto_resolve_duplicates');
            } catch (error) {
                report.errors.push(`Automatic duplicate resolution failed: ${String(error)}`);
            }
        }

        return report;
    } catch (e: any) {
        return {
            success: false,
            directory,
            total_scanned: 0,
            imported: 0,
            duplicates_matched: 0,
            duplicates_skipped: 0,
            errors: [String(e?.message || e)],
        };
    }
}

/**
 * Remove a single track from the library
 */
export async function removeTrack(trackId: number): Promise<void> {
    return invokeCommand<void>('remove_track', { trackId });
}

/**
 * FE-6: stop the auto-rescan watcher of a library path (if any).
 * @returns true when a watcher existed and was stopped.
 */
export async function stopLibraryWatcher(directory: string): Promise<boolean> {
    return invokeCommand<boolean>('stop_library_watcher_command', { directory });
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
export async function syncFavorites(service: string, favType?: string, accountId?: number | null): Promise<FavoritesSyncResult> {
    const raw = await invokeCommand<unknown>('sync_favorites', { service, favType, accountId: accountId ?? null });
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

// ==============================================
// LIBRARY MAINTENANCE (IN-5: exposed backend capabilities)
// ==============================================

/** Track metadata incl. import sources (matches Rust TrackMetadata of get_track_metadata). */
export interface TrackMetadataWithSources {
    track_id: number;
    title: string;
    artist_name: string | null;
    artist_id: number | null;
    album_name: string | null;
    album_id: number | null;
    duration_ms: number | null;
    track_number: number | null;
    disc_number: number | null;
    isrc: string | null;
    explicit: boolean | null;
    genre: string | null;
    bpm: number | null;
    musical_key: string | null;
    release_year: number | null;
    musicbrainz_id: string | null;
    cover_art_url: string | null;
    file_path: string | null;
    sources?: TrackSourceAvailability[] | null;
    // PL-04 full metadata tab: album/download/file details plus the sidecar
    // paths resolved by the backend against the real filesystem.
    album_artist?: string | null;
    label?: string | null;
    upc?: string | null;
    release_date?: string | null;
    total_tracks?: number | null;
    total_discs?: number | null;
    album_musicbrainz_id?: string | null;
    musicbrainz_release_id?: string | null;
    file_format?: string | null;
    file_size_bytes?: number | null;
    bit_depth?: number | null;
    sample_rate?: number | null;
    downloaded_at?: string | null;
    imported_from?: string | null;
    downloaded_from?: string | null;
    tempo_source?: string | null;
    tempo_confidence?: number | null;
    file_disambiguator?: string | null;
    animated_cover_path?: string | null;
    booklet_path?: string | null;
    hd_cover_path?: string | null;
}

function normalizeTrackMetadata(raw: unknown): TrackMetadataWithSources {
    // `TrackMetadata` en Rust lleva `#[serde(rename_all = "camelCase")]`
    // (trackId, artistName, filePath…). `pick` acepta ambas grafías para no
    // volver a romper ids/nombres si el backend cambia de convención: un id
    // perdido aquí se propaga a la persistencia, MPRIS y los listens.
    const optStr = (keys: string[]): string | null => {
        const value = pick<unknown>(raw, keys);
        return typeof value === 'string' ? value : null;
    };
    const optNum = (keys: string[]): number | null => {
        const value = pick<unknown>(raw, keys);
        return typeof value === 'number' && Number.isFinite(value) ? value : null;
    };
    const optBool = (keys: string[]): boolean | null => {
        const value = pick<unknown>(raw, keys);
        return typeof value === 'boolean' ? value : null;
    };
    const sources = pick<unknown>(raw, ['sources']);
    return {
        track_id: pickNumber(raw, ['track_id', 'trackId']),
        title: asString(pick<unknown>(raw, ['title'])),
        artist_name: optStr(['artist_name', 'artistName']),
        artist_id: optNum(['artist_id', 'artistId']),
        album_name: optStr(['album_name', 'albumName']),
        album_id: optNum(['album_id', 'albumId']),
        duration_ms: optNum(['duration_ms', 'durationMs']),
        track_number: optNum(['track_number', 'trackNumber']),
        disc_number: optNum(['disc_number', 'discNumber']),
        isrc: optStr(['isrc']),
        explicit: optBool(['explicit']),
        genre: optStr(['genre']),
        bpm: optNum(['bpm']),
        musical_key: optStr(['musical_key', 'musicalKey']),
        release_year: optNum(['release_year', 'releaseYear']),
        musicbrainz_id: optStr(['musicbrainz_id', 'musicbrainzId']),
        cover_art_url: optStr(['cover_art_url', 'coverArtUrl']),
        file_path: optStr(['file_path', 'filePath']),
        sources: Array.isArray(sources) ? (sources as TrackSourceAvailability[]) : null,
        album_artist: optStr(['album_artist', 'albumArtist']),
        label: optStr(['label']),
        upc: optStr(['upc']),
        release_date: optStr(['release_date', 'releaseDate']),
        total_tracks: optNum(['total_tracks', 'totalTracks']),
        total_discs: optNum(['total_discs', 'totalDiscs']),
        album_musicbrainz_id: optStr(['album_musicbrainz_id', 'albumMusicbrainzId']),
        musicbrainz_release_id: optStr(['musicbrainz_release_id', 'musicbrainzReleaseId']),
        file_format: optStr(['file_format', 'fileFormat']),
        file_size_bytes: optNum(['file_size_bytes', 'fileSizeBytes']),
        bit_depth: optNum(['bit_depth', 'bitDepth']),
        sample_rate: optNum(['sample_rate', 'sampleRate']),
        downloaded_at: optStr(['downloaded_at', 'downloadedAt']),
        imported_from: optStr(['imported_from', 'importedFrom']),
        downloaded_from: optStr(['downloaded_from', 'downloadedFrom']),
        tempo_source: optStr(['tempo_source', 'tempoSource']),
        tempo_confidence: optNum(['tempo_confidence', 'tempoConfidence']),
        file_disambiguator: optStr(['file_disambiguator', 'fileDisambiguator']),
        animated_cover_path: optStr(['animated_cover_path', 'animatedCoverPath']),
        booklet_path: optStr(['booklet_path', 'bookletPath']),
        hd_cover_path: optStr(['hd_cover_path', 'hdCoverPath']),
    };
}

/**
 * Fetch the stored metadata of a single track including its import sources.
 */
export async function getTrackMetadata(trackId: number): Promise<TrackMetadataWithSources> {
    const raw = await invokeCommand<unknown>('get_track_metadata', { trackId });
    return normalizeTrackMetadata(raw);
}

/**
 * The listen crediting (`record_local_play`) is no longer an IPC command
 * (P01, #132): the backend accumulates the 30 real seconds in its ticker and
 * credits once per session on its own — the UI never times playback.
 */

/**
 * Repair tracks missing their artist links (creates Unknown Artist entries
 * for orphan tracks). Returns the backend JSON report.
 */
export async function repairArtistLinks(): Promise<Record<string, unknown>> {
    const raw = await invokeCommand<unknown>('repair_artist_links');
    return asRecord(raw) ?? {};
}

/** Ghost-artists resolution report (matches Rust GhostArtistReport). */
export interface GhostArtistReport {
    duplicates_merged: number;
    musicbrainz_resolved: number;
    external_ids_linked: number;
    total_processed: number;
}

function countRecord(raw: unknown, keys: string[]): Record<string, number> {
    const rec = asRecord(raw) ?? {};
    const out: Record<string, number> = {};
    for (const k of keys) {
        const v = rec[k];
        out[k] = typeof v === 'number' && Number.isFinite(v) ? v : 0;
    }
    return out;
}

/**
 * Resolve ghost favorite artists (merge duplicates, link MusicBrainz/external ids).
 */
export async function resolveGhostArtists(): Promise<GhostArtistReport> {
    const raw = await invokeCommand<unknown>('resolve_ghost_artists');
    return countRecord(raw, ['duplicates_merged', 'musicbrainz_resolved', 'external_ids_linked', 'total_processed']) as unknown as GhostArtistReport;
}

/** Stub-album hydration report (matches Rust StubAlbumHydrationReport). */
export interface StubAlbumHydrationReport {
    duplicate_stubs_merged: number;
    albums_hydrated: number;
    tracks_inserted: number;
    total_processed: number;
}

/**
 * Hydrate stub favorite albums with tracklists from MusicBrainz or the library.
 */
export async function hydrateStubAlbums(): Promise<StubAlbumHydrationReport> {
    const raw = await invokeCommand<unknown>('hydrate_stub_albums');
    return countRecord(raw, ['duplicate_stubs_merged', 'albums_hydrated', 'tracks_inserted', 'total_processed']) as unknown as StubAlbumHydrationReport;
}

/** Album total_tracks recalculation report (TASK-138). */
export interface AlbumTotalTracksReconcileReport {
    updated_albums: number;
    divergent_before: number;
    divergent_after: number;
}

/**
 * Recalculate and reconcile albums.total_tracks against actual track counts.
 */
export async function recalculateAlbumTotalTracks(albumIds?: number[]): Promise<AlbumTotalTracksReconcileReport> {
    const raw = await invokeCommand<unknown>('recalculate_album_total_tracks', {
        albumIds: albumIds ?? null
    });
    const rec = asRecord(raw);
    const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);
    return {
        updated_albums: num(rec?.updated_albums),
        divergent_before: num(rec?.divergent_before),
        divergent_after: num(rec?.divergent_after),
    };
}

/** Orphan empty albums purge report (TASK-70). */
export interface PurgeOrphanEmptyAlbumsReport {
    purged_albums_count: number;
    purged_album_artists_count: number;
    preserved_stubs_count: number;
}

/**
 * Purge empty orphan albums (0 tracks) and clean up orphan album_artists rows.
 */
export async function purgeOrphanEmptyAlbums(): Promise<PurgeOrphanEmptyAlbumsReport> {
    const raw = await invokeCommand<unknown>('purge_orphan_empty_albums');
    const rec = asRecord(raw);
    const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);
    return {
        purged_albums_count: num(rec?.purged_albums_count),
        purged_album_artists_count: num(rec?.purged_album_artists_count),
        preserved_stubs_count: num(rec?.preserved_stubs_count),
    };
}

/** Storage reconciliation result (matches Rust StorageReconciliationResult). */
export interface StorageReconciliationResult {
    scanned_audio_files: number;
    relinked_downloads: number;
    purged_staging_files: number;
    ambiguous_files: string[];
    message: string;
}

/**
 * Reconcile orphan audio files on disk into `downloads` and purge .staging/*.part
 * residuals.
 */
export async function reconcileDownloadsFromStorage(musicDirOverride?: string): Promise<StorageReconciliationResult> {
    const raw = await invokeCommand<unknown>('reconcile_downloads_from_storage', {
        musicDirOverride: musicDirOverride ?? null
    });
    const rec = asRecord(raw);
    const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);
    return {
        scanned_audio_files: num(rec?.scanned_audio_files),
        relinked_downloads: num(rec?.relinked_downloads),
        purged_staging_files: num(rec?.purged_staging_files),
        ambiguous_files: asArray<string>(rec?.ambiguous_files).filter((f): f is string => typeof f === 'string'),
        message: asString(rec?.message),
    };
}

// Export as namespace
export const libraryApi = {
    getLibrary,
    queryLibrary,
    queryLibraryAlbums,
    previewLibraryAction,
    runLibraryAction,
    getFavoriteTracks,
    getFavoritesTracks,
    getFavoritesAlbums,
    getFavoritesArtists,
    syncFavorites,
    pushFavoriteToService,
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
    syncLocalLibrary,
    getLocalTrackMetadata,
    removeTrack,
    stopLibraryWatcher,
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
    getTrackMetadata,
    repairArtistLinks,
    resolveGhostArtists,
    hydrateStubAlbums,
    recalculateAlbumTotalTracks,
    purgeOrphanEmptyAlbums,
    reconcileDownloadsFromStorage,
    getTrackDetail,
    deleteTrackAudio,
};

// ==============================================
// TRACK DETAIL (L04, issue #101)
// ==============================================
// Panel lateral de detalle: UNA llamada IPC con el Track canónico, sus
// TrackSource por servicio, sus LibraryItem por cuenta y colección y la
// descarga. Las facetas de cuarentena y duplicados son contrato pendiente
// (#72/#73): el backend las devuelve null y la vista muestra la sección
// deshabilitada con su motivo — no se simulan.

export interface TrackDetailSource {
    serviceId: number;
    serviceName: string;
    serviceTrackId: string | null;
    format: string | null;
    bitDepth: number | null;
    sampleRate: number | null;
    bitrate: number | null;
    qualityScore: number | null;
    available: boolean;
    availabilityStatus: string;
    availabilityReason: string | null;
    lastChecked: string | null;
}

export interface TrackDetailMembership {
    accountId: number;
    displayName: string | null;
    serviceName: string;
    collection: 'favorite' | 'playlist';
    collectionId: number | null;
    collectionName: string | null;
    addedAt: string | null;
}

export interface TrackDetailDownload {
    status: string;
    effectiveService: string | null;
    filePath: string | null;
    fileFormat: string | null;
    bitDepth: number | null;
    sampleRate: number | null;
    downloadedAt: string | null;
}

export interface TrackDetail {
    track: LibraryTrack;
    sources: TrackDetailSource[];
    memberships: TrackDetailMembership[];
    download: TrackDetailDownload | null;
    /** `null` hasta #72/L09 (no hay tabla de cuarentena). */
    quarantine: null;
    /** `null` hasta #73/L10 (identidad ISRC∪huella). */
    duplicateGroup: null;
}

/** Error tipado del panel: una pista inexistente no es una fila vacía. */
export type TrackDetailError =
    | { kind: 'track_not_found'; trackId: number }
    | { kind: 'database'; message: string };

export interface DeleteTrackAudioResult {
    queueEntriesDeleted: number;
    downloadDeleted: boolean;
    audioFileDeleted: boolean;
}

/**
 * El comando devuelve el error tipado como objeto con `kind`; invokeCommand
 * envuelve lo que no es string/Error en TauriError con el original en
 * `details`. Normaliza ambos caminos al tipo discriminado.
 */
export function normalizeTrackDetailError(error: unknown): TrackDetailError {
    const raw: unknown = error !== null && typeof error === 'object' && 'kind' in error
        ? error
        : (error as { details?: unknown } | null | undefined)?.details;
    if (raw !== null && typeof raw === 'object' && (raw as Record<string, unknown>).kind === 'track_not_found') {
        return { kind: 'track_not_found', trackId: asNumber((raw as Record<string, unknown>).trackId) };
    }
    const message = error instanceof Error ? error.message : asString(error as string | undefined, 'Unknown error');
    return { kind: 'database', message };
}

function normalizeTrackDetailSource(item: unknown): TrackDetailSource {
    const rec = asRecord(item) ?? {};
    return {
        serviceId: asNumber(rec.serviceId ?? rec.service_id),
        serviceName: asString(rec.serviceName ?? rec.service_name, ''),
        serviceTrackId: (rec.serviceTrackId ?? rec.service_track_id) === undefined || (rec.serviceTrackId ?? rec.service_track_id) === null ? null : asString(rec.serviceTrackId ?? rec.service_track_id),
        format: rec.format === undefined || rec.format === null ? null : asString(rec.format),
        bitDepth: optionalNumber(rec.bitDepth ?? rec.bit_depth) ?? null,
        sampleRate: optionalNumber(rec.sampleRate ?? rec.sample_rate) ?? null,
        bitrate: optionalNumber(rec.bitrate) ?? null,
        qualityScore: optionalNumber(rec.qualityScore ?? rec.quality_score) ?? null,
        available: rec.available === true,
        availabilityStatus: asString(rec.availabilityStatus ?? rec.availability_status, 'unknown_unchecked'),
        availabilityReason: (rec.availabilityReason ?? rec.availability_reason) === undefined || (rec.availabilityReason ?? rec.availability_reason) === null ? null : asString(rec.availabilityReason ?? rec.availability_reason),
        lastChecked: (rec.lastChecked ?? rec.last_checked) === undefined || (rec.lastChecked ?? rec.last_checked) === null ? null : asString(rec.lastChecked ?? rec.last_checked),
    };
}

function normalizeTrackDetailMembership(item: unknown): TrackDetailMembership {
    const rec = asRecord(item) ?? {};
    const collection = asString(rec.collection, 'favorite');
    return {
        accountId: asNumber(rec.accountId ?? rec.account_id),
        displayName: (rec.displayName ?? rec.display_name) === undefined || (rec.displayName ?? rec.display_name) === null ? null : asString(rec.displayName ?? rec.display_name),
        serviceName: asString(rec.serviceName ?? rec.service_name, ''),
        collection: collection === 'playlist' ? 'playlist' : 'favorite',
        collectionId: optionalNumber(rec.collectionId ?? rec.collection_id) ?? null,
        collectionName: (rec.collectionName ?? rec.collection_name) === undefined || (rec.collectionName ?? rec.collection_name) === null ? null : asString(rec.collectionName ?? rec.collection_name),
        addedAt: (rec.addedAt ?? rec.added_at) === undefined || (rec.addedAt ?? rec.added_at) === null ? null : asString(rec.addedAt ?? rec.added_at),
    };
}

function normalizeTrackDetailDownload(rec: unknown): TrackDetailDownload | null {
    const d = asRecord(rec);
    if (!d) return null;
    return {
        status: asString(d.status, 'not_downloaded'),
        effectiveService: (d.effectiveService ?? d.effective_service) === undefined || (d.effectiveService ?? d.effective_service) === null ? null : asString(d.effectiveService ?? d.effective_service),
        filePath: (d.filePath ?? d.file_path) === undefined || (d.filePath ?? d.file_path) === null ? null : asString(d.filePath ?? d.file_path),
        fileFormat: (d.fileFormat ?? d.file_format) === undefined || (d.fileFormat ?? d.file_format) === null ? null : asString(d.fileFormat ?? d.file_format),
        bitDepth: optionalNumber(d.bitDepth ?? d.bit_depth) ?? null,
        sampleRate: optionalNumber(d.sampleRate ?? d.sample_rate) ?? null,
        downloadedAt: (d.downloadedAt ?? d.downloaded_at) === undefined || (d.downloadedAt ?? d.downloaded_at) === null ? null : asString(d.downloadedAt ?? d.downloaded_at),
    };
}

/**
 * L04 (issue #101): detalle de pista en UNA llamada para el panel lateral.
 * El rechazo es un `TrackDetailError` tipado (`kind: 'track_not_found'`
 * para una pista inexistente).
 */
export async function getTrackDetail(trackId: number): Promise<TrackDetail> {
    try {
        const raw = asRecord(await invokeCommand<unknown>('get_library_track_detail', { trackId }));
        const page = normalizeLibraryPage({ tracks: raw?.track ? [raw.track] : [], total: raw?.track ? 1 : 0 }, 0, 1);
        if (!page.tracks[0]) {
            // El backend nunca responde con la faceta vacía para un id que
            // existe (es error tipado allí); por blindaje local, una
            // respuesta sin Track se rechaza igual.
            throw { kind: 'track_not_found', trackId } satisfies TrackDetailError;
        }
        return {
            track: page.tracks[0],
            sources: asArray(raw?.sources).map(normalizeTrackDetailSource),
            memberships: asArray(raw?.memberships).map(normalizeTrackDetailMembership),
            download: normalizeTrackDetailDownload(raw?.download),
            quarantine: null,
            duplicateGroup: null,
        };
    } catch (error) {
        throw normalizeTrackDetailError(error);
    }
}

/**
 * L04 (issue #101): borrar el audio de una pista con confirmación previa en
 * la vista. Elimina solo el Download, sus entradas de cola y el fichero;
 * nunca el Track, sus TrackSource ni su pertenencia.
 */
export async function deleteTrackAudio(trackId: number): Promise<DeleteTrackAudioResult> {
    const raw = asRecord(await invokeCommand<unknown>('delete_track_audio', { trackId }));
    return {
        queueEntriesDeleted: asNumber(raw?.queueEntriesDeleted ?? raw?.queue_entries_deleted),
        downloadDeleted: raw?.downloadDeleted === true || raw?.download_deleted === true,
        audioFileDeleted: raw?.audioFileDeleted === true || raw?.audio_file_deleted === true,
    };
}





