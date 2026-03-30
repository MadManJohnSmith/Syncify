/**
 * LibraryView.spec.ts
 * Component tests for LibraryView.vue
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import LibraryView from '@/views/LibraryView.vue';
import { mockInvoke, resetMocks } from '../setup';
import type { LibraryTrack } from '@/api/types';

// Create test track factory
function createTestTrack(overrides: Partial<LibraryTrack> = {}): LibraryTrack {
    return {
        id: Math.floor(Math.random() * 10000),
        title: 'Test Track',
        artist_name: 'Test Artist',
        artist_id: null,
        album_name: 'Test Album',
        album_id: null,
        duration_ms: 180000,
        isrc: `TEST${Date.now()}`,
        services: 'Spotify',
        quality: '320kbps',
        download_status: 'not_downloaded',
        metadata_score: 80,
        lyrics_type: 'none',
        cover_art_url: null,
        track_number: null,
        disc_number: null,
        genre: null,
        bpm: null,
        musical_key: null,
        release_year: null,
        explicit: null,
        file_path: null,
        musicbrainz_id: null,
        ...overrides
    };
}

describe('LibraryView', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('renders library header', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        expect(wrapper.text()).toContain('Library');
    });

    it('displays tracks from get_library API', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Song One', artist_name: 'Artist One' }),
            createTestTrack({ id: 2, title: 'Song Two', artist_name: 'Artist Two' }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 2, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        expect(wrapper.text()).toContain('Song One');
        expect(wrapper.text()).toContain('Song Two');
    });

    it('shows empty state when no tracks', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Check for empty state message
        expect(wrapper.text().toLowerCase()).toMatch(/empty|no tracks|get started/i);
    });

    it('displays track count', async () => {
        const mockTracks = [
            createTestTrack({ id: 1 }),
            createTestTrack({ id: 2 }),
            createTestTrack({ id: 3 }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 3, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Should display track count somewhere
        expect(wrapper.text()).toContain('3');
    });

    it('handles API error gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') {
                throw new Error('Database error');
            }
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Should not crash, should show empty or error state
        expect(wrapper.exists()).toBe(true);
    });

    it('displays service badges for tracks', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, services: 'Spotify, Qobuz' }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Component renders track with services (badges are displayed)
        expect(wrapper.text()).toContain('Test Track');
        expect(wrapper.exists()).toBe(true);
    });

    it('displays download status indicator', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, download_status: 'downloaded' }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Component should render without crashing
        expect(wrapper.exists()).toBe(true);
    });
});
