/**
 * metadata_commands.spec.ts
 *
 * TASK-04 verification suite:
 * Verifies that ui/src/api/metadata.ts correctly aligns with native Tauri IPC commands:
 * 1. `writeTrackTags` -> `write_track_tags` with { trackId, tags, metadata }
 * 2. `readTrackTags` -> `read_track_tags` with { trackId }
 * 3. Backward compatibility aliases `writeMetadataToFile` and `readMetadataFromFile`
 * 4. `enrichAllNeeding` -> `start_library_enrichment` with { mode: 'incomplete_only' }
 * 5. Incremental library enrichment suite (`startLibraryEnrichment`, `previewLibraryEnrichment`, etc.)
 * 6. `autoMatchMusicBrainz` and `enrichMetadataMusicBrainz` -> `enrich_metadata_musicbrainz` / `start_library_enrichment`
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    writeTrackTags,
    readTrackTags,
    writeMetadataToFile,
    readMetadataFromFile,
    enrichAllNeeding,
    startLibraryEnrichment,
    previewLibraryEnrichment,
    cancelLibraryEnrichment,
    getLibraryEnrichmentStatus,
    autoMatchMusicBrainz,
    enrichMetadataMusicBrainz,
    type TrackTags,
    type TrackTagsSnapshot,
    type TagVerification,
} from '@/api/metadata';
import { mockInvoke, resetMocks } from '../setup';

describe('TASK-04: Metadata and Tag Tauri IPC Commands', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    describe('Track Tags Read & Write (Disk FLAC / Container)', () => {
        it('writeTrackTags invokes native write_track_tags with trackId and tags payload', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'write_track_tags') {
                    const mockReport: TagVerification = {
                        file_exists: true,
                        flac_valid: true,
                        tags_match: true,
                        cover_present: true,
                        cover_size_bytes: 45000,
                        cover_mime: 'image/jpeg',
                        lyrics_present: true,
                        synced_lyrics_present: false,
                        unsynced_lyrics_present: true,
                        bpm_present: true,
                    };
                    return mockReport;
                }
                return null;
            });

            const tagsToEdit: TrackTags = {
                title: 'Stairway to Heaven',
                artist: 'Led Zeppelin',
                album: 'Led Zeppelin IV',
                genre: 'Classic Rock',
                release_year: '1971',
                track_number: 4,
                bpm: 82,
            };

            const result = await writeTrackTags(42, tagsToEdit);

            expect(invokedCmd).toBe('write_track_tags');
            expect(invokedPayload).toMatchObject({
                trackId: 42,
                tags: tagsToEdit,
                metadata: tagsToEdit,
            });
            expect(result.file_exists).toBe(true);
            expect(result.flac_valid).toBe(true);
            expect(result.tags_match).toBe(true);
            expect(result.bpm_present).toBe(true);
        });

        it('readTrackTags invokes native read_track_tags with trackId and returns TrackTagsSnapshot', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'read_track_tags') {
                    const mockSnapshot: TrackTagsSnapshot = {
                        track_id: 101,
                        file_path: '/music/Pink Floyd/Money.flac',
                        file_format: 'FLAC',
                        all_tags: {
                            TITLE: ['Money'],
                            ARTIST: ['Pink Floyd'],
                            ALBUM: ['The Dark Side of the Moon'],
                        },
                        has_cover: true,
                        cover_mime: 'image/jpeg',
                    };
                    return mockSnapshot;
                }
                return null;
            });

            const snapshot = await readTrackTags(101);

            expect(invokedCmd).toBe('read_track_tags');
            expect(invokedPayload).toEqual({ trackId: 101 });
            expect(snapshot.track_id).toBe(101);
            expect(snapshot.file_path).toBe('/music/Pink Floyd/Money.flac');
            expect(snapshot.file_format).toBe('FLAC');
            expect(snapshot.all_tags.TITLE).toEqual(['Money']);
            expect(snapshot.has_cover).toBe(true);
        });

        it('writeMetadataToFile compatibility alias delegates to write_track_tags and returns boolean', async () => {
            let invokedCmd = '';

            mockInvoke((cmd) => {
                invokedCmd = cmd;
                if (cmd === 'write_track_tags') {
                    return {
                        file_exists: true,
                        flac_valid: true,
                        tags_match: true,
                        cover_present: false,
                        lyrics_present: false,
                        synced_lyrics_present: false,
                        unsynced_lyrics_present: false,
                        bpm_present: false,
                    };
                }
                return null;
            });

            const ok = await writeMetadataToFile(77, {
                title: 'Test Song',
                artist: 'Test Artist',
                album: 'Test Album',
            });

            expect(invokedCmd).toBe('write_track_tags');
            expect(ok).toBe(true);
        });

        it('readMetadataFromFile compatibility alias parses numeric ID and delegates to read_track_tags', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'read_track_tags') {
                    return {
                        track_id: 88,
                        file_path: '/music/track.flac',
                        file_format: 'FLAC',
                        all_tags: {},
                        has_cover: false,
                    };
                }
                return null;
            });

            const res = await readMetadataFromFile(88);
            expect(invokedCmd).toBe('read_track_tags');
            expect(invokedPayload).toEqual({ trackId: 88 });
            expect((res as TrackTagsSnapshot).track_id).toBe(88);

            const resStr = await readMetadataFromFile('88');
            expect(invokedCmd).toBe('read_track_tags');
            expect(invokedPayload).toEqual({ trackId: 88 });
            expect((resStr as TrackTagsSnapshot).track_id).toBe(88);
        });
    });

    describe('Library Enrichment IPC Alignment', () => {
        it('enrichAllNeeding invokes native start_library_enrichment with mode incomplete_only', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'start_library_enrichment') {
                    return {
                        jobId: 'job-1234',
                        mode: 'incomplete_only',
                        status: 'completed',
                        totalTracks: 50,
                        processedTracks: 50,
                        modifiedTracks: 35,
                        skippedCompleteTracks: 10,
                        skippedPrecedenceTracks: 5,
                        failedTracks: 0,
                        availableSources: ['musicbrainz', 'lastfm'],
                        startedAt: '2025-01-01T00:00:00Z',
                    };
                }
                return null;
            });

            const result = await enrichAllNeeding();

            expect(invokedCmd).toBe('start_library_enrichment');
            expect(invokedPayload).toEqual({ mode: 'incomplete_only' });
            expect(result.total).toBe(50);
            expect(result.enriched).toBe(35);
            expect(result.failed).toBe(0);
            expect(result.jobSummary?.jobId).toBe('job-1234');
        });

        it('startLibraryEnrichment invokes start_library_enrichment with custom mode and trackIds', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'start_library_enrichment') {
                    return {
                        jobId: 'job-selection-1',
                        mode: 'selection',
                        status: 'running',
                        totalTracks: 2,
                        processedTracks: 1,
                        modifiedTracks: 1,
                        skippedCompleteTracks: 0,
                        skippedPrecedenceTracks: 0,
                        failedTracks: 0,
                        availableSources: ['musicbrainz'],
                        startedAt: '2025-01-01T00:00:00Z',
                    };
                }
                return null;
            });

            const summary = await startLibraryEnrichment('selection', [10, 20]);

            expect(invokedCmd).toBe('start_library_enrichment');
            expect(invokedPayload).toEqual({ mode: 'selection', trackIds: [10, 20] });
            expect(summary.jobId).toBe('job-selection-1');
            expect(summary.totalTracks).toBe(2);
        });

        it('previewLibraryEnrichment invokes preview_library_enrichment', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'preview_library_enrichment') {
                    return {
                        totalTracks: 100,
                        totalEligible: 25,
                        totalComplete: 70,
                        totalSkippedPrecedence: 5,
                        availableSources: ['musicbrainz'],
                        mode: 'incomplete_only',
                    };
                }
                return null;
            });

            const preview = await previewLibraryEnrichment('incomplete_only');

            expect(invokedCmd).toBe('preview_library_enrichment');
            expect(invokedPayload).toEqual({ mode: 'incomplete_only', trackIds: undefined });
            expect(preview.totalEligible).toBe(25);
            expect(preview.totalTracks).toBe(100);
        });

        it('cancelLibraryEnrichment and getLibraryEnrichmentStatus invoke native commands', async () => {
            mockInvoke((cmd) => {
                if (cmd === 'cancel_library_enrichment') return true;
                if (cmd === 'get_library_enrichment_status') {
                    return {
                        jobId: 'job-running',
                        mode: 'incomplete_only',
                        status: 'running',
                        totalTracks: 200,
                        processedTracks: 50,
                        modifiedTracks: 45,
                        skippedCompleteTracks: 5,
                        skippedPrecedenceTracks: 0,
                        failedTracks: 0,
                        availableSources: ['musicbrainz'],
                        startedAt: '2025-01-01T00:00:00Z',
                    };
                }
                return null;
            });

            const cancelled = await cancelLibraryEnrichment();
            expect(cancelled).toBe(true);

            const status = await getLibraryEnrichmentStatus();
            expect(status?.jobId).toBe('job-running');
            expect(status?.status).toBe('running');
        });
    });

    describe('MusicBrainz Matching Alignment', () => {
        it('autoMatchMusicBrainz with specific trackIds invokes start_library_enrichment in selection mode', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'start_library_enrichment') {
                    return {
                        totalTracks: 3,
                        modifiedTracks: 2,
                        failedTracks: 1,
                    };
                }
                return null;
            });

            const res = await autoMatchMusicBrainz([1, 2, 3]);

            expect(invokedCmd).toBe('start_library_enrichment');
            expect(invokedPayload).toEqual({ mode: 'selection', trackIds: [1, 2, 3] });
            expect(res.matched).toBe(2);
            expect(res.failed).toBe(1);
            expect(res.noMatch).toBe(0);
        });

        it('autoMatchMusicBrainz without trackIds invokes enrich_metadata_musicbrainz', async () => {
            let invokedCmd = '';

            mockInvoke((cmd) => {
                invokedCmd = cmd;
                if (cmd === 'enrich_metadata_musicbrainz') {
                    return {
                        total: 10,
                        enriched: 7,
                        failed: 1,
                    };
                }
                return null;
            });

            const res = await autoMatchMusicBrainz();

            expect(invokedCmd).toBe('enrich_metadata_musicbrainz');
            expect(res.matched).toBe(7);
            expect(res.failed).toBe(1);
            expect(res.noMatch).toBe(2); // 10 - (7 + 1)
        });

        it('enrichMetadataMusicBrainz invokes native enrich_metadata_musicbrainz with limit', async () => {
            let invokedCmd = '';
            let invokedPayload: unknown = null;

            mockInvoke((cmd, args) => {
                invokedCmd = cmd;
                invokedPayload = args;
                if (cmd === 'enrich_metadata_musicbrainz') {
                    return {
                        total: 15,
                        enriched: 12,
                        failed: 3,
                    };
                }
                return null;
            });

            const res = await enrichMetadataMusicBrainz(50);

            expect(invokedCmd).toBe('enrich_metadata_musicbrainz');
            expect(invokedPayload).toEqual({ limit: 50 });
            expect(res.total).toBe(15);
            expect(res.enriched).toBe(12);
            expect(res.failed).toBe(3);
        });
    });
});
