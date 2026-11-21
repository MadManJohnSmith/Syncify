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
 * Like asString but preserves "field absent" (null) for optional contracts.
 */
export function optionalString(value: unknown): string | null {
    return typeof value === 'string' && value.length > 0 ? value : null;
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

/** Bullet mask `maskEmail` renders in place of hidden characters. */
const EMAIL_MASK = '•••';

/**
 * Masks an email address for display (ADR-0007 decision 7): the address is
 * identity data, but the full value must never be rendered in the UI nor
 * written to logs. Keeps the first characters of the local part and the whole
 * domain, so two accounts of the same provider stay tellable apart, and hides
 * the rest.
 *
 * - `ana@spotify.com` → `an•••@spotify.com` (local part ≥ 3 chars: keep 2)
 * - `jo@example.com`  → `j•••@example.com`  (2 chars: keep 1)
 * - `a@example.com`   → `•••@example.com`   (1 char: keep none)
 * - `not-an-email`, `a@b@c.com`, `@x.com`, `x@` → `•••` (not an address: hide it all)
 * - null, undefined or blank → null (absence stays absent)
 *
 * Callers feed raw backend values only: the API layer masks each payload once
 * at the boundary, so a masked address is never masked a second time.
 */
export function maskEmail(value: string | null | undefined): string | null {
    if (value == null) return null;
    const trimmed = value.trim();
    if (!trimmed) return null;
    const parts = trimmed.split('@');
    if (parts.length !== 2 || !parts[0] || !parts[1]) return EMAIL_MASK;
    const [local, domain] = parts;
    const stem = local.length >= 3 ? local.slice(0, 2) : local.slice(0, local.length - 1);
    return `${stem}${EMAIL_MASK}@${domain}`;
}
