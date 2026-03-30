<template>
  <div class="app-updater">
    <!-- Update Available Toast -->
    <Transition name="slide-up">
      <div 
        v-if="showUpdateToast && !showUpdateModal" 
        class="update-notification fixed bottom-6 right-6 z-[100] bg-white dark:bg-surface-dark rounded-xl shadow-2xl border border-gray-200 dark:border-border-dark p-4 w-80"
      >
        <div class="flex items-start gap-3">
          <div class="w-10 h-10 rounded-xl bg-primary/10 flex items-center justify-center shrink-0">
            <span class="material-symbols-outlined text-primary">system_update</span>
          </div>
          <div class="flex-1">
            <p class="font-semibold text-gray-900 dark:text-white">Syncify v{{ updateInfo.version }} available</p>
            <p class="text-sm text-gray-500 mt-0.5">A new version is ready to install</p>
            <div class="flex gap-2 mt-3">
              <button @click="openUpdateModal" class="px-3 py-1.5 bg-primary hover:bg-primary-hover text-white text-sm font-medium rounded-lg">
                View Update
              </button>
              <button @click="dismissToast" class="px-3 py-1.5 text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight text-sm rounded-lg">
                Dismiss
              </button>
            </div>
          </div>
          <button @click="dismissToast" class="text-gray-400 hover:text-gray-600">
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>
      </div>
    </Transition>
    
    <!-- Critical Update Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showCriticalModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="p-6 text-center">
              <div class="w-16 h-16 mx-auto rounded-full bg-red-500/10 flex items-center justify-center mb-4">
                <span class="material-symbols-outlined text-red-500 text-3xl">security_update_warning</span>
              </div>
              <h3 class="text-xl font-bold text-gray-900 dark:text-white mb-2">Important Security Update</h3>
              <p class="text-gray-500">Version {{ updateInfo.version }} includes critical security fixes. Please update as soon as possible.</p>
            </div>
            <div class="px-6 pb-6 flex flex-col gap-2">
              <button @click="installUpdate" class="w-full py-3 bg-red-500 hover:bg-red-600 text-white font-semibold rounded-xl">
                Update Now
              </button>
              <button @click="remindLater(60)" class="w-full py-3 text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-xl">
                Remind Me in 1 Hour
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Update Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showUpdateModal" class="update-modal fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="showUpdateModal = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-xl overflow-hidden shadow-2xl max-h-[85vh] flex flex-col">
            <!-- Header -->
            <div class="px-6 py-5 border-b border-gray-200 dark:border-border-dark shrink-0">
              <div class="flex items-center gap-4">
                <div class="w-14 h-14 rounded-2xl bg-gradient-to-br from-primary to-primary-600 flex items-center justify-center shadow-lg">
                  <span class="material-symbols-outlined text-white text-3xl">music_note</span>
                </div>
                <div>
                  <h2 class="text-xl font-bold text-gray-900 dark:text-white">Update Available</h2>
                  <div class="flex items-center gap-2 mt-1">
                    <span class="text-sm text-gray-500">v{{ currentVersion }}</span>
                    <span class="material-symbols-outlined text-gray-400 text-sm">arrow_forward</span>
                    <span class="px-2 py-0.5 bg-green-500/10 text-green-600 text-sm font-medium rounded">v{{ updateInfo.version }}</span>
                  </div>
                </div>
                <button @click="showUpdateModal = false" class="ml-auto p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">
                  <span class="material-symbols-outlined text-gray-400">close</span>
                </button>
              </div>
              <p class="text-sm text-gray-500 mt-3">Released {{ updateInfo.releaseDate }}</p>
            </div>
            
            <!-- Changelog -->
            <div class="changelog flex-1 overflow-y-auto px-6 py-5 custom-scrollbar">
              <!-- New Features -->
              <div v-if="updateInfo.features.length > 0" class="changelog-section mb-5">
                <h4 class="flex items-center gap-2 text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3">
                  <span class="text-lg">✨</span> New Features
                </h4>
                <ul class="space-y-2">
                  <li v-for="feature in updateInfo.features" :key="feature" class="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                    <span class="text-green-500 mt-1">•</span>
                    {{ feature }}
                  </li>
                </ul>
              </div>
              
              <!-- Improvements -->
              <div v-if="updateInfo.improvements.length > 0" class="changelog-section mb-5">
                <h4 class="flex items-center gap-2 text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3">
                  <span class="text-lg">🔧</span> Improvements
                </h4>
                <ul class="space-y-2">
                  <li v-for="item in updateInfo.improvements" :key="item" class="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                    <span class="text-blue-500 mt-1">•</span>
                    {{ item }}
                  </li>
                </ul>
              </div>
              
              <!-- Bug Fixes -->
              <div v-if="updateInfo.bugFixes.length > 0" class="changelog-section mb-5">
                <h4 class="flex items-center gap-2 text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3">
                  <span class="text-lg">🐛</span> Bug Fixes
                </h4>
                <ul class="space-y-2">
                  <li v-for="fix in updateInfo.bugFixes" :key="fix" class="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                    <span class="text-amber-500 mt-1">•</span>
                    {{ fix }}
                  </li>
                </ul>
              </div>
              
              <!-- Breaking Changes -->
              <div v-if="updateInfo.breakingChanges.length > 0" class="changelog-section mb-5">
                <h4 class="flex items-center gap-2 text-sm font-semibold text-red-600 mb-3">
                  <span class="text-lg">⚠️</span> Breaking Changes
                </h4>
                <ul class="space-y-2">
                  <li v-for="change in updateInfo.breakingChanges" :key="change" class="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                    <span class="text-red-500 mt-1">•</span>
                    {{ change }}
                  </li>
                </ul>
              </div>
            </div>
            
            <!-- Download Info & Actions -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark shrink-0">
              <div class="flex items-center justify-between text-sm text-gray-500 mb-4">
                <span>Download size: {{ updateInfo.downloadSize }}</span>
                <a href="#" class="text-primary hover:underline flex items-center gap-1">
                  View full release notes
                  <span class="material-symbols-outlined text-sm">open_in_new</span>
                </a>
              </div>
              <div class="flex gap-3">
                <button @click="installUpdate" class="flex-1 py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl">
                  Install and Restart
                </button>
                <button @click="downloadBackground" class="px-4 py-3 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-xl">
                  Download in Background
                </button>
              </div>
              <div class="flex items-center justify-center gap-4 mt-3">
                <button @click="remindLater(24 * 60)" class="text-sm text-gray-500 hover:text-gray-700">Remind Me Later</button>
                <button @click="skipVersion" class="text-sm text-gray-400 hover:text-gray-500">Skip This Version</button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Download Progress Card -->
    <Transition name="slide-up">
      <div 
        v-if="isDownloading" 
        class="download-progress fixed bottom-6 right-6 z-[100] bg-white dark:bg-surface-dark rounded-xl shadow-2xl border border-gray-200 dark:border-border-dark p-4 w-72"
      >
        <div class="flex items-center justify-between mb-2">
          <span class="text-sm font-medium text-gray-900 dark:text-white">
            {{ downloadComplete ? 'Update ready to install' : 'Downloading update...' }}
          </span>
          <span class="text-xs text-gray-500">{{ downloadProgress }}%</span>
        </div>
        <div class="h-2 bg-gray-100 dark:bg-surface-highlight rounded-full overflow-hidden mb-2">
          <div 
            class="h-full bg-primary rounded-full transition-all duration-300"
            :style="{ width: downloadProgress + '%' }"
          ></div>
        </div>
        <div class="flex items-center justify-between text-xs text-gray-500">
          <span>{{ downloadedSize }} / {{ updateInfo.downloadSize }}</span>
          <span v-if="!downloadComplete">{{ downloadSpeed }}</span>
        </div>
        <button 
          v-if="downloadComplete"
          @click="installUpdate" 
          class="w-full mt-3 py-2 bg-primary hover:bg-primary-hover text-white text-sm font-medium rounded-lg"
        >
          Install and Restart
        </button>
      </div>
    </Transition>
    
    <!-- Install Prompt Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showInstallPrompt" class="install-prompt fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-sm overflow-hidden shadow-2xl text-center">
            <div class="p-6">
              <span class="material-symbols-outlined text-5xl text-primary mb-4 block">restart_alt</span>
              <h3 class="text-lg font-bold text-gray-900 dark:text-white mb-2">Ready to Restart</h3>
              <p class="text-gray-500 mb-4">Save your work. Syncify will restart to complete the update.</p>
              <p class="text-3xl font-bold text-primary">{{ restartCountdown }}</p>
              <p class="text-sm text-gray-400">seconds</p>
            </div>
            <div class="px-6 pb-6 flex gap-3">
              <button @click="cancelInstall" class="flex-1 py-2.5 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-xl">
                Cancel
              </button>
              <button @click="restartNow" class="flex-1 py-2.5 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl">
                Restart Now
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Update Success Modal (after restart) -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showSuccessModal" class="whats-new-viewer fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="showSuccessModal = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl text-center">
            <div class="p-6">
              <div class="text-6xl mb-4">🎉</div>
              <h3 class="text-xl font-bold text-gray-900 dark:text-white mb-2">Update Successful!</h3>
              <p class="text-gray-500 mb-4">Welcome to Syncify v{{ currentVersion }}</p>
              
              <div class="text-left bg-gray-50 dark:bg-surface-highlight rounded-xl p-4 mb-4">
                <h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">What's New:</h4>
                <ul class="space-y-1">
                  <li v-for="(feature, i) in updateInfo.features.slice(0, 3)" :key="i" class="text-sm text-gray-600 dark:text-gray-400 flex items-start gap-2">
                    <span class="text-green-500">•</span>
                    {{ feature }}
                  </li>
                </ul>
              </div>
            </div>
            <div class="px-6 pb-6">
              <button @click="showSuccessModal = false" class="w-full py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl">
                Get Started
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Update Badge (for Settings tab) -->
    <div v-if="hasUpdate" class="update-badge absolute -top-1 -right-1 w-3 h-3 bg-primary rounded-full border-2 border-white dark:border-surface-dark"></div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

