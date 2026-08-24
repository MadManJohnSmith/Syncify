/**
 * playlists.spec.ts
 * Regression tests: playlist lists and sync counters default safely.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getPlaylists, getPlaylistTracks, syncPlaylists, syncPlaylist } from '@/api/playlists';
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
});
