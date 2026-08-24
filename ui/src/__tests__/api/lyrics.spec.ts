/**
 * lyrics.spec.ts
 * Regression tests: batch counters default to 0 when the backend omits them.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { batchFetchLyrics, batchEmbedLyrics, fetchMissingLyrics, getLyricsStats, getAllLyrics } from '@/api/lyrics';
import { mockInvoke, resetMocks } from '../setup';

describe('lyrics_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('defaults fetched/failed/skipped to 0 when missing or null', async () => {
        mockInvoke((cmd) => (cmd === 'batch_fetch_lyrics' ? { fetched: 5 } : null));

        const res = await batchFetchLyrics([1, 2]);
        expect(res.fetched).toBe(5);
        expect(res.failed).toBe(0);
        expect(res.skipped).toBe(0);

        mockInvoke(() => null);
        const missing = await fetchMissingLyrics();
        expect(missing).toEqual({ fetched: 0, failed: 0, skipped: 0 });
    });

    it('defaults embedded/failed/skipped for batch_embed_lyrics', async () => {
        mockInvoke((cmd) => (cmd === 'batch_embed_lyrics' ? {} : null));

        const res = await batchEmbedLyrics([1]);
        expect(res.embedded).toBe(0);
        expect(res.failed).toBe(0);
        expect(res.skipped).toBe(0);
    });

    it('defaults every rendered counter of get_lyrics_stats (DashboardView)', async () => {
        mockInvoke((cmd) => (cmd === 'get_lyrics_stats' ? { total_tracks: 30, with_lyrics: 10 } : null));

        const stats = await getLyricsStats();
        expect(stats.total_tracks).toBe(30);
        expect(stats.with_lyrics).toBe(10);
        expect(stats.synced_lyrics).toBe(0);
        expect(stats.embedded_lyrics).toBe(0);

        mockInvoke(() => null);
        const zeroed = await getLyricsStats();
        expect(zeroed.total_tracks).toBe(0);
        expect(zeroed.with_lyrics).toBe(0);
        expect(zeroed.synced_lyrics).toBe(0);
        expect(zeroed.embedded_lyrics).toBe(0);
    });

    it('coerces non-array get_all_lyrics responses to []', async () => {
        mockInvoke(() => null);
        expect(await getAllLyrics()).toEqual([]);
    });
});
