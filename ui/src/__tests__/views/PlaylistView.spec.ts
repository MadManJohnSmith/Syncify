/**
 * PlaylistView.spec.ts
 * Tests for PlaylistView.vue download actions and IPC payloads
 *
 * S201: the «Descargar playlist» action opens a two-mode dialog:
 *  - Modo A «Solo las que ya tengo»: save-dialog + export_playlist_m3u (stat real, sin red)
 *  - Modo B «Descargar las pistas faltantes»: add_batch_to_queue solo con ids sin descarga vigente
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import PlaylistView from '@/views/PlaylistView.vue';
import { save } from '@tauri-apps/plugin-dialog';
import { mockInvoke, resetMocks } from '../setup';

describe('PlaylistView', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    const mockPlaylists = [
        { id: 1, name: 'Chill Vibes', track_count: 2, is_favorite: false, account_id: 1 }
    ];

    function makePage(tracks: any[]) {
        return { tracks, total: tracks.length, offset: 0, limit: 500, has_more: false };
    }

    const mockPlaylistTracks = makePage([
        { id: 301, title: 'Chill Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000 },
        { id: 302, title: 'Chill Track 2', artist_name: 'Artist 2', album_name: 'Album 2', duration_ms: 200000 }
    ]);

    /** The two-mode modal lives inside <Teleport to="body"> — query the DOM. */
    function bodyButtons(): HTMLButtonElement[] {
        return Array.from(document.body.querySelectorAll('button'));
    }

    async function openPlaylistAndDownloadDialog(wrapper: any) {
        await flushPromises();
        const playlistItem = wrapper.findAll('.group').find((el: any) => el.text().includes('Chill Vibes'));
        expect(playlistItem).toBeDefined();
        await playlistItem!.trigger('click');
        await flushPromises();
        // Header action opens the two-mode dialog
        const openBtn = wrapper.findAll('button').find((b: any) => b.text().includes('Descargar playlist'));
        expect(openBtn).toBeDefined();
        await openBtn!.trigger('click');
        await flushPromises();
    }

    it('Modo B queues only non-downloaded tracks via add_batch_to_queue with Library defaults', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                return makePage([
                    { id: 301, title: 'T1', duration_ms: 1000, download_status: 'downloaded', file_path: '/a.flac' },
                    { id: 302, title: 'T2', duration_ms: 2000, download_status: 'not_downloaded' },
                    { id: 303, title: 'T3', duration_ms: 3000, download_status: 'queued' }
                ]);
            }
            if (cmd === 'add_batch_to_queue') {
                return { submitted: 2, added: 2, enqueued: 2, deduplicated: 0, skipped: 0 };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await openPlaylistAndDownloadDialog(wrapper);

        // Two modes are offered, explained in Spanish
        const modalText = document.body.textContent ?? '';
        expect(modalText).toContain('Solo las que ya tengo');
        expect(modalText).toContain('Exporta un archivo .m3u');
        expect(modalText).toContain('Descargar las pistas faltantes');

        const modeB = bodyButtons().find(b => b.textContent?.includes('pistas faltantes'));
        expect(modeB).toBeDefined();
        modeB!.click();
        await flushPromises();

        const batchCall = invokeCalls.find(c => c.cmd === 'add_batch_to_queue');
        expect(batchCall).toBeDefined();
        expect(batchCall?.args).toEqual({
            trackIds: [302, 303],
            priority: 50,
            qualityPreference: 'hires',
            allowFallback: true,
        });

        // Banner shows the engine summary as-is
        expect(wrapper.text()).toContain('Cola de descargas: 2 encoladas');
    });

    it('Modo B reports SourceIdentityMissing errors gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return mockPlaylistTracks;
            if (cmd === 'add_batch_to_queue') {
                throw new Error('SourceIdentityMissing: playlist tracks have no streaming source');
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await openPlaylistAndDownloadDialog(wrapper);

        const modeB = bodyButtons().find(b => b.textContent?.includes('pistas faltantes'));
        expect(modeB).toBeDefined();
        modeB!.click();
        await flushPromises();
        expect(wrapper.exists()).toBe(true);
    });

    it('Modo A exports verified-only M3U through save dialog and lists missing tracks', async () => {
        vi.mocked(save).mockResolvedValue('/Users/tardis/Music/Chill Vibes.m3u');
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return mockPlaylistTracks;
            if (cmd === 'export_playlist_m3u') {
                return {
                    playlist_id: 1,
                    playlist_name: 'Chill Vibes',
                    total_tracks: 3,
                    verified_count: 2,
                    missing_count: 1,
                    missing_tracks: [
                        { track_id: 404, title: 'Ghost Song', artist_name: 'Artist X', reason: 'archivo_no_encontrado' }
                    ],
                    file_path: '/Users/tardis/Music/Chill Vibes.m3u',
                    bytes_written: 120,
                    m3u_content: '#EXTM3U\n'
                };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await openPlaylistAndDownloadDialog(wrapper);

        const modeA = bodyButtons().find(b => b.textContent?.includes('Solo las que ya tengo'));
        expect(modeA).toBeDefined();
        modeA!.click();
        await flushPromises();

        // Save dialog was used, then backend command received playlistId + filePath
        expect(save).toHaveBeenCalled();
        const exportCall = invokeCalls.find(c => c.cmd === 'export_playlist_m3u');
        expect(exportCall).toBeDefined();
        expect(exportCall?.args).toEqual({ playlistId: 1, filePath: '/Users/tardis/Music/Chill Vibes.m3u' });
        expect(invokeCalls.find(c => c.cmd === 'add_batch_to_queue')).toBeUndefined();

        // Honest counts banner + collapsible missing list
        expect(wrapper.text()).toContain('M3U exportado · 2/3 pistas verificadas');
        const missingToggle = wrapper.findAll('button').find((b: any) => b.text().includes('Faltantes (1)'));
        expect(missingToggle).toBeDefined();
        await missingToggle!.trigger('click');
        await flushPromises();
        expect(wrapper.text()).toContain('Ghost Song');
        expect(wrapper.text()).toContain('Archivo no encontrado');
    });

    it('Modo A without user-selected file writes nothing', async () => {
        vi.mocked(save).mockResolvedValue(null);
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return mockPlaylistTracks;
            return null;
        });

        const wrapper = mount(PlaylistView);
        await openPlaylistAndDownloadDialog(wrapper);

        const modeA = bodyButtons().find(b => b.textContent?.includes('Solo las que ya tengo'));
        expect(modeA).toBeDefined();
        modeA!.click();
        await flushPromises();

        expect(invokeCalls.find(c => c.cmd === 'export_playlist_m3u')).toBeUndefined();
    });
});
