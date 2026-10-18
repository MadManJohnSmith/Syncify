/**
 * Fase 1 — cola (identidades, replaceQueueAndPlay, reordenación), modos
 * repeat/shuffle, next/previous, fallos de avance y carreras resolver–stop,
 * sobre el motor nativo.
 *
 * El transporte del motor (`api/player`) está mockeado: los ticks y el
 * `ended` llegan por las suscripciones capturadas, como desde el backend.
 * Cada test reimporta el módulo bajo test (estado del singleton fresco).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

vi.mock('../../api/player', () => ({
    playerLoad: vi.fn(),
    playerLoadTrack: vi.fn(),
    playerPause: vi.fn(),
    playerResume: vi.fn(),
    playerStop: vi.fn(),
    playerSeek: vi.fn(),
    playerSetVolume: vi.fn(),
    playerSetRate: vi.fn(),
    onPlayerTick: vi.fn(),
    onPlayerStreamState: vi.fn(),
    onPlayerEnded: vi.fn(),
    playerMprisUpdate: vi.fn(() => Promise.resolve()),
    onMprisAction: vi.fn(() => Promise.resolve()),
}));

const A = { id: 1, title: 'One', artist: 'X' };
const B = { id: 2, title: 'Two', artist: 'X' };
const C = { id: 3, title: 'Three', artist: 'X' };

type TauriModule = typeof import('../../api/tauri');
type PlayerModule = typeof import('../../api/player');
type UsePlayerModule = typeof import('../../composables/usePlayer');

let tauriApi: TauriModule;
let playerApi: PlayerModule;
let usePlayer: UsePlayerModule['usePlayer'];

let seq = 0;
let tickCb: ((tick: import('../../api/player').PlayerTick) => void) | null = null;
let endedCb: ((seq: number) => void) | null = null;

function mockInvoke(): ReturnType<typeof vi.mocked<TauriModule['invokeCommand']>> {
    return vi.mocked(tauriApi.invokeCommand);
}

function engine(): ReturnType<typeof vi.mocked<PlayerModule>> {
    return vi.mocked(playerApi);
}

function configureEngineMocks(): void {
    seq = 0;
    engine().playerLoadTrack.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 200, stream: null }));
    engine().playerPause.mockReset().mockResolvedValue(undefined);
    engine().playerResume.mockReset().mockResolvedValue(undefined);
    engine().playerStop.mockReset().mockResolvedValue(undefined);
    engine().playerSeek.mockReset().mockResolvedValue(undefined);
    engine().playerSetVolume.mockReset().mockResolvedValue(undefined);
    engine().playerSetRate.mockReset().mockResolvedValue(undefined);
    engine().onPlayerTick.mockReset().mockImplementation((cb) => {
        tickCb = cb;
        return Promise.resolve(() => {});
    });
    engine().onPlayerStreamState.mockReset().mockResolvedValue(() => {});
    engine().onPlayerEnded.mockReset().mockImplementation((cb) => {
        endedCb = cb;
        return Promise.resolve(() => {});
    });
}

function tick(positionSec: number): void {
    tickCb?.({ seq, state: 'playing', positionSec, durationSec: 200 });
}

function ended(): void {
    endedCb?.(seq);
}

function resolveByTrackId(): void {
    mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
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

/** Una carga del motor por pista: `player_load_track` decide local o streaming. */
function resolveCalls(): number[] {
    return engine().playerLoadTrack.mock.calls.map(c => c[0] as number);
}

