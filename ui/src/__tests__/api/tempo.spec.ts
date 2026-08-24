/**
 * tempo.spec.ts
 * Regression tests: BPM batch summary counters default to 0.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { analyzeLibraryBpm } from '@/api/tempo';
import { mockInvoke, resetMocks } from '../setup';

describe('tempo_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('passes through a full summary unchanged', async () => {
        mockInvoke((cmd) => (cmd === 'analyze_library_bpm'
            ? { total: 10, analyzed: 7, skipped: 1, low_confidence: 1, failed: 1 }
            : null));

        const summary = await analyzeLibraryBpm({ only_missing: true });
        expect(summary.total).toBe(10);
        expect(summary.analyzed).toBe(7);
        expect(summary.skipped).toBe(1);
        expect(summary.low_confidence).toBe(1);
        expect(summary.failed).toBe(1);
    });

    it('defaults every counter to 0 when missing or null', async () => {
        mockInvoke((cmd) => (cmd === 'analyze_library_bpm' ? {} : null));
        const partial = await analyzeLibraryBpm();
        expect(partial.total).toBe(0);
        expect(partial.analyzed).toBe(0);

        mockInvoke(() => null);
        const zeroed = await analyzeLibraryBpm({ force: true });
        expect(zeroed).toEqual({ total: 0, analyzed: 0, skipped: 0, low_confidence: 0, failed: 0 });
    });
});
