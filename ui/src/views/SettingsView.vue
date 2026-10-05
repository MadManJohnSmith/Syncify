<template>
  <div class="h-full flex bg-background-light dark:bg-background-dark overflow-hidden">
    <!-- Settings Sidebar -->
    <aside class="w-64 flex-shrink-0 border-r border-gray-200 dark:border-border-dark flex flex-col bg-white/50 dark:bg-sidebar/50 backdrop-blur-sm">
      <div class="p-6 pb-4">
        <h1 class="text-2xl font-bold tracking-tight text-gray-900 dark:text-white">Settings</h1>
      </div>
      <nav class="flex-1 overflow-y-auto px-3 pb-4 space-y-1 custom-scrollbar">
        <button 
          v-for="item in settingsCategories" 
          :key="item.id"
          @click="activeCategory = item.id"
          :class="[
            'w-full flex items-center gap-3 px-3 py-2 text-sm font-medium rounded-lg transition-colors',
            activeCategory === item.id 
              ? 'bg-primary text-white shadow-lg shadow-primary/20' 
              : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-white/5 hover:text-gray-900 dark:hover:text-white'
          ]"
        >
          <span class="material-symbols-outlined text-[20px]">{{ item.icon }}</span>
          {{ item.name }}
        </button>
      </nav>
      <!-- Bottom Actions -->
      <div class="p-4 border-t border-gray-200 dark:border-border-dark bg-gray-50/50 dark:bg-[#0f1520]/50 space-y-3">
        <button 
          @click="handleSaveChanges"
          :disabled="savingSettings || isLoading"
          class="w-full py-2 px-4 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium shadow-lg shadow-primary/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2">
          <span v-if="savingSettings" class="material-symbols-outlined text-sm animate-spin">sync</span>
          {{ savingSettings ? 'Saving...' : 'Save Changes' }}
        </button>
        <button 
          @click="handleResetToDefaults"
          :disabled="savingSettings || isLoading"
          class="w-full py-2 px-4 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed">
          Reset to Defaults
        </button>
        <div v-if="lastSavedText" class="text-center">
           <span class="text-xs text-text-secondary">{{ lastSavedText }}</span>
        </div>
      </div>
    </aside>

    <!-- Settings Content Panel -->
    <main class="flex-1 overflow-y-auto custom-scrollbar p-8">
      <div class="max-w-4xl mx-auto space-y-8">
        
        <!-- Headers for Active Category -->
        <div>
          <h2 class="text-2xl font-bold text-gray-900 dark:text-white">{{ activeCategoryName }}</h2>
          <p class="text-text-secondary mt-1">{{ activeCategoryDescription }}</p>
        </div>

        <!-- Global Loader -->
        <div v-if="isLoading" class="flex flex-col items-center justify-center py-20 animate-in fade-in duration-500">
          <div class="relative w-16 h-16">
            <div class="absolute inset-0 rounded-full border-4 border-primary/20"></div>
            <div class="absolute inset-0 rounded-full border-4 border-primary border-t-transparent animate-spin"></div>
          </div>
          <p class="mt-4 text-gray-600 dark:text-gray-400 font-medium">Loading settings...</p>
        </div>

        <!-- Settings Content (v-else) -->
        <template v-else>
          <SettingsGeneral v-if="activeCategory === 'general'" />
          <SettingsServices v-if="activeCategory === 'services'" />
          <SettingsQuality v-if="activeCategory === 'quality'" />
          <SettingsMetadata v-if="activeCategory === 'metadata'" />
          <SettingsLyrics v-if="activeCategory === 'lyrics'" />
          <SettingsDuplicates v-if="activeCategory === 'duplicates'" />
          <SettingsProcessing v-if="activeCategory === 'processing'" />
          <SettingsDownloads v-if="activeCategory === 'folders'" />
          <SettingsSync v-if="activeCategory === 'sync'" />
          <SettingsBackup v-if="activeCategory === 'backup'" />
          <SettingsAdvanced v-if="activeCategory === 'advanced'" />
        </template>
        </div>
      </main>
    </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute } from 'vue-router'
import { settingsApi } from '@/api/settings'
// R13: confirm unificado (antes plugin-dialog nativo, fuera de la ventana).
import { confirm } from '@/composables/useToast'
import { useToast } from '@/composables/useToast'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import { useLyricsSettings } from '@/composables/useLyricsSettings'
import { useAdvancedSettings } from '@/composables/useAdvancedSettings'
import { useGeneralSettings } from '@/composables/useGeneralSettings'
import { useMetadataSettings } from '@/composables/useMetadataSettings'
import { useSyncSettings } from '@/composables/useSyncSettings'
import SettingsMetadata from './settings/SettingsMetadata.vue'
import SettingsSync from './settings/SettingsSync.vue'
import SettingsDownloads from './settings/SettingsDownloads.vue'
import SettingsQuality from './settings/SettingsQuality.vue'
import SettingsGeneral from './settings/SettingsGeneral.vue'
import SettingsServices from './settings/SettingsServices.vue'
import SettingsLyrics from './settings/SettingsLyrics.vue'
import SettingsDuplicates from './settings/SettingsDuplicates.vue'
import SettingsProcessing from './settings/SettingsProcessing.vue'
import SettingsBackup from './settings/SettingsBackup.vue'
import SettingsAdvanced from './settings/SettingsAdvanced.vue'

