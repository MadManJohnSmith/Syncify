// useDownloadSettings.ts - Manages Sprint 2 download and file settings
// Integrates with backend database via Tauri commands

import { reactive, ref, computed, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { settingsApi, type PathValidationResult } from '@/api/settings'
import type {
    QualityPreference,
    FolderSettings,
    DuplicateSettings,
    AudioProcessingSettings,
} from '@/api/types'

// Singleton state
const isLoading = ref(true)
const error = ref<string | null>(null)

// Quality preferences by service
const qualityPreferences = ref<QualityPreference[]>([])

// Folder settings (singleton)
const folderSettings = reactive<FolderSettings>({
    id: 1,
    base_folder: '',
    folder_template: '{AlbumArtist}/{Album}',
    file_template: '{TrackNumber:pad2} - {Title}',
    artist_separator: ', ',
    replace_spaces_with: null,
    max_path_length: 255,
    fallback_action: 'try_next',
})

// Duplicate settings (singleton)
const duplicateSettings = reactive<DuplicateSettings>({
    id: 1,
    enable_detection: true,
    prefer_higher_quality: true,
    prefer_lossless: true,
    replace_same_quality_different_source: false,
    quality_threshold_kbps: 64,
    delete_duplicates_immediately: false,
    move_to_trash: true,
})

// Audio processing settings (singleton)
const audioProcessingSettings = reactive<AudioProcessingSettings>({
    id: 1,
    replay_gain_mode: 'off',
    target_loudness_lufs: -14.0,
    transcode_enabled: false,
    transcode_format: 'mp3',
    transcode_bitrate: 320,
    keep_original_after_transcode: true,
    embed_lyrics: true,
    embed_artwork: true,
    artwork_max_size: 1200,
})

// Temporary staging directory
const temporaryPath = ref('')

// General download KV settings (singleton)
const generalSettings = reactive({
    concurrentDownloads: '3',
    retryFailed: '3',
    retryCount: '3',
    retryDelay: '5000',
    downloadPath: '',
    organizeByArtist: true,
    organizeByAlbum: true,
    autoDownloadFavorites: false,
    maxSpeed: 0,
    pauseOnMetered: false,
})

// Computed helper bindings
const downloadPath = computed({
    get: () => generalSettings.downloadPath,
    set: (val: string) => {
        generalSettings.downloadPath = val
        folderSettings.base_folder = val
    }
})

const concurrentDownloads = computed({
    get: () => parseInt(generalSettings.concurrentDownloads, 10) || 3,
    set: (val: number) => {
        generalSettings.concurrentDownloads = val.toString()
    }
})

const fallbackAction = computed({
    get: () => folderSettings.fallback_action || 'try_next',
    set: (val: string) => {
        folderSettings.fallback_action = val
    }
})

// Load all settings from backend
async function loadFromBackend() {
    isLoading.value = true
    error.value = null

    try {
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
        ]

        const [quality, folder, duplicate, audio, generalKV, defaultDownloadPath, defaultTemp] = await Promise.all([
            settingsApi.getQualityPreferences(),
            settingsApi.getFolderSettings(),
            settingsApi.getDuplicateSettings(),
            settingsApi.getAudioProcessingSettings(),
            settingsApi.getSettingsByKeys(generalKeys),
            settingsApi.getDefaultDownloadPath(),
            settingsApi.getDefaultTempPath(),
        ])

        // Update quality preferences
        qualityPreferences.value = quality || []

        // Update folder settings
        if (folder) {
            Object.assign(folderSettings, folder)
        }

        // Update duplicate settings
        if (duplicate) Object.assign(duplicateSettings, duplicate)

        // Update audio processing settings
        if (audio) Object.assign(audioProcessingSettings, audio)

        // Update general settings
        const kv = generalKV || {}
        if (kv.dl_concurrent_downloads) generalSettings.concurrentDownloads = kv.dl_concurrent_downloads
        if (kv.dl_retry_failed) generalSettings.retryFailed = kv.dl_retry_failed
        if (kv.dl_retry_count) generalSettings.retryCount = kv.dl_retry_count
        if (kv.dl_retry_delay) generalSettings.retryDelay = kv.dl_retry_delay
        
        const configuredDownloadPath = (kv.dl_download_path ?? kv.download_dir ?? '').trim()
        const resolvedPath = configuredDownloadPath || folder?.base_folder || defaultDownloadPath || ''
        generalSettings.downloadPath = resolvedPath
        if (folder) folderSettings.base_folder = resolvedPath

        temporaryPath.value = kv.dl_temp_dir || kv.temp_dir || defaultTemp || ''

        if (kv.dl_create_artist_folder) generalSettings.organizeByArtist = kv.dl_create_artist_folder === 'true'
        if (kv.dl_create_album_folder) generalSettings.organizeByAlbum = kv.dl_create_album_folder === 'true'
        if (kv.dl_auto_download_favorites) generalSettings.autoDownloadFavorites = kv.dl_auto_download_favorites === 'true'

        console.log('Loaded Sprint 2 settings from backend:', {
            qualityPrefs: quality?.length ?? 0,
            folderTemplate: folder?.folder_template ?? '',
            concurrentDownloads: generalSettings.concurrentDownloads,
            downloadPath: generalSettings.downloadPath,
            temporaryPath: temporaryPath.value,
            fallbackAction: folderSettings.fallback_action
        })
    } catch (e) {
        console.error('Failed to load download settings:', e)
        error.value = String(e)
    } finally {
        isLoading.value = false
    }
}

