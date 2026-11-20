/**
 * FavoritesView.spec.ts — P16 (#147, spec #130): vista de Favoritos.
 *
 * Contrato:
 * - las dos rejillas cargan datos reales del backend (`get_favorites_albums`
 *   con stubs incluidos y `get_favorites_artists`), con paginación por
 *   offset/limit y el filtro de servicio viajando al backend;
 * - reproducir un álbum carga sus pistas (get_album) y las encola en orden
 *   vía replaceQueueAndPlay; encolar las añade al final con enqueue;
 * - un álbum sin tracklist expandida no llama al player: avisa;
 * - abrir ficha va a /album/:id y el artista a /artist/:id (sin top-tracks
 *   inventados: el detalle pide los suyos);
 * - estado vacío honesto por rejilla y «N+» cuando puede haber más páginas.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import FavoritesView from '@/views/FavoritesView.vue'
import { useToast } from '@/composables/useToast'
import { mockInvoke, resetMocks } from '../setup'
import type { FavoriteAlbumItem, FavoriteArtistItem } from '@/api/library'

const mockPlayerReplaceQueue = vi.fn().mockResolvedValue(undefined)
const mockPlayerEnqueue = vi.fn().mockResolvedValue('queued')
vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        replaceQueueAndPlay: mockPlayerReplaceQueue,
        enqueue: mockPlayerEnqueue,
    }),
}))

function favoriteAlbum(id: number, overrides: Partial<FavoriteAlbumItem> = {}): FavoriteAlbumItem {
    return {
        id,
        service_album_id: `svc-${id}`,
        title: `Album ${id}`,
        artist: `Artist ${id}`,
        upc: null,
        cover_art_url: null,
        service: 'spotify',
        total_tracks: 10,
        release_date: '2020-01-01',
        favorited_at: '2026-01-01T00:00:00Z',
        ...overrides,
    }
}

function favoriteArtist(id: number, overrides: Partial<FavoriteArtistItem> = {}): FavoriteArtistItem {
    return {
        id,
        service_artist_id: `svc-${id}`,
        name: `Artist ${id}`,
        image_url: null,
        service: 'tidal',
        favorited_at: '2026-01-01T00:00:00Z',
        ...overrides,
    }
}

async function mountFavorites() {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/favorites', name: 'Favorites', component: FavoritesView },
            { path: '/album/:id', name: 'AlbumDetail', component: { template: '<div data-testid="album-detail-stub" />' } },
            { path: '/artist/:id', name: 'ArtistDetail', component: { template: '<div data-testid="artist-detail-stub" />' } },
        ],
    })
    await router.push('/favorites')
    await router.isReady()
    const wrapper = mount(FavoritesView, { global: { plugins: [router] } })
    await flushPromises()
    await flushPromises()
    return { wrapper, router }
}

describe('FavoritesView — rejillas de favoritos (P16 #147)', () => {
    beforeEach(() => {
        resetMocks()
        vi.clearAllMocks()
        // El toast es un singleton de módulo: se vacía para asertar sin ruido.
        for (const t of [...useToast().toasts.value]) useToast().dismiss(t.id)
        document.body.innerHTML = ''
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_albums') return [];
            if (cmd === 'get_favorites_artists') return [];
            if (cmd === 'get_services') return [];
            return null;
        })
    })

    it('carga ambas rejillas del backend con stubs incluidos y paginación por página completa', async () => {
        const albumCalls: any[] = [];
        const artistCalls: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_favorites_albums') {
                albumCalls.push(args);
                return [favoriteAlbum(1), favoriteAlbum(2, { service: 'qobuz' })];
            }
            if (cmd === 'get_favorites_artists') {
                artistCalls.push(args);
                return [favoriteArtist(7)];
            }
            return null;
        });

        const { wrapper } = await mountFavorites();
        expect(wrapper.findAll('[data-testid="favorites-album-card"]')).toHaveLength(2);
        expect(wrapper.findAll('[data-testid="favorites-artist-card"]')).toHaveLength(1);
        expect(wrapper.find('[data-testid="favorites-total"]').text()).toContain('2 albums');
        expect(wrapper.find('[data-testid="favorites-total"]').text()).toContain('1 artists');

        // Datos reales en pantalla, no decoraciones.
        const cards = wrapper.findAll('[data-testid="favorites-album-card"]');
        expect(cards[0].text()).toContain('Album 1');
        expect(cards[0].text()).toContain('Artist 1');
        expect(cards[0].find('[data-testid="favorites-album-service"]').text()).toBe('spotify');
        expect(cards[1].find('[data-testid="favorites-album-service"]').text()).toBe('qobuz');

        // Contrato IPC: stubs incluidos, primera página, tamaño de página fijo.
        expect(albumCalls[0]).toMatchObject({ offset: 0, limit: 100, includeStubs: true });
        expect(artistCalls[0]).toMatchObject({ offset: 0, limit: 100 });
    });

    it('muestra el total de pistas y la pista de tracklist sin expandir', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_albums') {
                return [
                    favoriteAlbum(1, { total_tracks: 12 }),
                    favoriteAlbum(2, { total_tracks: null, title: 'Stub album' }),
                ];
            }
            return null;
        });

        const { wrapper } = await mountFavorites();
        const cards = wrapper.findAll('[data-testid="favorites-album-card"]');
        expect(cards[0].text()).toContain('12 tracks');
        expect(cards[1].text()).toContain('Tracklist not expanded');
    });

    it('reproducir un álbum encola sus pistas en orden vía replaceQueueAndPlay', async () => {
        mockInvoke((cmd, args) => {
            if (cmd === 'get_favorites_albums') return [favoriteAlbum(42, { title: 'Favourite', artist: 'Iron Maiden', cover_art_url: 'https://cdn.test/cover.jpg' })];
            if (cmd === 'get_album') {
                expect(args).toMatchObject({ albumId: 42 });
                return {
                    id: 42,
                    title: 'Favourite',
                    artist_name: 'Iron Maiden',
                    cover_art_url: 'https://cdn.test/cover.jpg',
                    tracks: [
                        { id: 201, title: 'Track A', artist_name: 'Iron Maiden' },
                        { id: 202, title: 'Track B', artist_name: 'Iron Maiden' },
                    ],
                };
            }
            return null;
        });

        const { wrapper } = await mountFavorites();
        await wrapper.find('[data-testid="favorites-album-play"]').trigger('click');
        await flushPromises();

        expect(mockPlayerReplaceQueue).toHaveBeenCalledTimes(1);
        expect(mockPlayerReplaceQueue.mock.calls[0][0]).toEqual([
            { id: 201, title: 'Track A', artist: 'Iron Maiden', album: 'Favourite', coverUrl: 'https://cdn.test/cover.jpg' },
            { id: 202, title: 'Track B', artist: 'Iron Maiden', album: 'Favourite', coverUrl: 'https://cdn.test/cover.jpg' },
        ]);
        // Reproducir no es descargar (eso es P17).
        expect(mockPlayerEnqueue).not.toHaveBeenCalled();
    });

    it('encolar un álbum añade sus pistas al final con enqueue, sin arrancar la descarga', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_albums') return [favoriteAlbum(9)];
            if (cmd === 'get_album') {
                return {
                    id: 9,
                    title: 'Album 9',
                    artist_name: 'Artist 9',
                    cover_art_url: null,
                    tracks: [
                        { id: 301, title: 'Track A' },
                        { id: 302, title: 'Track B' },
                        { id: 303, title: 'Track C' },
                    ],
                };
            }
            return null;
        });

        const { wrapper } = await mountFavorites();
        await wrapper.find('[data-testid="favorites-album-queue"]').trigger('click');
        await flushPromises();

        expect(mockPlayerEnqueue).toHaveBeenCalledTimes(3);
        expect(mockPlayerEnqueue.mock.calls.map((c) => c[0].id)).toEqual([301, 302, 303]);
        expect(mockPlayerReplaceQueue).not.toHaveBeenCalled();
        expect(useToast().toasts.value.some((t) => t.type === 'success' && t.title === 'Queued for playback')).toBe(true);
    });

    it('un álbum sin pistas indexadas avisa y no toca el player', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_albums') return [favoriteAlbum(5, { title: 'Stub album' })];
            if (cmd === 'get_album') return { id: 5, title: 'Stub album', artist_name: 'X', tracks: [] };
            return null;
        });

        const { wrapper } = await mountFavorites();
        await wrapper.find('[data-testid="favorites-album-play"]').trigger('click');
        await flushPromises();

        expect(mockPlayerReplaceQueue).not.toHaveBeenCalled();
        expect(mockPlayerEnqueue).not.toHaveBeenCalled();
        expect(useToast().toasts.value.some((t) => t.type === 'warning' && t.title === 'No indexed tracks')).toBe(true);
    });

    it('abre la ficha /album/:id al pulsar la tarjeta y /artist/:id en la de artista', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_albums') return [favoriteAlbum(77)];
            if (cmd === 'get_favorites_artists') return [favoriteArtist(31)];
            return null;
        });

        const { wrapper, router } = await mountFavorites();
        await wrapper.find('[data-testid="favorites-album-card"]').trigger('click');
        await flushPromises();
        expect(router.currentRoute.value.path).toBe('/album/77');
    });

    it('el artista abre su detalle, sin botones de reproducción inventados', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_artists') return [favoriteArtist(31)];
            return null;
        });

        const { wrapper, router } = await mountFavorites();
        const artistCards = wrapper.findAll('[data-testid="favorites-artist-card"]');
        expect(artistCards).toHaveLength(1);
        // La rejilla de artistas no inventa top-tracks ni play: solo abre detalle.
        expect(artistCards[0].find('button').exists()).toBe(false);

        await artistCards[0].trigger('click');
        await flushPromises();
        expect(router.currentRoute.value.path).toBe('/artist/31');
    });

    it('el filtro de servicio viaja al backend y recarga ambas rejillas desde offset 0', async () => {
        const albumCalls: any[] = [];
        const artistCalls: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_favorites_albums') {
                albumCalls.push(args);
                return albumCalls.length > 1 ? [favoriteAlbum(2, { service: 'qobuz' })] : [favoriteAlbum(1)];
            }
            if (cmd === 'get_favorites_artists') {
                artistCalls.push(args);
                return [];
            }
            if (cmd === 'get_services') {
                return [
                    { id: 1, name: 'spotify', supports_download: 1, max_quality: null },
                    { id: 2, name: 'qobuz', supports_download: 1, max_quality: null },
                ];
            }
            return null;
        });

        const { wrapper } = await mountFavorites();
        // El backend no envía display_name para servicios: la opción muestra el nombre crudo.
        expect(wrapper.find('[data-testid="favorites-filter-service"]').text()).toContain('qobuz');

        await wrapper.find('[data-testid="favorites-filter-service"]').setValue('qobuz');
        await flushPromises();

        expect(albumCalls[albumCalls.length - 1]).toMatchObject({ service: 'qobuz', offset: 0, includeStubs: true });
        expect(artistCalls[artistCalls.length - 1]).toMatchObject({ service: 'qobuz', offset: 0 });
        // El filtro aplica a ambas rejillas: la de álbumes se recargó con la página nueva.
        const cards = wrapper.findAll('[data-testid="favorites-album-card"]');
        expect(cards).toHaveLength(1);
        expect(cards[0].text()).toContain('Album 2');
    });

    it('estado vacío honesto en ambas rejillas cuando el backend no devuelve nada', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_albums') return [];
            if (cmd === 'get_favorites_artists') return [];
            return null;
        });

        const { wrapper } = await mountFavorites();
        expect(wrapper.find('[data-testid="favorites-albums-empty"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="favorites-albums-empty"]').text()).toContain('No favorite albums match this filter.');
        expect(wrapper.find('[data-testid="favorites-artists-empty"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="favorites-artists-empty"]').text()).toContain('No favorite artists match this filter.');
        expect(wrapper.find('[data-testid="favorites-album-card"]').exists()).toBe(false);
        expect(wrapper.find('[data-testid="favorites-artist-card"]').exists()).toBe(false);
    });

    it('pide la página siguiente solo cuando la página vino completa, y el contador marca el truncajo con «+»', async () => {
        const albumCalls: any[] = [];
        const fullPage = Array.from({ length: 100 }, (_, i) => favoriteAlbum(i + 1));
        mockInvoke((cmd, args) => {
            if (cmd === 'get_favorites_albums') {
                albumCalls.push(args);
                if ((args?.offset ?? 0) === 0) return fullPage;
                return [favoriteAlbum(101), favoriteAlbum(102)];
            }
            return null;
        });

        const { wrapper } = await mountFavorites();
        expect(wrapper.find('[data-testid="favorites-total"]').text()).toContain('100+ albums');
        expect(wrapper.find('[data-testid="favorites-albums-more"]').exists()).toBe(true);

        await wrapper.find('[data-testid="favorites-albums-more"]').trigger('click');
        await flushPromises();

        expect(albumCalls[1]).toMatchObject({ offset: 100, limit: 100 });
        // Página parcial: ya no hay «más» ni truncajo.
        expect(wrapper.find('[data-testid="favorites-albums-more"]').exists()).toBe(false);
        expect(wrapper.find('[data-testid="favorites-total"]').text()).toContain('102 albums');
    });

    it('muestra error por rejilla con reintento cuando el backend falla', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_favorites_albums') throw new Error('DB error: boom');
            if (cmd === 'get_favorites_artists') return [favoriteArtist(3)];
            return null;
        });

        const { wrapper } = await mountFavorites();
        expect(wrapper.find('[data-testid="favorites-albums-error"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="favorites-albums-error"]').text()).toContain('DB error: boom');
        // La otra rejilla sigue funcionando.
        expect(wrapper.findAll('[data-testid="favorites-artist-card"]')).toHaveLength(1);
    });
});
