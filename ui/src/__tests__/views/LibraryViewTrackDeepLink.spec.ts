/**
 * LibraryViewTrackDeepLink.spec.ts
 * Regresión del ítem 45: /library?track=<id> hace scroll hasta la fila, la
 * resalta y limpia el query al montar (aunque la pista no exista).
 */
import { describe, it, expect, beforeAll, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import LibraryView from '@/views/LibraryView.vue'
import { mockInvoke, resetMocks } from '../setup'
import type { LibraryTrack } from '@/api/types'

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

function makePage(tracks: LibraryTrack[], hasMore: boolean) {
    return { tracks, total: tracks.length, offset: 0, limit: 100, has_more: hasMore };
}

async function mountAtQuery(query: Record<string, string>) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/library', name: 'Library', component: LibraryView }],
    })
    await router.push({ path: '/library', query })
    await router.isReady()

    const wrapper = mount(LibraryView, {
        attachTo: document.body,
        global: { plugins: [router] },
    })
    await flushPromises()
    await flushPromises()
    await wrapper.vm.$nextTick()
    return { wrapper, router }
}

describe('LibraryView deep-link ?track=<id> (ítem 45)', () => {
    beforeAll(() => {
        // jsdom no implementa scrollIntoView
        Element.prototype.scrollIntoView = vi.fn()
    })

    beforeEach(() => {
        resetMocks()
        document.body.innerHTML = ''
    })

    it('hace scroll a la fila, la resalta y limpia el query', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return makePage([createTestTrack(42, 'Target Track'), createTestTrack(43, 'Other')], false);
            return null;
        })

        const { wrapper, router } = await mountAtQuery({ track: '42' })

        const row = document.querySelector('[data-track-id="42"]')
        expect(row).not.toBeNull()
        // Scroll hasta la fila
        expect(row!.scrollIntoView).toHaveBeenCalledWith({ behavior: 'smooth', block: 'center' })
        // Resaltada
        expect(row!.classList.contains('bg-primary/10')).toBe(true)
        // El query se consumió
        expect(router.currentRoute.value.query.track).toBeUndefined()
        wrapper.unmount()
    })

    it('pagina automáticamente hasta encontrar la pista enlazada', async () => {
        const page1 = [createTestTrack(1, 'First'), createTestTrack(2, 'Second')];
        const page2 = [createTestTrack(77, 'Deep Track')];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') {
                return (args?.offset ?? 0) === 0
                    ? { ...makePage(page1, true), offset: 0 }
                    : { ...makePage(page2, false), offset: 2 };
            }
            return null;
        })

        const { wrapper, router } = await mountAtQuery({ track: '77' })

        expect(document.querySelector('[data-track-id="77"]')).not.toBeNull()
        expect(Element.prototype.scrollIntoView).toHaveBeenCalled()
        expect(router.currentRoute.value.query.track).toBeUndefined()
        wrapper.unmount()
    })

    it('limpia el query sin crashear cuando la pista no existe', async () => {
        const scrollSpy = Element.prototype.scrollIntoView as unknown as ReturnType<typeof vi.fn>
        scrollSpy.mockClear()

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return makePage([createTestTrack(1, 'Only')], false);
            return null;
        })

        const { wrapper, router } = await mountAtQuery({ track: '999' })

        expect(document.querySelector('[data-track-id="999"]')).toBeNull()
        expect(scrollSpy).not.toHaveBeenCalled()
        expect(router.currentRoute.value.query.track).toBeUndefined()
        // La vista sigue renderizando la biblioteca
        expect(wrapper.text()).toContain('Only')
        wrapper.unmount()
    })

    it('ignora un track no numérico pero limpia el query igualmente', async () => {
        const scrollSpy = Element.prototype.scrollIntoView as unknown as ReturnType<typeof vi.fn>
        scrollSpy.mockClear()

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return makePage([createTestTrack(1, 'Only')], false);
            return null;
        })

        const { wrapper, router } = await mountAtQuery({ track: 'abc' })

        expect(scrollSpy).not.toHaveBeenCalled()
        expect(router.currentRoute.value.query.track).toBeUndefined()
        expect(wrapper.text()).toContain('Only')
        wrapper.unmount()
    })
})
