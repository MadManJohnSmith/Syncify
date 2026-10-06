/**
 * Regresión: los fallos de arranque de reproducción en LibraryView eran
 * 100% silenciosos. Los catch comentaban «player.error already carries the
 * message for the NowPlayingBar», pero la barra solo se renderiza con pista
 * en curso (`v-if="current"`): sin current, el mensaje no llegaba a la UI
 * (sin toast, sin DOM, sin aria-live). Ahora `surfacePlayFailure` divide la
 * superficie: sin current → toast; con current → la barra ya enseña
 * player.error (sin toast duplicado).
 *
 * usePlayer se mockea para controlar el rechazo de play/playNext/enqueue;
 * useToast es el real (singleton de módulo) y se inspecciona su estado.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils';
import LibraryView from '@/views/LibraryView.vue';
import { useToast } from '@/composables/useToast';
import { mockInvoke, resetMocks } from '../setup';
import type { LibraryTrack } from '@/api/types';

const player = vi.hoisted(() => ({
    current: { value: null as null | { id: number; title: string; artist: string; album: string | null; coverUrl: string | null } },
    play: vi.fn(),
    playNext: vi.fn(),
    enqueue: vi.fn(),
}));

vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        play: player.play,
        playNext: player.playNext,
        enqueue: player.enqueue,
        toggle: vi.fn(),
        current: player.current,
    }),
}));

const BACKEND_MSG = 'El track 7 no tiene archivo local descargado; descárgalo primero';

function createTestTrack(overrides: Partial<LibraryTrack> = {}): LibraryTrack {
    return {
        id: 7,
        title: 'Song Seven',
        artist_name: 'Test Artist',
        artist_id: null,
        album_name: 'Test Album',
        album_id: null,
        duration_ms: 180000,
        isrc: 'TEST7',
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
        ...overrides,
    };
}

async function mountLibrary(attach = false): Promise<VueWrapper> {
    mockInvoke((cmd) => {
        if (cmd === 'get_library') {
            return { tracks: [createTestTrack()], total: 1, offset: 0, limit: 50, has_more: false };
        }
        return null;
    });
    const wrapper = attach ? mount(LibraryView, { attachTo: document.body }) : mount(LibraryView);
    await flushPromises();
    return wrapper;
}

function errorToasts() {
    return useToast().toasts.value.filter(t => t.type === 'error');
}

function lastErrorToast() {
    const errors = errorToasts();
    expect(errors.length).toBeGreaterThan(0);
    return errors[errors.length - 1];
}

async function clickContextMenuItem(wrapper: VueWrapper, label: string): Promise<void> {
    await wrapper.find('[data-track-id="7"]').trigger('contextmenu');
    await flushPromises();
    const button = [...document.body.querySelectorAll('button')]
        .find(b => b.textContent?.includes(label));
    expect(button, `menú contextual con "${label}"`).toBeDefined();
    (button as HTMLButtonElement).click();
    await flushPromises();
}

describe('LibraryView — los fallos de reproducción llegan al usuario (regresión)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        player.current.value = null;
        // El singleton de toasts vive a nivel de módulo: limpiarlo entre tests.
        for (const t of useToast().toasts.value) {
            useToast().dismiss(t.id);
        }
    });

    it('dblclick sobre pista sin archivo con player parado: toast con el error del backend', async () => {
        player.play.mockRejectedValue(new Error(BACKEND_MSG));
        const wrapper = await mountLibrary();

        await wrapper.find('[data-track-id="7"]').trigger('dblclick');
        await flushPromises();

        expect(lastErrorToast().title).toBe('No se pudo reproducir "Song Seven"');
        expect(lastErrorToast().description).toBe(BACKEND_MSG);
    });

    it('fallo de arranque con pista en curso: sin toast duplicado (lo muestra la barra con player.error)', async () => {
        player.current.value = { id: 9, title: 'Now Playing', artist: 'X', album: null, coverUrl: null };
        player.play.mockRejectedValue(new Error(BACKEND_MSG));
        const wrapper = await mountLibrary();

        await wrapper.find('[data-track-id="7"]').trigger('dblclick');
        await flushPromises();

        expect(errorToasts()).toHaveLength(0);
    });

    it('"Play Next" sobre player parado con fallo: toast con el error del backend', async () => {
        player.playNext.mockRejectedValue(new Error(BACKEND_MSG));
        const wrapper = await mountLibrary(true);
        try {
            await clickContextMenuItem(wrapper, 'Play Next');
            expect(lastErrorToast().description).toBe(BACKEND_MSG);
        } finally {
            wrapper.unmount();
        }
    });

    it('"Add to Play Queue" sobre player parado con fallo: toast con el error del backend', async () => {
        player.enqueue.mockRejectedValue(new Error(BACKEND_MSG));
        const wrapper = await mountLibrary(true);
        try {
            await clickContextMenuItem(wrapper, 'Add to Play Queue');
            expect(lastErrorToast().description).toBe(BACKEND_MSG);
        } finally {
            wrapper.unmount();
        }
    });
});
