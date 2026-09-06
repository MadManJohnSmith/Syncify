/**
 * Security Test Suite: TASK-07 / SEC-011
 * 
 * Verifies secret and token obfuscation in Tauri IPC logger (ui/src/api/tauri.ts):
 * 1. Redaction of sensitive fields in arguments before logging to DevTools console.
 * 2. Redaction of sensitive fields in returned command results before logging.
 * 3. Preservation of unredacted payloads for Rust invoke IPC and for caller return values.
 * 4. Deep recursive sanitization of nested objects, arrays, and JSON strings.
 * 5. Preservation of non-sensitive metadata (e.g. credentials_invalid: boolean, author, title).
 * 6. Complete suppression of console.debug in production environment (import.meta.env.PROD === true).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
    invokeCommand,
    sanitizeSensitivePayload,
    isSensitiveKey,
    sanitizeSensitiveString,
} from '@/api/tauri';

describe('TASK-07 / SEC-011: Tauri Logger Security & Secret Obfuscation', () => {
    let debugSpy: ReturnType<typeof vi.spyOn>;
    const originalProd = import.meta.env.PROD;

    beforeEach(() => {
        debugSpy = vi.spyOn(console, 'debug').mockImplementation(() => {});
        (import.meta.env as any).PROD = false;
        vi.mocked(invoke).mockReset();
    });

    afterEach(() => {
        debugSpy.mockRestore();
        (import.meta.env as any).PROD = originalProd;
    });

    describe('1. isSensitiveKey unit evaluations', () => {
        it('identifies sensitive keys in snake_case, camelCase, and varied casing', () => {
            const sensitiveKeys = [
                'password',
                'Password',
                'user_password',
                'passwd',
                'pwd',
                'secret',
                'client_secret',
                'spotify_client_secret',
                'clientSecret',
                'spotifyClientSecret',
                'credentials',
                'credentials_json',
                'credentialsJson',
                'token',
                'access_token',
                'accessToken',
                'refresh_token',
                'refreshToken',
                'api_key',
                'apiKey',
                'apikey',
                'crypto_key',
                'cryptoKey',
                'private_key',
                'privateKey',
                'auth',
                'authorization',
                'auth_token',
                'authToken',
            ];

            for (const key of sensitiveKeys) {
                expect(isSensitiveKey(key), `Expected "${key}" to be detected as sensitive`).toBe(true);
            }
        });

        it('does not flag non-sensitive metadata or status flags', () => {
            const nonSensitiveKeys = [
                'title',
                'artist_name',
                'author',
                'authority',
                'duration_ms',
                'isrc',
                'credentials_invalid',
                'credentialsInvalid',
                'credentials_valid',
                'credentials_expired',
                'download_path',
                'track_id',
                'token_status',
                'token_count',
                'requires_auth',
                'requiresAuth',
                'is_authenticated',
                'isAuthenticated',
            ];

            for (const key of nonSensitiveKeys) {
                expect(isSensitiveKey(key), `Expected "${key}" to NOT be detected as sensitive`).toBe(false);
            }
        });
    });

    describe('2. sanitizeSensitivePayload deep recursive masking', () => {
        it('redacts top-level and nested sensitive keys in plain objects', () => {
            const payload = {
                username: 'auditor',
                password: 'plain_password_123',
                spotify_client_secret: 'spot_sec_999',
                account: {
                    service: 'qobuz',
                    credentials_json: '{"app_secret":"nested_secret"}',
                    nested: {
                        access_token: 'acc_tok_456',
                        refreshToken: 'ref_tok_789',
                        apiKey: 'key_abc',
                        cryptoKey: 'crypto_xyz',
                        privateKey: 'priv_key_000',
                    },
                },
                safe_metadata: {
                    track: 'Bohemian Rhapsody',
                    author: 'Queen',
                    credentials_invalid: false,
                },
            };

            const sanitized = sanitizeSensitivePayload(payload) as any;

            expect(sanitized.username).toBe('auditor');
            expect(sanitized.password).toBe('[REDACTED]');
            expect(sanitized.spotify_client_secret).toBe('[REDACTED]');
            expect(sanitized.account.credentials_json).toBe('[REDACTED]');
            expect(sanitized.account.nested.access_token).toBe('[REDACTED]');
            expect(sanitized.account.nested.refreshToken).toBe('[REDACTED]');
            expect(sanitized.account.nested.apiKey).toBe('[REDACTED]');
            expect(sanitized.account.nested.cryptoKey).toBe('[REDACTED]');
            expect(sanitized.account.nested.privateKey).toBe('[REDACTED]');
            expect(sanitized.safe_metadata.track).toBe('Bohemian Rhapsody');
            expect(sanitized.safe_metadata.author).toBe('Queen');
            expect(sanitized.safe_metadata.credentials_invalid).toBe(false);
        });

        it('does not mutate the original input payload in place', () => {
            const original = {
                user: 'admin',
                spotify_client_secret: 'original_secret_123',
                nested: {
                    token: 'original_token_456',
                },
            };

            const copy = JSON.parse(JSON.stringify(original));
            sanitizeSensitivePayload(original);

            expect(original).toEqual(copy);
            expect(original.spotify_client_secret).toBe('original_secret_123');
            expect(original.nested.token).toBe('original_token_456');
        });

        it('redacts elements within arrays and array of objects', () => {
            const list = [
                { id: 1, access_token: 'tok1', name: 'Item 1' },
                { id: 2, password: 'pwd2', name: 'Item 2' },
                'normal_string',
            ];

            const sanitized = sanitizeSensitivePayload(list) as any[];

            expect(sanitized[0].access_token).toBe('[REDACTED]');
            expect(sanitized[0].name).toBe('Item 1');
            expect(sanitized[1].password).toBe('[REDACTED]');
            expect(sanitized[1].name).toBe('Item 2');
            expect(sanitized[2]).toBe('normal_string');
        });

        it('parses and redacts secrets within serialized JSON strings', () => {
            const rawJsonString = JSON.stringify({
                service: 'spotify',
                client_secret: 'sec_in_json',
                details: {
                    refresh_token: 'ref_in_json',
                },
                safe_name: 'dev_app',
            });

            const sanitized = sanitizeSensitivePayload(rawJsonString);
            expect(typeof sanitized).toBe('string');

            const parsed = JSON.parse(sanitized as string);
            expect(parsed.service).toBe('spotify');
            expect(parsed.client_secret).toBe('[REDACTED]');
            expect(parsed.details.refresh_token).toBe('[REDACTED]');
            expect(parsed.safe_name).toBe('dev_app');
        });

        it('sanitizes Bearer tokens, URLs with credentials, and JWTs in free-form strings', () => {
            const bearerStr = 'Authorization: Bearer secret_bearer_token_xyz';
            expect(sanitizeSensitiveString(bearerStr)).toBe('Authorization: Bearer [REDACTED]');

            const urlWithPass = 'https://syncify_user:super_secret_pw@music.local/stream';
            expect(sanitizeSensitiveString(urlWithPass)).toBe('https://syncify_user:[REDACTED]@music.local/stream');

            const jwt = 'Header eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.c2VjcmV0X3NpZ25hdHVyZQ== Trailer';
            expect(sanitizeSensitiveString(jwt)).toBe('Header [REDACTED] Trailer');
        });

        it('gracefully handles circular references without throwing', () => {
            const circular: any = { name: 'cyclic_node' };
            circular.self = circular;

            const sanitized = sanitizeSensitivePayload(circular) as any;
            expect(sanitized.name).toBe('cyclic_node');
            expect(sanitized.self).toBe('[CIRCULAR]');
        });

        it('preserves primitive values and null/undefined', () => {
            expect(sanitizeSensitivePayload(null)).toBeNull();
            expect(sanitizeSensitivePayload(undefined)).toBeUndefined();
            expect(sanitizeSensitivePayload(42)).toBe(42);
            expect(sanitizeSensitivePayload(true)).toBe(true);
            expect(sanitizeSensitivePayload('simple text')).toBe('simple text');
        });
    });

    describe('3. invokeCommand logging and isolation', () => {
        it('redacts sensitive arguments in console.debug while passing unredacted payload to invoke', async () => {
            vi.mocked(invoke).mockResolvedValueOnce({ status: 'ok' });

            const sensitiveArgs = {
                service: 'spotify',
                settings: {
                    spotify_client_id: 'client_123',
                    spotify_client_secret: 'actual_real_secret_do_not_leak',
                },
            };

            await invokeCommand('save_settings_batch', sensitiveArgs);

            // Verify Tauri invoke IPC received the intact, unmodified secret
            expect(invoke).toHaveBeenCalledTimes(1);
            expect(invoke).toHaveBeenCalledWith('save_settings_batch', sensitiveArgs);
            expect(sensitiveArgs.settings.spotify_client_secret).toBe('actual_real_secret_do_not_leak');

            // Verify console.debug received the redacted copy
            expect(debugSpy).toHaveBeenCalledTimes(2);
            const invokeLogArgs = debugSpy.mock.calls[0];
            expect(invokeLogArgs[0]).toBe('[Tauri] Invoking: save_settings_batch');

            const loggedPayload = invokeLogArgs[1] as any;
            expect(loggedPayload.settings.spotify_client_id).toBe('client_123');
            expect(loggedPayload.settings.spotify_client_secret).toBe('[REDACTED]');
        });

        it('redacts sensitive result payloads in console.debug while returning unredacted result to caller', async () => {
            const rawBackendResponse = {
                accountId: 10,
                access_token: 'tok_live_123456',
                refresh_token: 'tok_refresh_987654',
                expires_in: 3600,
            };

            vi.mocked(invoke).mockResolvedValueOnce(rawBackendResponse);

            const result = await invokeCommand<typeof rawBackendResponse>('authenticate_account', { code: 'auth_code_123' });

            // Caller receives unmutated original data
            expect(result.access_token).toBe('tok_live_123456');
            expect(result.refresh_token).toBe('tok_refresh_987654');
            expect(result.expires_in).toBe(3600);

            // Log output masked both args and results
            expect(debugSpy).toHaveBeenCalledTimes(2);
            const successLog = debugSpy.mock.calls[1];
            expect(successLog[0]).toBe('[Tauri] authenticate_account success:');

            const loggedResult = successLog[1] as any;
            expect(loggedResult.access_token).toBe('[REDACTED]');
            expect(loggedResult.refresh_token).toBe('[REDACTED]');
            expect(loggedResult.expires_in).toBe(3600);
        });

        it('preserves non-sensitive payloads completely in logs', async () => {
            const safeArgs = { query: 'Mozart', limit: 20 };
            const safeResult = { tracks: [{ id: 1, title: 'Requiem' }] };

            vi.mocked(invoke).mockResolvedValueOnce(safeResult);

            const result = await invokeCommand('search_tracks', safeArgs);
            expect(result).toEqual(safeResult);

            expect(debugSpy).toHaveBeenCalledWith('[Tauri] Invoking: search_tracks', safeArgs);
            expect(debugSpy).toHaveBeenCalledWith('[Tauri] search_tracks success:', safeResult);
        });

        it('completely suppresses console.debug when import.meta.env.PROD === true', async () => {
            (import.meta.env as any).PROD = true;

            vi.mocked(invoke).mockResolvedValueOnce({ secret: 'should_not_log_anything' });

            const res = await invokeCommand('get_secret_in_prod', { token: 'secret_tok' });
            expect(res).toEqual({ secret: 'should_not_log_anything' });

            // In production, debug logs must NOT be emitted at all
            expect(debugSpy).not.toHaveBeenCalled();
        });
    });
});
