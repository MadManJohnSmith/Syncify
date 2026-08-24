/**
 * library.spec.ts
 * API wrapper tests for library.ts
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
    getLibrary,
    getPlaylists,
    addToPlaylist,
    createPlaylist,
    searchTracks,
    enqueueTracks,
    normalizeQualityPreference
} from '@/api/library';
import { resetMocks, mockInvoke } from '../setup';

describe('libraryApi', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    describe('getLibrary', () => {
        it('calls get_library command', async () => {
            const mockPage = { tracks: [], total: 0, offset: 0, limit: 50, has_more: false };
            mockInvoke((cmd) => {
                if (cmd === 'get_library') return mockPage;
                return null;
            });

            const result = await getLibrary();

            expect(invoke).toHaveBeenCalled();
            expect(result.tracks).toEqual([]);
        });

        it('returns tracks with all fields', async () => {
            const mockTracks = [{
                id: 1,
                title: 'Test',
                artist_name: 'Artist',
                album_name: 'Album',
                duration_ms: 180000,
                isrc: 'TEST123',
                services: 'Spotify',
                quality: '320kbps',
                download_status: 'downloaded',
                metadata_score: 80,
                lyrics_type: 'synced',
                cover_art_url: 'https://example.com/art.jpg'
            }];

            const mockPage = { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            mockInvoke((cmd) => {
                if (cmd === 'get_library') return mockPage;
                return null;
            });

            const result = await getLibrary();

            expect(result.tracks).toHaveLength(1);
            const track = result.tracks[0];
            expect(track.metadata_score).toBe(80);
            expect(track.lyrics_type).toBe('synced');
        });
    });

    describe('getPlaylists', () => {
        it('calls get_playlists command', async () => {
            mockInvoke((cmd) => {
                if (cmd === 'get_playlists') return [];
                return null;
            });

            const result = await getPlaylists();

            expect(invoke).toHaveBeenCalled();
            expect(result).toEqual([]);
        });

        it('returns playlists with track count', async () => {
            const mockPlaylists = [{
                id: 1,
                name: 'My Playlist',
                description: 'Test playlist',
                owner_name: 'User',
                track_count: 10,
                image_url: null,
                service_name: 'Spotify'
            }];

            mockInvoke((cmd) => {
                if (cmd === 'get_playlists') return mockPlaylists;
                return null;
            });

            const result = await getPlaylists();

            expect(result[0].track_count).toBe(10);
        });
    });

    describe('addToPlaylist', () => {
        it('calls add_to_playlist with correct args', async () => {
            mockInvoke((cmd, args) => {
                if (cmd === 'add_to_playlist') {
                    return 'Added 2 tracks to playlist';
                }
                return null;
            });

            const result = await addToPlaylist(1, [10, 20]);

            expect(invoke).toHaveBeenCalledWith('add_to_playlist', {
                playlistId: 1,
                trackIds: [10, 20]
            });
            expect(result).toContain('Added');
        });
    });

    describe('createPlaylist', () => {
        it('calls create_playlist and returns new id', async () => {
            mockInvoke((cmd, args) => {
                if (cmd === 'create_playlist') return 42;
                return null;
            });

            const result = await createPlaylist(1, 'New Playlist', 'Description');

            expect(invoke).toHaveBeenCalledWith('create_playlist', {
                accountId: 1,
                name: 'New Playlist',
                description: 'Description'
            });
            expect(result).toBe(42);
        });

        it('works without description', async () => {
            mockInvoke((cmd) => {
                if (cmd === 'create_playlist') return 1;
                return null;
            });

            const result = await createPlaylist(1, 'No Desc');

            expect(result).toBe(1);
        });
    });

    describe('searchTracks', () => {
        it('calls search_tracks with query', async () => {
            mockInvoke((cmd, args) => {
                if (cmd === 'search_tracks') return [];
                return null;
            });

            const result = await searchTracks('test query');

            expect(invoke).toHaveBeenCalledWith('search_tracks', { query: 'test query' });
        });
    });

    describe('enqueueTracks', () => {
        it('enqueue_handles_missing_fields_test: does not throw when excluded_preflight or skip_reasons are missing and defaults safely', async () => {
            // Simulate minimal backend response without excluded_preflight, skip_reasons, tracks
            mockInvoke((cmd) => {
                if (cmd === 'enqueue_tracks') {
                    return {
                        selected: 100,
                        eligible: 100,
                        enqueued: 100
                    };
                }
                return null;
            });

            const res = await enqueueTracks([1, 2, 3]);

            expect(res).toBeDefined();
            expect(res.selected).toBe(100);
            expect(res.eligible).toBe(100);
            expect(res.enqueued).toBe(100);
            expect(res.excluded_preflight).toBe(0);
            expect(res.skip_reasons).toEqual([]);
            expect(res.tracks).toEqual([]);
            expect(res.skipped).toBe(0);
            expect(res.deduplicated).toBe(0);
        });

        it('enqueue_shows_counts_test: correctly normalizes and reports selected, eligible, excluded, enqueued, and skip_reasons', async () => {
            mockInvoke((cmd) => {
                if (cmd === 'enqueue_tracks') {
                    return {
                        selected: 100,
                        eligible: 95,
                        enqueued: 95,
                        skipped: 5,
                        deduplicated: 0,
                        excluded_preflight: [
                            { track_id: 101, title: 'Excluded Song 1', artist: 'Artist', status: 'no_download_provider', skip_reason: 'No download provider configured' },
                            { track_id: 102, title: 'Excluded Song 2', artist: 'Artist', status: 'already_downloaded', skip_reason: 'Track is already downloaded in local library' }
                        ],
                        skip_reasons: [
                            'No download provider configured',
                            'Track is already downloaded in local library'
                        ],
                        tracks: [
                            { track_id: 1, is_eligible: true },
                            { track_id: 2, is_eligible: true }
                        ]
                    };
                }
                return null;
            });

            const res = await enqueueTracks([1, 2]);

            expect(res.selected).toBe(100);
            expect(res.eligible).toBe(95);
            expect(res.enqueued).toBe(95);
            expect(res.excluded_preflight).toBe(2);
            expect(res.skip_reasons).toEqual([
                'No download provider configured',
                'Track is already downloaded in local library'
            ]);
            expect(res.tracks).toHaveLength(2);
        });
    });

    describe('normalizeQualityPreference', () => {
        it('maps quality settings constants and raw strings to canonical DB CHECK values', () => {
            expect(normalizeQualityPreference('HI_RES_LOSSLESS')).toBe('hires');
            expect(normalizeQualityPreference('hires')).toBe('hires');
            expect(normalizeQualityPreference('HI_RES')).toBe('hires');
            expect(normalizeQualityPreference('hi-res')).toBe('hires');
            expect(normalizeQualityPreference('LOSSLESS')).toBe('lossless');
            expect(normalizeQualityPreference('lossless')).toBe('lossless');
            expect(normalizeQualityPreference('flac')).toBe('lossless');
            expect(normalizeQualityPreference('HIGH')).toBe('high');
            expect(normalizeQualityPreference('320')).toBe('high');
            expect(normalizeQualityPreference('ANY')).toBe('any');
            expect(normalizeQualityPreference('auto')).toBe('any');
            expect(normalizeQualityPreference(undefined)).toBeUndefined();
            expect(normalizeQualityPreference('')).toBeUndefined();
            expect(normalizeQualityPreference('unknown_garbage')).toBeUndefined();
        });
    });
});
