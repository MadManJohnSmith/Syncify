<template>
  <Transition name="fade">
    <div v-if="isVisible" class="onboarding-wizard fixed inset-0 z-50 bg-gradient-to-br from-gray-900 via-gray-900 to-primary-900/20 flex items-center justify-center overflow-hidden">
      
      <!-- Step Indicator (not on welcome) -->
      <div v-if="currentStep > 0 && currentStep < 5" class="step-indicator absolute top-8 left-1/2 -translate-x-1/2 flex items-center gap-2">
        <div v-for="step in 4" :key="step" :class="[
          'w-2.5 h-2.5 rounded-full transition-all',
          currentStep === step ? 'w-8 bg-primary' : currentStep > step ? 'bg-primary' : 'bg-gray-600'
        ]"></div>
        <span class="ml-3 text-sm text-gray-400">Step {{ currentStep }} of 4</span>
      </div>
      
      <!-- Content Container -->
      <div class="wizard-content w-full max-w-3xl mx-auto px-8">
        
        <!-- Step 0: Welcome -->
        <Transition name="slide" mode="out-in">
          <div v-if="currentStep === 0" key="welcome" class="wizard-step text-center">
            <!-- Logo -->
            <div class="logo-container mb-8 animate-fade-in-up">
              <div class="w-24 h-24 mx-auto rounded-2xl bg-gradient-to-br from-primary to-primary-600 flex items-center justify-center shadow-2xl shadow-primary/30">
                <span class="material-symbols-outlined text-5xl text-white">music_note</span>
              </div>
            </div>
            
            <!-- Heading -->
            <h1 class="text-4xl font-bold text-white mb-3 animate-fade-in-up" style="animation-delay: 100ms">Welcome to Syncify</h1>
            <p class="text-xl text-gray-400 mb-12 animate-fade-in-up" style="animation-delay: 200ms">Your unified music library orchestrator</p>
            
            <!-- Feature Cards -->
            <div class="grid grid-cols-3 gap-4 mb-12 animate-fade-in-up" style="animation-delay: 300ms">
              <div class="p-6 bg-white/5 backdrop-blur rounded-2xl border border-white/10">
                <div class="h-12 w-12 mx-auto rounded-xl bg-blue-500/20 text-blue-400 flex items-center justify-center mb-3">
                  <span class="material-symbols-outlined text-2xl">cloud_download</span>
                </div>
                <p class="text-white font-medium">Import from any service</p>
              </div>
              <div class="p-6 bg-white/5 backdrop-blur rounded-2xl border border-white/10">
                <div class="h-12 w-12 mx-auto rounded-xl bg-purple-500/20 text-purple-400 flex items-center justify-center mb-3">
                  <span class="material-symbols-outlined text-2xl">high_quality</span>
                </div>
                <p class="text-white font-medium">Download in best quality</p>
              </div>
              <div class="p-6 bg-white/5 backdrop-blur rounded-2xl border border-white/10">
                <div class="h-12 w-12 mx-auto rounded-xl bg-green-500/20 text-green-400 flex items-center justify-center mb-3">
                  <span class="material-symbols-outlined text-2xl">sync</span>
                </div>
                <p class="text-white font-medium">Sync across platforms</p>
              </div>
            </div>
            
            <!-- Buttons -->
            <div class="flex flex-col items-center gap-4 animate-fade-in-up" style="animation-delay: 400ms">
              <button @click="nextStep" class="px-8 py-4 bg-primary hover:bg-primary-hover text-white text-lg font-semibold rounded-xl transition-colors shadow-lg shadow-primary/30">
                Get Started
              </button>
              <button @click="skipSetup" class="text-gray-500 hover:text-gray-400 text-sm transition-colors">
                Skip Setup
              </button>
            </div>
          </div>
        </Transition>
        
        <!-- Step 1: Download Location -->
        <Transition name="slide" mode="out-in">
          <div v-if="currentStep === 1" key="location" class="wizard-step">
            <h2 class="text-3xl font-bold text-white text-center mb-2">Where should we save your music?</h2>
            <p class="text-gray-400 text-center mb-10">Choose a folder with plenty of space</p>
            
            <!-- Folder Selector -->
            <div class="p-6 bg-white/5 backdrop-blur rounded-2xl border border-white/10 mb-6">
              <div class="flex items-center gap-4">
                <div class="h-16 w-16 rounded-xl bg-amber-500/20 text-amber-400 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-3xl">folder</span>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-white font-medium truncate">{{ downloadPath }}</p>
                  <p class="text-sm text-gray-400 mt-1">Available space: <span class="text-green-400">450 GB</span></p>
                </div>
                <button class="px-4 py-2 bg-white/10 hover:bg-white/20 text-white rounded-lg text-sm font-medium transition-colors">
                  Choose Different Folder
                </button>
              </div>
            </div>
            
            <!-- Organization Preview -->
            <div class="p-6 bg-white/5 backdrop-blur rounded-2xl border border-white/10">
              <p class="text-sm text-gray-400 mb-3">Your music will be organized like this:</p>
              <div class="font-mono text-sm space-y-1">
                <p class="text-gray-300">📁 Syncify</p>
                <p class="text-gray-300 ml-4">└── 📁 Queen</p>
                <p class="text-gray-300 ml-8">└── 📁 A Night at the Opera</p>
                <p class="text-gray-400 ml-12">└── 🎵 01 - Bohemian Rhapsody.flac</p>
              </div>
              <p class="text-xs text-gray-500 mt-4">You can customize this later in Settings</p>
            </div>
            
            <!-- Navigation -->
            <div class="flex items-center justify-between mt-10">
              <button @click="prevStep" class="px-6 py-3 text-gray-400 hover:text-white transition-colors">
                Back
              </button>
              <button @click="skipSetup" class="text-gray-500 hover:text-gray-400 text-sm transition-colors">
                Skip Setup
              </button>
              <button @click="nextStep" class="px-8 py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl transition-colors">
                Next
              </button>
            </div>
          </div>
        </Transition>
        
        <!-- Step 2: Connect Service -->
        <Transition name="slide" mode="out-in">
          <div v-if="currentStep === 2" key="service" class="wizard-step">
            <h2 class="text-3xl font-bold text-white text-center mb-2">Connect a streaming service</h2>
            <p class="text-gray-400 text-center mb-10">Start by linking your favorite music platform</p>
            
            <!-- Service Grid -->
            <div class="service-selection grid grid-cols-3 gap-4 mb-6">
              <button 
                v-for="service in services" 
                :key="service.id"
                @click="connectService(service)"
                :disabled="service.loading"
                :class="[
                  'p-6 rounded-2xl border transition-all text-center relative',
                  service.connected 
                    ? 'bg-green-500/10 border-green-500/50' 
                    : 'bg-white/5 border-white/10 hover:border-primary/50 hover:bg-white/10',
                  service.loading ? 'opacity-70 cursor-wait' : ''
                ]"
              >
                <div :class="[
                  'h-16 w-16 mx-auto rounded-xl flex items-center justify-center mb-3',
                  service.connected ? 'bg-green-500/20' : 'bg-white/10'
                ]" :style="{ background: service.connected ? '' : service.bgColor }">
                  <span v-if="service.loading" class="material-symbols-outlined text-2xl animate-spin text-white">progress_activity</span>
                  <span v-else class="text-2xl font-bold text-white">{{ service.letter }}</span>
                </div>
                <p class="text-white font-medium mb-1">{{ service.name }}</p>
                <p v-if="service.loading" class="text-sm text-primary flex items-center justify-center gap-1">
                  Connecting...
                </p>
                <p v-else-if="service.connected" class="text-sm text-green-400 flex items-center justify-center gap-1">
                  <span class="material-symbols-outlined text-[16px]">check_circle</span>
                  Connected
                </p>
                <p v-else class="text-sm text-gray-500">Click to connect</p>
                <p v-if="service.tracks" class="text-xs text-gray-400 mt-1">Found {{ service.tracks.toLocaleString() }} tracks</p>
                <p v-if="service.error" class="text-xs text-red-400 mt-1 truncate" :title="service.error">{{ service.error }}</p>
              </button>
            </div>
            
            <!-- Info Box -->
            <div class="p-4 bg-blue-500/10 border border-blue-500/20 rounded-xl flex items-start gap-3">
              <span class="material-symbols-outlined text-blue-400 mt-0.5">info</span>
              <div>
                <p class="text-sm text-gray-300">Don't worry, you can add more services later</p>
                <p class="text-xs text-gray-500 mt-1">Your credentials are encrypted and stored locally</p>
              </div>
            </div>
            
            <!-- Navigation -->
            <div class="flex items-center justify-between mt-10">
              <button @click="prevStep" class="px-6 py-3 text-gray-400 hover:text-white transition-colors">
                Back
              </button>
              <button @click="nextStep" class="text-gray-500 hover:text-gray-400 text-sm transition-colors">
                Skip for Now
              </button>
              <button @click="nextStep" :disabled="!hasConnectedService" :class="[
                'px-8 py-3 font-semibold rounded-xl transition-colors',
                hasConnectedService ? 'bg-primary hover:bg-primary-hover text-white' : 'bg-gray-700 text-gray-500 cursor-not-allowed'
              ]">
                Next
              </button>
            </div>
          </div>
        </Transition>
        
        <!-- Step 3: Quality Preferences -->
        <Transition name="slide" mode="out-in">
          <div v-if="currentStep === 3" key="quality" class="wizard-step">
            <h2 class="text-3xl font-bold text-white text-center mb-2">Choose your audio quality</h2>
            <p class="text-gray-400 text-center mb-10">Balance quality and storage space</p>
            
            <!-- Quality Presets -->
            <div class="space-y-4 mb-6">
              <button 
                v-for="preset in qualityPresets" 
                :key="preset.id"
                @click="selectedQuality = preset.id"
                :class="[
                  'quality-preset w-full p-5 rounded-2xl border transition-all text-left flex items-center gap-4',
                  selectedQuality === preset.id 
                    ? 'bg-primary/10 border-primary/50' 
                    : 'bg-white/5 border-white/10 hover:border-white/30'
                ]"
              >
                <div :class="[
                  'h-12 w-12 rounded-xl flex items-center justify-center shrink-0',
                  preset.id === 'audiophile' ? 'bg-amber-500/20 text-amber-400' :
                  preset.id === 'balanced' ? 'bg-gray-400/20 text-gray-300' :
                  'bg-gray-600/20 text-gray-500'
                ]">
                  <span class="material-symbols-outlined text-2xl">{{ preset.icon }}</span>
                </div>
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <p class="text-white font-medium">{{ preset.name }}</p>
                    <span v-if="preset.recommended" class="px-2 py-0.5 bg-green-500/20 text-green-400 text-[10px] font-medium rounded">RECOMMENDED</span>
                  </div>
                  <p class="text-sm text-gray-400 mt-0.5">{{ preset.description }}</p>
                  <p class="text-xs text-gray-500 mt-1">~{{ preset.storage }} per album</p>
                </div>
                <div :class="[
                  'w-5 h-5 rounded-full border-2 flex items-center justify-center shrink-0',
                  selectedQuality === preset.id ? 'border-primary bg-primary' : 'border-gray-600'
                ]">
                  <span v-if="selectedQuality === preset.id" class="w-2 h-2 bg-white rounded-full"></span>
                </div>
              </button>
            </div>
            
            <!-- Custom Option -->
            <button @click="showCustomQuality = !showCustomQuality" class="w-full p-4 bg-white/5 border border-white/10 rounded-xl text-left flex items-center justify-between hover:bg-white/10 transition-colors">
              <span class="text-gray-400">Custom settings...</span>
              <span class="material-symbols-outlined text-gray-400">{{ showCustomQuality ? 'expand_less' : 'expand_more' }}</span>
            </button>
            
            <Transition name="slide-down">
              <div v-if="showCustomQuality" class="mt-4 p-4 bg-white/5 border border-white/10 rounded-xl space-y-4">
                <div>
                  <label class="text-sm text-gray-400 mb-2 block">Max Sample Rate</label>
                  <input type="range" min="44100" max="192000" step="44100" class="w-full">
                </div>
                <div>
                  <label class="text-sm text-gray-400 mb-2 block">Max Bit Depth</label>
                  <input type="range" min="16" max="32" step="8" class="w-full">
                </div>
              </div>
            </Transition>
            
            <p class="text-center text-sm text-gray-500 mt-6">You can change this anytime in Settings</p>
            
            <!-- Navigation -->
            <div class="flex items-center justify-between mt-10">
              <button @click="prevStep" class="px-6 py-3 text-gray-400 hover:text-white transition-colors">
                Back
              </button>
              <button @click="skipSetup" class="text-gray-500 hover:text-gray-400 text-sm transition-colors">
                Skip Setup
              </button>
              <button @click="nextStep" class="px-8 py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl transition-colors">
                Next
              </button>
            </div>
          </div>
        </Transition>
        
        <!-- Step 4: Import Content -->
        <Transition name="slide" mode="out-in">
          <div v-if="currentStep === 4" key="import" class="wizard-step">
            <h2 class="text-3xl font-bold text-white text-center mb-2">What would you like to import?</h2>
            <p class="text-gray-400 text-center mb-10">We'll add these to your library</p>
            
            <template v-if="hasConnectedService">
              <!-- Import Options -->
              <div class="import-options space-y-3 mb-6">
                <label v-for="option in importOptions" :key="option.id" :class="[
                  'flex items-center gap-4 p-4 rounded-xl border cursor-pointer transition-all',
                  option.selected ? 'bg-primary/10 border-primary/50' : 'bg-white/5 border-white/10 hover:border-white/30'
                ]">
                  <input type="checkbox" v-model="option.selected" class="w-5 h-5 rounded border-gray-600 text-primary focus:ring-primary">
                  <div class="flex-1">
                    <p class="text-white font-medium">{{ option.name }}</p>
                    <p class="text-sm text-gray-500">{{ option.count }}</p>
                  </div>
                  <span class="material-symbols-outlined text-gray-400">{{ option.icon }}</span>
                </label>
              </div>
              
              <!-- Auto-download toggle -->
              <label class="flex items-center gap-3 p-4 bg-green-500/10 border border-green-500/20 rounded-xl cursor-pointer">
                <input type="checkbox" v-model="autoDownload" class="w-5 h-5 rounded border-gray-600 text-green-500 focus:ring-green-500">
                <div class="flex-1">
                  <p class="text-white font-medium">Auto-download my favorites</p>
                  <p class="text-sm text-gray-500">Start downloading immediately after import</p>
                </div>
              </label>
            </template>
            
            <template v-else>
              <!-- No service connected options -->
              <div class="space-y-4">
                <div class="p-6 bg-white/5 border border-white/10 rounded-2xl">
                  <p class="text-white font-medium mb-3">Scan local music folder</p>
                  <div class="flex gap-3">
                    <input type="text" placeholder="C:\Users\username\Music" class="flex-1 px-4 py-2 bg-white/5 border border-white/10 rounded-lg text-white placeholder-gray-500">
                    <button class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium">Browse</button>
                  </div>
                </div>
                
                <div class="p-6 bg-white/5 border border-white/10 rounded-2xl">
                  <p class="text-white font-medium mb-3">Import from URL</p>
                  <div class="flex gap-3">
                    <input type="text" placeholder="https://open.spotify.com/playlist/..." class="flex-1 px-4 py-2 bg-white/5 border border-white/10 rounded-lg text-white placeholder-gray-500">
                    <button class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium">Import</button>
                  </div>
                </div>
              </div>
            </template>
            
            <!-- Navigation -->
            <div class="flex items-center justify-between mt-10">
              <button @click="prevStep" class="px-6 py-3 text-gray-400 hover:text-white transition-colors">
                Back
              </button>
              <button @click="skipSetup" class="text-gray-500 hover:text-gray-400 text-sm transition-colors">
                Skip Setup
              </button>
              <button @click="nextStep" class="px-8 py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl transition-colors">
                Next
              </button>
            </div>
          </div>
        </Transition>
        
        <!-- Step 5: All Set -->
        <Transition name="slide" mode="out-in">
          <div v-if="currentStep === 5" key="complete" class="wizard-step setup-complete text-center">
            <!-- Celebration -->
            <div class="text-6xl mb-6 animate-bounce">🎉</div>
            
            <h2 class="text-3xl font-bold text-white mb-2">You're all set!</h2>
            <p class="text-gray-400 mb-10">Syncify is ready to go</p>
            
            <!-- Summary Cards -->
            <div class="grid grid-cols-2 gap-4 mb-8">
              <div class="p-4 bg-white/5 border border-white/10 rounded-xl flex items-center gap-3 text-left">
                <span class="material-symbols-outlined text-green-400">check_circle</span>
                <div>
                  <p class="text-sm text-gray-400">Connected</p>
                  <p class="text-white font-medium">{{ connectedServiceName || 'No service' }}</p>
                </div>
              </div>
              <div class="p-4 bg-white/5 border border-white/10 rounded-xl flex items-center gap-3 text-left">
                <span class="material-symbols-outlined text-amber-400">folder</span>
                <div>
                  <p class="text-sm text-gray-400">Download location</p>
                  <p class="text-white font-medium truncate text-sm">{{ downloadPath }}</p>
                </div>
              </div>
              <div class="p-4 bg-white/5 border border-white/10 rounded-xl flex items-center gap-3 text-left">
                <span class="material-symbols-outlined text-purple-400">high_quality</span>
                <div>
                  <p class="text-sm text-gray-400">Quality</p>
                  <p class="text-white font-medium">{{ qualityLabel }}</p>
                </div>
              </div>
              <div class="p-4 bg-white/5 border border-white/10 rounded-xl flex items-center gap-3 text-left">
                <span class="material-symbols-outlined text-blue-400">library_music</span>
                <div>
                  <p class="text-sm text-gray-400">Imported</p>
                  <p class="text-white font-medium">{{ importedTracks.toLocaleString() }} tracks</p>
                </div>
              </div>
            </div>
            
            <!-- Optional Features -->
            <div class="space-y-3 mb-10 text-left max-w-md mx-auto">
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="takeTour" class="w-4 h-4 rounded border-gray-600 text-primary focus:ring-primary">
                <span class="text-gray-300">Take a quick tour</span>
              </label>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="showTips" checked class="w-4 h-4 rounded border-gray-600 text-primary focus:ring-primary">
                <span class="text-gray-300">Show me tips and tricks</span>
              </label>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="autoUpdate" checked class="w-4 h-4 rounded border-gray-600 text-primary focus:ring-primary">
                <span class="text-gray-300">Check for updates automatically</span>
              </label>
            </div>
            
            <!-- Start Button -->
            <button @click="completeSetup" class="px-10 py-4 bg-primary hover:bg-primary-hover text-white text-lg font-semibold rounded-xl transition-colors shadow-lg shadow-primary/30">
              Start Using Syncify
            </button>
          </div>
        </Transition>
      </div>
      
      <!-- App Tour Overlay -->
      <Transition name="fade">
        <div v-if="showTourOverlay" class="app-tour fixed inset-0 z-60">
          <div class="absolute inset-0 bg-black/80"></div>
          
          <!-- Spotlight Effect -->
          <div class="spotlight-effect" :style="spotlightStyle"></div>
          
          <!-- Tour Tooltip -->
          <div class="tour-tooltip absolute bg-white dark:bg-surface-dark rounded-xl p-5 shadow-2xl max-w-sm" :style="tooltipStyle">
            <p class="text-gray-900 dark:text-white font-medium mb-2">{{ tourSteps[tourIndex].title }}</p>
            <p class="text-sm text-gray-600 dark:text-gray-400 mb-4">{{ tourSteps[tourIndex].description }}</p>
            <div class="flex items-center justify-between">
              <span class="text-xs text-gray-400">{{ tourIndex + 1 }} of {{ tourSteps.length }}</span>
              <div class="flex gap-2">
                <button @click="skipTour" class="px-3 py-1.5 text-gray-500 hover:text-gray-700 text-sm">Skip Tour</button>
                <button @click="nextTourStep" class="px-4 py-1.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium">
                  {{ tourIndex === tourSteps.length - 1 ? 'Got it!' : 'Next' }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { accountsApi } from '@/api/accounts'
import { getDefaultDownloadPath } from '@/api/settings'

export interface OnboardingService {
  id: string
  name: string
  letter: string
  bgColor: string
  connected: boolean
  tracks: number
  loading?: boolean
  error?: string | null
}

const emit = defineEmits<{
  (e: 'complete'): void
  (e: 'skip'): void
}>()

// Wizard State
const isVisible = ref(true)
const currentStep = ref(0)

// Step 1: Download Location
const downloadPath = ref('C:\\Users\\username\\Music\\Syncify')

// Step 2: Services
const services = ref<OnboardingService[]>([
  { id: 'spotify', name: 'Spotify', letter: 'S', bgColor: 'linear-gradient(135deg, #1DB954, #1ed760)', connected: false, tracks: 0 },
  { id: 'apple', name: 'Apple Music', letter: 'A', bgColor: 'linear-gradient(135deg, #fc3c44, #fa2d55)', connected: false, tracks: 0 },
  { id: 'qobuz', name: 'Qobuz', letter: 'Q', bgColor: 'linear-gradient(135deg, #0170ef, #003366)', connected: false, tracks: 0 },
  { id: 'tidal', name: 'Tidal', letter: 'T', bgColor: 'linear-gradient(135deg, #000000, #333333)', connected: false, tracks: 0 },
  { id: 'deezer', name: 'Deezer', letter: 'D', bgColor: 'linear-gradient(135deg, #ff0092, #a100ff)', connected: false, tracks: 0 },
  { id: 'soundcloud', name: 'SoundCloud', letter: 'S', bgColor: 'linear-gradient(135deg, #ff7700, #ff5500)', connected: false, tracks: 0 },
])

const hasConnectedService = computed(() => services.value.some(s => s.connected))
const connectedServiceName = computed(() => services.value.find(s => s.connected)?.name || '')

// Step 3: Quality
const selectedQuality = ref('audiophile')
const showCustomQuality = ref(false)

const qualityPresets = ref([
  { id: 'audiophile', name: 'Maximum Quality', description: '24-bit Hi-Res FLAC when available', storage: '100 MB', icon: 'star', recommended: true },
  { id: 'balanced', name: 'High Quality', description: '16-bit CD Quality FLAC', storage: '50 MB', icon: 'star_half' },
  { id: 'saver', name: 'Good Quality', description: '320kbps MP3', storage: '10 MB', icon: 'star_outline' },
])

const qualityLabel = computed(() => {
  const preset = qualityPresets.value.find(p => p.id === selectedQuality.value)
  return preset?.name || 'Custom'
})

// Step 4: Import
const importOptions = ref([
  { id: 'favorites', name: 'My Favorites', count: '1,234 tracks', icon: 'favorite', selected: true },
  { id: 'playlists', name: 'My Playlists', count: '23 playlists', icon: 'queue_music', selected: false },
  { id: 'albums', name: 'Saved Albums', count: '156 albums', icon: 'album', selected: false },
  { id: 'artists', name: 'Followed Artists', count: '89 artists', icon: 'person', selected: false },
])
const autoDownload = ref(false)
const importedTracks = ref(1234)

// Step 5: Completion
const takeTour = ref(false)
const showTips = ref(true)
const autoUpdate = ref(true)

// Tour State
const showTourOverlay = ref(false)
const tourIndex = ref(0)
const tourSteps = ref([
  { title: 'Library Tab', description: 'This is your unified music collection from all connected services.', target: 'library' },
  { title: 'Downloads Tab', description: 'Manage your download queue and see progress here.', target: 'downloads' },
  { title: 'Connect Services', description: 'Click here to add more streaming services to Syncify.', target: 'accounts' },
  { title: 'Quick Search', description: 'Press Ctrl+K anytime to search your entire library.', target: 'search' },
  { title: 'Settings', description: 'Customize quality, storage, and other preferences here.', target: 'settings' },
])

const spotlightStyle = computed(() => ({
  // Placeholder spotlight position
  top: '100px',
  left: '50px',
  width: '200px',
  height: '50px',
}))

const tooltipStyle = computed(() => ({
  top: '170px',
  left: '50px',
}))

// Methods
function nextStep() {
  if (currentStep.value < 5) {
    currentStep.value++
  }
}

function prevStep() {
  if (currentStep.value > 0) {
    currentStep.value--
  }
}

function skipSetup() {
  isVisible.value = false
  emit('skip')
}

/**
 * Deterministically verifies account/connection status using real IPC commands / APIs.
 * Does not use Math.random().
 */
async function testConnection(serviceOrId: OnboardingService | string): Promise<boolean> {
  const service = typeof serviceOrId === 'string'
    ? services.value.find(s => s.id === serviceOrId)
    : serviceOrId

  const serviceId = typeof serviceOrId === 'string'
    ? serviceOrId
    : serviceOrId.id

  const backendServiceName = serviceId === 'apple' ? 'apple_music' : serviceId

  if (service) {
    service.loading = true
    service.error = null
  }

  try {
    // 1. Query auth status via IPC / API
    const authStatus = await accountsApi.checkAuthStatus(backendServiceName)
    if (authStatus && authStatus.success) {
      if (service) {
        service.connected = true
        const trackCount = (authStatus.data && typeof authStatus.data.track_count === 'number')
          ? (authStatus.data.track_count as number)
          : (authStatus.data && typeof authStatus.data.tracks === 'number')
            ? (authStatus.data.tracks as number)
            : 0
        service.tracks = trackCount
      }
      return true
    }

    // 2. Query configured accounts from DB
    const accounts = await accountsApi.getAccounts()
    const servicesList = await accountsApi.getServices()
    const matchedService = servicesList.find(
      s => s.name.toLowerCase() === backendServiceName.toLowerCase()
    )
    const activeAccount = matchedService
      ? accounts.find(a => a.service_id === matchedService.id && a.is_active)
      : null

    if (activeAccount) {
      if (service) {
        service.connected = true
        service.tracks = 0
      }
      return true
    }

    if (service) {
      service.connected = false
      service.error = authStatus?.error || null
    }
    return false
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    if (service) {
      service.connected = false
      service.error = msg
    }
    return false
  } finally {
    if (service) {
      service.loading = false
    }
  }
}

async function connectService(service: OnboardingService) {
  // First check if already connected or has viable auth
  const isAlreadyConnected = await testConnection(service)
  if (isAlreadyConnected) return

  const backendServiceName = service.id === 'apple' ? 'apple_music' : service.id
  service.loading = true
  service.error = null
  try {
    const result = await accountsApi.startAuthAndSave(backendServiceName)
    if (result.success) {
      service.connected = true
      const trackCount = (result.data && typeof result.data.track_count === 'number')
        ? (result.data.track_count as number)
        : (result.data && typeof result.data.tracks === 'number')
          ? (result.data.tracks as number)
          : 0
      service.tracks = trackCount
    } else {
      service.error = result.error || `Failed to connect to ${service.name}`
    }
  } catch (err: unknown) {
    service.error = err instanceof Error ? err.message : `Failed to connect to ${service.name}`
  } finally {
    service.loading = false
  }
}

function completeSetup() {
  if (takeTour.value) {
    showTourOverlay.value = true
    tourIndex.value = 0
  } else {
    isVisible.value = false
    emit('complete')
  }
}

function nextTourStep() {
  if (tourIndex.value < tourSteps.value.length - 1) {
    tourIndex.value++
  } else {
    skipTour()
  }
}

function skipTour() {
  showTourOverlay.value = false
  isVisible.value = false
  emit('complete')
}

onMounted(async () => {
  // Asynchronously query default download location if available
  try {
    const defaultPath = await getDefaultDownloadPath()
    if (defaultPath && defaultPath.trim()) {
      downloadPath.value = defaultPath.trim()
    }
  } catch {
    // Keep default fallback path
  }

  // Check initial account states
  try {
    const accounts = await accountsApi.getAccounts()
    const servicesList = await accountsApi.getServices()
    if (accounts && accounts.length > 0 && servicesList && servicesList.length > 0) {
      for (const service of services.value) {
        const backendName = service.id === 'apple' ? 'apple_music' : service.id
        const matched = servicesList.find(s => s.name.toLowerCase() === backendName.toLowerCase())
        if (matched && accounts.some(a => a.service_id === matched.id && a.is_active)) {
          service.connected = true
        }
      }
    }
  } catch {
    // Non-blocking initialization
  }
})

defineExpose({
  testConnection,
  connectService,
  services,
  isVisible,
  currentStep,
  skipSetup,
  completeSetup,
  skipTour,
})
</script>

<style scoped>
/* Animations */
@keyframes fade-in-up {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.animate-fade-in-up {
  animation: fade-in-up 0.5s ease-out forwards;
  opacity: 0;
}

/* Step Transitions */
.slide-enter-active,
.slide-leave-active {
  transition: all 0.3s ease;
}

.slide-enter-from {
  opacity: 0;
  transform: translateX(30px);
}

.slide-leave-to {
  opacity: 0;
  transform: translateX(-30px);
}

/* Fade Transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Slide Down Transition */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.2s ease;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

/* Spotlight Effect */
.spotlight-effect {
  position: absolute;
  border: 3px solid #6366f1;
  border-radius: 8px;
  box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.8);
  pointer-events: none;
}

/* Range inputs */
input[type="range"] {
  -webkit-appearance: none;
  appearance: none;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  height: 6px;
}

input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 16px;
  height: 16px;
  background: #6366f1;
  border-radius: 50%;
  cursor: pointer;
}
</style>
