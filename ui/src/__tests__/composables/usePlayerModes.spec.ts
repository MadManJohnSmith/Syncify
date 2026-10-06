/**
 * Fase 1 — cola (identidades, replaceQueueAndPlay, reordenación), modos
 * repeat/shuffle, next/previous, fallos de avance y carreras resolver–stop.
 *
 * jsdom no reproduce audio: HTMLMediaElement está stubbed y el singleton se
 * captura para disparar eventos de medios.
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

const A = { id: 1, title: 'One', artist: 'X' };
const B = { id: 2, title: 'Two', artist: 'X' };
const C = { id: 3, title: 'Three', artist: 'X' };

let usePlayer: typeof import('../../composables/usePlayer').usePlayer;

function resolveByTrackId(): void {
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
        if (cmd === 'resolve_playback_source') {
            return Promise.resolve({ track_id: args?.trackId, file_path: `/lib/${args?.trackId}.flac` });
        }
        if (cmd === 'get_track_metadata') {
            // DTO mínimo para la validación justamente-antes-de-sonar (entradas
            // del historial/restauradas sin proyección).
            return Promise.resolve({
                track_id: args?.trackId,
                title: `Track ${args?.trackId}`,
                artist_name: 'X',
                album_name: null,
                cover_art_url: null,
                file_path: `/lib/${args?.trackId}.flac`,
            });
        }
        return Promise.resolve(null);
    });
}

function resolveCalls(): number[] {
    return mockInvoke.mock.calls
        .filter(c => c[0] === 'resolve_playback_source')
        .map(c => (c[1] as Record<string, unknown> | undefined)?.trackId as number);
}

function media(): HTMLMediaElement {
    if (!audioInstance) throw new Error('audio singleton no capturado');
    return audioInstance;
}

function ended(): void {
    media().dispatchEvent(new Event('ended'));
}

beforeAll(async () => {
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
    ({ usePlayer } = await import('../../composables/usePlayer'));
});

describe('usePlayer — cola, modos, next/previous y carreras (fase 1)', () => {
    beforeEach(async () => {
        mockInvoke.mockReset();
        localStorage.clear();
        // Módulo fresco: el singleton arrastra modos entre tests y stop() no
        // los toca (son preferencias persistentes por diseño).
        vi.resetModules();
        ({ usePlayer } = await import('../../composables/usePlayer'));
        usePlayer().stop();
        playStub.mockClear();
        pauseStub.mockClear();
        loadStub.mockClear();
    });

    it('replaceQueueAndPlay reproduce la primera y encola el resto en su orden', async () => {
        resolveByTrackId();
        const player = usePlayer();

        await player.replaceQueueAndPlay([A, B, C]);

        expect(player.current.value?.id).toBe(1);
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]);
        // Solo la primera se resuelve: el resto se resolverá al avanzar.
        expect(resolveCalls()).toEqual([1]);
    });

    it('repeat-all reconstruye el ciclo con el orden base y sin duplicados entre vueltas', async () => {
        resolveByTrackId();
        const player = usePlayer();
        player.cycleRepeat(); // off → all

        await player.replaceQueueAndPlay([A, B, C]);
        ended(); await flushPromises(); // A termina → B
        ended(); await flushPromises(); // B termina → C
        ended(); await flushPromises(); // C termina → ciclo agotado → reconstruye → A
        expect(player.current.value?.id).toBe(1);
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]);

        ended(); await flushPromises();
        ended(); await flushPromises();
        ended(); await flushPromises(); // segunda vuelta completa
        expect(player.current.value?.id).toBe(1);
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]); // sin crecimiento ni duplicados
        expect(player.queue.value).toHaveLength(2);
    });

    it('la cola avanza al ended con pendientes incluso en repeat-one', async () => {
        resolveByTrackId();
        const player = usePlayer();
        player.cycleRepeat(); // off → all
        player.cycleRepeat(); // all → one

        await player.play(A);
        await player.playNext(B);
        ended(); await flushPromises();

        // Avanza a B (la cola manda con pendientes); repeat-one solo aplica al agotarse.
        expect(player.current.value?.id).toBe(2);

        // Cola agotada + repeat-one: reinicia B con currentTime = 0 y nueva resolución.
        const before = resolveCalls().length;
        ended(); await flushPromises();
        expect(player.current.value?.id).toBe(2);
        expect(player.positionSec.value).toBe(0);
        expect(resolveCalls().length).toBe(before + 1);
        expect(resolveCalls()).toContain(2);
    });

    it('next() salta inmediatamente e ignora repeat-one; agotado con repeat-one es no-op', async () => {
        resolveByTrackId();
        const player = usePlayer();
        player.cycleRepeat(); // all
        player.cycleRepeat(); // one

        await player.play(A);
        await player.playNext(B);
        player.next();
        await flushPromises();
        expect(player.current.value?.id).toBe(2);
        expect(resolveCalls()).toContain(2);

        const before = resolveCalls().length;
        player.next(); // cola agotada + repeat-one → no-op
        await flushPromises();
        expect(player.current.value?.id).toBe(2);
        expect(resolveCalls().length).toBe(before);
    });

    it('next() con cola agotada y repeat all reconstruye y reproduce', async () => {
        resolveByTrackId();
        const player = usePlayer();
        player.cycleRepeat(); // all

        await player.play(A);
        const before = resolveCalls().length;
        player.next();
        await flushPromises();
        expect(resolveCalls().length).toBe(before + 1);
        expect(player.current.value?.id).toBe(1);
    });

    it('previous(): posición > 3 s reinicia a 0; si no, pop del historial y la actual vuelve al frente', async () => {
        resolveByTrackId();
        const player = usePlayer();
        const m = media();

        await player.play(A);
        await player.playNext(B);
        ended(); await flushPromises(); // current B, history [A]

        player.previous();
        await flushPromises();
        // positionSec 0 (≤ 3 s) → pop del historial: suena A y B vuelve al frente.
        expect(player.current.value?.id).toBe(1);
        expect(player.upNext.value.map(t => t.id)).toEqual([2]);

        // Posición > 3 s → seek(0), sin nueva resolución.
        (m as unknown as { currentTime: number }).currentTime = 5;
        player.seek(5);
        const before = resolveCalls().length;
        player.previous();
        await flushPromises();
        expect(player.positionSec.value).toBe(0);
        expect(player.current.value?.id).toBe(1);
        expect(resolveCalls().length).toBe(before);
    });

    it('previous() sin historial hace seek(0)', async () => {
        resolveByTrackId();
        const player = usePlayer();
        await player.play(A);
        player.seek(4);
        player.previous();
        await flushPromises();
        expect(player.positionSec.value).toBe(0);
        expect(resolveCalls()).toEqual([1]); // sin resoluciones nuevas
    });

    it('fallo de avance conserva la cabeza, detiene la cadena y muestra el error', async () => {
        mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'resolve_playback_source') {
                if (args?.trackId === 2) return Promise.reject(new Error('El track 2 no tiene archivo local descargado; descárgalo primero'));
                return Promise.resolve({ track_id: args?.trackId, file_path: `/lib/${args?.trackId}.flac` });
            }
            return Promise.resolve(null);
        });
        const player = usePlayer();

        await player.play(A);
        await player.playNext(B);
        ended(); await flushPromises();

        expect(player.error.value).toContain('no tiene archivo local');
        expect(player.current.value?.id).toBe(1); // la actual no se toca
        expect(player.upNext.value.map(t => t.id)).toEqual([2]); // la cabeza se conserva
    });

    it('carrera resolver–stop: sin error y sin current', async () => {
        let release!: (v: unknown) => void;
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') {
                return new Promise(res => { release = res; });
            }
            return Promise.resolve(null);
        });
        const player = usePlayer();

        const pending = player.play(A);
        player.stop();
        release({ track_id: 1, file_path: '/lib/1.flac' });
        await pending;
        await flushPromises();

        expect(player.error.value).toBeNull();
        expect(player.current.value).toBeNull();
        expect(playStub).not.toHaveBeenCalled();
    });

    it('AbortError de un play() interrumpido: sin error y sin current', async () => {
        resolveByTrackId();
        playStub.mockImplementationOnce(() =>
            Promise.reject(Object.assign(new Error('The play() request was interrupted'), { name: 'AbortError' })));
        const player = usePlayer();

        await expect(player.play(A)).resolves.toBeUndefined();
        expect(player.error.value).toBeNull();
        expect(player.current.value).toBeNull();
    });

    it('volumen acotado a [0,1] y mute independiente', () => {
        const player = usePlayer();
        const m = media() as unknown as { volume: number; muted: boolean };

        player.setVolume(2);
        expect(player.volume.value).toBe(1);
        expect(m.volume).toBe(1);
        player.setVolume(-0.5);
        expect(player.volume.value).toBe(0);
        player.setVolume(0.42);
        expect(player.volume.value).toBe(0.42);
        player.setMuted(true);
        expect(player.muted.value).toBe(true);
        expect(m.muted).toBe(true);
        expect(player.volume.value).toBe(0.42); // el mute no altera el volumen
        player.setMuted(false);
        expect(player.muted.value).toBe(false);
    });

    it('desactivar shuffle restaura el orden base de los pendientes', async () => {
        resolveByTrackId();
        const player = usePlayer();

        await player.play(A);
        await player.enqueue(B);
        await player.enqueue(C);
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]);

        player.toggleShuffle(); // on: permuta pendientes
        expect([...player.upNext.value.map(t => t.id)].sort()).toEqual([2, 3]);

        player.toggleShuffle(); // off: restaura el orden base
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]);
    });

    it('clearQueue conserva la pista en curso; moveEntry y removeEntry operan pendientes', async () => {
        resolveByTrackId();
        const player = usePlayer();

        await player.play(A);
        await player.playNext(B);
        await player.playNext(C);
        expect(player.upNext.value.map(t => t.id)).toEqual([3, 2]);

        const firstId = player.queue.value[0]!.entryId;
        player.moveEntry(firstId, 1);
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]);

        player.removeEntry(player.queue.value[0]!.entryId);
        expect(player.upNext.value.map(t => t.id)).toEqual([3]);

        player.clearQueue();
        expect(player.queue.value).toHaveLength(0);
        expect(player.current.value?.id).toBe(1); // la actual nunca se toca
    });

    it('sin MediaSession el player funciona con degradación limpia', async () => {
        // jsdom no implementa mediaSession: ni metadata ni handlers ni crash.
        resolveByTrackId();
        const player = usePlayer();
        await player.play(A);
        ended(); await flushPromises();
        player.stop();
        expect(player.current.value).toBeNull();
    });

    it('con MediaSession presente: metadatos, playbackState, posición con guard de valores finitos y handlers que delegan', async () => {
        // El binding de MediaSession ocurre una sola vez por módulo (en el
        // primer bindAudioEvents): se importa un módulo fresco CON el stub ya
        // definido, igual que en un webview real donde existe desde el arranque.
        ({ usePlayer } = await import('../../composables/usePlayer'));
        resolveByTrackId();
        const setActionHandler = vi.fn((action: string, handler: (details?: unknown) => void) => { handlers[action] = handler; });
        const setPositionState = vi.fn();
        const handlers: Record<string, (details?: unknown) => void> = {};
        Object.defineProperty(navigator, 'mediaSession', {
            configurable: true,
            value: { setActionHandler, playbackState: 'none' as string, setPositionState, metadata: null },
        });
        class MediaMetadataStub {
            constructor(public init: unknown) {}
        }
        vi.stubGlobal('MediaMetadata', MediaMetadataStub);
        const player = usePlayer();
        const m = media();

        try {
            await player.play(A);
            await flushPromises();
            const ms = (navigator as unknown as { mediaSession: { metadata: MediaMetadataStub | null; playbackState: string; setPositionState: ReturnType<typeof vi.fn> } }).mediaSession;

            // Metadatos al confirmar el inicio.
            expect(ms.metadata).toBeInstanceOf(MediaMetadataStub);
            expect((ms.metadata?.init as { title: string }).title).toBe('One');

            // playbackState (propiedad del estándar) desde el evento 'play'.
            m.dispatchEvent(new Event('play'));
            await flushPromises();
            expect(ms.playbackState).toBe('playing');

            // setPositionState solo con valores finitos: duration NaN → no se llama.
            m.dispatchEvent(new Event('timeupdate'));
            expect(ms.setPositionState).not.toHaveBeenCalled();
            Object.defineProperty(m, 'duration', { configurable: true, value: 180 });
            (m as unknown as { currentTime: number }).currentTime = 10;
            m.dispatchEvent(new Event('timeupdate'));
            expect(ms.setPositionState).toHaveBeenCalledWith({ duration: 180, position: 10, playbackRate: 1 });

            // Handlers registrados y delegación en las acciones del player.
            for (const action of ['play', 'pause', 'previoustrack', 'nexttrack', 'seekto']) {
                expect(handlers[action]).toBeDefined();
            }
            await player.playNext(B);
            handlers['nexttrack']!();
            await flushPromises();
            expect(player.current.value?.id).toBe(2);
            handlers['previoustrack']!();
            await flushPromises();
            expect(player.current.value?.id).toBe(1);

            // stop limpia metadatos y estado.
            player.stop();
            expect(ms.metadata).toBeNull();
            expect(ms.playbackState).toBe('none');
        } finally {
            delete (navigator as unknown as Record<string, unknown>).mediaSession;
            vi.unstubAllGlobals();
        }
    });

    it('tras stop() los handlers de MediaSession se re-registran al volver a reproducir', async () => {
        // Regresión: stop() → clearMediaSession() desregistraba los action
        // handlers y el bind inicial (bindAudioEvents, guard de módulo) no se
        // repetía: PlayPause/Next/Prev por tecla multimedia o D-Bus/MPRIS
        // quedaban muertos hasta reiniciar la app.
        resolveByTrackId();
        const handlers: Record<string, ((details?: unknown) => void) | null> = {};
        Object.defineProperty(navigator, 'mediaSession', {
            configurable: true,
            value: {
                setActionHandler: vi.fn((action: string, handler: ((details?: unknown) => void) | null) => {
                    handlers[action] = handler;
                }),
                playbackState: 'none' as string,
                metadata: null,
            },
        });
        const player = usePlayer();
        const actions = ['play', 'pause', 'previoustrack', 'nexttrack', 'seekto'] as const;

        try {
            await player.play(A);
            await flushPromises();
            for (const action of actions) {
                expect(typeof handlers[action]).toBe('function');
            }

            player.stop();
            for (const action of actions) {
                expect(handlers[action]).toBeNull();
            }

            await player.play(B);
            await flushPromises();
            for (const action of actions) {
                expect(typeof handlers[action]).toBe('function');
            }
        } finally {
            delete (navigator as unknown as Record<string, unknown>).mediaSession;
        }
    });
});
