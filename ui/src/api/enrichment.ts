/**
 * Incremental Enrichment API (S144)
 *
 * Wraps the library-enrichment Tauri commands with defensive normalization
 * so composables never receive partial payloads.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asString, asRecord } from './normalize';
import type { EnrichmentMode, EnrichmentPreview, EnrichmentJobSummary, TrackEnrichmentReportItem } from './types';

const VALID_MODES: readonly EnrichmentMode[] = ['incomplete_only', 'revalidate_all', 'selection'];

function normalizeMode(rawMode: unknown): EnrichmentMode {
    return (VALID_MODES as readonly string[]).includes(String(rawMode)) ? (rawMode as EnrichmentMode) : 'incomplete_only';
}

function normalizeStatus(rawStatus: unknown): import('./types').JobStatus {
    const status = String(rawStatus);
    const valid: readonly string[] = ['queued', 'running', 'completed', 'cancelled', 'failed'];
    return (valid.includes(status) ? status : 'queued') as import('./types').JobStatus;
}

export function normalizeEnrichmentPreview(raw: unknown): EnrichmentPreview {
    const rec = asRecord(raw);
    return {
        totalTracks: asNumber(rec?.totalTracks ?? rec?.total_tracks),
        totalEligible: asNumber(rec?.totalEligible ?? rec?.total_eligible),
        totalComplete: asNumber(rec?.totalComplete ?? rec?.total_complete),
        totalSkippedPrecedence: asNumber(rec?.totalSkippedPrecedence ?? rec?.total_skipped_precedence),
        availableSources: asArray<string>(rec?.availableSources ?? rec?.available_sources).filter(
            (s) => typeof s === 'string'
        ),
        mode: normalizeMode(rec?.mode),
    };
}

export function normalizeEnrichmentJobSummary(raw: unknown): EnrichmentJobSummary {
    const rec = asRecord(raw);
    return {
        jobId: asString(rec?.jobId ?? rec?.job_id) || 'unknown-job',
        mode: normalizeMode(rec?.mode),
        status: normalizeStatus(rec?.status),
        totalTracks: asNumber(rec?.totalTracks ?? rec?.total_tracks),
        processedTracks: asNumber(rec?.processedTracks ?? rec?.processed_tracks),
        modifiedTracks: asNumber(rec?.modifiedTracks ?? rec?.modified_tracks),
        skippedCompleteTracks: asNumber(rec?.skippedCompleteTracks ?? rec?.skipped_complete_tracks),
        skippedPrecedenceTracks: asNumber(rec?.skippedPrecedenceTracks ?? rec?.skipped_precedence_tracks),
        failedTracks: asNumber(rec?.failedTracks ?? rec?.failed_tracks),
        currentTrack: (rec?.currentTrack as string | null | undefined) ?? null,
        currentPhase: (rec?.currentPhase as string | null | undefined) ?? null,
        items: asArray<TrackEnrichmentReportItem>(rec?.items).filter((i) => asRecord(i) !== null),
        availableSources: asArray<string>(rec?.availableSources ?? rec?.available_sources).filter(
            (s) => typeof s === 'string'
        ),
        startedAt: asString(rec?.startedAt ?? rec?.started_at),
        completedAt: (rec?.completedAt as string | null | undefined) ?? null,
    };
}

/**
 * Preview how many tracks an enrichment run would touch
 */
export async function previewLibraryEnrichment(
    mode: EnrichmentMode,
    trackIds?: number[] | null
): Promise<EnrichmentPreview> {
    const raw = await invokeCommand<unknown>('preview_library_enrichment', {
        mode,
        trackIds: trackIds || null,
    });
    return normalizeEnrichmentPreview(raw);
}

/**
 * Start an incremental enrichment job
 */
export async function startLibraryEnrichment(
    mode: EnrichmentMode,
    trackIds?: number[] | null
): Promise<EnrichmentJobSummary> {
    const raw = await invokeCommand<unknown>('start_library_enrichment', {
        mode,
        trackIds: trackIds || null,
    });
    return normalizeEnrichmentJobSummary(raw);
}

/**
 * Cancel a running enrichment job
 */
export async function cancelLibraryEnrichment(): Promise<void> {
    return invokeCommand<void>('cancel_library_enrichment');
}

/**
 * Fetch the current enrichment job summary (null when idle)
 */
export async function getLibraryEnrichmentStatus(): Promise<EnrichmentJobSummary | null> {
    const raw = await invokeCommand<unknown>('get_library_enrichment_status');
    if (asRecord(raw) === null) return null;
    return normalizeEnrichmentJobSummary(raw);
}

export const enrichmentApi = {
    previewLibraryEnrichment,
    startLibraryEnrichment,
    cancelLibraryEnrichment,
    getLibraryEnrichmentStatus,
};
