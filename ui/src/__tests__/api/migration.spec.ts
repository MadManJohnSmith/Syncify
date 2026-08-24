/**
 * migration.spec.ts
 * Regression tests: migration lists and preview totals default safely.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getMigrationHistory,
    getMigrationTemplates,
    retryFailedItems,
    previewMigration,
} from '@/api/migration';
import { mockInvoke, resetMocks } from '../setup';

describe('migration_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('coerces non-array list responses to empty arrays', async () => {
        mockInvoke(() => null);
        expect(await getMigrationHistory(10)).toEqual([]);
        expect(await getMigrationTemplates()).toEqual([]);
    });

    it('coerces retry_failed_items count to a number', async () => {
        mockInvoke(() => null);
        expect(await retryFailedItems('job-1')).toBe(0);

        mockInvoke((cmd) => (cmd === 'retry_failed_items' ? 6 : null));
        expect(await retryFailedItems('job-1')).toBe(6);
    });

    it('defaults preview_migration totals and playlist list', async () => {
        mockInvoke((cmd) => (cmd === 'preview_migration'
            ? { total_tracks: 20, matched_tracks: 15, unmatched_tracks: 5, playlists: [{ id: 'p1', name: 'Mix', track_count: 10, matched_count: 8 }] }
            : null));

        const preview = await previewMigration('spotify', 'tidal');
        expect(preview.total_tracks).toBe(20);
        expect(preview.matched_tracks).toBe(15);
        expect(preview.unmatched_tracks).toBe(5);
        expect(preview.playlists).toHaveLength(1);

        mockInvoke(() => null);
        const zeroed = await previewMigration('spotify', 'tidal');
        expect(zeroed.total_tracks).toBe(0);
        expect(zeroed.matched_tracks).toBe(0);
        expect(zeroed.unmatched_tracks).toBe(0);
        expect(zeroed.playlists).toEqual([]);
    });
});
