/**
 * Dashboard API
 * 
 * Tauri commands for dashboard statistics and historical snapshots.
 */

import { invokeCommand } from './tauri';
import type { LibrarySnapshot, ServiceHealthInfo } from './types';

/**
 * Get service health status for all connected services
 */
export async function getServiceHealth(): Promise<ServiceHealthInfo[]> {
    return invokeCommand<ServiceHealthInfo[]>('get_service_health');
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
    return invokeCommand<LibrarySnapshot[]>('get_library_snapshots', { days });
}

/**
 * Get duplicate tracks statistics (by Title + Primary Artist)
 */
export async function getDuplicateStats(): Promise<number> {
    return invokeCommand<number>('get_duplicate_stats');
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
    return invokeCommand<DashboardStats>('get_dashboard_stats');
}

/**
 * Get real-time system health checks
 */
export async function getHealthChecks(): Promise<SystemHealthChecks> {
    return invokeCommand<SystemHealthChecks>('get_health_checks');
}

// Export as namespace
export const dashboardApi = {
    getServiceHealth,
    createLibrarySnapshot,
    getLibrarySnapshots,
    getDuplicateStats,
    getDashboardStats,
    getHealthChecks,
};

