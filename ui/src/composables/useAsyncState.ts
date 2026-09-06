/**
 * Async State Composable
 * 
 * Wraps async operations with loading, error, and data state.
 */

import { ref, readonly, watch, type Ref } from 'vue';

export interface AsyncState<T> {
    data: Readonly<Ref<T | null>>;
    loading: Readonly<Ref<boolean>>;
    error: Readonly<Ref<Error | null>>;
    execute: (...args: any[]) => Promise<T>;
    reset: () => void;
}

/**
 * Composable for handling async operations with state management
 * 
 * @example
 * const { data: tracks, loading, error, execute } = useAsyncState(
 *   () => libraryApi.getLibrary(),
 *   []
 * );
 * 
 * onMounted(() => execute());
 */
export function useAsyncState<T, Args extends any[] = []>(
    fn: (...args: Args) => Promise<T>,
    initialValue: T | null = null
): AsyncState<T> {
    const data = ref<T | null>(initialValue) as Ref<T | null>;
    const loading = ref(false);
    const error = ref<Error | null>(null);

    async function execute(...args: Args): Promise<T> {
        loading.value = true;
        error.value = null;

        try {
            const result = await fn(...args);
            data.value = result;
            return result;
        } catch (e) {
            error.value = e instanceof Error ? e : new Error(String(e));
            throw e;
        } finally {
            loading.value = false;
        }
    }

    function reset(): void {
        data.value = initialValue;
        loading.value = false;
        error.value = null;
    }

    return {
        data: readonly(data) as Readonly<Ref<T | null>>,
        loading: readonly(loading),
        error: readonly(error),
        execute,
        reset,
    };
}

/**
 * Composable for persisted state in localStorage
 * 
 * @example
 * const viewMode = usePersistedState('library-view-mode', 'list');
 */
export function usePersistedState<T>(
    key: string,
    defaultValue: T
): Ref<T> {
    const stored = localStorage.getItem(key);
    let initialValue = defaultValue;
    if (stored !== null) {
        try {
            initialValue = JSON.parse(stored);
        } catch {
            initialValue = defaultValue;
        }
    }
    const state = ref<T>(initialValue) as Ref<T>;

    // Watch for changes and persist
    watch(
        state,
        (newValue) => {
            localStorage.setItem(key, JSON.stringify(newValue));
        },
        { deep: true }
    );

    return state;
}

/**
 * Debounce helper
 */
export function debounce<T extends (...args: any[]) => any>(
    fn: T,
    delay: number
): (...args: Parameters<T>) => void {
    let timeoutId: ReturnType<typeof setTimeout>;

    return (...args: Parameters<T>) => {
        clearTimeout(timeoutId);
        timeoutId = setTimeout(() => fn(...args), delay);
    };
}
