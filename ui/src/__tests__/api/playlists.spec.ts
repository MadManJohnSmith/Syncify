/**
 * playlists.spec.ts
 * Regression tests: playlist lists and sync counters default safely.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getPlaylists,
    getPlaylistTracks,
    syncPlaylists,
    syncPlaylist,
    searchPlaylists,
    createPlaylist,
    addTracksToPlaylist,
    removeTracksFromPlaylist,
    reorderPlaylistTracks,
    importPlaylists,
} from '@/api/playlists';
import { mockInvoke, resetMocks } from '../setup';

describe('playlists_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('coerces non-array list responses to empty arrays', async () => {
        mockInvoke(() => null);
        expect(await getPlaylists()).toEqual([]);
        expect(await getPlaylistTracks(1)).toEqual([]);
    });

    it('defaults SyncPlaylistsResult counters and message', async () => {
        mockInvoke((cmd) => (cmd === 'sync_playlists' ? { playlists_synced: 4, tracks_linked: 40, message: 'ok' } : null));

        const res = await syncPlaylists('tidal');
        expect(res.playlists_synced).toBe(4);
        expect(res.tracks_linked).toBe(40);
        expect(res.message).toBe('ok');

        mockInvoke((cmd) => (cmd === 'sync_playlists' ? { playlists_synced: 2 } : null));
        const partial = await syncPlaylists();
        expect(partial.playlists_synced).toBe(2);
        expect(partial.tracks_linked).toBe(0);
        expect(partial.message).toBe('');
    });

    it('normalizes sync_playlist ImportResult on partial payload', async () => {
        mockInvoke((cmd) => (cmd === 'sync_playlist' ? null : null));

        const res = await syncPlaylist(9);
        expect(res.imported).toBe(0);
        expect(res.skipped).toBe(0);
        expect(res.errors).toEqual([]);
    });

    it('searchPlaylists filters playlists by query matching name or description', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') {
                return [
                    { id: 1, name: 'Rock Classics', description: 'Best 70s and 80s' },
                    { id: 2, name: 'Jazz Chill', description: 'Relaxing jazz vibes' },
                    { id: 3, name: 'Electronic', description: 'Classics and modern synth' },
                ];
            }
            return null;
        });

        const all = await searchPlaylists('');
        expect(all).toHaveLength(3);

        const rock = await searchPlaylists('rock');
        expect(rock).toHaveLength(1);
        expect(rock[0].name).toBe('Rock Classics');

        const classics = await searchPlaylists('classics');
        expect(classics).toHaveLength(2);
    });

    it('createPlaylist invokes create_playlist and retrieves resulting playlist', async () => {
        let createdArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'create_playlist') {
                createdArgs = args as Record<string, unknown>;
                return 42;
            }
            if (cmd === 'get_playlist') {
                return {
                    id: (args as { id: number }).id,
                    name: 'My New Playlist',
                    description: 'Test Desc',
                    track_count: 0,
                    owner_name: null,
                    image_url: null,
                    service_name: 'local',
                };
            }
            return null;
        });

        const playlist = await createPlaylist({ name: 'My New Playlist', description: 'Test Desc' });
        expect(createdArgs).toEqual({ accountId: 1, name: 'My New Playlist', description: 'Test Desc' });
        expect(playlist.id).toBe(42);
        expect(playlist.name).toBe('My New Playlist');
    });

    it('addTracksToPlaylist invokes add_to_playlist with camelCase params', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'add_to_playlist') {
                passedArgs = args as Record<string, unknown>;
                return 'Added 2 tracks';
            }
            return null;
        });

        const count = await addTracksToPlaylist(10, [101, 102]);
        expect(count).toBe(2);
        expect(passedArgs).toEqual({ playlistId: 10, trackIds: [101, 102] });
    });

    it('removeTracksFromPlaylist invokes remove_from_playlist with camelCase params', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'remove_from_playlist') {
                passedArgs = args as Record<string, unknown>;
                return 1;
            }
            return null;
        });

        const count = await removeTracksFromPlaylist(10, [101]);
        expect(count).toBe(1);
        expect(passedArgs).toEqual({ playlistId: 10, trackIds: [101] });
    });

    it('reorderPlaylistTracks passes positions with camelCase fields', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'reorder_playlist_tracks') {
                passedArgs = args as Record<string, unknown>;
                return null;
            }
            return null;
        });

        await reorderPlaylistTracks(5, [{ trackId: 1, newPosition: 2 }]);
        expect(passedArgs).toEqual({
            playlistId: 5,
            positions: [{ trackId: 1, newPosition: 2 }],
        });
    });

    it('importPlaylists routes to specific commands for spotify and qobuz', async () => {
        const calledCmds: string[] = [];
        mockInvoke((cmd) => {
            calledCmds.push(cmd);
            return { imported: 5, skipped: 1, errors: [] };
        });

        const spotRes = await importPlaylists('spotify');
        expect(spotRes.imported).toBe(5);
        expect(calledCmds).toContain('import_spotify_playlists');

        const qobRes = await importPlaylists('QOBUZ');
        expect(qobRes.imported).toBe(5);
        expect(calledCmds).toContain('import_qobuz_playlists');

        const otherRes = await importPlaylists('tidal');
        expect(otherRes.imported).toBe(5);
        expect(calledCmds).toContain('import_playlists');
    });
});
