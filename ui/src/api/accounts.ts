/**
 * Accounts API
 * 
 * Tauri commands for service accounts and authentication.
 */

import { invokeCommand } from './tauri';
import { asArray, asNumber, asRecord } from './normalize';
import type {
    Service,
    Account,
    ServiceStatus,
    SessionStatus,
    AuthResult,
    ImportResult,
    UrlParseResult,
    ImportPreferences,
    ServiceSyncResult,
} from './types';

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

// ==============================================
// SERVICES
// ==============================================

/**
 * Get all available services
 */
export async function getServices(): Promise<Service[]> {
    const raw = await invokeCommand<unknown>('get_services');
    // Array identity is preserved intentionally: views hold and mutate fetched
    // objects across refreshes, so only non-array payloads are substituted.
    return asArray<Service>(raw);
}

/**
 * Get service statuses (legacy)
 */
export async function getServiceStatuses(): Promise<ServiceStatus[]> {
    const raw = await invokeCommand<unknown>('get_service_statuses');
    return asArray<ServiceStatus>(raw);
}

// ==============================================
// ACCOUNTS
// ==============================================

/**
 * Get all connected accounts
 */
export async function getAccounts(): Promise<Account[]> {
    const raw = await invokeCommand<unknown>('get_accounts');
    return asArray<Account>(raw);
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
 * Check auth status for a service (alias to getAuthStatus)
 */
export async function checkAuthStatus(service: string): Promise<AuthResult> {
    return getAuthStatus(service);
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
    const raw = await invokeCommand<unknown>('validate_all_sessions');
    return asArray<SessionStatus>(raw).filter((s) => asRecord(s) !== null);
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
// SPOTIFY API CREDENTIALS (Sprint S196)
// ==============================================

/** Redirect URI the packaged app's OAuth window binds; users must register
 *  EXACTLY this URI in their Spotify dashboard app. Must stay in sync with
 *  `SPOTIFY_DEFAULT_REDIRECT_URI` (src-tauri/src/services/spotify.rs). */
export const SPOTIFY_DEFAULT_REDIRECT_URI = 'http://127.0.0.1:8888/callback';

/** User-visible status of the Spotify developer credentials.
 *  The secret is NEVER returned by the backend: only a `****last4` mask. */
export interface SpotifyApiConfig {
    clientId: string;
    /** Masked secret ('****abcd') or '' when not configured. */
    secretMask: string;
    /** Effective redirect URI shown read-only in the UI. */
    redirectUri: string;
    /** True when both client id and secret are present in DB settings. */
    configured: boolean;
}

/**
 * Load the Spotify API credentials status from DB settings.
 * Also rehydrates the backend credential cache (BD > env resolution).
 */
export async function getSpotifyApiConfig(): Promise<SpotifyApiConfig> {
    const kv = await invokeCommand<Record<string, string>>('get_kv_settings', {
        keys: ['spotify_client_id', 'spotify_client_secret', 'spotify_redirect_uri'],
    });
    const clientId = (kv['spotify_client_id'] ?? '').trim();
    const secretMask = (kv['spotify_client_secret'] ?? '').trim();
    const storedRedirect = (kv['spotify_redirect_uri'] ?? '').trim();
    return {
        clientId,
        secretMask,
        redirectUri: storedRedirect !== '' ? storedRedirect : SPOTIFY_DEFAULT_REDIRECT_URI,
        configured: clientId !== '' && secretMask.startsWith('****'),
    };
}

/**
 * Persist the Spotify API credentials (BD settings; secret encrypted at rest).
 *
 * - `clientSecret === null` → keep the currently stored secret untouched.
 * - `clientSecret === ''`   → explicitly clear it.
 * - any other value         → replace it (sent encrypted by the backend).
 */
export async function saveSpotifyApiConfig(
    clientId: string,
    clientSecret: string | null,
    redirectUri: string
): Promise<void> {
    const settings: Record<string, string> = {
        spotify_client_id: clientId.trim(),
        spotify_redirect_uri: redirectUri.trim(),
    };
    if (clientSecret !== null) {
        settings.spotify_client_secret = clientSecret;
    }
    await invokeCommand('save_settings_batch', { settings });
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
    return normalizeImportResult(await invokeCommand<unknown>('import_spotify_library'));
}

/**
 * Import Spotify playlists and their tracks
 */
export async function importSpotifyPlaylists(): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('import_spotify_playlists'));
}

/**
 * Import Qobuz library
 */
export async function importQobuzLibrary(): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('import_qobuz_library'));
}

/**
 * Import Qobuz playlists
 */
export async function importQobuzPlaylists(): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('import_qobuz_playlists'));
}

/**
 * Import Tidal library
 */
export async function importTidalLibrary(): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('import_tidal_library'));
}

/**
 * Import Deezer library
 */
export async function importDeezerLibrary(): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('import_deezer_library'));
}

/**
 * Import SoundCloud library
 */
export async function importSoundCloudLibrary(): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('import_soundcloud_library'));
}

/**
 * Import Apple Music library
 */
export async function importAppleMusicLibrary(): Promise<ImportResult> {
    return normalizeImportResult(await invokeCommand<unknown>('import_apple_music_library'));
}

/**
 * Parse a streaming service URL
 */
export async function importFromUrl(url: string): Promise<UrlParseResult> {
    return invokeCommand<UrlParseResult>('import_from_url', { url });
}

/**
 * Perform unified sync for a service with real auth verification and granular preferences
 */
export async function syncService(
    service: string,
    accountId?: number | null,
    preferences?: ImportPreferences | null
): Promise<ServiceSyncResult> {
    return invokeCommand<ServiceSyncResult>('sync_service', {
        service,
        accountId: accountId ?? null,
        preferences: preferences ?? null,
    });
}

/**
 * Get granular import preferences for a service from backend
 */
export async function getServiceImportPreferences(service: string): Promise<ImportPreferences> {
    return invokeCommand<ImportPreferences>('get_service_import_preferences', { service });
}

/**
 * Update granular import preferences for a service in backend
 */
export async function updateServiceImportPreferences(preferences: ImportPreferences): Promise<ImportPreferences> {
    return invokeCommand<ImportPreferences>('update_service_import_preferences', { preferences });
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
    checkAuthStatus,
    logoutService,
    validateAllSessions,
    // Spotify
    startSpotifyAuth,
    spotifyAuthCallback,
    spotifyAuthWebview,
    getSpotifyApiConfig,
    saveSpotifyApiConfig,
    // Import
    syncService,
    getServiceImportPreferences,
    updateServiceImportPreferences,
    importService,
    importSpotifyLibrary,
    importSpotifyPlaylists,
    importQobuzLibrary,
    importQobuzPlaylists,
    importTidalLibrary,
    importDeezerLibrary,
    importSoundCloudLibrary,
    importAppleMusicLibrary,
    importFromUrl,
};

