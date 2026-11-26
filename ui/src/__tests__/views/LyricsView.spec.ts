/**
 * LyricsView.spec.ts
 * Component tests for the fully-wired Lyrics tab: real player bindings,
 * fetch dialog, batch operations, sync editor, quality report, providers.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import LyricsView from '@/views/LyricsView.vue';
import { mockInvoke, resetMocks, emitMockEvent } from '../setup';
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

    it.each(['word', 'syllable'] as const)('edits %s tokens, rejects invalid timestamps, saves at original granularity and cancels without writes', async (level) => {
        const content = level === 'word'
            ? '[00:01.00][00:02.00] <00:01.00>你好 <00:01.50>café'
            : '[1000,2400]<0,500,0>你<500,700,0>好';
        const lyric = { ...lrcLyrics(1), sync_level: level, content };
        const writes: unknown[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ lyrics_type: 'synced' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return lyric;
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 1, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            if (cmd === 'save_lyrics') { writes.push(args); return { ...lyric, content: (args as { params: { content: string } }).params.content }; }
            return null;
        });
        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();
        expect(wrapper.text()).toContain(`Sync · ${level}`);
        await wrapper.find('button[title="Edit"]').trigger('click');
        expect(wrapper.find('[data-testid="token-editor"]').exists()).toBe(true);
        await wrapper.find('input[aria-label="Text token 1-1"]').setValue('改 ');
        await wrapper.findAll('.lyrics-editor button').find(b => b.text().includes('Cancel'))!.trigger('click');
        expect(writes).toHaveLength(0);
        await wrapper.find('button[title="Edit"]').trigger('click');
        expect((wrapper.find('input[aria-label="Text token 1-1"]').element as HTMLInputElement).value).not.toBe('改 ');
        await wrapper.find('input[aria-label="Timestamp token 1-1"]').setValue('bad');
        await wrapper.findAll('.lyrics-editor button').find(b => b.text().includes('Save Changes'))!.trigger('click');
        expect(writes).toHaveLength(0);
        await wrapper.find('input[aria-label="Timestamp token 1-1"]').setValue(level === 'word' ? '00:01.20' : '100,500,0');
        await wrapper.find('input[aria-label="Text token 1-1"]').setValue('改 ');
        await wrapper.findAll('.lyrics-editor button').find(b => b.text().includes('Save Changes'))!.trigger('click');
        await flushPromises();
        expect(writes).toHaveLength(1);
        expect((writes[0] as { params: { syncLevel: string; content: string } }).params).toMatchObject({ syncLevel: level });
        expect((writes[0] as { params: { content: string } }).params.content).toContain('改 ');
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
        // LY-01: In listening mode timestamps are hidden by default
        expect(wrapper.text()).not.toContain('00:05.50');
        // When toggle button is clicked, timestamps are rendered
        await wrapper.find('[data-testid="toggle-timestamps"]').trigger('click');
        expect(wrapper.text()).toContain('00:05.50');
    });

    it('Q-07: listening mode hides word timestamps without changing stored lyrics', async () => {
        const wordLyrics = {
            ...lrcLyrics(1),
            sync_level: 'word' as const,
            content: '[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you',
        };
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 1, lyrics_type: 'synced' })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics') return wordLyrics;
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 1, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('.lyrics-track-list .cursor-pointer')[0].trigger('click');
        await flushPromises();

        const visibleLine = wrapper.find('.synced-line');
        expect(visibleLine.text()).toContain('I wish you');
        expect(visibleLine.text()).not.toContain('<00:');
        expect(wordLyrics.content).toContain('<00:10.50>');
        await wrapper.find('[data-testid="toggle-timestamps"]').trigger('click');
        expect(wrapper.find('.synced-line .timestamp').text()).toBe('00:10.00');
        expect(wrapper.find('.synced-line').text()).not.toContain('<00:');
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
        expect(wrapper.text()).toContain('Lyrics fetched from lrclib');
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

        await wrapper.find('[title="Lyrics sources and preferences"]').trigger('click');
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

        await wrapper.find('[title="Export lyrics in their native format"]').trigger('click');
        await flushPromises();

        expect(writes.length).toBe(1);
        expect(writes[0].path).toBe('/Users/tardis/Music/backup.json');
        expect(writes[0].contents).toContain('[00:05.50]Second line');
        expect(writes[0].contents).toContain('[00:01.00]First line');
    });

    it('the cancel button stops the backend loop after the track in flight', async () => {
        let resolveBatch!: (value: unknown) => void;
        const batch = new Promise((resolve) => { resolveBatch = resolve; });
        const invoked: string[] = [];
        mockInvoke((cmd) => {
            invoked.push(cmd);
            if (cmd === 'get_library') return {
                tracks: [createTestTrack({ id: 5, title: 'Batch Song', lyrics_type: 'none' })],
                total: 1, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'update_lyrics_config') return { ...CONFIG };
            if (cmd === 'batch_fetch_lyrics_with_progress') return batch;
            if (cmd === 'cancel_batch_fetch_lyrics') return true;
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 0, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.find('input[type="checkbox"]').trigger('click');
        await flushPromises();
        const fetchBtn = wrapper.findAll('button').find(b => b.text().includes('Fetch Lyrics'))!;
        await fetchBtn.trigger('click');
        await wrapper.findAll('button').find(b => b.text().trim() === 'Prefer synced')!.trigger('click');
        await flushPromises();

        expect(document.body.textContent).toContain('Fetching Lyrics');
        const cancelBtn = document.querySelector('[data-testid="batch-cancel"]') as HTMLButtonElement;
        cancelBtn.click();
        await flushPromises();

        // The dialog stays open, visibly waiting for the in-flight track, and
        // the backend was actually asked to stop.
        expect(invoked).toContain('cancel_batch_fetch_lyrics');
        expect(document.body.textContent).toContain('Cancelling after the current track');
        expect(cancelBtn.disabled).toBe(true);

        emitMockEvent('lyrics-fetch-progress', {
            status: 'cancelled', current: 1, total: 2, track: 'Batch Song', message: '',
        });
        resolveBatch({ fetched: 1, failed: 0, skipped: 0, cancelled: true });
        await flushPromises();

        // A cancelled run ends without the running dialog lingering.
        expect(document.body.textContent).not.toContain('Fetching Lyrics');
        expect(document.querySelector('[data-testid="batch-cancel"]')).toBeNull();
    });

    it('a batch started after a cancelled one starts clean and can complete', async () => {
        const deferred: Array<(value: unknown) => void> = [];
        let cancelCalls = 0;
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return {
                tracks: [createTestTrack({ id: 5, title: 'Batch Song', lyrics_type: 'none' })],
                total: 1, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'update_lyrics_config') return { ...CONFIG };
            if (cmd === 'batch_fetch_lyrics_with_progress') {
                return new Promise((resolve) => { deferred.push(resolve); });
            }
            if (cmd === 'cancel_batch_fetch_lyrics') { cancelCalls++; return true; }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 0, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        const startBatch = async () => {
            await wrapper.find('input[type="checkbox"]').trigger('click');
            await flushPromises();
            await wrapper.findAll('button').find(b => b.text().includes('Fetch Lyrics'))!.trigger('click');
            await wrapper.findAll('button').find(b => b.text().trim() === 'Prefer synced')!.trigger('click');
            await flushPromises();
        };

        await startBatch();
        (document.querySelector('[data-testid="batch-cancel"]') as HTMLButtonElement).click();
        await flushPromises();
        emitMockEvent('lyrics-fetch-progress', {
            status: 'cancelled', current: 1, total: 2, track: 'Batch Song', message: '',
        });
        deferred[0]({ fetched: 1, failed: 0, skipped: 0, cancelled: true });
        await flushPromises();
        expect(document.body.textContent).not.toContain('Fetching Lyrics');
        expect(cancelCalls).toBe(1);

        // Second run: the previous cancellation must not leak into it.
        await startBatch();
        expect(document.body.textContent).toContain('Fetching Lyrics');
        expect(document.body.textContent).not.toContain('Cancelling');
        emitMockEvent('lyrics-fetch-progress', {
            status: 'completed', current: 2, total: 2, track: 'Batch Song', message: '',
        });
        deferred[1]({ fetched: 2, failed: 0, skipped: 0, cancelled: false });
        await flushPromises();
        expect(cancelCalls).toBe(1);
        expect(document.body.textContent).not.toContain('Fetching Lyrics');
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
            if (cmd === 'batch_fetch_lyrics_with_progress') return { fetched: 1, failed: 0, skipped: 0, cancelled: false };
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

    it('#378 H5: a cancelled "Synced only" batch neither prunes nor deletes lyrics', async () => {
        const deletedTracks: number[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return {
                tracks: [createTestTrack({ id: 5, title: 'Batch Song', lyrics_type: 'none' })],
                total: 1, offset: 0, limit: 500, has_more: false,
            };
            // An unsynced payload is stored, but the run was cancelled: it must
            // survive because the batch never reached every target.
            if (cmd === 'get_lyrics') return plainLyrics(5);
            if (cmd === 'batch_fetch_lyrics_with_progress') {
                return { fetched: 1, failed: 0, skipped: 0, cancelled: true };
            }
            if (cmd === 'delete_lyrics') { deletedTracks.push(Number((args as { trackId: number }).trackId)); return null; }
            if (cmd === 'update_lyrics_config') return { ...CONFIG, min_sync_level: 'word' };
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.find('input[type="checkbox"]').trigger('click');
        await flushPromises();
        await wrapper.findAll('button').find(b => b.text().includes('Fetch Lyrics'))!.trigger('click');
        await wrapper.findAll('button').find(b => b.text().trim() === 'Synced only')!.trigger('click');
        await flushPromises();

        expect(deletedTracks).toEqual([]);
    });

    it('#378: a cancel names the running batch id and the next batch gets a fresh one', async () => {
        const batchRunIds: number[] = [];
        const cancelRunIds: number[] = [];
        const deferred: Array<(value: unknown) => void> = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return {
                tracks: [createTestTrack({ id: 5, title: 'Batch Song', lyrics_type: 'none' })],
                total: 1, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'update_lyrics_config') return { ...CONFIG };
            if (cmd === 'batch_fetch_lyrics_with_progress') {
                batchRunIds.push(Number((args as { runId: number }).runId));
                return new Promise((resolve) => { deferred.push(resolve); });
            }
            if (cmd === 'cancel_batch_fetch_lyrics') {
                cancelRunIds.push(Number((args as { runId: number }).runId));
                return true;
            }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 0, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        const startBatch = async () => {
            await wrapper.find('input[type="checkbox"]').trigger('click');
            await flushPromises();
            await wrapper.findAll('button').find(b => b.text().includes('Fetch Lyrics'))!.trigger('click');
            await wrapper.findAll('button').find(b => b.text().trim() === 'Prefer synced')!.trigger('click');
            await flushPromises();
        };

        await startBatch();
        (document.querySelector('[data-testid="batch-cancel"]') as HTMLButtonElement).click();
        await flushPromises();
        // The cancel carries the id of the run it wants to stop, so the backend
        // can honour it even if it lands before the command did.
        expect(batchRunIds[0]).toBeGreaterThan(0);
        expect(cancelRunIds).toEqual([batchRunIds[0]]);

        deferred[0]({ fetched: 0, failed: 0, skipped: 0, cancelled: true });
        await flushPromises();

        // A new batch never reuses the previous id (a late cancel cannot kill it).
        await startBatch();
        expect(batchRunIds[1]).not.toBe(batchRunIds[0]);
        deferred[1]({ fetched: 1, failed: 0, skipped: 0, cancelled: false });
        await flushPromises();
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
        expect(wrapper.text()).toContain('Lyrics found as an .lrc file next to the audio');
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

        const scanBtn = wrapper.findAll('button').find(b => b.text().includes('Scan disk'))!;
        expect(scanBtn).toBeTruthy();
        await scanBtn.trigger('click');
        await flushPromises();

        expect(harvests).toHaveLength(1);
        expect(wrapper.text()).toContain('Scan: 5 lyrics recovered (3 sidecar, 2 embedded) from 12 tracks');
    });
});
