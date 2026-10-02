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
export async function getPlaylistTracks(
    playlistId: number,
    offset?: number,
    limit?: number
): Promise<PlaylistTrack[]> {
    const raw = await invokeCommand<unknown>('get_playlist_tracks', {
        playlistId,
        offset,
        limit,
    });
    // Support direct array or paginated LibraryPage response
    if (Array.isArray(raw)) {
        return raw as PlaylistTrack[];
    }
    if (raw && typeof raw === 'object' && 'tracks' in raw && Array.isArray((raw as { tracks: unknown }).tracks)) {
        const page = raw as { tracks: Array<Record<string, unknown>> };
        return page.tracks.map((t, idx) => ({
            id: asNumber(t.id),
            track_id: asNumber(t.track_id, asNumber(t.id)),
            position: asNumber(t.position, asNumber(t.track_number, idx + 1)),
            title: asString(t.title),
            artist_name: asString(t.artist_name, 'Unknown Artist'),
            album_name: typeof t.album_name === 'string' ? t.album_name : null,
        }));
    }
    return [];
}

/**
 * Search playlists by name
 */
export async function searchPlaylists(query: string): Promise<Playlist[]> {
    const playlists = await getPlaylists();
    const q = (query || '').trim().toLowerCase();
    if (!q) return playlists;
    return playlists.filter(p => {
        const nameMatch = typeof p?.name === 'string' && p.name.toLowerCase().includes(q);
        const descMatch = typeof p?.description === 'string' && p.description.toLowerCase().includes(q);
        return nameMatch || descMatch;
    });
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
    accountId?: number;
}): Promise<Playlist> {
    const raw = await invokeCommand<unknown>('create_playlist', {
        accountId: params.accountId ?? 1,
        name: params.name,
        description: params.description ?? null,
    });

    let id: number | null = null;
    if (typeof raw === 'number') {
        id = raw;
    } else if (raw && typeof raw === 'object') {
        const rec = raw as Record<string, unknown>;
        if (typeof rec.id === 'number') {
            id = rec.id;
        } else if (typeof rec.id === 'string') {
            const parsed = parseInt(rec.id, 10);
            if (!isNaN(parsed)) id = parsed;
        }
    }

    if (id !== null) {
        try {
            const playlist = await getPlaylist(id);
            if (playlist) {
                return playlist;
            }
        } catch {
            // fallback below
        }
    }

    const rec = asRecord(raw);
    return {
        id: id ?? asNumber(rec?.id, 0),
        name: asString(rec?.name, params.name),
        description: rec?.description !== undefined ? (rec.description as string | null) : (params.description ?? null),
        owner_name: rec?.owner_name !== undefined ? (rec.owner_name as string | null) : null,
        track_count: asNumber(rec?.track_count, 0),
        image_url: rec?.image_url !== undefined ? (rec.image_url as string | null) : null,
        service_name: asString(rec?.service_name, 'local'),
    } as unknown as Playlist;
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
    await invokeCommand<unknown>('add_to_playlist', {
        playlistId,
        trackIds,
    });
    return trackIds.length;
}

/**
 * Remove tracks from a playlist
 */
