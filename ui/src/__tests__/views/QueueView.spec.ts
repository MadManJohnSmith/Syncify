/**
 * Unit tests for QueueView.vue
 * Tests queue display, filtering, worker controls, and event handling
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import QueueView from '../../views/QueueView.vue';
import { mockInvoke, resetMocks, emitMockEvent } from '../setup';
import { invoke } from '@tauri-apps/api/core';

// Mock data
const mockQueueItems = [
    {
        id: 1,
        track_id: 101,
        title: 'Test Song 1',
        artist: 'Test Artist',
        status: 'queued',
        priority: 1,
        progress_percent: 0,
        error_message: null,
        created_at: '2024-01-15T10:00:00Z',
    },
    {
        id: 2,
        track_id: 102,
        title: 'Downloading Track',
        artist: 'Another Artist',
        status: 'downloading',
        priority: 1,
        progress_percent: 45,
        error_message: null,
        created_at: '2024-01-15T10:01:00Z',
    },
    {
        id: 3,
        track_id: 103,
        title: 'Completed Song',
        artist: 'Completed Artist',
        status: 'complete',
        priority: 1,
        progress_percent: 100,
        error_message: null,
        created_at: '2024-01-15T09:00:00Z',
    },
    {
        id: 4,
        track_id: 104,
        title: 'Failed Track',
        artist: 'Failed Artist',
        status: 'failed',
        priority: 1,
        progress_percent: 0,
        error_message: 'Download timeout',
        created_at: '2024-01-15T08:00:00Z',
    },
];

const mockStats = {
    queued: 1,
    downloading: 1,
    complete: 1,
    failed: 1,
    cancelled: 0,
};

const mockWorkerStatus = {
    running: true,
    paused: false,
    active_downloads: 1,
    max_concurrent: 3,
};

describe('QueueView', () => {
    beforeEach(() => {
        resetMocks();
    });

    it('renders loading state initially', () => {
        mockInvoke(() => new Promise(() => { })); // Never resolves
        const wrapper = mount(QueueView);
        expect(wrapper.find('.loading').exists()).toBe(true);
    });

    it('renders queue stats after data loads', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        // Check stats bar is rendered
        const statsBar = wrapper.find('.stats-bar');
        expect(statsBar.exists()).toBe(true);

        // Check stat values
        expect(wrapper.text()).toContain('4'); // Total
        expect(wrapper.text()).toContain('Queued');
        expect(wrapper.text()).toContain('Downloading');
        expect(wrapper.text()).toContain('Complete');
        expect(wrapper.text()).toContain('Failed');
    });

    it('renders queue items with correct status icons', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        const queueItems = wrapper.findAll('.queue-item');
        expect(queueItems.length).toBe(4);

        // Check item content
        expect(wrapper.text()).toContain('Test Song 1');
        expect(wrapper.text()).toContain('Downloading Track');
        expect(wrapper.text()).toContain('Completed Song');
        expect(wrapper.text()).toContain('Failed Track');
    });

    it('filters queue by status when clicking stat item', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        // Initially shows all 4 items
        expect(wrapper.findAll('.queue-item').length).toBe(4);

        // Click on "Failed" filter
        const failedStat = wrapper.findAll('.stat-item')[4]; // 5th stat item
        await failedStat.trigger('click');

        // Should only show 1 failed item
        expect(wrapper.findAll('.queue-item').length).toBe(1);
        expect(wrapper.text()).toContain('Failed Track');
    });

    it('displays worker status with pause/resume button', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        // Check worker status is displayed
        expect(wrapper.text()).toContain('Running');
        expect(wrapper.text()).toContain('1/3'); // active/max

        // Pause button should be visible (since not paused)
        const pauseButton = wrapper.find('.worker-controls .btn-secondary');
        expect(pauseButton.text()).toBe('Pause');
    });

    it('calls pause_downloads when clicking Pause', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        const pauseButton = wrapper.find('.worker-controls .btn-secondary');
        await pauseButton.trigger('click');
        await flushPromises();

        expect(invoke).toHaveBeenCalledWith('pause_downloads');
    });

    it('shows Resume button when paused', async () => {
        const pausedWorkerStatus = { ...mockWorkerStatus, paused: true };

        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return pausedWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        expect(wrapper.text()).toContain('Paused');
        const resumeButton = wrapper.find('.worker-controls .btn-primary');
        expect(resumeButton.text()).toBe('Resume');
    });

    it('displays progress bar for downloading items', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        const progressBars = wrapper.findAll('.progress-bar');
        expect(progressBars.length).toBe(1); // Only downloading item has progress bar

        const progressFill = wrapper.find('.progress-fill');
        expect(progressFill.attributes('style')).toContain('width: 45%');
    });

    it('displays error message for failed items', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        expect(wrapper.text()).toContain('Download timeout');
    });

    it('calls retry_queue_item when clicking retry button', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        // Find retry button on failed item
        const failedItem = wrapper.findAll('.queue-item')[3];
        const retryButton = failedItem.find('.btn-icon');
        await retryButton.trigger('click');
        await flushPromises();

        expect(invoke).toHaveBeenCalledWith('retry_queue_item', { queueId: 4 });
    });

    it('shows empty state when no items in queue', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return [];
            if (command === 'get_queue_stats') return { queued: 0, downloading: 0, complete: 0, failed: 0, cancelled: 0 };
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        expect(wrapper.find('.empty-state').exists()).toBe(true);
        expect(wrapper.text()).toContain('No items in queue');
    });

    it('updates queue item on progress event', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') return mockQueueItems;
            if (command === 'get_queue_stats') return mockStats;
            if (command === 'get_worker_status') return mockWorkerStatus;
            return [];
        });

        const wrapper = mount(QueueView);
        await flushPromises();

        // Initially 45% progress
        expect(wrapper.find('.progress-fill').attributes('style')).toContain('width: 45%');

        // Emit progress event
        emitMockEvent('syncify:download_progress', {
            queue_id: 2,
            track_id: 102,
            title: 'Downloading Track',
            artist: 'Another Artist',
            status: 'started',
            progress_percent: 75,
            message: null,
        });

        await flushPromises();

        // Progress should be updated
        expect(wrapper.find('.progress-fill').attributes('style')).toContain('width: 75%');
    });
});
