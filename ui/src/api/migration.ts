/**
 * Migration API wrappers - Sprint 6
 */
import { invoke } from '@tauri-apps/api/core';
import { asArray, asNumber, asRecord } from './normalize';
import type {
    MigrationJob,
    MigrationItem,
    MigrationTemplate,
    MigrationOptions,
    MigrationPreviewResult,
    DestinationTrackMatch,
    PlaylistPreview
} from './types';

// ==============================================
// MIGRATION HISTORY
// ==============================================

export async function getMigrationHistory(limit?: number): Promise<MigrationJob[]> {
    const raw = await invoke<unknown>('get_migration_history', { limit });
    return asArray<MigrationJob>(raw);
}

export async function getMigrationDetails(jobId: string): Promise<MigrationJob> {
    return invoke('get_migration_details', { jobId });
}

export async function getMigrationItemsByStatus(
    jobId: string,
    status?: string
): Promise<MigrationItem[]> {
    const raw = await invoke<unknown>('get_migration_items_by_status', { jobId, status });
    return asArray<MigrationItem>(raw);
}

export async function deleteMigration(jobId: string): Promise<string> {
    return invoke('delete_migration', { jobId });
}

// ==============================================
// MIGRATION EXECUTION
// ==============================================

/**
 * Services the backend engine can migrate TO (data-driven destination list).
 * A new destination only needs backend support; the wizard renders whatever
 * this returns (intersected with the user's connected accounts).
 */
export async function getMigrationDestinations(): Promise<string[]> {
    const raw = await invoke<unknown>('get_migration_destinations');
    return asArray<string>(raw).map(service => String(service).toLowerCase());
}

function validateMigrationAccounts(
    sourceService: string,
    destinationService: string,
    sourceAccountId?: number | null,
    destinationAccountId?: number | null
): void {
    if (sourceService.toLowerCase() !== destinationService.toLowerCase()) return;
    // null is the global legacy source (library_items plus every playlist),
    // never a safe alias for the active account of this same service.
    if (sourceAccountId == null || destinationAccountId == null || sourceAccountId === destinationAccountId) {
        throw new Error('Same-service migration requires two explicit, distinct accounts');
    }
}

export async function previewMigration(
    sourceService: string,
    destinationService: string,
    playlistIds?: string[],
    options?: MigrationOptions,
    sourceAccountId?: number | null,
    destinationAccountId?: number | null
): Promise<MigrationPreviewResult> {
    validateMigrationAccounts(sourceService, destinationService, sourceAccountId, destinationAccountId);
    return invoke('preview_migration', {
        sourceService,
        destinationService,
        playlistIds,
        sourceAccountId: sourceAccountId ?? null,
        destinationAccountId: destinationAccountId ?? null,
        options: options || {
            match_threshold: 0.80,
            skip_unmatched: true,
            create_playlists: true,
            merge_existing: false,
            download_matched: true
        }
    }).then(normalizePreviewMigration);
}

function normalizePreviewMigration(raw: unknown): MigrationPreviewResult {
    const rec = asRecord(raw);
    return {
        total_tracks: asNumber(rec?.total_tracks),
        matched_tracks: asNumber(rec?.matched_tracks),
        unmatched_tracks: asNumber(rec?.unmatched_tracks),
        playlists: asArray<PlaylistPreview>(rec?.playlists),
    };
}

export async function startMigration(
    sourceService: string,
    destinationService: string,
    playlistIds?: string[],
    options?: MigrationOptions,
    sourceAccountId?: number | null,
    destinationAccountId?: number | null
): Promise<string> {
    validateMigrationAccounts(sourceService, destinationService, sourceAccountId, destinationAccountId);
    return invoke('start_migration', {
        sourceService,
        destinationService,
        playlistIds,
        sourceAccountId: sourceAccountId ?? null,
        destinationAccountId: destinationAccountId ?? null,
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
    const retried = await invoke<unknown>('retry_failed_items', { jobId });
    return typeof retried === 'number' ? retried : 0;
}

// ==============================================
// MIGRATION TEMPLATES
// ==============================================

export async function getMigrationTemplates(): Promise<MigrationTemplate[]> {
    const raw = await invoke<unknown>('get_migration_templates');
    return asArray<MigrationTemplate>(raw);
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
    query: string,
    accountId?: number | null
): Promise<DestinationTrackMatch[]> {
    const raw = await invoke<unknown>('search_destination_track', { service, query, accountId: accountId ?? null });
    return asArray<DestinationTrackMatch>(raw);
}

export async function manualMatchItem(
    itemId: number,
    destinationTrackId: string
): Promise<string> {
    return invoke('manual_match_item', { itemId, destinationTrackId });
}

/** Migration schema/state audit report (matches Rust MigrationReport). */
export interface MigrationReport {
    schema_version: number;
    schema_ok: boolean;
    missing_tables: string[];
    legacy_services_detected: string[];
    summary: string;
}

/**
 * Run the migration schema/state audit shown in MigrationView (IN-5).
 */
export async function runMigrationAudit(): Promise<MigrationReport> {
    const raw = await invoke<unknown>('run_migration_audit');
    const rec = asRecord(raw);
    return {
        schema_version: asNumber(rec?.schema_version),
        schema_ok: rec?.schema_ok === true,
        missing_tables: asArray<string>(rec?.missing_tables),
        legacy_services_detected: asArray<string>(rec?.legacy_services_detected),
        summary: typeof rec?.summary === 'string' ? rec.summary : '',
    };
}

// Export as namespace
export const migrationApi = {
    getMigrationHistory,
    getMigrationDetails,
    getMigrationItemsByStatus,
    deleteMigration,
    getMigrationDestinations,
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
    runMigrationAudit,
};
