/**
 * Fase 1 — persistencia (`syncify.playerState.v1`) y restauración perezosa.
 *
 * Verifica que la clave versionada nunca contiene `file_path`, URLs ni el DTO
 * completo; que el estado corrupto se descarta completo; que la restauración
 * re-resuelve la fuente (jamás reusa concesiones), queda pausada y solo valida
 * la pista actual (los pendientes se validan justo antes de sonar); y que
 * importar el módulo no dispara invokes ni reproducción.
 */
import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'

const playStub = vi.fn(() => Promise.resolve())
const pauseStub = vi.fn()
const loadStub = vi.fn()

let audioInstance: HTMLMediaElement | null = null

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string, protocol?: string) =>
        `${protocol ?? 'asset'}://localhost/${encodeURIComponent(path)}`,
}));

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri'

const mockInvoke = vi.mocked(invokeCommand);
const KEY = 'syncify.playerState.v1';
const VALID_UUID = 'a3bb189e-8bf9-4888-9912-ace4e6543002';
const VALID_UUID_2 = 'c1a0f4e2-77aa-4c31-9d20-5b6f0a1e8890';

type UsePlayerModule = typeof import('../../composables/usePlayer');

let mod: UsePlayerModule | null = null;

/** Re-importa el módulo bajo test: estado del singleton y guard de restauración frescos. */
async function freshPlayer(): Promise<UsePlayerModule> {
    vi.resetModules();
    mod = await import('../../composables/usePlayer');
    return mod;
}

type MutableMedia = HTMLMediaElement & { currentTime: number; readyState: number };

function media(): MutableMedia {
    if (!audioInstance) throw new Error('audio singleton no capturado');
    return audioInstance as unknown as MutableMedia;
}

function seedState(overrides: Record<string, unknown>): void {
    localStorage.setItem(KEY, JSON.stringify({
        version: 1,
        queue: [],
        baseOrder: [],
        history: [],
        currentTrackId: null,
        positionSec: 0,
        shuffle: false,
        repeat: 'off',
        volume: 1,
        muted: false,
        pendingListens: [],
        ...overrides,
    }));
}

beforeAll(() => {
    Object.defineProperty(window.HTMLMediaElement.prototype, 'play', { configurable: true, value: playStub });
    Object.defineProperty(window.HTMLMediaElement.prototype, 'pause', { configurable: true, value: pauseStub });
    Object.defineProperty(window.HTMLMediaElement.prototype, 'load', { configurable: true, value: loadStub });
    class CapturingAudio extends window.Audio {
        constructor() {
            super();
            audioInstance = this;
        }
    }
    (window as unknown as { Audio: typeof Audio }).Audio = CapturingAudio;
});

