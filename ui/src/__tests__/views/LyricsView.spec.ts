/**
 * LyricsView.spec.ts
 * Component tests for the fully-wired Lyrics tab: real player bindings,
 * fetch dialog, batch operations, sync editor, quality report, providers.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import LyricsView from '@/views/LyricsView.vue';
import { mockInvoke, resetMocks } from '../setup';
import type { LibraryTrack, Lyrics } from '@/api/types';

function createTestTrack(overrides: Partial<LibraryTrack> = {}): LibraryTrack {
    return {
        id: 1,
        title: 'Test Track',
        artist_name: 'Test Artist',
        artist_id: null,
        album_name: 'Test Album',
        album_id: null,
        duration_ms: 180000,
        isrc: null,
        services: 'Spotify',
        quality: '320kbps',
        download_status: 'downloaded',
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
        file_path: '/music/test.flac',
        musicbrainz_id: null,
        ...overrides
    };
}

function lrcLyrics(trackId: number): Lyrics {
    return {
        id: 10,
        track_id: trackId,
        format: 'lrc',
        sync_level: 'line',
        source: 'lrclib',
        content: '[00:01.00]First line\n[00:05.50]Second line\n[00:09.00]Third line',
        language: 'en',
        embedded_in_file: false,
        created_at: '2026-08-25T10:00:00Z',
    };
}

function plainLyrics(trackId: number): Lyrics {
    return {
        id: 11,
        track_id: trackId,
        format: 'plain',
        sync_level: 'none',
        source: 'genius',
        content: 'Verse one text\n\nChorus text',
        language: 'en',
        embedded_in_file: false,
        created_at: '2026-08-24T10:00:00Z',
    };
}

const PROVIDERS = [
    { id: 1, provider_id: 'lrclib', provider_name: 'LRCLIB', enabled: true, priority: 1, sync_level: 'line' },
    { id: 2, provider_id: 'musixmatch', provider_name: 'Musixmatch', enabled: false, priority: 2, sync_level: 'line' },
];

const CONFIG = {
    id: 1, min_sync_level: 'none', preferred_language: '', storage_format: 'lrc',
    auto_fetch_on_import: false, retry_failed: false, retry_frequency: 'always',
};

describe('LyricsView (fully wired tab)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    it('renders tracks from get_library with lyrics status badges', async () => {
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return {
                tracks: [
                    createTestTrack({ id: 1, title: 'Song One', lyrics_type: 'synced' }),
                    createTestTrack({ id: 2, title: 'Song Two', lyrics_type: 'plain' }),
                    createTestTrack({ id: 3, title: 'Song Three', lyrics_type: 'none' }),
                ],
                total: 3, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_lyrics_stats') return { total_tracks: 3, with_lyrics: 2, synced_lyrics: 1, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        expect(wrapper.text()).toContain('Song One');
        expect(wrapper.text()).toContain('Song Two');
        expect(wrapper.text()).toContain('Song Three');
        // Real stats strip
        expect(wrapper.text()).toContain('1 synced');
    });

    it('shows the empty-library state when there are no tracks', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics_stats') return { total_tracks: 0, with_lyrics: 0, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return [];
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        expect(wrapper.text()).toContain('No tracks in library');
    });

    it('loads and renders parsed synced lines when a synced track is selected', async () => {
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 1, lyrics_type: 'synced' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return lrcLyrics(Number((args as { trackId: number }).trackId));
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 1, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('Second line');
        // Timestamp labels rendered from real LRC parsing
        expect(wrapper.text()).toContain('00:05.50');
    });

    it('shows unsynced paragraphs for plain-format lyrics', async () => {
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 2, title: 'Plain Song', lyrics_type: 'plain' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return plainLyrics(Number((args as { trackId: number }).trackId));
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('Unsynced');
        expect(wrapper.text()).toContain('Chorus text');
    });

    it('auto-fetch on the no-lyrics state calls fetch_and_save_lyrics and applies the result', async () => {
        const fetched = plainLyrics(9);
        fetched.source = 'lrclib';
        let fetchCalled = false;
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 9, lyrics_type: 'none' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return fetchCalled ? fetched : null;
            if (cmd === 'fetch_and_save_lyrics') { fetchCalled = true; return fetched; }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        expect(wrapper.text()).toContain('No Lyrics Available');

        const searchBtn = wrapper.findAll('button').find(b => b.text().includes('Search for Lyrics'))!;
        await searchBtn.trigger('click');
        await flushPromises();

        expect(fetchCalled).toBe(true);
        expect(wrapper.text()).toContain('Letra obtenida desde lrclib');
    });

    it('quality report computes its numbers from real stats, not placeholders', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return {
                tracks: [
                    createTestTrack({ id: 1, lyrics_type: 'synced' }),
                    createTestTrack({ id: 2, lyrics_type: 'plain' }),
                    createTestTrack({ id: 3, lyrics_type: 'none' }),
                ],
                total: 3, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_all_lyrics') return [lrcLyrics(1), plainLyrics(2)];
            if (cmd === 'get_lyrics_stats') return { total_tracks: 3, with_lyrics: 2, synced_lyrics: 1, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        const analyticsBtn = wrapper.find('[title="Lyrics Quality Report"]');
        await analyticsBtn.trigger('click');
        await flushPromises();

        const modalEl = document.body.querySelector('.quality-report');
        expect(modalEl).not.toBeNull();
        // Real derived numbers: 1 synced / 1 unsynced / 1 without lyrics
        expect(modalEl!.textContent).toContain('Synced (33%)');
        expect(modalEl!.textContent).toContain('No Lyrics (33%)');
        // Sync-level breakdown from actual records
        expect(modalEl!.textContent).toContain('Line-level');
    });

    it('provider settings load from the backend and toggles persist via update_lyrics_provider', async () => {
        const updatedProviders: Array<{ provider_id: string; enabled?: boolean }> = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics_stats') return { total_tracks: 0, with_lyrics: 0, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS.map(p => ({ ...p }));
            if (cmd === 'get_lyrics_config') return CONFIG;
            if (cmd === 'update_lyrics_provider') {
                const a = args as { providerId: string; enabled: boolean; priority: number };
                updatedProviders.push({ provider_id: a.providerId, enabled: a.enabled });
                return { id: 1, provider_id: a.providerId, provider_name: 'LRCLIB', enabled: a.enabled, priority: a.priority, sync_level: 'line' };
            }
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        await wrapper.find('[title="Fuentes de letras y preferencias"]').trigger('click');
        await flushPromises();

        const modalEl = document.body.querySelector('.provider-settings');
        expect(modalEl).not.toBeNull();
        expect(modalEl!.textContent).toContain('LRCLIB');
        expect(modalEl!.textContent).toContain('Musixmatch');

        // Toggle the first provider switch off (role=switch buttons)
        const switches = Array.from(modalEl!.querySelectorAll('button[role="switch"]')) as HTMLButtonElement[];
        expect(switches.length).toBe(2);
        switches[0].click();
        await flushPromises();

        expect(updatedProviders.length).toBe(1);
        expect(updatedProviders[0].provider_id).toBe('lrclib');
        expect(updatedProviders[0].enabled).toBe(false);
    });

    it('saving edited lyrics persists through save_lyrics with detected plain format', async () => {
        const savedPayloads: unknown[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 2, title: 'Plain Song', lyrics_type: 'plain' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return plainLyrics(2);
            if (cmd === 'save_lyrics') { savedPayloads.push(args); return { ...plainLyrics(2), content: 'Edited line' }; }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        await wrapper.find('[title="Edit"]').trigger('click');

        const textarea = wrapper.find('textarea');
        await textarea.setValue('Edited line');

        const saveBtn = wrapper.findAll('button').find(b => b.text().includes('Save Changes'))!;
        await saveBtn.trigger('click');
        await flushPromises();

        expect(savedPayloads.length).toBe(1);
        const params = (savedPayloads[0] as { params: { format: string; content: string } }).params;
        expect(params.format).toBe('plain');
        expect(params.content).toBe('Edited line');
    });

    it('exports the current lyrics to a dialog-resolved path via write_text_file', async () => {
        const writes: Array<{ path: string; contents: string }> = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 1, lyrics_type: 'synced' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return lrcLyrics(1);
            if (cmd === 'write_text_file') {
                const a = args as { path: string; contents: string };
                writes.push({ path: a.path, contents: a.contents });
                return a.contents.length;
            }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 1, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        await wrapper.find('[title="Exportar letra en su formato nativo"]').trigger('click');
        await flushPromises();

        expect(writes.length).toBe(1);
        expect(writes[0].path).toBe('/Users/tardis/Music/backup.json');
        expect(writes[0].contents).toContain('[00:05.50]Second line');
        expect(writes[0].contents).toContain('[00:01.00]First line');
    });

    it('"Synced only" fetch mode persists min_sync_level and enforces it after the batch', async () => {
        const configUpdates: Array<Record<string, unknown>> = [];
        const deletedTracks: number[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return {
                tracks: [createTestTrack({ id: 5, title: 'Batch Song', lyrics_type: 'none' })],
                total: 1, offset: 0, limit: 500, has_more: false,
            };
            // After the fetch the stored lyric is unsynced → must be dropped.
            if (cmd === 'get_lyrics') return plainLyrics(5);
            if (cmd === 'batch_fetch_lyrics_with_progress') return { fetched: 1, failed: 0, skipped: 0 };
            if (cmd === 'delete_lyrics') { deletedTracks.push(Number((args as { trackId: number }).trackId)); return null; }
            if (cmd === 'update_lyrics_config') {
                configUpdates.push((args as { config: Record<string, unknown> }).config);
                return { ...CONFIG, ...(args as { config: Record<string, unknown> }).config };
            }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 0, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        const checkbox = wrapper.find('input[type="checkbox"]');
        await checkbox.trigger('click');
        await flushPromises();

        // Open fetch dropdown and pick "Synced only"
        const fetchBtn = wrapper.findAll('button').find(b => b.text().includes('Fetch Lyrics'))!;
        await fetchBtn.trigger('click');
        const syncedOnly = wrapper.findAll('button')
            .find(b => b.text().trim() === 'Synced only');
        expect(syncedOnly).toBeDefined();
        await syncedOnly!.trigger('click');
        await flushPromises();

        expect(configUpdates.length).toBe(1);
        expect(configUpdates[0].min_sync_level).toBe('word');
        // The unsynced payload was deleted per the "synced only" preference
        expect(deletedTracks).toEqual([5]);
    });
});

describe('S200: local lyrics harvest', () => {
    it('probes the local file (embedded/sidecar) when the DB has no lyrics', async () => {
        const embedded = plainLyrics(7);
        embedded.source = 'sidecar';
        let probeCalls: number[] = [];
        mockInvoke((cmd, args) => {
            // After a successful probe the refreshed list reports the lyrics.
            if (cmd === 'get_library' && probeCalls.length > 0) return { tracks: [createTestTrack({ id: 7, lyrics_type: 'plain' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 7, lyrics_type: 'none' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return null;
            if (cmd === 'probe_track_lyrics') { probeCalls.push(Number((args as { trackId: number }).trackId)); return embedded; }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        expect(probeCalls).toEqual([7]);
        expect(wrapper.text()).toContain('Chorus text');
        expect(wrapper.text()).toContain('Letra encontrada como archivo .lrc junto al audio');
    });

    it('"Escanear disco" sweeps sidecars + embedded lyrics and refreshes stats', async () => {
        const harvests: Array<Record<string, unknown>> = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 1 })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'harvest_missing_lyrics') { harvests.push(args ?? {}); return { scanned: 12, sidecar_found: 3, embedded_found: 2, failed: 0 }; }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 3, with_lyrics: 2, synced_lyrics: 1, embedded_lyrics: 2 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        // The feedback banner renders inside the viewer pane → select a track first.
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        const scanBtn = wrapper.findAll('button').find(b => b.text().includes('Escanear disco'))!;
        expect(scanBtn).toBeTruthy();
        await scanBtn.trigger('click');
        await flushPromises();

        expect(harvests).toHaveLength(1);
        expect(wrapper.text()).toContain('5 letras recuperadas (3 sidecar, 2 incrustadas) de 12 pistas');
    });
});
