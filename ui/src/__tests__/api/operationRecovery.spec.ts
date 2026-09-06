import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getRecoveryAuditSummary,
    triggerStartupReconciliation,
    normalizeRecoveryAuditSummary,
} from '../../api/metadata';
import {
    executeWithRecovery,
    isRetryableError,
    createOperationRecoveryTracker,
} from '../../api/resilience';
import type {
    RecoveryAuditSummary,
    OperationRecoveryDetail,
} from '../../api/types';

// Mock invokeCommand
vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri';

describe('Operation Recovery & Resilience API (TASK-29)', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('Resilience & Exponential Backoff Retries', () => {
        it('retries transient Rust errors with exponential backoff and succeeds upon reconnection', async () => {
            let attempts = 0;
            const retryDelays: number[] = [];
            const recordedErrors: unknown[] = [];

            // Mock operation failing twice with transient errors, then succeeding
            const mockUnstableOperation = async () => {
                attempts++;
                if (attempts === 1) {
                    throw 'Database locked'; // Rust string error
                }
                if (attempts === 2) {
                    throw new Error('TemporaryNetworkFailure { message: "Connection reset" }');
                }
                return { success: true, recoveredCount: 1 };
            };

            const sleepMock = vi.fn(async (ms: number) => {
                retryDelays.push(ms);
            });

            const result = await executeWithRecovery(mockUnstableOperation, {
                maxRetries: 3,
                initialDelayMs: 15,
                backoffMultiplier: 2,
                sleepFn: sleepMock,
                onRetry: (err, attempt, delayMs) => {
                    recordedErrors.push(err);
                },
            });

            expect(attempts).toBe(3);
            expect(result).toEqual({ success: true, recoveredCount: 1 });
            expect(sleepMock).toHaveBeenCalledTimes(2);
            // Exponential backoff delays: 15ms, then 15 * 2 = 30ms
            expect(retryDelays).toEqual([15, 30]);
            expect(recordedErrors).toHaveLength(2);
            expect(recordedErrors[0]).toBe('Database locked');
        });

        it('aborts retries immediately on non-retryable terminal errors and triggers controlled fallback', async () => {
            let attempts = 0;

            const mockTerminalOperation = async () => {
                attempts++;
                throw new Error('AuthInvalid { message: "Session token expired" }');
            };

            const fallbackValue = { success: false, fallbackActive: true };

            const result = await executeWithRecovery(mockTerminalOperation, {
                maxRetries: 3,
                initialDelayMs: 10,
                fallback: fallbackValue,
            });

            // Terminal error: should not retry 3 times
            expect(attempts).toBe(1);
            expect(result).toEqual(fallbackValue);
        });

        it('re-throws terminal error when no fallback is configured', async () => {
            const mockFailingOperation = async () => {
                throw new Error('Terminal: unrecoverable disk corruption');
            };

            await expect(
                executeWithRecovery(mockFailingOperation, { maxRetries: 3 })
            ).rejects.toThrow(/Terminal: unrecoverable disk corruption/i);
        });

        it('correctly classifies retryable vs non-retryable error strings and objects', () => {
            expect(isRetryableError('Database locked')).toBe(true);
            expect(isRetryableError('TemporaryNetworkFailure')).toBe(true);
            expect(isRetryableError('IPC disconnected')).toBe(true);
            expect(isRetryableError(new Error('Connection reset by peer'))).toBe(true);

            expect(isRetryableError('AuthInvalid { token: "expired" }')).toBe(false);
            expect(isRetryableError(new Error('Unauthorized request'))).toBe(false);
            expect(isRetryableError(new Error('InvalidInput: trackId cannot be zero'))).toBe(false);
        });
    });

    describe('Local State Preservation & Reconnection Reconciliation', () => {
        it('preserves local operation state during IPC disconnection and reconciles upon reconnection', async () => {
            const tracker = createOperationRecoveryTracker<{ trackTitle: string }>();

            // 1. Register an in-flight operation
            const opId = 'op-rec-qobuz-101';
            tracker.registerOperation(opId, 'download_qobuz', { trackTitle: 'Cosmic Journey' });

            const initialOp = tracker.getOperation(opId);
            expect(initialOp?.status).toBe('in_progress');
            expect(initialOp?.data?.trackTitle).toBe('Cosmic Journey');

            // 2. Simulate IPC crash / network disconnection
            tracker.markInterrupted(opId, 'Process disconnected during promotion');

            const interruptedOp = tracker.getOperation(opId);
            expect(interruptedOp?.status).toBe('interrupted');
            expect(interruptedOp?.error).toBe('Process disconnected during promotion');
            // Crucial: local operation data is preserved
            expect(interruptedOp?.data?.trackTitle).toBe('Cosmic Journey');

            // 3. Reconnect and trigger startup reconciliation
            const mockBackendRecoveryReport: RecoveryAuditSummary = {
                total_journal_scanned: 1,
                active_operations_found: 1,
                recovered_count: 1,
                interrupted_retryable_count: 0,
                failed_terminal_count: 0,
                cleaned_staging_files: 1,
                details: [
                    {
                        operation_id: opId,
                        operation_type: 'download_qobuz',
                        previous_status: 'checkpointed',
                        new_status: 'recovered',
                        phase: 'promotion',
                        action_taken: 'CompletePromotion',
                        message: 'Promoted staging file to destination on restart',
                        ui_label: 'Recovered after restart',
                        error_taxonomy: null,
                    },
                ],
            };

            vi.mocked(invokeCommand).mockResolvedValueOnce(mockBackendRecoveryReport);

            const reconciliationSummary = await triggerStartupReconciliation();
            expect(invokeCommand).toHaveBeenCalledWith('trigger_startup_reconciliation');
            expect(reconciliationSummary.recovered_count).toBe(1);

            // Reconcile tracker with backend summary
            tracker.reconcileWithSummary(reconciliationSummary);

            const reconciledOp = tracker.getOperation(opId);
            expect(reconciledOp?.status).toBe('recovered');
            expect(reconciledOp?.error).toBeNull();
            expect(reconciledOp?.data?.trackTitle).toBe('Cosmic Journey');
        });
    });

    describe('RecoveryAuditSummary Normalization & Taxonomy Contract', () => {
        it('normalizes backend camelCase fields and infers default UI labels according to taxonomy', async () => {
            const rawBackendReport = {
                totalJournalScanned: 3,
                activeOperationsFound: 3,
                recoveredCount: 1,
                interruptedRetryableCount: 1,
                failedTerminalCount: 1,
                cleanedStagingFiles: 2,
                details: [
                    {
                        operationId: 'op-01',
                        operationType: 'download_qobuz',
                        previousStatus: 'checkpointed',
                        newStatus: 'recovered',
                        phase: 'promotion',
                        actionTaken: 'CompletePromotion',
                        message: 'Promoted staging file',
                        // uiLabel omitted to test automated taxonomy fallback
                    },
                    {
                        operationId: 'op-02',
                        operationType: 'download_tidal',
                        previousStatus: 'started',
                        newStatus: 'interrupted',
                        phase: 'transfer',
                        actionTaken: 'ScheduleRetry',
                        message: 'Reset to queued',
                        // uiLabel omitted
                    },
                    {
                        operationId: 'op-03',
                        operationType: 'download_tidal',
                        previousStatus: 'checkpointed',
                        newStatus: 'failed_terminal',
                        phase: 'transfer',
                        actionTaken: 'MarkTerminal',
                        message: 'Auth expired',
                        // uiLabel omitted
                    },
                ],
            };

            vi.mocked(invokeCommand).mockResolvedValueOnce(rawBackendReport);

            const result = await getRecoveryAuditSummary();

            expect(invokeCommand).toHaveBeenCalledWith('get_recovery_audit_summary');
            expect(result.total_journal_scanned).toBe(3);
            expect(result.recovered_count).toBe(1);
            expect(result.interrupted_retryable_count).toBe(1);
            expect(result.failed_terminal_count).toBe(1);
            expect(result.cleaned_staging_files).toBe(2);
            expect(result.details).toHaveLength(3);

            // Verify taxonomy default mapping
            expect(result.details[0].ui_label).toBe('Recovered after restart');
            expect(result.details[1].ui_label).toBe('Interrupted — retry available');
            expect(result.details[2].ui_label).toBe('Failed terminal — user action required');
        });

        it('returns zeroed contract safely on null or empty payload', () => {
            const normalized = normalizeRecoveryAuditSummary(null);
            expect(normalized.total_journal_scanned).toBe(0);
            expect(normalized.active_operations_found).toBe(0);
            expect(normalized.recovered_count).toBe(0);
            expect(normalized.interrupted_retryable_count).toBe(0);
            expect(normalized.failed_terminal_count).toBe(0);
            expect(normalized.cleaned_staging_files).toBe(0);
            expect(normalized.details).toEqual([]);
        });
    });
});
