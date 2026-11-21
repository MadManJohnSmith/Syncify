/**
 * syncProfile.spec.ts — issue #69 (ADR-0006 decisiones 3-5, 7 y 30).
 *
 * El wrapper es la única puerta IPC del perfil de sincronización: fija los
 * comandos, sus argumentos y que la vista consume el perfil YA RESUELTO.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { syncProfileApi, type SyncProfile } from '@/api/syncProfile';
import { mockInvoke, resetMocks } from '../setup';

const resolvedProfile: SyncProfile = {
    account_id: 42,
    service_name: 'qobuz',
    enabled: true,
    mode: 'import_only',
    mode_source: 'service',
    interval_hours: 24,
    interval_source: 'service',
    collections: { favorites: true, playlists: true, albums: true, artists: true },
    collections_source: 'service',
    modes: [
        { value: 'import_only', enabled: true, reason: null },
        { value: 'mirror_quarantine', enabled: false, reason: 'mirror_engine_not_available' },
        { value: 'strict_mirror', enabled: false, reason: 'mirror_engine_not_available' },
        { value: 'bidirectional', enabled: false, reason: 'write_back_not_available' },
    ],
};

describe('sync_profile_ipc_contract_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('get_sync_profile pasa accountId y devuelve el perfil resuelto', async () => {
        const calls: Array<{ command: string; args: unknown }> = [];
        mockInvoke((command, args) => {
            calls.push({ command, args });
            return command === 'get_sync_profile' ? resolvedProfile : null;
        });

        const profile = await syncProfileApi.getSyncProfile(42);

        expect(profile).toEqual(resolvedProfile);
        expect(calls).toContainEqual({
            command: 'get_sync_profile',
            args: { accountId: 42 },
        });
    });

    it('update_sync_profile envía el override completo y devuelve el perfil nuevo', async () => {
        const updated: SyncProfile = {
            ...resolvedProfile,
            enabled: false,
            interval_hours: 6,
            interval_source: 'account',
            collections: { favorites: true, playlists: false, albums: true, artists: false },
            collections_source: 'account',
        };
        const calls: Array<{ command: string; args: unknown }> = [];
        mockInvoke((command, args) => {
            calls.push({ command, args });
            return command === 'update_sync_profile' ? updated : null;
        });

        const payload = {
            enabled: false,
            mode: 'import_only',
            interval_hours: 6,
            collections: { favorites: true, playlists: false, albums: true, artists: false },
        };
        const profile = await syncProfileApi.updateSyncProfile(42, payload);

        expect(profile).toEqual(updated);
        expect(calls).toContainEqual({
            command: 'update_sync_profile',
            args: { accountId: 42, profile: payload },
        });
    });
});
