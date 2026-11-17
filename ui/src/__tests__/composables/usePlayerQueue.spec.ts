/**
 * FE-9 regression — usePlayer up-next playback queue (native engine).
 *
 * The Library context menu "Play Next" inserts a track into an up-next
 * queue; queueing with an idle player starts the track right away so the
 * action always has an audible effect. Since #132 (P01) the BACKEND advances
 * on the natural end of a track and notifies through `player:advanced`: the
 * UI does not advance on `ended` anymore, it syncs its projection from the
 * backend event.
 *
 * The engine transport (`api/player`) is mocked; `player:ended` and
 * `player:advanced` are delivered through the captured subscriptions, like
 * the Rust ticker.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
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

import { invokeCommand } from '../../api/tauri'
import * as playerApi from '../../api/player'
import type { PlayerTick } from '../../api/player'
import { usePlayer } from '../../composables/usePlayer'

const mockInvoke = vi.mocked(invokeCommand);
const engine = vi.mocked(playerApi);

// The load sequence the fake engine reports; `ended()` uses the current one.
let seq = 0;
let tickCb: ((tick: PlayerTick) => void) | null = null;
let endedCb: ((seq: number) => void) | null = null;
let advancedCb: ((advanced: import('../../api/player').PlayerAdvanced) => void) | null = null;

function configureEngineMocks(): void {
    seq = 0;
    engine.playerLoadTrack.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 200, stream: null }));
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
    engine.onPlayerStreamState.mockReset().mockResolvedValue(() => {});
    engine.onPlayerEnded.mockReset().mockImplementation((cb) => {
        endedCb = cb;
        return Promise.resolve(() => {});
    });
    engine.onPlayerAdvanced.mockReset().mockImplementation((cb) => {
        advancedCb = cb;
        return Promise.resolve(() => {});
    });
}

function ended(): void {
    endedCb?.(seq);
}

function advanced(trackId: number, entryId: string): void {
    advancedCb?.({ seq: ++seq, trackId, entryId });
}

describe('usePlayer playback queue (FE-9)', () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        configureEngineMocks();
        usePlayer().stop();
        mockInvoke.mockClear();
    });

    it('starts the track right away when nothing is loaded', async () => {
        const player = usePlayer();

        const outcome = await player.playNext({ id: 5, title: 'A', artist: 'X' });

        expect(outcome).toBe('started');
        expect(engine.playerLoadTrack).toHaveBeenCalledWith(5, true, 1);
        expect(player.current.value?.id).toBe(5);
        expect(player.upNext.value).toHaveLength(0);
    });

    it('inserts "Play Next" at the front of the queue while playing', async () => {
        const player = usePlayer();
        await player.play({ id: 1, title: 'One', artist: 'X' });

        await player.playNext({ id: 3, title: 'Three', artist: 'X' });
        const outcome = await player.playNext({ id: 2, title: 'Two', artist: 'X' });

        expect(outcome).toBe('queued');
        // The last queued track plays first.
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]);
        expect(player.current.value?.id).toBe(1);
    });

    it('does NOT advance on ended (P01): the backend advances and notifies with player:advanced', async () => {
        mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
            if (cmd === 'playback_queue_enqueue') {
                return Promise.resolve([`be-${(args?.trackIds as number[])[0]}`]);
            }
            if (cmd === 'get_track_metadata') {
                const trackId = (args?.trackId as number) ?? 0;
                return Promise.resolve({
                    track_id: trackId,
                    title: `Song ${trackId}`,
                    artist_name: 'Artist',
                    album_name: null,
                    cover_art_url: null,
                    sources: [],
                });
            }
            return Promise.resolve(undefined);
        });
        const player = usePlayer();
        await player.play({ id: 1, title: 'One', artist: 'X' });
        await player.playNext({ id: 2, title: 'Two', artist: 'X' });
        expect(player.upNext.value).toHaveLength(1);

        ended();
        await flushPromises();

        // The UI stays where it was: no new load, still on track 1.
        expect(engine.playerLoadTrack).toHaveBeenCalledTimes(1);
        expect(player.current.value?.id).toBe(1);

        // The backend decides the next track, loads it in the engine and
        // notifies; the UI syncs its projection from the event.
        advanced(2, 'be-2');
        await flushPromises();

        expect(player.current.value?.id).toBe(2);
        expect(player.isPlaying.value).toBe(true);
        expect(player.queue.value).toHaveLength(0);
        expect(player.history.value[player.history.value.length - 1]?.trackId).toBe(1);
    });

    it('stop clears the up-next queue', async () => {
        const player = usePlayer();
        await player.play({ id: 1, title: 'One', artist: 'X' });
        await player.playNext({ id: 2, title: 'Two', artist: 'X' });
        expect(player.upNext.value).toHaveLength(1);

        player.stop();

        expect(player.upNext.value).toHaveLength(0);
        expect(player.current.value).toBeNull();
    });
});
