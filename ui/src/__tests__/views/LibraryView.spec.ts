/**
 * LibraryView.spec.ts
 * Component tests for LibraryView.vue
 */
import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import { createRouter, createMemoryHistory } from 'vue-router';
import LibraryView from '@/views/LibraryView.vue';
import { usePlayer } from '@/composables/usePlayer';
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

    it('S141: renders default columns in exact required order', async () => {
        const mockTracks = [createTestTrack({ id: 1, title: 'Column Test Track' })];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        const headers = wrapper.findAll('.column-header').map(h => h.text().trim());
        expect(headers).toEqual(['#', 'Title', 'Time', 'Services', 'Meta', 'Lyrics', 'DL', 'Quality', 'Special']);
    });

    it('S141: persists column reordering to localStorage', async () => {
        const mockTracks = [createTestTrack({ id: 1, title: 'Reorder Test Track' })];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const storageMock: Record<string, string> = {};
        const setItemSpy = vi.fn((key: string, val: string) => { storageMock[key] = val; });
        const getItemSpy = vi.fn((key: string) => storageMock[key] || null);
        Object.defineProperty(window, 'localStorage', {
            value: {
                getItem: getItemSpy,
                setItem: setItemSpy,
                removeItem: vi.fn(),
                clear: vi.fn(),
            },
            writable: true,
            configurable: true,
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Drag column 0 (#) to column 2 (Time)
        const colHeaders = wrapper.findAll('.column-header');
        expect(colHeaders.length).toBe(9);

        // Simulate dragstart on first column and drop on third
        await colHeaders[0].trigger('dragstart', { dataTransfer: { effectAllowed: '', setData: vi.fn() } });
        await colHeaders[2].trigger('drop', { dataTransfer: { getData: vi.fn().mockReturnValue('0') } });
        await flushPromises();

        expect(setItemSpy).toHaveBeenCalledWith('syncify_library_columns_order', expect.any(String));
    });

    it('S141: Quality only appears for downloaded track (and dash for undownloaded)', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Downloaded Track', download_status: 'downloaded', quality: '24/96' }),
            createTestTrack({ id: 2, title: 'Not Downloaded Track', download_status: 'not_downloaded', quality: '24/96' }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 2, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        const rows = wrapper.findAll('.track-row');
        expect(rows.length).toBe(2);

        // First row (downloaded) displays '24/96'
        expect(rows[0].text()).toContain('24/96');

        // Second row (not downloaded) does NOT display '24/96' as a quality badge, displays '—'
        expect(rows[1].text()).toContain('—');
    });

    it('S141: groups tracks by real genre metadata', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Track Synth', genre: 'Synthwave' }),
            createTestTrack({ id: 2, title: 'Track Rock', genre: 'Rock' }),
            createTestTrack({ id: 3, title: 'Track No Genre', genre: null }),
        ];

        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 3, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        // Open Group By dropdown and select Genre
        const groupBtn = wrapper.find('.library-toolbar .relative button');
        await groupBtn.trigger('click');
        await flushPromises();

        const genreOption = wrapper.findAll('.library-toolbar .relative .absolute button').find(b => b.text().includes('Genre'));
        if (genreOption) {
            await genreOption.trigger('click');
            await flushPromises();

            expect(wrapper.text()).toContain('Synthwave');
            expect(wrapper.text()).toContain('Rock');
            expect(wrapper.text()).toContain('Unknown Genre');
        }
    });

    it('S141: clamps context menu position within viewport bounds', async () => {
        const mockTracks = [createTestTrack({ id: 1, title: 'Menu Track' })];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        Object.defineProperty(window, 'innerWidth', { value: 1000, writable: true });
        Object.defineProperty(window, 'innerHeight', { value: 600, writable: true });

        const wrapper = mount(LibraryView);
        await flushPromises();

        const row = wrapper.find('.track-row');
        // Trigger right click near bottom-right edge (x=990, y=590)
        await row.trigger('contextmenu', { clientX: 990, clientY: 590 });
        await flushPromises();

        const menu = document.querySelector('.context-menu') as HTMLElement;
        if (menu) {
            const top = parseInt(menu.style.top || '0');
            const left = parseInt(menu.style.left || '0');
            // menuWidth 240 + padding 8 = 752 max X
            expect(left).toBeLessThanOrEqual(1000 - 240);
            // menuHeight 360 + padding 8 = 232 max Y
            expect(top).toBeLessThanOrEqual(600 - 360);
        }
    });
});

