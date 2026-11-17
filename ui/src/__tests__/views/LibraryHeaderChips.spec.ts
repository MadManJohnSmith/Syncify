/**
 * LibraryHeaderChips.spec.ts — L03 (#100, spec #96): cabecera con 5 chips
 * de estado sobre la biblioteca.
 *
 * Contrato:
 * - cinco chips: Álbumes, Artistas, Descargadas, Cuarentena, Duplicados;
 * - Descargadas aplica y retira su filtro (descarga), y su contador es el
 *   que L01 pide con la primera página (calculado en servidor SIN la
 *   condición del propio chip, para que el número coincida con el total al
 *   aplicarlo);
 * - los chips cuya dependencia no existe (#97 pertenencia de álbum/artista,
 *   #72 cuarentena, #73 duplicados) van DESHABILITADOS con su motivo y su
 *   contador no se inventa ('—');
 * - los contadores se conservan al paginar (no se recalculan con scroll).
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import LibraryView from '@/views/LibraryView.vue'
import { mockInvoke, resetMocks } from '../setup'
import type { LibraryTrack } from '@/api/types'

function createTestTrack(id: number, title: string, download_status: LibraryTrack['download_status'] = 'not_downloaded'): LibraryTrack {
    return {
        id,
        title,
        artist_name: 'Artist',
        artist_id: null,
        album_name: 'Album',
        album_id: null,
        duration_ms: 180000,
        isrc: `TEST${id}`,
        services: 'Spotify',
        quality: '320kbps',
        download_status,
        metadata_score: 80,
        lyrics_type: 'none',
        cover_art_url: null,
        track_number: null,
        disc_number: null,
        genre: null,
        bpm: null,
        musical_key: null,
        release_year: null,
        explicit: null,
        file_path: null,
        musicbrainz_id: null,
    };
}

function pageWithCounts(tracks: LibraryTrack[], counts: Record<string, unknown>) {
    return { tracks, total: tracks.length, offset: 0, limit: 100, has_more: false, counts };
}

async function mountLibrary(query: Record<string, string> = {}) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/library', name: 'Library', component: LibraryView }],
    })
    await router.push({ path: '/library', query })
    await router.isReady()
    const wrapper = mount(LibraryView, { global: { plugins: [router] } })
    await flushPromises()
    await flushPromises()
    return { wrapper, router }
}

describe('LibraryHeaderChips — cinco chips de estado (L03 #100)', () => {
    beforeEach(() => {
        resetMocks()
        document.body.innerHTML = ''
    })

    it('renderiza los cinco chips con el total y los contadores de L01', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'query_library') {
                return pageWithCounts([createTestTrack(1, 'One')], {
                    total: 31,
                    favorites: 8,
                    downloaded: 12,
                    inPlaylists: 5,
                    byService: [],
                    byAccount: [],
                });
            }
            return null;
        });

        const { wrapper } = await mountLibrary();

        expect(wrapper.find('[data-testid="library-chips"]').exists()).toBe(true);
        for (const id of ['albums', 'artists', 'downloaded', 'quarantine', 'duplicates']) {
            expect(wrapper.find(`[data-testid="library-chip-${id}"]`).exists()).toBe(true);
        }
        // El contador del chip Descargadas sale de los contadores de la
        // primera página (guardados por L01), no de la cola de descargas.
        const downloaded = wrapper.find('[data-testid="library-chip-downloaded"]');
        expect(downloaded.text()).toContain('Downloaded');
        expect(downloaded.text()).toContain('12');
    });

    it('el contador del chip se calcula sin su propia condición y coincide con el total al aplicarlo', async () => {
        // Servidor: 2 pistas; SIN el filtro de descarga hay 12 descargadas.
        // Al aplicar el chip la lista pasa a 2 y el chip DEBE seguir
        // mostrando 12 (contador sin su propia dimensión), no 2.
        const all = [createTestTrack(1, 'One', 'downloaded'), createTestTrack(2, 'Two', 'downloaded')];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                const q = (args?.query ?? {}) as { offset?: number; downloadStatuses?: string[] };
                const filtered = q.downloadStatuses?.length
                    ? all.filter((t) => q.downloadStatuses!.includes(t.download_status as string))
                    : all;
                if ((q.offset ?? 0) === 0) {
                    return {
                        tracks: filtered,
                        total: filtered.length,
                        offset: 0,
                        limit: 100,
                        has_more: false,
                        counts: { total: all.length, favorites: 0, downloaded: 12, inPlaylists: 0, byService: [], byAccount: [] },
                    };
                }
                return { tracks: filtered, total: filtered.length, offset: q.offset ?? 0, limit: 100, has_more: false };
            }
            return null;
        });

        const { wrapper } = await mountLibrary();
        const chip = () => wrapper.find('[data-testid="library-chip-downloaded"]');
        expect(chip().text()).toContain('12');

        // Aplicar el chip: la consulta lleva la dimensión y el total de la
        // cabecera cambia, pero el contador del chip no se recalcula.
        await chip().trigger('click');
        await flushPromises();
        await flushPromises();
        expect(chip().attributes('aria-pressed')).toBe('true');
        expect(chip().text()).toContain('12');
        expect(wrapper.text()).toContain('2 Tracks');

        // Retirarlo: vuelve el filtro fuera y el contador sigue siendo el
        // del servidor sin la condición.
        await chip().trigger('click');
        await flushPromises();
        await flushPromises();
        expect(chip().attributes('aria-pressed')).toBe('false');
        expect(chip().text()).toContain('12');
        expect(wrapper.text()).toContain('2 Tracks');
    });

    it('los chips sin dependencia van deshabilitados con su motivo y no alteran la consulta', async () => {
        const queries: Record<string, unknown>[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as Record<string, unknown>);
                return pageWithCounts([createTestTrack(1, 'One')], {
                    total: 1, favorites: 0, downloaded: 0, inPlaylists: 0, byService: [], byAccount: [],
                });
            }
            return null;
        });

        const { wrapper, router } = await mountLibrary();
        const callsBefore = queries.length;

        // Cuarentena (#72/L09) y duplicados (#73/L10): deshabilitados con
        // motivo, contador no inventado.
        const quarantine = wrapper.find('[data-testid="library-chip-quarantine"]');
        const duplicates = wrapper.find('[data-testid="library-chip-duplicates"]');
        const albums = wrapper.find('[data-testid="library-chip-albums"]');
        const artists = wrapper.find('[data-testid="library-chip-artists"]');
        for (const chip of [quarantine, duplicates, albums, artists]) {
            expect(chip.attributes('disabled')).toBeDefined();
            expect(chip.attributes('title')).toBeTruthy();
            expect(chip.text()).toContain('—');
        }
        expect(quarantine.attributes('title')).toContain('#72');
        expect(duplicates.attributes('title')).toContain('#73');
        // La pertenencia de álbum/artista es de L00 (#97).
        expect(albums.attributes('title')).toContain('#97');
        expect(artists.attributes('title')).toContain('#97');

        // Un click en un chip deshabilitado no filtra ni navega.
        await quarantine.trigger('click');
        await duplicates.trigger('click');
        await flushPromises();
        expect(queries.length).toBe(callsBefore);
        expect(router.currentRoute.value.query).toEqual({});
    });

    it('el contador del chip aparece como — hasta que la primera página trae contadores', async () => {
        // Página sin `counts` (contrato: solo offset 0 los trae): el chip no
        // muestra un cero inventado.
        mockInvoke((cmd) => {
            if (cmd === 'query_library') {
                return { tracks: [createTestTrack(1, 'One')], total: 1, offset: 0, limit: 100, has_more: false };
            }
            return null;
        });
        const { wrapper } = await mountLibrary();
        const counts = wrapper.findAll('[data-testid="library-chip-count"]');
        expect(counts.length).toBe(5);
        for (const el of counts) {
            expect(el.text()).toBe('—');
        }
    });

    it('el chip Descargadas escribe la dimensión en la consulta y en la URL', async () => {
        const queries: Record<string, unknown>[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as Record<string, unknown>);
                return pageWithCounts([createTestTrack(1, 'One', 'downloaded')], {
                    total: 1, favorites: 0, downloaded: 1, inPlaylists: 0, byService: [], byAccount: [],
                });
            }
            return null;
        });

        const { wrapper, router } = await mountLibrary();

        await wrapper.find('[data-testid="library-chip-downloaded"]').trigger('click');
        await flushPromises();
        await flushPromises();

        const last = queries[queries.length - 1] as { downloadStatuses?: string[] };
        expect(last.downloadStatuses).toEqual(['downloaded']);
        expect(router.currentRoute.value.query.dl).toBe('downloaded');

        await wrapper.find('[data-testid="library-chip-downloaded"]').trigger('click');
        await flushPromises();
        await flushPromises();
        const last2 = queries[queries.length - 1] as { downloadStatuses?: string[] };
        expect(last2.downloadStatuses).toEqual([]);
        expect(router.currentRoute.value.query.dl).toBeUndefined();
    });
});
