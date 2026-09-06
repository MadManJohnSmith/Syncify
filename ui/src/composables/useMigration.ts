/**
 * Migration Composable - Sprint 6
 * Manages migration state and backend integration
 */
import { ref, reactive, computed, getCurrentInstance, onUnmounted } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { TauriEvents } from '@/api/tauri';
import {
    getMigrationHistory,
    getMigrationDetails,
    getMigrationItemsByStatus,
    previewMigration,
    startMigration,
    cancelMigration,
    retryFailedItems,
    deleteMigration,
    getMigrationTemplates,
    saveMigrationTemplate,
    deleteMigrationTemplate,
    useMigrationTemplate,
    searchDestinationTrack,
    manualMatchItem
} from '../api/migration';
import type {
    MigrationJob,
    MigrationItem,
    MigrationTemplate,
    MigrationOptions,
    MigrationPreviewResult,
    MigrationProgress,
    DestinationTrackMatch
} from '../api/types';

export function useMigration() {
    // Loading states
    const isLoading = ref(false);
    const isStartingMigration = ref(false);
    const isPreviewing = ref(false);

    // Migration history
    const history = ref<MigrationJob[]>([]);
    const selectedJob = ref<MigrationJob | null>(null);
    const selectedJobItems = ref<MigrationItem[]>([]);

    // Templates
    const templates = ref<MigrationTemplate[]>([]);

    // Current migration
    const currentJobId = ref<string | null>(null);
    const previewResult = ref<MigrationPreviewResult | null>(null);
    const progress = reactive<MigrationProgress>({
        job_id: '',
        current_item: 0,
        total_items: 0,
        current_track: '',
        status: 'idle',
        completed_count: 0,
        failed_count: 0,
        skipped_count: 0
    });

    // Manual matching
    const isSearching = ref(false);
    const searchResults = ref<DestinationTrackMatch[]>([]);

    // Event listener cleanup
    let progressUnlisten: UnlistenFn | null = null;

    // Default options
    const defaultOptions: MigrationOptions = {
        match_threshold: 0.80,
        skip_unmatched: true,
        create_playlists: true,
        merge_existing: false,
        download_matched: true
    };

    // ========================
    // LOAD DATA
    // ========================

    async function loadHistory(limit = 50): Promise<void> {
        isLoading.value = true;
        try {
            history.value = await getMigrationHistory(limit);
        } catch (e) {
            console.error('Failed to load migration history:', e);
        } finally {
            isLoading.value = false;
        }
    }

    async function loadTemplates(): Promise<void> {
        try {
            templates.value = await getMigrationTemplates();
        } catch (e) {
            console.error('Failed to load templates:', e);
        }
    }

    async function loadJobDetails(jobId: string): Promise<void> {
        try {
            selectedJob.value = await getMigrationDetails(jobId);
            selectedJobItems.value = await getMigrationItemsByStatus(jobId);
        } catch (e) {
            console.error('Failed to load job details:', e);
        }
    }

    async function loadJobItemsByStatus(jobId: string, status?: string): Promise<void> {
        try {
            selectedJobItems.value = await getMigrationItemsByStatus(jobId, status);
        } catch (e) {
            console.error('Failed to load job items:', e);
        }
    }

    // ========================
    // MIGRATION EXECUTION
    // ========================

    async function preview(
        sourceService: string,
        destinationService: string,
        playlistIds?: string[],
        options: MigrationOptions = defaultOptions
    ): Promise<MigrationPreviewResult | null> {
        isPreviewing.value = true;
        try {
            previewResult.value = await previewMigration(
                sourceService,
                destinationService,
                playlistIds,
                options
            );
            return previewResult.value;
        } catch (e) {
            console.error('Failed to preview migration:', e);
            return null;
        } finally {
            isPreviewing.value = false;
        }
    }

    async function start(
        sourceService: string,
        destinationService: string,
        playlistIds?: string[],
        options: MigrationOptions = defaultOptions
    ): Promise<string | null> {
        isStartingMigration.value = true;
        try {
            const jobId = await startMigration(
                sourceService,
                destinationService,
                playlistIds,
                options
            );
            currentJobId.value = jobId;
            progress.job_id = jobId;
            progress.status = 'running';
            return jobId;
        } catch (e) {
            console.error('Failed to start migration:', e);
            return null;
        } finally {
            isStartingMigration.value = false;
        }
    }

    async function cancel(): Promise<boolean> {
        if (!currentJobId.value) return false;
        try {
            await cancelMigration(currentJobId.value);
            progress.status = 'cancelled';
            return true;
        } catch (e) {
            console.error('Failed to cancel migration:', e);
            return false;
        }
    }

    async function retryFailed(jobId: string): Promise<number> {
        try {
            const count = await retryFailedItems(jobId);
            await loadHistory();
            return count;
        } catch (e) {
            console.error('Failed to retry items:', e);
            return 0;
        }
    }

    async function deleteJob(jobId: string): Promise<boolean> {
        try {
            await deleteMigration(jobId);
            await loadHistory();
            return true;
        } catch (e) {
            console.error('Failed to delete migration:', e);
            return false;
        }
    }

    // ========================
    // TEMPLATES
    // ========================

    async function saveTemplate(
        name: string,
        description: string | null,
        sourceService: string,
        destinationService: string,
        options: MigrationOptions = defaultOptions
    ): Promise<boolean> {
        try {
            await saveMigrationTemplate(name, description, sourceService, destinationService, options);
            await loadTemplates();
            return true;
        } catch (e) {
            console.error('Failed to save template:', e);
            return false;
        }
    }

    async function deleteTemplate(templateId: number): Promise<boolean> {
        try {
            await deleteMigrationTemplate(templateId);
            await loadTemplates();
            return true;
        } catch (e) {
            console.error('Failed to delete template:', e);
            return false;
        }
    }

    async function getTemplateDetails(templateId: number): Promise<MigrationTemplate | null> {
        try {
            return await useMigrationTemplate(templateId);
        } catch (e) {
            console.error('Failed to get template:', e);
            return null;
        }
    }

    // ========================
    // MANUAL MATCHING
    // ========================

    async function searchTracks(service: string, query: string): Promise<void> {
        if (!query.trim()) {
            searchResults.value = [];
            return;
        }
        isSearching.value = true;
        try {
            searchResults.value = await searchDestinationTrack(service, query);
        } catch (e) {
            console.error('Failed to search tracks:', e);
        } finally {
            isSearching.value = false;
        }
    }

    async function matchItem(itemId: number, destTrackId: string): Promise<boolean> {
        try {
            await manualMatchItem(itemId, destTrackId);
            return true;
        } catch (e) {
            console.error('Failed to match item:', e);
            return false;
        }
    }

    // ========================
    // EVENT LISTENER
    // ========================

    async function setupProgressListener(): Promise<void> {
        // Clean up previous listener to prevent duplicate subscriptions
        cleanup();

        if (getCurrentInstance()) {
            onUnmounted(() => {
                cleanup();
            });
        }

        progressUnlisten = await listen<MigrationProgress>(TauriEvents.MIGRATION_PROGRESS, (event) => {
            const p = event.payload;
            progress.job_id = p.job_id;
            progress.current_item = p.current_item;
            progress.total_items = p.total_items;
            progress.current_track = p.current_track;
            progress.status = p.status;
            progress.completed_count = p.completed_count;
            progress.failed_count = p.failed_count;
            progress.skipped_count = p.skipped_count;

            // Refresh history when completed
            if (p.status === 'completed') {
                loadHistory();
            }
        });
    }

    function cleanup(): void {
        if (progressUnlisten) {
            progressUnlisten();
            progressUnlisten = null;
        }
    }

    // ========================
    // HELPERS
    // ========================

    function formatHistoryItem(job: MigrationJob) {
        const successRate = job.total_items > 0
            ? Math.round((job.completed_items / job.total_items) * 100)
            : 0;
        return {
            id: job.id,
            date: new Date(job.created_at).toLocaleString(),
            source: job.source_service,
            dest: job.destination_service,
            status: job.status,
            successRate,
            successCount: job.completed_items,
            totalCount: job.total_items,
            failedCount: job.failed_items,
            skippedCount: job.skipped_items
        };
    }

    // Auto-cleanup on unmount if composable is initialized in an active Vue component
    if (getCurrentInstance()) {
        onUnmounted(() => {
            cleanup();
        });
    }

    return {
        // State
        isLoading,
        isStartingMigration,
        isPreviewing,
        isSearching,
        history,
        templates,
        selectedJob,
        selectedJobItems,
        currentJobId,
        previewResult,
        progress,
        searchResults,
        defaultOptions,

        // Actions
        loadHistory,
        loadTemplates,
        loadJobDetails,
        loadJobItemsByStatus,
        preview,
        start,
        cancel,
        retryFailed,
        deleteJob,
        saveTemplate,
        deleteTemplate,
        getTemplateDetails,
        searchTracks,
        matchItem,
        setupProgressListener,
        cleanup,
        formatHistoryItem
    };
}