// State
const showUpdateToast = ref(false)
const showUpdateModal = ref(false)
const showCriticalModal = ref(false)
const showInstallPrompt = ref(false)
const showSuccessModal = ref(false)
const isDownloading = ref(false)
const downloadComplete = ref(false)
const downloadProgress = ref(0)
const downloadedSize = ref('0 MB')
const downloadSpeed = ref('0 MB/s')
const restartCountdown = ref(10)
const hasUpdate = ref(true)

// Version info
const currentVersion = ref('2.0.5')

// Mock update info
const updateInfo = ref({
  version: '2.1.0',
  releaseDate: 'December 23, 2024',
  downloadSize: '45 MB',
  isCritical: false,
  features: [
    'Added multi-account support for services',
    'New migration wizard with match preview',
    'Batch metadata editing with templates',
    'Advanced lyrics sync editor with waveform display',
  ],
  improvements: [
    'Faster library syncing (2x speed)',
    'Better lyrics matching accuracy',
    'Improved UI responsiveness',
    'Reduced memory usage',
  ],
  bugFixes: [
    'Fixed crash when importing large playlists',
    'Resolved Qobuz authentication issue',
    'Corrected metadata quality scoring',
    'Fixed download resume issues',
  ],
  breakingChanges: [
    'Changed default download folder structure',
  ],
})

