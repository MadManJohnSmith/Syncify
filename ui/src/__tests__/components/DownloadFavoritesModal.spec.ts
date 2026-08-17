/**
 * DownloadFavoritesModal.spec.ts
 * Tests for DownloadFavoritesModal.vue IPC payloads and error handling
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import DownloadFavoritesModal from '@/components/DownloadFavoritesModal.vue';
import { mockInvoke, resetMocks } from '../setup';

describe('DownloadFavoritesModal', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('invokes download_favorites with sanitized parameters and exact IPC payload', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'download_favorites') {
                return {
                    total_candidates: 5,
                    enqueued: 5,
                    already_downloaded: 0,
                    already_queued: 0,
                    unresolved_sources: 0,
                    stale_sources: 0,
                    ambiguous_sources: 0,
                    is_preflight: false,
                    message: 'Enqueued 5 favorites'
                };
            }
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: {
                modelValue: true,
            }
        });
        await flushPromises();

        expect(wrapper.text()).toContain('Download Favorites');

        // Click Enqueue Downloads button
        const startBtn = wrapper.findAll('button').find(b => b.text().includes('Enqueue Downloads'));
        expect(startBtn).toBeDefined();
        await startBtn!.trigger('click');
        await flushPromises();

        const dlCall = invokeCalls.find(c => c.cmd === 'download_favorites');
        expect(dlCall).toBeDefined();
        expect(dlCall?.args).toEqual({
            service: undefined,
            itemType: undefined,
            qualityPreference: 'lossless',
            priority: 60,
            limit: 5,
            dryRun: false,
        });

        expect(wrapper.text()).toContain('Enqueued Successfully');
    });

    it('handles preflight guardrail when limit is All', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'download_favorites') {
                return {
                    total_candidates: 250,
                    enqueued: 250,
                    already_downloaded: 0,
                    already_queued: 0,
                    unresolved_sources: 2,
                    stale_sources: 1,
                    ambiguous_sources: 3,
                    is_preflight: true,
                    message: 'Preflight dry run completed for 250 items'
                };
            }
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: {
                modelValue: true,
            }
        });
        await flushPromises();

        // Select 'All' limit option
        const allLimitBtn = wrapper.findAll('button').find(b => b.text().includes('10k+'));
        if (allLimitBtn) {
            await allLimitBtn.trigger('click');
            await flushPromises();
        }

        const startBtn = wrapper.findAll('button').find(b => b.text().includes('Run Preflight Check') || b.text().includes('Enqueue Downloads'));
        expect(startBtn).toBeDefined();
        await startBtn!.trigger('click');
        await flushPromises();

        const dlCall = invokeCalls.find(c => c.cmd === 'download_favorites');
        expect(dlCall).toBeDefined();
        expect(dlCall?.args.dryRun).toBe(true);
        expect(wrapper.text()).toContain('Preflight Verification Guardrail');
        expect(wrapper.text()).toContain('Confirmation Required');
    });

    it('handles backend error gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'download_favorites') {
                throw new Error('Database connection failed');
            }
            return null;
        });

        const wrapper = mount(DownloadFavoritesModal, {
            props: {
                modelValue: true,
            }
        });
        await flushPromises();

        const startBtn = wrapper.findAll('button').find(b => b.text().includes('Enqueue Downloads'));
        expect(startBtn).toBeDefined();
        await startBtn!.trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('Database connection failed');
    });
});
