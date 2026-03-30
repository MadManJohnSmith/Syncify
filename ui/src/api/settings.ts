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
