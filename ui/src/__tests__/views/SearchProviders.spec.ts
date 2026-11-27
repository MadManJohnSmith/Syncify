/**
 * SearchProviders.spec.ts — P10b (#409): pestaña Proveedores de SearchView.
 *
 * Contrato:
 * - los resultados se agrupan por proveedor y por tipo, y un fallo parcial
 *   queda visible sin ocultar las secciones sanas;
 * - reproducir/encolar una pista usa una entrada efímera (`service_id` +
 *   `service_track_id`), nunca un `tracks.id` de biblioteca;
 * - la ficha de álbum pide sus pistas y la de artista sus álbumes al gateway;
 * - un fallo completo de la búsqueda se pinta como error, no como «sin
 *   resultados».
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import SearchView from '@/views/SearchView.vue';
import { useToast } from '@/composables/useToast';
import { mockInvoke, resetMocks } from '../setup';
import type {
    CatalogAlbum,
    CatalogArtist,
    CatalogSearchResponse,
    CatalogSection,
    CatalogTrack,
} from '@/api/types/catalogSearch';

vi.mock('vue-router', () => ({
    useRouter: () => ({ push: vi.fn() }),
}));

const mockPlayerPlay = vi.fn().mockResolvedValue(undefined);
const mockPlayProviderItem = vi.fn().mockResolvedValue(undefined);
const mockEnqueueProvider = vi.fn().mockResolvedValue('queued');
vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        play: mockPlayerPlay,
        playProviderItem: mockPlayProviderItem,
        enqueueProvider: mockEnqueueProvider,
    }),
}));

const discoveryTrack: CatalogTrack = {
    id: 'q-track-1',
    title: 'One More Time',
    artist: 'Daft Punk',
    album: 'Discovery',
    isrc: 'GBDUW0000059',
    duration_ms: 320_000,
    bit_depth: 24,
    sample_rate: 96,
};

const homeworkTrack: CatalogTrack = {
    ...discoveryTrack,
    id: 'q-track-2',
    title: 'Around the World',
    album: 'Homework',
};

const discoveryAlbum: CatalogAlbum = {
    id: 'q-alb-1',
    title: 'Discovery',
    artist: 'Daft Punk',
    track_count: 14,
    released_at: 981_158_400,
    cover_url: 'https://example.invalid/discovery.jpg',
};

const daftPunk: CatalogArtist = {
    id: 'q-art-1',
    name: 'Daft Punk',
    image_url: null,
    album_count: 8,
};

function page<T>(items: T[]): CatalogSection<T> {
    return { page: { items, total: items.length, offset: 0, limit: 20 } };
}

function searchResponse(): CatalogSearchResponse {
    return {
        query: 'daft punk',
        page: { offset: 0, limit: 20 },
        providers: [
            {
                provider: 'qobuz',
                service_id: 2,
                account_id: 7,
                tracks: page([discoveryTrack, homeworkTrack]),
                albums: page([discoveryAlbum]),
                artists: page([daftPunk]),
            },
            {
                provider: 'tidal',
                service_id: 3,
                account_id: null,
                tracks: page([] as CatalogTrack[]),
                albums: {
                    error: {
                        provider: 'tidal',
                        kind: 'auth_expired',
                        message: 'Tidal session expired',
                    },
                },
                artists: page([] as CatalogArtist[]),
            },
        ],
    };
}

const albumTracks: CatalogSection<CatalogTrack> = {
    page: { items: [discoveryTrack, homeworkTrack], total: null, offset: 0, limit: 100 },
};

const artistAlbums: CatalogSection<CatalogAlbum> = {
    page: { items: [discoveryAlbum], total: null, offset: 0, limit: 100 },
};

type Call = { cmd: string; args: Record<string, unknown> | undefined };

async function openProviders(query: string, calls: Call[] = []) {
    mockInvoke((cmd, args) => {
        calls.push({ cmd, args });
        if (cmd === 'catalog_search') return searchResponse();
        if (cmd === 'catalog_album_tracks') return albumTracks;
        if (cmd === 'catalog_artist_albums') return artistAlbums;
        return null;
    });

    const wrapper = mount(SearchView);
    await flushPromises();

    const tab = document.body.querySelector('[data-testid="search-tab-providers"]') as HTMLButtonElement;
    expect(tab).not.toBeNull();
    tab.click();
    await flushPromises();

    const input = document.body.querySelector('[data-testid="search-modal-input"]') as HTMLInputElement;
    input.value = query;
    input.dispatchEvent(new Event('input'));
    await new Promise(resolve => setTimeout(resolve, 600));
    await flushPromises();
    await flushPromises();

    return wrapper;
}

function first(testId: string): HTMLElement | null {
    return document.body.querySelector(`[data-testid="${testId}"]`) as HTMLElement | null;
}

function all(testId: string): HTMLElement[] {
    return Array.from(document.body.querySelectorAll(`[data-testid="${testId}"]`)) as HTMLElement[];
}

describe('SearchView — pestaña Proveedores (P10b #409)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        for (const toast of [...useToast().toasts.value]) useToast().dismiss(toast.id);
        document.body.innerHTML = '';
    });

    afterEach(() => {
        document.body.innerHTML = '';
    });

    it('agrupa por proveedor y tipo y muestra el fallo parcial sin ocultar lo sano', async () => {
        await openProviders('daft punk');

        const providers = all('catalog-provider');
        expect(providers.map(provider => provider.getAttribute('data-provider'))).toEqual([
            'qobuz',
            'tidal',
        ]);

        const qobuz = document.body.querySelector('[data-provider="qobuz"]') as HTMLElement;
        expect(qobuz.querySelector('[data-testid="catalog-provider-status"]')?.textContent).toContain('connected');
        expect(qobuz.querySelector('[data-testid="catalog-section-tracks"]')?.textContent).toContain('One More Time');
        expect(qobuz.querySelector('[data-testid="catalog-section-albums"]')?.textContent).toContain('Discovery');
        expect(qobuz.querySelector('[data-testid="catalog-section-artists"]')?.textContent).toContain('Daft Punk');

        const tidal = document.body.querySelector('[data-provider="tidal"]') as HTMLElement;
        expect(tidal.textContent).not.toContain('No results');
        expect(tidal.querySelector('[data-testid="catalog-provider-status"]')?.textContent).toContain('expired');
        expect(tidal.querySelector('[data-testid="catalog-section-error"]')?.textContent).toContain('Session expired');
        expect(tidal.querySelector('[data-testid="catalog-section-error"]')?.textContent).toContain('Tidal session expired');

        // El proveedor sano sigue mostrando sus pistas aunque el otro falle.
        expect(all('catalog-track').some(row => row.textContent?.includes('One More Time'))).toBe(true);
    });

    it('reproduce un resultado como entrada efímera, sin id de biblioteca', async () => {
        await openProviders('daft punk');

        first('catalog-play-track')!.click();
        await flushPromises();

        expect(mockPlayProviderItem).toHaveBeenCalledTimes(1);
        expect(mockPlayProviderItem).toHaveBeenCalledWith({
            serviceId: 2,
            serviceTrackId: 'q-track-1',
            title: 'One More Time',
            artist: 'Daft Punk',
            album: 'Discovery',
            coverUrl: null,
        });
        expect(mockPlayerPlay).not.toHaveBeenCalled();
    });

    it('encola un resultado en la cola de reproducción del proveedor', async () => {
        await openProviders('daft punk');

        all('catalog-queue-track')[0].click();
        await flushPromises();

        expect(mockEnqueueProvider).toHaveBeenCalledWith({
            serviceId: 2,
            serviceTrackId: 'q-track-1',
            title: 'One More Time',
            artist: 'Daft Punk',
            album: 'Discovery',
            coverUrl: null,
        });
    });

    it('abre la ficha de álbum con sus pistas y reproduce desde ella', async () => {
        const calls: Call[] = [];
        await openProviders('daft punk', calls);

        first('catalog-album')!.click();
        await flushPromises();
        await flushPromises();

        const albumCall = calls.find(call => call.cmd === 'catalog_album_tracks');
        expect(albumCall?.args).toEqual({
            provider: 'qobuz',
            title: 'Discovery',
            artist: 'Daft Punk',
        });
        expect(first('catalog-album-title')?.textContent).toContain('Discovery');
        expect(all('catalog-detail-track')).toHaveLength(2);

        first('catalog-detail-play')!.click();
        await flushPromises();

        expect(mockPlayProviderItem).toHaveBeenLastCalledWith({
            serviceId: 2,
            serviceTrackId: 'q-track-1',
            title: 'One More Time',
            artist: 'Daft Punk',
            album: 'Discovery',
            coverUrl: 'https://example.invalid/discovery.jpg',
        });
    });

    it('abre la ficha de artista con sus álbumes y navega al álbum', async () => {
        const calls: Call[] = [];
        await openProviders('daft punk', calls);

        first('catalog-artist')!.click();
        await flushPromises();
        await flushPromises();

        const artistCall = calls.find(call => call.cmd === 'catalog_artist_albums');
        expect(artistCall?.args).toEqual({ provider: 'qobuz', name: 'Daft Punk' });
        expect(first('catalog-artist-title')?.textContent).toContain('Daft Punk');
        expect(all('catalog-detail-album')).toHaveLength(1);

        first('catalog-detail-album')!.click();
        await flushPromises();
        await flushPromises();

        expect(calls.filter(call => call.cmd === 'catalog_album_tracks')).toHaveLength(1);
        expect(first('catalog-album-title')?.textContent).toContain('Discovery');
    });

    it('muestra un fallo completo como error y permite reintentar', async () => {
        let attempts = 0;
        mockInvoke((cmd) => {
            if (cmd === 'catalog_search') {
                attempts += 1;
                if (attempts === 1) throw new Error('Catalog gateway unavailable');
                return searchResponse();
            }
            if (cmd === 'catalog_album_tracks') return albumTracks;
            if (cmd === 'catalog_artist_albums') return artistAlbums;
            return null;
        });

        const wrapper = mount(SearchView);
        await flushPromises();
        (first('search-tab-providers') as HTMLButtonElement).click();
        await flushPromises();
        const input = first('search-modal-input') as HTMLInputElement;
        input.value = 'daft punk';
        input.dispatchEvent(new Event('input'));
        await new Promise(resolve => setTimeout(resolve, 600));
        await flushPromises();

        expect(first('catalog-search-error')?.textContent).toContain('Catalog gateway unavailable');
        expect(first('catalog-no-results')).toBeNull();

        first('catalog-retry')!.click();
        await flushPromises();
        await flushPromises();

        expect(attempts).toBe(2);
        expect(first('catalog-search-error')).toBeNull();
        expect(all('catalog-provider')).toHaveLength(2);
        expect(wrapper.exists()).toBe(true);
    });
});
