/**
 * Unit tests for SettingsView.vue
 * Tests component rendering and navigation, matching the current sidebar-based structure
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import SettingsView from '../../views/SettingsView.vue';
import { mockInvoke, resetMocks } from '../setup';

describe('SettingsView', () => {
    beforeEach(() => {
        resetMocks();
        // Mock all the settings commands
        mockInvoke((command) => {
            // Services and accounts for dynamic UI
            if (command === 'get_services') return [
                { id: 1, name: 'spotify', display_name: 'Spotify' },
                { id: 2, name: 'apple_music', display_name: 'Apple Music' },
                { id: 3, name: 'tidal', display_name: 'Tidal' },
                { id: 4, name: 'qobuz', display_name: 'Qobuz' },
                { id: 5, name: 'deezer', display_name: 'Deezer' },
                { id: 6, name: 'soundcloud', display_name: 'SoundCloud' },
            ];
            if (command === 'get_accounts') return [];
            // Sync settings composable
            if (command === 'get_service_preferences') return [];
            if (command === 'get_service_sync_settings') return [];
            if (command === 'get_sync_settings') return null;
            // Download settings composable
            if (command === 'get_quality_preferences') return [];
            if (command === 'get_folder_settings') return null;
            if (command === 'get_duplicate_settings') return null;
            if (command === 'get_audio_processing_settings') return null;
            // Lyrics settings composable
            if (command === 'get_lyrics_providers') return [];
            if (command === 'get_lyrics_config') return null;
            // Advanced settings composable
            if (command === 'get_advanced_settings') return null;
            // Health check
            if (command === 'health_check') return {
                database_ok: true,
                python_ok: true,
                ffmpeg_available: true,
                chromaprint_available: false,
                services_configured: ['spotify'],
                errors: []
            };
            return null;
        });
    });

    it('renders the Settings header', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();
        expect(wrapper.text()).toContain('Settings');
    });

    it('renders sidebar with settings categories', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();

        // Check for some expected category buttons
        expect(wrapper.text()).toContain('General');
        expect(wrapper.text()).toContain('Services');
        expect(wrapper.text()).toContain('Advanced');
    });

    it('renders Save Changes button in sidebar', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();

        const saveButton = wrapper.findAll('button').find(b => b.text().includes('Save Changes'));
        expect(saveButton).toBeDefined();
    });

    it('renders Reset to Defaults button in sidebar', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();

        const resetButton = wrapper.findAll('button').find(b => b.text().includes('Reset to Defaults'));
        expect(resetButton).toBeDefined();
    });

    it('starts with General category active by default', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();

        // General category content should be visible
        expect(wrapper.text()).toContain('Application behavior');
    });

    it('navigates to Services category when clicked', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();

        // Find and click Services button
        const buttons = wrapper.findAll('button');
        const servicesButton = buttons.find(b => b.text().includes('Services'));
        expect(servicesButton).toBeDefined();

        await servicesButton!.trigger('click');
        await flushPromises();

        // Should now show Services content
        expect(wrapper.text()).toContain('Services & Priorities');
        expect(wrapper.text()).toContain('Manage connections');
    });

    it('navigates to Advanced category when clicked', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();

        // Find and click Advanced button
        const buttons = wrapper.findAll('button');
        const advancedButton = buttons.find(b => b.text().includes('Advanced'));
        expect(advancedButton).toBeDefined();

        await advancedButton!.trigger('click');
        await flushPromises();

        // Should now show Advanced content
        expect(wrapper.text()).toContain('Database, networking, and debug');
        expect(wrapper.text()).toContain('Advanced');
    });

    it('renders service cards in Services category', async () => {
        const wrapper = mount(SettingsView);
        await flushPromises();

        // Navigate to Services
        const buttons = wrapper.findAll('button');
        const servicesButton = buttons.find(b => b.text().includes('Services'));
        await servicesButton!.trigger('click');
        await flushPromises();

        // Should show services section heading
        expect(wrapper.text()).toContain('Services & Priorities');
    });
});
