<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    
    <!-- Section 1: Library & Download Location -->
    <section class="space-y-4">
      <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <span class="material-symbols-outlined text-primary text-[22px]">folder_open</span>
            Library & Download Location
          </h3>
          <p class="text-xs text-text-secondary mt-0.5">Base directory where downloaded audio tracks, lyrics (.lrc), artwork, and booklets are stored</p>
        </div>
        <span v-if="saveStatus" :class="['text-xs px-2.5 py-1 rounded-full font-medium transition-all', saveStatus === 'saved' ? 'bg-emerald-500/10 text-emerald-500 border border-emerald-500/20' : 'bg-primary/10 text-primary border border-primary/20']">
          {{ saveStatus === 'saved' ? '✓ Changes saved' : 'Saving...' }}
        </span>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-3">
        <div class="flex items-center justify-between">
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300">Storage Directory</label>
          <span :class="['text-xs flex items-center gap-1 font-medium', pathStatusBadgeClass]">
            <span class="material-symbols-outlined text-[15px]">{{ pathStatusIcon }}</span>
            {{ pathStatusLabel }}
            <span v-if="downloadSettings.downloadDto.free_space_bytes" class="text-text-secondary font-normal">
              ({{ formattedFreeSpace }} free)
            </span>
          </span>
        </div>
        <div class="flex gap-2">
          <div class="relative flex-1">
            <input 
              type="text" 
              :value="downloadSettings.downloadDto.library_root"
              @input="handleInputPath(($event.target as HTMLInputElement).value)"
              @change="handlePathChange"
              placeholder="Select library directory..." 
              class="w-full px-3.5 py-2.5 bg-gray-50 dark:bg-surface-highlight/40 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white font-mono text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none transition-all"
            />
          </div>
          <button 
            type="button"
            @click="handleBrowseFolder"
            class="px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium shadow-md shadow-primary/20 flex items-center gap-1.5 transition-all cursor-pointer"
            title="Browse folder with native dialog"
          >
            <span class="material-symbols-outlined text-[18px]">folder</span>
            Browse...
          </button>
          <button 
            type="button"
            @click="handleResetPath"
            class="px-3 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-100 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors cursor-pointer"
            title="Reset to default system music directory"
          >
            <span class="material-symbols-outlined text-[18px]">restart_alt</span>
          </button>
        </div>
        <div class="flex items-center justify-between text-xs text-text-secondary">
          <p class="flex items-center gap-1">
            <span class="material-symbols-outlined text-[14px] text-gray-400">info</span>
            Audio files, synced lyrics, and metadata sidecars will be saved relative to this root path.
          </p>
          <p class="font-mono text-[11px]">
            Staging: <span class="text-gray-600 dark:text-gray-300">{{ downloadSettings.downloadDto.staging_root || '.staging' }}</span>
          </p>
        </div>
      </div>
    </section>

    <!-- Section 2: Audio Quality & Format Preferences -->
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[22px]">high_quality</span>
          Audio Quality Preferences
        </h3>
        <p class="text-xs text-text-secondary mt-0.5">Maximum audio quality cap and preferred file format across streaming providers</p>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <h4 class="text-sm font-medium text-gray-900 dark:text-white">Default Quality Preset</h4>
            <p class="text-xs text-text-secondary">Applies default quality target across all connected streaming services</p>
          </div>
          <span class="text-xs px-2 py-0.5 bg-primary/10 text-primary font-semibold rounded uppercase tracking-wider">
            {{ selectedGlobalQuality }}
          </span>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1.5">Max Quality Target</label>
            <div class="relative">
              <select 
                v-model="selectedGlobalQuality"
                @change="handleGlobalQualityChange"
                class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer"
              >
                <option value="hires">Hi-Res Lossless (24-bit / 96kHz+ FLAC)</option>
                <option value="lossless">Lossless CD Quality (16-bit / 44.1kHz FLAC)</option>
                <option value="high">High Quality (320 kbps MP3 / AAC)</option>
                <option value="normal">Standard (128-256 kbps)</option>
              </select>
              <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-gray-500">
                <span class="material-symbols-outlined text-[18px]">expand_more</span>
              </div>
            </div>
          </div>

          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1.5">Preferred Format</label>
            <div class="relative">
              <select 
                v-model="selectedGlobalFormat"
                @change="handleGlobalQualityChange"
                class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer"
              >
                <option value="flac">FLAC (Free Lossless Audio Codec)</option>
                <option value="alac">ALAC (Apple Lossless)</option>
                <option value="mp3">MP3 (MPEG Audio Layer III)</option>
                <option value="aac">AAC (Advanced Audio Coding)</option>
                <option value="ogg">Ogg Vorbis</option>
              </select>
              <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-gray-500">
                <span class="material-symbols-outlined text-[18px]">expand_more</span>
              </div>
            </div>
          </div>
        </div>

        <!-- Service-level overrides summary -->
        <div v-if="downloadSettings.qualityPreferences.value.length > 0" class="pt-3 border-t border-gray-100 dark:border-border-dark/60">
          <div class="flex items-center justify-between mb-2">
            <span class="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider">Per-Service Quality Limits</span>
          </div>
          <div class="grid grid-cols-2 sm:grid-cols-3 gap-2 text-xs">
            <div 
              v-for="pref in downloadSettings.qualityPreferences.value" 
              :key="pref.service_name"
              class="p-2 bg-gray-50 dark:bg-surface-highlight/30 rounded border border-gray-200 dark:border-gray-700/60 flex items-center justify-between"
            >
              <span class="font-medium text-gray-800 dark:text-gray-200 capitalize">{{ pref.service_name }}</span>
              <span class="px-1.5 py-0.5 rounded bg-gray-200/70 dark:bg-surface-highlight font-mono text-[11px] text-gray-600 dark:text-gray-300">
                {{ pref.max_quality }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Section 3: Service Fallback Policy (Downgrade Strategy) -->
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[22px]">call_split</span>
          Service & Quality Fallback Policy
        </h3>
        <p class="text-xs text-text-secondary mt-0.5">Determine behavior when a requested source track or lossless quality is unavailable</p>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">When primary source is unavailable</label>
          <div class="relative">
            <select 
              v-model="downloadSettings.folderSettings.fallback_action"
              @change="handleFallbackActionChange"
              class="w-full px-3.5 py-2.5 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer"
            >
              <option value="try_next">Try next available service (Fallback enabled)</option>
              <option value="skip">Skip track (Strict source quality matching only)</option>
              <option value="prompt">Prompt user for action</option>
            </select>
            <div class="absolute inset-y-0 right-0 flex items-center px-3 pointer-events-none text-gray-500">
              <span class="material-symbols-outlined text-[20px]">expand_more</span>
            </div>
          </div>
        </div>

        <div class="p-3.5 bg-gray-50/70 dark:bg-surface-highlight/30 rounded-lg border border-gray-200/80 dark:border-gray-700/60 flex items-center justify-between">
          <div>
            <span class="text-sm font-medium text-gray-800 dark:text-gray-200">Allow Quality Downgrade</span>
            <p class="text-xs text-text-secondary mt-0.5">If 24-bit Hi-Res is not available, download 16-bit FLAC or 320kbps instead of failing</p>
          </div>
          <button 
            type="button"
            @click="toggleAllowDowngrade"
            :class="[
              'relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none',
              allowDowngrade ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-700'
            ]"
            role="switch"
            :aria-checked="allowDowngrade"
          >
            <span 
              :class="[
                'pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
                allowDowngrade ? 'translate-x-5' : 'translate-x-0'
              ]" 
            />
          </button>
        </div>
      </div>
    </section>

    <!-- Section 3b: Global Maximum Download Quality (S203 ceiling) -->
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[22px]">vertical_align_top</span>
          Calidad máxima global
        </h3>
        <p class="text-xs text-text-secondary mt-0.5">Hard download ceiling applied to every queue item: effective cap = min(global, per-service quality limit)</p>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Techo global de calidad de descarga</label>
          <div class="relative sm:max-w-md">
            <select
              v-model="globalMaxQuality"
              @change="handleGlobalMaxQualityChange"
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
        </div>

        <div
          :class="[
            'p-3 rounded-lg border text-xs',
            globalMaxQuality === 'lossless' || globalMaxQuality === 'high'
              ? 'bg-amber-50 dark:bg-amber-500/10 border-amber-200 dark:border-amber-500/30 text-amber-800 dark:text-amber-200'
              : 'bg-gray-50/70 dark:bg-surface-highlight/30 border-gray-200/80 dark:border-gray-700/60 text-text-secondary'
          ]"
        >
          <span v-if="globalMaxQuality === 'lossless'">
            Con techo <strong>lossless</strong>, ninguna descarga pedirá 24-bit: Qobuz recibe solo format_id 6 y Tidal el parámetro LOSSLESS (aunque la pista tenga master disponible).
          </span>
          <span v-else-if="globalMaxQuality === 'high'">
            Con techo <strong>high</strong>, todas las descargas se sirven en el tramo 320 kbps (Qobuz format_id 5 / Tidal AAC HIGH).
          </span>
          <span v-else-if="globalMaxQuality === 'hires'">
            Techo <strong>hires</strong>: se permite 24-bit; cada servicio respeta además su propio límite de calidad si es más estricto.
          </span>
          <span v-else>
            Sin techo activo: cada descarga usa su preferencia por servicio y el máximo que la cuenta permita.
          </span>
        </div>
      </div>
    </section>

    <!-- Section 4: Download Concurrency & Performance -->
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[22px]">speed</span>
          Download Concurrency
        </h3>
        <p class="text-xs text-text-secondary mt-0.5">Number of parallel tracks downloading simultaneously (1 - 10 threads)</p>
      </div>

      <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h4 class="text-sm font-medium text-gray-900 dark:text-white">Active Concurrency Limit</h4>
          <p class="text-xs text-text-secondary mt-0.5">Current worker configuration: {{ downloadSettings.generalSettings.concurrentDownloads }} parallel threads</p>
        </div>

        <div class="flex items-center gap-1.5 p-1 bg-gray-100 dark:bg-surface-highlight/70 rounded-lg border border-gray-200 dark:border-gray-700">
          <button 
            v-for="threads in [1, 2, 3, 4, 5, 6, 8, 10]" 
            :key="threads"
            type="button"
            @click="handleSetConcurrency(threads)"
            :class="[
              'px-3.5 py-1.5 text-xs font-semibold rounded-md transition-all cursor-pointer',
              currentThreads === threads 
                ? 'bg-primary text-white shadow-sm' 
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-white/50 dark:hover:bg-white/10'
            ]"
            :title="`Set ${threads} concurrent download thread${threads > 1 ? 's' : ''}`"
          >
            {{ threads }} {{ threads === 1 ? 'Thread' : 'Threads' }}
          </button>
        </div>
      </div>
    </section>

    <!-- Section 5: Folder & File Structure (Template Editor) -->
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span class="material-symbols-outlined text-primary text-[22px]">segment</span>
          Folder & File Naming Structure
        </h3>
        <p class="text-xs text-text-secondary mt-0.5">Define naming templates and organization schema for music files</p>
      </div>

      <div>
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Preset Templates</label>
        <div class="relative">
          <select 
            v-model="selectedPreset" 
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer"
          >
            <option v-for="(_preset, key) in folderPresets" :key="key" :value="key">{{ key }}</option>
            <option value="Custom">Custom (User Defined)</option>
          </select>
          <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-gray-500">
            <span class="material-symbols-outlined text-[20px]">expand_more</span>
          </div>
        </div>
      </div>

      <div class="p-6 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-4">
        <h4 class="font-medium text-gray-900 dark:text-white mb-2">Template Editor</h4>
        
        <div class="space-y-4">
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Folder template</label>
            <input 
              type="text" 
              v-model="folderTemplate" 
              @input="selectedPreset = 'Custom'" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white font-mono text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none"
            />
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">File naming template</label>
            <input 
              type="text" 
              v-model="fileTemplate" 
              @input="selectedPreset = 'Custom'" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white font-mono text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none"
            />
          </div>
        </div>

        <!-- Template Variables Cheat Sheet -->
        <div class="mt-4">
          <button 
            type="button"
            @click="showVariables = !showVariables" 
            class="flex items-center gap-2 text-sm text-primary hover:text-primary-hover font-medium cursor-pointer"
          >
            <span class="material-symbols-outlined text-[18px]">{{ showVariables ? 'expand_less' : 'expand_more' }}</span>
            {{ showVariables ? 'Hide available variables' : 'Show available variables' }}
          </button>
          
          <div v-if="showVariables" class="mt-3 p-4 bg-background-light dark:bg-background-dark rounded-lg border border-gray-200 dark:border-gray-700 text-xs space-y-3 max-h-60 overflow-y-auto custom-scrollbar">
            <div v-for="(vars, category) in templateVariables" :key="category">
              <h5 class="font-bold text-gray-900 dark:text-white mb-1.5">{{ category }}</h5>
              <div class="flex flex-wrap gap-2">
                <span 
                  v-for="v in vars" 
                  :key="v" 
                  @click="insertVariable(v)"
                  class="px-1.5 py-0.5 bg-gray-100 dark:bg-surface-highlight rounded border border-gray-200 dark:border-gray-600 text-gray-600 dark:text-gray-300 font-mono cursor-pointer hover:bg-primary/10 hover:text-primary hover:border-primary/30 transition-colors"
                  title="Click to copy variable"
                >
                  {{ v }}
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- Live Path Preview -->
        <div class="mt-4 p-4 bg-background-light dark:bg-background-dark rounded-lg border border-dashed border-gray-300 dark:border-gray-600">
          <div class="flex justify-between items-center mb-2">
            <span class="text-xs text-text-secondary uppercase tracking-wider font-semibold">Generated Path Preview</span>
          </div>
          <div class="font-mono text-sm text-gray-700 dark:text-gray-300 break-all flex items-center gap-1.5">
            <span class="text-primary material-symbols-outlined text-[18px]">audiotrack</span>
            <span class="text-gray-400 font-sans text-xs">{{ downloadSettings.downloadDto.library_root || 'Music' }}\</span>
            <span>{{ previewPath }}</span>
          </div>
        </div>
      </div>

      <!-- File Naming Rules -->
      <div class="p-6 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-4">
        <h4 class="font-medium text-gray-900 dark:text-white">File Naming Rules</h4>
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Replace invalid characters with</label>
            <input 
              type="text" 
              v-model="replaceInvalidChars"
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary outline-none"
            />
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Truncate long names with</label>
            <input 
              type="text" 
              v-model="truncateChars"
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary outline-none"
            />
          </div>
        </div>
        <div>
          <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Max filename length</label>
          <input 
            type="number" 
            v-model="downloadSettings.folderSettings.max_path_length"
            @change="downloadSettings.saveFolderSettings()"
            min="64"
            max="1024"
            class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary outline-none"
          />
        </div>
      </div>
    </section>

    <!-- Save Settings Button -->
    <div class="pt-4 flex justify-end gap-3 border-t border-gray-200 dark:border-border-dark">
      <button 
        type="button"
        @click="handleManualSave"
        :disabled="isSaving"
        class="px-6 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-semibold shadow-lg shadow-primary/20 transition-all flex items-center gap-2 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <span v-if="isSaving" class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span v-else class="material-symbols-outlined text-[18px]">save</span>
        {{ isSaving ? 'Saving...' : 'Save Settings' }}
      </button>
    </div>

  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import { getGlobalMaxQuality, setGlobalMaxQuality, type GlobalMaxQuality } from '@/api/settings'

