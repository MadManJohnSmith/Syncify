/**
 * Migration API wrappers - Sprint 6
 */
import { invoke } from '@tauri-apps/api/core';
import type {
    MigrationJob,
    MigrationItem,
    MigrationTemplate,
    MigrationOptions,
    MigrationPreviewResult,
    DestinationTrackMatch
} from './types';

// ==============================================
// MIGRATION HISTORY
// ==============================================

export async function getMigrationHistory(limit?: number): Promise<MigrationJob[]> {
    return invoke('get_migration_history', { limit });
}

export async function getMigrationDetails(jobId: string): Promise<MigrationJob> {
    return invoke('get_migration_details', { jobId });
}

export async function getMigrationItemsByStatus(
    jobId: string,
    status?: string
): Promise<MigrationItem[]> {
    return invoke('get_migration_items_by_status', { jobId, status });
}

export async function deleteMigration(jobId: string): Promise<string> {
    return invoke('delete_migration', { jobId });
}

// ==============================================
// MIGRATION EXECUTION
// ==============================================

export async function previewMigration(
    sourceService: string,
    destinationService: string,
    playlistIds?: string[],
    options?: MigrationOptions
): Promise<MigrationPreviewResult> {
    return invoke('preview_migration', {
        sourceService,
        destinationService,
        playlistIds,
        options: options || {
            match_threshold: 0.80,
            skip_unmatched: true,
            create_playlists: true,
            merge_existing: false,
            download_matched: true
        }
    });
}

export async function startMigration(
    sourceService: string,
    destinationService: string,
    playlistIds?: string[],
    options?: MigrationOptions
): Promise<string> {
    return invoke('start_migration', {
        sourceService,
        destinationService,
        playlistIds,
        options: options || {
            match_threshold: 0.80,
            skip_unmatched: true,
            create_playlists: true,
            merge_existing: false,
            download_matched: true
        }
    });
}

export async function cancelMigration(jobId: string): Promise<string> {
    return invoke('cancel_migration', { jobId });
}

export async function retryFailedItems(jobId: string): Promise<number> {
    return invoke('retry_failed_items', { jobId });
}

// ==============================================
// MIGRATION TEMPLATES
// ==============================================

export async function getMigrationTemplates(): Promise<MigrationTemplate[]> {
    return invoke('get_migration_templates');
}

export async function saveMigrationTemplate(
    name: string,
    description: string | null,
    sourceService: string,
    destinationService: string,
    options: MigrationOptions
): Promise<number> {
    return invoke('save_migration_template', {
        name,
        description,
        sourceService,
        destinationService,
        options
    });
}

export async function deleteMigrationTemplate(templateId: number): Promise<string> {
    return invoke('delete_migration_template', { templateId });
}

export async function useMigrationTemplate(templateId: number): Promise<MigrationTemplate> {
    return invoke('use_migration_template', { templateId });
}

// ==============================================
// MANUAL MATCHING
// ==============================================

export async function searchDestinationTrack(
    service: string,
    query: string
): Promise<DestinationTrackMatch[]> {
    return invoke('search_destination_track', { service, query });
}

export async function manualMatchItem(
    itemId: number,
    destinationTrackId: string
): Promise<string> {
    return invoke('manual_match_item', { itemId, destinationTrackId });
}

// Export as namespace
export const migrationApi = {
    getMigrationHistory,
    getMigrationDetails,
    getMigrationItemsByStatus,
    deleteMigration,
    previewMigration,
    startMigration,
    cancelMigration,
    retryFailedItems,
    getMigrationTemplates,
    saveMigrationTemplate,
    deleteMigrationTemplate,
    useMigrationTemplate,
    searchDestinationTrack,
    manualMatchItem,
};
