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
    identifyAudio,
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
        mockInvoke((cmd, args) => {
            if (cmd === 'enrich_metadata') return { updatedFields: ['title'] };
            if (cmd === 'batch_enrich_metadata') return { enriched: 3 };
            if (cmd === 'start_library_enrichment') {
                const mode = (args as { mode?: string })?.mode;
                if (mode === 'selection') {
                    return { totalTracks: 1, modifiedTracks: 1, failedTracks: 0 };
                }
                return { totalTracks: 9, modifiedTracks: 5, failedTracks: 2 };
            }
            if (cmd === 'enrich_metadata_musicbrainz') return { total: 1, enriched: 1, failed: 0 };
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

/**
 * identifyAudio must read the contract that scripts/fingerprint_bridge.py emits:
 * `{ success, data: { matches: [...] } }`. It used to return the whole envelope
 * through asArray(), so every caller saw [] and AcoustID identification never
 * matched a track.
 */
describe('identifyAudio reads the fingerprint bridge matches contract', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    const bridgePayload = {
        success: true,
        data: {
            file: '/music/track.flac',
            duration: 180,
            matches: [
                {
                    acoustid: 'acoustid-id-1',
                    acoustid_id: 'acoustid-id-1',
                    score: 0.98,
                    recording_id: 'recording-mbid-1',
                    title: 'Test Track',
                    artist: 'Test Artist',
                    album: null,
                    duration: 180,
                    musicbrainz_artistid: 'artist-mbid-1',
                },
            ],
        },
    };

    it('maps data.matches[] into ranked AcoustID matches', async () => {
        mockInvoke((cmd) => (cmd === 'identify_audio' ? bridgePayload : null));

        const matches = await identifyAudio('/music/track.flac');

        expect(matches).toHaveLength(1);
        expect(matches[0].recording_id).toBe('recording-mbid-1');
        expect(matches[0].acoustid_id).toBe('acoustid-id-1');
        expect(matches[0].musicbrainz_artistid).toBe('artist-mbid-1');
        expect(matches[0].title).toBe('Test Track');
        expect(matches[0].artist).toBe('Test Artist');
        expect(matches[0].score).toBe(0.98);
        expect(matches[0].album).toBeNull();
        expect(matches[0].duration).toBe(180);
    });

    it('forwards the file path as filePath', async () => {
        const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args: args as Record<string, unknown> });
            return cmd === 'identify_audio' ? bridgePayload : null;
        });

        await identifyAudio('/music/track.flac');

        expect(calls).toContainEqual({ cmd: 'identify_audio', args: { filePath: '/music/track.flac' } });
    });

    it('returns [] for failures and malformed payloads instead of throwing', async () => {
        mockInvoke((cmd) => (cmd === 'identify_audio'
            ? { success: false, error: 'No matches found' }
            : null));
        expect(await identifyAudio('/music/track.flac')).toEqual([]);

        // Old envelope shape (data.recordings) and empty responses stay safe.
        mockInvoke((cmd) => (cmd === 'identify_audio'
            ? { success: true, data: { recordings: [{ id: 'recording-mbid-1' }] } }
            : null));
        expect(await identifyAudio('/music/track.flac')).toEqual([]);

        mockInvoke(() => null);
        expect(await identifyAudio('/music/track.flac')).toEqual([]);

        mockInvoke((cmd) => (cmd === 'identify_audio' ? { success: true, data: { matches: null } } : null));
        expect(await identifyAudio('/music/track.flac')).toEqual([]);
    });

    it('fills missing match fields with nulls so the UI never crashes', async () => {
        mockInvoke((cmd) => (cmd === 'identify_audio'
            ? { success: true, data: { matches: [{ recording_id: 'recording-mbid-2' }] } }
            : null));

        const [match] = await identifyAudio('/music/track.flac');

        expect(match.recording_id).toBe('recording-mbid-2');
        expect(match.acoustid).toBe('');
        expect(match.acoustid_id).toBeNull();
        expect(match.title).toBeNull();
        expect(match.artist).toBeNull();
        expect(match.album).toBeNull();
        expect(match.duration).toBeNull();
        expect(match.musicbrainz_artistid).toBeNull();
        expect(match.score).toBe(0);
    });
});
