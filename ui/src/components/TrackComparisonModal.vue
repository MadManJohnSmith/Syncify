<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="isOpen" class="comparison-modal fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="close">
        <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-5xl max-h-[90vh] overflow-hidden shadow-2xl flex flex-col">
          <!-- Header -->
          <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center gap-4">
            <div class="w-14 h-14 rounded-lg bg-gray-200 dark:bg-gray-700 overflow-hidden shrink-0">
              <img v-if="track.albumArt" :src="track.albumArt" class="w-full h-full object-cover">
              <div v-else class="w-full h-full flex items-center justify-center">
                <span class="material-symbols-outlined text-gray-400 text-2xl">album</span>
              </div>
            </div>
            <div class="flex-1 min-w-0">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white truncate">{{ track.title }}</h3>
              <p class="text-sm text-gray-500 truncate">{{ track.artist }} · {{ track.album }}</p>
            </div>
            <div class="flex items-center gap-3">
              <button @click="exportComparison" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg" title="Export comparison">
                <span class="material-symbols-outlined text-gray-400">ios_share</span>
              </button>
              <button @click="close" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
          </div>
          
          <!-- Filters -->
          <div class="px-6 py-3 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
            <div class="flex items-center gap-4">
              <select v-model="filter" class="px-3 py-1.5 bg-gray-100 dark:bg-surface-highlight rounded-lg text-sm text-gray-700 dark:text-gray-300">
                <option value="all">All Services</option>
                <option value="available">Available Only</option>
                <option value="subscribed">Subscribed Only</option>
              </select>
              <label class="flex items-center gap-2 cursor-pointer text-sm text-gray-600 dark:text-gray-400">
                <input type="checkbox" v-model="showDifferencesOnly" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                Show only differences
              </label>
            </div>
            <div class="text-sm text-gray-500">
              Comparing {{ filteredServices.length }} services
            </div>
          </div>
          
          <!-- Comparison Table -->
          <div class="flex-1 overflow-x-auto custom-scrollbar">
            <table class="comparison-table w-full min-w-[800px]">
              <!-- Service Headers -->
              <thead>
                <tr class="bg-gray-50 dark:bg-surface-highlight">
                  <th class="p-4 text-left text-sm font-medium text-gray-500 w-40"></th>
                  <th 
                    v-for="service in filteredServices" 
                    :key="service.id"
                    :class="['service-column p-4 text-center', service.isRecommended && 'recommended-column bg-primary/5']"
                  >
                    <div class="flex flex-col items-center gap-2">
                      <div :class="['w-10 h-10 rounded-xl flex items-center justify-center', service.color]">
                        <span class="text-white text-lg font-bold">{{ service.name[0] }}</span>
                      </div>
                      <span class="text-sm font-medium text-gray-900 dark:text-white">{{ service.name }}</span>
                      <span v-if="service.available" class="px-2 py-0.5 bg-green-100 dark:bg-green-500/20 text-green-600 dark:text-green-400 text-xs rounded-full">
                        Available
                      </span>
                      <span v-else class="px-2 py-0.5 bg-gray-100 dark:bg-gray-700 text-gray-500 text-xs rounded-full">
                        Not Available
                      </span>
                      <span v-if="service.isRecommended" class="px-2 py-0.5 bg-primary/10 text-primary text-xs rounded-full font-medium flex items-center gap-1">
                        <span class="material-symbols-outlined text-xs">star</span>
                        Recommended
                      </span>
                    </div>
                  </th>
                </tr>
              </thead>
              
              <tbody class="divide-y divide-gray-200 dark:divide-border-dark">
                <!-- Quality Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">high_quality</span>
                    Quality
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center">
                    <div v-if="service.available" class="flex flex-col items-center gap-1">
                      <span :class="['quality-badge px-3 py-1 rounded-full text-sm font-medium', getQualityColor(service.quality)]">
                        {{ service.quality.label }}
                      </span>
                      <span v-if="service.isBestQuality" class="text-green-500 flex items-center gap-1 text-xs">
                        <span class="material-symbols-outlined text-sm">check_circle</span>
                        Best
                      </span>
                    </div>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
                
                <!-- Format Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">audio_file</span>
                    Format
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center">
                    <div v-if="service.available" class="flex flex-col items-center gap-1">
                      <span class="text-sm font-medium text-gray-900 dark:text-white">{{ service.format }}</span>
                      <span :class="['text-xs', service.isLossless ? 'text-green-500' : 'text-amber-500']">
                        {{ service.isLossless ? 'Lossless' : 'Lossy' }}
                      </span>
                    </div>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
                
                <!-- Bitrate/Sample Rate Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">speed</span>
                    Bitrate / Sample Rate
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center text-sm text-gray-600 dark:text-gray-400">
                    <template v-if="service.available">
                      <template v-if="service.isLossless">
                        {{ service.sampleRate }} kHz / {{ service.bitDepth }}-bit
                      </template>
                      <template v-else>
                        {{ service.bitrate }} kbps
                      </template>
                    </template>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
                
                <!-- File Size Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">folder</span>
                    Est. File Size
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center">
                    <span v-if="service.available" :class="['text-sm', (service.fileSize || 0) < 30 ? 'text-green-500' : (service.fileSize || 0) > 60 ? 'text-amber-500' : 'text-gray-600 dark:text-gray-400']">
                      ~{{ service.fileSize }} MB
                    </span>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
                
                <!-- Availability Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">cloud_download</span>
                    Availability
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center">
                    <div v-if="service.available" class="flex flex-col items-center gap-1 text-xs">
                      <span v-if="service.canStream" class="text-green-500 flex items-center gap-1">
                        <span class="material-symbols-outlined text-sm">check</span>
                        Streaming
                      </span>
                      <span v-if="service.canDownload" class="text-green-500 flex items-center gap-1">
                        <span class="material-symbols-outlined text-sm">check</span>
                        Download
                      </span>
                      <span v-if="service.isExclusive" class="text-amber-500 flex items-center gap-1">
                        <span class="material-symbols-outlined text-sm">star</span>
                        Exclusive
                      </span>
                    </div>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
                
                <!-- Metadata Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">info</span>
                    Metadata
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center">
                    <span v-if="service.available" :class="['text-sm font-medium', getMetadataColor(service.metadataScore)]">
                      {{ service.metadataScore }}%
                    </span>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
                
                <!-- Lyrics Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">lyrics</span>
                    Lyrics
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center">
                    <div v-if="service.available">
                      <span v-if="service.lyrics === 'synced'" class="text-blue-500 flex items-center justify-center gap-1 text-xs">
                        <span class="material-symbols-outlined text-sm">check</span>
                        Synced
                      </span>
                      <span v-else-if="service.lyrics === 'unsynced'" class="text-gray-500 flex items-center justify-center gap-1 text-xs">
                        <span class="material-symbols-outlined text-sm">check</span>
                        Unsynced
                      </span>
                      <span v-else class="text-gray-400">—</span>
                    </div>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
                
                <!-- Price Row -->
                <tr class="metric-row hover:bg-gray-50 dark:hover:bg-surface-highlight">
                  <td class="p-4 text-sm font-medium text-gray-700 dark:text-gray-300">
                    <span class="material-symbols-outlined text-lg mr-2 align-middle">payments</span>
                    Price
                  </td>
                  <td v-for="service in filteredServices" :key="service.id" class="p-4 text-center text-sm">
                    <template v-if="service.available">
                      <span v-if="service.includedInSub" class="text-green-500">Included</span>
                      <span v-else-if="service.price" class="text-gray-700 dark:text-gray-300">{{ service.price }}</span>
                      <span v-else class="text-green-500">Free</span>
                    </template>
                    <span v-else class="text-gray-400">N/A</span>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
          
          <!-- Actions -->
          <div class="download-actions px-6 py-4 border-t border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight">
            <div class="flex items-center justify-between">
              <!-- Per-service download buttons -->
              <div class="flex items-center gap-2 overflow-x-auto">
                <button 
                  v-for="service in availableServices" 
                  :key="service.id"
                  @click="downloadFrom(service)"
                  :class="[
                    'px-4 py-2 rounded-lg text-sm font-medium whitespace-nowrap flex items-center gap-2 transition-colors',
                    service.isRecommended 
                      ? 'bg-primary text-white hover:bg-primary-hover' 
                      : 'bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight'
                  ]"
                >
                  <span :class="['w-5 h-5 rounded flex items-center justify-center text-xs font-bold', service.color, service.isRecommended ? '' : 'text-white']">
                    {{ service.name[0] }}
                  </span>
                  Download from {{ service.name }}
                </button>
              </div>
              
              <!-- Global actions -->
              <div class="flex items-center gap-3 shrink-0 ml-4">
                <button @click="downloadBest" class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white font-semibold rounded-lg flex items-center gap-2">
                  <span class="material-symbols-outlined">download</span>
                  Download Best Quality
                </button>
                <button @click="close" class="px-4 py-2 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200">
                  Cancel
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'

