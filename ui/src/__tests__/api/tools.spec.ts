/**
 * tools.spec.ts
 * Regression tests: bridge results always expose a real boolean success flag.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { toolsApi } from '@/api/tools';
import { mockInvoke, resetMocks } from '../setup';

describe('tools_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('normalizes check_ffmpeg_available / check_fingerprint_available results', async () => {
        mockInvoke((cmd) => (cmd === 'check_ffmpeg_available' ? { success: true, data: { version: '6.0' } } : null));

        const ffmpeg = await toolsApi.checkFfmpeg();
        expect(ffmpeg.success).toBe(true);
        expect(ffmpeg.data).toEqual({ version: '6.0' });
        expect(ffmpeg.error).toBeUndefined();

        mockInvoke((cmd) => (cmd === 'check_fingerprint_available' ? { error: 'fpcalc not found' } : null));
        const fpcalc = await toolsApi.checkFingerprint();
        expect(fpcalc.success).toBe(false); // DashboardView renders this as 'missing'
        expect(fpcalc.error).toBe('fpcalc not found');
    });

    it('never returns undefined success for null or malformed payloads', async () => {
        mockInvoke(() => null);
        expect(await toolsApi.checkFfmpeg()).toMatchObject({ success: false });

        mockInvoke(() => 'garbage');
        expect(await toolsApi.checkFingerprint()).toMatchObject({ success: false });
    });
});
