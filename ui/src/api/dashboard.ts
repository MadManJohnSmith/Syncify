/**
 * Dashboard API
 * 
 * Tauri commands for dashboard statistics and historical snapshots.
 */

import { invokeCommand } from './tauri';
import { asArray, asString, asRecord, pickNumber } from './normalize';
import type { LibrarySnapshot, ServiceHealthInfo } from './types';

/**
 * Get service health status for all connected services
 */
export async function getServiceHealth(): Promise<ServiceHealthInfo[]> {
    const raw = await invokeCommand<unknown>('get_service_health');
    return asArray<ServiceHealthInfo>(raw);
}

/**
 * Create a library snapshot for historical tracking
 */
export async function createLibrarySnapshot(): Promise<LibrarySnapshot> {
    return invokeCommand<LibrarySnapshot>('create_library_snapshot');
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

// Export as namespace
export const dashboardApi = {
    getServiceHealth,
    createLibrarySnapshot,
    getLibrarySnapshots,
    getDuplicateStats,
    getDashboardStats,
    getHealthChecks,
    autoResolveDuplicates,
};

