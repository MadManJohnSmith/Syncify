/**
 * accounts.spec.ts
 * Regression tests: account/service lists and import results default safely.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getServices,
    getServiceStatuses,
    getAccounts,
    validateAllSessions,
    importSpotifyLibrary,
    setActiveAccount,
    logoutService,
    getServiceAuthStatus,
    syncService,
} from '@/api/accounts';
import { mockInvoke, resetMocks } from '../setup';

describe('accounts_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('coerces non-array list responses to empty arrays', async () => {
        mockInvoke(() => null);

        expect(await getServices()).toEqual([]);
        expect(await getServiceStatuses()).toEqual([]);
        expect(await getAccounts()).toEqual([]);
        expect(await validateAllSessions()).toEqual([]);
    });

    it('preserves array identity for status payloads (views mutate fetched objects)', async () => {
        const statuses = [{ name: 'Spotify', connected: true, credentials_invalid: false, library_count: 5 }];
        mockInvoke((cmd) => (cmd === 'get_service_statuses' ? statuses : null));

        const res = await getServiceStatuses();
        expect(res).toBe(statuses);
    });

    it('scopes logout, activation and sync to the specified account', async () => {
        const calls: Array<[string, Record<string, unknown> | undefined]> = [];
        mockInvoke((cmd, args) => { calls.push([cmd, args]); return null; });
        await setActiveAccount(42);
        await logoutService('spotify', 42);
        await logoutService('spotify');
        await syncService('spotify', 42);
        expect(calls).toContainEqual(['set_active_account', { accountId: 42 }]);
        expect(calls).toContainEqual(['logout_service', { service: 'spotify', accountId: 42 }]);
        expect(calls).toContainEqual(['logout_service', { service: 'spotify', accountId: null }]);
        expect(calls).toContainEqual(['sync_service', { service: 'spotify', accountId: 42, preferences: null }]);
    });

    it('retains is_active in normalized service auth status', async () => {
        mockInvoke(() => ({ service: 'tidal', status: 'connected_valid', account_id: 42, is_active: true, is_authenticated: true }));
        expect((await getServiceAuthStatus('tidal', 42)).is_active).toBe(true);
        mockInvoke(() => ({ service: 'tidal', status: 'requires_auth', is_active: false }));
        expect((await getServiceAuthStatus('tidal')).is_active).toBe(false);
    });

    it('normalizes ImportResult counters and errors array', async () => {
        mockInvoke((cmd) => (cmd === 'import_spotify_library' ? { imported: 12, skipped: 3, errors: ['bad row'] } : null));

        const res = await importSpotifyLibrary();
        expect(res.imported).toBe(12);
        expect(res.skipped).toBe(3);
        expect(res.errors).toEqual(['bad row']);

        mockInvoke((cmd) => (cmd === 'import_spotify_library' ? { imported: 1 } : null));
        const partial = await importSpotifyLibrary();
        expect(partial.imported).toBe(1);
        expect(partial.skipped).toBe(0);
        expect(partial.errors).toEqual([]);

        mockInvoke(() => null);
        const zeroed = await importSpotifyLibrary();
        expect(zeroed.imported).toBe(0);
        expect(zeroed.skipped).toBe(0);
        expect(zeroed.errors).toEqual([]);
    });
});
