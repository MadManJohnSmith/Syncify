/**
 * #131 (P00) — persistencia en el BACKEND (SQLite, `api/playbackQueue.ts`) y
 * restauración perezosa del reproductor.
 *
 * Desde #131 la cola, el historial, la pista actual, la posición, los modos y
 * el volumen viven en SQLite y la UI es una vista sobre los comandos
 * `playback_queue_*` / `playback_state_*`. Este spec fija que:
 *   1. importar el módulo no dispara invokes ni reproducción (restauración
 *      perezosa);
 *   2. la restauración lee `playback_queue_get`, repone cola/historial/modos
 *      y carga la pista PAUSADA en la posición guardada, re-resolviendo la
 *      fuente con `player_load_track` (jamás reusa rutas ni concesiones);
 *   3. la clave antigua `syncify.playerState.v1` se DESCARTA sin migración y
 *      la UI nunca vuelve a escribir localStorage de reproducción;
 *   4. cada mutación del agregado va a SU comando (enqueue con identidad del
 *      backend, set_current al avanzar, reset en stop, modos y volumen);
 *   5. la pista actual desaparecida arranca sin current conservando la cola,
 *      y el pendiente desaparecido se descarta al vuelo al sonar.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

vi.mock('../../api/player', () => ({
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
    onPlayerAdvanced: vi.fn(),
    playerMprisUpdate: vi.fn(() => Promise.resolve()),
    onMprisAction: vi.fn(() => Promise.resolve()),
}));

type TauriModule = typeof import('../../api/tauri');
type PlayerModule = typeof import('../../api/player');
type UsePlayerModule = typeof import('../../composables/usePlayer');

let tauriApi: TauriModule;
let playerApi: PlayerModule;
let mod: UsePlayerModule | null = null;

const LEGACY_KEY = 'syncify.playerState.v1';

let seq = 0;
let tickCb: ((tick: import('../../api/player').PlayerTick) => void) | null = null;

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
    engine().onPlayerAdvanced.mockReset().mockResolvedValue(() => {});
    engine().onPlayerEnded.mockReset().mockResolvedValue(() => {});
}

/** Un tick del ticker: avanza el reloj 250 ms y entrega la posición. */
function tick(positionSec: number): void {
    vi.advanceTimersByTime(250);
    tickCb?.({ seq, state: 'playing', positionSec, durationSec: 200 });
}

/** Instantánea que devuelve `playback_queue_get` en los tests. */
function snapshot(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        pending: [],
        history: [],
        current: null,
        position_sec: 0,
        shuffle: false,
        repeat: 'off',
        volume: 1,
        muted: false,
        ...overrides,
    };
}

/** Instala un mock de invokeCommand que responde los comandos del agregado. */
function backend(overrides: Record<string, (args?: Record<string, unknown>) => unknown> = {}): void {
    mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
        if (overrides[cmd]) return Promise.resolve(overrides[cmd](args));
        if (cmd === 'playback_queue_get') return Promise.resolve(snapshot());
        if (cmd === 'playback_queue_enqueue') return Promise.resolve([] as string[]);
        if (cmd === 'get_track_metadata') {
            const trackId = (args?.trackId as number) ?? 0;
            return Promise.resolve({
                track_id: trackId,
                title: `Song ${trackId}`,
                artist_name: 'Artist',
                album_name: null,
                cover_art_url: null,
                file_path: `/lib/${trackId}.flac`,
                sources: [],
            });
        }
        return Promise.resolve(undefined);
    });
}

/** Re-importa el módulo bajo test: estado del singleton y guard de restauración frescos. */
async function freshPlayer(): Promise<UsePlayerModule> {
    vi.resetModules();
    // Reimportar los módulos mockeados para enlazar con las mismas instancias
    // que verá el módulo bajo test tras el reset.
    tauriApi = await import('../../api/tauri');
    playerApi = await import('../../api/player');
    // mockClear (no mockReset): conserva la implementación que el test haya
    // instalado con backend() antes de pedir el módulo fresco.
    mockInvoke().mockClear();
    configureEngineMocks();
    mod = await import('../../composables/usePlayer');
    return mod;
}

