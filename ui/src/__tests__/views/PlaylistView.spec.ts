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
import { useDialog } from '@/composables/useToast';

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
        const openBtn = wrapper.findAll('button').find((b: any) => b.text().includes('Download Playlist'));
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
        expect(modalText).toContain('Only the ones I already have');
        expect(modalText).toContain('Exports an .m3u file');
        expect(modalText).toContain('Download missing tracks');

        const modeB = bodyButtons().find(b => b.textContent?.includes('missing tracks'));
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
            jobKind: 'playlist',
        });

        // Banner shows the engine summary as-is
        expect(wrapper.text()).toContain('Download queue: 2 queued');
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

        const modeB = bodyButtons().find(b => b.textContent?.includes('missing tracks'));
        expect(modeB).toBeDefined();
        modeB!.click();
        await flushPromises();
        expect(wrapper.exists()).toBe(true);
    });

    it('Modo A exports the complete M3U even when only the first page is loaded', async () => {
        vi.mocked(save).mockResolvedValue('/Users/tardis/Music/Chill Vibes.m3u');
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                return { ...mockPlaylistTracks, total: 501, has_more: true };
            }
            if (cmd === 'export_playlist_m3u') {
                return {
                    playlist_id: 1,
                    playlist_name: 'Chill Vibes',
                    total_tracks: 501,
                    verified_count: 500,
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

        const modeA = bodyButtons().find(b => b.textContent?.includes('Only the ones I already have'));
        expect(modeA).toBeDefined();
        modeA!.click();
        await flushPromises();

        // Save dialog was used, then backend command received playlistId + filePath
        expect(save).toHaveBeenCalled();
        const exportCall = invokeCalls.find(c => c.cmd === 'export_playlist_m3u');
        expect(exportCall).toBeDefined();
        expect(exportCall?.args).toEqual({ playlistId: 1, filePath: '/Users/tardis/Music/Chill Vibes.m3u' });
        expect(invokeCalls.find(c => c.cmd === 'add_batch_to_queue')).toBeUndefined();
        expect(invokeCalls.filter(c => c.cmd === 'get_local_playlist_tracks')).toHaveLength(1);

        // Honest counts banner + collapsible missing list
        expect(wrapper.text()).toContain('M3U exported · 500/501 tracks verified');
        const missingToggle = wrapper.findAll('button').find((b: any) => b.text().includes('Missing (1)'));
        expect(missingToggle).toBeDefined();
        await missingToggle!.trigger('click');
        await flushPromises();
        expect(wrapper.text()).toContain('Ghost Song');
        expect(wrapper.text()).toContain('File not found');
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

        const modeA = bodyButtons().find(b => b.textContent?.includes('Only the ones I already have'));
        expect(modeA).toBeDefined();
        modeA!.click();
        await flushPromises();

        expect(invokeCalls.find(c => c.cmd === 'export_playlist_m3u')).toBeUndefined();
    });
    it('imports a supported playlist URL through the matching persisted service command', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'import_spotify_playlists') return { imported: 2, skipped: 0 };
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        const importButton = wrapper.findAll('button').find((b: any) => b.text().includes('Import URL'));
        expect(importButton).toBeDefined();
        await importButton!.trigger('click');
        await flushPromises();

        const input = document.body.querySelector('input[type="url"]') as HTMLInputElement;
        input.value = 'https://open.spotify.com/playlist/abc123';
        input.dispatchEvent(new Event('input'));
        const confirm = bodyButtons().find(b => b.textContent?.trim() === 'Import');
        expect(confirm).toBeDefined();
        confirm!.click();
        await flushPromises();

        expect(invokeCalls.some(c => c.cmd === 'import_spotify_playlists')).toBe(true);
        expect(document.body.querySelector('input[type="url"]')).toBeNull();
    });

    it('persists playlist name and description edits through update_playlist', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return [{ ...mockPlaylists[0], description: 'Old description' }];
            if (cmd === 'get_local_playlist_tracks') return mockPlaylistTracks;
            if (cmd === 'update_playlist') return { ...mockPlaylists[0], description: args?.description ?? 'Old description', name: args?.name ?? 'Chill Vibes' };
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.group').find((el: any) => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();

        await wrapper.find('h1').trigger('click');
        const nameInput = wrapper.find('input.text-2xl');
        await nameInput.setValue('Renamed Playlist');
        await nameInput.trigger('keyup.enter');
        await flushPromises();

        const description = wrapper.findAll('p').find((el: any) => el.text().includes('Old description'))!;
        await description.trigger('click');
        const descriptionInput = wrapper.find('textarea');
        await descriptionInput.setValue('Updated description');
        await descriptionInput.trigger('blur');
        await flushPromises();

        expect(invokeCalls).toContainEqual({ cmd: 'update_playlist', args: { id: 1, name: 'Renamed Playlist' } });
        expect(invokeCalls).toContainEqual({ cmd: 'update_playlist', args: { id: 1, description: 'Updated description' } });
    });

    it('removes a track through remove_from_playlist', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return mockPlaylistTracks;
            if (cmd === 'remove_from_playlist') return 1;
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.group').find((el: any) => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();
        await wrapper.find('button[aria-label="Remove Chill Track 1 from playlist"]').trigger('click');
        await flushPromises();

        expect(invokeCalls).toContainEqual({ cmd: 'remove_from_playlist', args: { playlistId: 1, trackIds: [301] } });
    });

    it('quitar una repetición deja la otra ocurrencia en la lista', async () => {
        // PL02 (#155): la misma pista dos veces en posiciones distintas; quitar
        // una fila no puede borrar las demás repeticiones del mismo track id.
        const invokeCalls: { cmd: string; args: any }[] = [];
        let playlistFetches = 0;
        const duplicatePage = makePage([
            { id: 301, position: 1, title: 'Chill Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000 },
            { id: 301, position: 3, title: 'Chill Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000 }
        ]);
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                playlistFetches += 1;
                return playlistFetches === 1
                    ? duplicatePage
                    : makePage([{ id: 301, position: 1, title: 'Chill Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000 }]);
            }
            if (cmd === 'remove_from_playlist') return 1;
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.group').find((el: any) => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();
        expect(wrapper.findAll('.track-row')).toHaveLength(2);

        const removeButtons = wrapper.findAll('button[aria-label="Remove Chill Track 1 from playlist"]');
        await removeButtons[1].trigger('click');
        await flushPromises();

        expect(invokeCalls).toContainEqual({ cmd: 'remove_from_playlist', args: { playlistId: 1, trackIds: [301] } });
        expect(wrapper.findAll('.track-row')).toHaveLength(1);
    });

    it('keeps the import dialog open for unsupported URLs without invoking an import', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('button').find((b: any) => b.text().includes('Import URL'))!.trigger('click');
        const input = document.body.querySelector('input[type="url"]') as HTMLInputElement;
        input.value = 'https://example.com/playlist/abc';
        input.dispatchEvent(new Event('input'));
        bodyButtons().find(b => b.textContent?.trim() === 'Import')!.click();
        await flushPromises();

        expect(document.body.querySelector('input[type="url"]')).not.toBeNull();
        expect(invokeCalls.some(c => c.cmd.startsWith('import_'))).toBe(false);
    });

    it('imports playlist URL without protocol prefix (e.g. open.spotify.com/...)', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'import_spotify_playlists') return { success: true, imported: 1, errors: [] };
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('button').find((b: any) => b.text().includes('Import URL'))!.trigger('click');
        const input = document.body.querySelector('input[type="url"]') as HTMLInputElement;
        input.value = 'open.spotify.com/playlist/xyz789';
        input.dispatchEvent(new Event('input'));
        bodyButtons().find(b => b.textContent?.trim() === 'Import')!.click();
        await flushPromises();

        expect(invokeCalls.some(c => c.cmd === 'import_spotify_playlists')).toBe(true);
    });

    it('allows deleting smart playlists', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return [
                { id: 99, name: 'My Smart Playlist', track_count: 5, is_smart: true, rules_json: '[]' }
            ];
            if (cmd === 'get_local_playlist_tracks') return makePage([]);
            if (cmd === 'delete_playlist') return null;
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        // Select the smart playlist
        const smartItem = wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('My Smart Playlist'));
        expect(smartItem).toBeDefined();
        await smartItem!.trigger('click');
        await flushPromises();

        // The delete button in header should be present
        const deleteButton = wrapper.find('button[aria-label="Delete playlist"]');
        expect(deleteButton.exists()).toBe(true);
        await deleteButton.trigger('click');
        await flushPromises();

        const { resolveDialog } = useDialog();
        resolveDialog('confirm');
        await flushPromises();

        expect(invokeCalls).toContainEqual({ cmd: 'delete_playlist', args: { id: 99 } });
    });

    it('PL03: loads only the first 100-track page and virtualizes rendered rows', async () => {
        const tracks = Array.from({ length: 1200 }, (_, index) => ({
            id: index + 1,
            position: index + 1,
            title: `Track ${index + 1}`,
            artist_name: 'Artist',
            album_name: 'Album',
            duration_ms: 180_000,
        }));
        const calls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                const offset = Number(args?.offset ?? 0);
                const limit = Number(args?.limit ?? 0);
                const pageTracks = tracks.slice(offset, offset + limit);
                return {
                    tracks: pageTracks,
                    total: tracks.length,
                    offset,
                    limit,
                    has_more: offset + pageTracks.length < tracks.length,
                };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();

        expect(calls.filter(c => c.cmd === 'get_local_playlist_tracks')).toEqual([
            { cmd: 'get_local_playlist_tracks', args: { playlistId: 1, offset: 0, limit: 100 } },
        ]);
        expect(wrapper.findAll('.track-row').length).toBeGreaterThan(0);
        expect(wrapper.findAll('.track-row').length).toBeLessThan(100);
        expect(wrapper.text()).toContain('Track 1');
        expect(wrapper.find('[data-testid="playlist-tracks-truncated"]').exists()).toBe(false);
    });

    it('PL03: appends pages on scroll and can reach the final track beyond 500', async () => {
        const tracks = Array.from({ length: 750 }, (_, index) => ({
            id: index === 99 ? 7 : index + 1,
            position: index + 1,
            title: `Track ${index + 1}`,
            artist_name: 'Artist',
            album_name: 'Album',
            duration_ms: 180_000,
        }));
        const offsets: number[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                const offset = Number(args?.offset ?? 0);
                const limit = Number(args?.limit ?? 0);
                offsets.push(offset);
                const pageTracks = tracks.slice(offset, offset + limit);
                return {
                    tracks: pageTracks,
                    total: tracks.length,
                    offset,
                    limit,
                    has_more: offset + pageTracks.length < tracks.length,
                };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();
        expect(offsets).toEqual([0]);

        for (const loadedCount of [100, 200, 300, 400, 500, 600, 750]) {
            const scroller = wrapper.get('[data-testid="playlist-tracks-scroller"]');
            const element = scroller.element as HTMLElement;
            Object.defineProperty(element, 'clientHeight', { value: 600, configurable: true });
            Object.defineProperty(element, 'scrollHeight', { value: loadedCount * 56 + 600, configurable: true });
            Object.defineProperty(element, 'scrollTop', { value: loadedCount * 56, writable: true, configurable: true });
            await scroller.trigger('scroll');
            await flushPromises();
        }

        expect(offsets).toEqual([0, 100, 200, 300, 400, 500, 600, 700]);
        expect((wrapper.vm as any).$.setupState.playlistTracks).toHaveLength(750);
        expect(wrapper.findAll('.track-row').length).toBeLessThan(750);
        expect(wrapper.text()).toContain('Track 750');
        expect(wrapper.text()).toContain('Showing all 750 tracks');
    });

    it('PL03: keeps loaded rows and offers retry when a later page fails', async () => {
        let failSecondPage = true;
        const offsets: number[] = [];
        mockInvoke((cmd, args) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                const offset = Number(args?.offset ?? 0);
                offsets.push(offset);
                if (offset === 100 && failSecondPage) throw new Error('page two is unavailable');
                const pageTracks = Array.from({ length: 100 }, (_, index) => ({
                    id: offset + index + 1,
                    position: offset + index + 1,
                    title: `Track ${offset + index + 1}`,
                    artist_name: 'Artist',
                    album_name: 'Album',
                    duration_ms: 180_000,
                }));
                return {
                    tracks: pageTracks,
                    total: 200,
                    offset,
                    limit: 100,
                    has_more: offset === 0,
                };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();

        const scroller = wrapper.get('[data-testid="playlist-tracks-scroller"]');
        const element = scroller.element as HTMLElement;
        Object.defineProperty(element, 'clientHeight', { value: 600, configurable: true });
        Object.defineProperty(element, 'scrollHeight', { value: 6200, configurable: true });
        Object.defineProperty(element, 'scrollTop', { value: 5600, writable: true, configurable: true });
        await scroller.trigger('scroll');
        await flushPromises();

        expect(wrapper.find('[data-testid="playlist-tracks-load-more-error"]').text()).toContain('page two is unavailable');
        expect(wrapper.find('[data-testid="playlist-tracks-error"]').exists()).toBe(false);
        expect((wrapper.vm as any).$.setupState.playlistTracks).toHaveLength(100);

        failSecondPage = false;
        await wrapper.find('[data-testid="playlist-tracks-load-more-error"] button').trigger('click');
        await flushPromises();

        expect(offsets).toEqual([0, 100, 100]);
        expect((wrapper.vm as any).$.setupState.playlistTracks).toHaveLength(200);
        expect(wrapper.find('[data-testid="playlist-tracks-load-more-error"]').exists()).toBe(false);
        expect(wrapper.text()).toContain('Showing all 200 tracks');
    });

    it('PL03: discards a late page from the previous playlist and keeps the new one loadable', async () => {
        let resolveOldPage!: (value: any) => void;
        const calls: { cmd: string; args: any }[] = [];
        const playlists = [
            { id: 1, name: 'First Playlist', track_count: 200 },
            { id: 2, name: 'Second Playlist', track_count: 200 },
        ];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'get_playlists') return playlists;
            if (cmd === 'get_local_playlist_tracks') {
                const playlistId = Number(args?.playlistId);
                const offset = Number(args?.offset ?? 0);
                if (playlistId === 1 && offset === 100) {
                    return new Promise(resolve => { resolveOldPage = resolve; });
                }
                const baseId = playlistId === 1 ? 1000 : 2000;
                const tracks = Array.from({ length: 100 }, (_, index) => ({
                    id: baseId + offset + index + 1,
                    position: offset + index + 1,
                    title: `Playlist ${playlistId} Track ${offset + index + 1}`,
                    artist_name: 'Artist',
                    album_name: 'Album',
                    duration_ms: 180_000,
                }));
                return { tracks, total: 200, offset, limit: 100, has_more: offset === 0 };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('First Playlist'))!.trigger('click');
        await flushPromises();

        const scrollToEnd = async (loadedCount: number) => {
            const scroller = wrapper.get('[data-testid="playlist-tracks-scroller"]');
            const element = scroller.element as HTMLElement;
            Object.defineProperty(element, 'clientHeight', { value: 600, configurable: true });
            Object.defineProperty(element, 'scrollHeight', { value: loadedCount * 56 + 600, configurable: true });
            Object.defineProperty(element, 'scrollTop', { value: loadedCount * 56, writable: true, configurable: true });
            await scroller.trigger('scroll');
            await flushPromises();
        };

        await scrollToEnd(100);
        await wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Second Playlist'))!.trigger('click');
        await flushPromises();

        resolveOldPage({
            tracks: [{ id: 1101, position: 101, title: 'Stale Track', duration_ms: 1000 }],
            total: 200,
            offset: 100,
            limit: 100,
            has_more: false,
        });
        await flushPromises();

        const state = (wrapper.vm as any).$.setupState;
        expect(state.playlistTracks).toHaveLength(100);
        expect(state.playlistTracks[0].title).toBe('Playlist 2 Track 1');
        expect(state.isLoadingMore).toBe(false);

        await scrollToEnd(100);
        expect(calls).toContainEqual({ cmd: 'get_local_playlist_tracks', args: { playlistId: 2, offset: 100, limit: 100 } });
        expect(state.playlistTracks).toHaveLength(200);
    });

    it('PL03: deleting a playlist invalidates its pending track response', async () => {
        let resolvePage!: (value: any) => void;
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                return new Promise(resolve => { resolvePage = resolve; });
            }
            if (cmd === 'delete_playlist') return null;
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        await wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Chill Vibes'))!.trigger('click');
        await flushPromises();
        expect(wrapper.find('[data-testid="playlist-tracks-loading"]').exists()).toBe(true);

        await wrapper.find('button[aria-label="Delete playlist"]').trigger('click');
        await flushPromises();
        const { resolveDialog } = useDialog();
        resolveDialog('confirm');
        await flushPromises();

        resolvePage({
            tracks: [{ id: 301, position: 1, title: 'Deleted playlist track', duration_ms: 1000 }],
            total: 1,
            offset: 0,
            limit: 100,
            has_more: false,
        });
        await flushPromises();

        const state = (wrapper.vm as any).$.setupState;
        expect(state.selectedPlaylist).toBeNull();
        expect(state.playlistTracks).toHaveLength(0);
        expect(state.isLoading).toBe(false);
    });

    it('does not delete when the confirmation is cancelled', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return makePage([]);
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        const playlistItem = wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Chill Vibes'));
        expect(playlistItem).toBeDefined();
        await playlistItem!.trigger('click');
        await flushPromises();

        const deleteButton = wrapper.find('button[aria-label="Delete playlist"]');
        expect(deleteButton.exists()).toBe(true);
        await deleteButton.trigger('click');
        await flushPromises();

        // Cancelar el diálogo R13: la playlist se queda y no hay IPC de borrado.
        const { resolveDialog } = useDialog();
        resolveDialog('cancel');
        await flushPromises();

        expect(invokeCalls.some(c => c.cmd === 'delete_playlist')).toBe(false);
        expect(wrapper.text()).toContain('Chill Vibes');
    });

});
