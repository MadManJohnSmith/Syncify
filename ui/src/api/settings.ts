/**
 * Settings API
 * 
 * Tauri commands for application settings.
 */

import { invokeCommand } from './tauri';
import { asArray, asRecord } from './normalize';
import type {
    AppSettings,
    HealthCheck,
    DependencyCheckResult,
    BridgeResult,
    ServicePreference,
    SyncSettings,
    ServiceSyncSettings,
    QualityPreference,
    FolderSettings,
    DuplicateSettings,
    AudioProcessingSettings,
    LyricsProviderSetting,
    LyricsConfig,
    MetadataPreferences,
    DownloadSettingsDto,
    PathStatus,
    EffectiveDownloadPreferences,
} from './types';

// ==============================================
// SETTINGS
// ==============================================

/**
 * Get application settings
 */
export async function getSettings(): Promise<AppSettings> {
    return invokeCommand<AppSettings>('get_app_settings');
}

/**
 * Get the OS-aware default download path
 */
export async function getDefaultDownloadPath(): Promise<string> {
    return invokeCommand<string>('get_default_download_path');
}

/**
 * Save application settings
 */
export async function saveSettings(settings: AppSettings): Promise<string> {
    return invokeCommand<string>('service_save_settings', { settings });
}

/**
 * Get multiple settings by keys mapping them to a string dictionary
 */
export async function getSettingsByKeys(keys: string[]): Promise<Record<string, string>> {
    return invokeCommand<Record<string, string>>('get_kv_settings', { keys });
}

/**
 * Save a single setting
 */
export async function saveSetting(key: string, value: string): Promise<void> {
    return invokeCommand<void>('save_setting', { key, value });
}

/**
 * Save multiple settings via batch transaction
 */
export async function saveSettingsBatch(settings: Record<string, string>): Promise<void> {
    return invokeCommand<void>('save_settings_batch', { settings });
}

// ==============================================
// HEALTH CHECK
// ==============================================

/**
 * Run health check
 */
export async function runHealthCheck(): Promise<HealthCheck> {
    return invokeCommand<HealthCheck>('run_health_check');
}

// ==============================================
// DEPENDENCY MANAGEMENT
// ==============================================

/**
 * Check all dependencies
 */
export async function checkDependencies(): Promise<DependencyCheckResult> {
    return invokeCommand<DependencyCheckResult>('check_dependencies');
}

/**
 * Install a specific dependency
 */
export async function installDependency(name: string): Promise<BridgeResult> {
    return invokeCommand<BridgeResult>('install_dependency', { name });
}

/**
 * Install all dependencies
 */
export async function installAllDependencies(): Promise<BridgeResult> {
    return invokeCommand<BridgeResult>('install_all_dependencies');
}

/**
 * Ensure a dependency is available
 */
export async function ensureDependency(name: string): Promise<BridgeResult> {
    return invokeCommand<BridgeResult>('ensure_dependency', { name });
}

// ==============================================
// SPRINT 1: SERVICE PREFERENCES & SYNC SETTINGS
// ==============================================

/**
 * Get all service preferences ordered by priority
 */
export async function getServicePreferences(): Promise<ServicePreference[]> {
    return invokeCommand<ServicePreference[]>('get_service_preferences');
}

/**
 * Update a service preference's auto-import setting
 */
export async function updateServicePreference(
    serviceName: string,
    autoImportEnabled: boolean
): Promise<ServicePreference> {
    return invokeCommand<ServicePreference>('update_service_preference', {
        serviceName,
        autoImportEnabled,
    });
}

/**
 * Reorder service priorities based on the provided order
 */
export async function reorderServicePriorities(
    serviceNames: string[]
): Promise<ServicePreference[]> {
    return invokeCommand<ServicePreference[]>('reorder_service_priorities', {
        serviceNames,
    });
}

/**
 * Get global sync settings
 */
export async function getSyncSettings(): Promise<SyncSettings> {
    return invokeCommand<SyncSettings>('get_sync_settings');
}

/**
 * Update global sync settings
 */
export async function updateSyncSettings(settings: SyncSettings): Promise<SyncSettings> {
    return invokeCommand<SyncSettings>('update_sync_settings', { settings });
}

/**
 * Get per-service sync settings
 */
