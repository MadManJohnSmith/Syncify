/**
 * Dashboard API
 * 
 * Tauri commands for dashboard statistics and historical snapshots.
 */

import { invokeCommand } from './tauri';
import { asArray, asString, asRecord, pickNumber } from './normalize';
import type { LibrarySnapshot, LibraryStats, MetadataStats, ServiceHealthInfo, LyricsStats } from './types';

/**
 * Get service health status for all connected services
 */
export async function getServiceHealth(): Promise<ServiceHealthInfo[]> {
    const raw = await invokeCommand<unknown>('get_service_health');
    return asArray<ServiceHealthInfo>(raw);
}

/**
 * FE-PERF-04 (#51): payload unificado del Dashboard. Una llamada
 * (`get_dashboard_snapshot`) devuelve los seis recuentos del snapshot y la
 * vista `library_stats` resueltos en UNA lectura, más el historial de
 * snapshots y la cobertura de metadatos y letras que el Dashboard pedía por
 * separado (`create_library_snapshot`, `get_library_stats`,
 * `get_library_snapshots`, `get_metadata_stats`, `get_lyrics_stats`).
 */
export interface DashboardSnapshotPayload {
    /** Vista `library_stats` — misma semántica que `get_library_stats`. */
    stats: LibraryStats;
    /** Snapshot del día, ya escrito. */
    snapshot: LibrarySnapshot;
    /** Historial para la gráfica de crecimiento. */
    snapshots: LibrarySnapshot[];
    /** Cobertura de metadatos — misma consulta que `get_metadata_stats`. */
    metadata: MetadataStats;
    /** Cobertura de letras — misma consulta que `get_lyrics_stats`. */
    lyrics: LyricsStats;
}

function normalizeSnapshot(raw: unknown): LibrarySnapshot {
    const rec = asRecord(raw);
    return {
        id: pickNumber(rec, ['id']),
        snapshot_date: asString(rec?.snapshot_date),
        total_tracks: pickNumber(rec, ['total_tracks']),
        total_albums: pickNumber(rec, ['total_albums']),
        total_artists: pickNumber(rec, ['total_artists']),
        total_size_bytes: pickNumber(rec, ['total_size_bytes']),
        tracks_with_lyrics: pickNumber(rec, ['tracks_with_lyrics']),
        tracks_lossless: pickNumber(rec, ['tracks_lossless']),
        tracks_hires: pickNumber(rec, ['tracks_hires']),
        downloaded_tracks: pickNumber(rec, ['downloaded_tracks']),
    };
}

/**
 * Lectura unificada de los agregados del Dashboard.
 * @param days - Días de historial de snapshots para la gráfica de crecimiento
 */
export async function getDashboardSnapshot(days: number = 30): Promise<DashboardSnapshotPayload> {
    const raw = await invokeCommand<unknown>('get_dashboard_snapshot', { days });
    const rec = asRecord(raw);
    return {
        stats: {
            total_tracks: pickNumber(rec?.stats, ['total_tracks', 'totalTracks']),
            total_artists: pickNumber(rec?.stats, ['total_artists', 'totalArtists']),
            total_albums: pickNumber(rec?.stats, ['total_albums', 'totalAlbums']),
            total_downloads: pickNumber(rec?.stats, ['total_downloads', 'totalDownloads']),
            queued_downloads: pickNumber(rec?.stats, ['queued_downloads', 'queuedDownloads']),
            active_downloads: pickNumber(rec?.stats, ['active_downloads', 'activeDownloads']),
            library_entries: pickNumber(rec?.stats, ['library_entries', 'libraryEntries']),
            playlists: pickNumber(rec?.stats, ['playlists']),
            services_with_data: pickNumber(rec?.stats, ['services_with_data', 'servicesWithData']),
        },
        snapshot: normalizeSnapshot(rec?.snapshot),
        snapshots: asArray<LibrarySnapshot>(rec?.snapshots),
        metadata: {
            total_tracks: pickNumber(rec?.metadata, ['total_tracks', 'totalTracks']),
            with_isrc: pickNumber(rec?.metadata, ['with_isrc', 'withIsrc']),
            with_musicbrainz_id: pickNumber(rec?.metadata, ['with_musicbrainz_id', 'withMusicbrainzId', 'withMusicBrainzId']),
            with_album: pickNumber(rec?.metadata, ['with_album', 'withAlbum']),
            with_year: pickNumber(rec?.metadata, ['with_year', 'withYear']),
            with_genre: pickNumber(rec?.metadata, ['with_genre', 'withGenre']),
            with_art: pickNumber(rec?.metadata, ['with_art', 'withArt']),
            average_completeness: pickNumber(rec?.metadata, ['average_completeness', 'averageCompleteness']),
        },
        lyrics: {
            total_tracks: pickNumber(rec?.lyrics, ['total_tracks', 'totalTracks']),
            with_lyrics: pickNumber(rec?.lyrics, ['with_lyrics', 'withLyrics']),
            synced_lyrics: pickNumber(rec?.lyrics, ['synced_lyrics', 'syncedLyrics']),
            embedded_lyrics: pickNumber(rec?.lyrics, ['embedded_lyrics', 'embeddedLyrics']),
        },
    };
}