describe('usePlayer — persistencia en el backend y restauración (#131)', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        localStorage.clear();
    });
    afterEach(() => {
        vi.useRealTimers();
        tickCb = null;
        mod = null;
    });

    it('importar el módulo no dispara invokes ni reproducción (restauración perezosa)', async () => {
        await freshPlayer();
        expect(mockInvoke()).not.toHaveBeenCalled();
        expect(engine().playerLoadTrack).not.toHaveBeenCalled();
    });

    it('restaura track/cola/posición/modos desde playback_queue_get, PAUSADO y sin file_path', async () => {
        backend();
        const player = (await freshPlayer()).usePlayer();
        mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'playback_queue_get') {
                return Promise.resolve(snapshot({
                    pending: [
                        { entry_id: 'e-2', track_id: 2, kind: 'pending', position: 0 },
                        { entry_id: 'e-3', track_id: 3, kind: 'pending', position: 1 },
                    ],
                    history: [{ entry_id: 'e-1', track_id: 1, kind: 'history', position: 0 }],
                    current: { entry_id: 'e-cur', track_id: 7, kind: 'current', position: 0 },
                    position_sec: 42.5,
                    shuffle: true,
                    repeat: 'all',
                    volume: 0.5,
                    muted: true,
                }));
            }
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
            return Promise.resolve(undefined);
        });

        await player.restoreFromPersistence();
        await flushPromises();

        expect(mockInvoke()).toHaveBeenCalledWith('playback_queue_get');
        expect(mockInvoke()).toHaveBeenCalledWith('get_track_metadata', { trackId: 7 });
        // Cargado en PAUSA (autoplay=false) y con el volumen efectivo (muted → 0).
        expect(engine().playerLoadTrack).toHaveBeenCalledWith(7, false, 0);
        expect(engine().playerSeek).toHaveBeenCalledWith(42.5);
        // Proyección estricta: sin file_path ni DTO completo.
        expect(player.current.value).toEqual({
            id: 7,
            title: 'Song',
            artist: 'Artist',
            album: 'Album',
            coverUrl: 'https://cdn.test/a.jpg',
        });
        expect(Object.keys(player.current.value ?? {})).not.toContain('file_path');
        // La identidad de la pista actual es la PERSISTIDA, no una nueva.
        expect(player.positionSec.value).toBe(42.5);
        expect(player.durationSec.value).toBe(200);
        expect(player.isPlaying.value).toBe(false);
        expect(engine().playerResume).not.toHaveBeenCalled(); // restaurar NO reproduce
        expect(player.shuffle.value).toBe(true);
        expect(player.repeat.value).toBe('all');
        expect(player.volume.value).toBe(0.5);
        expect(player.muted.value).toBe(true);
        expect(player.queue.value.map(e => e.trackId)).toEqual([2, 3]);
        expect(player.history.value.map(e => e.trackId)).toEqual([1]);
    });

    it('la clave antigua syncify.playerState.v1 se descarta sin migrar y la UI no escribe localStorage de reproducción', async () => {
        backend();
        localStorage.setItem(LEGACY_KEY, JSON.stringify({ version: 1, queue: [{ entryId: 'e', trackId: 1 }] }));
        const setItemSpy = vi.spyOn(Storage.prototype, 'setItem');
        const player = (await freshPlayer()).usePlayer();

        await player.restoreFromPersistence();
        await flushPromises();

        // Limpieza: la clave antigua desaparece y no queda rastro de estado de
        // reproducción en localStorage.
        expect(localStorage.getItem(LEGACY_KEY)).toBeNull();

        // Un ciclo completo de uso no escribe NADA de reproducción.
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        await player.enqueue({ id: 8, title: 'Other', artist: 'Artist' });
        await player.playNext({ id: 9, title: 'Next', artist: 'Artist' });
        player.setVolume(0.4);
        player.setMuted(true);
        player.toggleShuffle();
        player.cycleRepeat();
        player.clearQueue();
        player.stop();
        await flushPromises();

        expect(setItemSpy).not.toHaveBeenCalled();
        expect(localStorage.getItem(LEGACY_KEY)).toBeNull();
        setItemSpy.mockRestore();
    });

    it('enqueue adopta la identidad del backend y set_current acompaña cada avance', async () => {
        backend();
        const player = (await freshPlayer()).usePlayer();

        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        await flushPromises();
        mockInvoke().mockClear();
        backend();
        mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'playback_queue_enqueue') {
                return Promise.resolve(['be-1']);
            }
            if (cmd === 'playback_queue_set_current') return Promise.resolve(undefined);
            if (cmd === 'get_track_metadata') {
                return Promise.resolve({ track_id: args?.trackId, title: 'S', artist_name: 'A', album_name: null, cover_art_url: null, file_path: '/lib/x.flac', sources: [] });
            }
            return Promise.resolve(undefined);
        });

        const queued = await player.enqueue({ id: 8, title: 'Other', artist: 'Artist' });
        await flushPromises();
        expect(queued).toBe('queued');
        expect(mockInvoke()).toHaveBeenCalledWith('playback_queue_enqueue', { trackIds: [8], atFront: false });
        // La entrada local usa la identidad DEVUELTA por el backend.
        expect(player.queue.value.map(e => e.entryId)).toEqual(['be-1']);

        // next(): consume la cabeza y el backend hace la transición atómica.
        player.next();
        await flushPromises();
        expect(mockInvoke()).toHaveBeenCalledWith('playback_queue_set_current', { entryId: 'be-1', trackId: 8 });
        expect(player.current.value?.id).toBe(8);
        expect(player.queue.value).toHaveLength(0);
    });

    it('stop vuelca el agregado (playback_state_reset) y stop conserva modos en el backend', async () => {
        backend();
        const player = (await freshPlayer()).usePlayer();
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        await flushPromises();
        mockInvoke().mockClear();

        player.stop();
        await flushPromises();
        expect(mockInvoke()).toHaveBeenCalledWith('playback_state_reset');
        expect(player.current.value).toBeNull();
        expect(player.queue.value).toHaveLength(0);
    });

    it('modos y volumen persisten con playback_state_set_modes / _set_volume', async () => {
        backend();
        const player = (await freshPlayer()).usePlayer();
        mockInvoke().mockClear();

        player.toggleShuffle();
        player.cycleRepeat();
        player.setVolume(0.25);
        player.setMuted(true);
        await flushPromises();

        const modes = mockInvoke().mock.calls.filter(c => c[0] === 'playback_state_set_modes');
        expect(modes).toHaveLength(2);
        expect(modes[0]?.[1]).toEqual({ shuffle: true, repeat: 'off' });
        expect(modes[1]?.[1]).toEqual({ shuffle: true, repeat: 'all' });
        expect(mockInvoke()).toHaveBeenCalledWith('playback_state_set_volume', { volume: 0.25, muted: false });
        expect(mockInvoke()).toHaveBeenCalledWith('playback_state_set_volume', { volume: 0.25, muted: true });
    });

    it('visibilitychange (hidden) persiste la posición en el backend', async () => {
        backend();
        const player = (await freshPlayer()).usePlayer();
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        await flushPromises();
        mockInvoke().mockClear();

        Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => 'hidden' });
        document.dispatchEvent(new Event('visibilitychange'));
        await flushPromises();
        try {
            expect(mockInvoke()).toHaveBeenCalledWith('playback_state_set_position', { positionSec: 0 });
        } finally {
            delete (document as unknown as Record<string, unknown>).visibilityState;
        }
    });

    it('la UI no cronometra (#132): los ticks de posición jamás invocan record_local_play', async () => {
        backend({
            playback_queue_get: () => Promise.resolve(snapshot({
                current: { entry_id: 'e-cur', track_id: 7, kind: 'current', position: 0 },
                position_sec: 120,
            })),
        });
        const player = (await freshPlayer()).usePlayer();

        await player.restoreFromPersistence();
        await flushPromises();
        mockInvoke().mockClear();

        // El usuario reanuda: el estado pasa a 'playing'.
        player.toggle();
        await flushPromises();
        expect(engine().playerResume).toHaveBeenCalled();
        expect(player.isPlaying.value).toBe(true);

        // 200 ticks (50 s simulados) sobre la posición restaurada: la
        // acreditación es del BACKEND (P01) — la UI no llama al comando
        // (que ya ni siquiera existe como IPC).
        tick(120);
        for (let i = 1; i <= 200; i++) tick(120 + i * 0.25);
        await flushPromises();
        expect(mockInvoke().mock.calls.filter(c => c[0] === 'record_local_play')).toHaveLength(0);
    });

    it('la pista actual desaparecida arranca sin current conservando la cola', async () => {
        backend({
            playback_queue_get: () => Promise.resolve(snapshot({
                pending: [{ entry_id: 'e-2', track_id: 2, kind: 'pending', position: 0 }],
                current: { entry_id: 'e-cur', track_id: 9, kind: 'current', position: 0 },
                position_sec: 10,
            })),
            get_track_metadata: () => Promise.reject(new Error('El track 9 no existe en la biblioteca')),
        });
        const player = (await freshPlayer()).usePlayer();

        await player.restoreFromPersistence();
        await flushPromises();

        expect(player.current.value).toBeNull();
        expect(player.queue.value.map(e => e.trackId)).toEqual([2]);
        expect(engine().playerLoadTrack).not.toHaveBeenCalled();
    });

    it('fallo de IPC al leer la instantánea deja el reproductor limpio, sin bloquear', async () => {
        backend({ playback_queue_get: () => Promise.reject(new Error('IPC caído')) });
        const player = (await freshPlayer()).usePlayer();

        await expect(player.restoreFromPersistence()).resolves.toBeUndefined();
        expect(player.current.value).toBeNull();
        expect(player.queue.value).toHaveLength(0);
        expect(engine().playerLoadTrack).not.toHaveBeenCalled();
    });

    it('validación perezosa: el pendiente desaparecido se descarta al vuelo y suena el siguiente', async () => {
        backend({
            playback_queue_get: () => Promise.resolve(snapshot({
                pending: [
                    { entry_id: 'e-999', track_id: 999, kind: 'pending', position: 0 },
                    { entry_id: 'e-5', track_id: 5, kind: 'pending', position: 1 },
                ],
            })),
            get_track_metadata: (args?: Record<string, unknown>) => {
                if (args?.trackId === 999) return Promise.reject(new Error('El track 999 no existe en la biblioteca'));
                return Promise.resolve({ track_id: 5, title: 'Five', artist_name: 'Artist', album_name: null, cover_art_url: null, file_path: '/lib/5.flac', sources: [] });
            },
        });
        const player = (await freshPlayer()).usePlayer();

        await player.restoreFromPersistence();
        await flushPromises();
        mockInvoke().mockClear();

        // play() sin argumento arranca la primera pendiente de la cola restaurada.
        await player.play();
        await flushPromises();

        const tracked = mockInvoke().mock.calls.map(c => `${c[0]}:${(c[1] as Record<string, unknown> | undefined)?.trackId}`);
        // La cabeza muerta se valida y descarta al vuelo; la siguiente suena
        // y el avance persiste con playback_queue_set_current (transición
        // atómica: cabeza consumida, current anterior → historial).
        expect(tracked).toEqual(['get_track_metadata:999', 'get_track_metadata:5', 'playback_queue_set_current:5']);
        expect(engine().playerLoadTrack).toHaveBeenLastCalledWith(5, true, 1);
        expect(player.current.value?.id).toBe(5);
        expect(player.queue.value).toHaveLength(0);
    });
});
