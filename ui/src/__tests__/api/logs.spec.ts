/**
 * logs.spec.ts
 * Regression tests: partial log entries must never crash LogsView rendering.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getSystemLogs, getLoggingStatus, recordSystemLog } from '@/api/logs';
import { mockInvoke, resetMocks } from '../setup';

describe('logs_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('fills defaults for entries missing id/timestamp/level/target/module/message', async () => {
        mockInvoke((cmd) => (cmd === 'get_system_logs'
            ? [{ level: 'error', message: 'boom' }, {}, null]
            : null));

        const logs = await getSystemLogs({ limit: 10 });

        expect(logs).toHaveLength(3);
        expect(logs[0].level).toBe('error');
        expect(logs[0].message).toBe('boom');
        expect(logs[0].id).toBeTruthy();
        expect(logs[0].timestamp).toBe('');
        expect(logs[0].target).toBe('');
        expect(logs[0].module).toBe('');

        // Entry without any fields gets safe defaults and a generated id
        expect(logs[1].level).toBe('info');
        expect(logs[1].message).toBe('');
        expect(logs[1].id).toBeTruthy();

        // Null entry still renders as a valid log row
        expect(logs[2].level).toBe('info');
    });

    it('maps unknown/aliased levels to a renderable level', async () => {
        mockInvoke((cmd) => (cmd === 'get_system_logs'
            ? [{ level: 'warning', message: 'w' }, { level: 'CRITICAL', message: 'c' }]
            : null));

        const logs = await getSystemLogs();

        expect(logs[0].level).toBe('warn');
        expect(logs[1].level).toBe('info');
    });

    it('returns [] when get_system_logs resolves null', async () => {
        mockInvoke(() => null);
        expect(await getSystemLogs()).toEqual([]);
    });

    it('defaults LoggingStatus counters and strings', async () => {
        mockInvoke(() => null);

        const status = await getLoggingStatus();

        expect(status.is_development).toBe(false);
        expect(status.file_logging_active).toBe(false);
        expect(status.log_dir).toBe('');
        expect(status.log_level).toBe('info');
        expect(status.buffer_count).toBe(0);
        expect(status.retention_days).toBe(0);
        expect(status.max_file_size_mb).toBe(0);
    });

    it('record_system_log normalizes the echoed entry', async () => {
        mockInvoke((cmd) => (cmd === 'record_system_log' ? {} : null));

        const entry = await recordSystemLog({ level: 'info', message: 'hello' });

        expect(entry.level).toBe('info');
        expect(entry.message).toBe('');
        expect(entry.id).toBeTruthy();
    });
});
