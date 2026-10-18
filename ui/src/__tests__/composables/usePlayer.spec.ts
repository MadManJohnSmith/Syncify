/**
 * S194 residual — usePlayer composable tests (native engine transport).
 *
 * The engine transport (`api/player`) is mocked at module level; the Tauri
 * command layer (`api/tauri`) is mocked too. jsdom plays no audio and there
 * is no HTMLAudioElement: the composable is driven exactly like the Rust
 * backend would (commands in, events out).
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'

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

import { invokeCommand } from '../../api/tauri'
import * as playerApi from '../../api/player'
import { usePlayer } from '../../composables/usePlayer'

const mockInvoke = vi.mocked(invokeCommand);
const engine = vi.mocked(playerApi);

/** Engine mocks with resolved defaults; `playerLoadTrack` assigns increasing seqs. */
function configureEngineMocks(): void {
    let seq = 0;
    engine.playerLoadTrack.mockReset().mockImplementation(() =>
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
}

describe('usePlayer', () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        configureEngineMocks();
        // The composable is a module-level singleton: clear its state so
        // tests stay independent (stop() itself commands the engine).
        usePlayer().stop();
        engine.playerStop.mockClear();
    });

    it('loads a library track through the backend (local file or stream)', async () => {
        const player = usePlayer();

        await player.play({ id: 7, title: 'Song', artist: 'Artist' });

        // The backend picks the downloaded file or resolves streaming; the
        // frontend never sees paths or stream URLs.
        expect(engine.playerLoadTrack).toHaveBeenCalledWith(7, true, 1);
        expect(player.isBuffering.value).toBe(false);
        expect(player.streamQuality.value).toBeNull();
        expect(player.current.value?.title).toBe('Song');
        expect(player.isPlaying.value).toBe(true);
        expect(player.durationSec.value).toBe(180);
        expect(player.error.value).toBeNull();
    });

    it('reloads the engine on a same-track replay (no element to inherit)', async () => {
        const player = usePlayer();

        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });

        expect(engine.playerLoadTrack).toHaveBeenCalledTimes(2);
    });

    it('exposes the honest stream quality and buffering state', async () => {
        engine.playerLoadTrack.mockResolvedValueOnce({
            seq: 3,
            durationSec: 210,
            stream: {
                serviceName: 'qobuz',
                serviceTrackId: '123',
                format: 'FLAC',
                codec: 'flac',
                container: 'FLAC',
                extension: 'flac',
                bitDepth: 24,
                sampleRate: 192000,
                qualityLabel: 'FLAC 24 bits / 192 kHz',
                qualityTier: 'hires',
                sourceName: 'Qobuz API oficial',
            },
        });
        const player = usePlayer();

        await player.play({ id: 11, title: 'Hi-Res', artist: 'Artist' });

        expect(player.streamQuality.value).toEqual({
            serviceName: 'qobuz',
            qualityLabel: 'FLAC 24 bits / 192 kHz',
            qualityTier: 'hires',
        });
    });

    it('surfaces backend errors honestly when no source can be resolved', async () => {
        engine.playerLoadTrack.mockRejectedValueOnce(
            new Error('No se pudo resolver ninguna fuente reproducible: qobuz: cuenta Qobuz desconectada')
        );
        const player = usePlayer();

        await expect(
            player.play({ id: 9, title: 'X', artist: 'Y' })
        ).rejects.toThrow(/ninguna fuente reproducible/);

        expect(player.error.value).toContain('ninguna fuente reproducible');
        expect(player.current.value).toBeNull();
    });

    it('shows the engine failure sanitized by the backend (never a path)', async () => {
        engine.playerLoadTrack.mockRejectedValueOnce(new Error('audio output unavailable: device busy'));
        const player = usePlayer();

        await expect(
            player.play({ id: 8, title: 'Broken', artist: 'Artist' })
        ).rejects.toThrow(/audio output unavailable/);

        expect(player.error.value).toContain('audio output unavailable');
        expect(player.error.value).not.toContain('/private');
        expect(player.isPlaying.value).toBe(false);
        expect(player.current.value).toBeNull();
    });

    it('pause/resume via toggle drives the engine and the state', async () => {
        const player = usePlayer();
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });

        player.toggle();
        await Promise.resolve();
        expect(engine.playerPause).toHaveBeenCalledTimes(1);
        expect(player.isPlaying.value).toBe(false);

        player.toggle();
        await Promise.resolve();
        expect(engine.playerResume).toHaveBeenCalledTimes(1);
        expect(player.isPlaying.value).toBe(true);
    });

    it('toggle without a loaded track is a no-op', () => {
        const player = usePlayer();
        player.toggle();
        expect(engine.playerPause).not.toHaveBeenCalled();
        expect(engine.playerResume).not.toHaveBeenCalled();
    });

    it('stop resets state and stops the engine', async () => {
        const player = usePlayer();
        await player.play({ id: 1, title: 'a', artist: 'b' });

        player.stop();

        expect(engine.playerStop).toHaveBeenCalled();
        expect(player.current.value).toBeNull();
        expect(player.positionSec.value).toBe(0);
        expect(player.durationSec.value).toBe(0);
        expect(player.isPlaying.value).toBe(false);
        expect(player.isBuffering.value).toBe(false);
        expect(player.streamQuality.value).toBeNull();
    });
});
