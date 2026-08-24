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
    getWorkerStatus,
    clearQueue,
    restoreInterrupted,
    preflightDownloadBatch,
    enqueueEligibleBatch,
    cancelItem,
    queueApi,
} from '@/api/queue';
import { mockInvoke, resetMocks } from '../setup';

describe('Queue API IPC audit', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('addToQueue correctly serializes payload and defaults allowFallback to true', async () => {
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
            allowFallback: true,
        });
    });

    it('addToQueue preserves explicit allowFallback: false when specified', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'add_to_queue') return 123;
            return null;
        });

        await addToQueue({
            trackId: 42,
            qualityPreference: 'HI_RES_LOSSLESS',
            serviceName: 'qobuz',
            allowFallback: false,
        });

        expect(invokeCalls[0].args.allowFallback).toBe(false);
    });

    it('addBatchToQueue passes array of trackIds and normalizes quality with default allowFallback true', async () => {
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
            allowFallback: true,
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

describe('queue_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('returns [] when get_queue resolves null or a non-array', async () => {
        mockInvoke(() => null);
        expect(await getQueue()).toEqual([]);

        mockInvoke((cmd) => (cmd === 'get_queue' ? { not: 'an array' } : null));
        expect(await getQueue()).toEqual([]);
    });

    it('passes through full queue payloads untouched (identity preserved)', async () => {
        const items = [{ id: 1, track_id: 11, status: 'queued', priority: 5, progress_percent: 0 }];
        mockInvoke((cmd) => (cmd === 'get_queue' ? items : null));

        const res = await getQueue();
        expect(res).toBe(items); // same reference: consumers mutate items across polls
    });

    it('defaults every counter of get_queue_stats to 0 when missing', async () => {
        mockInvoke((cmd) => (cmd === 'get_queue_stats' ? { queued: 2 } : null));

        const stats = await getQueueStats();

        expect(stats.queued).toBe(2);
        expect(stats.total).toBe(0);
        expect(stats.downloading).toBe(0);
        expect(stats.completed).toBe(0);
        expect(stats.failed).toBe(0);
        expect(stats.paused).toBe(0);

        mockInvoke(() => null);
        const zeroed = await getQueueStats();
        expect(zeroed.total).toBe(0);
        expect(zeroed.completed).toBe(0);
    });

    it('accepts the legacy complete key as an alias for completed', async () => {
        mockInvoke((cmd) => (cmd === 'get_queue_stats' ? { queued: 1, downloading: 1, complete: 1, failed: 1, cancelled: 0 } : null));

        const stats = await getQueueStats();
        expect(stats.completed).toBe(1);
    });

    it('normalizes worker status booleans and counters', async () => {
        mockInvoke((cmd) => (cmd === 'get_worker_status' ? { running: 1, active_downloads: 'x' } : null));

        const worker = await getWorkerStatus();
        expect(worker.running).toBe(false); // non-boolean coerced to false
        expect(worker.paused).toBe(false);
        expect(worker.active_downloads).toBe(0);
        expect(worker.max_concurrent).toBe(0);

        mockInvoke(() => null);
        const empty = await getWorkerStatus();
        expect(empty.running).toBe(false);
        expect(empty.max_concurrent).toBe(0);
    });

    it('coerces clearQueue and restoreInterrupted counts to numbers', async () => {
        mockInvoke(() => null);
        expect(await clearQueue('complete')).toBe(0);
        expect(await restoreInterrupted()).toBe(0);

        mockInvoke((cmd) => (cmd === 'clear_queue' ? 7 : cmd === 'restore_interrupted_downloads' ? 3 : null));
        expect(await clearQueue()).toBe(7);
        expect(await restoreInterrupted()).toBe(3);
    });

    it('preflight_download_batch returns complete summary/tracks/estimated size defaults', async () => {
        mockInvoke(() => null);

        const res = await preflightDownloadBatch({ trackIds: [1, 2] });

        expect(res.summary.requested_total).toBe(0);
        expect(res.summary.eligible_total).toBe(0);
        expect(res.summary.ready_exact).toBe(0);
        expect(res.tracks).toEqual([]);
        expect(res.estimated_size_mb).toBe(0);
    });

    it('enqueue_eligible_batch defaults all counters and collections', async () => {
        mockInvoke((cmd) => (cmd === 'enqueue_eligible_batch' ? { added: 2, enqueued: 2, summary: { requested_total: 4 } } : null));

        const res = await enqueueEligibleBatch({ trackIds: [1, 2] });

        expect(res.submitted).toBe(0);
        expect(res.added).toBe(2);
        expect(res.enqueued).toBe(2);
        expect(res.deduplicated).toBe(0);
        expect(res.skipped).toBe(0);
        expect(res.summary.requested_total).toBe(4);
        expect(res.summary.eligible_total).toBe(0);
        expect(res.tracks).toEqual([]);
    });
});
