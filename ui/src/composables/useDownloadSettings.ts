// useDownloadSettings.ts - Manages Sprint 2 download and file settings
// Integrates with backend database via Tauri commands

import { reactive, ref, computed } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import {
    settingsApi,
    type PathValidationResult,
    type PathStatus,
    type DownloadSettingsDto,
    type EffectiveDownloadPreferences,
    deriveStagingRoot,
    determinePathStatus
} from '@/api/settings'
import { useEventBus } from '@/composables/useEventBus'
import type {
    QualityPreference,
    FolderSettings,
    DuplicateSettings,
    AudioProcessingSettings,
} from '@/api/types'

// Singleton reactive state
const isLoading = ref(true)
const error = ref<string | null>(null)

// Unified DownloadSettingsDto state (Single Source of Truth)
const downloadDto = reactive<DownloadSettingsDto>({
    library_root: '',
    staging_root: '',
    path_status: 'valid',
    free_space_bytes: null,
})

// Last known valid library root
const lastValidLibraryRoot = ref<string>('')

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
    organizeByArtist: true,
    organizeByAlbum: true,
    autoDownloadFavorites: false,
    maxSpeed: 0,
    pauseOnMetered: false,
})

// Computed helper bindings
const libraryRoot = computed({
    get: () => downloadDto.library_root,
    set: (val: string) => {
        downloadDto.library_root = val
        downloadDto.staging_root = deriveStagingRoot(val)
        folderSettings.base_folder = val
    }
})

// downloadPath alias for backward compatibility
const downloadPath = computed({
    get: () => downloadDto.library_root,
    set: (val: string) => {
        libraryRoot.value = val
    }
})

