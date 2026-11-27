/**
 * playlists.spec.ts
 * Regression tests: playlist lists and sync counters default safely.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getPlaylists,
    getPlaylistTracks,
    syncPlaylists,
    searchPlaylists,
    createPlaylist,
    addTracksToPlaylist,
    removeTracksFromPlaylist,
    reorderPlaylistTracks,
    importPlaylists,
    previewSmartPlaylistCount,
    createSmartPlaylist,
    getPlaylistConflictState,
    setPlaylistConflictMode,
    listPlaylistConflictReviews,
    resolvePlaylistConflictReview,
} from '@/api/playlists';
import { mockInvoke, resetMocks } from '../setup';

describe('playlists_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('coerces non-array list responses to empty arrays', async () => {
        mockInvoke(() => null);
        expect(await getPlaylists()).toEqual([]);
        expect(await getPlaylistTracks(1)).toEqual([]);
    });

    it('defaults SyncPlaylistsResult counters and message', async () => {
        mockInvoke((cmd) => (cmd === 'sync_playlists' ? { playlists_synced: 4, tracks_linked: 40, message: 'ok' } : null));

        const res = await syncPlaylists('tidal');
        expect(res.playlists_synced).toBe(4);
        expect(res.tracks_linked).toBe(40);
        expect(res.message).toBe('ok');

        mockInvoke((cmd) => (cmd === 'sync_playlists' ? { playlists_synced: 2 } : null));
        const partial = await syncPlaylists();
        expect(partial.playlists_synced).toBe(2);
        expect(partial.tracks_linked).toBe(0);
        expect(partial.message).toBe('');
    });

    it('searchPlaylists filters playlists by query matching name or description', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') {
                return [
                    { id: 1, name: 'Rock Classics', description: 'Best 70s and 80s' },
                    { id: 2, name: 'Jazz Chill', description: 'Relaxing jazz vibes' },
                    { id: 3, name: 'Electronic', description: 'Classics and modern synth' },
                ];
            }
            return null;
        });

        const all = await searchPlaylists('');
        expect(all).toHaveLength(3);

        const rock = await searchPlaylists('rock');
        expect(rock).toHaveLength(1);
        expect(rock[0].name).toBe('Rock Classics');

        const classics = await searchPlaylists('classics');
        expect(classics).toHaveLength(2);
    });

    it('createPlaylist invokes create_playlist and retrieves resulting playlist', async () => {
        let createdArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'create_playlist') {
                createdArgs = args as Record<string, unknown>;
                return 42;
            }
            if (cmd === 'get_playlist') {
                return {
                    id: (args as { id: number }).id,
                    name: 'My New Playlist',
                    description: 'Test Desc',
                    track_count: 0,
                    owner_name: null,
                    image_url: null,
                    service_name: 'local',
                };
            }
            return null;
        });

        const playlist = await createPlaylist({ name: 'My New Playlist', description: 'Test Desc' });
        expect(createdArgs).toEqual({ accountId: 1, name: 'My New Playlist', description: 'Test Desc' });
        expect(playlist.id).toBe(42);
        expect(playlist.name).toBe('My New Playlist');
    });

    it('addTracksToPlaylist invokes add_to_playlist with camelCase params', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'add_to_playlist') {
                passedArgs = args as Record<string, unknown>;
                return 'Added 2 tracks';
            }
            return null;
        });

        const count = await addTracksToPlaylist(10, [101, 102]);
        expect(count).toBe(2);
        expect(passedArgs).toEqual({ playlistId: 10, trackIds: [101, 102] });
    });

    it('removeTracksFromPlaylist invokes remove_from_playlist with camelCase params', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'remove_from_playlist') {
                passedArgs = args as Record<string, unknown>;
                return 1;
            }
            return null;
        });

        const count = await removeTracksFromPlaylist(10, [101]);
        expect(count).toBe(1);
        expect(passedArgs).toEqual({ playlistId: 10, trackIds: [101] });
    });

    it('reorderPlaylistTracks passes positions with camelCase fields', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'reorder_playlist_tracks') {
                passedArgs = args as Record<string, unknown>;
                return null;
            }
            return null;
        });

        await reorderPlaylistTracks(5, [{ trackId: 1, newPosition: 2 }]);
        expect(passedArgs).toEqual({
            playlistId: 5,
            positions: [{ trackId: 1, newPosition: 2 }],
        });
    });

    it('importPlaylists routes every supported service to a registered command', async () => {
        const calledCmds: string[] = [];
        mockInvoke((cmd) => {
            calledCmds.push(cmd);
            return { imported: 5, skipped: 1, errors: [] };
        });

        const spotRes = await importPlaylists('spotify');
        expect(spotRes.imported).toBe(5);
        expect(calledCmds).toContain('import_spotify_playlists');

        const qobRes = await importPlaylists('QOBUZ');
        expect(qobRes.imported).toBe(5);
        expect(calledCmds).toContain('import_qobuz_playlists');

        const tidalRes = await importPlaylists('tidal');
        expect(tidalRes.imported).toBe(5);
        expect(calledCmds).toContain('import_tidal_library');

        const deezerRes = await importPlaylists('deezer');
        expect(deezerRes.imported).toBe(5);
        expect(calledCmds).toContain('import_deezer_library');
    });

    it('TASK-38: getPlaylistTracks passes playlistId, offset, limit and normalizes LibraryPage', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'get_playlist_tracks') {
                passedArgs = args as Record<string, unknown>;
                return {
                    tracks: [
                        {
                            id: 101,
                            title: 'Track A',
                            artist_name: 'Artist A',
                            album_name: 'Album A',
                            position: 1,
                            track_number: 1,
                        },
                        {
                            id: 102,
                            title: 'Track B',
                            artist_name: null,
                            album_name: null,
                            position: 2,
                            track_number: 2,
                        },
                    ],
                    total: 2,
                    offset: 0,
                    limit: 50,
                    has_more: false,
                };
            }
            return null;
        });

        const tracks = await getPlaylistTracks(7, 0, 50);
        expect(passedArgs).toEqual({ playlistId: 7, offset: 0, limit: 50 });
        expect(tracks).toHaveLength(2);
        expect(tracks[0]).toEqual({
            id: 101,
            track_id: 101,
            position: 1,
            title: 'Track A',
            artist_name: 'Artist A',
            album_name: 'Album A',
        });
        expect(tracks[1]).toEqual({
            id: 102,
            track_id: 102,
            position: 2,
            title: 'Track B',
            artist_name: 'Unknown Artist',
            album_name: null,
        });
    });

    it('TASK-38: getPlaylistTracks handles direct array payload', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlist_tracks') {
                return [
                    {
                        id: 201,
                        track_id: 201,
                        position: 1,
                        title: 'Direct Track',
                        artist_name: 'Direct Artist',
                        album_name: 'Direct Album',
                    },
                ];
            }
            return null;
        });

        const tracks = await getPlaylistTracks(12);
        expect(tracks).toHaveLength(1);
        expect(tracks[0].title).toBe('Direct Track');
    });

    it('TASK-21: previewSmartPlaylistCount invokes preview_smart_playlist_count with rulesJson', async () => {
        let capturedCmd: string | null = null;
        let capturedArgs: any = null;

        mockInvoke((cmd, args) => {
            capturedCmd = cmd;
            capturedArgs = args;
            if (cmd === 'preview_smart_playlist_count') {
                return 15;
            }
            return null;
        });

        const rulesJson = JSON.stringify([{ field: 'genre', operator: 'contains', value: 'Rock' }]);
        const count = await previewSmartPlaylistCount(rulesJson);
        expect(capturedCmd).toBe('preview_smart_playlist_count');
        expect(capturedArgs).toEqual({ rulesJson });
        expect(count).toBe(15);
    });

    it('TASK-21: createSmartPlaylist invokes create_smart_playlist and returns created playlist', async () => {
        let capturedCmd: string | null = null;
        let capturedArgs: any = null;

        mockInvoke((cmd, args) => {
            capturedCmd = cmd;
            capturedArgs = args;
            if (cmd === 'create_smart_playlist') {
                return {
                    id: 99,
                    name: '80s Rock',
                    description: null,
                    track_count: 15,
                    owner_name: null,
                    image_url: null,
                    service_name: null,
                    is_smart: true,
                    rules_json: (args as any)?.rulesJson,
                };
            }
            return null;
        });

        const rulesJson = JSON.stringify([{ field: 'genre', operator: 'contains', value: 'Rock' }]);
        const res = await createSmartPlaylist({
            name: '80s Rock',
            rulesJson,
            accountId: 1,
        });

        expect(capturedCmd).toBe('create_smart_playlist');
        expect(capturedArgs).toEqual({
            name: '80s Rock',
            rulesJson,
            accountId: 1,
        });
        expect(res.id).toBe(99);
        expect(res.name).toBe('80s Rock');
        expect(res.track_count).toBe(15);
        expect(res.is_smart).toBe(true);
    });
});

describe('playlist_conflict_api_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('getPlaylistConflictState normalizes the mode and pending reviews', async () => {
        let capturedArgs: any = null;
        mockInvoke((cmd, args) => {
            capturedArgs = args;
            if (cmd === 'get_playlist_conflict_state') {
                return {
                    playlist_id: 7,
                    conflict_mode: 'editable',
                    service_name: 'spotify',
                    is_remote: true,
                    requires_mode_choice: false,
                    pending_reviews: [
                        {
                            id: 3,
                            playlist_id: 7,
                            playlist_name: 'Road trip',
                            service_name: 'spotify',
                            conflict_kind: 'membership',
                            local_value: '[1,2]',
                            provider_value: '[1,3]',
                            status: 'needs_review',
                            created_at: '2026-11-27 10:00:00',
                            updated_at: '2026-11-27 10:00:00',
                            resolved_at: null,
                            local_track_ids: [1, 2],
                            provider_track_ids: [1, 3],
                        },
                    ],
                };
            }
            return null;
        });

        const state = await getPlaylistConflictState(7);
        expect(capturedArgs).toEqual({ playlistId: 7 });
        expect(state.conflict_mode).toBe('editable');
        expect(state.is_remote).toBe(true);
        expect(state.requires_mode_choice).toBe(false);
        expect(state.pending_reviews).toHaveLength(1);
        expect(state.pending_reviews[0].conflict_kind).toBe('membership');
        expect(state.pending_reviews[0].provider_track_ids).toEqual([1, 3]);
    });

    it('getPlaylistConflictState defaults a malformed payload safely', async () => {
        mockInvoke(() => null);
        const state = await getPlaylistConflictState(9);
        expect(state.playlist_id).toBe(9);
        expect(state.conflict_mode).toBe('gana_proveedor');
        expect(state.is_remote).toBe(false);
        expect(state.requires_mode_choice).toBe(false);
        expect(state.pending_reviews).toEqual([]);
    });

    it('setPlaylistConflictMode invokes the command with camelCase ids', async () => {
        let capturedArgs: any = null;
        mockInvoke((cmd, args) => {
            capturedArgs = args;
            if (cmd === 'set_playlist_conflict_mode') {
                return {
                    playlist_id: 4,
                    conflict_mode: (args as any).mode,
                    service_name: 'tidal',
                    is_remote: true,
                    requires_mode_choice: false,
                    pending_reviews: [],
                };
            }
            return null;
        });

        const state = await setPlaylistConflictMode(4, 'gana_local');
        expect(capturedArgs).toEqual({ playlistId: 4, mode: 'gana_local' });
        expect(state.conflict_mode).toBe('gana_local');
    });

    it('listPlaylistConflictReviews sends explicit filters and normalizes rows', async () => {
        let capturedArgs: any = null;
        mockInvoke((cmd, args) => {
            capturedArgs = args;
            if (cmd === 'list_playlist_conflict_reviews') {
                return [
                    {
                        id: 11,
                        playlist_id: 4,
                        playlist_name: 'Mix',
                        service_name: null,
                        conflict_kind: 'name',
                        local_value: 'Mío',
                        provider_value: 'Suyo',
                        status: 'needs_review',
                        created_at: 'now',
                        updated_at: 'now',
                        resolved_at: null,
                        local_track_ids: null,
                        provider_track_ids: null,
                    },
                ];
            }
            return null;
        });

        const reviews = await listPlaylistConflictReviews();
        expect(capturedArgs).toEqual({ playlistId: null, status: null });
        expect(reviews).toHaveLength(1);
        expect(reviews[0].local_value).toBe('Mío');
        expect(reviews[0].service_name).toBeNull();

        await listPlaylistConflictReviews({ playlistId: 4, status: 'needs_review' });
        expect(capturedArgs).toEqual({ playlistId: 4, status: 'needs_review' });
    });

    it('resolvePlaylistConflictReview invokes the explicit resolution', async () => {
        let capturedArgs: any = null;
        mockInvoke((cmd, args) => {
            capturedArgs = args;
            if (cmd === 'resolve_playlist_conflict_review') {
                return {
                    id: 11,
                    playlist_id: 4,
                    playlist_name: 'Mix',
                    service_name: null,
                    conflict_kind: 'name',
                    local_value: 'Mío',
                    provider_value: 'Suyo',
                    status: 'resolved_provider',
                    created_at: 'now',
                    updated_at: 'now',
                    resolved_at: 'now',
                    local_track_ids: null,
                    provider_track_ids: null,
                };
            }
            return null;
        });

        const review = await resolvePlaylistConflictReview(11, 'provider');
        expect(capturedArgs).toEqual({ reviewId: 11, resolution: 'provider' });
        expect(review.status).toBe('resolved_provider');
        expect(review.resolved_at).toBe('now');
    });
});
