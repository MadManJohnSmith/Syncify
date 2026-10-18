/**
 * usePlayer × MPRIS (Fase 1): la UI publica metadatos y modos al panel MPRIS
 * del escritorio, y enruta las acciones del panel (media keys) por las mismas
 * funciones que usan la cola y el transporte. El estado de transporte lo
 * publica el backend desde su ticker; aquí sólo se prueba lo que la UI envía
 * (`playerMprisUpdate`) y lo que hace con lo que recibe (`player:mpris-action`).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
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
import type { MprisActionPayload, MprisStateUpdate, PlayerTick } from '../../api/player'
import { usePlayer } from '../../composables/usePlayer'

const mockInvoke = vi.mocked(invokeCommand);
const engine = vi.mocked(playerApi);

let seq = 0;
let tickCb: ((tick: PlayerTick) => void) | null = null;
let mprisCb: ((action: MprisActionPayload) => void) | null = null;

function configureEngineMocks(): void {
    seq = 0;
    engine.playerLoad.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 180 }));
    engine.playerPause.mockReset().mockResolvedValue(undefined);
    engine.playerResume.mockReset().mockResolvedValue(undefined);
    engine.playerStop.mockReset().mockResolvedValue(undefined);
    engine.playerSeek.mockReset().mockResolvedValue(undefined);
    engine.playerSetVolume.mockReset().mockResolvedValue(undefined);
    engine.playerSetRate.mockReset().mockResolvedValue(undefined);
    engine.playerMprisUpdate.mockReset().mockResolvedValue(undefined);
    engine.onPlayerTick.mockReset().mockImplementation((cb) => {
        tickCb = cb;
        return Promise.resolve(() => {});
    });
    engine.onPlayerEnded.mockReset().mockImplementation(() => Promise.resolve(() => {}));
    engine.onMprisAction.mockReset().mockImplementation((cb) => {
        mprisCb = cb;
        return Promise.resolve(() => {});
    });
}

function lastMprisUpdate(): MprisStateUpdate | undefined {
    const calls = engine.playerMprisUpdate.mock.calls;
    return calls[calls.length - 1]?.[0];
}

async function startTrack(id = 1): Promise<ReturnType<typeof usePlayer>> {
    mockInvoke.mockResolvedValueOnce({ track_id: id, file_path: `/lib/${id}.flac`, format: 'FLAC' });
    const player = usePlayer();
    await player.play({ id, title: `Track ${id}`, artist: 'Artist' });
    await flushPromises();
    return player;
}

describe('usePlayer × MPRIS (Fase 1)', () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        configureEngineMocks();
        usePlayer().stop();
        mockInvoke.mockClear();
    });

    it('publica los metadatos de la pista al empezar a sonar', async () => {
        await startTrack(7);

        expect(lastMprisUpdate()).toEqual({
            trackId: 7,
            title: 'Track 7',
            artist: 'Artist',
            album: null,
            durationSec: 180,
            artUrl: null,
            shuffle: false,
            loopStatus: 'None',
        });
    });

    it('stop() limpia los metadatos publicados', async () => {
        const player = await startTrack(3);
        engine.playerMprisUpdate.mockClear();

        player.stop();

        expect(engine.playerMprisUpdate).toHaveBeenCalledTimes(1);
        expect(lastMprisUpdate()?.trackId).toBeNull();
        expect(lastMprisUpdate()?.title).toBeNull();
        expect(lastMprisUpdate()?.durationSec).toBeNull();
    });

    it('toggleShuffle y cycleRepeat publican los modos', async () => {
        const player = await startTrack(1);
        engine.playerMprisUpdate.mockClear();

        player.toggleShuffle();
        expect(lastMprisUpdate()?.shuffle).toBe(true);

        player.cycleRepeat(); // off → all
        expect(lastMprisUpdate()?.loopStatus).toBe('Playlist');

        player.cycleRepeat(); // all → one
        expect(lastMprisUpdate()?.loopStatus).toBe('Track');
    });

    it('playPause del panel pausa cuando suena', async () => {
        const player = await startTrack(1);
        expect(mprisCb).not.toBeNull();

        mprisCb!({ action: 'playPause' });
        await flushPromises();

        expect(engine.playerPause).toHaveBeenCalledTimes(1);
        expect(player.isPlaying.value).toBe(false);
    });

    it('seek relativo del panel parte de la posición actual', async () => {
        await startTrack(1);
        tickCb?.({ seq: 1, state: 'playing', positionSec: 10, durationSec: 180 });

        mprisCb!({ action: 'seek', offsetUs: 5_000_000 });
        await flushPromises();

        expect(engine.playerSeek).toHaveBeenCalledWith(15);
    });

    it('setPosition del panel busca una posición absoluta', async () => {
        await startTrack(1);
        tickCb?.({ seq: 1, state: 'playing', positionSec: 10, durationSec: 180 });

        mprisCb!({ action: 'setPosition', positionUs: 42_000_000 });
        await flushPromises();

        expect(engine.playerSeek).toHaveBeenCalledWith(42);
    });

    it('next del panel avanza la cola sin resolver la fuente actual otra vez', async () => {
        mockInvoke.mockImplementation((_cmd: string, args?: Record<string, unknown>) =>
            Promise.resolve({ track_id: args?.trackId, file_path: `/lib/${args?.trackId}.flac` }));
        const player = usePlayer();
        await player.play({ id: 1, title: 'One', artist: 'X' });
        await player.playNext({ id: 2, title: 'Two', artist: 'X' });
        const resolvesBefore = mockInvoke.mock.calls.filter(c => c[0] === 'resolve_playback_source').length;

        mprisCb!({ action: 'next' });
        await flushPromises();

        expect(mockInvoke.mock.calls.filter(c => c[0] === 'resolve_playback_source').length).toBe(resolvesBefore + 1);
        expect(player.current.value?.id).toBe(2);
    });

    it('stop del panel detiene y limpia el reproductor', async () => {
        const player = await startTrack(2);
        engine.playerStop.mockClear();

        mprisCb!({ action: 'stop' });
        await flushPromises();

        expect(engine.playerStop).toHaveBeenCalledTimes(1);
        expect(player.current.value).toBeNull();
        expect(player.isPlaying.value).toBe(false);
    });

    it('play del panel reanuda cuando está en pausa', async () => {
        const player = await startTrack(1);
        player.toggle();
        await flushPromises();
        expect(player.isPlaying.value).toBe(false);
        engine.playerResume.mockClear();

        mprisCb!({ action: 'play' });
        await flushPromises();

        expect(engine.playerResume).toHaveBeenCalledTimes(1);
        expect(player.isPlaying.value).toBe(true);
    });
});
