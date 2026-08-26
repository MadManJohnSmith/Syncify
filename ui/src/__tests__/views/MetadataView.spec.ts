/**
 * MetadataView.spec.ts
 * Component tests for the fully-wired Metadata tab: quality report with real
 * numbers, auto-fix tools, file↔DB comparison, batch operations, exports.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import MetadataView from '@/views/MetadataView.vue';
import { mockInvoke, resetMocks, emitMockEvent } from '../setup';
import type { LibraryTrack } from '@/api/types';

vi.mock('vue-router', () => ({
    useRouter: () => ({ push: vi.fn() }),
    useRoute: () => ({ query: {}, params: {} }),
}));

function createTestTrack(overrides: Partial<LibraryTrack> = {}): LibraryTrack {
    return {
        id: 1,
        title: 'Test Track',
        artist_name: 'Test Artist',
        artist_id: null,
        album_name: 'Test Album',
        album_id: null,
        duration_ms: 180000,
        isrc: `ISRC${1000}`,
        services: 'Spotify',
        quality: '320kbps',
        download_status: 'downloaded',
        metadata_score: 80,
        lyrics_type: 'none',
        cover_art_url: 'https://example.com/cover.jpg',
        track_number: 1,
        disc_number: 1,
        genre: 'Rock',
        bpm: 120,
        musical_key: 'C Major',
        release_year: 2020,
        explicit: false,
        file_path: '/music/test.flac',
        musicbrainz_id: 'mbid-1',
        ...overrides
    };
}

const STATS = {
    total_tracks: 3, with_isrc: 2, with_musicbrainz_id: 1, with_album: 3,
    with_year: 2, with_genre: 2, with_art: 2, average_completeness: 78.5,
};

const PREFS = {
    enable_musicbrainz: true, enable_lastfm: false, enable_acoustid: false,
    weight_album: 1, weight_isrc: 1, weight_mb_id: 1, weight_cover: 1,
    weight_year: 1, weight_genre: 1,
};

describe('MetadataView (fully wired tab)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    it('renders tracks and real metadata stats', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return {
                tracks: [
                    createTestTrack({ id: 1, title: 'Song One' }),
                    createTestTrack({ id: 2, title: 'Song Two' }),
                ],
                total: 2, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        expect(wrapper.text()).toContain('Song One');
        expect(wrapper.text()).toContain('Song Two');
        // Real get_metadata_stats data drives the completeness strip
        expect(wrapper.text()).toContain('3 pistas');
        expect(wrapper.text()).toContain('ISRC 67%');
        expect(wrapper.text()).toContain('Completitud media 78.5%');
    });

    it('quality report shows counts computed from the loaded library', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return {
                tracks: [
                    createTestTrack({ id: 1 }),                       // complete
                    createTestTrack({ id: 2, isrc: null }),           // no ISRC
                    createTestTrack({ id: 3, genre: null, musicbrainz_id: null }),
                ],
                total: 3, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        await wrapper.find('[title="Analyze Quality"]').trigger('click');
        await flushPromises();

        const modalEl = document.body.querySelector('.quality-report');
        expect(modalEl).not.toBeNull();
        expect(modalEl!.textContent).toContain('No ISRC');
        expect(modalEl!.textContent).toContain('1 tracks');
        expect(modalEl!.textContent).toContain('Missing MusicBrainz IDs');
    });

    it('clicking a quality-report issue filters the list to the affected subset', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return {
                tracks: [
                    createTestTrack({ id: 1, title: 'With Isrc' }),
                    createTestTrack({ id: 2, title: 'No Isrc Here', isrc: null }),
                ],
                total: 2, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        await wrapper.find('[title="Analyze Quality"]').trigger('click');
        await flushPromises();

        const modalEl = document.body.querySelector('.quality-report')!;
        const noIsrcRow = Array.from(modalEl.querySelectorAll('button'))
            .find(b => b.textContent?.includes('No ISRC'))! as HTMLButtonElement;
        noIsrcRow.click();
        await flushPromises();

        expect(document.body.querySelector('.quality-report')).toBeNull();
        expect(wrapper.text()).toContain('No Isrc Here');
        expect(wrapper.text()).not.toContain('With Isrc');
    });

    it('comparison modal diffs file tags against the database and adopts on click', async () => {
        const updates: Array<{ trackId: number; metadata: Record<string, unknown> }> = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 1 })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            if (cmd === 'read_track_tags') {
                return {
                    file_path: '/music/test.flac',
                    cover_present: true,
                    unsynced_lyrics_present: false,
                    tags_match: false,
                    all_tags: {
                        TITLE: ['File Title'],
                        ARTIST: ['Test Artist'],
                        ALBUM: ['Test Album'],
                        GENRE: ['Rock'],
                        TRACKNUMBER: ['7'],
                    },
                };
            }
            if (cmd === 'update_track_metadata') {
                const a = args as { trackId: number; metadata: Record<string, unknown> };
                updates.push(a);
                return createTestTrack({ id: a.trackId });
            }
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        // Select the only track
        await wrapper.find('.track-row').trigger('click');
        await flushPromises();

        await wrapper.findAll('button').find(b => b.text().includes('Compare Sources'))!.trigger('click');
        await flushPromises();

        const modalEl = document.body.querySelector('.comparison-modal');
        expect(modalEl).not.toBeNull();
        // Diff row shows both sides
        expect(modalEl!.textContent).toContain('File Title');

        // Adopt the file value for Title (first differing row)
        const diffRow = Array.from(modalEl!.querySelectorAll('tbody tr'))
            .find(tr => tr.textContent?.includes('Title')) as HTMLTableRowElement;
        diffRow.click();
        await flushPromises();

        // Close modal and persist through Save Changes
        const doneBtn = Array.from(modalEl!.querySelectorAll('button'))
            .find(b => b.textContent?.includes('Listo'))! as HTMLButtonElement;
        doneBtn.click();
        await flushPromises();

        const saveBtn = wrapper.findAll('button').find(b => b.text().trim() === 'Save Changes');
        await saveBtn!.trigger('click');
        await flushPromises();

        const titleUpdate = updates.find(u => u.metadata.title === 'File Title');
        expect(titleUpdate).toBeDefined();
        expect(titleUpdate!.trackId).toBe(1);
    });

    it('"Fix Common Issues" strips junk suffixes from selected titles via update_track_metadata', async () => {
        const updates: Array<{ trackId: number; metadata: Record<string, unknown> }> = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return {
                tracks: [createTestTrack({ id: 4, title: 'Dirty Song (Official Audio)' })],
                total: 1, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            if (cmd === 'update_track_metadata') {
                const a = args as { trackId: number; metadata: Record<string, unknown> };
                updates.push(a);
                return createTestTrack({ id: a.trackId, title: String(a.metadata.title ?? '') });
            }
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        // Select the track to reveal the batch toolbar
        await wrapper.find('.track-row input[type="checkbox"]').trigger('click');
        await flushPromises();

        // Open Auto-Fix panel
        await wrapper.findAll('button').find(b => b.text().includes('Auto-Fix'))!.trigger('click');
        await flushPromises();

        const applyBtn = wrapper.findAll('button').find(b => b.text().trim() === 'Apply Fixes');
        expect(applyBtn).toBeDefined();
        await applyBtn!.trigger('click');
        await flushPromises();

        expect(updates.length).toBe(1);
        expect(updates[0].metadata.title).toBe('Dirty Song');
    });

    it('MusicBrainz enrichment is scoped per-track when a selection exists', async () => {
        const enrichedIds: number[] = [];
        let globalCalled = false;
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return {
                tracks: [
                    createTestTrack({ id: 1 }),
                    createTestTrack({ id: 2, musicbrainz_id: null }),
                ],
                total: 2, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            if (cmd === 'enrich_metadata') {
                enrichedIds.push(Number((args as { trackId: number }).trackId));
                return { success: true, updatedFields: [] };
            }
            if (cmd === 'enrich_metadata_musicbrainz') {
                globalCalled = true;
                return { total: 0, enriched: 0, failed: 0 };
            }
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        // Select both tracks
        const checkboxes = wrapper.findAll('.track-row input[type="checkbox"]');
        await checkboxes[0].trigger('click');
        await checkboxes[1].trigger('click');
        await flushPromises();

        await wrapper.findAll('button').find(b => b.text().includes('Auto-Fix'))!.trigger('click');
        await flushPromises();

        await wrapper.findAll('button').find(b => b.text().includes('Fix Selected'))!.trigger('click');
        await flushPromises();

        expect(globalCalled).toBe(false);
        expect(enrichedIds.sort()).toEqual([1, 2]);
    });

    it('exports selected-track metadata as JSON through write_text_file', async () => {
        const writes: Array<{ path: string; contents: string }> = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return {
                tracks: [createTestTrack({ id: 1, title: 'Export Me' })],
                total: 1, offset: 0, limit: 500, has_more: false,
            };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            if (cmd === 'write_text_file') {
                const a = args as { path: string; contents: string };
                writes.push({ path: a.path, contents: a.contents });
                return a.contents.length;
            }
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        await wrapper.find('.track-row input[type="checkbox"]').trigger('click');
        await flushPromises();

        await wrapper.findAll('button').find(b => b.text().includes('Export'))!.trigger('click');
        await flushPromises();

        expect(writes.length).toBe(1);
        expect(writes[0].path).toBe('/Users/tardis/Music/backup.json');
        const parsed = JSON.parse(writes[0].contents);
        expect(Array.isArray(parsed)).toBe(true);
        expect(parsed[0].title).toBe('Export Me');
    });

    it('surfaces live background enrichment status events in a banner', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        emitMockEvent('background-enrichment-status', {
            type: 'musicbrainz',
            status: 'running',
            pending: 5,
            enriched: 12,
            message: 'Enriching from MusicBrainz…',
        });
        await flushPromises();

        expect(wrapper.text()).toContain('Enriching from MusicBrainz…');
    });
});

describe('S200: Last.fm API key + full tag visibility', () => {
    it('shows key status on mount and saves a new API key from the Last.fm card', async () => {
        let savedKey: string | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 1 })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            if (cmd === 'get_lastfm_api_key_status') {
                return savedKey
                    ? { configured: true, masked: `••••${savedKey.slice(-4)}`, source: 'settings' }
                    : { configured: false, masked: null, source: 'none' };
            }
            if (cmd === 'set_lastfm_api_key') { savedKey = String((args as { apiKey: string }).apiKey); return null; }
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();

        // Select track → open Auto-Fix panel → the Last.fm card is reachable
        await wrapper.find('.track-row input[type="checkbox"]').trigger('click');
        await flushPromises();
        await wrapper.findAll('button').find(b => b.text().includes('Auto-Fix'))!.trigger('click');
        await flushPromises();

        // Status line reflects the unconfigured state before saving
        expect(wrapper.text()).toContain('Consíguela gratis en last.fm/api');

        const input = wrapper.find('input[type="password"]');
        expect(input.exists()).toBe(true);
        await input.setValue('abcd1234');
        const saveBtn = wrapper.findAll('button').find(b => b.text().trim() === 'Guardar')!;
        await saveBtn.trigger('click');
        await flushPromises();

        expect(savedKey).toBe('abcd1234');
    });

    it('renders the raw facet dump without requiring expansion', async () => {
        mockInvoke(async (cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack({ id: 3 })], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_metadata_stats') return STATS;
            if (cmd === 'get_metadata_preferences') return PREFS;
            if (cmd === 'read_track_tags') {
                void args;
                return {
                    track_id: 3,
                    file_path: '/music/test.flac',
                    file_format: 'FLAC',
                    all_tags: { TITLE: ['Hidden Song'], REPLAYGAIN_TRACK_GAIN: ['-7.20 dB'], UNSYNCEDLYRICS: ['la la la'] },
                    has_cover: true,
                    cover_mime: 'image/jpeg',
                };
            }
            return null;
        });

        const wrapper = mount(MetadataView);
        await flushPromises();
        await wrapper.find('.track-row').trigger('click');
        await flushPromises();

        // Load the snapshot first…
        await wrapper.findAll('button').find(b => b.text().includes('Leer tags del archivo'))!.trigger('click');
        await flushPromises();

        // …then the dump heading renders without any <details> expansion.
        expect(wrapper.text()).toContain('Todas las facetas crudas (3 claves)');
        expect(wrapper.text()).toContain('REPLAYGAIN_TRACK_GAIN');
    });
});