export async function getServiceSyncSettings(): Promise<ServiceSyncSettings[]> {
    return invokeCommand<ServiceSyncSettings[]>('get_service_sync_settings');
}

/**
 * Update per-service sync settings
 */
export async function updateServiceSyncSettings(
    serviceName: string,
    syncFavorites: boolean,
    syncPlaylists: boolean,
    syncAlbums: boolean,
    incrementalSync: boolean
): Promise<ServiceSyncSettings> {
    return invokeCommand<ServiceSyncSettings>('update_service_sync_settings', {
        serviceName,
        syncFavorites,
        syncPlaylists,
        syncAlbums,
        incrementalSync,
    });
}

// ==============================================
// SPRINT 2: DOWNLOADS + FILE SETTINGS
// ==============================================

/**
 * Get quality preferences for all services
 */
export async function getQualityPreferences(): Promise<QualityPreference[]> {
    return invokeCommand<QualityPreference[]>('get_quality_preferences');
}

/**
 * Update quality preference for a service
 */
export async function updateQualityPreference(
    serviceName: string,
    maxQuality: string,
    preferredFormat: string,
    fallbackQuality: string,
    fallbackFormat: string
): Promise<QualityPreference> {
    return invokeCommand<QualityPreference>('update_quality_preference', {
        serviceName,
        maxQuality,
        preferredFormat,
        fallbackQuality,
        fallbackFormat,
    });
}

/**
 * Get folder settings
 */
export async function getFolderSettings(): Promise<FolderSettings> {
    return invokeCommand<FolderSettings>('get_folder_settings');
}

/**
 * Update folder settings
 */
export async function updateFolderSettings(settings: FolderSettings): Promise<FolderSettings> {
    return invokeCommand<FolderSettings>('update_folder_settings', { settings });
}

/**
 * Preview folder path for a track
 */
export async function previewFolderPath(trackId: number): Promise<string> {
    return invokeCommand<string>('preview_folder_path', { trackId });
}

/**
 * Get duplicate settings
 */
export async function getDuplicateSettings(): Promise<DuplicateSettings> {
    return invokeCommand<DuplicateSettings>('get_duplicate_settings');
}

/**
 * Update duplicate settings
 */
export async function updateDuplicateSettings(settings: DuplicateSettings): Promise<DuplicateSettings> {
    return invokeCommand<DuplicateSettings>('update_duplicate_settings', { settings });
}

/**
/**
 * Get audio processing settings
 */
export async function getAudioProcessingSettings(): Promise<AudioProcessingSettings> {
    return invokeCommand<AudioProcessingSettings>('get_audio_processing_settings');
}

/**
 * Update audio processing settings
 */
export async function updateAudioProcessingSettings(
    settings: AudioProcessingSettings
): Promise<AudioProcessingSettings> {
    return invokeCommand<AudioProcessingSettings>('update_audio_processing_settings', { settings });
}

export interface DownloadSettings {
    download_path: string;
    temporary_root?: string;
    concurrent_downloads: number;
    fallback_action: string;
    quality_preferences?: QualityPreference[];
    folder_settings?: FolderSettings;
}

export interface PathValidationResult {
    valid: boolean;
    exists: boolean;
    is_dir: boolean;
    is_writable: boolean;
    available_bytes: number;
    drive_mounted: boolean;
    canonical_path: string;
    error_message?: string | null;
}

export type { DownloadSettingsDto, PathStatus, EffectiveDownloadPreferences } from './types';

/**
 * Raw wire contract of the `get_download_settings` command.
 *
 * The canonical Rust DTO (`DownloadSettingsDto` in commands/settings.rs) uses
 * snake_case fields, while legacy builds exposed several alternative key names;
 * every field is optional because `getDownloadSettings()` tolerates all shapes
 * and derives safe defaults for anything missing.
 */
export interface RawDownloadSettingsResponse {
    // Canonical library root / download path spellings
    library_root?: string | null;
    download_path?: string;
    base_folder?: string;
    dl_download_path?: string;
    // Canonical staging root / temp path spellings
    staging_root?: string | null;
    temporary_root?: string | null;
    temp_dir?: string;
    dl_temp_dir?: string;
    // Concurrency and fallback behaviour
    max_concurrent_downloads?: number;
    concurrent_downloads?: number;
    fallback_action?: string;
    // Folder template fields
    folder_template?: string;
    file_template?: string;
    artist_separator?: string;
    replace_spaces_with?: string | null;
    max_path_length?: number;
}

