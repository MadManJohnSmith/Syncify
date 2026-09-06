import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getConcurrencyStatsSummary,
    getActiveConcurrencyLocks,
    normalizeConcurrencyStatsSummary,
} from '../../api/metadata';
import {
    createConcurrencyGuard,
    createLatestAsyncCaller,
    createAsyncQueue,
} from '../../api/concurrency';
import * as tauri from '../../api/tauri';

describe('Concurrency Diagnostics & Asynchronous Flow Controls (TASK-29)', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    describe('Race Conditions & Out-of-Order Response Discarding', () => {
        it('discards stale out-of-order responses when a slower initial request resolves after a faster subsequent request', async () => {
            // Function that resolves with variable artificial delay
            const queryCatalog = async (query: string, delayMs: number) => {
                await new Promise((resolve) => setTimeout(resolve, delayMs));
                return `results_for_${query}`;
            };

            const latestCaller = createLatestAsyncCaller(queryCatalog);
            let currentState = '';

            // Request A is initiated first (slow: 60ms)
            const promiseA = latestCaller.call('queryA', 60).then((res) => {
                if (res.status === 'resolved') {
                    currentState = res.data;
                }
                return res;
            });

            // Request B is initiated shortly after (fast: 15ms)
            const promiseB = latestCaller.call('queryB', 15).then((res) => {
                if (res.status === 'resolved') {
                    currentState = res.data;
                }
                return res;
            });

            const [resA, resB] = await Promise.all([promiseA, promiseB]);

            // Request B should have resolved and updated state
            expect(resB.status).toBe('resolved');
            if (resB.status === 'resolved') {
                expect(resB.data).toBe('results_for_queryB');
            }

            // Request A should have been safely discarded as stale
            expect(resA.status).toBe('discarded');
            if (resA.status === 'discarded') {
                expect(resA.reason).toBe('stale');
            }

            // State reflects the newest request (B), NOT overwritten by slow request (A)
            expect(currentState).toBe('results_for_queryB');
        });

        it('discards in-flight responses when explicitly cancelled', async () => {
            const slowOperation = async () => {
                await new Promise((resolve) => setTimeout(resolve, 40));
                return 'sensitive_completed_data';
            };

            const latestCaller = createLatestAsyncCaller(slowOperation);
            let state = 'initial';

            const callPromise = latestCaller.call().then((res) => {
                if (res.status === 'resolved') {
                    state = res.data;
                }
                return res;
            });

            expect(latestCaller.isPending()).toBe(true);

            // Cancel immediately while in-flight
            latestCaller.cancel();

            const result = await callPromise;

            expect(result.status).toBe('discarded');
            if (result.status === 'discarded') {
                expect(result.reason).toBe('cancelled');
            }
            // State remains untouched
            expect(state).toBe('initial');
            expect(latestCaller.isPending()).toBe(false);
        });
    });

    describe('Concurrency Guards & Serialization', () => {
        it('guards against concurrent overlapping executions and blocks secondary invocations', async () => {
            const guard = createConcurrencyGuard({ maxConcurrent: 1 });
            let runningCount = 0;
            let peakConcurrent = 0;

            const executeGuarded = async (id: number, delayMs: number) => {
                return guard.run(async () => {
                    runningCount++;
                    peakConcurrent = Math.max(peakConcurrent, runningCount);
                    await new Promise((resolve) => setTimeout(resolve, delayMs));
                    runningCount--;
                    return `done_${id}`;
                });
            };

            // Start first task
            const task1Promise = executeGuarded(1, 40);

            // Attempt overlapping task 2 immediately
            await expect(executeGuarded(2, 20)).rejects.toThrow(
                /ConcurrencyLimitExceeded: Maximum concurrent executions \(1\) reached/i
            );

            // Wait for task 1 to complete
            const result1 = await task1Promise;
            expect(result1).toBe('done_1');
            expect(peakConcurrent).toBe(1);
            expect(guard.isBusy()).toBe(false);

            // Subsequent task 3 can now proceed
            const result3 = await executeGuarded(3, 10);
            expect(result3).toBe('done_3');
        });

        it('releases lock cleanly in finally block even when a guarded task throws an error', async () => {
            const guard = createConcurrencyGuard({ maxConcurrent: 1 });

            // Run a task that rejects
            await expect(
                guard.run(async () => {
                    throw new Error('Task internal failure');
                })
            ).rejects.toThrow('Task internal failure');

            // Guard must not be locked or deadlocked
            expect(guard.isBusy()).toBe(false);
            expect(guard.activeCount()).toBe(0);

            // Next call must succeed
            const nextResult = await guard.run(async () => 'recovered_call');
            expect(nextResult).toBe('recovered_call');
        });

        it('serializes async tasks in strict FIFO order using createAsyncQueue', async () => {
            const queue = createAsyncQueue(1);
            const executionOrder: number[] = [];

            const enqueueTask = (id: number, durationMs: number) => {
                return queue.enqueue(async () => {
                    await new Promise((resolve) => setTimeout(resolve, durationMs));
                    executionOrder.push(id);
                    return id;
                });
            };

            // Enqueue tasks where task 1 takes longer than task 2 and 3
            const p1 = enqueueTask(1, 30);
            const p2 = enqueueTask(2, 10);
            const p3 = enqueueTask(3, 5);

            await Promise.all([p1, p2, p3]);

            // Strict FIFO order preserved despite varying delays
            expect(executionOrder).toEqual([1, 2, 3]);
            expect(queue.isIdle()).toBe(true);
        });
    });

    describe('getConcurrencyStatsSummary Normalization & Diagnostics', () => {
        it('normalizes real backend camelCase metrics and defaults missing fields to 0', async () => {
            const rawBackendSummary = {
                totalAcquisitions: 150,
                contendedAcquisitions: 12,
                timeouts: 0,
                activeLocksCount: 3,
                maxWaitDurationMs: 45,
                maxHeldDurationMs: 120,
            };

            const invokeSpy = vi.spyOn(tauri, 'invokeCommand').mockResolvedValue(rawBackendSummary);

            const result = await getConcurrencyStatsSummary();

            expect(invokeSpy).toHaveBeenCalledWith('get_concurrency_stats_summary');
            expect(result.total_acquisitions).toBe(150);
            expect(result.contended_acquisitions).toBe(12);
            expect(result.timeouts).toBe(0);
            expect(result.active_locks_count).toBe(3);
            expect(result.max_wait_duration_ms).toBe(45);
            expect(result.max_held_duration_ms).toBe(120);

            invokeSpy.mockRestore();
        });

        it('returns safe zeroed counters when backend returns null or empty payload', () => {
            const normalizedFromNull = normalizeConcurrencyStatsSummary(null);
            expect(normalizedFromNull.total_acquisitions).toBe(0);
            expect(normalizedFromNull.contended_acquisitions).toBe(0);
            expect(normalizedFromNull.timeouts).toBe(0);
            expect(normalizedFromNull.active_locks_count).toBe(0);
            expect(normalizedFromNull.max_wait_duration_ms).toBe(0);
            expect(normalizedFromNull.max_held_duration_ms).toBe(0);
        });
    });

    describe('getActiveConcurrencyLocks Sanitization', () => {
        it('normalizes lock list, filtering out nulls, non-strings, and whitespace entries', async () => {
            const rawLocks = [
                'lock:1234567890abcdef',
                null,
                '',
                '   ',
                'lock:fedcba0987654321',
                12345, // invalid type
            ];

            const invokeSpy = vi.spyOn(tauri, 'invokeCommand').mockResolvedValue(rawLocks);

            const result = await getActiveConcurrencyLocks();

            expect(invokeSpy).toHaveBeenCalledWith('get_active_concurrency_locks');
            expect(result).toHaveLength(2);
            expect(result[0]).toBe('lock:1234567890abcdef');
            expect(result[1]).toBe('lock:fedcba0987654321');

            invokeSpy.mockRestore();
        });

        it('returns [] when backend returns null for lock list', async () => {
            const invokeSpy = vi.spyOn(tauri, 'invokeCommand').mockResolvedValue(null);

            const result = await getActiveConcurrencyLocks();

            expect(result).toEqual([]);
            invokeSpy.mockRestore();
        });
    });
});