const downloadSettings = useDownloadSettings()

const showVariables = ref(false)
const selectedPreset = ref('Standard')
const folderTemplate = ref('{AlbumArtist}/{Album}')
const fileTemplate = ref('{TrackNumber:pad2} - {Title}.{Format:lower}')
const selectedGlobalQuality = ref('hires')
const selectedGlobalFormat = ref('flac')
// S203: global download-quality ceiling (KV global_max_quality)
const globalMaxQuality = ref<GlobalMaxQuality>('any')
const replaceInvalidChars = ref('_')
const truncateChars = ref('...')
const isSaving = ref(false)
const saveStatus = ref<string | null>(null)

const currentThreads = computed(() => {
  return parseInt(downloadSettings.generalSettings.concurrentDownloads || '3', 10)
})

const allowDowngrade = computed(() => {
  return downloadSettings.folderSettings.fallback_action === 'try_next'
})

const pathStatusIcon = computed(() => {
  switch (downloadSettings.downloadDto.path_status) {
    case 'valid': return 'check_circle'
    case 'missing': return 'folder_off'
    case 'not_writable': return 'lock'
    case 'unavailable': return 'disc_full'
    default: return 'help'
  }
})

const pathStatusLabel = computed(() => {
  switch (downloadSettings.downloadDto.path_status) {
    case 'valid': return 'Valid & Accessible'
    case 'missing': return 'Directory Missing'
    case 'not_writable': return 'Read-Only (Not Writable)'
    case 'unavailable': return 'Drive Unmounted / Unavailable'
    default: return 'Unknown Status'
  }
})

