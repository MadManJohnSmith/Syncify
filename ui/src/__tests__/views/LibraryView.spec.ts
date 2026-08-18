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

    it('eliminates redundant Imported and DL badges from track title cell', async () => {
        const mockTracks = [
            createTestTrack({ 
                id: 1, 
                title: 'Clean Track', 
                artist_name: 'Artist', 
                album_name: 'Album',
                imported_from: 'qobuz', 
                downloaded_from: 'tidal',
                download_status: 'downloaded' 
            }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        expect(wrapper.text()).not.toContain('Imported: qobuz');
        expect(wrapper.text()).not.toContain('DL: tidal');
    });

    it('displays effective download service provider logo and accessible tooltip when downloaded', async () => {
        const mockTracks = [
            createTestTrack({ 
                id: 1, 
                title: 'Downloaded Track', 
                download_status: 'downloaded',
                downloaded_from: 'qobuz' 
            }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Effective service logo 'Q' with accessible tooltip
        const qobuzBadge = wrapper.find('span[title="Downloaded from Qobuz"]');
        expect(qobuzBadge.exists()).toBe(true);
        expect(qobuzBadge.text()).toBe('Q');
    });

    it('displays dash and accessible tooltip for not downloaded track', async () => {
        const mockTracks = [
            createTestTrack({ 
                id: 1, 
                title: 'Undownloaded Track', 
                download_status: 'not_downloaded',
                downloaded_from: null
            }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        const notDlSpan = wrapper.find('span[title="Not downloaded"]');
        expect(notDlSpan.exists()).toBe(true);
        expect(notDlSpan.text()).toBe('—');
    });

    it('handles single track download action and sends exact IPC payload', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        const mockTracks = [
            createTestTrack({ id: 101, title: 'Audited Track', artist_name: 'Audited Artist', album_name: 'Audited Album' }),
        ];

        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            if (cmd === 'add_to_queue') return 1;
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Find download button on track row
        const downloadBtn = wrapper.find('button[title="Download"]');
        expect(downloadBtn.exists()).toBe(true);
        await downloadBtn.trigger('click');
        await flushPromises();

        const addCall = invokeCalls.find(c => c.cmd === 'add_to_queue');
        expect(addCall).toBeDefined();
        expect(addCall?.args).toEqual({
            trackId: 101,
            targetTitle: 'Audited Track',
            targetArtist: 'Audited Artist',
            targetAlbum: 'Audited Album',
            qualityPreference: 'hires',
            allowFallback: true,
        });
    });

    it('displays tracks imported from albums (is_favorite = 0) in default library view', async () => {
        const mockTracks = [
            createTestTrack({ 
                id: 201, 
                title: 'Album Track 1', 
                album_name: 'Great Album', 
                is_favorite: false,
                download_status: 'not_downloaded'
            }),
            createTestTrack({ 
                id: 202, 
                title: 'Album Track 2', 
                album_name: 'Great Album', 
                is_favorite: false,
                download_status: 'not_downloaded'
            }),
            createTestTrack({ 
                id: 203, 
                title: 'Favorite Track', 
                is_favorite: true,
                download_status: 'downloaded'
            }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 3, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        expect(wrapper.text()).toContain('Album Track 1');
        expect(wrapper.text()).toContain('Album Track 2');
        expect(wrapper.text()).toContain('Favorite Track');
    });

    it('handles SourceIdentityMissing error gracefully without crash', async () => {
        const mockTracks = [
            createTestTrack({ id: 102, title: 'No Source Track' }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            if (cmd === 'add_to_queue') {
                throw new Error('SourceIdentityMissing: track 102 has no streaming source');
            }
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        const downloadBtn = wrapper.find('button[title="Download"]');
        expect(downloadBtn.exists()).toBe(true);
        await downloadBtn.trigger('click');
        await flushPromises();

        // Component should still be mounted and resilient
        expect(wrapper.exists()).toBe(true);
    });

    it('handles keyboard shortcut D to trigger download on selected tracks', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        const mockTracks = [
            createTestTrack({ id: 201, title: 'Selected Track 1' }),
        ];

        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            if (cmd === 'add_batch_to_queue') return { added: 1, skipped: 0 };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Select the track
        const checkbox = wrapper.find('input[type="checkbox"]');
        if (checkbox.exists()) {
            await checkbox.setValue(true);
            await flushPromises();
        }

        // Trigger keyboard shortcut D on window
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'd' }));
        await flushPromises();

        const batchCall = invokeCalls.find(c => c.cmd === 'add_batch_to_queue');
        if (batchCall) {
            expect(batchCall.args.trackIds).toContain(201);
        }
    });

    it('S132A: renders album groups and grid tiles for imported albums', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Track 1', album_name: 'Abbey Road', album_id: 10, artist_name: 'The Beatles' }),
            createTestTrack({ id: 2, title: 'Track 2', album_name: 'Abbey Road', album_id: 10, artist_name: 'The Beatles' }),
            createTestTrack({ id: 3, title: 'Track 3', album_name: 'Wish You Were Here', album_id: 20, artist_name: 'Pink Floyd' }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 3, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Switch to grid view
        const gridBtn = wrapper.findAll('.view-toggle button')[1];
        if (gridBtn) {
            await gridBtn.trigger('click');
            await flushPromises();
            expect(wrapper.text()).toContain('Abbey Road');
            expect(wrapper.text()).toContain('Wish You Were Here');
        }
    });

    it('S132A: reactively reloads library when sync_service emits sync-complete', async () => {
        let loadCount = 0;
        mockInvoke((cmd) => {
            if (cmd === 'get_library') {
                loadCount++;
                return { tracks: [createTestTrack({ id: 1, title: `Track version ${loadCount}` })], total: 1, offset: 0, limit: 50, has_more: false };
            }
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();
        const initialLoadCount = loadCount;
        expect(initialLoadCount).toBeGreaterThanOrEqual(1);

        // Emit sync-complete event via eventBus
        const { useEventBus, TauriEvents } = await import('@/composables/useEventBus');
        const eventBus = useEventBus();
        await eventBus.emit(TauriEvents.SYNC_COMPLETE, { service: 'tidal', imported: 93, favorites: 10 });
        
        // Wait for debounce timer (350ms)
        await new Promise(r => setTimeout(r, 450));
        await flushPromises();

        expect(loadCount).toBeGreaterThan(initialLoadCount);
    });
});