/**
 * Get canonical effective download preferences
 */
export async function getEffectiveDownloadPreferences(): Promise<EffectiveDownloadPreferences> {
    return invokeCommand<EffectiveDownloadPreferences>('get_effective_download_preferences');
}

/**
 * Save canonical effective download preferences atomically
 */
export async function saveEffectiveDownloadPreferences(
    preferences: EffectiveDownloadPreferences
): Promise<EffectiveDownloadPreferences> {
    return invokeCommand<EffectiveDownloadPreferences>('save_effective_download_preferences', { preferences });
}


/**
 * Normalizes any download settings shape into DownloadSettingsDto
 */
export function deriveStagingRoot(libraryRoot: unknown): string {
    if (typeof libraryRoot !== 'string' || !libraryRoot.trim()) return '';
    const trimmed = libraryRoot.trim().replace(/[\\/]+$/, '');
    if (!trimmed) return '';
    const sep = trimmed.includes('/') && !trimmed.includes('\\') ? '/' : '\\';
    return `${trimmed}${sep}.staging`;
}

export function determinePathStatus(validation: PathValidationResult | null | undefined): PathStatus {
    if (!validation) return 'valid';
    if (!validation.drive_mounted) return 'unavailable';
    if (!validation.exists) return 'missing';
    if (!validation.is_writable) return 'not_writable';
    if (validation.valid) return 'valid';
    return 'unavailable';
}

/**
 * Get default temporary staging directory
 */
export async function getDefaultTempPath(): Promise<string> {
    try {
        return await invokeCommand<string>('get_default_temp_path');
    } catch {
        return '';
    }
}

/**
 * Validate directory existence, drive mount, writability, and space
 */
export async function validateDirectoryPath(path: string): Promise<PathValidationResult> {
    try {
        return await invokeCommand<PathValidationResult>('validate_directory_path', { path });
    } catch (err) {
        return {
            valid: false,
            exists: false,
            is_dir: false,
            is_writable: false,
            available_bytes: 0,
            drive_mounted: false,
            canonical_path: path,
            error_message: String(err),
        };
    }
}

/**
 * Set max concurrent downloads on active worker
 */
export async function setMaxConcurrentDownloads(max: number): Promise<void> {
    return invokeCommand<void>('set_max_concurrent_downloads', { max });
}

/**
 * Update fallback action setting
 */
export async function updateFallbackAction(fallbackAction: string): Promise<FolderSettings> {
    try {
        return await invokeCommand<FolderSettings>('update_fallback_action', { fallbackAction });
    } catch {
        const folder = await getFolderSettings();
        folder.fallback_action = fallbackAction;
        return updateFolderSettings(folder);
    }
}

/**
 * Get unified DownloadSettingsDto contract
 */
export async function getUnifiedDownloadSettings(): Promise<DownloadSettingsDto> {
    const raw = await getDownloadSettings();
    const library_root = raw.download_path || '';
    const derived_staging = deriveStagingRoot(library_root);
    const staging_root = raw.temporary_root || derived_staging;

    let path_status: PathStatus = 'valid';
    let free_space_bytes: number | null = null;

    if (library_root) {
        try {
            const validation = await validateDirectoryPath(library_root);
            path_status = determinePathStatus(validation);
            free_space_bytes = validation.available_bytes ?? null;
        } catch {
            path_status = 'unavailable';
        }
    } else {
        path_status = 'missing';
    }

    return {
        library_root,
        staging_root,
        path_status,
        free_space_bytes,
    };
}

/**
 * Get consolidated download settings
 */
