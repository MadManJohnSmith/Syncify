/**
 * enrichment.spec.ts
 * Regression tests: enrichment preview/status payloads normalize defensively.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    previewLibraryEnrichment,
    startLibraryEnrichment,
    getLibraryEnrichmentStatus,
} from '@/api/enrichment';
import { mockInvoke, resetMocks } from '../setup';

describe('enrichment_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('fills defaults for a minimal EnrichmentPreview (camelCase backend keys)', async () => {
        mockInvoke((cmd) => (cmd === 'preview_library_enrichment' ? { totalTracks: 50, availableSources: ['musicbrainz'] } : null));

        const preview = await previewLibraryEnrichment('incomplete_only');

        expect(preview.totalTracks).toBe(50);
        expect(preview.totalEligible).toBe(0);
        expect(preview.totalComplete).toBe(0);
        expect(preview.totalSkippedPrecedence).toBe(0);
        expect(preview.availableSources).toEqual(['musicbrainz']);
        expect(preview.mode).toBe('incomplete_only'); // missing mode falls back safely
    });

    it('returns a complete EnrichmentJobSummary for a sparse status payload', async () => {
        mockInvoke((cmd) => (cmd === 'get_library_enrichment_status' ? { jobId: 'j1', status: 'running' } : null));

        const status = await getLibraryEnrichmentStatus();

        expect(status).not.toBeNull();
        expect(status!.jobId).toBe('j1');
        expect(status!.status).toBe('running');
        expect(status!.totalTracks).toBe(0);
        expect(status!.processedTracks).toBe(0);
        expect(status!.items).toEqual([]);
        expect(status!.availableSources).toEqual([]);
    });

    it('returns null for an idle/empty enrichment status', async () => {
        mockInvoke(() => null);
        expect(await getLibraryEnrichmentStatus()).toBeNull();
    });

    it('start_library_enrichment normalizes snake_case legacy payloads too', async () => {
        mockInvoke((cmd) => (cmd === 'start_library_enrichment'
            ? { job_id: 'legacy', mode: 'selection', total_tracks: 3, processed_tracks: 1 }
            : null));

        const summary = await startLibraryEnrichment('selection', [1]);

        expect(summary.jobId).toBe('legacy');
        expect(summary.mode).toBe('selection');
        expect(summary.totalTracks).toBe(3);
        expect(summary.processedTracks).toBe(1);
        expect(summary.modifiedTracks).toBe(0);
        expect(summary.status).toBe('queued');
    });
});
