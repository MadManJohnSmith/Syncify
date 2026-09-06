/**
 * Concurrency & Asynchronous Flow Controls
 *
 * Provides primitives to prevent race conditions, serialize dependent calls,
 * enforce concurrency guards, and discard stale out-of-order responses.
 */

export interface ConcurrencyGuardOptions {
    maxConcurrent?: number;
}

export interface ConcurrencyGuard {
    run<T>(task: () => Promise<T>): Promise<T>;
    isBusy(): boolean;
    activeCount(): number;
}

/**
 * Creates a concurrency guard that limits the number of simultaneous executions
 * of an asynchronous task (default 1 = Mutex).
 * Ensures lock release in finally block even upon errors.
 */
export function createConcurrencyGuard(options: ConcurrencyGuardOptions = {}): ConcurrencyGuard {
    const max = Math.max(1, options.maxConcurrent ?? 1);
    let active = 0;

    return {
        async run<T>(task: () => Promise<T>): Promise<T> {
            if (active >= max) {
                throw new Error(
                    `ConcurrencyLimitExceeded: Maximum concurrent executions (${max}) reached.`
                );
            }
            active++;
            try {
                return await task();
            } finally {
                active = Math.max(0, active - 1);
            }
        },
        isBusy(): boolean {
            return active >= max;
        },
        activeCount(): number {
            return active;
        },
    };
}

export type LatestAsyncResult<T> =
    | { status: 'resolved'; data: T; sequence: number }
    | { status: 'discarded'; sequence: number; reason: 'stale' | 'cancelled' };

export interface LatestAsyncCaller<TArgs extends any[], TResult> {
    call(...args: TArgs): Promise<LatestAsyncResult<TResult>>;
    cancel(): void;
    latestSequence(): number;
    isPending(): boolean;
}

/**
 * Wraps an async function to prevent race conditions from out-of-order responses.
 * Each invocation receives a strictly monotonic sequence ID. If a newer request
 * finishes before an older request, the older request's response is safely discarded.
 * Supports explicit cancellation of in-flight requests.
 */
export function createLatestAsyncCaller<TArgs extends any[], TResult>(
    fn: (...args: TArgs) => Promise<TResult>
): LatestAsyncCaller<TArgs, TResult> {
    let nextSequence = 0;
    let latestResolvedSequence = 0;
    let pendingCount = 0;
    let cancelled = false;

    return {
        async call(...args: TArgs): Promise<LatestAsyncResult<TResult>> {
            cancelled = false;
            const seq = ++nextSequence;
            pendingCount++;

            try {
                const result = await fn(...args);

                if (cancelled) {
                    return { status: 'discarded', sequence: seq, reason: 'cancelled' };
                }

                // If a newer response has already been accepted, this one is stale
                if (seq < latestResolvedSequence) {
                    return { status: 'discarded', sequence: seq, reason: 'stale' };
                }

                latestResolvedSequence = seq;
                return { status: 'resolved', data: result, sequence: seq };
            } catch (err) {
                throw err;
            } finally {
                pendingCount = Math.max(0, pendingCount - 1);
            }
        },

        cancel(): void {
            cancelled = true;
            latestResolvedSequence = nextSequence;
        },

        latestSequence(): number {
            return latestResolvedSequence;
        },

        isPending(): boolean {
            return pendingCount > 0;
        },
    };
}

/**
 * Sequential FIFO async queue that processes tasks strictly in order.
 */
export function createAsyncQueue(concurrency: number = 1) {
    const limit = Math.max(1, concurrency);
    let running = 0;
    const queue: Array<() => void> = [];

    const next = () => {
        if (running < limit && queue.length > 0) {
            running++;
            const task = queue.shift();
            if (task) task();
        }
    };

    return {
        enqueue<T>(fn: () => Promise<T>): Promise<T> {
            return new Promise<T>((resolve, reject) => {
                const execute = async () => {
                    try {
                        const res = await fn();
                        resolve(res);
                    } catch (err) {
                        reject(err);
                    } finally {
                        running--;
                        next();
                    }
                };

                queue.push(execute);
                next();
            });
        },
        size(): number {
            return queue.length;
        },
        isIdle(): boolean {
            return running === 0 && queue.length === 0;
        },
    };
}
