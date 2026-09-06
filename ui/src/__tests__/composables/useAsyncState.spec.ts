import { describe, it, expect, beforeEach, vi } from 'vitest';
import { nextTick } from 'vue';
import { useAsyncState, usePersistedState, debounce } from '@/composables/useAsyncState';

describe('useAsyncState Composable (TASK-58)', () => {
    beforeEach(() => {
        localStorage.clear();
        vi.clearAllMocks();
    });

    describe('usePersistedState', () => {
        it('returns defaultValue when localStorage is empty', () => {
            const state = usePersistedState('test_key', 'initial');
            expect(state.value).toBe('initial');
        });

        it('loads initial value correctly when valid JSON exists in localStorage', () => {
            localStorage.setItem('test_key', JSON.stringify({ count: 42 }));
            const state = usePersistedState('test_key', { count: 0 });
            expect(state.value).toEqual({ count: 42 });
        });

        it('handles corrupted or invalid JSON in localStorage gracefully and falls back to defaultValue', () => {
            localStorage.setItem('corrupted_key', '{invalid-json');
            expect(() => {
                const state = usePersistedState('corrupted_key', { safe: true });
                expect(state.value).toEqual({ safe: true });
            }).not.toThrow();
        });

        it('handles malformed primitive strings in localStorage gracefully', () => {
            localStorage.setItem('malformed_num', '123badNumber');
            const state = usePersistedState('malformed_num', 999);
            expect(state.value).toBe(999);
        });

        it('persists mutations to localStorage', async () => {
            const state = usePersistedState('sync_key', 'first');
            expect(state.value).toBe('first');

            state.value = 'second';
            await nextTick();

            expect(localStorage.getItem('sync_key')).toBe(JSON.stringify('second'));
        });

        it('persists deep object mutations to localStorage', async () => {
            const state = usePersistedState('deep_key', { nested: { val: 1 } });

            state.value.nested.val = 2;
            await nextTick();

            expect(JSON.parse(localStorage.getItem('deep_key')!)).toEqual({ nested: { val: 2 } });
        });
    });

    describe('useAsyncState', () => {
        it('executes async function and manages loading, data, and error state', async () => {
            const mockFn = vi.fn().mockResolvedValue('success data');
            const { data, loading, error, execute } = useAsyncState(mockFn, 'default');

            expect(data.value).toBe('default');
            expect(loading.value).toBe(false);
            expect(error.value).toBeNull();

            const promise = execute();
            expect(loading.value).toBe(true);

            const result = await promise;
            expect(result).toBe('success data');
            expect(data.value).toBe('success data');
            expect(loading.value).toBe(false);
            expect(error.value).toBeNull();
        });

        it('handles execution errors and updates error state', async () => {
            const mockError = new Error('Async failure');
            const mockFn = vi.fn().mockRejectedValue(mockError);
            const { data, loading, error, execute } = useAsyncState(mockFn, null);

            await expect(execute()).rejects.toThrow('Async failure');

            expect(loading.value).toBe(false);
            expect(error.value).toBe(mockError);
            expect(data.value).toBeNull();
        });

        it('resets state to initial values', async () => {
            const mockFn = vi.fn().mockResolvedValue('new data');
            const { data, loading, error, execute, reset } = useAsyncState(mockFn, 'initial');

            await execute();
            expect(data.value).toBe('new data');

            reset();
            expect(data.value).toBe('initial');
            expect(loading.value).toBe(false);
            expect(error.value).toBeNull();
        });
    });

    describe('debounce', () => {
        it('debounces execution of provided function', () => {
            vi.useFakeTimers();
            const fn = vi.fn();
            const debounced = debounce(fn, 200);

            debounced('call 1');
            debounced('call 2');
            debounced('call 3');

            expect(fn).not.toHaveBeenCalled();

            vi.advanceTimersByTime(199);
            expect(fn).not.toHaveBeenCalled();

            vi.advanceTimersByTime(1);
            expect(fn).toHaveBeenCalledTimes(1);
            expect(fn).toHaveBeenCalledWith('call 3');

            vi.useRealTimers();
        });
    });
});