describe('usePlayer — cola, modos, next/previous y carreras (fase 1)', () => {
    beforeEach(async () => {
        localStorage.clear();
        vi.resetModules();
        // Reimportar los módulos mockeados para enlazar con las mismas
        // instancias que verá el módulo bajo test tras el reset.
        tauriApi = await import('../../api/tauri');
        playerApi = await import('../../api/player');
        mockInvoke().mockReset();
        configureEngineMocks();
        ({ usePlayer } = await import('../../composables/usePlayer'));
        usePlayer().stop();
        mockInvoke().mockClear();
    });
    afterEach(() => {
        tickCb = null;
        endedCb = null;
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

        // Cola agotada + repeat-one: reinicia B en la posición 0 y nueva resolución.
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

        await player.play(A);
        await player.playNext(B);
        ended(); await flushPromises(); // current B, history [A]

        player.previous();
        await flushPromises();
        // positionSec 0 (≤ 3 s) → pop del historial: suena A y B vuelve al frente.
        expect(player.current.value?.id).toBe(1);
        expect(player.upNext.value.map(t => t.id)).toEqual([2]);

        // Posición > 3 s → seek(0), sin nueva resolución.
        tick(5);
        player.seek(5);
        const before = resolveCalls().length;
        player.previous();
        await flushPromises();
        expect(player.positionSec.value).toBe(0);
        expect(player.current.value?.id).toBe(1);
        expect(resolveCalls().length).toBe(before);
        expect(engine().playerSeek).toHaveBeenLastCalledWith(0);
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
        mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_track_metadata') {
                return Promise.resolve({ track_id: args?.trackId, title: `Track ${args?.trackId}`, artist_name: 'X', album_name: null, cover_art_url: null, file_path: null });
            }
            return Promise.resolve(null);
        });
        engine().playerLoadTrack.mockImplementation((trackId: number) => {
            if (trackId === 2) {
                return Promise.reject(new Error('No se pudo resolver ninguna fuente reproducible: qobuz: sin cuenta conectada'));
            }
            return Promise.resolve({ seq: ++seq, durationSec: 200, stream: null });
        });
        const player = usePlayer();

        await player.play(A);
        await player.playNext(B);
        ended(); await flushPromises();

        expect(player.error.value).toContain('ninguna fuente reproducible');
        expect(player.current.value?.id).toBe(1); // la actual no se toca
        expect(player.upNext.value.map(t => t.id)).toEqual([2]); // la cabeza se conserva
    });

    it('carrera carga–stop: sin error y sin current', async () => {
        let release!: (v: import('../../api/player').PlayerTrackLoadResult) => void;
        engine().playerLoadTrack.mockImplementationOnce(() =>
            new Promise<import('../../api/player').PlayerTrackLoadResult>(res => { release = res; }));
        const player = usePlayer();

        const pending = player.play(A);
        player.stop();
        release({ seq: 1, durationSec: 100, stream: null });
        await pending;
        await flushPromises();

        expect(player.error.value).toBeNull();
        expect(player.current.value).toBeNull();
    });

    it('carrera playerLoadTrack–stop: la carga que llega tarde no toca el estado', async () => {
        resolveByTrackId();
        let releaseLoad!: (v: import('../../api/player').PlayerTrackLoadResult) => void;
        engine().playerLoadTrack.mockImplementationOnce(() =>
            new Promise<import('../../api/player').PlayerTrackLoadResult>(res => { releaseLoad = res; }));
        const player = usePlayer();

        const pending = player.play(A);
        await flushPromises(); // carga pendiente
        player.stop();
        releaseLoad({ seq: 99, durationSec: 100, stream: null });
        await pending;
        await flushPromises();

        expect(player.error.value).toBeNull();
        expect(player.current.value).toBeNull();
        expect(player.isPlaying.value).toBe(false);
    });

    it('volumen acotado a [0,1] y mute independiente', () => {
        const player = usePlayer();

        player.setVolume(2);
        expect(player.volume.value).toBe(1);
        expect(engine().playerSetVolume).toHaveBeenLastCalledWith(1);
        player.setVolume(-0.5);
        expect(player.volume.value).toBe(0);
        expect(engine().playerSetVolume).toHaveBeenLastCalledWith(0);
        player.setVolume(0.42);
        expect(player.volume.value).toBe(0.42);
        expect(engine().playerSetVolume).toHaveBeenLastCalledWith(0.42);
        player.setMuted(true);
        expect(player.muted.value).toBe(true);
        expect(engine().playerSetVolume).toHaveBeenLastCalledWith(0); // el mute envía 0
        expect(player.volume.value).toBe(0.42); // el mute no altera el volumen
        player.setMuted(false);
        expect(player.muted.value).toBe(false);
        expect(engine().playerSetVolume).toHaveBeenLastCalledWith(0.42);
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

        try {
            await player.play(A);
            await flushPromises();
            const ms = (navigator as unknown as { mediaSession: { metadata: MediaMetadataStub | null; playbackState: string; setPositionState: ReturnType<typeof vi.fn> } }).mediaSession;

            // Metadatos al confirmar el inicio.
            expect(ms.metadata).toBeInstanceOf(MediaMetadataStub);
            expect((ms.metadata?.init as { title: string }).title).toBe('One');
            expect(ms.playbackState).toBe('playing');

            // setPositionState solo con valores finitos: sin tick con duración → no se llama.
            expect(ms.setPositionState).not.toHaveBeenCalled();
            tick(10); // el tick entrega duración 200
            expect(ms.setPositionState).toHaveBeenCalledWith({ duration: 200, position: 10, playbackRate: 1 });

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
        // handlers y el bind inicial (bindEngineEvents, guard de módulo) no se
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
