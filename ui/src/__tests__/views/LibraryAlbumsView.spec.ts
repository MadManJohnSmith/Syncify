/**
 * LibraryAlbumsView.spec.ts — L07 (#104, spec #96): pestaña de álbumes.
 *
 * Contrato:
 * - una fila por álbum con artista/año, pistas, descargadas, cobertura,
 *   cuentas y servicios;
 * - la cobertura nula (total cero) se muestra como '—', nunca como 0 %;
 * - filtros de texto, cuenta, servicio y cobertura viajan al servidor;
 * - orden por título/artista/año/fecha añadida en el servidor;
 * - paginación en servidor (offset 0 y 100) y lista virtualizada (no se
 *   renderizan todos los álbumes cargados);
 * - un click abre el detalle /album/:id.
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import LibraryAlbumsView from '@/views/LibraryAlbumsView.vue'
import { mockInvoke, resetMocks } from '../setup'
import type { LibraryAlbumRow } from '@/api/types'

function albumRow(id: number, overrides: Partial<LibraryAlbumRow> = {}): LibraryAlbumRow {
    return {
        albumId: id,
        title: `Album ${id}`,
        artistName: `Artist ${id}`,
        releaseYear: 1999,
        trackCount: 10,
        downloadedCount: 4,
        coverage: 0.4,
        accounts: [
            { accountId: 1, displayName: 'A1', serviceName: 'spotify' },
        ],
        services: [
            { serviceId: 1, serviceName: 'spotify' },
            { serviceId: 2, serviceName: 'qobuz' },
        ],
        dateAdded: '2026-01-01T00:00:00Z',
        coverArtUrl: null,
        ...overrides,
    }
}

function albumsPage(albums: LibraryAlbumRow[], total = albums.length, hasMore = false) {
    return {
        albums,
        total,
        offset: 0,
        limit: 100,
        hasMore,
    }
}

async function mountAlbums(query: Record<string, string> = {}) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/library/albums', name: 'LibraryAlbums', component: LibraryAlbumsView },
            { path: '/album/:id', name: 'AlbumDetail', component: { template: '<div data-testid="album-detail-stub" />' } },
        ],
    })
    await router.push({ path: '/library/albums', query })
    await router.isReady()
    const wrapper = mount(LibraryAlbumsView, { global: { plugins: [router] } })
    await flushPromises()
    await flushPromises()
    return { wrapper, router }
}

describe('LibraryAlbumsView — pestaña de álbumes (L07 #104)', () => {
    beforeEach(() => {
        resetMocks()
        document.body.innerHTML = ''
    })

    it('muestra el total del servidor y una fila por álbum con sus badges y cobertura', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'query_library_albums') {
                return albumsPage([
                    albumRow(1, { coverage: 0.4, downloadedCount: 4, trackCount: 10 }),
                    albumRow(2, {
                        title: 'No downloads',
                        downloadedCount: 0,
                        coverage: 0,
                        accounts: [],
                        services: [],
                    }),
                ], 2);
            }
            if (cmd === 'get_accounts' || cmd === 'get_services') return [];
            return null;
        });

        const { wrapper } = await mountAlbums();
        expect(wrapper.find('[data-testid="library-albums-total"]').text()).toContain('2 Albums');
        const rows = wrapper.findAll('[data-testid="album-row"]');
        expect(rows).toHaveLength(2);
        expect(rows[0].text()).toContain('Album 1');
        expect(rows[0].text()).toContain('Artist 1');
        expect(rows[0].text()).toContain('1999');
        expect(rows[0].text()).toContain('10 tracks');
        expect(rows[0].text()).toContain('4 downloaded');
        expect(rows[0].text()).toContain('40% downloaded');
        expect(rows[0].find('[data-testid="album-accounts"]').text()).toContain('A1');
        expect(rows[0].find('[data-testid="album-services"]').text()).toContain('qobuz');
        // Cero descargadas es un 0 % honesto (total > 0).
        expect(rows[1].text()).toContain('0% downloaded');
        // Sin pertenencias no se inventa una cuenta.
        expect(rows[1].find('[data-testid="album-accounts"]').text()).toBe('');
    });

    it('no inventa porcentaje cuando el total de pistas es cero', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'query_library_albums') {
                return albumsPage([albumRow(1, { trackCount: 0, downloadedCount: 0, coverage: null })], 1);
            }
            return null;
        });

        const { wrapper } = await mountAlbums();
        const coverage = wrapper.find('[data-testid="album-coverage"]');
        expect(coverage.text()).toContain('—');
        expect(coverage.text()).not.toContain('0%');
    });

    it('envía texto, cuenta, servicio y cobertura al servidor', async () => {
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_albums') {
                queries.push(args?.query);
                return albumsPage([albumRow(1)]);
            }
            if (cmd === 'get_accounts') {
                return [{ id: 2, display_name: 'A2', service_name: 'qobuz', email: null, is_active: true }];
            }
            if (cmd === 'get_services') {
                return [
                    { id: 1, name: 'spotify', supports_download: 1, max_quality: null },
                    { id: 2, name: 'qobuz', supports_download: 1, max_quality: null },
                ];
            }
            return null;
        });

        const { wrapper } = await mountAlbums();
        await wrapper.find('[data-testid="albums-filter-account"]').setValue('2');
        await flushPromises();
        await wrapper.find('[data-testid="albums-filter-service"]').setValue('1');
        await flushPromises();
        await wrapper.find('[data-testid="albums-filter-coverage"]').setValue('none');
        await flushPromises();
        await wrapper.find('[data-testid="albums-filter-text"]').setValue('nightfall');
        await new Promise((resolve) => setTimeout(resolve, 300));
        await flushPromises();

        const last = queries[queries.length - 1];
        expect(last.text).toBe('nightfall');
        expect(last.accountIds).toEqual([2]);
        expect(last.serviceIds).toEqual([1]);
        expect(last.coverage).toEqual(['none']);
        // Primera página del servidor desde offset 0.
        expect(last.offset).toBe(0);
        expect(last.limit).toBe(100);
    });

    it('ordena en el servidor por título/artista/año/fecha añadida', async () => {
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_albums') {
                queries.push(args?.query);
                return albumsPage([albumRow(1)]);
            }
            return null;
        });

        const { wrapper } = await mountAlbums();
        await wrapper.find('[data-testid="albums-sort"]').setValue('year');
        await flushPromises();
        expect(queries[queries.length - 1].sort).toBe('year');
        expect(queries[queries.length - 1].direction).toBe('desc');

        await wrapper.find('[data-testid="albums-direction"]').trigger('click');
        await flushPromises();
        expect(queries[queries.length - 1].sort).toBe('year');
        expect(queries[queries.length - 1].direction).toBe('asc');
    });

    it('pagina en servidor y no renderiza todos los álbumes cargados', async () => {
        const queries: any[] = [];
        const firstPage = Array.from({ length: 100 }, (_, index) => albumRow(index + 1));
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_albums') {
                const query = args?.query as { offset: number };
                queries.push(query);
                if (query.offset === 0) return albumsPage(firstPage, 103, true);
                return albumsPage([albumRow(101), albumRow(102), albumRow(103)], 103, false);
            }
            return null;
        });

        const { wrapper } = await mountAlbums();
        expect(queries[0].offset).toBe(0);
        // Virtualización: la ventana inicial renderiza una porción, no 100.
        const rendered = wrapper.findAll('[data-testid="album-row"]').length;
        expect(rendered).toBeGreaterThan(0);
        expect(rendered).toBeLessThan(100);

        await wrapper.find('[data-testid="albums-scroll"]').trigger('scroll');
        await flushPromises();
        await flushPromises();

        expect(queries.some((query) => query.offset === 100)).toBe(true);
        expect(wrapper.find('[data-testid="library-albums-total"]').text()).toContain('103 Albums');
    });

    it('abre /album/:id al hacer click en una fila', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'query_library_albums') return albumsPage([albumRow(42)]);
            return null;
        });

        const { wrapper, router } = await mountAlbums();
        await wrapper.find('[data-testid="album-row"]').trigger('click');
        await flushPromises();
        expect(router.currentRoute.value.path).toBe('/album/42');
    });

    it('lee el texto y el orden compartidos desde la URL', async () => {
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_albums') {
                queries.push(args?.query);
                return albumsPage([]);
            }
            return null;
        });

        await mountAlbums({ q: 'amo', sort: 'artist', dir: 'asc' });
        expect(queries[0].text).toBe('amo');
        expect(queries[0].sort).toBe('artist');
        expect(queries[0].direction).toBe('asc');
    });
});
