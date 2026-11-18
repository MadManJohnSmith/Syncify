/**
 * DownloadFavoritesModal.spec.ts
 * D08 contract (#120): the favorites modal is the download-job creator. It
 * carries source, destination, account override, quality, type and limit;
 * only jobs above 1.000 tracks ask for confirmation, and the modal closes
 * without waiting for the create promise.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import DownloadFavoritesModal from '@/components/DownloadFavoritesModal.vue';
import { mockInvoke, resetMocks } from '../setup';

function estimateFixture(total: number) {
    return {
        total,
        bySource: [{ service: 'qobuz', count: total }],
        byDestination: [{ service: 'qobuz', count: total }],
        byCapacity: { hires: 0, lossless: total, high: 0, lossy: 0, unknown: 0 },
        estimatedSizeMb: total * 35,
    };
}

const accountsFixture = [
    {
        id: 7,
        service_id: 1,
        service_name: 'qobuz',
        display_name: 'Studio Account',
        email: 'studio@example.com',
        is_active: true,
        last_synced: null,
        created_at: null,
    },
];

describe('DownloadFavoritesModal — job creator (D08, #120)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('sends source, destination, account override, quality, type and limit', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_accounts') return accountsFixture;
            if (cmd === 'estimate_download_job') return estimateFixture(25);
            // Never resolves: the modal must not wait for it.
            if (cmd === 'create_download_job') return new Promise(() => {});
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: { modelValue: true },
        });
        await flushPromises();

        await wrapper.find('[data-testid="job-source"]').setValue('qobuz');
        await wrapper.find('[data-testid="job-destination"]').setValue('qobuz');
        await flushPromises();
        await wrapper.find('[data-testid="job-account"]').setValue('7');
        await wrapper.find('[data-testid="job-type"]').setValue('albums');
        const hires = wrapper.findAll('button').find(b => b.text().includes('Hi-Res FLAC'));
        await hires!.trigger('click');
        const limit25 = wrapper.findAll('button').find(b => b.text().includes('Quick'));
        await limit25!.trigger('click');

        await wrapper.find('[data-testid="job-submit"]').trigger('click');
        await flushPromises();

        const expectedRequest = {
            kind: 'favorites',
            sourceService: 'qobuz',
            destinationService: 'qobuz',
            preferredAccountId: 7,
            qualityPreference: 'hires',
            priority: 60,
            allowFallback: true,
            limit: 25,
            filter: { itemType: 'albums' },
        };

        const estimateCall = invokeCalls.find(c => c.cmd === 'estimate_download_job');
        expect(estimateCall).toBeDefined();
        expect(estimateCall?.args).toEqual({ request: expectedRequest });

        const createCall = invokeCalls.find(c => c.cmd === 'create_download_job');
        expect(createCall).toBeDefined();
        expect(createCall?.args).toEqual({ request: expectedRequest });
    });

    it('creates at exactly 1.000 tracks without confirmation and closes immediately', async () => {
        let createResolved = false;
        mockInvoke((cmd) => {
            if (cmd === 'estimate_download_job') return estimateFixture(1000);
            if (cmd === 'create_download_job') {
                return new Promise((resolve) => {
                    // Keep the promise pending; the modal must close anyway.
                    void resolve;
                    createResolved = true;
                });
            }
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: { modelValue: true },
        });
        await flushPromises();

        await wrapper.find('[data-testid="job-submit"]').trigger('click');
        await flushPromises();

        expect(wrapper.find('[data-testid="job-estimate-confirmation"]').exists()).toBe(false);
        expect(createResolved).toBe(true);
        // The modal closes before the create promise resolves.
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([false]);
    });

    it('asks for confirmation with the aggregate estimate only above 1.000 tracks', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'estimate_download_job') return estimateFixture(1001);
            if (cmd === 'create_download_job') return new Promise(() => {});
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: { modelValue: true },
        });
        await flushPromises();

        await wrapper.find('[data-testid="job-submit"]').trigger('click');
        await flushPromises();

        const confirmation = wrapper.find('[data-testid="job-estimate-confirmation"]');
        expect(confirmation.exists()).toBe(true);
        expect(wrapper.find('[data-testid="job-estimate-summary"]').exists()).toBe(true);
        expect(confirmation.text()).toContain('1,001');
        expect(confirmation.text()).toContain('Sources');
        expect(confirmation.text()).toContain('Destinations');
        expect(wrapper.emitted('update:modelValue')).toBeUndefined();
        expect(invokeCalls.some(c => c.cmd === 'create_download_job')).toBe(false);

        await wrapper.find('[data-testid="job-confirm"]').trigger('click');
        await flushPromises();

        expect(invokeCalls.some(c => c.cmd === 'create_download_job')).toBe(true);
        expect(wrapper.emitted('update:modelValue')?.[0]).toEqual([false]);
    });

    it('keeps the modal open when the estimate has no candidates', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'estimate_download_job') return estimateFixture(0);
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: { modelValue: true },
        });
        await flushPromises();

        await wrapper.find('[data-testid="job-submit"]').trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('No tracks match these criteria');
        expect(wrapper.emitted('update:modelValue')).toBeUndefined();
        expect(invokeCalls.some(c => c.cmd === 'create_download_job')).toBe(false);
    });

    it('shows the estimate error and stays open when the backend rejects', async () => {
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

        await wrapper.find('[data-testid="job-submit"]').trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('Database connection failed');
        expect(wrapper.emitted('update:modelValue')).toBeUndefined();
    });
});