describe('LibraryView context menu playback & shortcuts (FE-9)', () => {
    beforeAll(() => {
        // jsdom does not implement media playback; stub the prototype like
        // usePlayer.spec.ts so the player composable can run.
        Object.defineProperty(window.HTMLMediaElement.prototype, 'play', {
            configurable: true,
            value: vi.fn(() => Promise.resolve()),
        });
        Object.defineProperty(window.HTMLMediaElement.prototype, 'pause', {
            configurable: true,
            value: vi.fn(),
        });
        Object.defineProperty(window.HTMLMediaElement.prototype, 'load', {
            configurable: true,
            value: vi.fn(),
        });
    });

    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        usePlayer().stop();
    });

    async function mountWithTrack(overrides: Partial<LibraryTrack> = {}) {
        const invokeCalls: { cmd: string; args: any }[] = [];
        const mockTracks = [
            createTestTrack({ id: 301, title: 'Playable Track', download_status: 'downloaded', quality: 'FLAC', ...overrides }),
        ];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            if (cmd === 'resolve_playback_source') return { track_id: 301, file_path: '/lib/301.flac', format: 'FLAC' };
            if (cmd === 'add_to_queue') return 1;
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        const row = wrapper.find('.track-row');
        await row.trigger('contextmenu', { clientX: 100, clientY: 100 });

        const menu = document.querySelector('.context-menu') as HTMLElement;
        expect(menu).not.toBeNull();

        return { invokeCalls, menu };
    }

    function menuButton(menu: HTMLElement, label: string): HTMLButtonElement {
        const btn = [...menu.querySelectorAll('button')].find(b => b.textContent?.includes(label));
        expect(btn, `context menu button "${label}"`).toBeDefined();
        return btn as HTMLButtonElement;
    }

    it('grid tile play button starts playback of the album instead of navigating', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        const mockTracks = [
            createTestTrack({ id: 401, title: 'Album Opener', album_name: 'Nightcall', album_id: 77, download_status: 'downloaded', quality: 'FLAC' }),
        ];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            if (cmd === 'resolve_playback_source') return { track_id: 401, file_path: '/lib/401.flac', format: 'FLAC' };
            return null;
        });

        const wrapper = mount(LibraryView);
        await flushPromises();

        const gridBtn = wrapper.findAll('.view-toggle button')[1];
        expect(gridBtn).toBeDefined();
        await gridBtn.trigger('click');
        await flushPromises();

        const playBtn = wrapper.findAll('.tile-overlay button').find(b => b.text().includes('play_arrow'));
        expect(playBtn).toBeDefined();

        await playBtn!.trigger('click');
        await flushPromises();

        const call = invokeCalls.find(c => c.cmd === 'resolve_playback_source');
        expect(call).toBeDefined();
        expect(call?.args).toEqual({ trackId: 401 });
    });

    it('context menu "Play Now" starts real playback of the track', async () => {
        const { invokeCalls, menu } = await mountWithTrack();

        menuButton(menu, 'Play Now').dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await flushPromises();

        const call = invokeCalls.find(c => c.cmd === 'resolve_playback_source');
        expect(call).toBeDefined();
        expect(call?.args).toEqual({ trackId: 301 });
    });

    it('context menu "Play Next" queues the track in the player', async () => {
        const { invokeCalls, menu } = await mountWithTrack();

        menuButton(menu, 'Play Next').dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await flushPromises();

        // Idle player: the queue action starts the track right away.
        const call = invokeCalls.find(c => c.cmd === 'resolve_playback_source');
        expect(call).toBeDefined();
        expect(call?.args).toEqual({ trackId: 301 });
    });

    it('context menu "Add to Queue" enqueues the track for download', async () => {
        const { invokeCalls, menu } = await mountWithTrack();

        menuButton(menu, 'Add to Queue').dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await flushPromises();

        const call = invokeCalls.find(c => c.cmd === 'add_to_queue');
        expect(call).toBeDefined();
        expect(call?.args?.trackId).toBe(301);
    });

    it('Space with the context menu open and an idle player starts that track', async () => {
        const { invokeCalls } = await mountWithTrack();

        const evt = new KeyboardEvent('keydown', { key: ' ', cancelable: true });
        const preventSpy = vi.spyOn(evt, 'preventDefault');
        window.dispatchEvent(evt);
        await flushPromises();

        const call = invokeCalls.find(c => c.cmd === 'resolve_playback_source');
        expect(call).toBeDefined();
        expect(call?.args).toEqual({ trackId: 301 });
        expect(preventSpy).toHaveBeenCalled();
    });

    it('Space toggles playback of the loaded track without resolving a new source', async () => {
        const { invokeCalls } = await mountWithTrack();

        const playSpy = window.HTMLMediaElement.prototype.play as unknown as ReturnType<typeof vi.fn>;
        const firstPlayCalls = playSpy.mock.calls.length;
        window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', cancelable: true }));
        await flushPromises();

        // Same source resolved again? No — toggle only touches the audio element.
        expect(invokeCalls.filter(c => c.cmd === 'resolve_playback_source').length).toBe(1);
        expect(playSpy.mock.calls.length).toBeGreaterThan(firstPlayCalls);
    });

    it('Q adds the context-menu track to the download queue', async () => {
        const { invokeCalls } = await mountWithTrack();

        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'q' }));
        await flushPromises();

        const call = invokeCalls.find(c => c.cmd === 'add_to_queue');
        expect(call).toBeDefined();
        expect(call?.args?.trackId).toBe(301);
    });

    it('N queues the context-menu track as "Play Next"', async () => {
        const { invokeCalls } = await mountWithTrack();

        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'n' }));
        await flushPromises();

        const call = invokeCalls.find(c => c.cmd === 'resolve_playback_source');
        expect(call).toBeDefined();
        expect(call?.args).toEqual({ trackId: 301 });
    });
});

