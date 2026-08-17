/**
 * queue.spec.ts
 * Tests for queue API functions, parameter sanitization, and IPC payloads
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    addToQueue,
    addBatchToQueue,
    enqueueDownload,
    forceRedownloadTracks,
    retryItem,
    retryAllFailed,
    clearAllFailed,
    getQueue,
    getQueueStats,
    cancelItem,
    queueApi,
} from '@/api/queue';
import { mockInvoke, resetMocks } from '../setup';

describe('Queue API IPC audit', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('addToQueue correctly serializes payload without undefined strings', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'add_to_queue') return 123;
            return null;
        });

        const id = await addToQueue({
            trackId: 42,
            qualityPreference: 'HI_RES_LOSSLESS',
            targetTitle: 'Audited Track',
            allowFallback: false,
        });

        expect(id).toBe(123);
        expect(invokeCalls.length).toBe(1);
        expect(invokeCalls[0].cmd).toBe('add_to_queue');
        expect(invokeCalls[0].args).toEqual({
            trackId: 42,
            priority: undefined,
            qualityPreference: 'hires',
            serviceId: undefined,
            serviceName: undefined,
            serviceTrackId: undefined,
            serviceAlbumId: undefined,
            targetTitle: 'Audited Track',
            targetArtist: undefined,
            targetAlbum: undefined,
            targetIsrc: undefined,
            smartStudioOrigin: undefined,
            allowFallback: false,
        });
    });

    it('addBatchToQueue passes array of trackIds and normalizes quality with allowFallback false', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'add_batch_to_queue') return { added: 3, skipped: 0 };
            return null;
        });

        const res = await addBatchToQueue({
            trackIds: [1, 2, 3],
            qualityPreference: '24-96',
        });

        expect(res).toEqual({ added: 3, skipped: 0 });
        expect(invokeCalls[0].cmd).toBe('add_batch_to_queue');
        expect(invokeCalls[0].args).toEqual({
            trackIds: [1, 2, 3],
            priority: undefined,
            qualityPreference: 'hires',
            serviceName: undefined,
            smartStudioOrigin: undefined,
            allowFallback: false,
        });
    });

    it('forceRedownloadTracks sends exact force_redownload_tracks command', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'force_redownload_tracks') return 2;
            return null;
        });

        const res = await forceRedownloadTracks([10, 20], 50, 'lossless');
        expect(res).toBe(2);
        expect(invokeCalls[0].cmd).toBe('force_redownload_tracks');
        expect(invokeCalls[0].args).toEqual({
            trackIds: [10, 20],
            priority: 50,
            qualityPreference: 'lossless',
        });
    });

    it('retryItem and retryAllFailed send expected commands', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'retry_queue_item') return null;
            if (cmd === 'retry_failed_downloads') return 'Retrying 2 items';
            return null;
        });

        await retryItem(999);
        expect(invokeCalls[0].cmd).toBe('retry_queue_item');
        expect(invokeCalls[0].args).toEqual({ queueId: 999 });

        const retryMsg = await retryAllFailed();
        expect(retryMsg).toBe('Retrying 2 items');
        expect(invokeCalls[1].cmd).toBe('retry_failed_downloads');
    });

    it('queueApi namespace exports all required methods', () => {
        expect(typeof queueApi.addToQueue).toBe('function');
        expect(typeof queueApi.addBatchToQueue).toBe('function');
        expect(typeof queueApi.forceRedownloadTracks).toBe('function');
        expect(typeof queueApi.retryItem).toBe('function');
        expect(typeof queueApi.retryAllFailed).toBe('function');
    });
});
