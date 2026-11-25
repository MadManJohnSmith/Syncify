/**
 * PlaylistViewPlayer.spec.ts
 * Tests for usePlayer integration in PlaylistView.vue (TASK-22)
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import PlaylistView from '@/views/PlaylistView.vue';
import { mockInvoke, resetMocks } from '../setup';

const mockPlayerPlay = vi.fn().mockResolvedValue(undefined);
const mockPlayerPlayNext = vi.fn().mockResolvedValue('queued');
const mockPlayerReplaceQueue = vi.fn().mockResolvedValue(undefined);
vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        play: mockPlayerPlay,
        playNext: mockPlayerPlayNext,
        replaceQueueAndPlay: mockPlayerReplaceQueue,
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

    it('playAll() plays the first track and queues the rest via replaceQueueAndPlay', async () => {
        const wrapper = await setupPlaylistViewWithTracks();

        // Find Play All button
        const playAllBtn = wrapper.findAll('button').find(b => b.text().includes('Play All'));
        expect(playAllBtn).toBeDefined();
        await playAllBtn!.trigger('click');
        await flushPromises();

        expect(mockPlayerReplaceQueue).toHaveBeenCalledTimes(1);
        expect(mockPlayerReplaceQueue).toHaveBeenCalledWith([
            {
                id: 301,
                title: 'Chill Track 1',
                artist: 'Artist 1',
                album: 'Album 1',
                coverUrl: 'http://img1.jpg',
            },
            {
                id: 302,
                title: 'Chill Track 2',
                artist: 'Artist 2',
                album: 'Album 2',
                coverUrl: 'http://img2.jpg',
            },
        ]);
    });

    it('playAll() preserves playlist order in the queue (fase 1 corrige la inversión de playNext)', async () => {
        const wrapper = await setupPlaylistViewWithTracks();

        const playAllBtn = wrapper.findAll('button').find(b => b.text().includes('Play All'));
        await playAllBtn!.trigger('click');
        await flushPromises();

        const queuedIds = mockPlayerReplaceQueue.mock.calls[0][0].map((t: { id: number }) => t.id);
        expect(queuedIds).toEqual([301, 302]);
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

    it('playPlaylist() loads the playlist and queues it in order via replaceQueueAndPlay', async () => {
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

        expect(mockPlayerReplaceQueue).toHaveBeenCalledTimes(1);
        const queuedIds = mockPlayerReplaceQueue.mock.calls[0][0].map((t: { id: number }) => t.id);
        expect(queuedIds).toEqual([301, 302]);
    });

    it('playAll() loads every page before queueing a large playlist', async () => {
        const tracks = Array.from({ length: 250 }, (_, index) => ({
            ...mockTracks[0],
            id: index === 99 ? 7 : index + 1,
            position: index + 1,
            title: `Track ${index + 1}`,
        }));
        const offsets: number[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                const offset = Number(args?.offset ?? 0);
                if (Number(args?.limit) === 100) offsets.push(offset);
                const pageTracks = tracks.slice(offset, offset + 100);
                return {
                    tracks: pageTracks,
                    total: tracks.length,
                    offset,
                    limit: 100,
                    has_more: offset + pageTracks.length < tracks.length,
                };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.playlist-item').find(el => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();

        const playAllBtn = wrapper.findAll('button').find(b => b.text().includes('Play All'));
        await playAllBtn!.trigger('click');
        await flushPromises();

        expect(offsets).toEqual([0, 0, 100, 200]);
        expect(mockPlayerReplaceQueue).toHaveBeenCalledTimes(1);
        const queuedIds = mockPlayerReplaceQueue.mock.calls[0][0].map((track: { id: number }) => track.id);
        expect(queuedIds).toEqual(tracks.map(track => track.id));
    });

    it('ignores stale Play All work after changing playlist and deduplicates repeated clicks', async () => {
        let resolveOldPage!: (value: any) => void;
        const calls: { cmd: string; args: any }[] = [];
        const playlists = [
            { id: 1, name: 'First Playlist', track_count: 200 },
            { id: 2, name: 'Second Playlist', track_count: 1 },
        ];
        const firstPage = Array.from({ length: 100 }, (_, index) => ({
            ...mockTracks[0],
            id: index + 1,
            position: index + 1,
            title: `First ${index + 1}`,
        }));
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'get_playlists') return playlists;
            if (cmd === 'get_local_playlist_tracks') {
                const playlistId = Number(args?.playlistId);
                const offset = Number(args?.offset ?? 0);
                if (playlistId === 1 && offset === 100) {
                    return new Promise(resolve => { resolveOldPage = resolve; });
                }
                if (playlistId === 2) {
                    return {
                        tracks: [{ ...mockTracks[0], id: 201, position: 1, title: 'Second only' }],
                        total: 1,
                        offset: 0,
                        limit: 100,
                        has_more: false,
                    };
                }
                return { tracks: firstPage, total: 200, offset, limit: 100, has_more: true };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.playlist-item').find(el => el.text().includes('First Playlist'))!.trigger('click');
        await flushPromises();

        const firstPlayAll = wrapper.findAll('button').find(b => b.text().includes('Play All'))!;
        await firstPlayAll.trigger('click');
        await firstPlayAll.trigger('click');
        await flushPromises();

        expect(calls.filter(call => call.cmd === 'get_local_playlist_tracks' && call.args.playlistId === 1 && call.args.offset === 0 && call.args.limit === 100)).toHaveLength(2);
        expect(wrapper.findAll('button').find(b => b.text().includes('Loading…'))?.attributes('disabled')).toBeDefined();

        await wrapper.findAll('.playlist-item').find(el => el.text().includes('Second Playlist'))!.trigger('click');
        await flushPromises();
        resolveOldPage({
            tracks: [{ ...mockTracks[0], id: 101, position: 101, title: 'Stale tail' }],
            total: 200,
            offset: 100,
            limit: 100,
            has_more: false,
        });
        await flushPromises();

        expect(mockPlayerReplaceQueue).not.toHaveBeenCalled();
        const secondPlayAll = wrapper.findAll('button').find(b => b.text().includes('Play All'))!;
        expect(secondPlayAll.attributes('disabled')).toBeUndefined();
        await secondPlayAll.trigger('click');
        await flushPromises();

        expect(mockPlayerReplaceQueue).toHaveBeenCalledTimes(1);
        expect(mockPlayerReplaceQueue.mock.calls[0][0].map((track: { id: number }) => track.id)).toEqual([201]);
    });
});
