// useDownloadSettings.ts - Manages Sprint 2 download and file settings
// Integrates with backend database via Tauri commands

import { reactive, ref, computed, watch } from 'vue'
import { settingsApi } from '@/api/settings'
import type {
    QualityPreference,
    FolderSettings,
    DuplicateSettings,
    AudioProcessingSettings
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
            'dl_create_artist_folder',
            'dl_create_album_folder',
            'dl_auto_download_favorites'
        ]

        const [quality, folder, duplicate, audio, generalKV, defaultDownloadPath] = await Promise.all([
            settingsApi.getQualityPreferences(),
            settingsApi.getFolderSettings(),
            settingsApi.getDuplicateSettings(),
            settingsApi.getAudioProcessingSettings(),
            settingsApi.getSettingsByKeys(generalKeys),
            settingsApi.getDefaultDownloadPath(),
        ])

        // Update quality preferences
        qualityPreferences.value = quality || []

        // Update folder settings
        if (folder) Object.assign(folderSettings, folder)

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
        const configuredDownloadPath = (kv.dl_download_path ?? '').trim()
        generalSettings.downloadPath = configuredDownloadPath || defaultDownloadPath || ''
        if (kv.dl_create_artist_folder) generalSettings.organizeByArtist = kv.dl_create_artist_folder === 'true'
        if (kv.dl_create_album_folder) generalSettings.organizeByAlbum = kv.dl_create_album_folder === 'true'
        if (kv.dl_auto_download_favorites) generalSettings.autoDownloadFavorites = kv.dl_auto_download_favorites === 'true'

        console.log('Loaded Sprint 2 settings from backend:', {
            qualityPrefs: quality?.length ?? 0,
            folderTemplate: folder?.folder_template ?? '',
            concurrentDownloads: generalSettings.concurrentDownloads
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
            dl_create_artist_folder: generalSettings.organizeByArtist.toString(),
            dl_create_album_folder: generalSettings.organizeByAlbum.toString(),
            dl_auto_download_favorites: generalSettings.autoDownloadFavorites.toString(),
        }
        await settingsApi.saveSettingsBatch(mapped)
    } catch (e) {
        console.error('Failed to save general download settings:', e)
        throw e
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
    fallbackQuality: string,
    fallbackFormat: string
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
        }

        return updated
    } catch (e) {
        console.error(`Failed to update quality for ${serviceName}:`, e)
        throw e
    }
}

// Save folder settings
async function saveFolderSettings() {
    try {
        const updated = await settingsApi.updateFolderSettings({ ...folderSettings })
        Object.assign(folderSettings, updated)
        return updated
    } catch (e) {
        console.error('Failed to save folder settings:', e)
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
        duplicateSettings,
        audioProcessingSettings,

        // Load
        loadSettings: loadFromBackend,

        // Quality
        getQualityForService,
        updateQualityForService,

        // Folder
        saveFolderSettings,
        previewPath,

        // General
        saveGeneralSettings,

        // Duplicate
        saveDuplicateSettings,

        // Audio Processing
        saveAudioProcessingSettings,
    }
}