export async function removeTracksFromPlaylist(playlistId: number, trackIds: number[]): Promise<number> {
    const res = await invokeCommand<unknown>('remove_from_playlist', {
        playlistId,
        trackIds,
    });
    return typeof res === 'number' ? res : trackIds.length;
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
    const s = service.toLowerCase();
    let raw: unknown;
    if (s === 'spotify') {
        raw = await invokeCommand<unknown>('import_spotify_playlists');
    } else if (s === 'qobuz') {
        raw = await invokeCommand<unknown>('import_qobuz_playlists');
    } else if (s === 'tidal') {
        raw = await invokeCommand<unknown>('import_tidal_library');
    } else if (s === 'deezer') {
        raw = await invokeCommand<unknown>('import_deezer_library');
    } else {
        throw new Error(`Unsupported playlist service: ${service}`);
    }
    return normalizeImportResult(raw);
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

/**
 * Calculate dynamic count of tracks matching smart playlist rules
 */
export async function previewSmartPlaylistCount(rulesJson: string): Promise<number> {
    const raw = await invokeCommand<unknown>('preview_smart_playlist_count', {
        rulesJson,
    });
    return asNumber(raw, 0);
}

/**
 * Create a smart playlist, evaluate its rules against library tracks, and persist it
 */
export async function createSmartPlaylist(params: {
    name: string;
    rulesJson: string;
    accountId?: number;
}): Promise<Playlist> {
    return invokeCommand<Playlist>('create_smart_playlist', {
        name: params.name,
        rulesJson: params.rulesJson,
        accountId: params.accountId ?? 1,
    });
}

// ==============================================
// REMOTE PLAYLIST BRIDGE (IN-5: exposed backend capabilities)
// ==============================================

/** Generic bridge envelope returned by playlist_bridge.py commands. */
export interface PlaylistBridgeResult {
    success: boolean;
    data?: Record<string, unknown> | null;
    error?: string | null;
}

/**
 * Fetch the tracks of a remote service playlist (playlist_bridge.py `get`).
 * Exposes `fetch_remote_playlist_tracks`, the only way to inspect a remote
 * playlist's contents before importing it.
 */
export async function fetchRemotePlaylistTracks(
    service: string,
    playlistId: string
): Promise<PlaylistBridgeResult> {
    const raw = await invokeCommand<unknown>('fetch_remote_playlist_tracks', { service, playlistId });
    const rec = asRecord(raw);
    return {
        success: rec?.success === true,
        data: asRecord(rec?.data ?? null),
        error: typeof rec?.error === 'string' ? rec.error : undefined,
    };
}

/**
 * Classify the entries of a playlist file by ISRC availability for a transfer to
 * `targetService` (playlist_bridge.py `match`).
 *
 * The target service catalog is NOT queried: the bridge reports which entries carry
 * an ISRC. Real ISRC matching happens against the local library in Rust
 * (commands::playlists::match_entry_to_track).
 */
export async function matchPlaylistToService(
    playlistFile: string,
    targetService: string
): Promise<PlaylistBridgeResult> {
    const raw = await invokeCommand<unknown>('match_playlist_to_service', {
        playlistFile,
        targetService
    });
    const rec = asRecord(raw);
    return {
        success: rec?.success === true,
        data: asRecord(rec?.data ?? null),
        error: typeof rec?.error === 'string' ? rec.error : undefined,
    };
}

/** TASK-107 sanitization statistics (camelCase, matches Rust PlaylistSanitizationStats). */
export interface PlaylistSanitizationStats {
    duplicate_tracks_purged: number;
    playlists_recompacted: number;
    track_counts_updated: number;
    playlist_names_disambiguated: number;
}

/**
 * Sanitize all playlists in the library (purge intra-playlist duplicates,
 * recompact positions, sync track counts, disambiguate names).
 */
export async function sanitizePlaylists(): Promise<PlaylistSanitizationStats> {
    const raw = await invokeCommand<unknown>('sanitize_playlists');
    const rec = asRecord(raw);
    return {
        duplicate_tracks_purged: asNumber(rec?.duplicate_tracks_purged),
        playlists_recompacted: asNumber(rec?.playlists_recompacted),
        track_counts_updated: asNumber(rec?.track_counts_updated),
        playlist_names_disambiguated: asNumber(rec?.playlist_names_disambiguated),
    };
}

// ==============================================
// FE-6: IMPORT DE PLAYLISTS DESDE ARCHIVO
// ==============================================

/** Pista del archivo que no se pudo enlazar (camelCase, matches Rust UnmatchedImportedEntry). */
export interface UnmatchedImportedEntry {
    title: string;
    artist: string | null;
}

/** Resultado del import desde archivo (camelCase, matches Rust ImportPlaylistFromFileResult). */
export interface ImportPlaylistFromFileResult {
    playlistId: number;
    playlistName: string;
    /** Entradas totales parseadas del archivo. */
    totalEntries: number;
    /** Entradas enlazadas a pistas existentes. */
    matchedCount: number;
    unmatched: UnmatchedImportedEntry[];
}

/**
 * FE-6: import a playlist from a file's content (.m3u/.m3u8/.csv/.txt).
 *
 * The frontend reads the file via the File API (no backend filesystem scope
 * needed) and sends name + content; the backend parses it, matches each entry
 * against the local library (ISRC, local file path, title+artist, title),
 * creates the playlist and reports unmatched entries honestly.
 */
export async function importPlaylistFromFile(params: {
    fileName: string;
    content: string;
    name?: string;
    accountId?: number;
}): Promise<ImportPlaylistFromFileResult> {
    const raw = await invokeCommand<unknown>('import_playlist_from_file', {
        fileName: params.fileName,
        content: params.content,
        name: params.name ?? null,
        accountId: params.accountId ?? null,
    });
    const rec = asRecord(raw);
    return {
        playlistId: asNumber(rec?.playlistId),
        playlistName: asString(rec?.playlistName),
        totalEntries: asNumber(rec?.totalEntries),
        matchedCount: asNumber(rec?.matchedCount),
        unmatched: asArray<UnmatchedImportedEntry>(rec?.unmatched),
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
    syncPlaylists,
    previewSmartPlaylistCount,
    createSmartPlaylist,
    fetchRemotePlaylistTracks,
    matchPlaylistToService,
    sanitizePlaylists,
    importPlaylistFromFile,
};