// Save general settings
async function saveGeneralSettings() {
    try {
        const mapped: Record<string, string> = {
            dl_concurrent_downloads: generalSettings.concurrentDownloads,
            dl_retry_failed: generalSettings.retryFailed,
            dl_retry_count: generalSettings.retryCount,
            dl_retry_delay: generalSettings.retryDelay,
            dl_download_path: generalSettings.downloadPath,
            download_dir: generalSettings.downloadPath,
            dl_create_artist_folder: generalSettings.organizeByArtist.toString(),
            dl_create_album_folder: generalSettings.organizeByAlbum.toString(),
            dl_auto_download_favorites: generalSettings.autoDownloadFavorites.toString(),
        }
        if (temporaryPath.value) {
            mapped.dl_temp_dir = temporaryPath.value
            mapped.temp_dir = temporaryPath.value
        }
        await settingsApi.saveSettingsBatch(mapped)
    } catch (e) {
        console.error('Failed to save general download settings:', e)
        throw e
    }
}

// Set max concurrency (1 to 5 threads) and persist
async function setMaxConcurrent(max: number) {
    const clamped = Math.max(1, Math.min(5, max))
    generalSettings.concurrentDownloads = clamped.toString()
    try {
        await settingsApi.setMaxConcurrentDownloads(clamped)
    } catch (err) {
        console.warn('Failed to set worker live concurrency:', err)
    }
    await settingsApi.saveSetting('dl_concurrent_downloads', clamped.toString())
}

// Update fallback action (e.g. 'try_next', 'skip', 'prompt')
async function updateFallbackAction(action: string) {
    folderSettings.fallback_action = action
    try {
        const updated = await settingsApi.updateFallbackAction(action)
        if (updated) Object.assign(folderSettings, updated)
    } catch (e) {
        console.error('Failed to update fallback action:', e)
        await saveFolderSettings()
    }
}

// Get quality preference for a service
function getQualityForService(serviceName: string): QualityPreference | undefined {
    return qualityPreferences.value.find(
        q => q.service_name.toLowerCase() === serviceName.toLowerCase()
    )
}

// Update quality preference for a service
async function updateQualityForService(
    serviceName: string,
    maxQuality: string,
    preferredFormat: string,
    fallbackQuality: string = 'high',
    fallbackFormat: string = 'mp3'
) {
    try {
        const updated = await settingsApi.updateQualityPreference(
            serviceName,
            maxQuality,
            preferredFormat,
            fallbackQuality,
            fallbackFormat
        )

        const idx = qualityPreferences.value.findIndex(
            q => q.service_name.toLowerCase() === serviceName.toLowerCase()
        )
        if (idx >= 0) {
            qualityPreferences.value[idx] = updated
        } else {
            qualityPreferences.value.push(updated)
        }

        return updated
    } catch (e) {
        console.error(`Failed to update quality for ${serviceName}:`, e)
        throw e
    }
}

// Update global quality preference across all services
async function updateGlobalQuality(maxQuality: string, preferredFormat?: string) {
    const knownServices = ['qobuz', 'tidal', 'spotify', 'deezer', 'apple_music', 'soundcloud']
    const format = preferredFormat || (maxQuality === 'hires' || maxQuality === 'lossless' ? 'flac' : 'mp3')
    
    await Promise.all(
        knownServices.map(async (svc) => {
            const existing = getQualityForService(svc)
            await updateQualityForService(
                svc,
                maxQuality,
                preferredFormat || existing?.preferred_format || format,
                existing?.fallback_quality || 'high',
                existing?.fallback_format || 'mp3'
            )
        })
    )
}

