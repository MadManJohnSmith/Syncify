/**
 * LibraryViewTrackDetailPanel.spec.ts
 * L04 (issue #101): panel lateral de detalle de pista.
 *
 * Criterios cubiertos:
 * - UNA llamada al abrir el panel (sin N+1: no hay consulta por campo,
 *   fuente o pertenencia) y refresco completo tras una acción;
 * - una pista inexistente muestra el error tipado, no una fila vacía;
 * - el deep link ?track=<id> abre el panel, hace scroll a la fila, la
 *   resalta y consume el parámetro (contrato del ítem 45 intacto);
 * - Escape, el fondo y un cambio de ruta cierran el panel;
 * - cuarentena y duplicados deshabilitados con su motivo (#72/#73): no se
 *   simulan.
 */
import { describe, it, expect, beforeAll, beforeEach, afterEach, vi } from 'vitest'
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

/** Fixture del contrato `get_library_track_detail` (camelCase del backend). */
const detailFixture = {
    track: {
        ...createTestTrack(1, 'Target Track'),
        is_favorite: false,
        display_title: 'Target Track',
    },
    sources: [
        {
            serviceId: 1,
            serviceName: 'spotify',
            serviceTrackId: 'sp-1',
            format: 'FLAC',
            bitDepth: 24,
            sampleRate: 96000,
            bitrate: 4000,
            qualityScore: 95,
            available: true,
            availabilityStatus: 'available',
            availabilityReason: null,
            lastChecked: null,
        },
        {
            serviceId: 2,
            serviceName: 'qobuz',
            serviceTrackId: 'qb-1',
            format: 'MP3',
            bitDepth: null,
            sampleRate: null,
            bitrate: 320,
            qualityScore: 60,
            available: false,
            availabilityStatus: 'stale_404',
            availabilityReason: 'Provider returned 404',
            lastChecked: null,
        },
    ],
    memberships: [
        {
            accountId: 1,
            displayName: 'A1',
            serviceName: 'spotify',
            collection: 'favorite',
            collectionId: null,
            collectionName: null,
            addedAt: '2026-01-03T10:00:00Z',
        },
        {
            accountId: 2,
            displayName: 'A2',
            serviceName: 'qobuz',
            collection: 'playlist',
            collectionId: 1,
            collectionName: 'A2 list',
            addedAt: '2026-01-05T10:00:00Z',
        },
    ],
    download: {
        status: 'downloaded',
        effectiveService: 'qobuz',
        filePath: '/music/qobuz/Target Track.flac',
        fileFormat: 'FLAC',
        bitDepth: 24,
        sampleRate: 96000,
        downloadedAt: '2026-01-06T09:30:00Z',
    },
    quarantine: null,
    duplicateGroup: null,
};

const detailCalls: Array<{ command: string; args?: Record<string, unknown> }> = []

function installDefaultDetailMocks() {
    mockInvoke((cmd, args) => {
        detailCalls.push({ command: cmd, args });
        if (cmd === 'query_library') return makePage([createTestTrack(1, 'Target Track'), createTestTrack(2, 'Other')], false);
        if (cmd === 'get_library_track_detail') return JSON.parse(JSON.stringify(detailFixture));
        if (cmd === 'get_playlists') return [];
        return null;
    });
}

async function mountLibrary(query: Record<string, string> = {}) {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/library', name: 'Library', component: LibraryView },
            { path: '/metadata', name: 'Metadata', component: { template: '<div/>' } },
            { path: '/other', name: 'Other', component: { template: '<div/>' } },
        ],
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

async function openPanelForTrack1() {
    const button = document.querySelector('[data-testid="track-detail-open-1"]') as HTMLButtonElement | null;
    expect(button).not.toBeNull();
    button!.click();
    await flushPromises();
    await flushPromises();
}