// temporaryPath alias for staging root
const temporaryPath = computed({
    get: () => downloadDto.staging_root,
    set: (val: string) => {
        downloadDto.staging_root = val
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

/**
 * Validate path status and update free space bytes in downloadDto
 */
async function validateAndRefreshPath(path: string): Promise<PathValidationResult> {
    if (!path || !path.trim()) {
        downloadDto.path_status = 'missing'
        downloadDto.free_space_bytes = null
        return {
            valid: false,
            exists: false,
            is_dir: false,
            is_writable: false,
            available_bytes: 0,
            drive_mounted: false,
            canonical_path: '',
            error_message: 'Path is required',
        }
    }

    try {
        const validation = await settingsApi.validateDirectoryPath(path)
        if (validation) {
            const status = determinePathStatus(validation)
            downloadDto.path_status = status
            downloadDto.free_space_bytes = validation.available_bytes ?? null

            if (status === 'valid') {
                lastValidLibraryRoot.value = path
            }
            return validation
        }
    } catch {
        // Fallback when validator is unavailable
    }

    downloadDto.path_status = 'valid'
    lastValidLibraryRoot.value = path
    return {
        valid: true,
        exists: true,
        is_dir: true,
        is_writable: true,
        available_bytes: 0,
        drive_mounted: true,
        canonical_path: path,
        error_message: null,
    }
}

// Load all settings from backend
async function loadFromBackend() {
    isLoading.value = true
    error.value = null

    try {
        // Try loading canonical effective preferences first
        try {
            const effective = await settingsApi.getEffectiveDownloadPreferences()
            if (effective) {
                qualityPreferences.value = effective.serviceQualities || []
                folderSettings.base_folder = effective.downloadPath
                folderSettings.folder_template = effective.folderTemplate
                folderSettings.file_template = effective.fileTemplate
                folderSettings.artist_separator = effective.artistSeparator
                folderSettings.replace_spaces_with = effective.replaceSpacesWith
                folderSettings.max_path_length = effective.maxPathLength
                folderSettings.fallback_action = effective.fallbackAction

                downloadDto.library_root = effective.downloadPath
                downloadDto.staging_root = effective.stagingPath
                downloadDto.path_status = (effective.pathStatus as PathStatus) || 'valid'
                downloadDto.free_space_bytes = effective.freeSpaceBytes

                generalSettings.concurrentDownloads = effective.maxConcurrentDownloads.toString()
                generalSettings.retryCount = effective.maxRetries.toString()
                generalSettings.retryFailed = effective.maxRetries.toString()
                generalSettings.retryDelay = (effective.retryDelaySeconds * 1000).toString()
                generalSettings.autoDownloadFavorites = effective.autoDownloadFavorites

                if (effective.downloadPath) {
                    lastValidLibraryRoot.value = effective.downloadPath
                }

                // Also load auxiliary singleton tables
                const [duplicate, audio] = await Promise.all([
                    settingsApi.getDuplicateSettings().catch(() => null),
                    settingsApi.getAudioProcessingSettings().catch(() => null),
                ])
                if (duplicate) Object.assign(duplicateSettings, duplicate)
                if (audio) Object.assign(audioProcessingSettings, audio)

                return
            }
        } catch {
            // Fall back to legacy multi-endpoint loading
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
        ]

        const [quality, folder, duplicate, audio, generalKV, defaultDownloadPath, unifiedDto] = await Promise.all([
            settingsApi.getQualityPreferences().catch(() => []),
            settingsApi.getFolderSettings().catch(() => null),
            settingsApi.getDuplicateSettings().catch(() => null),
            settingsApi.getAudioProcessingSettings().catch(() => null),
            settingsApi.getSettingsByKeys(generalKeys).catch(() => ({} as Record<string, string>)),
            settingsApi.getDefaultDownloadPath().catch(() => ''),
            settingsApi.getUnifiedDownloadSettings().catch(() => null),
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
        const kv = (generalKV || {}) as Record<string, string>
        if (kv.dl_concurrent_downloads) generalSettings.concurrentDownloads = kv.dl_concurrent_downloads
        if (kv.dl_retry_failed) generalSettings.retryFailed = kv.dl_retry_failed
        if (kv.dl_retry_count) generalSettings.retryCount = kv.dl_retry_count
        if (kv.dl_retry_delay) generalSettings.retryDelay = kv.dl_retry_delay

        const configuredDownloadPath = (kv.dl_download_path ?? kv.download_dir ?? '').trim()
        const resolvedLibraryRoot = unifiedDto?.library_root || configuredDownloadPath || folder?.base_folder || defaultDownloadPath || ''
        const resolvedStaging = unifiedDto?.staging_root || (kv.dl_temp_dir ?? kv.temp_dir ?? '').trim() || deriveStagingRoot(resolvedLibraryRoot)

        downloadDto.library_root = resolvedLibraryRoot
        downloadDto.staging_root = resolvedStaging
        if (folder) folderSettings.base_folder = resolvedLibraryRoot

        if (resolvedLibraryRoot) {
            await validateAndRefreshPath(resolvedLibraryRoot)
        } else {
            downloadDto.path_status = 'missing'
            downloadDto.free_space_bytes = null
        }

        if (kv.dl_create_artist_folder) generalSettings.organizeByArtist = kv.dl_create_artist_folder === 'true'
        if (kv.dl_create_album_folder) generalSettings.organizeByAlbum = kv.dl_create_album_folder === 'true'
        if (kv.dl_auto_download_favorites) generalSettings.autoDownloadFavorites = kv.dl_auto_download_favorites === 'true'

        console.log('Loaded unified download settings:', {
            library_root: downloadDto.library_root,
            staging_root: downloadDto.staging_root,
            path_status: downloadDto.path_status,
            free_space_bytes: downloadDto.free_space_bytes,
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
        const root = downloadDto.library_root
        const staging = downloadDto.staging_root || deriveStagingRoot(root)
        const mapped: Record<string, string> = {
            dl_concurrent_downloads: generalSettings.concurrentDownloads,
            dl_retry_failed: generalSettings.retryFailed,
            dl_retry_count: generalSettings.retryCount,
            dl_retry_delay: generalSettings.retryDelay,
            dl_download_path: root,
            download_dir: root,
            dl_create_artist_folder: generalSettings.organizeByArtist.toString(),
            dl_create_album_folder: generalSettings.organizeByAlbum.toString(),
            dl_auto_download_favorites: generalSettings.autoDownloadFavorites.toString(),
        }
        if (staging) {
            mapped.dl_temp_dir = staging
            mapped.temp_dir = staging
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
    const eventBus = useEventBus()
    eventBus.emit('download-settings-updated', { concurrentDownloads: clamped })
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
    const eventBus = useEventBus()
    eventBus.emit('download-settings-updated', { fallbackAction: action })
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

        const eventBus = useEventBus()
        eventBus.emit('quality-settings-updated', { serviceName, maxQuality, preferredFormat })
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

    const eventBus = useEventBus()
    eventBus.emit('quality-settings-updated', { global: true, maxQuality, preferredFormat: format })
}

// Validate directory path with backend
async function validateDirectory(path: string): Promise<PathValidationResult> {
    return validateAndRefreshPath(path)
}

// Set library root and validate
async function setLibraryRoot(path: string): Promise<PathValidationResult> {
    const validation = await validateAndRefreshPath(path)
    if (validation.valid) {
        downloadDto.library_root = path
        downloadDto.staging_root = deriveStagingRoot(path)
        folderSettings.base_folder = path
        await Promise.all([
            saveGeneralSettings(),
            saveFolderSettings(),
        ])
    }
    return validation
}

// Browse and choose native download directory via Tauri dialog
async function browseDownloadDirectory(): Promise<string | null> {
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            defaultPath: downloadDto.library_root || undefined,
            title: 'Select Download Directory',
        })

        if (selected && typeof selected === 'string') {
            const validation = await validateAndRefreshPath(selected)
            if (validation.valid) {
                downloadDto.library_root = selected
                downloadDto.staging_root = deriveStagingRoot(selected)
                folderSettings.base_folder = selected
                await Promise.all([
                    saveGeneralSettings(),
                    saveFolderSettings(),
                ])
                return selected
            } else {
                console.warn(`Selected path invalid (${validation.error_message}), retaining last valid: ${lastValidLibraryRoot.value}`)
                // Retain last valid path in library_root
                if (lastValidLibraryRoot.value) {
                    downloadDto.library_root = lastValidLibraryRoot.value
                    downloadDto.staging_root = deriveStagingRoot(lastValidLibraryRoot.value)
                }
                return null
            }
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
            defaultPath: downloadDto.staging_root || undefined,
            title: 'Select Temporary Staging Directory',
        })

        if (selected && typeof selected === 'string') {
            downloadDto.staging_root = selected
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
            downloadDto.library_root = defaultPath
            downloadDto.staging_root = deriveStagingRoot(defaultPath)
            folderSettings.base_folder = defaultPath
            await validateAndRefreshPath(defaultPath)
            await Promise.all([
                saveGeneralSettings(),
                saveFolderSettings(),
            ])
            return defaultPath
        }
        return downloadDto.library_root
    } catch (e) {
        console.error('Failed to reset download path:', e)
        throw e
    }
}

// Reset temporary staging path
async function resetTemporaryPath(): Promise<string> {
    try {
        const derived = deriveStagingRoot(downloadDto.library_root)
        downloadDto.staging_root = derived
        await saveGeneralSettings()
        return derived
    } catch (e) {
        console.error('Failed to reset temporary path:', e)
        throw e
    }
}

// Save folder settings
async function saveFolderSettings() {
    try {
        folderSettings.base_folder = downloadDto.library_root
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
        await loadFromBackend()
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

// Subscribe to sync settings updates to keep concurrency in sync
const eventBus = useEventBus()
eventBus.on<{ globalSettings?: { maxConcurrentDownloads?: number } }>('sync-settings-updated', (payload) => {
    if (payload?.globalSettings?.maxConcurrentDownloads) {
        generalSettings.concurrentDownloads = payload.globalSettings.maxConcurrentDownloads.toString()
    }
})

export function useDownloadSettings() {
    return {
        // State
        isLoading,
        error,
        downloadDto,
        lastValidLibraryRoot,
        qualityPreferences,
        folderSettings,
        generalSettings,
        temporaryPath,
        duplicateSettings,
        audioProcessingSettings,

        // Computed
        libraryRoot,
        downloadPath,
        concurrentDownloads,
        fallbackAction,

        // Load
        loadSettings: loadFromBackend,

        // Download Location & General
        validateDirectory,
        setLibraryRoot,
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
