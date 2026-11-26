/**
 * P02 (#133): la UI ya no publica metadatos MPRIS ni ejecuta acciones del
 * panel. El backend es el dueño del transporte; aquí se prueba que la
 * proyección local aplica `player:modes-changed` (incluido el orden de shuffle
 * resuelto en Rust) y el `previous` del backend (`previousEntryId`), sin
 * volver a invocar el comando retirado `player_mpris_update`.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

vi.mock('../../api/player', () => ({
    playerLoadTrack: vi.fn(),
    playerLoadQueueEntry: vi.fn(),
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
    onPlayerQueueEntryChanged: vi.fn(),
    onPlayerModesChanged: vi.fn(),
}));

import * as tauriApi from '../../api/tauri'
import * as playerApi from '../../api/player'
import type { PlayerAdvanced, PlayerModesChanged } from '../../api/player'
import { usePlayer } from '../../composables/usePlayer'

function mockInvoke(): ReturnType<typeof vi.mocked<typeof tauriApi.invokeCommand>> {
    return vi.mocked(tauriApi.invokeCommand);
}
const engine = vi.mocked(playerApi);

let seq = 0;
let advancedCb: ((advanced: PlayerAdvanced) => void) | null = null;
let modesCb: ((modes: PlayerModesChanged) => void) | null = null;

function configureEngineMocks(): void {
    seq = 0;
    engine.playerLoadTrack.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 180, stream: null }));
    engine.playerLoadQueueEntry.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 180, stream: null }));
    engine.playerPause.mockReset().mockResolvedValue(undefined);
    engine.playerResume.mockReset().mockResolvedValue(undefined);
    engine.playerStop.mockReset().mockResolvedValue(undefined);
    engine.playerSeek.mockReset().mockResolvedValue(undefined);
    engine.playerSetVolume.mockReset().mockResolvedValue(undefined);
    engine.playerSetRate.mockReset().mockResolvedValue(undefined);
    engine.onPlayerTick.mockReset().mockResolvedValue(() => {});
    engine.onPlayerStreamState.mockReset().mockResolvedValue(() => {});
    engine.onPlayerEnded.mockReset().mockResolvedValue(() => {});
    engine.onPlayerAdvanced.mockReset().mockImplementation((cb) => {
        advancedCb = cb;
        return Promise.resolve(() => {});
    });
    engine.onPlayerModesChanged.mockReset().mockImplementation((cb) => {
        modesCb = cb;
        return Promise.resolve(() => {});
    });
    engine.onPlayerQueueEntryChanged.mockReset().mockResolvedValue(() => {});
}

function metadataFor(trackId: number): Record<string, unknown> {
    return {
        track_id: trackId,
        title: `Track ${trackId}`,
        artist_name: 'Artist',
        album_name: null,
        cover_art_url: null,
        file_path: `/lib/${trackId}.flac`,
    };
}

function stableQueueIds(): void {
    mockInvoke().mockImplementation((cmd: string, args?: Record<string, unknown>) => {
        if (cmd === 'get_track_metadata') return Promise.resolve(metadataFor(Number(args?.trackId)));
        if (cmd === 'playback_queue_enqueue') return Promise.resolve([`be-${(args?.trackIds as number[])[0]}`]);
        return Promise.resolve(undefined);
    });
}

async function startTrack(id = 1): Promise<ReturnType<typeof usePlayer>> {
    const player = usePlayer();
    await player.play({ id, title: `Track ${id}`, artist: 'Artist' });
    await flushPromises();
    return player;
}

describe('usePlayer × backend MPRIS (P02)', () => {
    beforeEach(() => {
        mockInvoke().mockReset();
        configureEngineMocks();
        usePlayer().stop();
        mockInvoke().mockClear();
        stableQueueIds();
    });

    it('no publica metadatos con player_mpris_update: el backend los resuelve', async () => {
        await startTrack(7);
        expect(mockInvoke().mock.calls.some(([cmd]) => cmd === 'player_mpris_update')).toBe(false);
    });

    it('aplica shuffle/repeat y el orden pendiente que decidió el backend', async () => {
        const player = await startTrack(1);
        await player.enqueue({ id: 2, title: 'Two', artist: 'X' });
        await player.enqueue({ id: 3, title: 'Three', artist: 'X' });
        const ids = player.queue.value.map(entry => entry.entryId);
        expect(ids).toEqual(['be-2', 'be-3']);

        modesCb!({ shuffle: true, repeat: 'all', pendingEntryIds: ['be-3', 'be-2'] });
        await flushPromises();

        expect(player.shuffle.value).toBe(true);
        expect(player.repeat.value).toBe('all');
        expect(player.queue.value.map(entry => entry.entryId)).toEqual(['be-3', 'be-2']);

        modesCb!({ shuffle: false, repeat: 'one', pendingEntryIds: ['be-2', 'be-3'] });
        await flushPromises();

        expect(player.shuffle.value).toBe(false);
        expect(player.repeat.value).toBe('one');
        expect(player.queue.value.map(entry => entry.entryId)).toEqual(['be-2', 'be-3']);
    });

    it('previous del backend devuelve la pista que sonaba al frente de la cola', async () => {
        const player = await startTrack(1);
        await player.playNext({ id: 2, title: 'Two', artist: 'X' });
        const firstEntry = [...mockInvoke().mock.calls]
            .reverse()
            .find(([cmd]) => cmd === 'playback_queue_set_current')?.[1]?.entryId as string;
        expect(firstEntry).toBeTruthy();

        // El backend avanza a 2: A pasa al historial y B sale de pendientes.
        advancedCb!({ seq: ++seq, trackId: 2, entryId: 'be-2' });
        await flushPromises();
        expect(player.current.value?.id).toBe(2);
        expect(player.history.value.map(entry => entry.entryId)).toEqual([firstEntry]);
        expect(player.queue.value).toHaveLength(0);

        // previous: la entrada 2 (historial) pasa a actual; A vuelve a pendientes.
        advancedCb!({
            seq: ++seq,
            trackId: 1,
            entryId: firstEntry,
            previousEntryId: 'be-2',
        });
        await flushPromises();

        expect(player.current.value?.id).toBe(1);
        expect(player.history.value).toHaveLength(0);
        expect(player.queue.value.map(entry => entry.entryId)).toEqual(['be-2']);
        expect(player.queue.value[0]?.trackId).toBe(2);
    });
});
