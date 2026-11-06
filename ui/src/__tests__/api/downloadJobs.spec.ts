/**
 * downloadJobs.spec.ts — D02 IPC contract for the four download-job commands.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    createDownloadJob,
    estimateDownloadJob,
    getDownloadJobs,
    getDownloadJob,
} from '@/api/downloadJobs';
import { mockInvoke, resetMocks } from '../setup';

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
