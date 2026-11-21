/**
 * AccountSyncProfile.spec.ts — issue #69.
 *
 * La ficha de la cuenta muestra el perfil RESUELTO (una sola lectura del
 * backend) y permite editarlo: toggle de participación, modo, intervalo y
 * colecciones. El bidireccional aparece deshabilitado con su motivo.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import AccountSyncProfile from '@/components/AccountSyncProfile.vue';
import { mockInvoke, resetMocks } from '../setup';
import type { SyncProfile } from '@/api/syncProfile';

const resolved: SyncProfile = {
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
        { value: 'mirror_quarantine', enabled: true, reason: null },
        { value: 'strict_mirror', enabled: true, reason: null },
        { value: 'bidirectional', enabled: false, reason: 'write_back_not_available' },
    ],
};

describe('account_sync_profile', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('muestra el perfil resuelto sin mezclar fuentes', async () => {
        mockInvoke((command) => (command === 'get_sync_profile' ? resolved : null));

        const wrapper = mount(AccountSyncProfile, { props: { accountId: 42 } });
        await flushPromises();

        const summary = wrapper.get('[data-testid="sync-profile-summary-42"]');
        expect(summary.text()).toContain('Import only');
        expect(summary.text()).toContain('every 24 h');
        expect(summary.text()).toContain('participates');
        expect(summary.text()).toContain('Favorites');
    });

    it('muestra el bidireccional deshabilitado con su motivo', async () => {
        mockInvoke((command) => (command === 'get_sync_profile' ? resolved : null));

        const wrapper = mount(AccountSyncProfile, { props: { accountId: 42 } });
        await flushPromises();
        await wrapper.get('[data-testid="sync-profile-open-42"]').trigger('click');
        await flushPromises();

        const mode = wrapper.get('[data-testid="sync-profile-mode-42"]');
        const bidirectional = mode
            .findAll('option')
            .find((option) => option.attributes('value') === 'bidirectional');
        expect(bidirectional).toBeDefined();
        expect(bidirectional!.attributes('disabled')).toBeDefined();

        const reason = wrapper.get('[data-testid="sync-profile-bidirectional-reason-42"]');
        expect(reason.text()).toContain('write_back_not_available');
    });

    it('guarda el override (toggle, modo, intervalo y colecciones) y muestra el perfil nuevo', async () => {
        let updateArgs: unknown = null;
        mockInvoke((command, args) => {
            if (command === 'get_sync_profile') return resolved;
            if (command === 'update_sync_profile') {
                updateArgs = args;
                return {
                    ...resolved,
                    enabled: false,
                    mode: 'mirror_quarantine',
                    mode_source: 'account',
                    interval_hours: 6,
                    interval_source: 'account',
                    collections: { favorites: true, playlists: false, albums: true, artists: false },
                    collections_source: 'account',
                };
            }
            return null;
        });

        const wrapper = mount(AccountSyncProfile, { props: { accountId: 42 } });
        await flushPromises();
        await wrapper.get('[data-testid="sync-profile-open-42"]').trigger('click');
        await flushPromises();

        await wrapper.get('[data-testid="sync-profile-enabled-42"]').setValue(false);
        await wrapper.get('[data-testid="sync-profile-mode-42"]').setValue('mirror_quarantine');
        await wrapper.get('[data-testid="sync-profile-interval-42"]').setValue(6);
        await wrapper
            .get('[data-testid="sync-profile-collection-42-playlists"]')
            .setValue(false);
        await wrapper.get('[data-testid="sync-profile-save-42"]').trigger('click');
        await flushPromises();

        expect(updateArgs).toEqual({
            accountId: 42,
            profile: {
                enabled: false,
                mode: 'mirror_quarantine',
                interval_hours: 6,
                collections: { favorites: true, playlists: false, albums: true, artists: true },
            },
        });

        const effective = wrapper.get('[data-testid="sync-profile-effective-42"]');
        expect(effective.text()).toContain('Mirror with quarantine');
        expect(effective.text()).toContain('every 6 h');
        expect(effective.text()).toContain('paused');
        expect(effective.text()).not.toContain('Playlists');
    });

    it('rechaza un intervalo fuera de rango antes de llamar al backend', async () => {
        const calls: string[] = [];
        mockInvoke((command) => {
            calls.push(command);
            return command === 'get_sync_profile' ? resolved : null;
        });

        const wrapper = mount(AccountSyncProfile, { props: { accountId: 42 } });
        await flushPromises();
        await wrapper.get('[data-testid="sync-profile-open-42"]').trigger('click');
        await flushPromises();

        await wrapper.get('[data-testid="sync-profile-interval-42"]').setValue(0);
        await wrapper.get('[data-testid="sync-profile-save-42"]').trigger('click');
        await flushPromises();

        expect(calls).not.toContain('update_sync_profile');
        expect(wrapper.get('[data-testid="sync-profile-error-42"]').text()).toContain(
            'between 1 and 720'
        );
    });
});
