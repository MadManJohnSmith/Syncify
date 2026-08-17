// useSyncSettings.ts - Manages per-service sync preferences
// Now integrated with backend database via Tauri commands
import { reactive, ref, computed, watch, onMounted } from 'vue'
import { settingsApi } from '@/api/settings'
import type {
    ServicePreference,
    SyncSettings as BackendSyncSettings,
    ServiceSyncSettings as BackendServiceSyncSettings
} from '@/api/types'

export interface ServiceSyncSettingsLocal {
    syncFavorites: boolean
    syncPlaylists: boolean
    syncSavedAlbums: boolean  // Maps to sync_albums in backend
    incrementalOnly: boolean  // Maps to incremental_sync in backend
    lastSynced: string | null
}

export type ServiceName = 'spotify' | 'qobuz' | 'tidal' | 'deezer' | 'soundcloud' | 'apple_music'

const defaultServiceSettings: ServiceSyncSettingsLocal = {
    syncFavorites: true,
    syncPlaylists: true,
    syncSavedAlbums: true,
    incrementalOnly: false,
    lastSynced: null,
}

// Singleton state
const isLoading = ref(true)
const error = ref<string | null>(null)

// Service preferences (priority ordering + auto-import)
const servicePreferences = ref<ServicePreference[]>([])

// Global sync settings
const globalSyncSettings = reactive<{
    autoSyncEnabled: boolean
    syncIntervalValue: number
    syncIntervalUnit: 'minutes' | 'hours' | 'days'
    syncOnStartup: boolean
    backgroundDownload: boolean
    maxConcurrentDownloads: number
    rateLimitDelayMs: number
    pauseOnMetered: boolean
    pauseOnLowBattery: boolean
}>({
    autoSyncEnabled: true,
    syncIntervalValue: 1,
    syncIntervalUnit: 'hours',
    syncOnStartup: true,
    backgroundDownload: true,
    maxConcurrentDownloads: 3,
    rateLimitDelayMs: 500,
    pauseOnMetered: true,
    pauseOnLowBattery: true,
})

// Per-service sync settings (reactive map)
const serviceSyncSettings = reactive<Record<ServiceName, ServiceSyncSettingsLocal>>({
    spotify: { ...defaultServiceSettings },
    qobuz: { ...defaultServiceSettings },
    tidal: { ...defaultServiceSettings },
    deezer: { ...defaultServiceSettings },
    soundcloud: { ...defaultServiceSettings },
    apple_music: { ...defaultServiceSettings },
})

// Map backend service name to local key
function toServiceKey(name: string): ServiceName {
    const normalized = name.toLowerCase().replace(/[\s-]/g, '_')
    if (normalized === 'apple_music' || normalized === 'applemusic') return 'apple_music'
    return normalized as ServiceName
}

// Load all settings from backend
async function loadFromBackend() {
    isLoading.value = true
    error.value = null

    try {
        // Fetch all in parallel
        const [prefs, globalSettings, perServiceSettings] = await Promise.all([
            settingsApi.getServicePreferences(),
            settingsApi.getSyncSettings(),
            settingsApi.getServiceSyncSettings(),
        ])

        // Update service preferences
        servicePreferences.value = prefs || []

        // Update global sync settings
        if (globalSettings) {
            globalSyncSettings.autoSyncEnabled = globalSettings.auto_sync_enabled
            globalSyncSettings.syncIntervalValue = globalSettings.sync_interval_value
            globalSyncSettings.syncIntervalUnit = globalSettings.sync_interval_unit
            globalSyncSettings.syncOnStartup = globalSettings.sync_on_startup
            globalSyncSettings.backgroundDownload = globalSettings.background_download
            globalSyncSettings.maxConcurrentDownloads = globalSettings.max_concurrent_downloads
            globalSyncSettings.rateLimitDelayMs = globalSettings.rate_limit_delay_ms
            globalSyncSettings.pauseOnMetered = globalSettings.pause_on_metered
            globalSyncSettings.pauseOnLowBattery = globalSettings.pause_on_low_battery
        }

        // Update per-service sync settings
        if (perServiceSettings && Array.isArray(perServiceSettings)) {
            for (const svc of perServiceSettings) {
                const key = toServiceKey(svc.service_name)
                if (serviceSyncSettings[key]) {
                    serviceSyncSettings[key] = {
                        syncFavorites: svc.sync_favorites,
                        syncPlaylists: svc.sync_playlists,
                        syncSavedAlbums: svc.sync_albums,
                        incrementalOnly: svc.incremental_sync,
                        lastSynced: svc.last_synced,
                    }
                }
            }
        }

        console.log('Loaded sync settings from backend:', {
            preferences: prefs.length,
            services: perServiceSettings.length,
        })
    } catch (e) {
        console.error('Failed to load sync settings from backend:', e)
        error.value = String(e)
    } finally {
        isLoading.value = false
    }
}