const props = defineProps<{
  isOpen: boolean
  track: {
    id: string
    title: string
    artist: string
    album: string
    albumArt?: string
  }
}>()

const emit = defineEmits(['close', 'download'])

// State
const filter = ref('all')
const showDifferencesOnly = ref(false)

// Mock service comparison data
const services = ref([
  {
    id: 'spotify',
    name: 'Spotify',
    color: 'bg-green-500',
    available: true,
    isConnected: true,
    quality: { label: '320kbps OGG', level: 2 },
    format: 'OGG Vorbis',
    isLossless: false,
    bitrate: 320,
    sampleRate: null,
    bitDepth: null,
    fileSize: 12,
    canStream: true,
    canDownload: true,
    isExclusive: false,
    metadataScore: 92,
    lyrics: 'synced',
    includedInSub: true,
    price: null,
    isRecommended: false,
    isBestQuality: false,
  },
  {
    id: 'qobuz',
    name: 'Qobuz',
    color: 'bg-blue-600',
    available: true,
    isConnected: true,
    quality: { label: '24/96 FLAC', level: 5 },
    format: 'FLAC',
    isLossless: true,
    bitrate: null,
    sampleRate: 96,
    bitDepth: 24,
    fileSize: 58,
    canStream: true,
    canDownload: true,
    isExclusive: false,
    metadataScore: 98,
    lyrics: 'unsynced',
    includedInSub: true,
    price: null,
    isRecommended: true,
    isBestQuality: true,
  },
  {
    id: 'tidal',
    name: 'Tidal',
    color: 'bg-black',
    available: true,
    isConnected: true,
    quality: { label: '16/44.1 FLAC', level: 4 },
    format: 'FLAC',
    isLossless: true,
    bitrate: null,
    sampleRate: 44.1,
    bitDepth: 16,
    fileSize: 32,
    canStream: true,
    canDownload: true,
    isExclusive: false,
    metadataScore: 85,
    lyrics: 'synced',
    includedInSub: true,
    price: null,
    isRecommended: false,
    isBestQuality: false,
  },
  {
    id: 'deezer',
    name: 'Deezer',
    color: 'bg-purple-500',
    available: true,
    isConnected: false,
    quality: { label: '16/44.1 FLAC', level: 4 },
    format: 'FLAC',
    isLossless: true,
    bitrate: null,
    sampleRate: 44.1,
    bitDepth: 16,
    fileSize: 32,
    canStream: true,
    canDownload: true,
    isExclusive: false,
    metadataScore: 88,
    lyrics: null,
    includedInSub: true,
    price: null,
    isRecommended: false,
    isBestQuality: false,
  },
  {
    id: 'apple',
    name: 'Apple Music',
    color: 'bg-rose-500',
    available: false,
    isConnected: false,
    quality: { label: 'N/A', level: 0 },
    format: null,
    isLossless: false,
    bitrate: null,
    sampleRate: null,
    bitDepth: null,
    fileSize: null,
    canStream: false,
    canDownload: false,
    isExclusive: false,
    metadataScore: null,
    lyrics: null,
    includedInSub: false,
    price: null,
    isRecommended: false,
    isBestQuality: false,
  },
])

