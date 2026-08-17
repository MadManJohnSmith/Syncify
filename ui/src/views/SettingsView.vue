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
          :disabled="savingSettings"
          class="w-full py-2 px-4 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium shadow-lg shadow-primary/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed">
          {{ savingSettings ? 'Saving...' : 'Save Changes' }}
        </button>
        <button 
          @click="handleResetToDefaults"
          :disabled="savingSettings"
          class="w-full py-2 px-4 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-all disabled:opacity-50 disabled:cursor-not-allowed">
          Reset to Defaults
        </button>
        <div class="text-center">
           <span class="text-xs text-text-secondary">Last saved: Just now</span>
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
import { ref, computed, onMounted } from 'vue'
import { settingsApi } from '@/api/settings'
import { confirm } from '@tauri-apps/plugin-dialog'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import { useLyricsSettings } from '@/composables/useLyricsSettings'
import { useAdvancedSettings } from '@/composables/useAdvancedSettings'
import { useGeneralSettings } from '@/composables/useGeneralSettings'
import { useMetadataSettings } from '@/composables/useMetadataSettings'
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

const downloadSettings = useDownloadSettings()
const lyricsSettings = useLyricsSettings()
const advancedSettings = useAdvancedSettings()
const metadataSettings = useMetadataSettings()
const generalSettings = useGeneralSettings()

const savingSettings = ref(false)
const isLoading = ref(true)

// Backend state
const healthStatus = ref<{ database_ok: boolean; python_ok: boolean; ffmpeg_available: boolean; chromaprint_available: boolean; services_configured: string[]; errors: string[] } | null>(null)

async function handleSaveChanges() {
  savingSettings.value = true
  try {
    await Promise.all([
      generalSettings.saveSettings(),
      metadataSettings.saveSettings(),
      advancedSettings.saveSettings(),
      downloadSettings.saveFolderSettings()
    ])
  } catch (err) {
    console.error('Failed to save settings:', err)
  } finally {
    savingSettings.value = false
  }
}

async function handleResetToDefaults() {
  const confirmed = await confirm('Reset all settings to their default values? This cannot be undone.', {
    title: 'Reset to Defaults',
    kind: 'warning'
  })
  if (confirmed !== true) return
  await generalSettings.resetToDefaults()
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

const activeCategory = ref('general')

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
      advancedSettings.loadSettings()
    ])

    const health = await settingsApi.runHealthCheck()
    healthStatus.value = health
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
