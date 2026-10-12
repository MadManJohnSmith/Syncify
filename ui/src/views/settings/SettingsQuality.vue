<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <!-- Global download ceiling (AQ-01, FS-02) -->
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[22px]">vertical_align_top</span>
          Calidad máxima global
        </h3>
        <p class="text-xs text-text-secondary mt-0.5">Límite estricto aplicado a toda descarga: la calidad efectiva es min(techo global, límite del servicio)</p>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Techo efectivo de descarga</label>
          <div class="relative sm:max-w-md">
            <select
              v-model="globalCeiling"
              @change="handleGlobalCeilingChange"
              data-testid="global-max-quality-select"
              class="w-full px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer"
            >
              <option value="any">Sin techo (comportamiento por defecto)</option>
              <option value="hires">Hi-Res máx. (24-bit / hasta 192kHz FLAC)</option>
              <option value="lossless">Lossless máx. (16-bit / 44.1kHz FLAC)</option>
              <option value="high">High máx. (320 kbps MP3 / AAC)</option>
            </select>
            <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-gray-500">
              <span class="material-symbols-outlined text-[18px]">expand_more</span>
            </div>
          </div>
          <p class="text-xs text-text-secondary mt-2">
            Aplica a todas las descargas un límite estricto: ninguna pista pedirá una calidad superior a la elegida aquí, sin importar la configuración por servicio ni el tier de la cuenta.
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
            Con techo <strong>lossless</strong>, ninguna descarga pedirá 24-bit: Qobuz recibe solo format_id 6 y Tidal el parámetro LOSSLESS (aunque la pista tenga master disponible).
          </span>
          <span v-else-if="globalCeiling === 'high'" data-testid="effective-ceiling-text">
            Con techo <strong>high</strong>, todas las descargas se sirven en el tramo 320 kbps (Qobuz format_id 5 / Tidal AAC HIGH).
          </span>
          <span v-else-if="globalCeiling === 'hires'" data-testid="effective-ceiling-text">
            Techo <strong>hires</strong>: se permite 24-bit; cada servicio respeta además su propio límite de calidad si es más estricto.
          </span>
          <span v-else data-testid="effective-ceiling-text">
            Sin techo activo. Cada servicio descargará a la calidad configurada en su preferencia individual (o el máximo que permita la cuenta conectada).
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
                <!-- AQ-05: las opciones vienen de services.max_quality vía
                     get_available_qualities_for_service, no de una lista fija:
                     así no se puede pedir a un servicio más de lo que ofrece. -->
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

// AQ-05: calidades que cada servicio declara en services.max_quality.
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
        // Sin dato del backend: se queda con la lista completa.
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
  // AQ-05: limitar cada selector a lo que el servicio realmente ofrece.
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