export async function getDownloadSettings(): Promise<DownloadSettings> {
    try {
        const res = await invokeCommand<RawDownloadSettingsResponse | null>('get_download_settings');
        if (res) {
            const path = res.library_root ?? res.download_path ?? res.base_folder ?? res.dl_download_path ?? '';
            const temp = res.staging_root ?? res.temporary_root ?? res.temp_dir ?? res.dl_temp_dir ?? deriveStagingRoot(path);
            return {
                download_path: path,
                temporary_root: temp,
                concurrent_downloads: res.max_concurrent_downloads ?? res.concurrent_downloads ?? 3,
                fallback_action: res.fallback_action ?? 'try_next',
                folder_settings: {
                    id: 1,
                    base_folder: path,
                    folder_template: res.folder_template ?? '{AlbumArtist}/{Album}',
                    file_template: res.file_template ?? '{TrackNumber:pad2} - {Title}',
                    artist_separator: res.artist_separator ?? ', ',
                    replace_spaces_with: res.replace_spaces_with ?? null,
                    max_path_length: res.max_path_length ?? 255,
                    fallback_action: res.fallback_action ?? 'try_next',
                }
            };
        }
    } catch {
        // Fallback to KV and folder settings
    }

    const generalKeys = [
        'dl_concurrent_downloads',
        'dl_retry_failed',
        'dl_retry_count',
        'dl_retry_delay',
        'dl_download_path',
        'download_dir',
        'dl_temp_dir',
        'temp_dir',
        'dl_create_artist_folder',
        'dl_create_album_folder',
        'dl_auto_download_favorites'
    ];
    const [folder, quality, kvRaw, defaultPath] = await Promise.all([
        getFolderSettings().catch(() => null),
        getQualityPreferences().catch(() => []),
        getSettingsByKeys(generalKeys).catch(() => ({} as Record<string, string>)),
        getDefaultDownloadPath().catch(() => ''),
    ]);
    const kv = (kvRaw || {}) as Record<string, string>;
    const configuredPath = (kv.dl_download_path ?? kv.download_dir ?? '').trim();
    const resolvedPath = configuredPath || folder?.base_folder || defaultPath || '';
    const configuredTemp = (kv.dl_temp_dir ?? kv.temp_dir ?? '').trim();
    const resolvedTemp = configuredTemp || deriveStagingRoot(resolvedPath);

    return {
        download_path: resolvedPath,
        temporary_root: resolvedTemp,
        concurrent_downloads: parseInt(kv.dl_concurrent_downloads || '3', 10),
        fallback_action: folder?.fallback_action || 'try_next',
        quality_preferences: quality || [],
        folder_settings: folder || undefined,
    };
}

/**
 * Save consolidated download settings
 */
export async function saveDownloadSettings(settings: Partial<DownloadSettings>): Promise<void> {
    try {
        await invokeCommand<void>('save_download_settings', { settings });
    } catch {
        const batch: Record<string, string> = {};
        if (settings.download_path !== undefined) {
            batch.dl_download_path = settings.download_path;
            batch.download_dir = settings.download_path;
        }
        if (settings.temporary_root !== undefined) {
            batch.dl_temp_dir = settings.temporary_root;
            batch.temp_dir = settings.temporary_root;
        }
        if (settings.concurrent_downloads !== undefined) {
            batch.dl_concurrent_downloads = settings.concurrent_downloads.toString();
            try {
                await setMaxConcurrentDownloads(settings.concurrent_downloads);
            } catch (err) {
                console.warn('Failed to set worker live concurrency:', err);
            }
        }
        if (Object.keys(batch).length > 0) {
            await saveSettingsBatch(batch);
        }
        if (settings.folder_settings || settings.download_path !== undefined || settings.fallback_action !== undefined) {
            const currentFolder = settings.folder_settings ?? await getFolderSettings();
            if (settings.download_path !== undefined) currentFolder.base_folder = settings.download_path;
            if (settings.fallback_action !== undefined) currentFolder.fallback_action = settings.fallback_action;
            await updateFolderSettings(currentFolder);
        }
    }
}

// ==============================================
// SPRINT 3: LYRICS TAB + SETTINGS
// ==============================================

/**
 * Get all lyrics provider settings ordered by priority
 */
export async function getLyricsProviders(): Promise<LyricsProviderSetting[]> {
    return invokeCommand<LyricsProviderSetting[]>('get_lyrics_providers');
}

/**
 * Update a lyrics provider setting
 */
export async function updateLyricsProvider(
    providerId: string,
    enabled: boolean,
    priority: number
): Promise<LyricsProviderSetting> {
    return invokeCommand<LyricsProviderSetting>('update_lyrics_provider', {
        providerId,
        enabled,
        priority,
    });
}

