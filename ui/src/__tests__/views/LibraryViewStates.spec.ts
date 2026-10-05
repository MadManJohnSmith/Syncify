/**
 * LibraryViewStates.spec.ts — regresión de auditoría 5 y 60.
 *
 * 5:  un fallo al leer la biblioteca se muestra como error reintentable, no como
 *     «tu biblioteca está vacía».
 * 60: `enrichment-progress` es un canal compartido; solo los eventos de
 *     MusicBrainz describen el trabajo que lanzó el botón.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';

const ENRICHMENT_PROGRESS = 'enrichment-progress';
const listenerEvents: { event: string; handler: (e: any) => void }[] = [];

let invokeHandler: (cmd: string, args?: Record<string, unknown>) => unknown = () => null;

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => invokeHandler(cmd, args)),
    convertFileSrc: (p: string) => `asset://${p}`,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(async (event: string, handler: (e: any) => void) => {
        listenerEvents.push({ event, handler });
        return () => {
            const idx = listenerEvents.findIndex(l => l.handler === handler);
            if (idx >= 0) listenerEvents.splice(idx, 1);
        };
    }),
}));

vi.mock('vue-router', () => ({
    useRouter: () => ({ push: vi.fn() }),
    useRoute: () => ({ query: {}, params: {} }),
}));

import LibraryView from '@/views/LibraryView.vue';

const mockTracks = [
    {
        id: 301,
        title: 'Library Track',
        artist_name: 'Artist 1',
        album_name: 'Album 1',
        duration_ms: 180000,
        cover_art_url: null,
        download_status: 'downloaded',
    },
];

const libraryPage = {
    tracks: mockTracks,
    total: 1,
    offset: 0,
    limit: 100,
    has_more: false,
};

async function mountView(handler: (cmd: string, args?: Record<string, unknown>) => unknown) {
    invokeHandler = handler;
    const wrapper = mount(LibraryView);
    await flushPromises();
    return wrapper;
}

function libraryInvoke(cmd: string) {
    if (cmd === 'get_library') return libraryPage;
    return null;
}

function emitProgress(payload: unknown) {
    listenerEvents
        .filter(l => l.event === ENRICHMENT_PROGRESS)
        .forEach(l => l.handler({ payload }));
}

describe('LibraryView — estados de carga y error (auditoría 5)', () => {
    beforeEach(() => {
        listenerEvents.length = 0;
        document.body.innerHTML = '';
    });

    afterEach(() => {
        document.body.innerHTML = '';
    });

    it('muestra un error reintentable cuando la biblioteca no se puede leer', async () => {
        let fail = true;
        const wrapper = await mountView((cmd) => {
            if (fail) throw new Error('library table locked');
            return libraryInvoke(cmd);
        });

        const errorBox = wrapper.find('[data-testid="library-load-error"]');
        expect(errorBox.exists()).toBe(true);
        expect(errorBox.text()).toContain('library table locked');
        // Un fallo de carga no puede parecerse a una biblioteca vacía.
        expect(wrapper.text()).not.toContain('Your library is empty');

        fail = false;
        await wrapper.find('[data-testid="library-load-error"] button').trigger('click');
        await flushPromises();

        expect(wrapper.find('[data-testid="library-load-error"]').exists()).toBe(false);
    });

    it('una biblioteca realmente vacía sí muestra el estado vacío', async () => {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 100, has_more: false };
            return null;
        });

        expect(wrapper.find('[data-testid="library-load-error"]').exists()).toBe(false);
        expect(wrapper.text()).toContain('Your library is empty');
    });
});

describe('LibraryView — canal enrichment-progress compartido (auditoría 60)', () => {
    beforeEach(() => {
        listenerEvents.length = 0;
        document.body.innerHTML = '';
    });

    afterEach(() => {
        document.body.innerHTML = '';
    });

    /** Arranca el enriquecimiento dejando el comando pendiente. */
    async function startEnriching() {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'enrich_metadata_musicbrainz') return new Promise(() => {});
            return libraryInvoke(cmd);
        });

        const button = wrapper.find('button[title="Enrich metadata using MusicBrainz"]');
        expect(button.exists()).toBe(true);
        await button.trigger('click');
        await flushPromises();

        expect(listenerEvents.some(l => l.event === ENRICHMENT_PROGRESS)).toBe(true);
        return wrapper;
    }

    function counterText(wrapper: any) {
        return wrapper.find('button[title="Enrich metadata using MusicBrainz"]').text();
    }

    const musicBrainzEvent = {
        status: 'progress',
        total: 120,
        current: 40,
        enriched: 38,
        failed: 2,
        currentTrack: 'Artist - Title',
    };

    it('muestra el contador real de MusicBrainz', async () => {
        const wrapper = await startEnriching();

        emitProgress(musicBrainzEvent);
        await flushPromises();

        expect(counterText(wrapper)).toContain('40/120');
    });

    it('ignora el payload de Last.fm, que no describe el trabajo de MusicBrainz', async () => {
        const wrapper = await startEnriching();
        emitProgress(musicBrainzEvent);
        await flushPromises();

        // Last.fm emite { type, status, current, total, message } por el mismo
        // canal: `current` cuenta aciertos y no llega `currentTrack`.
        emitProgress({
            type: 'lastfm_genre',
            status: 'progress',
            current: 0,
            total: 412,
            message: 'Enriched 0/412 tracks with genres',
        });
        await flushPromises();

        expect(counterText(wrapper)).toContain('40/120');
        expect(counterText(wrapper)).not.toContain('0/412');
    });

    it('ignora el resumen camelCase del enriquecimiento incremental', async () => {
        const wrapper = await startEnriching();
        emitProgress(musicBrainzEvent);
        await flushPromises();

        // EnrichmentJobSummary llega en camelCase (totalTracks, processedTracks).
        emitProgress({
            jobId: 'job-1',
            status: 'running',
            totalTracks: 900,
            processedTracks: 3,
            currentTrack: 'Otra - Pista',
            currentPhase: 'Enriching',
        });
        await flushPromises();

        expect(counterText(wrapper)).toContain('40/120');
        expect(counterText(wrapper)).not.toContain('undefined');
        expect(counterText(wrapper)).not.toContain('NaN');
    });
});