describe('usePlayer — persistencia y restauración (fase 1)', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        mockInvoke.mockReset();
        localStorage.clear();
        playStub.mockClear();
        pauseStub.mockClear();
        loadStub.mockClear();
    });
    afterEach(() => {
        vi.useRealTimers();
    });

    it('importar el módulo no dispara invokes ni reproducción (restauración perezosa)', async () => {
        await freshPlayer();
        expect(mockInvoke).not.toHaveBeenCalled();
        expect(playStub).not.toHaveBeenCalled();
    });

    it('restaura track/cola/posición/modos y queda PAUSADO, proyectando sin file_path', async () => {
        seedState({
            queue: [{ entryId: 'e-2', trackId: 2 }, { entryId: 'e-3', trackId: 3 }],
            baseOrder: ['e-2', 'e-3'],
            history: [{ entryId: 'e-1', trackId: 1 }],
            currentTrackId: 7,
            positionSec: 42.5,
            shuffle: true,
            repeat: 'all',
            volume: 0.5,
            muted: true,
        });
        mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_track_metadata') {
                expect(args?.trackId).toBe(7);
                return Promise.resolve({
                    track_id: 7,
                    title: 'Song',
                    artist_name: 'Artist',
                    album_name: 'Album',
                    cover_art_url: 'https://cdn.test/a.jpg',
                    file_path: '/lib/a.flac',
                    duration_ms: 200000,
                    sources: [],
                });
            }
            if (cmd === 'resolve_playback_source') {
                return Promise.resolve({ track_id: 7, file_path: '/lib/a.flac' });
            }
            return Promise.resolve(null);
        });

        const player = (await freshPlayer()).usePlayer();
        const m = media();
        const restoring = player.restoreFromPersistence();
        await flushPromises(); // get_track_metadata + resolve ya resueltos, src asignado
        m.dispatchEvent(new Event('loadedmetadata'));
        await restoring;
        await flushPromises();

        expect(mockInvoke).toHaveBeenCalledWith('get_track_metadata', { trackId: 7 });
        expect(mockInvoke).toHaveBeenCalledWith('resolve_playback_source', { trackId: 7 });
        // Proyección estricta: sin file_path ni DTO completo.
        expect(player.current.value).toEqual({
            id: 7,
            title: 'Song',
            artist: 'Artist',
            album: 'Album',
            coverUrl: 'https://cdn.test/a.jpg',
        });
        expect(Object.keys(player.current.value ?? {})).not.toContain('file_path');
        expect(m.src).toContain(encodeURIComponent('/lib/a.flac'));
        expect(player.positionSec.value).toBe(42.5);
        expect(player.isPlaying.value).toBe(false);
        expect(playStub).not.toHaveBeenCalled(); // restaurar NO reproduce
        expect(player.shuffle.value).toBe(true);
        expect(player.repeat.value).toBe('all');
        expect(player.volume.value).toBe(0.5);
        expect(player.muted.value).toBe(true);
        expect(player.queue.value.map(e => e.trackId)).toEqual([2, 3]);
        expect(player.history.value.map(e => e.trackId)).toEqual([1]);
    });

    it('reanudar tras restaurar arranca una sesión nueva: el acumulado empieza en 0', async () => {
        seedState({ currentTrackId: 7, positionSec: 120 });
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_track_metadata') {
                return Promise.resolve({ track_id: 7, title: 'Song', artist_name: 'Artist', album_name: null, cover_art_url: null, file_path: '/lib/a.flac' });
            }
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/a.flac' });
            if (cmd === 'record_local_play') return Promise.resolve({ track_id: 7, play_count: 1, last_played: null, counted: true });
            return Promise.resolve(null);
        });

        const player = (await freshPlayer()).usePlayer();
        const m = media();
        const restoring = player.restoreFromPersistence();
        await flushPromises();
        m.dispatchEvent(new Event('loadedmetadata'));
        await restoring;
        await flushPromises();

        // El usuario reanuda: el navegador dispara 'play'.
        player.toggle();
        Object.defineProperty(m, 'paused', { configurable: true, value: false });
        Object.defineProperty(m, 'readyState', { configurable: true, value: 3 });
        m.dispatchEvent(new Event('play'));
        await flushPromises();
        mockInvoke.mockClear();

        // Pese a posición 120: 119 ticks → 29,75 s → nada.
        tick(m, 120);
        for (let i = 1; i <= 119; i++) tick(m, 120 + i * 0.25);
        await flushPromises();
        expect(mockInvoke.mock.calls.filter(c => c[0] === 'record_local_play')).toHaveLength(0);
        tick(m, 150);
        await flushPromises();
        const calls = mockInvoke.mock.calls.filter(c => c[0] === 'record_local_play');
        expect(calls).toHaveLength(1);
        expect(calls[0]?.[1]?.trackId).toBe(7);
    });

    it('la pista actual desaparecida arranca sin current conservando la cola', async () => {
        seedState({
            queue: [{ entryId: 'e-2', trackId: 2 }],
            baseOrder: ['e-2'],
            currentTrackId: 9,
            positionSec: 10,
        });
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'get_track_metadata') return Promise.reject(new Error('El track 9 no existe en la biblioteca'));
            return Promise.resolve(null);
        });

        const player = (await freshPlayer()).usePlayer();
        await player.restoreFromPersistence();
        await flushPromises();

        expect(player.current.value).toBeNull();
        expect(player.queue.value.map(e => e.trackId)).toEqual([2]);
        expect(mockInvoke.mock.calls.some(c => c[0] === 'resolve_playback_source')).toBe(false);
    });

    it('estado corrupto o de versión desconocida se descarta completo sin bloquear', async () => {
        const badStates: unknown[] = [
            '{oops no es json',
            null,
            { version: 99 },
            { version: 1, positionSec: 'x' },
            { version: 1, repeat: 'sometimes' },
            { version: 1, queue: [{ entryId: 'e', trackId: 0 }] },
            { version: 1, currentTrackId: -5 },
            { version: 1, volume: 7 },
            { version: 1, history: 'no' },
            { version: 1, pendingListens: 'no' },
        ];
        for (const bad of badStates) {
            localStorage.setItem(KEY, typeof bad === 'string' ? bad : JSON.stringify(bad));
            const player = (await freshPlayer()).usePlayer();
            await expect(player.restoreFromPersistence()).resolves.toBeUndefined();
            expect(player.current.value).toBeNull();
            expect(player.queue.value).toHaveLength(0);
            expect(mockInvoke).not.toHaveBeenCalled();
        }
    });

    it('pendientes de escucha corruptos se descartan al cargar; el UUID en mayúsculas se normaliza', async () => {
        seedState({
            pendingListens: [
                { listenSessionId: 'not-a-uuid', trackId: 7, attempts: 0 },
                { listenSessionId: VALID_UUID.toUpperCase(), trackId: 8, attempts: 2 },
                { listenSessionId: VALID_UUID_2, trackId: -3, attempts: 0 },
                { listenSessionId: VALID_UUID_2, trackId: 9 },
            ],
        });
        const player = (await freshPlayer()).usePlayer();
        await player.restoreFromPersistence();
        await flushPromises();

        expect(player.pendingListens.value).toEqual([
            { trackId: 8, listenSessionId: VALID_UUID, attempts: 2 },
            { trackId: 9, listenSessionId: VALID_UUID_2, attempts: 0 },
        ]);
        // No se envía nada todavía: el reintento queda programado, no inmediato.
        expect(mockInvoke.mock.calls.some(c => c[0] === 'record_local_play')).toBe(false);
    });

    it('la clave persistida nunca contiene file_path, URLs de archivo ni el DTO completo', async () => {
        const player = (await freshPlayer()).usePlayer();
        mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'resolve_playback_source') {
                return Promise.resolve({ track_id: args?.trackId, file_path: `/lib/secret-${args?.trackId}.flac` });
            }
            return Promise.resolve(null);
        });
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        await player.enqueue({ id: 8, title: 'Other', artist: 'Artist' });

        const raw = localStorage.getItem(KEY);
        expect(raw).not.toBeNull();
        expect(raw).not.toContain('file_path');
        expect(raw).not.toContain('syncify-media');
        expect(raw).not.toContain('/lib/');
        const parsed = JSON.parse(raw as string) as Record<string, unknown>;
        expect(parsed.version).toBe(1);
        for (const e of parsed.queue as Array<Record<string, unknown>>) {
            expect(Object.keys(e).sort()).toEqual(['entryId', 'trackId']);
        }
        expect(parsed.currentTrackId).toBe(7);
        expect(parsed.volume).toBe(1);
        expect(Array.isArray(parsed.pendingListens)).toBe(true);
        expect(Array.isArray(parsed.baseOrder)).toBe(true);
    });

    it('validación perezosa: el pendiente desaparecido se descarta al vuelo y suena el siguiente', async () => {
        seedState({
            queue: [{ entryId: 'e-999', trackId: 999 }, { entryId: 'e-5', trackId: 5 }],
            baseOrder: ['e-999', 'e-5'],
        });
        mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_track_metadata') {
                if (args?.trackId === 999) return Promise.reject(new Error('El track 999 no existe en la biblioteca'));
                return Promise.resolve({ track_id: 5, title: 'Five', artist_name: 'Artist', album_name: null, cover_art_url: null, file_path: '/lib/5.flac' });
            }
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 5, file_path: '/lib/5.flac' });
            return Promise.resolve(null);
        });

        const player = (await freshPlayer()).usePlayer();
        await player.restoreFromPersistence();
        await flushPromises();
        mockInvoke.mockClear();

        // play() sin argumento arranca la primera pendiente de la cola restaurada.
        await player.play();
        await flushPromises();

        const seq = mockInvoke.mock.calls.map(c => `${c[0]}:${(c[1] as Record<string, unknown> | undefined)?.trackId}`);
        expect(seq).toEqual(['get_track_metadata:999', 'get_track_metadata:5', 'resolve_playback_source:5']);
        expect(player.current.value?.id).toBe(5);
        expect(player.queue.value).toHaveLength(0);
    });

    it('visibilitychange (hidden) persiste el estado', async () => {
        const player = (await freshPlayer()).usePlayer();
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/7.flac' });
            return Promise.resolve(null);
        });
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        localStorage.removeItem(KEY);

        Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
        document.dispatchEvent(new Event('visibilitychange'));
        try {
            const raw = localStorage.getItem(KEY);
            expect(raw).not.toBeNull();
            const parsed = JSON.parse(raw as string) as Record<string, unknown>;
            expect(parsed.currentTrackId).toBe(7);
        } finally {
            delete (document as unknown as Record<string, unknown>).visibilityState;
        }
    });

    function tick(m: MutableMedia, mediaSec: number, wallMs = 250): void {
        vi.advanceTimersByTime(wallMs);
        m.currentTime = mediaSec;
        m.dispatchEvent(new Event('timeupdate'));
    }
});
