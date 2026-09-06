/**
 * Tauri API Base Helper
 * 
 * Provides typed invoke wrapper with error handling for Tauri commands.
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Custom error class for Tauri command failures
 */
export class TauriError extends Error {
    constructor(
        message: string,
        public command?: string,
        public details?: unknown
    ) {
        super(message);
        this.name = 'TauriError';
    }
}

/**
 * Invoke a Tauri command with type safety and error handling
 */
export async function invokeCommand<T>(
    command: string,
    args?: Record<string, unknown>
): Promise<T> {
    try {
        console.debug(`[Tauri] Invoking: ${command}`, args);
        // Commands without arguments are invoked with a single argument so the
        // IPC signature matches the backend command exactly.
        const result = args === undefined
            ? await invoke<T>(command)
            : await invoke<T>(command, args);
        console.debug(`[Tauri] ${command} success:`, result);
        return result;
    } catch (error) {
        console.error(`[Tauri] Command "${command}" failed:`, error);

        // Parse Rust error response (typically a string)
        if (typeof error === 'string') {
            throw new TauriError(error, command);
        }

        // Handle structured error
        if (error instanceof Error) {
            throw new TauriError(error.message, command, error);
        }

        throw new TauriError(
            `Failed to execute command: ${command}`,
            command,
            error
        );
    }
}

/**
 * Type-safe command creator factory
 * 
 * @example
 * const getLibrary = createCommand<void, Track[]>('get_library');
 * const tracks = await getLibrary();
 */
export function createCommand<TArgs = void, TResult = void>(
    commandName: string
) {
    return async (args?: TArgs): Promise<TResult> => {
        return invokeCommand<TResult>(commandName, args as Record<string, unknown>);
    };
}

/**
 * Check if running in Tauri environment
 */
export function isTauri(): boolean {
    return (
        typeof window !== 'undefined' &&
        ('__TAURI_INTERNALS__' in window || '__TAURI__' in window || !!(window as any).isTauri)
    );
}

/**
 * Safe invoke that returns null if not in Tauri environment
 */
export async function safeInvoke<T>(
    command: string,
    args?: Record<string, unknown>
): Promise<T | null> {
    if (!isTauri()) {
        console.warn(`[Tauri] Not in Tauri environment, skipping: ${command}`);
        return null;
    }
    return invokeCommand<T>(command, args);
}
