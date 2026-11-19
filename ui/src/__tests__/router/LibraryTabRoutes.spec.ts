/**
 * LibraryTabRoutes.spec.ts — L03 (#100, spec #96): armazón de pestañas de
 * la biblioteca.
 *
 * - /library sigue siendo la vista de pistas;
 * - /library/albums ya es la lista real de L07 (#104); /library/artists y
 *   /library/duplicates muestran un placeholder deshabilitado CON SU
 *   MOTIVO (L08/L10) — el contenido no se simula;
 * - el cambio de pestaña conserva los filtros compatibles (texto y orden)
 *   y descarta los que no aplican;
 * - Playlists y descargas conservan sus vistas y rutas actuales.
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory, RouterView } from 'vue-router'
import { defineComponent, h } from 'vue'
import { routes } from '@/main'
import { mockInvoke, resetMocks } from '../setup'
import type { LibraryTrack } from '@/api/types'

function createTestTrack(id: number): LibraryTrack {
    return {
        id,
        title: `Track ${id}`,
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

function libraryHandler() {
    return (cmd: string) => {
        if (cmd === 'query_library') {
            return { tracks: [createTestTrack(1)], total: 1, offset: 0, limit: 100, has_more: false };
        }
        if (cmd === 'query_library_albums') {
            return { albums: [], total: 0, offset: 0, limit: 100, hasMore: false };
        }
        return [];
    };
}

/** Host mínimo con <router-view>: el wiring de rutas es el real (`routes`
 * de main.ts) sin el shell de App — su <transition mode="out-in"> no
 * completa en jsdom y bloquearía el intercambio de vista tras navegar. */
const RouterHost = defineComponent({
    name: 'RouterHost',
    setup: () => () => h('div', [h(RouterView)]),
})

async function mountAt(path: string) {
    const router = createRouter({ history: createMemoryHistory(), routes })
    await router.push(path)
    await router.isReady()
    const wrapper = mount(RouterHost, { global: { plugins: [router] } })
    await flushPromises()
    await flushPromises()
    return { wrapper, router }
}

/** La navegación con componentes asíncronos necesita más vueltas del loop
 * de eventos de las que da flushPromises: se espera activamente al path. */
async function waitForNavigation(router: ReturnType<typeof createRouter>, path: string) {
    for (let i = 0; i < 100; i++) {
        if (router.currentRoute.value.path === path) {
            await flushPromises()
            await flushPromises()
            return
        }
        await new Promise((resolve) => setTimeout(resolve, 20))
    }
    throw new Error(`la navegación a ${path} no completó`)
}

describe('LibraryTabRoutes — armazón de pestañas (L03 #100)', () => {
    beforeEach(() => {
        resetMocks()
        mockInvoke(libraryHandler())
        document.body.innerHTML = ''
    })

    it('las tres rutas de pestañas están registradas y /library sigue siendo la vista de pistas', () => {
        const paths = routes.map((r) => r.path);
        expect(paths).toContain('/library');
        expect(paths).toContain('/library/albums');
        expect(paths).toContain('/library/artists');
        expect(paths).toContain('/library/duplicates');
        const library = routes.find((r) => r.path === '/library') as { name?: string };
        expect(library.name).toBe('Library');
    });

    it('las pestañas sin carril muestran un placeholder con su motivo y sin contenido simulado', async () => {
        const expectations: Array<[string, string]> = [
            ['/library/artists', 'L08 (#105)'],
            ['/library/duplicates', 'L10 (#73)'],
        ];
        for (const [path, reason] of expectations) {
            const { wrapper } = await mountAt(path);
            const placeholder = wrapper.find('[data-testid="library-tab-placeholder"]');
            expect(placeholder.exists()).toBe(true);
            expect(placeholder.attributes('data-tab')).toBe(path.replace('/library/', ''));
            expect(wrapper.find('[data-testid="library-tab-reason"]').text()).toContain(reason);
            // El armazón no inventa listas: no hay filas de pista.
            expect(wrapper.findAll('.track-row').length).toBe(0);
            wrapper.unmount();
        }
    });

    it('/library/albums es la lista real de álbumes (L07), no el placeholder', async () => {
        const { wrapper } = await mountAt('/library/albums');
        expect(wrapper.find('[data-testid="library-tab-placeholder"]').exists()).toBe(false);
        expect(wrapper.find('[data-testid="albums-filters"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="library-albums-total"]').exists()).toBe(true);
    });

    it('la barra de pestañas marca la activa y enlaza las cuatro rutas', async () => {
        const { wrapper } = await mountAt('/library/albums');
        const tabs = wrapper.find('[data-testid="library-tabs"]');
        expect(tabs.exists()).toBe(true);
        expect(tabs.find('[data-testid="library-tab-albums"]').attributes('aria-current')).toBe('page');
        expect(tabs.find('[data-testid="library-tab-tracks"]').attributes('aria-current')).toBeUndefined();
        for (const id of ['tracks', 'albums', 'artists', 'duplicates']) {
            expect(tabs.find(`[data-testid="library-tab-${id}"]`).exists()).toBe(true);
        }
    });

    it('volver a pistas desde el placeholder restaura la vista de biblioteca', async () => {
        const { wrapper, router } = await mountAt('/library/duplicates');
        await wrapper.find('[data-testid="library-tab-back"]').trigger('click');
        await waitForNavigation(router, '/library');
        expect(router.currentRoute.value.path).toBe('/library');
        expect(wrapper.find('[data-testid="library-tabs"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="library-chips"]').exists()).toBe(true);
    });

    it('cambiar a una pestaña de armazón conserva el texto y descarta las dimensiones de pista', async () => {
        const { wrapper, router } = await mountAt('/library?q=amo&sort=title&dir=asc&dl=downloaded&fav=1');
        await wrapper.find('[data-testid="library-tab-albums"]').trigger('click');
        await waitForNavigation(router, '/library/albums');

        expect(router.currentRoute.value.path).toBe('/library/albums');
        // Compatibles: texto y orden. No compatibles con la lista de
        // álbumes: estado de descarga y favoritos.
        expect(router.currentRoute.value.query.q).toBe('amo');
        expect(router.currentRoute.value.query.sort).toBe('title');
        expect(router.currentRoute.value.query.dir).toBe('asc');
        expect(router.currentRoute.value.query.dl).toBeUndefined();
        expect(router.currentRoute.value.query.fav).toBeUndefined();
    });

    it('volver de la pestaña a pistas conserva los filtros compatibles; las pistas no recuperan los descartados', async () => {
        const { wrapper, router } = await mountAt('/library?q=amo&dl=downloaded');
        await wrapper.find('[data-testid="library-tab-artists"]').trigger('click');
        await waitForNavigation(router, '/library/artists');
        expect(router.currentRoute.value.query.q).toBe('amo');
        expect(router.currentRoute.value.query.dl).toBeUndefined();

        await wrapper.find('[data-testid="library-tab-tracks"]').trigger('click');
        await waitForNavigation(router, '/library');
        expect(router.currentRoute.value.query.q).toBe('amo');
        expect(router.currentRoute.value.query.dl).toBeUndefined();
    });

    it('Playlists y descargas conservan sus vistas y rutas actuales', () => {
        const paths = routes.map((r) => r.path);
        expect(paths).toContain('/playlists');
        expect(paths).toContain('/downloads');
        expect(paths).toContain('/queue');
        const downloads = routes.find((r) => r.path === '/queue') as { redirect?: string };
        expect(downloads.redirect).toBe('/downloads');
    });
});
