import { describe, it, expect, vi } from 'vitest';
import { getConcurrencyStatsSummary, getActiveConcurrencyLocks } from '../../api/metadata';
import * as tauri from '../../api/tauri';

describe('Concurrency Diagnostics API', () => {
    it('getConcurrencyStatsSummary calls get_concurrency_stats_summary command', async () => {
        const mockSummary = {
            total_acquisitions: 150,
            contended_acquisitions: 12,
            timeouts: 0,
            active_locks_count: 3,
            max_wait_duration_ms: 45,
            max_held_duration_ms: 120,
        };

        const invokeSpy = vi.spyOn(tauri, 'invokeCommand').mockResolvedValue(mockSummary);

        const result = await getConcurrencyStatsSummary();

        expect(invokeSpy).toHaveBeenCalledWith('get_concurrency_stats_summary');
        expect(result.total_acquisitions).toBe(150);
        expect(result.contended_acquisitions).toBe(12);
        expect(result.timeouts).toBe(0);
        expect(result.active_locks_count).toBe(3);

        invokeSpy.mockRestore();
    });

    it('getActiveConcurrencyLocks returns list of redacted lock hashes', async () => {
        const mockLocks = ['lock:1234567890abcdef', 'lock:fedcba0987654321'];

        const invokeSpy = vi.spyOn(tauri, 'invokeCommand').mockResolvedValue(mockLocks);

        const result = await getActiveConcurrencyLocks();

        expect(invokeSpy).toHaveBeenCalledWith('get_active_concurrency_locks');
        expect(result).toHaveLength(2);
        expect(result[0]).toContain('lock:');

        invokeSpy.mockRestore();
    });
});
