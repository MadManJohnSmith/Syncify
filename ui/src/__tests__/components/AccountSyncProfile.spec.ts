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
        { value: 'mirror_quarantine', enabled: false, reason: 'mirror_engine_not_available' },
        { value: 'strict_mirror', enabled: false, reason: 'mirror_engine_not_available' },
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

    it('solo ofrece import_only como modo ejecutable y nombra el motivo de los demás', async () => {
        mockInvoke((command) => (command === 'get_sync_profile' ? resolved : null));

        const wrapper = mount(AccountSyncProfile, { props: { accountId: 42 } });
        await flushPromises();
        await wrapper.get('[data-testid="sync-profile-open-42"]').trigger('click');
        await flushPromises();

        const mode = wrapper.get('[data-testid="sync-profile-mode-42"]');
        for (const value of ['mirror_quarantine', 'strict_mirror', 'bidirectional']) {
            const option = mode
                .findAll('option')
                .find((candidate) => candidate.attributes('value') === value);
            expect(option, `${value} must be modelled`).toBeDefined();
            expect(option!.attributes('disabled'), `${value} must not be selectable`).toBeDefined();
        }

        expect(
            wrapper.get('[data-testid="sync-profile-unavailable-42-bidirectional"]').text()
        ).toContain('write_back_not_available');
        for (const value of ['mirror_quarantine', 'strict_mirror']) {
            expect(
                wrapper.get(`[data-testid="sync-profile-unavailable-42-${value}"]`).text()
            ).toContain('mirror_engine_not_available');
        }
    });

    it('guarda el override (toggle, intervalo y colecciones) y muestra el perfil nuevo', async () => {
        let updateArgs: unknown = null;
        mockInvoke((command, args) => {
            if (command === 'get_sync_profile') return resolved;
            if (command === 'update_sync_profile') {
                updateArgs = args;
                return {
                    ...resolved,
                    enabled: false,
                    interval_hours: 6,
                    interval_source: 'account',
                    collections: { favorites: true, playlists: false, albums: true, artists: true },
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
                mode: 'import_only',
                interval_hours: 6,
                collections: { favorites: true, playlists: false, albums: true, artists: true },
            },
        });

        const effective = wrapper.get('[data-testid="sync-profile-effective-42"]');
        expect(effective.text()).toContain('Import only');
        expect(effective.text()).toContain('every 6 h');
        expect(effective.text()).toContain('paused');
        expect(effective.text()).not.toContain('Playlists');
    });

    it('rechaza un intervalo fuera de rango sin cerrar el formulario', async () => {
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

        // El error se pinta DENTRO del formulario: los controles sobreviven y
        // la ficha sigue siendo cerrable (no se queda atascada sin salida).
        const panel = wrapper.get('[data-testid="sync-profile-panel-42"]');
        expect(panel.get('[data-testid="sync-profile-error-42"]').text()).toContain(
            'between 1 and 720'
        );
        expect(panel.find('[data-testid="sync-profile-interval-42"]').exists()).toBe(true);
        expect(panel.find('[data-testid="sync-profile-save-42"]').exists()).toBe(true);
        expect(panel.find('[data-testid="sync-profile-close-42"]').exists()).toBe(true);

        await wrapper.get('[data-testid="sync-profile-close-42"]').trigger('click');
        await flushPromises();
        expect(wrapper.find('[data-testid="sync-profile-panel-42"]').exists()).toBe(false);
        expect(wrapper.find('[data-testid="sync-profile-open-42"]').exists()).toBe(true);
    });
});
