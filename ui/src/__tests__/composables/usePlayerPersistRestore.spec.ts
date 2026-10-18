/**
 * Fase 1 — persistencia (`syncify.playerState.v1`) y restauración perezosa,
 * sobre el motor nativo.
 *
 * Verifica que la clave versionada nunca contiene `file_path`, URLs ni el DTO
 * completo; que el estado corrupto se descarta completo; que la restauración
 * re-resuelve la fuente (jamás reusa rutas ni concesiones), carga el archivo
 * PAUSADO en la posición guardada y solo valida la pista actual (los
 * pendientes se validan justo antes de sonar); y que importar el módulo no
 * dispara invokes ni reproducción.
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

type TauriModule = typeof import('../../api/tauri');
type PlayerModule = typeof import('../../api/player');
type UsePlayerModule = typeof import('../../composables/usePlayer');

let tauriApi: TauriModule;
let playerApi: PlayerModule;
let mod: UsePlayerModule | null = null;

const KEY = 'syncify.playerState.v1';
const VALID_UUID = 'a3bb189e-8bf9-4888-9912-ace4e6543002';
const VALID_UUID_2 = 'c1a0f4e2-77aa-4c31-9d20-5b6f0a1e8890';

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
    engine().onPlayerEnded.mockReset().mockResolvedValue(() => {});
}

/** Un tick del ticker: avanza el reloj 250 ms y entrega la posición. */
function tick(positionSec: number): void {
    vi.advanceTimersByTime(250);
    tickCb?.({ seq, state: 'playing', positionSec, durationSec: 200 });
}

/** Re-importa el módulo bajo test: estado del singleton y guard de restauración frescos. */
async function freshPlayer(): Promise<UsePlayerModule> {
    vi.resetModules();
    // Reimportar los módulos mockeados para enlazar con las mismas instancias
    // que verá el módulo bajo test tras el reset.
    tauriApi = await import('../../api/tauri');
    playerApi = await import('../../api/player');
    mockInvoke().mockReset();
    configureEngineMocks();
    mod = await import('../../composables/usePlayer');
    return mod;
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

describe('usePlayer — persistencia y restauración (fase 1)', () => {
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

    it('restaura track/cola/posición/modos PAUSADO, proyectando sin file_path', async () => {
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
        const player = (await freshPlayer()).usePlayer();
        mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
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
            return Promise.resolve(null);
        });

        await player.restoreFromPersistence();
        await flushPromises();

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

    it('reanudar tras restaurar arranca una sesión nueva: el acumulado empieza en 0', async () => {
        seedState({ currentTrackId: 7, positionSec: 120 });
        const player = (await freshPlayer()).usePlayer();
        mockInvoke().mockImplementation((cmd: string) => {
            if (cmd === 'get_track_metadata') {
                return Promise.resolve({ track_id: 7, title: 'Song', artist_name: 'Artist', album_name: null, cover_art_url: null, file_path: '/lib/a.flac' });
            }
            if (cmd === 'record_local_play') return Promise.resolve({ track_id: 7, play_count: 1, last_played: null, counted: true });
            return Promise.resolve(null);
        });

        await player.restoreFromPersistence();
        await flushPromises();
        mockInvoke().mockClear();

        // El usuario reanuda: el estado pasa a 'playing'.
        player.toggle();
        await flushPromises();
        expect(engine().playerResume).toHaveBeenCalled();
        expect(player.isPlaying.value).toBe(true);

        // Pese a posición 120: 119 ticks → 29,75 s → nada.
        tick(120);
        for (let i = 1; i <= 119; i++) tick(120 + i * 0.25);
        await flushPromises();
        expect(mockInvoke().mock.calls.filter(c => c[0] === 'record_local_play')).toHaveLength(0);
        tick(150);
        await flushPromises();
        const calls = mockInvoke().mock.calls.filter(c => c[0] === 'record_local_play');
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
        const player = (await freshPlayer()).usePlayer();
        mockInvoke().mockImplementation((cmd: string) => {
            if (cmd === 'get_track_metadata') return Promise.reject(new Error('El track 9 no existe en la biblioteca'));
            return Promise.resolve(null);
        });

        await player.restoreFromPersistence();
        await flushPromises();

        expect(player.current.value).toBeNull();
        expect(player.queue.value.map(e => e.trackId)).toEqual([2]);
        expect(engine().playerLoadTrack).not.toHaveBeenCalled();
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
            expect(mockInvoke()).not.toHaveBeenCalled();
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
        expect(mockInvoke().mock.calls.some(c => c[0] === 'record_local_play')).toBe(false);
    });

    it('la clave persistida nunca contiene file_path, URLs de archivo ni el DTO completo', async () => {
        const player = (await freshPlayer()).usePlayer();
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
        const player = (await freshPlayer()).usePlayer();
        mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'get_track_metadata') {
                if (args?.trackId === 999) return Promise.reject(new Error('El track 999 no existe en la biblioteca'));
                return Promise.resolve({ track_id: 5, title: 'Five', artist_name: 'Artist', album_name: null, cover_art_url: null, file_path: '/lib/5.flac' });
            }
            return Promise.resolve(null);
        });

        await player.restoreFromPersistence();
        await flushPromises();
        mockInvoke().mockClear();

        // play() sin argumento arranca la primera pendiente de la cola restaurada.
        await player.play();
        await flushPromises();

        const tracked = mockInvoke().mock.calls.map(c => `${c[0]}:${(c[1] as Record<string, unknown> | undefined)?.trackId}`);
        expect(tracked).toEqual(['get_track_metadata:999', 'get_track_metadata:5']);
        expect(engine().playerLoadTrack).toHaveBeenLastCalledWith(5, true, 1);
        expect(player.current.value?.id).toBe(5);
        expect(player.queue.value).toHaveLength(0);
    });

    it('visibilitychange (hidden) persiste el estado', async () => {
        const player = (await freshPlayer()).usePlayer();
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
});