const route = useRoute()
const toast = useToast()
const downloadSettings = useDownloadSettings()
const lyricsSettings = useLyricsSettings()
const advancedSettings = useAdvancedSettings()
const metadataSettings = useMetadataSettings()
const generalSettings = useGeneralSettings()
const syncSettings = useSyncSettings()

const savingSettings = ref(false)
const isLoading = ref(true)
const lastSavedTimestamp = ref<Date | null>(null)

const lastSavedText = computed(() => {
  if (!lastSavedTimestamp.value) return null
  return `Last saved: ${lastSavedTimestamp.value.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })}`
})

// Backend state
const healthStatus = ref<{ database_ok: boolean; python_ok: boolean; ffmpeg_available: boolean; chromaprint_available: boolean; services_configured: string[]; errors: string[] } | null>(null)

async function handleSaveChanges() {
  if (savingSettings.value) return
  savingSettings.value = true
  try {
    await Promise.all([
      generalSettings.saveSettings(),
      metadataSettings.saveSettings(),
      advancedSettings.saveSettings(),
      downloadSettings.saveDownloadSettings(),
      lyricsSettings.saveConfig(),
      syncSettings.saveGlobalSettings()
    ])
    await Promise.all([
      generalSettings.loadSettings(),
      downloadSettings.loadSettings(),
      lyricsSettings.loadSettings(),
      syncSettings.loadSettings()
    ])
    lastSavedTimestamp.value = new Date()
    toast.success('Settings Saved', 'All configuration settings have been saved successfully.')
  } catch (err) {
    console.error('Failed to save settings:', err)
    const errorMsg = err instanceof Error ? err.message : String(err)
    toast.error('Failed to save settings', errorMsg || 'An error occurred while saving settings.')
  } finally {
    savingSettings.value = false
  }
}

async function handleResetToDefaults() {
  const confirmed = await confirm('Reset all settings to their default values? This cannot be undone.', {
    title: 'Reset to Defaults',
    variant: 'warning'
  })
  if (confirmed !== true) return
  isLoading.value = true
  try {
    // The confirmation promises *every* category, and the backend `all` branch
    // only wipes six of them, so the rest are restored explicitly below.
    await generalSettings.resetToDefaults()
    await resetCategoryDefaults()
    await Promise.all([
      generalSettings.loadSettings(),
      downloadSettings.loadSettings(),
      lyricsSettings.loadSettings(),
      metadataSettings.loadSettings(),
      syncSettings.loadSettings(),
      advancedSettings.loadSettings()
    ])
    toast.success('Settings Reset', 'All settings are back to their default values.')
  } catch (err) {
    console.error('Failed to reset settings:', err)
    const errorMsg = err instanceof Error ? err.message : String(err)
    toast.error('Failed to reset settings', errorMsg || 'An error occurred while resetting settings.')
  } finally {
    isLoading.value = false
  }
}

// Defaults mirror the schema seeds so a reset lands on the same state a fresh
// install has; each source is named because the backend `all` branch leaves
// these tables untouched.
const QUALITY_DEFAULTS: Record<string, { max_quality: string; preferred_format: string; fallback_quality: string; fallback_format: string }> = {
  spotify: { max_quality: 'high', preferred_format: 'ogg', fallback_quality: 'medium', fallback_format: 'ogg' },
  qobuz: { max_quality: 'hires', preferred_format: 'flac', fallback_quality: 'lossless', fallback_format: 'flac' },
  tidal: { max_quality: 'master', preferred_format: 'flac', fallback_quality: 'hifi', fallback_format: 'flac' },
  deezer: { max_quality: 'lossless', preferred_format: 'flac', fallback_quality: 'high', fallback_format: 'mp3' },
  soundcloud: { max_quality: 'high', preferred_format: 'mp3', fallback_quality: 'medium', fallback_format: 'mp3' },
}
// src-tauri/migrations/0009_quality_format_settings.sql column defaults, used
// for services seeded by a later migration (e.g. apple_music).
const QUALITY_COLUMN_DEFAULTS = { max_quality: 'lossless', preferred_format: 'flac', fallback_quality: 'high', fallback_format: 'mp3' }

// src-tauri/migrations/0007_service_preferences.sql
const SERVICE_PRIORITY_DEFAULTS = ['spotify', 'qobuz', 'tidal', 'deezer', 'soundcloud']

// src-tauri/migrations/0013_lyrics_provider_settings.sql
const LYRICS_PROVIDER_DEFAULTS = ['apple_music', 'lrclib', 'netease', 'genius']

// src-tauri/migrations/0021_metadata_preferences.sql + 0022 + 0023
const METADATA_DEFAULTS = {
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
}

