/**
 * LibraryArtistsView.spec.ts — L08 (#105, spec #96): pestaña de artistas.
 *
 * Contrato:
 * - una fila por artista con pistas, álbumes, colaboraciones, cuentas y
 *   servicios;
 * - filtros de texto, cuenta y servicio viajan al servidor;
 * - orden por nombre/pistas/fecha añadida en el servidor;
 * - paginación en servidor (offset 0 y 100) y lista virtualizada (no se
 *   renderizan todos los artistas cargados);
 * - un click abre el detalle /artist/:id.
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import LibraryArtistsView from '@/views/LibraryArtistsView.vue'
import { mockInvoke, resetMocks } from '../setup'
import type { LibraryArtistRow } from '@/api/types'

function artistRow(id: number, overrides: Partial<LibraryArtistRow> = {}): LibraryArtistRow {
    return {
        artistId: id,
        name: `Artist ${id}`,
        imageUrl: null,
        trackCount: 8,
        albumCount: 3,
        collaborationCount: 1,
        accounts: [
            { accountId: 1, displayName: 'A1', serviceName: 'spotify' },
        ],
        services: [
            { serviceId: 1, serviceName: 'spotify' },
            { serviceId: 2, serviceName: 'qobuz' },
        ],
        dateAdded: '2026-01-01T00:00:00Z',
        ...overrides,
    }
}

function artistsPage(artists: LibraryArtistRow[], total = artists.length, hasMore = false) {
    return {
        artists,
        total,
        offset: 0,
        limit: 100,
        hasMore,
    }
}

async function mountArtists(query: Record<string, string> = {}) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/library/artists', name: 'LibraryArtists', component: LibraryArtistsView },
            { path: '/artist/:id', name: 'ArtistDetail', component: { template: '<div data-testid="artist-detail-stub" />' } },
        ],
    })
    await router.push({ path: '/library/artists', query })
    await router.isReady()
    const wrapper = mount(LibraryArtistsView, { global: { plugins: [router] } })
    await flushPromises()
    await flushPromises()
    return { wrapper, router }
}

describe('LibraryArtistsView — pestaña de artistas (L08 #105)', () => {
    beforeEach(() => {
        resetMocks()
        document.body.innerHTML = ''
    })

    it('muestra el total del servidor y una fila por artista con sus badges', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'query_library_artists') {
                return artistsPage([
                    artistRow(1, { trackCount: 8, albumCount: 3, collaborationCount: 1 }),
                    artistRow(2, {
                        name: 'Solo Act',
                        trackCount: 1,
                        albumCount: 1,
                        collaborationCount: 0,
                        accounts: [],
                        services: [],
                    }),
                ], 2);
            }
            if (cmd === 'get_accounts' || cmd === 'get_services') return [];
            return null;
        });

        const { wrapper } = await mountArtists();
        expect(wrapper.find('[data-testid="library-artists-total"]').text()).toContain('2 Artists');
        const rows = wrapper.findAll('[data-testid="artist-row"]');
        expect(rows).toHaveLength(2);
        expect(rows[0].text()).toContain('Artist 1');
        expect(rows[0].text()).toContain('8 tracks');
        expect(rows[0].text()).toContain('3 albums');
        expect(rows[0].text()).toContain('1 collaboration');
        expect(rows[0].find('[data-testid="artist-accounts"]').text()).toContain('A1');
        expect(rows[0].find('[data-testid="artist-services"]').text()).toContain('qobuz');
        // Sin pertenencias no se inventa una cuenta.
        expect(rows[1].find('[data-testid="artist-accounts"]').text()).toBe('');
    });

    it('envía texto, cuenta y servicio al servidor', async () => {
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_artists') {
                queries.push(args?.query);
                return artistsPage([artistRow(1)]);
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

        const { wrapper } = await mountArtists();
        await wrapper.find('[data-testid="artists-filter-account"]').setValue('2');
        await flushPromises();
        await wrapper.find('[data-testid="artists-filter-service"]').setValue('1');
        await flushPromises();
        await wrapper.find('[data-testid="artists-filter-text"]').setValue('alpha');
        await new Promise((resolve) => setTimeout(resolve, 300));
        await flushPromises();

        const last = queries[queries.length - 1];
        expect(last.text).toBe('alpha');
        expect(last.accountIds).toEqual([2]);
        expect(last.serviceIds).toEqual([1]);
        // Primera página del servidor desde offset 0.
        expect(last.offset).toBe(0);
        expect(last.limit).toBe(100);
    });

    it('ordena en el servidor por nombre, pistas y fecha añadida', async () => {
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_artists') {
                queries.push(args?.query);
                return artistsPage([artistRow(1)]);
            }
            return null;
        });

        const { wrapper } = await mountArtists();
        // Por defecto: nombre A→Z.
        expect(queries[0].sort).toBe('name');
        expect(queries[0].direction).toBe('asc');

        await wrapper.find('[data-testid="artists-sort"]').setValue('tracks');
        await flushPromises();
        expect(queries[queries.length - 1].sort).toBe('tracks');

        await wrapper.find('[data-testid="artists-direction"]').trigger('click');
        await flushPromises();
        expect(queries[queries.length - 1].sort).toBe('tracks');
        expect(queries[queries.length - 1].direction).toBe('desc');

        await wrapper.find('[data-testid="artists-sort"]').setValue('dateAdded');
        await flushPromises();
        expect(queries[queries.length - 1].sort).toBe('dateAdded');
    });

    it('pagina en servidor y no renderiza todos los artistas cargados', async () => {
        const queries: any[] = [];
        const firstPage = Array.from({ length: 100 }, (_, index) => artistRow(index + 1));
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_artists') {
                const query = args?.query as { offset: number };
                queries.push(query);
                if (query.offset === 0) return artistsPage(firstPage, 103, true);
                return artistsPage([artistRow(101), artistRow(102), artistRow(103)], 103, false);
            }
            return null;
        });

        const { wrapper } = await mountArtists();
        expect(queries[0].offset).toBe(0);
        // Virtualización: la ventana inicial renderiza una porción, no 100.
        const rendered = wrapper.findAll('[data-testid="artist-row"]').length;
        expect(rendered).toBeGreaterThan(0);
        expect(rendered).toBeLessThan(100);

        await wrapper.find('[data-testid="artists-scroll"]').trigger('scroll');
        await flushPromises();
        await flushPromises();

        expect(queries.some((query) => query.offset === 100)).toBe(true);
        expect(wrapper.find('[data-testid="library-artists-total"]').text()).toContain('103 Artists');
    });

    it('abre /artist/:id al hacer click en una fila', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'query_library_artists') return artistsPage([artistRow(42)]);
            return null;
        });

        const { wrapper, router } = await mountArtists();
        await wrapper.find('[data-testid="artist-row"]').trigger('click');
        await flushPromises();
        expect(router.currentRoute.value.path).toBe('/artist/42');
    });

    it('lee el texto y el orden compartidos desde la URL y normaliza el no soportado', async () => {
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_artists') {
                queries.push(args?.query);
                return artistsPage([]);
            }
            return null;
        });

        await mountArtists({ q: 'amo', sort: 'tracks', dir: 'desc' });
        expect(queries[0].text).toBe('amo');
        expect(queries[0].sort).toBe('tracks');
        expect(queries[0].direction).toBe('desc');
    });

    it('normaliza un sort de pistas que no aplica y conserva la dirección', async () => {
        const queries: any[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library_artists') {
                queries.push(args?.query);
                return artistsPage([]);
            }
            return null;
        });

        await mountArtists({ sort: 'title', dir: 'desc' });
        expect(queries[0].sort).toBe('name');
        expect(queries[0].direction).toBe('desc');
    });
});
