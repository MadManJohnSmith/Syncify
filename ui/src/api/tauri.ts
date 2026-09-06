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
 * Sensitive key pattern to detect secrets, tokens, passwords, and cryptographic keys.
 * Matches case-insensitively: password, secret, client_secret, credentials,
 * credentials_json, token, access_token, refresh_token, api_key, crypto_key, private_key, auth.
 */
const SENSITIVE_KEY_REGEX = /(?:password|passwd|pwd|secret|client[_-]?secret|credentials?(?:[_-]?json)?|tokens?|access[_-]?token|refresh[_-]?token|api[_-]?key|apikey|crypto[_-]?key|private[_-]?key|^auth$|[_-]auth$|[_-]auth[_-]|authorization)/i;

/**
 * Suffixes that indicate status/state flags rather than sensitive secret values.
 * e.g., credentials_invalid: false, credentials_valid: true, credentials_expired: false.
 */
const NON_SENSITIVE_KEY_SUFFIX_REGEX = /(?:_valid|_invalid|_expired|_status|_count|_type)$/i;

/**
 * Check if an object key represents a sensitive field that must be redacted.
 */
export function isSensitiveKey(key: string): boolean {
    if (!key || typeof key !== 'string') return false;

    // Convert camelCase to snake_case for uniform evaluation
    const normalized = key.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase().trim();

    // Whitelist status/boolean/counter flags that contain credential/token/auth substrings
    // but only carry non-sensitive metadata (e.g. credentials_invalid, requires_auth, is_authenticated)
    if (NON_SENSITIVE_KEY_SUFFIX_REGEX.test(normalized) || /^(?:requires?_|is_)/i.test(normalized)) {
        return false;
    }

    return SENSITIVE_KEY_REGEX.test(normalized);
}

/**
 * Redact sensitive patterns inside free-form text strings (e.g. Bearer tokens, URLs with passwords, JWTs).
 */
export function sanitizeSensitiveString(str: string): string {
    if (!str || typeof str !== 'string') return str;

    return str
        .replace(/\b(bearer\s+)[a-zA-Z0-9_\-\.=]+/gi, '$1[REDACTED]')
        .replace(/\b(basic\s+)[a-zA-Z0-9+/=]{8,}/gi, '$1[REDACTED]')
        .replace(/\b(cookie\s*:\s*)[^\r\n]+/gi, '$1[REDACTED]')
        .replace(/\b((?:token|access_token|refresh_token|api[_-]?key|apikey|secret|client[_-]?secret|password|passwd|pwd|auth_token)\s*[:=]\s*)(['"]?)([^'"\s,;&]+)(\2)/gi, '$1$2[REDACTED]$4')
        .replace(/(https?:\/\/)([^:\s]+):([^@\s]+)@/gi, '$1$2:[REDACTED]@')
        .replace(/\beyJ[a-zA-Z0-9_-]{10,}\.eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_\-=]+/g, '[REDACTED]');
}

/**
 * Recursively sanitizes any payload (objects, arrays, strings, JSON strings)
 * to mask sensitive keys and credentials before logging to console or DevTools.
 *
 * Does not mutate the input argument.
 */
export function sanitizeSensitivePayload(data: unknown, visited = new WeakSet()): unknown {
    if (data === null || data === undefined) {
        return data;
    }

    if (typeof data === 'string') {
        const trimmed = data.trim();
        if ((trimmed.startsWith('{') && trimmed.endsWith('}')) || (trimmed.startsWith('[') && trimmed.endsWith(']'))) {
            try {
                const parsed = JSON.parse(trimmed);
                if (parsed !== null && typeof parsed === 'object') {
                    const sanitized = sanitizeSensitivePayload(parsed, visited);
                    return JSON.stringify(sanitized);
                }
            } catch {
                // Not valid JSON; fall through to string redaction
            }
        }
        return sanitizeSensitiveString(data);
    }

    if (typeof data !== 'object') {
        return data;
    }

    // Guard against circular references
    if (visited.has(data)) {
        return '[CIRCULAR]';
    }
    visited.add(data);

    if (Array.isArray(data)) {
        return data.map(item => sanitizeSensitivePayload(item, visited));
    }

    // Special objects (Date, RegExp, Error)
    if (data instanceof Date || data instanceof RegExp) {
        return data;
    }

    if (data instanceof Error) {
        return {
            name: data.name,
            message: sanitizeSensitiveString(data.message),
            stack: data.stack ? sanitizeSensitiveString(data.stack) : undefined,
        };
    }

    const sanitizedObj: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(data as Record<string, unknown>)) {
        if (isSensitiveKey(key)) {
            sanitizedObj[key] = '[REDACTED]';
        } else {
            sanitizedObj[key] = sanitizeSensitivePayload(value, visited);
        }
    }

    return sanitizedObj;
}

/**
 * Invoke a Tauri command with type safety and error handling
 */
export async function invokeCommand<T>(
    command: string,
    args?: Record<string, unknown>
): Promise<T> {
    const isProd = import.meta.env.PROD === true;
    try {
        if (!isProd) {
            console.debug(`[Tauri] Invoking: ${command}`, sanitizeSensitivePayload(args));
        }
        // Commands without arguments are invoked with a single argument so the
        // IPC signature matches the backend command exactly.
        const result = args === undefined
            ? await invoke<T>(command)
            : await invoke<T>(command, args);
        if (!isProd) {
            console.debug(`[Tauri] ${command} success:`, sanitizeSensitivePayload(result));
        }
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
