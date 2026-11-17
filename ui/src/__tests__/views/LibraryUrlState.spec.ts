/**
 * LibraryUrlState.spec.ts — L03 (#100, spec #96): el estado de la vista de
 * biblioteca vive en la URL.
 *
 * Historia 12 del spec: texto, servicios, cuentas, colecciones, orden,
 * dirección, descarga, calidad, «en playlist» y cuarentena en la URL. En el
 * armazón de L03 la URL lleva las dimensiones que la vista ya dirige hoy
 * (texto, orden, dirección, descarga, calidad, favoritos); las pendientes
 * (L00/#97, L09/#72, L11/#77) NO se escriben ni se aplican: un deep-link
 * hacia ellas se reporta como unsupported, no se simula.
 *
 * Atrás restaura el filtro: el estado se re-aplica desde la URL al volver.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import LibraryView from '@/views/LibraryView.vue'
import { mockInvoke, resetMocks } from '../setup'
import type { LibraryTrack } from '@/api/types'

import {
    LIBRARY_URL_DEFAULTS,
    buildLibraryUrlQuery,
    libraryTabQuery,
    parseLibraryUrlQuery,
} from '@/composables/libraryUrlState'

function createTestTrack(id: number, title: string): LibraryTrack {
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
        download_status: 'not_downloaded',
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

describe('libraryUrlState — parse y build (módulo puro)', () => {
    it('el query vacío es el estado por defecto', () => {
        const { state, unsupported, legacyQualityLabel } = parseLibraryUrlQuery({});
        expect(state).toEqual(LIBRARY_URL_DEFAULTS);
        expect(unsupported).toEqual([]);
        expect(legacyQualityLabel).toBeNull();
        expect(buildLibraryUrlQuery(LIBRARY_URL_DEFAULTS)).toEqual({});
    });

    it('parsea las claves gestionadas y descarta valores inválidos', () => {
        const { state } = parseLibraryUrlQuery({
            q: 'daft punk',
            sort: 'title',
            dir: 'asc',
            dl: 'downloaded,queued',
            quality: 'lossy',
            fav: '1',
        });
        expect(state).toEqual({
            text: 'daft punk',
            sort: 'title',
            direction: 'asc',
            downloadStatuses: ['downloaded', 'queued'],
            qualityBuckets: ['lossy'],
            favoritesOnly: true,
        });
        expect(buildLibraryUrlQuery(state)).toEqual({
            q: 'daft punk',
            sort: 'title',
            dir: 'asc',
            dl: 'downloaded,queued',
            quality: 'lossy',
            fav: '1',
        });
    });

    it('un valor de orden o dirección desconocido cae al default (un enlace a mano no rompe la vista)', () => {
        const { state } = parseLibraryUrlQuery({ sort: 'bogus', dir: 'sideways', dl: 'downloaded,bogus' });
        expect(state.sort).toBe('dateAdded');
        expect(state.direction).toBe('desc');
        expect(state.downloadStatuses).toEqual(['downloaded']);
    });

    it('acepta el deep-link legacy FE-10 del dashboard (etiqueta de calidad)', () => {
        const { state, legacyQualityLabel } = parseLibraryUrlQuery({
            filter: 'quality',
            quality: 'Hi-Res (24-bit+)',
        });
        expect(state.qualityBuckets).toEqual(['hi_res']);
        expect(legacyQualityLabel).toBe('Hi-Res (24-bit+)');
    });

    it('acepta también ids de bucket en la clave quality', () => {
        const { state, legacyQualityLabel } = parseLibraryUrlQuery({ quality: 'cd_quality' });
        expect(state.qualityBuckets).toEqual(['cd_quality']);
        expect(legacyQualityLabel).toBeNull();
    });

    it('reporta las dimensiones pendientes como unsupported sin aplicarlas', () => {
        const { unsupported, state } = parseLibraryUrlQuery({
            quarantine: 'only',
            duplicates: 'only',
            collections: 'album',
            inPlaylist: '1',
            services: '1',
            accounts: '2',
            filter: 'duplicates',
        });
        expect(unsupported).toEqual(expect.arrayContaining(['quarantine', 'duplicates', 'collections', 'inPlaylist', 'services', 'accounts', 'filter']));
        expect('quarantine' in state).toBe(false);
    });

    it('el deep-link legacy filter=<pill> se mapea a la dimensión equivalente', () => {
        const downloaded = parseLibraryUrlQuery({ filter: 'downloaded' });
        expect(downloaded.state.downloadStatuses).toEqual(['downloaded']);
        const favorites = parseLibraryUrlQuery({ filter: 'favorites' });
        expect(favorites.state.favoritesOnly).toBe(true);
        const flac = parseLibraryUrlQuery({ filter: 'flac' });
        expect(flac.state.qualityBuckets).toEqual(['hi_res', 'cd_quality']);
    });

    it('normaliza el orden de los buckets para que el round-trip sea estable', () => {
        const { state } = parseLibraryUrlQuery({ quality: 'lossy,cd_quality,hi_res' });
        expect(state.qualityBuckets).toEqual(['hi_res', 'cd_quality', 'lossy']);
        expect(buildLibraryUrlQuery(state).quality).toBe('hi_res,cd_quality,lossy');
    });
});

describe('libraryTabQuery — cambio de pestaña conserva solo lo compatible', () => {
    it('conserva texto y orden; descarta las dimensiones de pista y el legacy', () => {
        const kept = libraryTabQuery('albums', {
            q: 'amo',
            sort: 'title',
            dir: 'asc',
            dl: 'downloaded',
            fav: '1',
            filter: 'quality',
            quality: 'Lossy',
            track: '42',
        });
        expect(kept).toEqual({ q: 'amo', sort: 'title', dir: 'asc' });
    });

    it('con estado por defecto no lleva nada', () => {
        expect(libraryTabQuery('artists', {})).toEqual({});
        expect(libraryTabQuery('duplicates', { dl: 'downloaded', fav: '1' })).toEqual({});
    });
});

describe('LibraryView — estado en la URL (integración)', () => {
    beforeEach(() => {
        resetMocks();
        document.body.innerHTML = '';
    });

    it('un enlace directo restaura texto, orden, dirección y filtros en la consulta', async () => {
        const queries: Record<string, unknown>[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as Record<string, unknown>);
                return { tracks: [createTestTrack(1, 'One')], total: 1, offset: 0, limit: 100, has_more: false };
            }
            return null;
        });

        await mountLibrary({ q: 'daft', sort: 'title', dir: 'asc', dl: 'downloaded', quality: 'lossy', fav: '1' });

        const first = queries[0] as Record<string, unknown>;
        expect(first.text).toBe('daft');
        expect(first.sort).toBe('title');
        expect(first.direction).toBe('asc');
        expect(first.downloadStatuses).toEqual(['downloaded']);
        expect(first.qualityBuckets).toEqual(['lossy']);
        expect(first.favoritesOnly).toBe(true);
    });

    it('las dimensiones pendientes de un deep-link no llegan a la consulta', async () => {
        const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
        const queries: Record<string, unknown>[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as Record<string, unknown>);
                return { tracks: [], total: 0, offset: 0, limit: 100, has_more: false };
            }
            return null;
        });

        await mountLibrary({ quarantine: 'only', duplicates: 'only' });

        const first = queries[0] as Record<string, unknown>;
        expect(first.quarantine).toBe('exclude');
        expect(first.duplicates).toBeNull();
        expect(warn).toHaveBeenCalled();
        warn.mockRestore();
    });

    it('aplicar un filtro escribe la URL y atrás restaura el estado anterior', async () => {
        const queries: Record<string, unknown>[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as Record<string, unknown>);
                return { tracks: [createTestTrack(1, 'One')], total: 1, offset: 0, limit: 100, has_more: false };
            }
            return null;
        });

        const { wrapper, router } = await mountLibrary();

        await wrapper.find('[data-testid="library-chip-downloaded"]').trigger('click');
        await flushPromises();
        await flushPromises();
        expect(router.currentRoute.value.query.dl).toBe('downloaded');

        // Atrás: la URL vuelve y el estado se re-aplica (sin la dimensión).
        await router.back();
        await flushPromises();
        await flushPromises();
        expect(router.currentRoute.value.query.dl).toBeUndefined();
        const last = queries[queries.length - 1] as Record<string, unknown>;
        expect(last.downloadStatuses).toEqual([]);
    });

    it('escribir texto actualiza la URL con replace (una entrada, no una por tecla)', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'query_library') {
                return { tracks: [createTestTrack(1, 'One')], total: 1, offset: 0, limit: 100, has_more: false };
            }
            return null;
        });

        const { wrapper, router } = await mountLibrary();

        await wrapper.find('input[type="text"]').setValue('daft');
        // El debounce de la búsqueda (250 ms) es el que escribe la URL.
        await new Promise((resolve) => setTimeout(resolve, 300));
        await flushPromises();
        expect(router.currentRoute.value.query.q).toBe('daft');
    });
});