async function resetCategoryDefaults() {
  const failures: string[] = []

  const quality = await settingsApi.getQualityPreferences().catch(() => [])
  for (const pref of quality) {
    const defaults = QUALITY_DEFAULTS[pref.service_name] ?? QUALITY_COLUMN_DEFAULTS
    await settingsApi
      .updateQualityPreference(
        pref.service_name,
        defaults.max_quality,
        defaults.preferred_format,
        defaults.fallback_quality,
        defaults.fallback_format
      )
      .catch(() => failures.push(`quality:${pref.service_name}`))
  }

  await settingsApi
    .updateMetadataPreferences(METADATA_DEFAULTS)
    .catch(() => failures.push('metadata'))

  // Priorities only reorder the rows that already exist; sending the seed list
  // alone would silently drop a service the user connected.
  const preferences = await settingsApi.getServicePreferences().catch(() => [])
  const known = preferences.map(p => p.service_name)
  const seedOrder = SERVICE_PRIORITY_DEFAULTS.filter(name => known.includes(name))
  const remaining = known.filter(name => !seedOrder.includes(name))
  await syncSettings
    .reorderPriorities([...seedOrder, ...remaining])
    .catch(() => failures.push('service-priority'))
  for (const pref of preferences) {
    if (pref.auto_import_enabled) {
      await settingsApi
        .updateServicePreference(pref.service_name, false)
        .catch(() => failures.push(`auto-import:${pref.service_name}`))
    }
  }

  const providers = await settingsApi.getLyricsProviders().catch(() => [])
  const providerSeed = LYRICS_PROVIDER_DEFAULTS.filter(id => providers.some(p => p.provider_id === id))
  const providerRest = providers.map(p => p.provider_id).filter(id => !providerSeed.includes(id))
  if (providers.length > 0) {
    const seededOrder = [...providerSeed, ...providerRest]
    await settingsApi
      .reorderLyricsProviders(seededOrder)
      .catch(() => failures.push('lyrics-priority'))
    for (const [index, provider] of providers.entries()) {
      if (!provider.enabled || provider.priority !== index + 1) {
        await settingsApi
          .updateLyricsProvider(provider.provider_id, true, index + 1)
          .catch(() => failures.push(`lyrics:${provider.provider_id}`))
      }
    }
  }

  if (failures.length > 0) {
    // The backend tables are already wiped at this point, so report instead of
    // pretending the reset was complete.
    throw new Error(`Could not reset: ${failures.join(', ')}`)
  }
}

const settingsCategories = [
  { id: 'general', name: 'General', icon: 'tune', desc: 'Application behavior and storage paths' },
  { id: 'services', name: 'Services & Priorities', icon: 'hub', desc: 'Manage connections and download order' },
  { id: 'quality', name: 'Audio Quality', icon: 'high_quality', desc: 'Format preferences and limits' },
  { id: 'metadata', name: 'Metadata & Tags', icon: 'tag', desc: 'Tagging sources and rules' },
  { id: 'lyrics', name: 'Lyrics', icon: 'lyrics', desc: 'Lyrics providers and storage' },
  { id: 'duplicates', name: 'Duplicates', icon: 'content_copy', desc: 'Detection and upgrade policy' },
  { id: 'processing', name: 'Audio Processing', icon: 'graphic_eq', desc: 'Normalization and transcoding' },
  { id: 'folders', name: 'Folder Structure', icon: 'folder_open', desc: 'Naming templates and organization' },
  { id: 'sync', name: 'Sync & Scheduling', icon: 'sync', desc: 'Auto-sync and intervals' },
  { id: 'backup', name: 'Backup & Restore', icon: 'backup', desc: 'Export & import full library backups' },
  { id: 'advanced', name: 'Advanced', icon: 'terminal', desc: 'Database, networking, and debug' },
]

const initialCategory = typeof route?.query?.tab === 'string' 
  ? route.query.tab 
  : (typeof route?.query?.category === 'string' ? route.query.category : 'general')

const activeCategory = ref(settingsCategories.some(c => c.id === initialCategory) ? initialCategory : 'general')

watch(() => route?.query?.tab || route?.query?.category, (newTab) => {
  if (typeof newTab === 'string' && settingsCategories.some(c => c.id === newTab)) {
    activeCategory.value = newTab
  }
})

const activeCategoryName = computed(() => {
  return settingsCategories.find(c => c.id === activeCategory.value)?.name || 'Settings'
})
const activeCategoryDescription = computed(() => {
  return settingsCategories.find(c => c.id === activeCategory.value)?.desc || ''
})

// Lifecycle
onMounted(async () => {
  try {
    await Promise.all([
      generalSettings.loadSettings(),
      downloadSettings.loadSettings(),
      lyricsSettings.loadSettings(),
      advancedSettings.loadSettings(),
      syncSettings.loadSettings()
    ])

    const health = await settingsApi.runHealthCheck()
    healthStatus.value = health
    lastSavedTimestamp.value = new Date()
  } catch (err) {
    console.error('Failed to initialize settings:', err)
  } finally {
    isLoading.value = false
  }
})
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}
</style>
