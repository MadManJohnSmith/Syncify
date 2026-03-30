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
    searchTracks
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
});
