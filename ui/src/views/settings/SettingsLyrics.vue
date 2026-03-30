<template>
  <div class="space-y-8">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Lyrics Sources (Priority Order)</h3>
      <p class="text-sm text-text-secondary">Syncify will try sources in this order. Drag to reorder.</p>
      <div v-if="lyricsSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading lyrics providers...</span>
      </div>
      <div v-else class="space-y-2">
        <div 
          v-for="(provider, i) in lyricsSettings.orderedProviders.value" 
          :key="provider.provider_id"
          class="flex items-center justify-between p-3 bg-white dark:bg-surface-dark rounded-lg border border-gray-200 dark:border-border-dark"
        >
          <div class="flex items-center gap-3">
            <span class="w-6 h-6 flex items-center justify-center bg-gray-100 dark:bg-surface-highlight rounded text-xs font-medium text-gray-600 dark:text-gray-400">{{ i + 1 }}</span>
            <div>
              <span class="block text-sm font-medium text-gray-900 dark:text-white">{{ provider.provider_name }}</span>
              <span class="block text-xs text-text-secondary capitalize">{{ provider.sync_level }}-level sync</span>
            </div>
          </div>
          <div class="flex items-center gap-2">
            <button @click="lyricsSettings.moveProviderUp(provider.provider_id)" :disabled="i === 0" class="p-1 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded transition-colors disabled:opacity-30">
              <span class="material-symbols-outlined text-[18px] text-gray-500">arrow_upward</span>
            </button>
            <button @click="lyricsSettings.moveProviderDown(provider.provider_id)" :disabled="i === lyricsSettings.orderedProviders.value.length - 1" class="p-1 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded transition-colors disabled:opacity-30">
              <span class="material-symbols-outlined text-[18px] text-gray-500">arrow_downward</span>
            </button>
            <div class="w-px h-4 bg-gray-200 dark:bg-gray-700 mx-1"></div>
            <button @click="lyricsSettings.toggleProvider(provider.provider_id)" class="p-1 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded transition-colors">
              <span class="material-symbols-outlined text-[18px]" :class="provider.enabled ? 'text-success' : 'text-gray-400'">{{ provider.enabled ? 'check_circle' : 'cancel' }}</span>
            </button>
          </div>
        </div>
      </div>
    </section>

    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Lyrics Preferences</h3>
      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Minimum sync level</label>
          <select 
            :value="lyricsSettings.config.min_sync_level"
            @change="lyricsSettings.updateConfigField('min_sync_level', getEventValue($event))"
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
          >
            <option v-for="opt in lyricsSettings.syncLevelOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Preferred language</label>
          <select 
            :value="lyricsSettings.config.preferred_language"
            @change="lyricsSettings.updateConfigField('preferred_language', getEventValue($event))"
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
          >
            <option v-for="opt in lyricsSettings.languageOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
      </div>
    </section>
    
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Lyrics Storage</h3>
      <div>
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Storage format</label>
        <select 
          :value="lyricsSettings.config.storage_format"
          @change="lyricsSettings.updateConfigField('storage_format', getEventValue($event))"
          class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
        >
          <option v-for="opt in lyricsSettings.storageFormatOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
      </div>
      
      <div class="flex items-center justify-between py-2 cursor-pointer" @click="toggleAutoFetchLyrics">
        <div>
          <span class="block text-sm font-medium text-gray-900 dark:text-white">Auto-fetch lyrics on import</span>
          <span class="block text-xs text-text-secondary mt-0.5">Automatically search for lyrics when adding new tracks</span>
        </div>
        <div class="relative inline-block w-10 align-middle select-none">
          <div :class="lyricsSettings.config.auto_fetch_on_import ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
          <div :class="lyricsSettings.config.auto_fetch_on_import ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
        </div>
      </div>
    </section>

    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Retry Behavior</h3>
      <div class="flex items-center justify-between py-2 cursor-pointer" @click="toggleRetryFailed">
        <div>
          <span class="block text-sm font-medium text-gray-900 dark:text-white">Retry failed lookups</span>
          <span class="block text-xs text-text-secondary mt-0.5">Periodically retry tracks that failed to find lyrics</span>
        </div>
        <div class="relative inline-block w-10 align-middle select-none">
          <div :class="lyricsSettings.config.retry_failed ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
          <div :class="lyricsSettings.config.retry_failed ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
        </div>
      </div>
      
      <div v-if="lyricsSettings.config.retry_failed">
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Retry frequency</label>
        <select 
          :value="lyricsSettings.config.retry_frequency"
          @change="lyricsSettings.updateConfigField('retry_frequency', getEventValue($event))"
          class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
        >
          <option v-for="opt in lyricsSettings.retryFrequencyOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useLyricsSettings } from '@/composables/useLyricsSettings'

const getEventValue = (e: any) => e.target?.value || ''

const lyricsSettings = useLyricsSettings()

async function toggleAutoFetchLyrics() {
  lyricsSettings.config.auto_fetch_on_import = !lyricsSettings.config.auto_fetch_on_import
  await lyricsSettings.saveConfig()
}

async function toggleRetryFailed() {
  lyricsSettings.config.retry_failed = !lyricsSettings.config.retry_failed
  await lyricsSettings.saveConfig()
}

onMounted(async () => {
  if (!lyricsSettings.orderedProviders.value || lyricsSettings.orderedProviders.value.length === 0) {
    await lyricsSettings.loadSettings()
  }
})
</script>