describe('LibraryView panel lateral de detalle (L04, #101)', () => {
    beforeAll(() => {
        Element.prototype.scrollIntoView = vi.fn()
    })

    beforeEach(() => {
        resetMocks()
        detailCalls.length = 0
        document.body.innerHTML = ''
    })

    afterEach(() => {
        vi.restoreAllMocks()
    })

    it('abre con UNA llamada y muestra todas las facetas', async () => {
        installDefaultDetailMocks()
        const { wrapper } = await mountLibrary()

        await openPanelForTrack1()

        const panel = document.querySelector('[data-testid="track-detail-panel"]');
        expect(panel).not.toBeNull();
        // Una sola llamada IPC para todo el panel.
        const detailCommands = detailCalls.filter((c) => c.command === 'get_library_track_detail');
        expect(detailCommands.length).toBe(1);
        expect(detailCommands[0].args).toEqual({ trackId: 1 });

        const text = panel!.textContent ?? '';
        // Fuentes por servicio: formato, profundidad/frecuencia, bitrate y disponibilidad.
        expect(text).toContain('spotify');
        expect(text).toContain('FLAC · 24/96000 · 4000kbps');
        expect(text).toContain('available');
        expect(text).toContain('qobuz');
        expect(text).toContain('stale_404');
        // Cuentas y pertenencias por colección.
        expect(text).toContain('A1');
        expect(text).toContain('favorite');
        expect(text).toContain('A2');
        expect(text).toContain('playlist: A2 list');
        // Descarga: estado, servicio efectivo y ruta.
        expect(text).toContain('downloaded');
        expect(text).toContain('qobuz');
        expect(text).toContain('/music/qobuz/Target Track.flac');
        // Cuarentena y duplicados deshabilitados con su motivo (#72/#73):
        // secciones presentes, sin datos inventados.
        expect(document.querySelector('[data-testid="track-detail-quarantine-disabled"]')).not.toBeNull();
        expect(document.querySelector('[data-testid="track-detail-duplicates-disabled"]')).not.toBeNull();
        expect(text).toContain('arrives with #72/L09');
        expect(text).toContain('arrives with #73/L10');
        wrapper.unmount();
    })

    it('sin N+1: tras una acción refresca el panel una vez, no por campo', async () => {
        installDefaultDetailMocks()
        const { wrapper } = await mountLibrary()
        await openPanelForTrack1()

        detailCalls.length = 0;
        const favoriteButton = document.querySelector('[data-testid="track-detail-favorite"]') as HTMLButtonElement;
        expect(favoriteButton).not.toBeNull();
        favoriteButton.click();
        await flushPromises();
        await flushPromises();

        const detailCommands = detailCalls.filter((c) => c.command === 'get_library_track_detail');
        // El panel entero se refresca con la misma llamada única.
        expect(detailCommands.length).toBe(1);
        // Ninguna consulta de detalle por campo, fuente o pertenencia.
        const otherDetailCommands = detailCalls.filter((c) => c.command !== 'get_library_track_detail' && c.command !== 'toggle_favorite');
        expect(otherDetailCommands).toEqual([]);
        wrapper.unmount();
    })

    it('pista inexistente: error tipado, no una fila vacía', async () => {
        mockInvoke((cmd, args) => {
            detailCalls.push({ command: cmd, args });
            if (cmd === 'query_library') return makePage([createTestTrack(1, 'Only')], false);
            if (cmd === 'get_playlists') return [];
            if (cmd === 'get_library_track_detail') {
                // El backend rechaza con el error tipado; invokeCommand lo
                // envuelve y la vista muestra el estado de error.
                throw { kind: 'track_not_found', trackId: args?.trackId };
            }
            return null;
        })
        const { wrapper } = await mountLibrary()
        await openPanelForTrack1()

        const errorBox = document.querySelector('[data-testid="track-detail-error"]');
        expect(errorBox).not.toBeNull();
        const text = errorBox!.textContent ?? '';
        expect(text).toContain('Track not found');
        expect(text).toContain('No track with id 1.');
        // No se renderiza una ficha vacía.
        expect(document.querySelector('[data-testid="track-detail-sources"]')).toBeNull();
        wrapper.unmount();
    })

    it('Escape cierra el panel', async () => {
        installDefaultDetailMocks()
        const { wrapper } = await mountLibrary()
        await openPanelForTrack1()
        expect(document.querySelector('[data-testid="track-detail-panel"]')).not.toBeNull();

        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
        await wrapper.vm.$nextTick();
        expect(document.querySelector('[data-testid="track-detail-panel"]')).toBeNull();
        wrapper.unmount();
    })

    it('el fondo cierra el panel', async () => {
        installDefaultDetailMocks()
        const { wrapper } = await mountLibrary()
        await openPanelForTrack1()

        const backdrop = document.querySelector('[data-testid="track-detail-backdrop"]');
        expect(backdrop).not.toBeNull();
        backdrop!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await wrapper.vm.$nextTick();
        expect(document.querySelector('[data-testid="track-detail-panel"]')).toBeNull();
        wrapper.unmount();
    })

    it('un cambio de ruta cierra el panel', async () => {
        installDefaultDetailMocks()
        const { wrapper, router } = await mountLibrary()
        await openPanelForTrack1()
        expect(document.querySelector('[data-testid="track-detail-panel"]')).not.toBeNull();

        await router.push('/other');
        await flushPromises();
        expect(document.querySelector('[data-testid="track-detail-panel"]')).toBeNull();
        wrapper.unmount();
    })

    it('deep link ?track=<id>: abre el panel, resalta la fila y consume el parámetro', async () => {
        installDefaultDetailMocks()
        const { wrapper, router } = await mountLibrary({ track: '1' })

        // Contrato del ítem 45 intacto: scroll, resaltado y parámetro consumido.
        expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
        const row = document.querySelector('[data-track-id="1"]');
        expect(row).not.toBeNull();
        expect(row!.classList.contains('bg-primary/10')).toBe(true);
        expect(router.currentRoute.value.query.track).toBeUndefined();
        // Y el panel de esa pista abierto con UNA llamada.
        expect(document.querySelector('[data-testid="track-detail-panel"]')).not.toBeNull();
        const detailCommands = detailCalls.filter((c) => c.command === 'get_library_track_detail');
        expect(detailCommands.length).toBe(1);
        expect(detailCommands[0].args).toEqual({ trackId: 1 });
        wrapper.unmount();
    })

    it('deep link a una pista inexistente: consume el parámetro sin abrir panel ni crashear', async () => {
        installDefaultDetailMocks()
        const { wrapper, router } = await mountLibrary({ track: '999' })

        expect(router.currentRoute.value.query.track).toBeUndefined();
        expect(document.querySelector('[data-track-id="999"]')).toBeNull();
        expect(document.querySelector('[data-testid="track-detail-panel"]')).toBeNull();
        expect(wrapper.text()).toContain('Target Track');
        wrapper.unmount();
    })

    it('borrar audio con confirmación: una llamada, refresca el panel', async () => {
        installDefaultDetailMocks()
        const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
        const { wrapper } = await mountLibrary()
        await openPanelForTrack1()

        detailCalls.length = 0;
        const deleteButton = document.querySelector('[data-testid="track-detail-delete-audio"]') as HTMLButtonElement;
        expect(deleteButton).not.toBeNull();
        deleteButton.click();
        await flushPromises();
        await flushPromises();

        expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Target Track'));
        const deleteCalls = detailCalls.filter((c) => c.command === 'delete_track_audio');
        expect(deleteCalls.length).toBe(1);
        expect(deleteCalls[0].args).toEqual({ trackId: 1 });
        // El panel se refresca (una llamada) y la lista se recarga.
        const detailCommands = detailCalls.filter((c) => c.command === 'get_library_track_detail');
        expect(detailCommands.length).toBe(1);
        const listReloads = detailCalls.filter((c) => c.command === 'query_library');
        expect(listReloads.length).toBeGreaterThan(0);
        wrapper.unmount();
    })

    it('borrar audio sin confirmación no llama al backend', async () => {
        installDefaultDetailMocks()
        vi.spyOn(window, 'confirm').mockReturnValue(false);
        const { wrapper } = await mountLibrary()
        await openPanelForTrack1()

        detailCalls.length = 0;
        const deleteButton = document.querySelector('[data-testid="track-detail-delete-audio"]') as HTMLButtonElement;
        deleteButton.click();
        await flushPromises();

        expect(detailCalls.filter((c) => c.command === 'delete_track_audio')).toEqual([]);
        wrapper.unmount();
    })
})