/**
 * Reorder lyrics providers
 */
export async function reorderLyricsProviders(
    providerIds: string[]
): Promise<LyricsProviderSetting[]> {
    return invokeCommand<LyricsProviderSetting[]>('reorder_lyrics_providers', {
        providerIds,
    });
}

/**
 * Get lyrics configuration
 */
export async function getLyricsConfig(): Promise<LyricsConfig> {
    return invokeCommand<LyricsConfig>('get_lyrics_config');
}

/**
 * Update lyrics configuration
 */
export async function updateLyricsConfig(config: LyricsConfig): Promise<LyricsConfig> {
    return invokeCommand<LyricsConfig>('update_lyrics_config', { config });
}

/**
 * Test a lyrics provider connection
 */
export async function testLyricsProvider(providerId: string): Promise<boolean> {
    return invokeCommand<boolean>('test_lyrics_provider', { providerId });
}

// ==============================================
// SPRINT 4: DASHBOARD + LIBRARY DETAIL VIEWS API
// ==============================================


// Export as namespace
export const settingsApi = {
    getSettings,
    getDefaultDownloadPath,
    saveSettings,
    getSettingsByKeys,
    saveSetting,
    saveSettingsBatch,
    runHealthCheck,
    checkDependencies,
    installDependency,
    installAllDependencies,
    ensureDependency,
    // Sprint 1: Service Preferences & Sync Settings
    getServicePreferences,
    updateServicePreference,
    reorderServicePriorities,
    getSyncSettings,
    updateSyncSettings,
    getServiceSyncSettings,
    updateServiceSyncSettings,
    // Sprint 2: Downloads + File Settings
    getQualityPreferences,
    updateQualityPreference,
    getFolderSettings,
    updateFolderSettings,
    previewFolderPath,
    getDuplicateSettings,
    updateDuplicateSettings,
    getAudioProcessingSettings,
    updateAudioProcessingSettings,
    getDownloadSettings,
    getUnifiedDownloadSettings,
    getEffectiveDownloadPreferences,
    deriveStagingRoot,
    determinePathStatus,
    saveDownloadSettings,
    updateFallbackAction,
    setMaxConcurrentDownloads,
    getDefaultTempPath,
    validateDirectoryPath,
    // Sprint 3: Lyrics Tab + Settings
    getLyricsProviders,
    updateLyricsProvider,
    reorderLyricsProviders,
    getLyricsConfig,
    updateLyricsConfig,
    testLyricsProvider,
    // Sprint 14: Metadata & Tags Settings
    getMetadataPreferences,
    updateMetadataPreferences,
    // Sprint 5: Advanced Settings & Polish
    getAdvancedSettings,
    updateAdvancedSettings,
    vacuumDatabase,
    getCacheStats,
    clearCache,
    runDiagnostics,
    resetToDefaults,
    getLastfmApiKeyStatus,
    setLastfmApiKey,
    // S203: Global max quality ceiling
    getGlobalMaxQuality,
    setGlobalMaxQuality,
};

// ==============================================
// SPRINT 5: ADVANCED SETTINGS & POLISH API
// ==============================================

import type {
    AdvancedSettings,
    CacheStats,
    DiagnosticResult,
} from './types';

/**
 * Get advanced application settings
 */
export async function getAdvancedSettings(): Promise<AdvancedSettings> {
    return invokeCommand<AdvancedSettings>('get_advanced_settings');
}

/**
 * Update advanced application settings
 */
export async function updateAdvancedSettings(settings: AdvancedSettings): Promise<AdvancedSettings> {
    return invokeCommand<AdvancedSettings>('update_advanced_settings', { settings });
}

/**
 * Vacuum the database to reclaim space
 */
export async function vacuumDatabase(): Promise<string> {
    return invokeCommand<string>('vacuum_database');
}

/**
 * Get cache statistics
 */
export async function getCacheStats(): Promise<CacheStats[]> {
    const raw = await invokeCommand<unknown>('get_cache_stats');
    return asArray<CacheStats>(raw);
}

/**
 * Clear cache by type or all
 */
export async function clearCache(cacheType?: string): Promise<string> {
    return invokeCommand<string>('clear_cache', { cacheType });
}

