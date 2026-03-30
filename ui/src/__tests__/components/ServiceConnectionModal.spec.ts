/**
 * Unit tests for ServiceConnectionModal.vue
 * Tests component rendering, service connection flow, and user interactions
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import ServiceConnectionModal from '../../components/ServiceConnectionModal.vue';
import { mockInvoke, resetMocks } from '../setup';

describe('ServiceConnectionModal', () => {
    beforeEach(() => {
        resetMocks();
    });

    it('renders when modelValue is true', () => {
        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });
        expect(wrapper.find('.fixed').exists()).toBe(true);
        expect(wrapper.text()).toContain('Connect New Service');
    });

    it('does not render when modelValue is false', () => {
        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: false }
        });
        expect(wrapper.find('.fixed').exists()).toBe(false);
    });

    it('renders all 6 service buttons', () => {
        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });

        expect(wrapper.text()).toContain('Spotify');
        expect(wrapper.text()).toContain('Apple Music');
        expect(wrapper.text()).toContain('Tidal');
        expect(wrapper.text()).toContain('Qobuz');
        expect(wrapper.text()).toContain('Deezer');
        expect(wrapper.text()).toContain('SoundCloud');
    });

    it('emits update:modelValue when backdrop is clicked', async () => {
        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });

        await wrapper.find('.bg-black\\/60').trigger('click');
        expect(wrapper.emitted('update:modelValue')).toBeTruthy();
        expect(wrapper.emitted('update:modelValue')![0]).toEqual([false]);
    });

    it('emits update:modelValue when cancel button is clicked', async () => {
        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });

        const cancelButton = wrapper.findAll('button').find(b => b.text() === 'Cancel');
        await cancelButton!.trigger('click');
        expect(wrapper.emitted('update:modelValue')).toBeTruthy();
    });

    it('calls startAuthAndSave when service button is clicked', async () => {
        mockInvoke((command) => {
            if (command === 'start_auth_and_save') {
                return {
                    success: true,
                    data: { display_name: 'Test User', email: 'test@example.com' },
                    error: null
                };
            }
            return null;
        });

        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });

        // Find and click Spotify button
        const buttons = wrapper.findAll('button');
        const spotifyButton = buttons.find(b => b.text().includes('Spotify'));
        await spotifyButton!.trigger('click');
        await flushPromises();

        // Should emit connected event
        expect(wrapper.emitted('connected')).toBeTruthy();
        expect(wrapper.emitted('connected')![0]).toEqual(['spotify', 'Test User']);
    });

    it('shows loading state during connection', async () => {
        let resolveAuth: (value: unknown) => void;
        mockInvoke((command) => {
            if (command === 'start_auth_and_save') {
                return new Promise((resolve) => {
                    resolveAuth = resolve;
                });
            }
            return null;
        });

        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });

        // Click Qobuz button
        const buttons = wrapper.findAll('button');
        const qobuzButton = buttons.find(b => b.text().includes('Qobuz'));
        await qobuzButton!.trigger('click');

        // Should show loading spinner
        expect(wrapper.find('.animate-spin').exists()).toBe(true);

        // Resolve the promise
        resolveAuth!({
            success: true,
            data: { display_name: 'Qobuz User' },
            error: null
        });
        await flushPromises();

        // Loading should be gone
        expect(wrapper.find('.animate-spin').exists()).toBe(false);
    });

    it('shows error message on auth failure', async () => {
        mockInvoke((command) => {
            if (command === 'start_auth_and_save') {
                return {
                    success: false,
                    data: null,
                    error: 'Authentication failed'
                };
            }
            return null;
        });

        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });

        // Click Tidal button
        const buttons = wrapper.findAll('button');
        const tidalButton = buttons.find(b => b.text().includes('Tidal'));
        await tidalButton!.trigger('click');
        await flushPromises();

        // Should show error banner
        expect(wrapper.text()).toContain('Authentication failed');
    });

    it('disables close button during connection', async () => {
        let resolveAuth: (value: unknown) => void;
        mockInvoke((command) => {
            if (command === 'start_auth_and_save') {
                return new Promise((resolve) => {
                    resolveAuth = resolve;
                });
            }
            return null;
        });

        const wrapper = mount(ServiceConnectionModal, {
            props: { modelValue: true }
        });

        // Click a service button
        const buttons = wrapper.findAll('button');
        const spotifyButton = buttons.find(b => b.text().includes('Spotify'));
        await spotifyButton!.trigger('click');

        // Close button should be disabled
        const closeButton = wrapper.find('[aria-label="Close modal"]');
        expect(closeButton.attributes('disabled')).toBeDefined();

        // Cleanup
        resolveAuth!({ success: true, data: {}, error: null });
        await flushPromises();
    });
});
