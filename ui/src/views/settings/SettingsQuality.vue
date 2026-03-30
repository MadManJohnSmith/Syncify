<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Quality Caps (Per Service)</h3>
      <p class="text-sm text-text-secondary">Set maximum quality and preferred format for each streaming service</p>
      
      <div v-if="downloadSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary py-4">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading quality preferences...</span>
      </div>
      
      <div v-else class="space-y-4">
        <div v-for="pref in downloadSettings.qualityPreferences.value" :key="pref.service_name" class="p-4 bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark">
          <div class="flex items-center justify-between mb-3">
            <h4 class="font-medium text-gray-900 dark:text-white">{{ formatServiceName(pref.service_name) }}</h4>
            <span class="text-xs text-text-secondary px-2 py-1 bg-gray-100 dark:bg-surface-highlight rounded">{{ pref.max_quality }}</span>
          </div>
          <div class="grid grid-cols-2 gap-4">
            <div>
              <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Max Quality</label>
              <select 
                :value="pref.max_quality"
                @change="handleQualityChange(pref.service_name, getEventValue($event), pref.preferred_format)"
                class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary outline-none transition-all"
              >
                <option value="hires">Hi-Res (24-bit/96kHz+)</option>
                <option value="lossless">Lossless (16-bit/44.1kHz)</option>
                <option value="high">High (320 kbps)</option>
                <option value="normal">Normal (128-256 kbps)</option>
              </select>
            </div>
            <div>
              <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Preferred Format</label>
              <select 
                :value="pref.preferred_format"
                @change="handleQualityChange(pref.service_name, pref.max_quality, getEventValue($event))"
                class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary outline-none transition-all"
              >
                <option value="flac">FLAC</option>
                <option value="alac">ALAC</option>
                <option value="mp3">MP3</option>
                <option value="aac">AAC</option>
                <option value="ogg">Ogg Vorbis</option>
              </select>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useDownloadSettings } from '@/composables/useDownloadSettings'

const downloadSettings = useDownloadSettings()

onMounted(async () => {
  if (downloadSettings.qualityPreferences.value.length === 0) {
    console.log('[SettingsQuality] State empty, loading from backend...')
    await downloadSettings.loadSettings()
  }
})

// Helper for template events
const getEventValue = (e: Event) => (e.target as HTMLSelectElement).value;

// Format service name for display
function formatServiceName(name: string): string {
  const mapping: Record<string, string> = {
    'spotify': 'Spotify',
    'qobuz': 'Qobuz',
    'tidal': 'Tidal',
    'deezer': 'Deezer',
    'soundcloud': 'SoundCloud',
    'apple_music': 'Apple Music',
  }
  return mapping[name.toLowerCase()] || name
}

// Update quality preference for a service
async function updateQuality(
  serviceName: string, 
  maxQuality: string, 
  preferredFormat: string
) {
  const existing = downloadSettings.getQualityForService(serviceName)
  // CRITICAL: Ensure we pass all 5 parameters required by updateQualityForService
  await downloadSettings.updateQualityForService(
    serviceName,
    maxQuality,
    preferredFormat,
    existing?.fallback_quality || 'high',
    existing?.fallback_format || 'mp3'
  )
}

// Handle quality change from UI
async function handleQualityChange(serviceName: string, maxQuality: string, preferredFormat: string) {
  try {
    console.log(`[SettingsQuality] Quality change initiated for ${serviceName}: ${maxQuality}, ${preferredFormat}`)
    await updateQuality(serviceName, maxQuality, preferredFormat)
    console.log(`[SettingsQuality] Quality update confirmed for ${serviceName}`)
  } catch (e) {
    console.error(`[SettingsQuality] Failed to update quality for ${serviceName}:`, e)
  }
}
</script>
