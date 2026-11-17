/**
 * downloadJobs.spec.ts — D02 IPC contract for the four download-job commands.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    createDownloadJob,
    estimateDownloadJob,
    getDownloadJobs,
    getDownloadJob,
    cancelDownloadJob,
    retryDownloadJobFailed,
    clearDownloadJobs,
    onDownloadJobProgress,
    type DownloadJobProgress,
} from '@/api/downloadJobs';
import { TauriEvents } from '@/api/tauri';
import { mockInvoke, resetMocks, emitMockEvent } from '../setup';

describe('Download Jobs API contract', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('createDownloadJob sends the request payload and returns the job id', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'create_download_job') return { jobId: 42 };
            return null;
        });

        const request = {
            kind: 'favorites' as const,
            qualityPreference: 'lossless',
            limit: 10,
            filter: { itemType: 'tracks' as const },
        };
        const res = await createDownloadJob(request);

        expect(res.jobId).toBe(42);
        expect(invokeCalls.length).toBe(1);
        expect(invokeCalls[0].cmd).toBe('create_download_job');
        expect(invokeCalls[0].args).toEqual({ request });
    });

    it('estimateDownloadJob sends the same request shape without writing', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'estimate_download_job') {
                return {
                    total: 3,
                    bySource: [{ service: 'qobuz', count: 3 }],
                    byDestination: [{ service: 'qobuz', count: 3 }],
                    byCapacity: { hires: 0, lossless: 3, high: 0, lossy: 0, unknown: 0 },
                    estimatedSizeMb: 105,
                };
            }
            return null;
        });

        const request = { kind: 'album' as const, filter: { albumId: 7 } };
        const estimate = await estimateDownloadJob(request);

        expect(estimate.total).toBe(3);
        expect(invokeCalls[0].cmd).toBe('estimate_download_job');
        expect(invokeCalls[0].args).toEqual({ request });
    });

    it('getDownloadJobs forwards the limit and getDownloadJob the id', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_download_jobs') return [];
            if (cmd === 'get_download_job') return { id: 5, kind: 'single', status: 'running' };
             return null;
        });

        await getDownloadJobs(20);
        await getDownloadJob(5);

        expect(invokeCalls[0]).toEqual({ cmd: 'get_download_jobs', args: { limit: 20 } });
        expect(invokeCalls[1]).toEqual({ cmd: 'get_download_job', args: { id: 5 } });
    });
});

describe('Download Jobs lifecycle and event contract (D07, spec #111 / issue #119)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('cancelDownloadJob invokes cancel_download_job with the job id', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'cancel_download_job') {
                return { id: 42, kind: 'favorites', status: 'cancelled' };
            }
            return null;
        });

        const job = await cancelDownloadJob(42);

        expect(job.status).toBe('cancelled');
        expect(invokeCalls[0]).toEqual({ cmd: 'cancel_download_job', args: { jobId: 42 } });
    });

    it('retryDownloadJobFailed invokes retry_download_job_failed with the job id', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'retry_download_job_failed') {
                return { id: 42, kind: 'favorites', status: 'running' };
            }
            return null;
        });

        const job = await retryDownloadJobFailed(42);

        expect(job.status).toBe('running');
        expect(invokeCalls[0]).toEqual({
            cmd: 'retry_download_job_failed',
            args: { jobId: 42 },
        });
    });

    it('clearDownloadJobs invokes clear_download_jobs without args', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'clear_download_jobs') return 3;
            return null;
        });

        const removed = await clearDownloadJobs();

        expect(removed).toBe(3);
        expect(invokeCalls[0].cmd).toBe('clear_download_jobs');
    });

    it('onDownloadJobProgress receives the snake_case payload of the D07 event', async () => {
        const received: DownloadJobProgress[] = [];
        const unlisten = await onDownloadJobProgress((payload) => received.push(payload));

        emitMockEvent(TauriEvents.DOWNLOAD_JOB_PROGRESS, {
            job_id: 42,
            label: 'Favoritos',
            status: 'partial',
            total: 3,
            queued: 0,
            downloading: 0,
            completed: 1,
            failed: 1,
            cancelled: 1,
            skipped: 0,
            percent: 100,
            terminal: true,
        });

        expect(received).toHaveLength(1);
        expect(received[0].job_id).toBe(42);
        expect(received[0].status).toBe('partial');
        expect(received[0].percent).toBe(100);
        expect(received[0].terminal).toBe(true);

        unlisten();
        emitMockEvent(TauriEvents.DOWNLOAD_JOB_PROGRESS, {
            job_id: 7,
            label: null,
            status: 'running',
            total: 1,
            queued: 1,
            downloading: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
            percent: 0,
            terminal: false,
        });
        expect(received).toHaveLength(1); // unlisten corta la suscripción
    });
});