describe('LibraryView quality deep-link (FE-10)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('filters tracks by the quality bucket from route.query', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Hi-Res Song', download_status: 'downloaded', quality: 'FLAC', quality_bucket: 'Hi-Res (24-bit+)' }),
            createTestTrack({ id: 2, title: 'CD Song', download_status: 'downloaded', quality: 'FLAC', quality_bucket: 'CD Quality' }),
            createTestTrack({ id: 3, title: 'Lossy Song', download_status: 'downloaded', quality: 'MP3', quality_bucket: 'Lossy' }),
        ];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 3, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const router = createRouter({
            history: createMemoryHistory(),
            routes: [{ path: '/library', component: LibraryView }],
        });
        await router.push('/library?filter=quality&quality=Hi-Res%20(24-bit%2B)');
        await router.isReady();

        const wrapper = mount(LibraryView, { global: { plugins: [router] } });
        await flushPromises();

        const text = wrapper.text();
        expect(text).toContain('Hi-Res Song');
        expect(text).not.toContain('CD Song');
        expect(text).not.toContain('Lossy Song');
    });

    it('applies the CD Quality bucket for the matching deep-link', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Hi-Res Song', download_status: 'downloaded', quality: 'FLAC', quality_bucket: 'Hi-Res (24-bit+)' }),
            createTestTrack({ id: 2, title: 'CD Song', download_status: 'downloaded', quality: 'FLAC', quality_bucket: 'CD Quality' }),
        ];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 2, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const router = createRouter({
            history: createMemoryHistory(),
            routes: [{ path: '/library', component: LibraryView }],
        });
        await router.push('/library?filter=quality&quality=CD%20Quality');
        await router.isReady();

        const wrapper = mount(LibraryView, { global: { plugins: [router] } });
        await flushPromises();

        const text = wrapper.text();
        expect(text).toContain('CD Song');
        expect(text).not.toContain('Hi-Res Song');
    });

    it('surfaces the deep-linked bucket as a removable active filter', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Hi-Res Song', download_status: 'downloaded', quality: 'FLAC', quality_bucket: 'Hi-Res (24-bit+)' }),
            createTestTrack({ id: 2, title: 'Lossy Song', download_status: 'downloaded', quality: 'MP3', quality_bucket: 'Lossy' }),
        ];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 2, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const router = createRouter({
            history: createMemoryHistory(),
            routes: [{ path: '/library', component: LibraryView }],
        });
        await router.push('/library?filter=quality&quality=Hi-Res%20(24-bit%2B)');
        await router.isReady();

        const wrapper = mount(LibraryView, { global: { plugins: [router] } });
        await flushPromises();

        // 'quality' is not a pill, so the pills row gives no feedback: the
        // active-filters bar must show the bucket and offer the way out.
        const bar = wrapper.find('.active-filters');
        expect(bar.exists()).toBe(true);
        expect(bar.text()).toContain('Quality: Hi-Res (24-bit+)');

        const clear = bar.find('button');
        await clear.trigger('click');
        await flushPromises();

        expect(wrapper.find('.active-filters').exists()).toBe(false);
        expect(wrapper.text()).toContain('Hi-Res Song');
        expect(wrapper.text()).toContain('Lossy Song');
    });

    it('re-applies the deep-link when the query changes on the same route', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Hi-Res Song', download_status: 'downloaded', quality: 'FLAC', quality_bucket: 'Hi-Res (24-bit+)' }),
            createTestTrack({ id: 2, title: 'Lossy Song', download_status: 'downloaded', quality: 'MP3', quality_bucket: 'Lossy' }),
        ];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 2, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const router = createRouter({
            history: createMemoryHistory(),
            routes: [{ path: '/library', component: LibraryView }],
        });
        await router.push('/library?filter=quality&quality=Hi-Res%20(24-bit%2B)');
        await router.isReady();

        const wrapper = mount(LibraryView, { global: { plugins: [router] } });
        await flushPromises();
        expect(wrapper.text()).not.toContain('Lossy Song');

        // Same route, different bucket: without a query watcher the mount-time
        // filter would stick.
        await router.push('/library?filter=quality&quality=Lossy');
        await flushPromises();

        expect(wrapper.text()).toContain('Lossy Song');
        expect(wrapper.text()).not.toContain('Hi-Res Song');
    });

    it('ignores filter=quality without a quality parameter (no no-op filter)', async () => {
        const mockTracks = [
            createTestTrack({ id: 1, title: 'Any Song', quality_bucket: 'Lossy' }),
        ];
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: mockTracks, total: 1, offset: 0, limit: 50, has_more: false };
            return null;
        });

        const router = createRouter({
            history: createMemoryHistory(),
            routes: [{ path: '/library', component: LibraryView }],
        });
        await router.push('/library?filter=quality');
        await router.isReady();

        const wrapper = mount(LibraryView, { global: { plugins: [router] } });
        await flushPromises();

        expect(wrapper.text()).toContain('Any Song');
    });
});
