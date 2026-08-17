/**
 * Settings API
 * 
 * Tauri commands for application settings.
 */

import { invokeCommand } from './tauri';
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

/**
 * Get default temporary staging directory
 */
export async function getDefaultTempPath(): Promise<string> {
    try {
        return await invokeCommand<string>('get_default_temp_path');
    } catch {
        return 'C:\\Users\\User\\AppData\\Local\\Temp\\Syncify';
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
 * Get consolidated download settings
 */
export async function getDownloadSettings(): Promise<DownloadSettings> {
    try {
        return await invokeCommand<DownloadSettings>('get_download_settings');
    } catch {
        const generalKeys = [
            'dl_concurrent_downloads',
            'dl_retry_failed',
            'dl_retry_count',
            'dl_retry_delay',
            'dl_download_path',
            'dl_create_artist_folder',
            'dl_create_album_folder',
            'dl_auto_download_favorites'
        ];
        const [folder, quality, kv, defaultPath] = await Promise.all([
            getFolderSettings(),
            getQualityPreferences(),
            getSettingsByKeys(generalKeys),
            getDefaultDownloadPath(),
        ]);
        const configuredPath = (kv.dl_download_path ?? '').trim();
        return {
            download_path: configuredPath || folder.base_folder || defaultPath || '',
            concurrent_downloads: parseInt(kv.dl_concurrent_downloads || '3', 10),
            fallback_action: folder.fallback_action || 'try_next',
            quality_preferences: quality || [],
            folder_settings: folder,
        };
    }
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
    return invokeCommand<CacheStats[]>('get_cache_stats');
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
