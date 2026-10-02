/**
 * FE-9 regression — usePlayer up-next playback queue.
 *
 * The Library context menu "Play Next" inserts a track into an up-next
 * queue that auto-advances when the current track ends; queueing with an
 * idle player starts the track right away so the action always has an
 * audible effect.
 *
 * jsdom implements no media playback, so HTMLMediaElement methods are
 * stubbed and the module-level Audio singleton is captured to fire media
 * events on it.
 */
import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'

const playStub = vi.fn(() => Promise.resolve())
const pauseStub = vi.fn()
const loadStub = vi.fn()

// The audio element created by usePlayer's module scope.
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

let usePlayer: typeof import('../../composables/usePlayer').usePlayer;

beforeAll(async () => {
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
    // Capture the singleton audio element created by the module so tests can
    // dispatch 'ended' on it. The patch must be in place BEFORE the module
    // under test is imported, hence the dynamic import below.
    class CapturingAudio extends window.Audio {
        constructor() {
            super();
            audioInstance = this;
        }
    }
    (window as unknown as { Audio: typeof Audio }).Audio = CapturingAudio;
    ({ usePlayer } = await import('../../composables/usePlayer'));
});

describe('usePlayer playback queue (FE-9)', () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        usePlayer().stop();
        playStub.mockClear();
        pauseStub.mockClear();
        loadStub.mockClear();
    });

    it('starts the track right away when nothing is loaded', async () => {
        mockInvoke.mockResolvedValueOnce({ track_id: 5, file_path: '/lib/a.flac', format: 'FLAC' });
        const player = usePlayer();

        const outcome = await player.playNext({ id: 5, title: 'A', artist: 'X' });

        expect(outcome).toBe('started');
        expect(mockInvoke).toHaveBeenCalledWith('resolve_playback_source', { trackId: 5 });
        expect(playStub).toHaveBeenCalledTimes(1);
        expect(player.current.value?.id).toBe(5);
        expect(player.upNext.value).toHaveLength(0);
    });

    it('inserts "Play Next" at the front of the queue while playing', async () => {
        mockInvoke.mockResolvedValueOnce({ track_id: 1, file_path: '/lib/1.flac' });
        const player = usePlayer();
        await player.play({ id: 1, title: 'One', artist: 'X' });

        await player.playNext({ id: 3, title: 'Three', artist: 'X' });
        const outcome = await player.playNext({ id: 2, title: 'Two', artist: 'X' });

        expect(outcome).toBe('queued');
        // The last queued track plays first.
        expect(player.upNext.value.map(t => t.id)).toEqual([2, 3]);
        expect(player.current.value?.id).toBe(1);
    });

    it('auto-advances to the queued track when the current one ends', async () => {
        mockInvoke.mockImplementation((_cmd: string, args?: Record<string, unknown>) =>
            Promise.resolve({ track_id: args?.trackId, file_path: `/lib/${args?.trackId}.flac` }));
        const player = usePlayer();
        await player.play({ id: 1, title: 'One', artist: 'X' });
        await player.playNext({ id: 2, title: 'Two', artist: 'X' });

        audioInstance?.dispatchEvent(new Event('ended'));
        await flushPromises();

        expect(mockInvoke).toHaveBeenCalledWith('resolve_playback_source', { trackId: 2 });
        expect(player.current.value?.id).toBe(2);
        expect(player.upNext.value).toHaveLength(0);
    });

    it('stop clears the up-next queue', async () => {
        mockInvoke.mockResolvedValueOnce({ track_id: 1, file_path: '/lib/1.flac' });
        const player = usePlayer();
        await player.play({ id: 1, title: 'One', artist: 'X' });
        await player.playNext({ id: 2, title: 'Two', artist: 'X' });
        expect(player.upNext.value).toHaveLength(1);

        player.stop();

        expect(player.upNext.value).toHaveLength(0);
        expect(player.current.value).toBeNull();
    });
});