const pathStatusBadgeClass = computed(() => {
  switch (downloadSettings.downloadDto.path_status) {
    case 'valid': return 'text-emerald-500 dark:text-emerald-400'
    case 'missing': return 'text-amber-500 dark:text-amber-400'
    case 'not_writable': return 'text-red-500 dark:text-red-400'
    case 'unavailable': return 'text-red-500 dark:text-red-400'
    default: return 'text-text-secondary'
  }
})

const formattedFreeSpace = computed(() => {
  const bytes = downloadSettings.downloadDto.free_space_bytes
  if (!bytes || bytes <= 0) return null
  const gb = bytes / (1024 * 1024 * 1024)
  if (gb >= 1000) {
    return `${(gb / 1024).toFixed(1)} TB`
  }
  return `${gb.toFixed(1)} GB`
})

function handleInputPath(val: string) {
  downloadSettings.downloadDto.library_root = val
}

const folderPresets: Record<string, { folder: string, file: string }> = {
  'Standard': { folder: '{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Audiophile': { folder: '{AlbumArtist}/{Year} - {Album} [{Quality}]', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Symfonium-optimized': { folder: '{AlbumArtist}/{Album} ({Year})', file: '{TrackNumber:pad2}. {Title}.{Format:lower}' },
  'MusicBrainz-friendly': { folder: '{AlbumArtist}/{MBReleaseID} - {Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Label-centric': { folder: '{Label}/{AlbumArtist}/{Album} ({Year})', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Alphabetized': { folder: '{FirstLetter:upper}/{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Quality-Sorted': { folder: '{Quality}/{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Source-Based': { folder: '{Source}/{AlbumArtist}/{Album} [{Quality}]', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Genre-Based': { folder: '{Genre}/{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Compilation-Safe': { folder: '{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Classical Music': { folder: '{Composer}/{Work}', file: '{TrackNumber:pad2} - {Title} ({Conductor}, {Orchestra}).{Format:lower}' },
  'Multi-Disc': { folder: '{AlbumArtist}/{Album}/Disc {DiscNumber}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Plex/Jellyfin Compatible': { folder: '{AlbumArtist}/{Album} ({Year})', file: '{AlbumArtist} - {Album} - {TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Minimalist': { folder: '{AlbumArtist} - {Album}', file: '{TrackNumber:pad2} {Title}.{Format:lower}' },
  'Archival': { folder: '{AlbumArtist}/{Year} - {Album} [{Quality}] [{Source}] [{CatalogNumber}]', file: '{TrackNumber:pad2} - {Title} [{ISRC}].{Format:lower}' },
}

const templateVariables = {
  'Track Metadata': ['{Artist}', '{AlbumArtist}', '{Album}', '{Title}', '{TrackNumber}', '{TrackNumber:pad2}', '{DiscNumber}', '{Year}', '{Date}', '{Genre}'],
  'Audio Quality': ['{Quality}', '{SampleRate}', '{BitDepth}', '{Bitrate}', '{Format}', '{Codec}'],
  'Identifiers': ['{ISRC}', '{MBTrackID}', '{MBReleaseID}', '{Barcode}', '{CatalogNumber}'],
  'Source': ['{Source}', '{Label}', '{ReleaseType}', '{Media}'],
  'Organization': ['{FirstLetter}', '{FirstLetter:upper}', '{ArtistInitial}'],
  'Classical': ['{Composer}', '{Conductor}', '{Orchestra}', '{Work}', '{Opus}']
}

// Watch preset selection changes
watch(selectedPreset, (newPreset) => {
  if (newPreset !== 'Custom' && folderPresets[newPreset]) {
    folderTemplate.value = folderPresets[newPreset].folder
    fileTemplate.value = folderPresets[newPreset].file
  }
})

// Path preview
const previewPath = computed(() => {
  let path = `${folderTemplate.value}\\${fileTemplate.value}`
  
  const overrides: Record<string, string> = {
    '{Artist}': 'Queen', '{AlbumArtist}': 'Queen', '{Album}': 'A Night at the Opera', '{Title}': 'Bohemian Rhapsody',
    '{TrackNumber}': '1', '{TrackNumber:pad2}': '01', '{Year}': '1975', '{Genre}': 'Rock',
    '{Quality}': selectedGlobalQuality.value === 'hires' ? '24-96' : '16-44',
    '{Format}': selectedGlobalFormat.value.toUpperCase(),
    '{Format:lower}': selectedGlobalFormat.value.toLowerCase(),
    '{Source}': 'Qobuz',
    '{Label}': 'EMI', '{MBReleaseID}': '1e0eee38-a9f6-49bf-84de-e53f85bc47b7',
    '{FirstLetter:upper}': 'Q', '{Composer}': 'Freddie Mercury', '{Work}': 'Bohemian Rhapsody'
  }
  
  for (const [key, val] of Object.entries(overrides)) {
    path = path.replaceAll(key, val)
  }
  
  return path
})

const insertVariable = (v: string) => {
  navigator.clipboard?.writeText(v)
}

// Handlers
async function handlePathChange() {
  triggerSavingFeedback()
  await downloadSettings.saveGeneralSettings()
  await downloadSettings.saveFolderSettings()
}

async function handleBrowseFolder() {
  const chosen = await downloadSettings.browseDownloadDirectory()
  if (chosen) {
    triggerSavingFeedback()
  }
}

async function handleResetPath() {
  await downloadSettings.resetDownloadPath()
  triggerSavingFeedback()
}

async function handleGlobalQualityChange() {
  await downloadSettings.updateGlobalQuality(selectedGlobalQuality.value, selectedGlobalFormat.value)
  triggerSavingFeedback()
}

async function handleFallbackActionChange() {
  await downloadSettings.updateFallbackAction(downloadSettings.folderSettings.fallback_action)
  triggerSavingFeedback()
}

async function toggleAllowDowngrade() {
  const newAction = allowDowngrade.value ? 'skip' : 'try_next'
  await downloadSettings.updateFallbackAction(newAction)
  triggerSavingFeedback()
}

async function handleSetConcurrency(threads: number) {
  await downloadSettings.setMaxConcurrent(threads)
  triggerSavingFeedback()
}

// S203: persist the global download-quality ceiling
async function handleGlobalMaxQualityChange() {
  try {
    await setGlobalMaxQuality(globalMaxQuality.value)
  } catch (err) {
    console.error('Failed to save global max quality:', err)
  }
  triggerSavingFeedback()
}

async function handleManualSave() {
  isSaving.value = true
  try {
    downloadSettings.folderSettings.folder_template = folderTemplate.value
    downloadSettings.folderSettings.file_template = fileTemplate.value
    await downloadSettings.saveDownloadSettings()
    saveStatus.value = 'saved'
    setTimeout(() => { saveStatus.value = null }, 2500)
  } catch (err) {
    console.error('Failed to save download settings:', err)
  } finally {
    isSaving.value = false
  }
}

function triggerSavingFeedback() {
  saveStatus.value = 'saved'
  setTimeout(() => {
    if (saveStatus.value === 'saved') saveStatus.value = null
  }, 2000)
}

// Auto-save folder templates after debounce
let folderSaveTimeout: number | null = null
watch([folderTemplate, fileTemplate], () => {
  if (folderSaveTimeout) clearTimeout(folderSaveTimeout)
  folderSaveTimeout = window.setTimeout(async () => {
    downloadSettings.folderSettings.folder_template = folderTemplate.value
    downloadSettings.folderSettings.file_template = fileTemplate.value
    await downloadSettings.saveFolderSettings()
    triggerSavingFeedback()
  }, 1000)
})

onMounted(async () => {
  try {
    await downloadSettings.loadSettings()
    if (downloadSettings.folderSettings.folder_template) {
      folderTemplate.value = downloadSettings.folderSettings.folder_template
      fileTemplate.value = downloadSettings.folderSettings.file_template
    }
    const qobuzPref = downloadSettings.getQualityForService('qobuz') || downloadSettings.qualityPreferences.value[0]
    if (qobuzPref) {
      selectedGlobalQuality.value = qobuzPref.max_quality || 'hires'
      selectedGlobalFormat.value = qobuzPref.preferred_format || 'flac'
    }
    // S203: load the global ceiling last so an explicit KV value always wins
    globalMaxQuality.value = await getGlobalMaxQuality()
  } catch (err) {
    console.error('Failed to initialize download settings:', err)
  }
})
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}
</style>
