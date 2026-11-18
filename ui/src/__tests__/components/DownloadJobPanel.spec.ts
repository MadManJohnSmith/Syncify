/**
 * DownloadJobPanel.spec.ts
 * D08 (#120): the jobs panel lists state, counters and progress, and exposes
 * cancel-pending, retry-failed and clear-finished actions.
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import DownloadJobPanel from '@/components/DownloadJobPanel.vue';
import type { DownloadJob, DownloadJobProgress } from '@/api/downloadJobs';

function jobFixture(overrides: Partial<DownloadJob> = {}): DownloadJob {
    return {
        id: 42,
        kind: 'favorites',
        label: 'Favorites',
        status: 'running',
        sourceService: 'qobuz',
        destinationService: 'tidal',
        preferredAccountId: 7,
        qualityPreference: 'lossless',
        allowFallback: 1,
        filterJson: null,
        requestedCount: 10,
        estimatedCount: 10,
        estimatedSizeMb: 350,
        completedCount: 2,
        failedCount: 1,
        skippedCount: 1,
        skipBreakdownJson: null,
        error: null,
        createdAt: '2026-11-18T10:00:00Z',
        updatedAt: '2026-11-18T10:05:00Z',
        startedAt: '2026-11-18T10:00:00Z',
        completedAt: null,
        ...overrides,
    };
}

function progressFixture(overrides: Partial<DownloadJobProgress> = {}): DownloadJobProgress {
    return {
        job_id: 42,
        label: 'Favorites',
        status: 'running',
        total: 10,
        queued: 4,
        downloading: 2,
        completed: 2,
        failed: 1,
        cancelled: 0,
        skipped: 1,
        percent: 40,
        terminal: false,
        ...overrides,
    };
}

describe('DownloadJobPanel (D08, #120)', () => {
    beforeEach(() => {
        // Nothing global to reset: the panel is a pure presentational component.
    });

    it('renders status, counters, progress, account and requested quality', () => {
        const wrapper = mount(DownloadJobPanel, {
            props: {
                jobs: [jobFixture()],
                progress: { 42: progressFixture() },
                accountLabels: { 7: 'Studio Account (Qobuz)' },
            },
        });

        const row = wrapper.find('[data-testid="download-job-row"]');
        expect(row.exists()).toBe(true);
        expect(row.text()).toContain('Favorites');
        expect(row.text()).toContain('#42');
        expect(row.find('[data-testid="job-status"]').text()).toBe('Running');
        expect(row.find('[data-testid="job-account"]').text()).toContain('Studio Account');
        expect(row.find('[data-testid="job-quality"]').text()).toContain('Lossless');

        expect(row.find('[data-testid="job-counter-requested"]').text()).toBe('10');
        expect(row.find('[data-testid="job-counter-queued"]').text()).toBe('4');
        expect(row.find('[data-testid="job-counter-downloading"]').text()).toBe('2');
        expect(row.find('[data-testid="job-counter-completed"]').text()).toBe('2');
        expect(row.find('[data-testid="job-counter-failed"]').text()).toBe('1');
        expect(row.find('[data-testid="job-counter-skipped"]').text()).toBe('1');
        expect(row.find('[data-testid="job-percent"]').text()).toBe('40%');
        expect(row.find('[data-testid="job-progress-bar"]').attributes('style')).toContain('width: 40%');
    });

    it('emits cancel for pending rows and warns that the active download finishes', async () => {
        const wrapper = mount(DownloadJobPanel, {
            props: {
                jobs: [jobFixture()],
                progress: { 42: progressFixture() },
                accountLabels: {},
            },
        });

        const cancel = wrapper.find('[data-testid="job-cancel"]');
        expect(cancel.attributes('disabled')).toBeUndefined();
        expect(cancel.attributes('title')).toContain('active download will finish');
        expect(wrapper.find('[data-testid="job-active-notice"]').text()).toContain('active download will finish');

        await cancel.trigger('click');
        expect(wrapper.emitted('cancel')?.[0]).toEqual([42]);
    });

    it('disables cancel when the job has no queued rows', () => {
        const wrapper = mount(DownloadJobPanel, {
            props: {
                jobs: [jobFixture({ status: 'partial', completedCount: 9, failedCount: 1, requestedCount: 10 })],
                progress: { 42: progressFixture({ status: 'partial', queued: 0, downloading: 0, completed: 9, terminal: true }) },
                accountLabels: {},
            },
        });

        expect(wrapper.find('[data-testid="job-cancel"]').attributes('disabled')).toBeDefined();
        expect(wrapper.find('[data-testid="job-active-notice"]').exists()).toBe(false);
    });

    it('emits retry only when the job has failed rows', async () => {
        const wrapper = mount(DownloadJobPanel, {
            props: {
                jobs: [jobFixture()],
                progress: { 42: progressFixture() },
                accountLabels: {},
            },
        });

        const retry = wrapper.find('[data-testid="job-retry"]');
        expect(retry.attributes('disabled')).toBeUndefined();
        await retry.trigger('click');
        expect(wrapper.emitted('retry')?.[0]).toEqual([42]);
    });

    it('disables retry when the job has no failed rows', () => {
        const wrapper = mount(DownloadJobPanel, {
            props: {
                jobs: [jobFixture({ failedCount: 0 })],
                progress: { 42: progressFixture({ failed: 0 }) },
                accountLabels: {},
            },
        });

        expect(wrapper.find('[data-testid="job-retry"]').attributes('disabled')).toBeDefined();
    });

    it('clears finished jobs only when at least one is terminal', async () => {
        const running = mount(DownloadJobPanel, {
            props: {
                jobs: [jobFixture()],
                progress: { 42: progressFixture() },
                accountLabels: {},
            },
        });
        expect(running.find('[data-testid="job-clear-finished"]').attributes('disabled')).toBeDefined();

        const withTerminal = mount(DownloadJobPanel, {
            props: {
                jobs: [jobFixture({ status: 'cancelled' })],
                progress: { 42: progressFixture({ status: 'cancelled', terminal: true }) },
                accountLabels: {},
            },
        });
        const clear = withTerminal.find('[data-testid="job-clear-finished"]');
        expect(clear.attributes('disabled')).toBeUndefined();
        expect(clear.attributes('title')).toContain('ledger');
        await clear.trigger('click');
        expect(withTerminal.emitted('clear')).toHaveLength(1);
    });

    it('shows an empty state when there are no jobs', () => {
        const wrapper = mount(DownloadJobPanel, {
            props: { jobs: [], progress: {}, accountLabels: {} },
        });
        expect(wrapper.find('[data-testid="download-jobs-empty"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="job-clear-finished"]').attributes('disabled')).toBeDefined();
    });
});
