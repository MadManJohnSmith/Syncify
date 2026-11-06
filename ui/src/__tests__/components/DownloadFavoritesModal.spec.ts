/**
 * DownloadFavoritesModal.spec.ts
 * D02 contract: the modal estimates first when the batch is unbounded, then
 * creates a download job with the same request.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import DownloadFavoritesModal from '@/components/DownloadFavoritesModal.vue';
import { mockInvoke, resetMocks } from '../setup';

function estimateFixture(total: number) {
    return {
        total,
        bySource: [],
        byDestination: [],
        byCapacity: { hires: 0, lossless: total, high: 0, lossy: 0, unknown: 0 },
        estimatedSizeMb: total * 35,
    };
}

describe('DownloadFavoritesModal', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('creates a download job with the sanitized request payload', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'estimate_download_job') return estimateFixture(5);
            if (cmd === 'create_download_job') return { jobId: 42 };
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: { modelValue: true },
        });
        await flushPromises();

        const startBtn = wrapper.findAll('button').find(b => b.text().includes('Enqueue Downloads'));
        expect(startBtn).toBeDefined();
        await startBtn!.trigger('click');
        await flushPromises();

        const expectedRequest = {
            kind: 'favorites',
            sourceService: undefined,
            qualityPreference: 'lossless',
            priority: 60,
            allowFallback: true,
            limit: 5,
            filter: { itemType: undefined },
        };

        const estimateCall = invokeCalls.find(c => c.cmd === 'estimate_download_job');
        expect(estimateCall).toBeDefined();
        expect(estimateCall?.args).toEqual({ request: expectedRequest });

        const createCall = invokeCalls.find(c => c.cmd === 'create_download_job');
        expect(createCall).toBeDefined();
        expect(createCall?.args).toEqual({ request: expectedRequest });

        expect(wrapper.text()).toContain('Enqueued Successfully');
        expect(wrapper.text()).toContain('#42');
    });

    it('stops at the aggregate estimate when the limit is All', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'estimate_download_job') return estimateFixture(250);
            if (cmd === 'create_download_job') return { jobId: 7 };
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: { modelValue: true },
        });
        await flushPromises();

        const allLimitBtn = wrapper.findAll('button').find(b => b.text().includes('10k+'));
        if (allLimitBtn) {
            await allLimitBtn.trigger('click');
            await flushPromises();
        }

        const startBtn = wrapper.findAll('button').find(b => b.text().includes('Run Preflight Check') || b.text().includes('Enqueue Downloads'));
        expect(startBtn).toBeDefined();
        await startBtn!.trigger('click');
        await flushPromises();

        expect(invokeCalls.some(c => c.cmd === 'estimate_download_job')).toBe(true);
        expect(invokeCalls.some(c => c.cmd === 'create_download_job')).toBe(false);
        expect(wrapper.text()).toContain('Preflight Verification Guardrail');
        expect(wrapper.text()).toContain('Confirmation Required');
    });

    it('handles backend error gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'estimate_download_job') {
                throw new Error('Database connection failed');
            }
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: { modelValue: true },
        });
        await flushPromises();

        const startBtn = wrapper.findAll('button').find(b => b.text().includes('Enqueue Downloads'));
        expect(startBtn).toBeDefined();
        await startBtn!.trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('Database connection failed');
    });
});
