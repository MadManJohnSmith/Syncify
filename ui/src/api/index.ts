/**
 * Syncify API Layer
 * 
 * Barrel export for all API modules.
 */

// Base utilities
export * from './tauri';
export * from './types';

// API modules
export * from './library';
export * from './queue';
export * from './accounts';
export * from './settings';
export * from './dashboard';
export * from './lyrics';
export * from './metadata';
export * from './migration';
export * from './notifications';
export * from './logs';

// Playlists has collisions with library: getPlaylists, createPlaylist
// We export everything else from playlists, and the colliding ones with aliases
export {
    playlistsApi,
    getPlaylist,
    getPlaylistTracks,
    searchPlaylists,
    updatePlaylist,
    deletePlaylist,
    addTracksToPlaylist,
    removeTracksFromPlaylist,
    reorderPlaylistTracks,
    importPlaylists,
    exportPlaylist,
    syncPlaylist,
    getPlaylists as getPlaylistsFull,
    createPlaylist as createPlaylistFull
} from './playlists';

// Re-export namespaces
export { libraryApi } from './library';
export { queueApi } from './queue';
export { accountsApi } from './accounts';
export { settingsApi } from './settings';
export { dashboardApi } from './dashboard';
export { lyricsApi } from './lyrics';
export { metadataApi } from './metadata';
export { migrationApi } from './migration';
export { notificationsApi } from './notifications';
export { logsApi } from './logs';
export { toolsApi } from './tools';

