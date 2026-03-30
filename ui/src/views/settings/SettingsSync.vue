<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Auto-Sync</h3>
      
      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <span class="font-medium text-gray-900 dark:text-white">Enable automatic library sync</span>
        <button @click="syncSettings.globalSettings.autoSyncEnabled = !syncSettings.globalSettings.autoSyncEnabled; syncSettings.saveGlobalSettings()" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', syncSettings.globalSettings.autoSyncEnabled ? 'bg-primary' : 'bg-gray-300']">
          <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', syncSettings.globalSettings.autoSyncEnabled ? 'translate-x-6' : 'translate-x-1']"></span>
        </button>
      </div>

      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <span class="font-medium text-gray-900 dark:text-white">Sync interval</span>
        <div class="flex gap-2">
          <input type="number" v-model.number="syncSettings.globalSettings.syncIntervalValue" @change="syncSettings.saveGlobalSettings()" class="w-16 px-2 py-1 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-900 dark:text-white" min="1">
          <select v-model="syncSettings.globalSettings.syncIntervalUnit" @change="syncSettings.saveGlobalSettings()" class="px-2 py-1 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-900 dark:text-white">
            <option value="hours">hours</option>
            <option value="days">days</option>
            <option value="weeks">weeks</option>
          </select>
        </div>
      </div>

      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <span class="font-medium text-gray-900 dark:text-white">Sync on startup</span>
        <button @click="syncSettings.globalSettings.syncOnStartup = !syncSettings.globalSettings.syncOnStartup; syncSettings.saveGlobalSettings()" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', syncSettings.globalSettings.syncOnStartup ? 'bg-primary' : 'bg-gray-300']">
          <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', syncSettings.globalSettings.syncOnStartup ? 'translate-x-6' : 'translate-x-1']"></span>
        </button>
      </div>
    </section>
   
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Per-Service Sync Settings</h3>
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
         <div v-for="service in syncServicesList" :key="service.key" class="p-4 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark">
            <h4 class="font-medium text-gray-900 dark:text-white mb-3">{{ service.label }}</h4>
            <div class="space-y-2">
               <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300">
                 <input type="checkbox" v-model="syncSettings.settings[service.key].syncFavorites" @change="syncSettings.updateServiceSettings(service.key)" class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent">
                 Sync favorites
               </label>
               <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300">
                 <input type="checkbox" v-model="syncSettings.settings[service.key].syncPlaylists" @change="syncSettings.updateServiceSettings(service.key)" class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent">
                 Sync playlists
               </label>
               <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300">
                 <input type="checkbox" v-model="syncSettings.settings[service.key].syncSavedAlbums" @change="syncSettings.updateServiceSettings(service.key)" class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent">
                 Sync saved albums
               </label>
               <label class="flex items-start gap-2 text-xs text-gray-700 dark:text-gray-300">
                 <input type="checkbox" v-model="syncSettings.settings[service.key].incrementalOnly" @change="syncSettings.updateServiceSettings(service.key)" class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent mt-0.5">
                 <div><span>Incremental sync only</span><span class="block text-[10px] text-text-secondary">Only fetch changes since last sync</span></div>
               </label>
            </div>
         </div>
      </div>
    </section>

    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Background Downloads</h3>
      
      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <span class="font-medium text-gray-900 dark:text-white">Download in background when idle</span>
        <button @click="syncSettings.globalSettings.backgroundDownload = !syncSettings.globalSettings.backgroundDownload; syncSettings.saveGlobalSettings()" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', syncSettings.globalSettings.backgroundDownload ? 'bg-primary' : 'bg-gray-300']">
          <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', syncSettings.globalSettings.backgroundDownload ? 'translate-x-6' : 'translate-x-1']"></span>
        </button>
      </div>

      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <span class="font-medium text-gray-900 dark:text-white">Pause downloads when on metered connection</span>
        <button @click="syncSettings.globalSettings.pauseOnMetered = !syncSettings.globalSettings.pauseOnMetered; syncSettings.saveGlobalSettings()" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', syncSettings.globalSettings.pauseOnMetered ? 'bg-primary' : 'bg-gray-300']">
          <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', syncSettings.globalSettings.pauseOnMetered ? 'translate-x-6' : 'translate-x-1']"></span>
        </button>
      </div>

      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <div>
          <div class="font-medium text-gray-900 dark:text-white">Pause downloads when battery is low</div>
          <div class="text-xs text-text-secondary">For laptops, threshold at 20%</div>
        </div>
        <button @click="syncSettings.globalSettings.pauseOnLowBattery = !syncSettings.globalSettings.pauseOnLowBattery; syncSettings.saveGlobalSettings()" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', syncSettings.globalSettings.pauseOnLowBattery ? 'bg-primary' : 'bg-gray-300']">
          <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', syncSettings.globalSettings.pauseOnLowBattery ? 'translate-x-6' : 'translate-x-1']"></span>
        </button>
      </div>
    </section>

     <section class="space-y-4">
      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <span class="font-medium text-gray-900 dark:text-white">Max concurrent downloads</span>
        <input 
          type="number" 
          v-model.number="syncSettings.globalSettings.maxConcurrentDownloads" 
          @change="syncSettings.saveGlobalSettings()"
          min="1" max="10" 
          class="w-16 px-2 py-1 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-900 dark:text-white"
        >
      </div>

      <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <span class="font-medium text-gray-900 dark:text-white">Delay between downloads (ms)</span>
        <input 
          type="number" 
          v-model.number="syncSettings.globalSettings.rateLimitDelayMs" 
          @change="syncSettings.saveGlobalSettings()"
          min="0" max="5000" step="100"
          class="w-20 px-2 py-1 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-900 dark:text-white"
        >
      </div>

      <div class="mt-4" title="Coming in next update">
         <button disabled class="flex items-center gap-2 text-sm text-primary font-medium opacity-50 cursor-not-allowed">
            <span class="material-symbols-outlined text-[18px]">expand_more</span>
            Show per-service rate limits
         </button>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useSyncSettings } from '@/composables/useSyncSettings'

const syncSettings = useSyncSettings()

// Service list for sync settings UI
const syncServicesList = [
  { key: 'spotify' as const, label: 'Spotify' },
  { key: 'qobuz' as const, label: 'Qobuz' },
  { key: 'tidal' as const, label: 'Tidal' },
  { key: 'deezer' as const, label: 'Deezer' },
  { key: 'soundcloud' as const, label: 'SoundCloud' },
  { key: 'apple_music' as const, label: 'Apple Music' },
]

onMounted(async () => {
  try {
    await syncSettings.loadSettings()
  } catch (err) {
    console.error('Failed to load sync settings in isolated component:', err)
  }
})
</script>
