/**
 * Defensive response-normalization helpers
 *
 * Shared primitives used by every API module to guarantee that a Tauri command
 * response can never crash the UI because the backend omitted a field, returned
 * null instead of an array, or serialized keys in camelCase vs snake_case.
 *
 * Reference pattern: `enqueueTracks` in api/library.ts (crash regression S176Q).
 */

/**
 * Returns the value as an array; null/undefined/non-array payloads become [].
 */
export function asArray<T = unknown>(value: unknown): T[] {
    return Array.isArray(value) ? (value as T[]) : [];
}

/**
 * Returns the value as a finite number, or the fallback when missing/invalid.
 */
export function asNumber(value: unknown, fallback: number = 0): number {
    return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

/**
 * Like asNumber but preserves "field absent" (undefined) for optional contracts.
 */
export function optionalNumber(value: unknown): number | undefined {
    return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

/**
 * Returns the value as a string, or the fallback when missing/not a string.
 */
export function asString(value: unknown, fallback: string = ''): string {
    return typeof value === 'string' ? value : fallback;
}

/**
 * Returns the value as a boolean, or the fallback when missing/not a boolean.
 */
export function asBoolean(value: unknown, fallback: boolean = false): boolean {
    return typeof value === 'boolean' ? value : fallback;
}

/**
 * Like asBoolean but preserves "field absent" (undefined) for optional contracts.
 */
export function optionalBoolean(value: unknown): boolean | undefined {
    return typeof value === 'boolean' ? value : undefined;
}

/**
 * Returns the value as a plain object record, or null otherwise.
 */
export function asRecord(value: unknown): Record<string, unknown> | null {
    return value !== null && typeof value === 'object' && !Array.isArray(value)
        ? (value as Record<string, unknown>)
        : null;
}

/**
 * Reads the first non-nullish key from a raw payload.
 *
 * Backend commands mix camelCase (`#[serde(rename_all = "camelCase")]`) and
 * snake_case serialization across modules, so callers pass both spellings.
 */
export function pick<T = unknown>(raw: unknown, keys: string[]): T | undefined {
    const rec = asRecord(raw);
    if (!rec) return undefined;
    for (const key of keys) {
        const value = rec[key];
        if (value !== undefined && value !== null) return value as T;
    }
    return undefined;
}

/**
 * pick() + asNumber(): first present numeric key or the fallback.
 */
export function pickNumber(raw: unknown, keys: string[], fallback: number = 0): number {
    const value = pick<number>(raw, keys);
    return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

/**
 * pick() + asArray(): first present array-valued key or [].
 */
export function pickArray<T = unknown>(raw: unknown, keys: string[]): T[] {
    const value = pick<unknown>(raw, keys);
    return asArray<T>(value);
}