/**
 * Get library snapshots for historical tracking
 * @param days - Number of days to look back
 */
export async function getLibrarySnapshots(days: number = 30): Promise<LibrarySnapshot[]> {
    const raw = await invokeCommand<unknown>('get_library_snapshots', { days });
    return asArray<LibrarySnapshot>(raw);
}

/**
 * Get duplicate tracks statistics (by Title + Primary Artist)
 */
export async function getDuplicateStats(): Promise<number> {
    const stats = await invokeCommand<unknown>('get_duplicate_stats');
    return typeof stats === 'number' && Number.isFinite(stats) ? stats : 0;
}

export interface ServiceStatItem {
    service_name: string;
    track_count: number;
    album_count: number;
    artist_count: number;
    playlist_count: number;
}

export interface QualityStatItem {
    quality: string;
    count: number;
    percentage: number;
}

export interface DashboardStats {
    total_tracks: number;
    total_albums: number;
    total_artists: number;
    total_playlists: number;
    total_downloads: number;
    total_favorites: number;
    lyrics_coverage_percentage: number;
    enriched_metadata_percentage: number;
    services: ServiceStatItem[];
    quality_distribution: QualityStatItem[];
}

export interface ServiceHealthCheck {
    service: string;
    is_connected: boolean;
    account_name?: string;
    token_status: string;
    rate_limit_status: string;
    last_synced?: string;
    last_error?: string;
}

export interface SystemHealthChecks {
    database_ok: boolean;
    ffmpeg_ok: boolean;
    services: ServiceHealthCheck[];
    background_worker_active: boolean;
}

/**
 * Get aggregated dashboard statistics
 */
export async function getDashboardStats(): Promise<DashboardStats> {
    const raw = await invokeCommand<unknown>('get_dashboard_stats');
    return {
        total_tracks: pickNumber(raw, ['total_tracks', 'totalTracks']),
        total_albums: pickNumber(raw, ['total_albums', 'totalAlbums']),
        total_artists: pickNumber(raw, ['total_artists', 'totalArtists']),
        total_playlists: pickNumber(raw, ['total_playlists', 'totalPlaylists']),
        total_downloads: pickNumber(raw, ['total_downloads', 'totalDownloads']),
        total_favorites: pickNumber(raw, ['total_favorites', 'totalFavorites']),
        lyrics_coverage_percentage: pickNumber(raw, ['lyrics_coverage_percentage', 'lyricsCoveragePercentage']),
        enriched_metadata_percentage: pickNumber(raw, ['enriched_metadata_percentage', 'enrichedMetadataPercentage']),
        services: asArray<ServiceStatItem>((raw as Record<string, unknown> | null)?.services).filter((s) => asRecord(s) !== null),
        quality_distribution: asArray<QualityStatItem>((raw as Record<string, unknown> | null)?.quality_distribution).filter((q) => asRecord(q) !== null),
    };
}

/**
 * Get real-time system health checks
 */
export async function getHealthChecks(): Promise<SystemHealthChecks> {
    const raw = await invokeCommand<unknown>('get_health_checks');
    return {
        database_ok: (raw as Record<string, unknown> | null)?.database_ok === true,
        ffmpeg_ok: (raw as Record<string, unknown> | null)?.ffmpeg_ok === true,
        services: asArray<ServiceHealthCheck>((raw as Record<string, unknown> | null)?.services)
            .filter((s) => asRecord(s) !== null)
            .map((s) => ({
                service: asString(s?.service),
                is_connected: s?.is_connected === true,
                account_name: typeof s?.account_name === 'string' ? s.account_name : undefined,
                token_status: asString(s?.token_status),
                rate_limit_status: asString(s?.rate_limit_status),
                last_synced: typeof s?.last_synced === 'string' ? s.last_synced : undefined,
                last_error: typeof s?.last_error === 'string' ? s.last_error : undefined,
            })),
        background_worker_active: (raw as Record<string, unknown> | null)?.background_worker_active === true,
    };
}

export interface AutoResolveDuplicatesResult {
    groups_resolved: number;
    tracks_removed: number;
}

/**
 * Auto-resolve duplicate track groups keeping the highest quality version
 */
export async function autoResolveDuplicates(): Promise<AutoResolveDuplicatesResult> {
    const raw = await invokeCommand<unknown>('auto_resolve_duplicates');
    return {
        groups_resolved: pickNumber(raw, ['groups_resolved', 'groupsResolved']),
        tracks_removed: pickNumber(raw, ['tracks_removed', 'tracksRemoved']),
    };
}

/** Batch processing health report (matches Rust BatchHealthReport). */
export interface BatchHealthReport {
    [key: string]: unknown;
}

/**
 * Audit batch processing health, database integrity, queue state and the
 * staging directory (IN-5: exposed diagnostic capability).
 */
export async function runBatchHealthCheck(): Promise<BatchHealthReport> {
    const raw = await invokeCommand<unknown>('run_batch_health_check');
    return asRecord(raw) ?? {};
}

// Export as namespace
export const dashboardApi = {
    getServiceHealth,
    getDashboardSnapshot,
    getLibrarySnapshots,
    getDuplicateStats,
    getDashboardStats,
    getHealthChecks,
    autoResolveDuplicates,
    runBatchHealthCheck,
};

