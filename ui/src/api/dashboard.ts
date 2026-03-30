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

// Export as namespace
export const dashboardApi = {
    getServiceHealth,
    createLibrarySnapshot,
    getLibrarySnapshots,
    getDuplicateStats,
};
