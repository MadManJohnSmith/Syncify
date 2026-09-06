/**
 * Resilience & Operation Recovery Utilities
 *
 * Provides retry mechanisms with exponential backoff, error taxonomy classification,
 * controlled fallbacks, and local operation state preservation across disconnects.
 */

import type { RecoveryAuditSummary } from './types';

export interface RetryOptions<T> {
    maxRetries?: number;
    initialDelayMs?: number;
    backoffMultiplier?: number;
    maxDelayMs?: number;
    isRetryable?: (err: unknown) => boolean;
    fallback?: T | ((err: unknown) => T | Promise<T>);
    onRetry?: (err: unknown, attempt: number, delayMs: number) => void;
    sleepFn?: (ms: number) => Promise<void>;
}

const NON_RETRYABLE_PATTERNS = [
    /authinvalid/i,
    /unauthorized/i,
    /token expired/i,
    /forbidden/i,
    /not found/i,
    /invalidinput/i,
    /validation/i,
    /terminal/i,
];

const RETRYABLE_PATTERNS = [
    /database locked/i,
    /busy/i,
    /locked/i,
    /timeout/i,
    /econnreset/i,
    /econnrefused/i,
    /network/i,
    /temporary/i,
    /ipc disconnected/i,
    /connection reset/i,
    /temporarily unavailable/i,
];

/**
 * Classifies an error (Error instance, Rust error string, or unknown)
 * into retryable vs permanent terminal errors.
 */
export function isRetryableError(error: unknown): boolean {
    if (!error) return false;

    const message = error instanceof Error ? error.message : String(error);

    // Check non-retryable first
    for (const pattern of NON_RETRYABLE_PATTERNS) {
        if (pattern.test(message)) {
            return false;
        }
    }

    // Check retryable patterns
    for (const pattern of RETRYABLE_PATTERNS) {
        if (pattern.test(message)) {
            return true;
        }
    }

    // Default to true for transient errors if not explicitly classified as terminal
    return true;
}

const defaultSleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

/**
 * Executes an asynchronous operation with resilience:
 * - Retries on retryable errors using exponential backoff.
 * - Controlled fallback upon terminal failure or retry exhaustion.
 * - Hooks for notifications/logging on each retry attempt.
 */
export async function executeWithRecovery<T>(
    fn: () => Promise<T>,
    options: RetryOptions<T> = {}
): Promise<T> {
    const maxRetries = options.maxRetries ?? 3;
    const initialDelayMs = options.initialDelayMs ?? 50;
    const backoffMultiplier = options.backoffMultiplier ?? 2;
    const maxDelayMs = options.maxDelayMs ?? 2000;
    const isRetryable = options.isRetryable ?? isRetryableError;
    const sleep = options.sleepFn ?? defaultSleep;

    let attempt = 0;

    while (true) {
        try {
            return await fn();
        } catch (err) {
            attempt++;
            const canRetry = attempt <= maxRetries && isRetryable(err);

            if (canRetry) {
                const delay = Math.min(
                    initialDelayMs * Math.pow(backoffMultiplier, attempt - 1),
                    maxDelayMs
                );
                if (options.onRetry) {
                    options.onRetry(err, attempt, delay);
                }
                await sleep(delay);
                continue;
            }

            // Fallback handling
            if (options.fallback !== undefined) {
                if (typeof options.fallback === 'function') {
                    return await (options.fallback as (e: unknown) => T | Promise<T>)(err);
                }
                return options.fallback;
            }

            throw err;
        }
    }
}

export type OperationRecoveryState = 'pending' | 'in_progress' | 'interrupted' | 'recovered' | 'failed_terminal';

export interface TrackedOperation<TData = unknown> {
    id: string;
    type: string;
    status: OperationRecoveryState;
    data?: TData;
    error?: string | null;
    updatedAt: string;
}

/**
 * Coordinator to track and preserve in-flight operations locally across
 * network disconnects, crashes, and startup reconciliations.
 */
export function createOperationRecoveryTracker<TData = unknown>() {
    const operations = new Map<string, TrackedOperation<TData>>();

    return {
        registerOperation(id: string, type: string, initialData?: TData): TrackedOperation<TData> {
            const op: TrackedOperation<TData> = {
                id,
                type,
                status: 'in_progress',
                data: initialData,
                error: null,
                updatedAt: new Date().toISOString(),
            };
            operations.set(id, op);
            return { ...op };
        },

        markInterrupted(id: string, reason?: string): void {
            const op = operations.get(id);
            if (op) {
                op.status = 'interrupted';
                op.error = reason ?? 'Operation interrupted due to disconnect';
                op.updatedAt = new Date().toISOString();
            }
        },

        markRecovered(id: string): void {
            const op = operations.get(id);
            if (op) {
                op.status = 'recovered';
                op.error = null;
                op.updatedAt = new Date().toISOString();
            }
        },

        markFailedTerminal(id: string, error?: string): void {
            const op = operations.get(id);
            if (op) {
                op.status = 'failed_terminal';
                op.error = error ?? 'Operation failed permanently';
                op.updatedAt = new Date().toISOString();
            }
        },

        getOperation(id: string): TrackedOperation<TData> | undefined {
            const op = operations.get(id);
            return op ? { ...op } : undefined;
        },

        getAllOperations(): TrackedOperation<TData>[] {
            return Array.from(operations.values()).map((op) => ({ ...op }));
        },

        reconcileWithSummary(summary: RecoveryAuditSummary): void {
            for (const detail of summary.details) {
                const local = operations.get(detail.operation_id);
                if (local) {
                    if (detail.new_status === 'recovered') {
                        local.status = 'recovered';
                        local.error = null;
                    } else if (detail.new_status === 'interrupted') {
                        local.status = 'interrupted';
                        local.error = detail.message;
                    } else if (detail.new_status === 'failed_terminal') {
                        local.status = 'failed_terminal';
                        local.error = detail.message;
                    }
                    local.updatedAt = new Date().toISOString();
                }
            }
        },

        clear(): void {
            operations.clear();
        },
    };
}
