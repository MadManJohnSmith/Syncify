/**
 * Fase 1 — acreditación de escucha (30 s de reproducción real) y política de
 * reintentos de `record_local_play`, sobre el motor nativo.
 *
 * jsdom no reproduce audio: el transporte del motor (`api/player`) está
 * mockeado y los ticks de posición se entregan por la suscripción capturada
 * `onPlayerTick`, avanzando el reloj (fake timers) y la posición a la vez —
 * exactamente como el ticker de 250 ms del backend.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

vi.mock('../../api/player', () => ({
    playerLoad: vi.fn(),
    playerPause: vi.fn(),
    playerResume: vi.fn(),
    playerStop: vi.fn(),
    playerSeek: vi.fn(),
    playerSetVolume: vi.fn(),
    playerSetRate: vi.fn(),
    onPlayerTick: vi.fn(),
    onPlayerEnded: vi.fn(),
    playerMprisUpdate: vi.fn(() => Promise.resolve()),
    onMprisAction: vi.fn(() => Promise.resolve()),
}));

import { invokeCommand } from '../../api/tauri'
import * as playerApi from '../../api/player'
import type { PlayerTick } from '../../api/player'
import { usePlayer } from '../../composables/usePlayer'

const mockInvoke = vi.mocked(invokeCommand);
const engine = vi.mocked(playerApi);

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

let seq = 0;
let tickCb: ((tick: PlayerTick) => void) | null = null;
let endedCb: ((seq: number) => void) | null = null;

function configureEngineMocks(): void {
    seq = 0;
    engine.playerLoad.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 200 }));
    engine.playerPause.mockReset().mockResolvedValue(undefined);
    engine.playerResume.mockReset().mockResolvedValue(undefined);
    engine.playerStop.mockReset().mockResolvedValue(undefined);
    engine.playerSeek.mockReset().mockResolvedValue(undefined);
    engine.playerSetVolume.mockReset().mockResolvedValue(undefined);
    engine.playerSetRate.mockReset().mockResolvedValue(undefined);
    engine.onPlayerTick.mockReset().mockImplementation((cb) => {
        tickCb = cb;
        return Promise.resolve(() => {});
    });
    engine.onPlayerEnded.mockReset().mockImplementation((cb) => {
        endedCb = cb;
        return Promise.resolve(() => {});
    });
}

/** Un tick del ticker: avanza el reloj (fake timers) y la posición, y lo entrega. */
function tick(positionSec: number, state: PlayerTick['state'] = 'playing', wallMs = 250): void {
    vi.advanceTimersByTime(wallMs);
    tickCb?.({ seq, state, positionSec, durationSec: 200 });
}

function ended(): void {
    endedCb?.(seq);
}

function recordCalls(): Array<{ args?: Record<string, unknown> }> {
    return mockInvoke.mock.calls
        .filter(c => c[0] === 'record_local_play')
        .map(c => ({ args: c[1] as Record<string, unknown> | undefined }));
}

function mockHappyPath(): void {
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
        if (cmd === 'resolve_playback_source') {
            return Promise.resolve({ track_id: args?.trackId, file_path: `/lib/${args?.trackId}.flac`, format: 'FLAC' });
        }
        if (cmd === 'record_local_play') {
            return Promise.resolve({ track_id: args?.trackId, play_count: 1, last_played: '2026-10-06T00:00:00Z', counted: true });
        }
        return Promise.resolve(null);
    });
}