/**
 * Run system diagnostics
 */
export async function runDiagnostics(): Promise<DiagnosticResult[]> {
    return invokeCommand<DiagnosticResult[]>('run_diagnostics');
}

/**
 * Reset settings to defaults
 */
export async function resetToDefaults(settingsType: string): Promise<string> {
    return invokeCommand<string>('reset_to_defaults', { settingsType });
}

// ==============================================
// SPRINT 14: METADATA PREFERENCES API
// ==============================================

/**
 * Get metadata preferences
 */
export async function getMetadataPreferences(): Promise<MetadataPreferences> {
    return invokeCommand<MetadataPreferences>('get_metadata_preferences');
}

/**
 * Update metadata preferences
 */
export async function updateMetadataPreferences(
    settings: MetadataPreferences
): Promise<MetadataPreferences> {
    return invokeCommand<MetadataPreferences>('update_metadata_preferences', { settings });
}

// ==============================================
// S200: LAST.FM API KEY (settings KV, plain — client identifier)
// ==============================================

export interface LastfmKeyStatus {
    configured: boolean;
    masked: string | null;
    source: 'settings' | 'env' | 'none';
}

export async function getLastfmApiKeyStatus(): Promise<LastfmKeyStatus> {
    return invokeCommand<LastfmKeyStatus>('get_lastfm_api_key_status');
}

export async function setLastfmApiKey(apiKey: string): Promise<void> {
    return invokeCommand<void>('set_lastfm_api_key', { apiKey });
}

// ==============================================
// S203: GLOBAL MAX QUALITY CEILING (settings KV `global_max_quality`)
// ==============================================
//
// Single download-quality ceiling enforced by the Rust worker when it resolves
// every DownloadRequest: effective = min(global, per-service quality_preferences
// row), ordering any < high < lossless < hires. Types are colocated here because
// api/types.ts is out of bounds for this sprint.

export type GlobalMaxQuality = 'any' | 'hires' | 'lossless' | 'high';

export const GLOBAL_MAX_QUALITY_KEY = 'global_max_quality';

/** Canonical values accepted by the backend `set_global_max_quality` command. */
export const GLOBAL_MAX_QUALITY_VALUES: readonly GlobalMaxQuality[] = ['any', 'hires', 'lossless', 'high'];

function canonicalizeGlobalMaxQuality(raw: unknown): GlobalMaxQuality {
    const value = typeof raw === 'string' ? raw.trim().toLowerCase() : '';
    if ((GLOBAL_MAX_QUALITY_VALUES as readonly string[]).includes(value)) {
        return value as GlobalMaxQuality;
    }
    // Unknown / legacy spellings fail open to 'any' (= no ceiling), mirroring
    // canonical_global_max_quality on the Rust side.
    return 'any';
}

/**
 * Read the global download-quality ceiling.
 * Primary path is the dedicated IPC command; falls back to the generic KV row
 * (same key) on builds where that command is not registered yet.
 */
export async function getGlobalMaxQuality(): Promise<GlobalMaxQuality> {
    try {
        const v = await invokeCommand<string>('get_global_max_quality');
        return canonicalizeGlobalMaxQuality(v);
    } catch {
        try {
            const kv = await getSettingsByKeys([GLOBAL_MAX_QUALITY_KEY]);
            return canonicalizeGlobalMaxQuality(kv?.[GLOBAL_MAX_QUALITY_KEY]);
        } catch {
            return 'any';
        }
    }
}

/**
 * Persist the global download-quality ceiling (canonical vocabulary only).
 * Throws for non-canonical values; falls back to the generic KV write when the
 * dedicated command is not registered yet.
 */
export async function setGlobalMaxQuality(value: GlobalMaxQuality): Promise<void> {
    if (!(GLOBAL_MAX_QUALITY_VALUES as readonly string[]).includes(value)) {
        throw new Error(`Invalid global_max_quality '${value}': expected one of ${GLOBAL_MAX_QUALITY_VALUES.join('|')}`);
    }
    try {
        await invokeCommand<string>('set_global_max_quality', { value });
    } catch (err) {
        console.warn('[settings] set_global_max_quality unavailable, writing generic KV row:', err);
        await saveSetting(GLOBAL_MAX_QUALITY_KEY, value);
    }
}
