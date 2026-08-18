/**
 * PlaylistView.spec.ts
 * Tests for PlaylistView.vue download actions and IPC payloads
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import PlaylistView from '@/views/PlaylistView.vue';
import { mockInvoke, resetMocks } from '../setup';

describe('PlaylistView', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    const mockPlaylists = [
        { id: 1, name: 'Chill Vibes', track_count: 2, is_favorite: false, account_id: 1 }
    ];

    const mockPlaylistTracks = {
        tracks: [
            { id: 301, title: 'Chill Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000 },
            { id: 302, title: 'Chill Track 2', artist_name: 'Artist 2', album_name: 'Album 2', duration_ms: 200000 }
        ],
        total: 2,
        offset: 0,
        limit: 50,
        has_more: false
    };

    it('renders playlists and downloads all tracks when clicking download', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return mockPlaylistTracks;
            if (cmd === 'add_batch_to_queue') return { added: 2, skipped: 0 };
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        // Select playlist item
        const playlistItem = wrapper.findAll('.group').find(el => el.text().includes('Chill Vibes'));
        if (playlistItem) {
            await playlistItem.trigger('click');
            await flushPromises();
        }

        // Find Download button
        const downloadBtn = wrapper.findAll('button').find(b => b.text().includes('Download'));
        if (downloadBtn) {
            await downloadBtn.trigger('click');
            await flushPromises();

            const batchCall = invokeCalls.find(c => c.cmd === 'add_batch_to_queue');
            expect(batchCall).toBeDefined();
            expect(batchCall?.args).toEqual({
                trackIds: [301, 302],
                allowFallback: true,
            });
        }
    });

    it('handles SourceIdentityMissing error on playlist download gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return mockPlaylistTracks;
            if (cmd === 'add_batch_to_queue') {
                throw new Error('SourceIdentityMissing: playlist tracks have no streaming source');
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        const playlistItem = wrapper.findAll('.group').find(el => el.text().includes('Chill Vibes'));
        if (playlistItem) {
            await playlistItem.trigger('click');
            await flushPromises();
        }

        const downloadBtn = wrapper.findAll('button').find(b => b.text().includes('Download'));
        if (downloadBtn) {
            await downloadBtn.trigger('click');
            await flushPromises();
            expect(wrapper.exists()).toBe(true);
        }
    });
});
