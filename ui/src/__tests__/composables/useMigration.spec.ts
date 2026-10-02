/**
 * Unit tests for useMigration composable (TASK-57)
 * Tests auto-cleanup of Tauri event listeners on component unmount and state updates.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { defineComponent } from 'vue';
import { mount } from '@vue/test-utils';
import * as tauriEvent from '@tauri-apps/api/event';
import * as migrationApi from '@/api/migration';
import { useMigration } from '@/composables/useMigration';
import { resetMocks, emitMockEvent } from '../setup';

describe('useMigration Composable (TASK-57)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('automatically unlistens progress listener when component unmounts', async () => {
        const unlistenMock = vi.fn();
        vi.spyOn(tauriEvent, 'listen').mockResolvedValue(unlistenMock);

        let composable!: ReturnType<typeof useMigration>;

        const TestComponent = defineComponent({
            setup() {
                composable = useMigration();
                return {};
            },
            template: '<div>Test</div>',
        });

        const wrapper = mount(TestComponent);
        await composable.setupProgressListener();

        expect(tauriEvent.listen).toHaveBeenCalledWith('migration-progress', expect.any(Function));
        expect(unlistenMock).not.toHaveBeenCalled();

        // Unmount the component
        wrapper.unmount();

        // progressUnlisten must be automatically called
        expect(unlistenMock).toHaveBeenCalledTimes(1);
    });

    it('invokes cleanup on unmount when setupProgressListener is called inside setup()', async () => {
        const unlistenMock = vi.fn();
        vi.spyOn(tauriEvent, 'listen').mockResolvedValue(unlistenMock);

        const TestComponent = defineComponent({
            setup() {
                const migration = useMigration();
                migration.setupProgressListener();
                return { migration };
            },
            template: '<div>Test</div>',
        });

        const wrapper = mount(TestComponent);
        // Allow microtask to resolve the async listen()
        await Promise.resolve();

        expect(tauriEvent.listen).toHaveBeenCalledWith('migration-progress', expect.any(Function));
        expect(unlistenMock).not.toHaveBeenCalled();

        wrapper.unmount();
        expect(unlistenMock).toHaveBeenCalledTimes(1);
    });

    it('manually calling cleanup() triggers unlisten and prevents double unlistening', async () => {
        const unlistenMock = vi.fn();
        vi.spyOn(tauriEvent, 'listen').mockResolvedValue(unlistenMock);

        const migration = useMigration();
        await migration.setupProgressListener();

        expect(unlistenMock).not.toHaveBeenCalled();

        // Manual cleanup
        migration.cleanup();
        expect(unlistenMock).toHaveBeenCalledTimes(1);

        // Calling cleanup a second time does not re-invoke unlisten
        migration.cleanup();
        expect(unlistenMock).toHaveBeenCalledTimes(1);
    });

    it('cleans up previous listener when setupProgressListener is called consecutively', async () => {
        const firstUnlisten = vi.fn();
        const secondUnlisten = vi.fn();

        vi.spyOn(tauriEvent, 'listen')
            .mockResolvedValueOnce(firstUnlisten)
            .mockResolvedValueOnce(secondUnlisten);

        const migration = useMigration();
        await migration.setupProgressListener();
        expect(firstUnlisten).not.toHaveBeenCalled();

        // Second call should cleanup first listener
        await migration.setupProgressListener();
        expect(firstUnlisten).toHaveBeenCalledTimes(1);
        expect(secondUnlisten).not.toHaveBeenCalled();

        migration.cleanup();
        expect(secondUnlisten).toHaveBeenCalledTimes(1);
    });

    it('updates progress state and triggers history reload on completed status', async () => {
        let listenerHandler!: (event: { payload: any }) => void;
        vi.spyOn(tauriEvent, 'listen').mockImplementation(async (_name, handler) => {
            listenerHandler = handler as any;
            return () => {};
        });

        const migration = useMigration();
        const getHistorySpy = vi.spyOn(migrationApi, 'getMigrationHistory').mockResolvedValue([]);

        await migration.setupProgressListener();
        expect(listenerHandler).toBeDefined();

        listenerHandler({
            payload: {
                job_id: 'job-123',
                current_item: 5,
                total_items: 10,
                current_track: 'Test Song',
                status: 'running',
                completed_count: 4,
                failed_count: 1,
                skipped_count: 0,
            },
        });

        expect(migration.progress.job_id).toBe('job-123');
        expect(migration.progress.current_item).toBe(5);
        expect(migration.progress.total_items).toBe(10);
        expect(migration.progress.current_track).toBe('Test Song');
        expect(migration.progress.status).toBe('running');
        expect(getHistorySpy).not.toHaveBeenCalled();

        // Emit completed status
        listenerHandler({
            payload: {
                job_id: 'job-123',
                current_item: 10,
                total_items: 10,
                current_track: 'Finished Song',
                status: 'completed',
                completed_count: 9,
                failed_count: 1,
                skipped_count: 0,
            },
        });

        expect(migration.progress.status).toBe('completed');
        expect(getHistorySpy).toHaveBeenCalledTimes(1);
    });

    it('maps derived payload fields (percent, speed, eta, current_action) into progress state', async () => {
        // Regression for FE-1: the migration-progress payload carries the
        // derived metrics the MigrationView progress screen consumes.
        let listenerHandler!: (event: { payload: any }) => void;
        vi.spyOn(tauriEvent, 'listen').mockImplementation(async (_name, handler) => {
            listenerHandler = handler as any;
            return () => {};
        });

        const migration = useMigration();
        await migration.setupProgressListener();
        expect(listenerHandler).toBeDefined();

        listenerHandler({
            payload: {
                job_id: 'job-456',
                current_item: 25,
                total_items: 100,
                current_track: 'Queen - Bohemian Rhapsody',
                status: 'running',
                completed_count: 20,
                failed_count: 3,
                skipped_count: 2,
                percent: 25.0,
                speed: 25.0,
                eta: '3 min 0 s',
                current_action: 'Transferring Bohemian Rhapsody',
            },
        });

        expect(migration.progress.percent).toBe(25.0);
        expect(migration.progress.speed).toBe(25.0);
        expect(migration.progress.eta).toBe('3 min 0 s');
        expect(migration.progress.current_action).toBe('Transferring Bohemian Rhapsody');

        // Values are replaced on every event, not merged
        listenerHandler({
            payload: {
                job_id: 'job-456',
                current_item: 100,
                total_items: 100,
                current_track: 'Migration complete',
                status: 'completed',
                completed_count: 97,
                failed_count: 2,
                skipped_count: 1,
                percent: 100,
                speed: 50.0,
                eta: '0 s',
                current_action: 'Migration complete',
            },
        });

        expect(migration.progress.percent).toBe(100);
        expect(migration.progress.speed).toBe(50.0);
        expect(migration.progress.eta).toBe('0 s');
        expect(migration.progress.current_action).toBe('Migration complete');
    });
});
