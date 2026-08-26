/**
 * Playlists API
 * 
 * Tauri commands for playlist management.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asRecord, asString } from './normalize';
import type { Playlist, PlaylistTrack, ImportResult } from './types';

/**
 * Normalize an ImportResult so counts default to 0 and errors is always an array.
 */
function normalizeImportResult(raw: unknown): ImportResult {
    const rec = asRecord(raw);
    return {
        imported: asNumber(rec?.imported),
        skipped: asNumber(rec?.skipped),
        errors: Array.isArray(rec?.errors) ? (rec.errors as string[]) : [],
    };
}

function normalizePlaylistList(raw: unknown): Playlist[] {
    return asArray<Playlist>(raw);
}

// ==============================================
// PLAYLIST QUERIES
// ==============================================

/**
 * Get all playlists for the current user
 */
export async function getPlaylists(): Promise<Playlist[]> {
    return normalizePlaylistList(await invokeCommand<unknown>('get_playlists'));
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
    const raw = await invokeCommand<unknown>('get_playlist_tracks', { playlistId });
    return asArray<PlaylistTrack>(raw);
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
    return normalizeImportResult(await invokeCommand<unknown>('import_playlists', { service }));
}

/**
 * Export playlist to a service
 */
export async function exportPlaylist(playlistId: number, service: string): Promise<{ success: boolean; error?: string }> {
    const raw = await invokeCommand<unknown>('export_playlist', {
        playlistId,
        service
    });
    const rec = asRecord(raw);
    return {
        success: rec?.success === true,
        error: typeof rec?.error === 'string' ? rec.error : undefined,
    };
}

export interface PlaylistServiceSummary {
    service: string;
    playlists: number;
    tracks_linked: number;
    last_synced?: string | null;
}

export interface SyncPlaylistsResult {
    playlists_synced: number;
    tracks_linked: number;
    message: string;
    /** S189-F2-5: per-service breakdown of the local linked catalog. */
    services?: PlaylistServiceSummary[];
}

/**
 * Sync playlist with source service
 */
export async function syncPlaylist(playlistId: number): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('sync_playlist', { playlistId }));
}

/**
 * Sync playlists across connected services into SQLite
 */
export async function syncPlaylists(service?: string): Promise<SyncPlaylistsResult> {
    const raw = await invokeCommand<unknown>('sync_playlists', { service });
    const rec = asRecord(raw);
    return {
        playlists_synced: asNumber(rec?.playlists_synced),
        tracks_linked: asNumber(rec?.tracks_linked),
        message: asString(rec?.message),
    };
}

// ==============================================
// S201 - EXPORT M3U «SOLO LAS QUE YA TENGO» (Modo A)
// ==============================================

/** Pista que no pudo verificarse en disco (lista de faltantes). */
export interface MissingPlaylistFile {
    track_id: number;
    title: string;
    artist_name?: string | null;
    /** 'sin_archivo_local' | 'archivo_no_encontrado' */
    reason: string;
}

/** Contrato del comando export_playlist_m3u (conteos reales, sin inventar). */
export interface PlaylistM3uExportResult {
    playlist_id: number;
    playlist_name: string;
    total_tracks: number;
    verified_count: number;
    missing_count: number;
    missing_tracks: MissingPlaylistFile[];
    file_path?: string | null;
    bytes_written?: number | null;
    m3u_content: string;
}

/**
 * S201 Modo A «Solo las que ya tengo»: verifica con stat() real los archivos
 * locales de las pistas de la playlist y exporta un .m3u con SOLO las
 * verificadas. Con filePath escribe el archivo en backend; sin filePath
 * devuelve solo contenido y conteos (dry-run).
 */
export async function exportPlaylistM3u(playlistId: number, filePath?: string | null): Promise<PlaylistM3uExportResult> {
    const raw = await invokeCommand<unknown>('export_playlist_m3u', {
        playlistId,
        filePath: filePath ?? null,
    });
    const rec = asRecord(raw);
    return {
        playlist_id: asNumber(rec?.playlist_id, playlistId),
        playlist_name: asString(rec?.playlist_name),
        total_tracks: asNumber(rec?.total_tracks),
        verified_count: asNumber(rec?.verified_count),
        missing_count: asNumber(rec?.missing_count),
        missing_tracks: asArray<MissingPlaylistFile>(rec?.missing_tracks),
        file_path: typeof rec?.file_path === 'string' ? rec.file_path : null,
        bytes_written: typeof rec?.bytes_written === 'number' ? rec.bytes_written : null,
        m3u_content: asString(rec?.m3u_content),
    };
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
    exportPlaylistM3u,
    syncPlaylist,
    syncPlaylists,
};

