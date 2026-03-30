// useMetadataSettings.ts - Manages Sprint 14 metadata and tagging settings
// Integrates with backend database via Tauri commands

import { reactive, ref } from 'vue'
import { settingsApi } from '@/api/settings'
import type { MetadataPreferences } from '@/api/types'

// Singleton state
const isLoading = ref(true)
const error = ref<string | null>(null)

// Metadata settings (singleton)
const settings = reactive<MetadataPreferences>({
    id: 1,
    enable_musicbrainz: true,
    enable_lastfm: false,
    enable_acoustid: false,
    overwrite_on_reimport: false,
    preserve_custom_tags: true,
    multi_value_separator: ';',
    write_releasetype: true,
    write_label: true,
    write_work_composer: false,
    write_musicbrainz_ids: true,
    write_download_source: false,
    write_download_date: false,
    write_only_available_on: false,
    write_not_available_streaming: false,
    write_quality_score: false,
    write_lyrics_tags: false,
    weight_album: 1,
    weight_isrc: 1,
    weight_mb_id: 1,
    weight_cover: 1,
    weight_year: 1,
    weight_genre: 1,
})

// Load all settings from backend
async function loadFromBackend() {
    isLoading.value = true
    error.value = null

    try {
        const data = await settingsApi.getMetadataPreferences()
        Object.assign(settings, data)
        console.log('useMetadataSettings: loaded', JSON.parse(JSON.stringify(settings)))
    } catch (e) {
        console.error('Failed to load metadata settings:', e)
        error.value = String(e)
    } finally {
        isLoading.value = false
    }
}

// Save all settings to backend
async function saveToBackend() {
    isLoading.value = true
    error.value = null

    try {
        const updated = await settingsApi.updateMetadataPreferences({ ...settings })
        Object.assign(settings, updated)
        return updated
    } catch (e) {
        console.error('Failed to save metadata settings:', e)
        error.value = String(e)
        throw e
    } finally {
        isLoading.value = false
    }
}

export function useMetadataSettings() {
    return {
        // State
        isLoading,
        error,
        settings,

        // Actions
        loadSettings: loadFromBackend,
        saveSettings: saveToBackend,
    }
}
