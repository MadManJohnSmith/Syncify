import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
    getPlaybackQueueSnapshot,
    playbackQueueEnqueueProviderItem,
    playbackQueueSetCurrentEntry,
} from '@/api/playbackQueue';
import {
    onPlayerAdvanced,
    onPlayerQueueEntryChanged,
    onPlayerStreamState,
    playerLoadQueueEntry,
} from '@/api/player';
import { TauriEvents } from '@/api/tauri';
import { emitMockEvent, mockInvoke, resetMocks } from '../setup';

describe('Playback queue ephemeral API', () => {
    beforeEach(() => {
        resetMocks();
    });

    it('serializa la identidad del proveedor sin materializar biblioteca', async () => {
        mockInvoke((command, args) => {
            expect(command).toBe('playback_queue_enqueue_provider_item');
            expect(args).toEqual({
                serviceId: 2,
                serviceTrackId: 'qobuz-track-1',
                atFront: true,
            });
            return 'entry-ephemera';
        });

        await expect(
            playbackQueueEnqueueProviderItem(2, 'qobuz-track-1', true),
        ).resolves.toBe('entry-ephemera');
    });

    it('carga la efímera por entry_id sin exigir un track de biblioteca', async () => {
        mockInvoke((command, args) => {
            expect(command).toBe('player_load_queue_entry');
            expect(args).toEqual({
                entryId: 'entry-ephemera',
                autoplay: true,
                volume: 0.8,
            });
            return { seq: 4, duration_sec: 180, stream: null };
        });

        await expect(playerLoadQueueEntry('entry-ephemera', true, 0.8)).resolves.toEqual({
            seq: 4,
            durationSec: 180,
            stream: null,
        });
    });

    it('activa una entrada persistida solo por su identidad', async () => {
        const entry = {
            entry_id: 'entry-ephemera',
            track_id: 0,
            service_id: 2,
            service_track_id: 'qobuz-track-1',
            kind: 'current',
            position: 0,
            promotion_error: null,
        };
        mockInvoke((command, args) => {
            expect(command).toBe('playback_queue_set_current_entry');
            expect(args).toEqual({ entryId: 'entry-ephemera' });
            return entry;
        });

        await expect(playbackQueueSetCurrentEntry('entry-ephemera')).resolves.toEqual(entry);
    });

    it('proyecta la identidad de proveedor en advanced y stream-state', async () => {
        const advanced = vi.fn();
        const streamState = vi.fn();
        const queueEntryChanged = vi.fn();
        const unlistenAdvanced = await onPlayerAdvanced(advanced);
        const unlistenStreamState = await onPlayerStreamState(streamState);
        const unlistenQueueEntryChanged = await onPlayerQueueEntryChanged(queueEntryChanged);

        emitMockEvent(TauriEvents.PLAYER_ADVANCED, {
            seq: 4,
            track_id: 0,
            service_id: 2,
            service_track_id: 'qobuz-track-1',
            entry_id: 'entry-ephemera',
        });
        emitMockEvent(TauriEvents.PLAYER_STREAM_STATE, {
            seq: 4,
            trackId: 0,
            entryId: 'entry-ephemera',
            buffering: true,
            downloadedBytes: 10,
            totalBytes: 20,
            complete: false,
            pendingSeekSec: null,
            serviceName: 'qobuz',
            qualityLabel: 'FLAC 16-bit / 44.1 kHz',
            qualityTier: 'lossless',
            matchMethod: 'direct',
            error: null,
        });

        expect(advanced).toHaveBeenCalledWith({
            seq: 4,
            trackId: 0,
            serviceId: 2,
            serviceTrackId: 'qobuz-track-1',
            entryId: 'entry-ephemera',
            previousEntryId: undefined,
        });
        expect(streamState).toHaveBeenCalledWith(expect.objectContaining({
            trackId: 0,
            entryId: 'entry-ephemera',
        }));
        const promotedEntry = {
            entry_id: 'entry-ephemera',
            track_id: 42,
            service_id: null,
            service_track_id: null,
            kind: 'current',
            position: 0,
            promotion_error: null,
        };
        emitMockEvent(TauriEvents.PLAYER_QUEUE_ENTRY_CHANGED, { entry: promotedEntry });
        expect(queueEntryChanged).toHaveBeenCalledWith(promotedEntry);
        unlistenAdvanced();
        unlistenStreamState();
        unlistenQueueEntryChanged();
    });

    it('devuelve la instantánea con track_id = 0 para una efímera', async () => {
        mockInvoke(() => ({
            pending: [
                {
                    entry_id: 'entry-ephemera',
                    track_id: 0,
                    service_id: 2,
                    service_track_id: 'qobuz-track-1',
                    kind: 'pending',
                    position: 0,
                    promotion_error: null,
                },
            ],
            history: [],
            current: null,
            position_sec: 0,
            shuffle: false,
            repeat: 'off',
            volume: 1,
            muted: false,
        }));

        const snapshot = await getPlaybackQueueSnapshot();
        expect(snapshot.pending[0]).toMatchObject({
            track_id: 0,
            service_id: 2,
            service_track_id: 'qobuz-track-1',
        });
    });
});
