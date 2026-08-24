/**
 * storage.spec.ts
 * Regression tests: storage stats render safely with partial payloads.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getStorageStats } from '@/api/storage';
import { mockInvoke, resetMocks } from '../setup';

describe('storage_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('returns a complete StorageStats when fields are missing', async () => {
        mockInvoke((cmd) => (cmd === 'get_storage_stats'
            ? { used_bytes: 100, total_bytes: 500, available_bytes: 400, path: 'D:\\Music', breakdown: [{ format: 'flac', size_bytes: 90 }] }
            : null));

        const stats = await getStorageStats();

        expect(stats.used_bytes).toBe(100);
        expect(stats.total_bytes).toBe(500);
        expect(stats.available_bytes).toBe(400);
        expect(stats.path).toBe('D:\\Music');
        expect(stats.breakdown).toEqual([{ format: 'flac', size_bytes: 90 }]);
    });

    it('defaults to zeros and an empty breakdown on null payload', async () => {
        mockInvoke(() => null);

        const stats = await getStorageStats();

        expect(stats.used_bytes).toBe(0);
        expect(stats.total_bytes).toBe(0);
        expect(stats.available_bytes).toBe(0);
        expect(stats.path).toBe('');
        expect(stats.breakdown).toEqual([]);
    });
});