// Computed
const filteredServices = computed(() => {
  let result = [...services.value]
  
  if (filter.value === 'available') {
    result = result.filter(s => s.available)
  } else if (filter.value === 'subscribed') {
    result = result.filter(s => s.isConnected && s.available)
  }
  
  return result
})

const availableServices = computed(() => {
  return filteredServices.value.filter(s => s.available)
})

// Methods
function close() {
  emit('close')
}

function getQualityColor(quality: { level: number }) {
  if (quality.level >= 5) return 'bg-amber-100 dark:bg-amber-500/20 text-amber-600 dark:text-amber-400'
  if (quality.level >= 4) return 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300'
  if (quality.level >= 2) return 'bg-blue-100 dark:bg-blue-500/20 text-blue-600 dark:text-blue-400'
  return 'bg-gray-100 dark:bg-gray-700 text-gray-500'
}

function getMetadataColor(score: number | null) {
  if (!score) return 'text-gray-400'
  if (score >= 90) return 'text-green-500'
  if (score >= 70) return 'text-amber-500'
  return 'text-red-500'
}

function downloadFrom(service: any) {
  emit('download', { service: service.id, track: props.track })
  close()
}

function downloadBest() {
  const best = availableServices.value.find(s => s.isBestQuality) || availableServices.value[0]
  if (best) {
    downloadFrom(best)
  }
}

function exportComparison() {
  console.log('Exporting comparison...')
}
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.recommended-column {
  position: relative;
}

.recommended-column::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: 0;
  bottom: 0;
  border: 2px solid var(--color-primary);
  border-radius: 0.5rem;
  pointer-events: none;
}

.custom-scrollbar::-webkit-scrollbar {
  height: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: rgba(155, 155, 155, 0.3);
  border-radius: 3px;
}
</style>
