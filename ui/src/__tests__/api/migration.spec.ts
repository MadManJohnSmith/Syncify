/**
 * migration.spec.ts
 * Regression tests: migration lists and preview totals default safely.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getMigrationHistory,
    getMigrationTemplates,
    getMigrationDestinations,
    retryFailedItems,
    previewMigration,
    startMigration,
    searchDestinationTrack,
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
        expect(await getMigrationDestinations()).toEqual([]);
    });

    it('normalizes get_migration_destinations to lowercase service ids (4.2)', async () => {
        mockInvoke((cmd) => (cmd === 'get_migration_destinations'
            ? ['Qobuz', 'TIDAL', 'Spotify', 'deezer', 'soundcloud']
            : null));

        expect(await getMigrationDestinations()).toEqual(['qobuz', 'tidal', 'spotify', 'deezer', 'soundcloud']);
    });

    it('coerces retry_failed_items count to a number', async () => {
        mockInvoke(() => null);
        expect(await retryFailedItems('job-1')).toBe(0);

        mockInvoke((cmd) => (cmd === 'retry_failed_items' ? 6 : null));
        expect(await retryFailedItems('job-1')).toBe(6);
    });

    it('passes selected account IDs and explicit null defaults to migration commands', async () => {
        const calls: Array<[string, Record<string, unknown> | undefined]> = [];
        mockInvoke((cmd, args) => { calls.push([cmd, args]); return cmd === 'start_migration' ? 'job-1' : []; });
        await previewMigration('spotify', 'tidal', undefined, undefined, 5, 8);
        await startMigration('spotify', 'tidal', undefined, undefined, 5, 8);
        await searchDestinationTrack('tidal', 'song', 8);
        expect(calls.find(([cmd]) => cmd === 'preview_migration')?.[1]).toMatchObject({ sourceAccountId: 5, destinationAccountId: 8 });
        expect(calls.find(([cmd]) => cmd === 'start_migration')?.[1]).toMatchObject({ sourceAccountId: 5, destinationAccountId: 8 });
        expect(calls.find(([cmd]) => cmd === 'search_destination_track')?.[1]).toMatchObject({ accountId: 8 });
        calls.length = 0;
        await previewMigration('spotify', 'tidal');
        await startMigration('spotify', 'tidal');
        await searchDestinationTrack('tidal', 'song');
        expect(calls.find(([cmd]) => cmd === 'preview_migration')?.[1]).toMatchObject({ sourceAccountId: null, destinationAccountId: null });
        expect(calls.find(([cmd]) => cmd === 'start_migration')?.[1]).toMatchObject({ sourceAccountId: null, destinationAccountId: null });
        expect(calls.find(([cmd]) => cmd === 'search_destination_track')?.[1]).toMatchObject({ accountId: null });
    });

    it('rejects same-service global or identical accounts in preview and start', async () => {
        const calls: string[] = [];
        mockInvoke((cmd) => { calls.push(cmd); return cmd === 'start_migration' ? 'job-1' : {}; });
        for (const ids of [[null, null], [1, null], [null, 2], [1, 1]] as const) {
            await expect(previewMigration('spotify', 'spotify', undefined, undefined, ...ids)).rejects.toThrow('explicit, distinct accounts');
            await expect(startMigration('spotify', 'spotify', undefined, undefined, ...ids)).rejects.toThrow('explicit, distinct accounts');
        }
        expect(calls).toEqual([]);
        await previewMigration('spotify', 'spotify', undefined, undefined, 1, 2);
        await startMigration('spotify', 'spotify', undefined, undefined, 1, 2);
        expect(calls).toEqual(['preview_migration', 'start_migration']);
    });

    it('preserves source/destination account IDs on migration jobs', async () => {
        const job = { id: 'job-1', source_account_id: 5, destination_account_id: 8 };
        mockInvoke(() => [job]);
        expect(await getMigrationHistory()).toEqual([job]);
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
