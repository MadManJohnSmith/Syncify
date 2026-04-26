/**
 * Accounts API
 * 
 * Tauri commands for service accounts and authentication.
 */

import { invokeCommand } from './tauri';
import type {
    Service,
    Account,
    ServiceStatus,
    SessionStatus,
    AuthResult,
    ImportResult,
    UrlParseResult
} from './types';

// ==============================================
// SERVICES
// ==============================================

/**
 * Get all available services
 */
export async function getServices(): Promise<Service[]> {
    return invokeCommand<Service[]>('get_services');
}

/**
 * Get service statuses (legacy)
 */
export async function getServiceStatuses(): Promise<ServiceStatus[]> {
    return invokeCommand<ServiceStatus[]>('get_service_statuses');
}

// ==============================================
// ACCOUNTS
// ==============================================

/**
 * Get all connected accounts
 */
export async function getAccounts(): Promise<Account[]> {
    return invokeCommand<Account[]>('get_accounts');
}

/**
 * Add a new account
 */
export async function addAccount(params: {
    service_id: number;
    display_name: string;
    email?: string;
    credentials_json: string;
}): Promise<number> {
    return invokeCommand<number>('add_account', params);
}

/**
 * Remove an account
 */
export async function removeAccount(id: number): Promise<void> {
    return invokeCommand<void>('remove_account', { accountId: id });
}

/**
 * Toggle account active status
 */
export async function toggleAccountActive(id: number, isActive: boolean): Promise<void> {
    return invokeCommand<void>('toggle_account_active', { accountId: id, isActive });
}

/**
 * Update account sync time
 */
export async function updateAccountSyncTime(id: number): Promise<void> {
    return invokeCommand<void>('update_account_sync_time', { accountId: id });
}

// ==============================================
// AUTHENTICATION
// ==============================================

/**
 * Start auth flow and save credentials
 */
export async function startAuthAndSave(service: string): Promise<AuthResult> {
    return invokeCommand<AuthResult>('start_auth_and_save', { service });
}

/**
 * Start auth flow (without saving)
 */
export async function startAuth(service: string, action: string): Promise<AuthResult> {
    return invokeCommand<AuthResult>('start_auth', { service, action });
}

/**
 * Get auth status for a service
 */
export async function getAuthStatus(service: string): Promise<AuthResult> {
    return invokeCommand<AuthResult>('get_auth_status', { service });
}

/**
 * Logout from a service
 */
export async function logoutService(service: string): Promise<AuthResult> {
    return invokeCommand<AuthResult>('logout_service', { service });
}

/**
 * Validate all connected sessions
 */
export async function validateAllSessions(): Promise<SessionStatus[]> {
    return invokeCommand<SessionStatus[]>('validate_all_sessions');
}

// ==============================================
// SPOTIFY AUTH
// ==============================================

/**
 * Start Spotify OAuth flow
 */
export async function startSpotifyAuth(): Promise<string> {
    return invokeCommand<string>('start_spotify_auth');
}

/**
 * Handle Spotify OAuth callback
 */
export async function spotifyAuthCallback(code: string): Promise<string> {
    return invokeCommand<string>('spotify_auth_callback', { code });
}

/**
 * Authenticate Spotify via native WebView2 window (S65)
 * Bypasses Playwright/headless Chromium WAF restrictions
 */
export async function spotifyAuthWebview(): Promise<AuthResult> {
    return invokeCommand<AuthResult>('spotify_auth_webview');
}

// ==============================================
// IMPORT
// ==============================================

/**
 * Import from a specific service (dispatches to appropriate importer)
 */
export async function importService(serviceName: string): Promise<string> {
    return invokeCommand<string>('import_service', { serviceName });
}

/**
 * Import Spotify library
 */
export async function importSpotifyLibrary(): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_spotify_library');
}

/**
 * Import Spotify playlists and their tracks
 */
export async function importSpotifyPlaylists(): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_spotify_playlists');
}

/**
 * Import Qobuz library
 */
export async function importQobuzLibrary(): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_qobuz_library');
}

/**
 * Import Tidal library
 */
export async function importTidalLibrary(): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_tidal_library');
}

/**
 * Import Deezer library
 */
export async function importDeezerLibrary(): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_deezer_library');
}

/**
 * Import SoundCloud library
 */
export async function importSoundCloudLibrary(): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_soundcloud_library');
}

/**
 * Import Apple Music library
 */
export async function importAppleMusicLibrary(): Promise<ImportResult> {
    return invokeCommand<ImportResult>('import_apple_music_library');
}

/**
 * Parse a streaming service URL
 */
export async function importFromUrl(url: string): Promise<UrlParseResult> {
    return invokeCommand<UrlParseResult>('import_from_url', { url });
}

// Export as namespace
export const accountsApi = {
    // Services
    getServices,
    getServiceStatuses,
    // Accounts
    getAccounts,
    addAccount,
    removeAccount,
    toggleAccountActive,
    updateAccountSyncTime,
    // Auth
    startAuthAndSave,
    startAuth,
    getAuthStatus,
    logoutService,
    validateAllSessions,
    // Spotify
    startSpotifyAuth,
    spotifyAuthCallback,
    spotifyAuthWebview,
    // Import
    importService,
    importSpotifyLibrary,
    importSpotifyPlaylists,
    importQobuzLibrary,
    importTidalLibrary,
    importDeezerLibrary,
    importSoundCloudLibrary,
    importAppleMusicLibrary,
    importFromUrl,
};