// Save global sync settings to backend
async function saveGlobalSettings() {
    try {
        await settingsApi.updateSyncSettings({
            id: 1,
            auto_sync_enabled: globalSyncSettings.autoSyncEnabled,
            sync_interval_value: globalSyncSettings.syncIntervalValue,
            sync_interval_unit: globalSyncSettings.syncIntervalUnit,
            sync_on_startup: globalSyncSettings.syncOnStartup,
            background_download: globalSyncSettings.backgroundDownload,
            max_concurrent_downloads: globalSyncSettings.maxConcurrentDownloads,
            rate_limit_delay_ms: globalSyncSettings.rateLimitDelayMs,
            pause_on_metered: globalSyncSettings.pauseOnMetered,
            pause_on_low_battery: globalSyncSettings.pauseOnLowBattery,
        })
    } catch (e) {
        console.error('Failed to save global sync settings:', e)
        throw e
    }
}

// Update a single service's sync settings
async function updateServiceSettings(serviceName: ServiceName | string) {
    const key = toServiceKey(serviceName)
    const settings = serviceSyncSettings[key]

    try {
        await settingsApi.updateServiceSyncSettings(
            key,
            settings.syncFavorites,
            settings.syncPlaylists,
            settings.syncSavedAlbums,
            settings.incrementalOnly,
        )
    } catch (e) {
        console.error(`Failed to save ${key} sync settings:`, e)
        throw e
    }
}

// Reorder service priorities
async function reorderPriorities(serviceNames: string[]) {
    try {
        const updated = await settingsApi.reorderServicePriorities(serviceNames)
        servicePreferences.value = updated
    } catch (e) {
        console.error('Failed to reorder service priorities:', e)
        throw e
    }
}

// Update auto-import for a service
async function updateAutoImport(serviceName: string, enabled: boolean) {
    try {
        const updated = await settingsApi.updateServicePreference(serviceName, enabled)
        const idx = servicePreferences.value.findIndex(p =>
            p.service_name.toLowerCase() === serviceName.toLowerCase()
        )
        if (idx >= 0) {
            servicePreferences.value[idx] = updated
        }
    } catch (e) {
        console.error(`Failed to update auto-import for ${serviceName}:`, e)
        throw e
    }
}

export function useSyncSettings() {
    return {
        // State
        settings: serviceSyncSettings,
        globalSettings: globalSyncSettings,
        servicePreferences,
        isLoading,
        error,

        // Load from backend
        loadSettings: loadFromBackend,

        // Get settings for a specific service
        getService(name: ServiceName | string): ServiceSyncSettingsLocal {
            const key = toServiceKey(name)
            return serviceSyncSettings[key] || defaultServiceSettings
        },

        // Update a single service setting (local + backend)
        async updateSetting(service: ServiceName | string, settingKey: keyof ServiceSyncSettingsLocal, value: boolean) {
            const key = toServiceKey(service)
            if (serviceSyncSettings[key] && settingKey !== 'lastSynced') {
                serviceSyncSettings[key][settingKey] = value as any
                await updateServiceSettings(key)
            }
        },

        // Check if playlists should be synced for a service
        shouldSyncPlaylists(service: ServiceName | string): boolean {
            return this.getService(service).syncPlaylists
        },

        // Check if favorites should be synced for a service
        shouldSyncFavorites(service: ServiceName | string): boolean {
            return this.getService(service).syncFavorites
        },

        // Priority management
        reorderPriorities,
        updateAutoImport,

        // Persistence
        saveGlobalSettings,
        updateServiceSettings,
    }
}