describe('usePlayer — acreditación de escucha (30 s reales)', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        mockInvoke.mockReset();
        localStorage.clear();
        configureEngineMocks();
        mockHappyPath();
        usePlayer().stop();
        // El rate es estado persistente del módulo (stop() no lo toca): cada
        // test arranca a 1× o el anterior lo contaminaría.
        usePlayer().setRate(1);
        mockInvoke.mockClear();
    });
    afterEach(() => {
        vi.useRealTimers();
    });

    /** Reproduce la pista base y limpia el registro de comandos previo. */
    async function startTrack(trackId = 7): Promise<ReturnType<typeof usePlayer>> {
        const player = usePlayer();
        await player.play({ id: trackId, title: 'Song', artist: 'Artist' });
        mockInvoke.mockClear();
        return player;
    }

    it('29,75 s acreditados no cuentan; 30,0 s sí (acumulación por tick, no posición)', async () => {
        const player = await startTrack(7);

        tick(0); // primer tick elegible: solo re-ancla, acredita 0
        let t = 0;
        for (let i = 0; i < 119; i++) { t += 0.25; tick(t); }
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,75 s acreditados

        tick(30); // 30,0 s acreditados
        await flushPromises();
        const calls = recordCalls();
        expect(calls).toHaveLength(1);
        expect(calls[0].args?.trackId).toBe(7);
        expect(calls[0].args?.listenSessionId).toMatch(UUID_RE);
        // El mock confirma: el pendiente se retira.
        expect(player.pendingListens.value).toHaveLength(0);
    });

    it('las pausas suspenden la acreditación sin borrar el acumulado', async () => {
        await startTrack(7);

        tick(0);
        let t = 0;
        for (let i = 0; i < 40; i++) { t += 0.25; tick(t); }        // 10 s
        for (let i = 0; i < 8; i++) { t += 0.25; tick(t, 'paused'); } // 2 s en pausa: nada
        for (let i = 0; i < 79; i++) { t += 0.25; tick(t); }        // 19,5 s (el primero tras reanudar solo re-ancla)
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,5 acreditados (la pausa no acreditó ni borró)
        tick(32);      // +0,25 → 29,75
        tick(32.25);   // +0,25 → 30,0
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
    });

    it('los estados no reproducibles (stopped) suspenden sin borrar', async () => {
        await startTrack(7);

        tick(0);
        let t = 0;
        for (let i = 0; i < 40; i++) { t += 0.25; tick(t); }         // 10 s
        for (let i = 0; i < 12; i++) { t += 0.25; tick(t, 'stopped'); } // 3 s parado: nada
        for (let i = 0; i < 79; i++) { t += 0.25; tick(t); }         // 19,5 s (el primero tras el parón solo re-ancla)
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,5 acreditados
        tick(35);      // el medio quedó en 34,75: +0,25 → 29,75
        tick(35.25);   // +0,25 → 30,0
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
    });

    it('playbackRate 2× acredita la mitad del progreso de medio por segundo de reloj', async () => {
        const player = await startTrack(7);
        player.setRate(2);

        tick(0);
        let t = 0;
        // 0,5 s de reloj por tick; el medio avanza 1 s por tick → acredita 0,5.
        for (let i = 0; i < 59; i++) { t += 1; tick(t, 'playing', 500); }
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,5 s acreditados con el medio en 59 s
        tick(60, 'playing', 500);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1); // 30 s de reloj → 60 s de medio
    });

    it('los seeks repetidos de Lyrics no fabrican ni pierden escucha', async () => {
        await startTrack(7);

        tick(0);
        let pos = 0;
        for (let i = 0; i < 40; i++) { pos += 0.25; tick(pos); } // 10 s
        // Seek atrás (línea anterior): Δmedia negativo → el tick no acredita.
        tick(5);
        await flushPromises();
        expect(recordCalls()).toHaveLength(0);
        // Desde 5 s: 19,75 s de reloj más → 29,5 acreditados en total.
        pos = 5;
        for (let i = 0; i < 79; i++) { pos += 0.25; tick(pos); }
        await flushPromises();
        expect(recordCalls()).toHaveLength(0);
        tick(pos + 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
    });

    it('nunca usa la posición >= 30 como criterio: un salto a 100 s no acredita', async () => {
        await startTrack(7);

        tick(0);
        for (let i = 0; i < 8; i++) tick((i + 1) * 0.25); // 2 s
        tick(100);
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // ~2,25 s acreditados pese a la posición 100
    });

    it('pausa, seek y rate conservan la sesión: una sola acreditación por sesión', async () => {
        const player = await startTrack(7);

        tick(0);
        for (let i = 0; i < 120; i++) tick((i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);

        // Pausa, seek atrás y cambio de velocidad: la sesión sigue.
        tick(30.25, 'paused');
        tick(10);
        player.setRate(1.5);
        let pos = 10;
        for (let i = 0; i < 121; i++) { pos += 0.375; tick(pos); }
        await flushPromises();
        expect(recordCalls()).toHaveLength(1); // otra sesión habría acreditado de nuevo
    });

    it('cambiar de pista crea un token nuevo; repeat-one también', async () => {
        const player = await startTrack(7);

        tick(0);
        for (let i = 0; i < 120; i++) tick((i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
        const sid1 = recordCalls()[0].args?.listenSessionId;

        await player.play({ id: 8, title: 'Other', artist: 'Artist' });
        tick(0);
        for (let i = 0; i < 120; i++) tick((i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(2);
        const sid2 = recordCalls()[1].args?.listenSessionId;
        expect(sid2).not.toBe(sid1);

        // repeat-one: ended sin pendientes reinicia la pista con token nuevo.
        player.cycleRepeat(); // off → all
        player.cycleRepeat(); // all → one
        ended();
        await flushPromises();
        tick(0);
        for (let i = 0; i < 120; i++) tick((i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(3);
        const sid3 = recordCalls()[2].args?.listenSessionId;
        expect(sid3).not.toBe(sid2);
    });

    it('error transitorio: reintenta el mismo token con backoff hasta el tope de 5 y descarta', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/7.flac' });
            if (cmd === 'record_local_play') return Promise.reject(new Error('database is locked'));
            return Promise.resolve(null);
        });
        const player = await startTrack(7);

        tick(0);
        for (let i = 0; i < 120; i++) tick((i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
        expect(player.pendingListens.value).toHaveLength(1);
        expect(player.pendingListens.value[0]?.attempts).toBe(1);

        await vi.advanceTimersByTimeAsync(1000);  // intento 2
        await vi.advanceTimersByTimeAsync(2000);  // intento 3
        await vi.advanceTimersByTimeAsync(4000);  // intento 4
        await vi.advanceTimersByTimeAsync(8000);  // intento 5 → tope → descarta
        expect(recordCalls()).toHaveLength(5);
        expect(player.pendingListens.value).toHaveLength(0);
        await vi.advanceTimersByTimeAsync(60000); // nada más programado
        expect(recordCalls()).toHaveLength(5);
    });

    it('error transitorio seguido de éxito: el pendiente se confirma y se elimina', async () => {
        let recordAttempts = 0;
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/7.flac' });
            if (cmd === 'record_local_play') {
                recordAttempts++;
                return recordAttempts === 1
                    ? Promise.reject(new Error('database is locked'))
                    : Promise.resolve({ track_id: 7, play_count: 1, last_played: null, counted: true });
            }
            return Promise.resolve(null);
        });
        const player = await startTrack(7);

        tick(0);
        for (let i = 0; i < 120; i++) tick((i + 1) * 0.25);
        await flushPromises();
        expect(player.pendingListens.value).toHaveLength(1);

        await vi.advanceTimersByTimeAsync(1000);
        expect(recordCalls()).toHaveLength(2);
        expect(player.pendingListens.value).toHaveLength(0);
    });

    it('error definitivo (pista borrada): descarta sin reintentar', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/7.flac' });
            if (cmd === 'record_local_play') return Promise.reject(new Error('El track 7 no existe en la biblioteca'));
            return Promise.resolve(null);
        });
        const player = await startTrack(7);

        tick(0);
        for (let i = 0; i < 120; i++) tick((i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
        expect(player.pendingListens.value).toHaveLength(0);
        await vi.advanceTimersByTimeAsync(60000);
        expect(recordCalls()).toHaveLength(1); // sin reintentos
    });
});