// Methods
function dismissToast() {
  showUpdateToast.value = false
}

function openUpdateModal() {
  showUpdateToast.value = false
  showUpdateModal.value = true
}

function downloadBackground() {
  showUpdateModal.value = false
  isDownloading.value = true
  simulateDownload()
}

function simulateDownload() {
  let progress = 0
  const interval = setInterval(() => {
    progress += Math.random() * 5
    if (progress >= 100) {
      progress = 100
      downloadComplete.value = true
      clearInterval(interval)
    }
    downloadProgress.value = Math.floor(progress)
    downloadedSize.value = `${Math.floor(progress * 0.45)} MB`
    downloadSpeed.value = `${(Math.random() * 3 + 1).toFixed(1)} MB/s`
  }, 300)
}

function installUpdate() {
  showUpdateModal.value = false
  isDownloading.value = false
  showInstallPrompt.value = true
  startCountdown()
}

function startCountdown() {
  restartCountdown.value = 10
  const interval = setInterval(() => {
    restartCountdown.value--
    if (restartCountdown.value <= 0) {
      clearInterval(interval)
      restartNow()
    }
  }, 1000)
}

function cancelInstall() {
  showInstallPrompt.value = false
}

function restartNow() {
  showInstallPrompt.value = false
  // In real app, would trigger restart
  // For demo, show success modal
  currentVersion.value = updateInfo.value.version
  hasUpdate.value = false
  setTimeout(() => {
    showSuccessModal.value = true
  }, 500)
}

function remindLater(minutes: number) {
  showUpdateModal.value = false
  showCriticalModal.value = false
  showUpdateToast.value = false
  // Schedule reminder
  setTimeout(() => {
    showUpdateToast.value = true
  }, minutes * 60 * 1000)
}

function skipVersion() {
  showUpdateModal.value = false
  hasUpdate.value = false
  localStorage.setItem('syncify_skipped_version', updateInfo.value.version)
}

function checkForUpdates() {
  // Simulate update check
  const skippedVersion = localStorage.getItem('syncify_skipped_version')
  if (skippedVersion === updateInfo.value.version) {
    hasUpdate.value = false
    return
  }
  
  hasUpdate.value = true
  if (updateInfo.value.isCritical) {
    showCriticalModal.value = true
  } else {
    showUpdateToast.value = true
  }
}

onMounted(() => {
  // Check for updates on startup (delayed)
  setTimeout(() => {
    checkForUpdates()
  }, 2000)
})

// Expose for external use
defineExpose({
  checkForUpdates,
  openUpdateModal,
  hasUpdate
})
</script>

<style scoped>
/* Slide up animation */
.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.3s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(20px);
}

/* Fade animation */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Custom scrollbar */
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.1);
  border-radius: 3px;
}

.dark .custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
}
</style>
