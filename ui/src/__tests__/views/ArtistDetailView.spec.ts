/**
 * ArtistDetailView.spec.ts
 * Tests for ArtistDetailView.vue download actions and IPC payloads
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import ArtistDetailView from '@/views/ArtistDetailView.vue';
import { mockInvoke, resetMocks } from '../setup';

vi.mock('vue-router', () => ({
    useRoute: () => ({
        params: { id: '42' },
        query: {},
    }),
    useRouter: () => ({
        push: vi.fn(),
        back: vi.fn(),
    }),
}));

describe('ArtistDetailView', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    const mockArtist = {
        id: 42,
        name: 'The Beatles',
        bio: 'Legendary rock band',
        image_url: null,
        album_count: 12,
        track_count: 213,
        albums: [
            { id: 10, title: 'Abbey Road', release_year: 1969, track_count: 17, cover_art_url: null }
        ],
        top_tracks: [
            { id: 101, title: 'Come Together', album: 'Abbey Road', duration_ms: 259000 },
            { id: 102, title: 'Something', album: 'Abbey Road', duration_ms: 182000 }
        ]
    };

    it('renders artist information', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_artist') return mockArtist;
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        expect(wrapper.text()).toContain('The Beatles');
        expect(wrapper.text()).toContain('Abbey Road');
    });

    it('enqueues all artist top tracks with add_batch_to_queue', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_artist') return mockArtist;
            if (cmd === 'add_batch_to_queue') return { added: 2, skipped: 0 };
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        const downloadAllBtn = wrapper.findAll('button').find(b => b.text().includes('Download All'));
        expect(downloadAllBtn).toBeDefined();
        await downloadAllBtn!.trigger('click');
        await flushPromises();

        const batchCall = invokeCalls.find(c => c.cmd === 'add_batch_to_queue');
        expect(batchCall).toBeDefined();
        expect(batchCall?.args).toEqual({
            trackIds: [101, 102],
            allowFallback: true,
            jobKind: 'artist',
        });
    });

    it('enqueues single track download with add_to_queue', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_artist') return mockArtist;
            if (cmd === 'add_to_queue') return 1;
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        // Switch to tracks tab
        const tracksTab = wrapper.findAll('button').find(b => b.text().includes('All Tracks'));
        if (tracksTab) {
            await tracksTab.trigger('click');
            await flushPromises();
        }

        const trackDownloadBtn = wrapper.find('button[title="Download Track"]');
        expect(trackDownloadBtn.exists()).toBe(true);
        await trackDownloadBtn.trigger('click');
        await flushPromises();

        const addCall = invokeCalls.find(c => c.cmd === 'add_to_queue');
        expect(addCall).toBeDefined();
        expect(addCall?.args).toEqual({
            trackId: 101,
            targetTitle: 'Come Together',
            targetArtist: 'The Beatles',
            allowFallback: true,
        });
    });

    it('pagina todas las pistas del artista y muestra colaboradores y fuentes', async () => {
        const artistWithMoreTracks = {
            ...mockArtist,
            tracks_total: 7,
        };
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_artist') return artistWithMoreTracks;
            if (cmd === 'query_library_artist_tracks') {
                const query = args?.query as { offset: number };
                queries.push(query);
                const tracks = query.offset === 2
                    ? [{
                        id: 103,
                        title: 'Joint',
                        album: 'Abbey Road',
                        albumId: 10,
                        durationMs: 240000,
                        collaborators: ['Guest Vocalist'],
                        sources: [
                            { serviceId: 2, serviceName: 'qobuz' },
                            { serviceId: 1, serviceName: 'spotify' },
                        ],
                    }]
                    : [{
                        id: 104,
                        title: 'Later',
                        album: 'Abbey Road',
                        albumId: 10,
                        durationMs: 200000,
                        collaborators: [],
                        sources: [{ serviceId: 1, serviceName: 'spotify' }],
                    }];
                return {
                    tracks,
                    total: 7,
                    offset: query.offset,
                    limit: 100,
                    hasMore: query.offset + tracks.length < 7,
                };
            }
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        const tracksTab = wrapper.findAll('button').find(b => b.text().includes('All Tracks'));
        expect(tracksTab?.text()).toContain('7');
        await tracksTab!.trigger('click');
        await flushPromises();

        // Primera página = top_tracks (2 de 7). Al abrir la pestaña se pide
        // UNA página más (5 filas no desbordan el viewport y no habría scroll
        // para pedir el resto); no se cargan las 7 de golpe.
        expect(queries).toHaveLength(1);
        expect(queries[0]).toEqual({ artistId: 42, offset: 2, limit: 100 });
        let rows = wrapper.findAll('[data-testid="artist-track-row"]');
        expect(rows).toHaveLength(3);
        const joint = rows.find((row) => row.attributes('data-track-id') === '103');
        expect(joint?.text()).toContain('feat. Guest Vocalist');
        expect(joint?.find('[data-testid="artist-track-sources"]').text()).toContain('qobuz');
        expect(joint?.find('[data-testid="artist-track-sources"]').text()).toContain('spotify');

        // El scroll sigue paginando en servidor: offset de lo ya cargado.
        await wrapper.find('[data-testid="artist-tracks-scroll"]').trigger('scroll');
        await flushPromises();
        await flushPromises();

        expect(queries[queries.length - 1]).toEqual({ artistId: 42, offset: 3, limit: 100 });
        rows = wrapper.findAll('[data-testid="artist-track-row"]');
        expect(rows).toHaveLength(4);
    });

    it('handles SourceIdentityMissing error on artist download gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_artist') return mockArtist;
            if (cmd === 'add_batch_to_queue') {
                throw new Error('SourceIdentityMissing: tracks have no streaming source');
            }
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        const downloadAllBtn = wrapper.findAll('button').find(b => b.text().includes('Download All'));
        await downloadAllBtn!.trigger('click');
        await flushPromises();

        expect(wrapper.exists()).toBe(true);
    });
});