// Validate directory path with backend
async function validateDirectory(path: string): Promise<PathValidationResult> {
    return settingsApi.validateDirectoryPath(path)
}

// Browse and choose native download directory via Tauri dialog
async function browseDownloadDirectory(): Promise<string | null> {
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: generalSettings.downloadPath || undefined,
            title: 'Select Download Directory',
        })

        if (selected && typeof selected === 'string') {
            generalSettings.downloadPath = selected
            folderSettings.base_folder = selected
            await Promise.all([
                saveGeneralSettings(),
                saveFolderSettings(),
            ])
            return selected
        }
        return null
    } catch (e) {
        console.error('Failed to open directory dialog:', e)
        return null
    }
}

// Browse and choose native temporary staging directory via Tauri dialog
async function browseTemporaryDirectory(): Promise<string | null> {
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: temporaryPath.value || undefined,
            title: 'Select Temporary Staging Directory',
        })

        if (selected && typeof selected === 'string') {
            temporaryPath.value = selected
            await saveGeneralSettings()
            return selected
        }
        return null
    } catch (e) {
        console.error('Failed to open temporary directory dialog:', e)
        return null
    }
}

// Reset download path to OS-aware default
async function resetDownloadPath(): Promise<string> {
    try {
        const defaultPath = await settingsApi.getDefaultDownloadPath()
        if (defaultPath) {
            generalSettings.downloadPath = defaultPath
            folderSettings.base_folder = defaultPath
            await Promise.all([
                saveGeneralSettings(),
                saveFolderSettings(),
            ])
            return defaultPath
        }
        return generalSettings.downloadPath
    } catch (e) {
        console.error('Failed to reset download path:', e)
        throw e
    }
}

// Reset temporary staging path
async function resetTemporaryPath(): Promise<string> {
    try {
        const defaultTemp = await settingsApi.getDefaultTempPath()
        if (defaultTemp) {
            temporaryPath.value = defaultTemp
            await saveGeneralSettings()
            return defaultTemp
        }
        return temporaryPath.value
    } catch (e) {
        console.error('Failed to reset temporary path:', e)
        throw e
    }
}

// Save folder settings
async function saveFolderSettings() {
    try {
        folderSettings.base_folder = generalSettings.downloadPath
        const updated = await settingsApi.updateFolderSettings({ ...folderSettings })
        Object.assign(folderSettings, updated)
        return updated
    } catch (e) {
        console.error('Failed to save folder settings:', e)
        throw e
    }
}

// Consolidated save download settings
async function saveDownloadSettings() {
    try {
        await Promise.all([
            saveGeneralSettings(),
            saveFolderSettings(),
        ])
    } catch (e) {
        console.error('Failed to save consolidated download settings:', e)
        throw e
    }
}

// Save duplicate settings
async function saveDuplicateSettings() {
    try {
        const updated = await settingsApi.updateDuplicateSettings({ ...duplicateSettings })
        Object.assign(duplicateSettings, updated)
        return updated
    } catch (e) {
        console.error('Failed to save duplicate settings:', e)
        throw e
    }
}

// Save audio processing settings
async function saveAudioProcessingSettings() {
    try {
        const updated = await settingsApi.updateAudioProcessingSettings({ ...audioProcessingSettings })
        Object.assign(audioProcessingSettings, updated)
        return updated
    } catch (e) {
        console.error('Failed to save audio processing settings:', e)
        throw e
    }
}

// Preview folder path for a track
async function previewPath(trackId: number): Promise<string> {
    try {
        return await settingsApi.previewFolderPath(trackId)
    } catch (e) {
        console.error('Failed to preview folder path:', e)
        throw e
    }
}

export function useDownloadSettings() {
    return {
        // State
        isLoading,
        error,
        qualityPreferences,
        folderSettings,
        generalSettings,
        temporaryPath,
        duplicateSettings,
        audioProcessingSettings,

        // Computed
        downloadPath,
        concurrentDownloads,
        fallbackAction,

        // Load
        loadSettings: loadFromBackend,

        // Download Location & General
        validateDirectory,
        browseDownloadDirectory,
        browseTemporaryDirectory,
        resetDownloadPath,
        resetTemporaryPath,
        setMaxConcurrent,
        updateFallbackAction,
        saveGeneralSettings,
        saveDownloadSettings,

        // Quality
        getQualityForService,
        updateQualityForService,
        updateGlobalQuality,

        // Folder
        saveFolderSettings,
        previewPath,

        // Duplicate
        saveDuplicateSettings,

        // Audio Processing
        saveAudioProcessingSettings,
    }
}
