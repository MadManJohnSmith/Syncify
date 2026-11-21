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
    resetAccountSyncData,
    refreshAccountIdentities,
    setAccountDisplayName,
} from '@/api/accounts';
import { maskEmail } from '@/api/normalize';
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

    it('sends the account id to reset_account_sync_data', async () => {
        const calls: Array<[string, Record<string, unknown> | undefined]> = [];
        mockInvoke((cmd, args) => {
            calls.push([cmd, args]);
            return { account_id: 42, service: 'spotify', library_entries: 1, playlists: 0, favorites: 2 };
        });

        const summary = await resetAccountSyncData(42);

        expect(summary.account_id).toBe(42);
        expect(summary.service).toBe('spotify');
        expect(calls).toContainEqual(['reset_account_sync_data', { accountId: 42 }]);
    });

    it('sends the identity commands with their exact payloads', async () => {
        const calls: Array<[string, Record<string, unknown> | undefined]> = [];
        mockInvoke((cmd, args) => {
            calls.push([cmd, args]);
            if (cmd === 'refresh_account_identities') return { checked: 3, updated: 1, unresolved: 2 };
            return null;
        });

        const summary = await refreshAccountIdentities();
        expect(summary).toEqual({ checked: 3, updated: 1, unresolved: 2 });
        expect(calls).toContainEqual(['refresh_account_identities', undefined]);

        await setAccountDisplayName(7, 'Casa');
        expect(calls).toContainEqual(['set_account_display_name', { accountId: 7, name: 'Casa' }]);
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

describe('accounts_mask_emails_test (ADR-0007 decision 7)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('masks an address to two local chars plus the domain, or hides it all', () => {
        expect(maskEmail('ana@spotify.com')).toBe('an•••@spotify.com');
        expect(maskEmail('jo@example.com')).toBe('j•••@example.com');
        expect(maskEmail('a@example.com')).toBe('•••@example.com');
        expect(maskEmail('  ana@spotify.com  ')).toBe('an•••@spotify.com');
        // Not shaped like an address: hide it all instead of guessing.
        expect(maskEmail('not-an-email')).toBe('•••');
        expect(maskEmail('a@b@c.com')).toBe('•••');
        expect(maskEmail('@x.com')).toBe('•••');
        expect(maskEmail('x@')).toBe('•••');
        // Absence stays absent.
        expect(maskEmail(null)).toBeNull();
        expect(maskEmail(undefined)).toBeNull();
        expect(maskEmail('')).toBeNull();
        expect(maskEmail('   ')).toBeNull();
    });

    it('masks the email of every account at the IPC boundary', async () => {
        mockInvoke(() => [
            { id: 1, service_name: 'spotify', display_name: 'John', email: 'john.doe@example.com' },
            { id: 2, service_name: 'qobuz', display_name: null, email: 'ana@qobuz.com' },
        ]);

        const accounts = await getAccounts();

        expect(accounts[0].email).toBe('jo•••@example.com');
        expect(accounts[1].email).toBe('an•••@qobuz.com');
        // The full addresses never leave this module's payload.
        const serialized = JSON.stringify(accounts);
        expect(serialized).not.toContain('john.doe@example.com');
        expect(serialized).not.toContain('ana@qobuz.com');
    });

    it('keeps absent emails absent and collapses non-address values to the bullet mask', async () => {
        mockInvoke(() => [
            { id: 1, email: null },
            { id: 2, email: '' },
            { id: 3, email: 'not-an-email' },
            { id: 4, email: '@x.com' },
        ]);

        const accounts = await getAccounts();

        expect(accounts[0].email).toBeNull();
        expect(accounts[1].email).toBeNull();
        expect(accounts[2].email).toBe('•••');
        expect(accounts[3].email).toBe('•••');
    });

    it('masks the status account_email while preserving array identity', async () => {
        const statuses = [
            { name: 'Spotify', connected: true, account_email: 'john.doe@example.com' },
            { name: 'Qobuz', connected: false, account_email: null },
        ];
        mockInvoke(() => statuses);

        const res = await getServiceStatuses();

        expect(res).toBe(statuses);
        expect(statuses[0].account_email).toBe('jo•••@example.com');
        expect(statuses[1].account_email).toBeNull();
    });
});
