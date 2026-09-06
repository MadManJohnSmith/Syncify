/**
 * metadata.spec.ts
 * Regression tests: metadata stats and batch counters must survive missing fields
 * (the Rust MetadataStats serializes camelCase; both spellings are accepted).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getMetadataStats,
    getTracksNeedingMetadata,
    enrichMetadata,
    batchEnrichMetadata,
    enrichAllNeeding,
    autoMatchMusicBrainz,
    findAudioDuplicates,
} from '@/api/metadata';
import { mockInvoke, resetMocks } from '../setup';

describe('metadata_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('accepts the real camelCase backend payload for get_metadata_stats', async () => {
        mockInvoke((cmd) => (cmd === 'get_metadata_stats'
            ? { totalTracks: 100, withIsrc: 80, withMusicbrainzId: 60, withAlbum: 90, withYear: 70, withGenre: 50, withArt: 40, averageCompleteness: 78.5 }
            : null));

        const stats = await getMetadataStats();

        expect(stats.total_tracks).toBe(100);
        expect(stats.with_isrc).toBe(80);
        expect(stats.with_musicbrainz_id).toBe(60);
        expect(stats.with_album).toBe(90);
        expect(stats.with_year).toBe(70);
        expect(stats.with_genre).toBe(50);
        expect(stats.with_art).toBe(40);
        expect(stats.average_completeness).toBe(78.5);
    });

    it('defaults every counter of get_metadata_stats to 0 on empty/null payload', async () => {
        mockInvoke((cmd) => (cmd === 'get_metadata_stats' ? {} : null));

        const stats = await getMetadataStats();

        expect(stats.total_tracks).toBe(0);
        expect(stats.with_isrc).toBe(0);
        expect(stats.with_musicbrainz_id).toBe(0);
        expect(stats.with_album).toBe(0);
        expect(stats.with_year).toBe(0);
        expect(stats.with_genre).toBe(0);
        expect(stats.with_art).toBe(0);
        expect(stats.average_completeness).toBe(0);

        mockInvoke(() => null);
        expect((await getMetadataStats()).total_tracks).toBe(0);
    });

    it('coerces non-array track lists to []', async () => {
        mockInvoke(() => null);
        expect(await getTracksNeedingMetadata(10)).toEqual([]);
    });

    it('defaults enrichment result fields', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'enrich_metadata') return { updatedFields: ['title'] };
            if (cmd === 'batch_enrich_metadata') return { enriched: 3 };
            if (cmd === 'enrich_all_needing_metadata') return { total: 9, enriched: 5, failed: 2 };
            if (cmd === 'auto_match_musicbrainz') return { matched: 1 };
            if (cmd === 'find_audio_duplicates') return { groups: [{ fingerprint: 'abc', tracks: [] }] };
            return null;
        });

        const enrich = await enrichMetadata(1);
        expect(enrich.success).toBe(false); // missing success → false
        expect(enrich.updatedFields).toEqual(['title']);
        expect(enrich.error).toBeUndefined();

        const batch = await batchEnrichMetadata([1]);
        expect(batch.enriched).toBe(3);
        expect(batch.failed).toBe(0);
        expect(batch.skipped).toBe(0);

        const all = await enrichAllNeeding();
        expect(all.total).toBe(9);
        expect(all.enriched).toBe(5);
        expect(all.failed).toBe(2);

        const auto = await autoMatchMusicBrainz([1]);
        expect(auto.matched).toBe(1);
        expect(auto.failed).toBe(0);
        expect(auto.noMatch).toBe(0);

        const dupes = await findAudioDuplicates();
        expect(dupes.groups).toHaveLength(1);
        expect(dupes.totalDuplicates).toBe(0);

        // Full-null responses stay renderable
        mockInvoke(() => null);
        expect((await findAudioDuplicates()).groups).toEqual([]);
        expect((await enrichMetadata(1)).updatedFields).toEqual([]);
    });

    it('batchEnrichMetadata extracts fields correctly from BridgeResult data envelope and flat raw', async () => {
        // Flat raw format
        mockInvoke((cmd) => (cmd === 'batch_enrich_metadata' ? { enriched: 4, failed: 1, skipped: 0 } : null));
        const flatRes = await batchEnrichMetadata([10, 11]);
        expect(flatRes.enriched).toBe(4);
        expect(flatRes.failed).toBe(1);
        expect(flatRes.skipped).toBe(0);

        // BridgeResult format with nested data
        mockInvoke((cmd) => (cmd === 'batch_enrich_metadata' ? {
            success: true,
            data: {
                batch_id: 'test-batch-uuid',
                total: 5,
                enriched: 3,
                failed: 2,
                skipped: 0,
                results: []
            },
            error: null
        } : null));
        const bridgeRes = await batchEnrichMetadata([1, 2, 3, 4, 5]);
        expect(bridgeRes.enriched).toBe(3);
        expect(bridgeRes.failed).toBe(2);
        expect(bridgeRes.skipped).toBe(0);
    });
});
