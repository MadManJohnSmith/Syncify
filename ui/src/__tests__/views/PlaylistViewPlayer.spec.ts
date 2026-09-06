/**
 * PlaylistViewPlayer.spec.ts
 * Tests for usePlayer integration in PlaylistView.vue (TASK-22)
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import PlaylistView from '@/views/PlaylistView.vue';
import { mockInvoke, resetMocks } from '../setup';

const mockPlayerPlay = vi.fn().mockResolvedValue(undefined);
vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        play: mockPlayerPlay,
    }),
}));

describe('PlaylistView Player Integration', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    const mockPlaylists = [
        { id: 1, name: 'Chill Vibes', track_count: 2, is_favorite: false, account_id: 1 }
    ];

    function makePage(tracks: any[]) {
        return { tracks, total: tracks.length, offset: 0, limit: 500, has_more: false };
    }

    const mockTracks = [
        { id: 301, title: 'Chill Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000, cover_art_url: 'http://img1.jpg' },
        { id: 302, title: 'Chill Track 2', artist_name: 'Artist 2', album_name: 'Album 2', duration_ms: 200000, cover_art_url: 'http://img2.jpg' },
    ];

    async function setupPlaylistViewWithTracks() {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return makePage(mockTracks);
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        // Select the playlist
        const playlistItem = wrapper.findAll('.playlist-item').find(el => el.text().includes('Chill Vibes'));
        expect(playlistItem).toBeDefined();
        await playlistItem!.trigger('click');
        await flushPromises();

        return wrapper;
    }

    it('playAll() invokes player.play with the first track of the playlist', async () => {
        const wrapper = await setupPlaylistViewWithTracks();

        // Find Play All button
        const playAllBtn = wrapper.findAll('button').find(b => b.text().includes('Play All'));
        expect(playAllBtn).toBeDefined();
        await playAllBtn!.trigger('click');
        await flushPromises();

        expect(mockPlayerPlay).toHaveBeenCalledTimes(1);
        expect(mockPlayerPlay).toHaveBeenCalledWith({
            id: 301,
            title: 'Chill Track 1',
            artist: 'Artist 1',
            album: 'Album 1',
            coverUrl: 'http://img1.jpg',
        });
    });

    it('clicking track row play button invokes playTrack and calls player.play', async () => {
        const wrapper = await setupPlaylistViewWithTracks();

        // Find track rows
        const trackRows = wrapper.findAll('.track-row');
        expect(trackRows.length).toBe(2);

        // Click the play button on the second track
        const track2PlayBtn = trackRows[1].find('button');
        expect(track2PlayBtn.exists()).toBe(true);
        await track2PlayBtn.trigger('click');
        await flushPromises();

        expect(mockPlayerPlay).toHaveBeenCalledTimes(1);
        expect(mockPlayerPlay).toHaveBeenCalledWith({
            id: 302,
            title: 'Chill Track 2',
            artist: 'Artist 2',
            album: 'Album 2',
            coverUrl: 'http://img2.jpg',
        });
    });

    it('shufflePlay() invokes player.play with a track from the playlist', async () => {
        const wrapper = await setupPlaylistViewWithTracks();

        // Find Shuffle button (button with title or text containing shuffle / icon)
        const buttons = wrapper.findAll('button');
        const shuffleBtn = buttons.find(b => b.html().includes('shuffle'));
        expect(shuffleBtn).toBeDefined();
        await shuffleBtn!.trigger('click');
        await flushPromises();

        expect(mockPlayerPlay).toHaveBeenCalledTimes(1);
        const calledArg = mockPlayerPlay.mock.calls[0][0];
        expect([301, 302]).toContain(calledArg.id);
    });

    it('playPlaylist() loads playlist and plays first track', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return makePage(mockTracks);
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        // Click play button on playlist item in sidebar
        const sidebarItem = wrapper.findAll('.playlist-item').find(el => el.text().includes('Chill Vibes'));
        expect(sidebarItem).toBeDefined();
        const playlistPlayBtn = sidebarItem!.find('button');
        expect(playlistPlayBtn.exists()).toBe(true);
        await playlistPlayBtn.trigger('click');
        await flushPromises();

        expect(mockPlayerPlay).toHaveBeenCalledTimes(1);
        expect(mockPlayerPlay).toHaveBeenCalledWith({
            id: 301,
            title: 'Chill Track 1',
            artist: 'Artist 1',
            album: 'Album 1',
            coverUrl: 'http://img1.jpg',
        });
    });
});
