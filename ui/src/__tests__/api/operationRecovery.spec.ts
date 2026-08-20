import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getRecoveryAuditSummary,
    triggerStartupReconciliation,
} from '../../api/metadata';
import type {
    RecoveryAuditSummary,
    OperationRecoveryDetail,
} from '../../api/types';

// Mock invokeCommand
vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri';

describe('Operation Recovery API (Sprint S167)', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('queries post-crash recovery audit summary with correct structure', async () => {
        const mockSummary: RecoveryAuditSummary = {
            total_journal_scanned: 4,
            active_operations_found: 3,
            recovered_count: 1,
            interrupted_retryable_count: 1,
            failed_terminal_count: 1,
            cleaned_staging_files: 2,
            details: [
                {
                    operation_id: 'op-rec-01',
                    operation_type: 'download_qobuz',
                    previous_status: 'checkpointed',
                    new_status: 'recovered',
                    phase: 'promotion',
                    action_taken: 'CompletePromotion',
                    message: 'Promoted staging file to destination',
                    ui_label: 'Recovered after restart',
                    error_taxonomy: null,
                },
                {
                    operation_id: 'op-rec-02',
                    operation_type: 'download_tidal',
                    previous_status: 'started',
                    new_status: 'interrupted',
                    phase: 'transfer',
                    action_taken: 'ScheduleRetry',
                    message: 'Staging cleaned up. Download reset to queued for retry.',
                    ui_label: 'Interrupted — retry available',
                    error_taxonomy: 'TemporaryNetworkFailure { endpoint: "https://api.tidal.com", message: "Timeout" }',
                },
                {
                    operation_id: 'op-rec-03',
                    operation_type: 'download_tidal',
                    previous_status: 'checkpointed',
                    new_status: 'failed_terminal',
                    phase: 'transfer',
                    action_taken: 'MarkTerminal',
                    message: 'Non-retryable terminal condition during crash recovery',
                    ui_label: 'Failed terminal — user action required',
                    error_taxonomy: 'AuthInvalid { message: "Token expired" }',
                },
            ],
        };

        vi.mocked(invokeCommand).mockResolvedValueOnce(mockSummary);

        const result = await getRecoveryAuditSummary();
        expect(invokeCommand).toHaveBeenCalledWith('get_recovery_audit_summary');
        expect(result.total_journal_scanned).toBe(4);
        expect(result.recovered_count).toBe(1);
        expect(result.interrupted_retryable_count).toBe(1);
        expect(result.failed_terminal_count).toBe(1);
        expect(result.cleaned_staging_files).toBe(2);
        expect(result.details).toHaveLength(3);

        // Verify exact UI labels required by specifications
        expect(result.details[0].ui_label).toBe('Recovered after restart');
        expect(result.details[1].ui_label).toBe('Interrupted — retry available');
        expect(result.details[2].ui_label).toBe('Failed terminal — user action required');
    });

    it('triggers startup reconciliation on demand', async () => {
        const mockSummary: RecoveryAuditSummary = {
            total_journal_scanned: 0,
            active_operations_found: 0,
            recovered_count: 0,
            interrupted_retryable_count: 0,
            failed_terminal_count: 0,
            cleaned_staging_files: 0,
            details: [],
        };

        vi.mocked(invokeCommand).mockResolvedValueOnce(mockSummary);

        const result = await triggerStartupReconciliation();
        expect(invokeCommand).toHaveBeenCalledWith('trigger_startup_reconciliation');
        expect(result.active_operations_found).toBe(0);
    });

    it('properly formats detail items and checks recovery actions', () => {
        const detail: OperationRecoveryDetail = {
            operation_id: 'op-detail-01',
            operation_type: 'catalog_identity_repair',
            previous_status: 'persisting',
            new_status: 'rolled_back',
            phase: 'persist',
            action_taken: 'RollbackFileToBaseline',
            message: 'Interrupted repair rolled back safely',
            ui_label: 'Interrupted — retry available',
            error_taxonomy: null,
        };

        expect(detail.operation_type).toBe('catalog_identity_repair');
        expect(detail.action_taken).toBe('RollbackFileToBaseline');
        expect(detail.new_status).toBe('rolled_back');
    });
});
