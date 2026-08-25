/**
 * S194 residual — usePlayer composable tests.
 * jsdom implements no media playback, so HTMLMediaElement methods are
 * stubbed; the Tauri layer is mocked at module level.
 */
import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest'

const playStub = vi.fn(() => Promise.resolve())
const pauseStub = vi.fn()
const loadStub = vi.fn()

beforeAll(() => {
    Object.defineProperty(window.HTMLMediaElement.prototype, 'play', {
        configurable: true,
        value: playStub,
    });
    Object.defineProperty(window.HTMLMediaElement.prototype, 'pause', {
        configurable: true,
        value: pauseStub,
    });
    Object.defineProperty(window.HTMLMediaElement.prototype, 'load', {
        configurable: true,
        value: loadStub,
    });
});

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string, protocol?: string) =>
        `${protocol ?? 'asset'}://localhost/${encodeURIComponent(path)}`,
}));

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri'
import { usePlayer } from '../../composables/usePlayer'

const mockInvoke = vi.mocked(invokeCommand);

describe('usePlayer', () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        // The composable is a module-level singleton: clear its state so
        // tests stay independent. stop() itself drives pause/load, hence
        // the stub clears come AFTER it.
        usePlayer().stop();
        playStub.mockClear();
        pauseStub.mockClear();
        loadStub.mockClear();
    });

    it('resolves the source and starts playback of a downloaded file', async () => {
        mockInvoke.mockResolvedValueOnce({
            track_id: 7,
            file_path: '/lib/Artist - Song.flac',
            format: 'FLAC',
        });
        const player = usePlayer();

        await player.play({ id: 7, title: 'Song', artist: 'Artist' });

        expect(mockInvoke).toHaveBeenCalledWith('resolve_playback_source', { trackId: 7 });
        expect(playStub).toHaveBeenCalledTimes(1);
        expect(player.current.value?.title).toBe('Song');
        // The syncify-media protocol carries the percent-encoded absolute path.
        expect(player.error.value).toBeNull();
    });

    it('surfaces backend errors honestly when the track has no local file', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('El track 9 no tiene archivo local descargado; descárgalo primero'));
        const player = usePlayer();

        await expect(
            player.play({ id: 9, title: 'X', artist: 'Y' })
        ).rejects.toThrow(/no tiene archivo local/);

        expect(player.error.value).toContain('no tiene archivo local');
        expect(playStub).not.toHaveBeenCalled();
        expect(player.current.value).toBeNull();
    });

    it('toggle without an assigned source is a no-op', () => {
        const player = usePlayer();
        player.toggle();
        expect(playStub).not.toHaveBeenCalled();
        expect(pauseStub).not.toHaveBeenCalled();
    });

    it('stop resets state and tears down the audio element', async () => {
        mockInvoke.mockResolvedValueOnce({
            track_id: 1,
            file_path: '/lib/a.flac',
        });
        const player = usePlayer();
        await player.play({ id: 1, title: 'a', artist: 'b' });

        player.stop();
        expect(pauseStub).toHaveBeenCalled();
        expect(loadStub).toHaveBeenCalled();
        expect(player.current.value).toBeNull();
        expect(player.positionSec.value).toBe(0);
        expect(player.durationSec.value).toBe(0);
    });
});
