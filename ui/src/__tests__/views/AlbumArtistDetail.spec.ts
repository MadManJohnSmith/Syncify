/**
 * AlbumArtistDetail.spec.ts
 * Verifies interactive buttons in AlbumDetailView and ArtistDetailView (TASK-24):
 * - "Play Album" in AlbumDetailView plays the first track and queues the rest
 * - "Download All" in AlbumDetailView still calls add_batch_to_queue
 * - "Shuffle Play" in ArtistDetailView triggers shufflePlay -> player.play
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import AlbumDetailView from '@/views/AlbumDetailView.vue';
import ArtistDetailView from '@/views/ArtistDetailView.vue';
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

const mockRoute = {
    params: { id: '42' },
    query: {},
};

vi.mock('vue-router', () => ({
    useRoute: () => mockRoute,
    useRouter: () => ({
        push: vi.fn(),
        back: vi.fn(),
    }),
}));

describe('TASK-24: Album & Artist Detail interactive actions', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        mockRoute.params.id = '42';
    });

    const mockAlbum = {
        id: 42,
        title: 'Abbey Road',
        artist_name: 'The Beatles',
        artist_id: 10,
        release_year: 1969,
        track_count: 2,
        duration_ms: 441000,
        genre: 'Rock',
        tracks: [
            { id: 201, title: 'Come Together', duration_ms: 259000, track_number: 1, artist_name: 'The Beatles' },
            { id: 202, title: 'Something', duration_ms: 182000, track_number: 2, artist_name: 'The Beatles' },
        ],
    };

    const mockArtist = {
        id: 42,
        name: 'The Beatles',
        bio: 'Legendary rock band',
        image_url: null,
        album_count: 1,
        track_count: 2,
        albums: [
            { id: 42, title: 'Abbey Road', release_year: 1969, track_count: 2, cover_art_url: null },
        ],
        top_tracks: [
            { id: 201, title: 'Come Together', album: 'Abbey Road', duration_ms: 259000 },
            { id: 202, title: 'Something', album: 'Abbey Road', duration_ms: 182000 },
        ],
    };

    describe('AlbumDetailView - Play Album', () => {
        it('clicking "Play Album" plays the first track and queues the rest in order', async () => {
            const invokeCalls: { cmd: string; args: any }[] = [];
            mockInvoke((cmd, args) => {
                invokeCalls.push({ cmd, args });
                if (cmd === 'get_album') return { ...mockAlbum, cover_art_url: 'https://cdn.test/abbey.jpg' };
                return null;
            });

            const wrapper = mount(AlbumDetailView);
            await flushPromises();

            const playBtn = wrapper.findAll('button').find(b => b.text().includes('Play Album'));
            expect(playBtn).toBeDefined();

            await playBtn!.trigger('click');
            await flushPromises();

            expect(mockPlayerReplaceQueue).toHaveBeenCalledTimes(1);
            expect(mockPlayerReplaceQueue.mock.calls[0][0]).toEqual([
                {
                    id: 201,
                    title: 'Come Together',
                    artist: 'The Beatles',
                    album: 'Abbey Road',
                    coverUrl: 'https://cdn.test/abbey.jpg',
                },
                {
                    id: 202,
                    title: 'Something',
                    artist: 'The Beatles',
                    album: 'Abbey Road',
                    coverUrl: 'https://cdn.test/abbey.jpg',
                },
            ]);
            // Reproducir no es descargar: el botón no debe encolar descargas.
            expect(invokeCalls.find(c => c.cmd === 'add_batch_to_queue')).toBeUndefined();
        });

        it('does not play anything when album has no tracks', async () => {
            const emptyAlbum = { ...mockAlbum, tracks: [] };
            mockInvoke((cmd) => {
                if (cmd === 'get_album') return emptyAlbum;
                return null;
            });

            const wrapper = mount(AlbumDetailView);
            await flushPromises();

            const playBtn = wrapper.findAll('button').find(b => b.text().includes('Play Album'));
            expect(playBtn).toBeDefined();

            await playBtn!.trigger('click');
            await flushPromises();

            expect(mockPlayerReplaceQueue).not.toHaveBeenCalled();
            expect(mockPlayerPlay).not.toHaveBeenCalled();
        });

        it('"Download All" still calls add_batch_to_queue with track IDs', async () => {
            const invokeCalls: { cmd: string; args: any }[] = [];
            mockInvoke((cmd, args) => {
                invokeCalls.push({ cmd, args });
                if (cmd === 'get_album') return mockAlbum;
                if (cmd === 'add_batch_to_queue') return { added: 2, skipped: 0 };
                return null;
            });

            const wrapper = mount(AlbumDetailView);
            await flushPromises();

            const downloadBtn = wrapper.findAll('button').find(b => b.text().includes('Download All'));
            expect(downloadBtn).toBeDefined();

            await downloadBtn!.trigger('click');
            await flushPromises();

            const batchCall = invokeCalls.find(c => c.cmd === 'add_batch_to_queue');
            expect(batchCall).toBeDefined();
            expect(batchCall?.args).toEqual({
                trackIds: [201, 202],
                allowFallback: true,
            });
        });
    });

    describe('ArtistDetailView - Shuffle Play', () => {
        it('clicking "Shuffle Play" queues the artist tracks via replaceQueueAndPlay', async () => {
            mockInvoke((cmd) => {
                if (cmd === 'get_artist') return mockArtist;
                return null;
            });

            const wrapper = mount(ArtistDetailView);
            await flushPromises();

            const shuffleBtn = wrapper.findAll('button').find(b => b.text().includes('Shuffle Play'));
            expect(shuffleBtn).toBeDefined();

            await shuffleBtn!.trigger('click');
            await flushPromises();

            expect(mockPlayerReplaceQueue).toHaveBeenCalledTimes(1);
            const queued = mockPlayerReplaceQueue.mock.calls[0][0] as Array<{ id: number; title: string; artist: string; album: string; coverUrl: string | null }>;
            expect(queued.map(t => t.id).sort()).toEqual([201, 202]);
            for (const t of queued) {
                expect(['Come Together', 'Something']).toContain(t.title);
                expect(t.artist).toBe('The Beatles');
                expect(t.album).toBe('Abbey Road');
                expect(t.coverUrl).toBeNull();
            }
        });

        it('does not queue anything if top_tracks is empty', async () => {
            const emptyArtist = { ...mockArtist, top_tracks: [] };
            mockInvoke((cmd) => {
                if (cmd === 'get_artist') return emptyArtist;
                return null;
            });

            const wrapper = mount(ArtistDetailView);
            await flushPromises();

            const shuffleBtn = wrapper.findAll('button').find(b => b.text().includes('Shuffle Play'));
            expect(shuffleBtn).toBeDefined();

            await shuffleBtn!.trigger('click');
            await flushPromises();

            expect(mockPlayerReplaceQueue).not.toHaveBeenCalled();
        });
    });
});
