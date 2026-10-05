<template>
  <div class="space-y-8 animate-in fade-in duration-300">
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
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Download Scheduling</h3>

      <div class="flex items-center justify-between gap-4 p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <label for="max-concurrent-downloads" class="font-medium text-gray-900 dark:text-white">Max concurrent downloads</label>
        <input
          id="max-concurrent-downloads"
          type="number"
          v-model.number="syncSettings.globalSettings.maxConcurrentDownloads"
          @change="syncSettings.saveGlobalSettings()"
          min="1" max="10"
          class="w-16 px-2 py-1 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-900 dark:text-white"
        >
      </div>

      <div class="flex items-center justify-between gap-4 p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
        <label for="rate-limit-delay" class="font-medium text-gray-900 dark:text-white">Delay between downloads (ms)</label>
        <input
          id="rate-limit-delay"
          type="number"
          v-model.number="syncSettings.globalSettings.rateLimitDelayMs"
          @change="syncSettings.saveGlobalSettings()"
          min="0" max="5000" step="100"
          class="w-20 px-2 py-1 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded text-sm text-gray-900 dark:text-white"
        >
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
