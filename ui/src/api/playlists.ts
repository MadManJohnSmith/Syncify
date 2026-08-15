/**
 * Playlists API
 * 
 * Tauri commands for playlist management.
 */

import { invokeCommand } from './tauri';
import type { Playlist, PlaylistTrack, ImportResult } from './types';

// ==============================================
// PLAYLIST QUERIES
// ==============================================

/**
 * Get all playlists for the current user
 */
export async function getPlaylists(): Promise<Playlist[]> {
    return invokeCommand<Playlist[]>('get_playlists');
}

/**
 * Get a single playlist by ID
 */
export async function getPlaylist(id: number): Promise<Playlist> {
    return invokeCommand<Playlist>('get_playlist', { id });
}

/**
 * Get tracks in a playlist
 */
export async function getPlaylistTracks(playlistId: number): Promise<PlaylistTrack[]> {
    return invokeCommand<PlaylistTrack[]>('get_playlist_tracks', { playlistId });
}

/**
 * Search playlists by name
 */
export async function searchPlaylists(query: string): Promise<Playlist[]> {
    return invokeCommand<Playlist[]>('search_playlists', { query });
}

// ==============================================
// PLAYLIST MUTATIONS
// ==============================================

/**
 * Create a new playlist
 */
export async function createPlaylist(params: {
    name: string;
    description?: string;
    is_public?: boolean;
}): Promise<Playlist> {
    return invokeCommand<Playlist>('create_playlist', params);
}

/**
 * Update a playlist
 */
export async function updatePlaylist(params: {
    id: number;
    name?: string;
    description?: string;
    is_public?: boolean;
}): Promise<Playlist> {
    return invokeCommand<Playlist>('update_playlist', params);
}

/**
 * Delete a playlist
 */
export async function deletePlaylist(id: number): Promise<void> {
    return invokeCommand<void>('delete_playlist', { id });
}

/**
 * Add tracks to a playlist
 */
export async function addTracksToPlaylist(playlistId: number, trackIds: number[]): Promise<number> {
    return invokeCommand<number>('add_tracks_to_playlist', {
        playlistId,
        trackIds
    });
}

/**
 * Remove tracks from a playlist
 */
export async function removeTracksFromPlaylist(playlistId: number, trackIds: number[]): Promise<number> {
    return invokeCommand<number>('remove_tracks_from_playlist', {
        playlistId,
        trackIds
    });
}

/**
 * Reorder tracks in a playlist
 */
export async function reorderPlaylistTracks(playlistId: number, positions: { trackId: number; newPosition: number }[]): Promise<void> {
    return invokeCommand<void>('reorder_playlist_tracks', {
        playlistId,
        positions
    });
}

// ==============================================
// PLAYLIST SYNC
// ==============================================

/**
 * Import playlists from a service
 */
export async function importPlaylists(service: string): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_playlists', { service });
}

/**
 * Export playlist to a service
 */
export async function exportPlaylist(playlistId: number, service: string): Promise<{ success: boolean; error?: string }> {
    return invokeCommand<{ success: boolean; error?: string }>('export_playlist', {
        playlistId,
        service
    });
}

export interface SyncPlaylistsResult {
    playlists_synced: number;
    tracks_linked: number;
    message: string;
}

/**
 * Sync playlist with source service
 */
export async function syncPlaylist(playlistId: number): Promise<ImportResult> {
    return invokeCommand<ImportResult>('sync_playlist', { playlistId });
}

/**
 * Sync playlists across connected services into SQLite
 */
export async function syncPlaylists(service?: string): Promise<SyncPlaylistsResult> {
    return invokeCommand<SyncPlaylistsResult>('sync_playlists', { service });
}

// Export as namespace
export const playlistsApi = {
    getPlaylists,
    getPlaylist,
    getPlaylistTracks,
    searchPlaylists,
    createPlaylist,
    updatePlaylist,
    deletePlaylist,
    addTracksToPlaylist,
    removeTracksFromPlaylist,
    reorderPlaylistTracks,
    importPlaylists,
    exportPlaylist,
    syncPlaylist,
    syncPlaylists,
};

