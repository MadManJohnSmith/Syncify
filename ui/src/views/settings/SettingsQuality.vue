<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <!-- Global download ceiling (AQ-01, FS-02) -->
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[22px]">vertical_align_top</span>
          Global maximum quality
        </h3>
        <p class="text-xs text-text-secondary mt-0.5">Strict limit for every download: effective quality is min(global ceiling, service limit)</p>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Effective download ceiling</label>
          <div class="relative sm:max-w-md">
            <select
              v-model="globalCeiling"
              @change="handleGlobalCeilingChange"
              data-testid="global-max-quality-select"
              class="w-full px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer"
            >
              <option value="any">No ceiling (default)</option>
              <option value="hires">Hi-Res max. (24-bit / up to 192kHz FLAC)</option>
              <option value="lossless">Lossless max. (16-bit / 44.1kHz FLAC)</option>
              <option value="high">High max. (320 kbps MP3 / AAC)</option>
            </select>
            <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-gray-500">
              <span class="material-symbols-outlined text-[18px]">expand_more</span>
            </div>
          </div>
          <p class="text-xs text-text-secondary mt-2">
            Applies a strict limit to all downloads: no track will request a higher quality than selected here, regardless of per-service settings or account tier.
          </p>
        </div>

        <div
          :class="[
            'p-3 rounded-lg border text-xs',
            globalCeiling !== 'any'
              ? 'bg-amber-50 dark:bg-amber-500/10 border-amber-200 dark:border-amber-500/30 text-amber-800 dark:text-amber-200'
              : 'bg-gray-50/70 dark:bg-surface-highlight/30 border-gray-200/80 dark:border-gray-700/60 text-text-secondary'
          ]"
        >
          <span v-if="globalCeiling === 'lossless'" data-testid="effective-ceiling-text">
            With a <strong>lossless</strong> ceiling, no download requests 24-bit: Qobuz receives only format_id 6 and Tidal the LOSSLESS parameter (even when a master is available).
          </span>
          <span v-else-if="globalCeiling === 'high'" data-testid="effective-ceiling-text">
            With a <strong>high</strong> ceiling, all downloads use the 320 kbps tier (Qobuz format_id 5 / Tidal AAC HIGH).
          </span>
          <span v-else-if="globalCeiling === 'hires'" data-testid="effective-ceiling-text">
            <strong>Hi-res</strong> ceiling: 24-bit is allowed; each service also respects its own quality limit if stricter.
          </span>
          <span v-else data-testid="effective-ceiling-text">
            No ceiling active. Each service downloads at its configured quality (or the maximum allowed by the connected account).
          </span>
        </div>
      </div>
    </section>

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
                <!-- AQ-05: options come from services.max_quality via
                     get_available_qualities_for_service, not from a fixed list:
                     this way a service can never be asked for more than it offers. -->
                <option
                  v-for="q in availableQualities[pref.service_name] ?? [{ value: pref.max_quality, label: QUALITY_LABELS[pref.max_quality] ?? pref.max_quality }]"
                  :key="q.value"
                  :value="q.value"
                >{{ q.label }}</option>
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
import { ref, onMounted } from 'vue'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import {
  getGlobalMaxQuality,
  setGlobalMaxQuality,
  getAvailableQualitiesForService,
  type GlobalMaxQuality,
} from '@/api/settings'

const downloadSettings = useDownloadSettings()

// Global download quality ceiling (AQ-01, FS-02)
const globalCeiling = ref<GlobalMaxQuality>('any')

// AQ-05: qualities each service declares in services.max_quality.
const availableQualities = ref<Record<string, { value: string; label: string }[]>>({})

const QUALITY_LABELS: Record<string, string> = {
  hires: 'Hi-Res (24-bit/96kHz+)',
  lossless: 'Lossless (16-bit/44.1kHz)',
  high: 'High (320 kbps)',
  normal: 'Normal (128-256 kbps)',
}

async function loadAvailableQualities(serviceNames: string[]) {
  const result: Record<string, { value: string; label: string }[]> = {}
  await Promise.all(
    serviceNames.map(async (name) => {
      try {
        const raw = await getAvailableQualitiesForService(name)
        if (Array.isArray(raw) && raw.length > 0) {
          const persisted = downloadSettings.getQualityForService(name)?.max_quality
          const allowed = persisted && !raw.includes(persisted) ? [...raw, persisted] : raw
          result[name] = allowed.map((q: string) => ({
            value: q,
            label: QUALITY_LABELS[q] ?? q,
          }))
        }
      } catch {
        // No backend data: falls back to the full list.
      }
    })
  )
  availableQualities.value = result
}

onMounted(async () => {
  if (downloadSettings.qualityPreferences.value.length === 0) {
    console.log('[SettingsQuality] State empty, loading from backend...')
    await downloadSettings.loadSettings()
  }
  globalCeiling.value = await getGlobalMaxQuality().catch(() => 'any' as GlobalMaxQuality)
  // AQ-05: limit each selector to what the service actually offers.
  await loadAvailableQualities(
    downloadSettings.qualityPreferences.value.map(p => p.service_name)
  )
})

async function handleGlobalCeilingChange() {
  try {
    await setGlobalMaxQuality(globalCeiling.value)
  } catch (err) {
    console.error('Failed to save global max quality:', err)
  }
}

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
