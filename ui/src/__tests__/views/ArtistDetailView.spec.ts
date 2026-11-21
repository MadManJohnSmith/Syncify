/**
 * ArtistDetailView.spec.ts
 * Tests for ArtistDetailView.vue download actions and IPC payloads
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import ArtistDetailView from '@/views/ArtistDetailView.vue';
import { mockInvoke, resetMocks } from '../setup';

const mockReplaceQueueAndPlay = vi.fn().mockResolvedValue(undefined);
vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        replaceQueueAndPlay: mockReplaceQueueAndPlay,
    }),
}));

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
        tracks_total: 213,
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
            // Artista ya completo en la primera página: 2 de 2 vivas.
            if (cmd === 'get_artist') return { ...mockArtist, tracks_total: 2 };
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

    // #336: Download All encola TODA la colección viva del artista, no la
    // primera página de `top_tracks` que muestra la cabecera parcial.
    it('Download All encola todas las pistas vivas del artista, no solo top_tracks', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_artist') return { ...mockArtist, tracks_total: 7 };
            if (cmd === 'add_batch_to_queue') return { added: 7, skipped: 0 };
            if (cmd === 'query_library_artist_tracks') {
                const query = args?.query as { offset: number; limit: number };
                // Servidor: 7 pistas vivas, de una en una página de 2.
                const pageSize = 2;
                const ids = [101, 102, 103, 104, 105, 106, 107];
                const slice = ids.slice(query.offset, query.offset + pageSize);
                return {
                    tracks: slice.map(id => ({
                        id, title: `Track ${id}`, album: 'Abbey Road', albumId: 10,
                        durationMs: 200000, collaborators: [], sources: [],
                    })),
                    total: ids.length,
                    offset: query.offset,
                    limit: query.limit,
                    hasMore: query.offset + slice.length < ids.length,
                };
            }
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        await wrapper.find('[data-testid="artist-download-all"]').trigger('click');
        await flushPromises();

        const batchCall = invokeCalls.find(c => c.cmd === 'add_batch_to_queue');
        expect(batchCall?.args).toEqual({
            trackIds: [101, 102, 103, 104, 105, 106, 107],
            allowFallback: true,
            jobKind: 'artist',
        });
        // La enumeración completa pagina el MISMO query del detalle y no
        // toca la lista virtualizada (sigue con las 2 de `top_tracks`).
        const pages = invokeCalls.filter(c => c.cmd === 'query_library_artist_tracks');
        expect(pages.map(c => (c.args.query as { offset: number }).offset)).toEqual([0, 2, 4, 6]);
        expect(wrapper.findAll('[data-testid="artist-track-row"]')).toHaveLength(0);
    });

    // #336: Shuffle Play reproduce la misma colección completa.
    it('Shuffle Play reproduce todas las pistas vivas del artista', async () => {
        mockInvoke((cmd, args) => {
            if (cmd === 'get_artist') return { ...mockArtist, tracks_total: 4 };
            if (cmd === 'query_library_artist_tracks') {
                const query = args?.query as { offset: number };
                const ids = [101, 102, 103, 104];
                const slice = ids.slice(query.offset, query.offset + 200);
                return {
                    tracks: slice.map(id => ({
                        id, title: `Track ${id}`, album: 'Abbey Road', albumId: 10,
                        durationMs: 200000, collaborators: [], sources: [],
                    })),
                    total: ids.length,
                    offset: query.offset,
                    limit: 200,
                    hasMore: query.offset + slice.length < ids.length,
                };
            }
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        await wrapper.find('[data-testid="artist-shuffle-play"]').trigger('click');
        await flushPromises();

        expect(mockReplaceQueueAndPlay).toHaveBeenCalledTimes(1);
        const queued = mockReplaceQueueAndPlay.mock.calls[0][0] as { id: number }[];
        // Barajado: el conjunto es el de las 4 pistas vivas.
        expect(queued.map(t => t.id).sort((a, b) => a - b)).toEqual([101, 102, 103, 104]);
    });

    // #336: si la enumeración completa falla, la acción no encola nada y lo
    // avisa; no se encola la página parcial que sí estaba en pantalla.
    it('no encola una colección parcial si la enumeración completa falla', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_artist') return { ...mockArtist, tracks_total: 7 };
            if (cmd === 'query_library_artist_tracks') throw new Error('network down');
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        await wrapper.find('[data-testid="artist-download-all"]').trigger('click');
        await flushPromises();

        expect(invokeCalls.some(c => c.cmd === 'add_batch_to_queue')).toBe(false);
        expect(wrapper.find('[data-testid="artist-download-all"]').attributes('disabled')).toBeUndefined();
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

    it('reintenta la página siguiente tras un fallo de paginación', async () => {
        let attempts = 0;
        mockInvoke((cmd, args) => {
            if (cmd === 'get_artist') return { ...mockArtist, tracks_total: 3 };
            if (cmd === 'query_library_artist_tracks') {
                attempts += 1;
                if (attempts === 1) throw new Error('network down');
                const query = args?.query as { offset: number };
                return {
                    tracks: [{
                        id: 103,
                        title: 'Joint',
                        album: 'Abbey Road',
                        albumId: 10,
                        durationMs: 240000,
                        collaborators: [],
                        sources: [],
                    }],
                    total: 3,
                    offset: query.offset,
                    limit: 100,
                    hasMore: false,
                };
            }
            return null;
        });

        const wrapper = mount(ArtistDetailView);
        await flushPromises();

        await wrapper.findAll('button').find(b => b.text().includes('All Tracks'))!.trigger('click');
        await flushPromises();

        expect(wrapper.find('[data-testid="artist-tracks-error"]').exists()).toBe(true);
        expect(attempts).toBe(1);

        // El scroll no reintenta solo (evitaría un bucle con el error vivo).
        await wrapper.find('[data-testid="artist-tracks-scroll"]').trigger('scroll');
        await flushPromises();
        expect(attempts).toBe(1);

        // El botón explícito limpia el error y pide la página que faltaba.
        await wrapper.find('[data-testid="artist-tracks-retry"]').trigger('click');
        await flushPromises();

        expect(attempts).toBe(2);
        expect(wrapper.find('[data-testid="artist-tracks-error"]').exists()).toBe(false);
        expect(wrapper.findAll('[data-testid="artist-track-row"]')).toHaveLength(3);
    });

    it('handles SourceIdentityMissing error on artist download gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_artist') return { ...mockArtist, tracks_total: 2 };
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
