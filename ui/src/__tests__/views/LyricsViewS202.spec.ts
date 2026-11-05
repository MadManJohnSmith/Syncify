/**
 * LyricsViewS202.spec.ts
 * Component tests for the S202 karaoke refetch toolbar action in Lyrics.
 * The animated-cover sweep moved to MetadataView (MET-03 / LY-04).
 * See MetadataViewAnimatedCover.spec.ts for its replacement coverage.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import LyricsView from '@/views/LyricsView.vue';
import { mockInvoke, resetMocks, emitMockEvent } from '../setup';
import type { LibraryTrack } from '@/api/types';

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

const PROVIDERS = [
    { id: 1, provider_id: 'lrclib', provider_name: 'LRCLIB', enabled: true, priority: 1, sync_level: 'line' },
];
const CONFIG = {
    id: 1, min_sync_level: 'none', preferred_language: '', storage_format: 'lrc',
    auto_fetch_on_import: false, retry_failed: false, retry_frequency: 'always',
};

describe('S202: karaoke refetch + LY-04 absence of cover sweep', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    it('«Re-chequear todo (Karaoke)» runs the chosen scope and shows honest counts', async () => {
        const calls: Array<{ cmd: string; args: unknown }> = [];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'get_library') return { tracks: [createTestTrack()], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'refetch_karaoke_lyrics') {
                // Honest NO-DEGRADE counters straight from the backend contract.
                return {
                    checked: 10, upgraded_to_word: 4, upgraded_other: 1, filled_from_missing: 2,
                    kept: 2, downgraded_rejected: 1, failed: 0, embed_skipped: 0, cancelled: false,
                };
            }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 10, with_lyrics: 9, synced_lyrics: 7, embedded_lyrics: 3 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        const btn = wrapper.findAll('button').find(b => b.text().includes('Re-check all'))!;
        expect(btn).toBeTruthy();
        await btn.trigger('click');
        await flushPromises();

        const dialog = document.body.querySelector('.karaoke-dialog')!;
        expect(dialog).not.toBeNull();

        // Default scope is downloaded-only; run it.
        const runBtn = dialog.querySelector('[data-testid="karaoke-run"]') as HTMLButtonElement;
        runBtn.click();
        await flushPromises();

        const refetchCall = calls.find(c => c.cmd === 'refetch_karaoke_lyrics');
        expect(refetchCall).toBeTruthy();
        expect((refetchCall!.args as { scope: string }).scope).toBe('downloaded');

        const summary = document.body.querySelector('.karaoke-summary')!;
        expect(summary.textContent).toContain('Checked:');
        expect(summary.textContent).toContain('Upgraded to word:');
        expect(summary.textContent).toContain('Rejected as worse:');
    });

    it('updates the progress bar from real karaoke-refetch-progress events while running', async () => {
        let release!: (value: unknown) => void;
        const pending = new Promise(resolve => { release = resolve; });
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack()], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'refetch_karaoke_lyrics') return pending;
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        await wrapper.findAll('button').find(b => b.text().includes('Re-check all'))!.trigger('click');
        const dialog = document.body.querySelector('.karaoke-dialog')!;
        (dialog.querySelector('[data-testid="karaoke-run"]') as HTMLButtonElement).click();
        await flushPromises();

        emitMockEvent('karaoke-refetch-progress', {
            status: 'checking', current: 3, total: 10,
            track: 'Radiohead - Karma Police', message: 'Consultando proveedores...',
        });
        await flushPromises();

        expect(dialog.textContent).toContain('3 / 10');
        expect(dialog.textContent).toContain('Radiohead - Karma Police');

        release({
            checked: 10, upgraded_to_word: 0, upgraded_other: 0, filled_from_missing: 0,
            kept: 9, downgraded_rejected: 1, failed: 0, embed_skipped: 0, cancelled: false,
        });
        await flushPromises();

        // The listener is detached after completion; summary replaces live bar.
        expect(document.body.querySelector('.karaoke-summary')).not.toBeNull();
    });

    it('cancel button asks the backend to stop the running sweep', async () => {
        let release!: (value: unknown) => void;
        const pending = new Promise(resolve => { release = resolve; });
        const cancelCalls: unknown[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack()], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'refetch_karaoke_lyrics') return pending;
            if (cmd === 'cancel_karaoke_refetch') { cancelCalls.push(args); return true; }
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 0, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();
        await wrapper.findAll('button').find(b => b.text().includes('Re-check all'))!.trigger('click');
        const dialog = document.body.querySelector('.karaoke-dialog')!;
        (dialog.querySelector('[data-testid="karaoke-run"]') as HTMLButtonElement).click();
        await flushPromises();

        const cancelBtn = Array.from(dialog.querySelectorAll('button')).find(b => b.textContent!.includes('Cancel process'))!;
        cancelBtn.click();
        await flushPromises();

        expect(cancelCalls.length).toBe(1);
        release({
            checked: 2, upgraded_to_word: 0, upgraded_other: 0, filled_from_missing: 0,
            kept: 2, downgraded_rejected: 0, failed: 0, embed_skipped: 0, cancelled: true,
        });
        await flushPromises();
        // The honest cancelled flag surfaces in the dialog summary.
        const summary = document.body.querySelector('.karaoke-summary')!;
        expect(summary.textContent).toContain('cancelled');
    });

    it('«Portadas animadas» ya no vive en Letras: se movió a Metadatos (LY-04 / MET-03)', async () => {
        // El comportamiento no se borró: está registrado en matriz-ajustes.md y
        // sustituido por `MetadataViewAnimatedCover.spec.ts`, que exige además
        // que el barrido funcione sin cuenta Apple Music (R1).
        mockInvoke((cmd) => {
            if (cmd === 'get_library') return { tracks: [createTestTrack()], total: 1, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_lyrics_stats') return { total_tracks: 1, with_lyrics: 1, synced_lyrics: 1, embedded_lyrics: 0 };
            if (cmd === 'get_lyrics_providers') return PROVIDERS;
            if (cmd === 'get_lyrics_config') return CONFIG;
            return null;
        });

        const wrapper = mount(LyricsView);
        await flushPromises();

        const sweepButton = wrapper.findAll('button').find(b => b.text().includes('Portadas animadas'));
        expect(sweepButton).toBeUndefined();
        expect(document.body.querySelector('.cover-sweep-dialog')).toBeNull();
        expect(wrapper.text()).not.toContain('Barrido de portadas');
        expect(wrapper.text()).not.toContain('cover.webp');
    });
});
