/**
 * Unit tests for SpotifyApiConfigCard.vue (Sprint S196)
 * Verifies the UI-configurable Spotify credentials card:
 * - status badge (Configurado / No configurado) from DB-backed kv settings
 * - secret never rendered in full: only the backend `****last4` mask
 * - read-only redirect URI with copy button, preloaded with the app's URI
 * - Spanish onboarding instructions present in the component itself
 * - save flow sends masked-untouched secret semantics (null = keep)
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import SpotifyApiConfigCard from '../../components/SpotifyApiConfigCard.vue';
import { mockInvoke, resetMocks } from '../setup';

function kvHandler(overrides: Record<string, string> = {}) {
    return (cmd: string, args?: Record<string, unknown>) => {
        if (cmd === 'get_kv_settings') {
            return {
                spotify_client_id: '',
                spotify_client_secret: '',
                spotify_redirect_uri: '',
                ...overrides,
            };
        }
        if (cmd === 'save_settings_batch') {
            return null;
        }
        throw new Error(`Unexpected command in test: ${cmd}`);
    };
}

describe('SpotifyApiConfigCard', () => {
    beforeEach(() => {
        resetMocks();
        // navigator.clipboard may not exist in jsdom
        Object.assign(navigator, {
            clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
        });
    });

    it('shows "No configurado" and auto-expands instructions when nothing is stored', async () => {
        mockInvoke(kvHandler());
        const wrapper = mount(SpotifyApiConfigCard);
        await flushPromises();

        const status = wrapper.find('[data-testid="spotify-api-status"]');
        expect(status.text()).toContain('No configurado');
        // Onboarding panel is open for unconfigured users…
        expect(wrapper.find('[data-testid="spotify-instructions"]').exists()).toBe(true);
        // …and contains the 5 guided steps in Spanish.
        expect(wrapper.find('[data-testid="spotify-instructions"]').text())
            .toContain('developer.spotify.com/dashboard');
        expect(wrapper.text()).toContain('Create app');
        expect(wrapper.text()).toContain('Redirect URI');
        expect(wrapper.text()).toContain('Web API');
        expect(wrapper.text()).toContain('Client ID');
        expect(wrapper.text()).toContain('Client Secret');
    });

    it('shows "Configurado" with masked secret and collapsed instructions when configured', async () => {
        mockInvoke(kvHandler({
            spotify_client_id: 'abc123myclientid',
            spotify_client_secret: '****x9z4',
        }));
        const wrapper = mount(SpotifyApiConfigCard);
        await flushPromises();

        expect(wrapper.find('[data-testid="spotify-api-status"]').text()).toContain('Configurado');
        expect(wrapper.text()).toContain('****x9z4');
        expect(wrapper.text()).not.toContain('abc123myclientid'.slice(0, 8));
        // Instructions stay folded once configured.
        expect(wrapper.find('[data-testid="spotify-instructions"]').exists()).toBe(false);
    });

    it('preloads a read-only redirect URI with a working copy button', async () => {
        mockInvoke(kvHandler());
        const wrapper = mount(SpotifyApiConfigCard);
        await flushPromises();

        const input = wrapper.find('[data-testid="spotify-redirect-uri"]');
        expect(input.attributes('readonly')).toBeDefined();
        expect((input.element as HTMLInputElement).value).toBe('http://127.0.0.1:8888/callback');

        await wrapper.findAll('button').find(b => b.text().includes('Copiar'))!.trigger('click');
        expect(navigator.clipboard.writeText).toHaveBeenCalledWith('http://127.0.0.1:8888/callback');
    });

    it('saves client id + secret through save_settings_batch', async () => {
        const saveSpy = vi.fn((_cmd: string, _args?: Record<string, unknown>) => null);
        mockInvoke((cmd, args) => {
            if (cmd === 'get_kv_settings') return { spotify_client_id: '', spotify_client_secret: '', spotify_redirect_uri: '' };
            if (cmd === 'save_settings_batch') { saveSpy(cmd, args); return null; }
            throw new Error(`Unexpected command: ${cmd}`);
        });
        const wrapper = mount(SpotifyApiConfigCard);
        await flushPromises();

        await wrapper.find('#spotify-client-id').setValue('  my_id  ');
        await wrapper.find('#spotify-client-secret').setValue('my_secret');
        await wrapper.find('[data-testid="spotify-api-save"]').trigger('click');
        await flushPromises();

        expect(saveSpy).toHaveBeenCalledTimes(1);
        const payload = saveSpy.mock.calls[0][1] as { settings: Record<string, string> };
        expect(payload.settings['spotify_client_id']).toBe('my_id');
        expect(payload.settings['spotify_client_secret']).toBe('my_secret');
        expect(wrapper.emitted('saved')).toEqual([[true]]);
    });

    it('keeps the stored secret when the field is left empty (configured round-trip)', async () => {
        const saveSpy = vi.fn((_cmd: string, _args?: Record<string, unknown>) => null);
        mockInvoke((cmd, args) => {
            if (cmd === 'get_kv_settings') {
                return { spotify_client_id: 'stored_id', spotify_client_secret: '****ret1', spotify_redirect_uri: '' };
            }
            if (cmd === 'save_settings_batch') { saveSpy(cmd, args); return null; }
            throw new Error(`Unexpected command: ${cmd}`);
        });
        const wrapper = mount(SpotifyApiConfigCard);
        await flushPromises();

        // Only touch the client id; leave the secret blank.
        await wrapper.find('#spotify-client-id').setValue('new_id');
        await wrapper.find('[data-testid="spotify-api-save"]').trigger('click');
        await flushPromises();

        const payload = saveSpy.mock.calls[0][1] as { settings: Record<string, string> };
        expect(payload.settings['spotify_client_secret']).toBeUndefined();
        expect(payload.settings['spotify_client_id']).toBe('new_id');
    });
});
