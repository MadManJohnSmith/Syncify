<template>
  <div v-if="!isCollapsed" class="status-bar fixed bottom-0 left-0 right-0 h-8 bg-[#1e1e1e] border-t border-gray-800 flex items-center px-3 z-[50] text-xs">
    <!-- Left Section: Sync Status -->
    <div 
      class="status-section sync-status flex items-center gap-2 px-3 py-1 rounded hover:bg-white/5 cursor-pointer transition-colors"
      @click="toggleSyncPopover"
      :title="syncTooltip"
    >
      <!-- Syncing -->
      <template v-if="syncState === 'syncing'">
        <span class="material-symbols-outlined text-blue-400 text-sm animate-spin">sync</span>
        <span class="text-blue-400">Syncing {{ syncService }}...</span>
        <span class="text-blue-300">{{ syncProgress }}%</span>
      </template>
      
      <!-- Idle -->
      <template v-else-if="syncState === 'idle'">
        <span class="material-symbols-outlined text-green-400 text-sm">check_circle</span>
        <span class="text-gray-400">All synced</span>
      </template>
      
      <!-- Error -->
      <template v-else-if="syncState === 'error'">
        <span class="material-symbols-outlined text-red-400 text-sm">warning</span>
        <span class="text-red-400">Sync failed</span>
      </template>
      
      <!-- Paused -->
      <template v-else-if="syncState === 'paused'">
        <span class="material-symbols-outlined text-amber-400 text-sm">pause_circle</span>
        <span class="text-amber-400">Sync paused</span>
      </template>
    </div>
    
    <!-- Sync Popover -->
    <Transition name="slide-up">
      <div v-if="showSyncPopover" class="status-popover absolute bottom-10 left-3 w-64 bg-gray-900 border border-gray-700 rounded-xl shadow-xl p-4">
        <div class="flex items-center justify-between mb-3">
          <span class="font-medium text-white">Sync Details</span>
          <button @click="showSyncPopover = false" class="p-1 hover:bg-gray-700 rounded">
            <span class="material-symbols-outlined text-gray-400 text-sm">close</span>
          </button>
        </div>
        
        <div v-if="syncState === 'syncing'" class="space-y-2">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-blue-400 text-lg animate-spin">sync</span>
            <span class="text-gray-300">{{ syncService }}</span>
          </div>
          <div class="text-sm text-gray-400">{{ syncCurrent }} / {{ syncTotal }} items</div>
          <div class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
            <div class="h-full bg-blue-500 rounded-full transition-all" :style="{ width: syncProgress + '%' }"></div>
          </div>
          <div class="text-xs text-gray-500">ETA: ~{{ syncETA }}</div>
          <button @click="cancelSync" class="w-full mt-2 py-1.5 text-red-400 hover:bg-red-500/10 rounded-lg text-sm">
            Cancel Sync
          </button>
        </div>
        
        <div v-else-if="syncState === 'idle'" class="text-sm text-gray-400">
          <p>Last synced: {{ lastSyncTime }}</p>
          <button @click="syncAll" class="w-full mt-3 py-1.5 bg-primary/20 text-primary hover:bg-primary/30 rounded-lg text-sm">
            Sync All Services
          </button>
        </div>
        
        <div v-else-if="syncState === 'error'" class="text-sm">
          <p class="text-red-400 mb-2">{{ syncErrorMessage }}</p>
          <button @click="retrySync" class="w-full py-1.5 bg-primary/20 text-primary hover:bg-primary/30 rounded-lg text-sm">
            Retry Sync
          </button>
        </div>
      </div>
    </Transition>
    
    <!-- Center Section: Network & Operations -->
    <div class="flex-1 flex items-center justify-center gap-6">
      <!-- Network Status -->
      <div 
        class="status-section network-status flex items-center gap-2 px-3 py-1 rounded hover:bg-white/5 cursor-pointer transition-colors"
        @click="toggleNetworkPopover"
        title="Network status"
      >
        <span :class="['w-2 h-2 rounded-full', isOnline ? 'bg-green-400' : 'bg-red-400']"></span>
        <span :class="isOnline ? 'text-gray-400' : 'text-red-400'">{{ isOnline ? 'Online' : 'Offline' }}</span>
      </div>
      
      <!-- Active Operations -->
      <div v-if="activeOperations.length > 0" class="flex items-center gap-2 text-gray-400">
        <span class="material-symbols-outlined text-sm animate-pulse">pending</span>
        <span>{{ activeOperations[0] }}</span>
        <span v-if="activeOperations.length > 1" class="text-gray-500">+{{ activeOperations.length - 1 }} more</span>
      </div>
    </div>
    
    <!-- Network Popover -->
    <Transition name="slide-up">
      <div v-if="showNetworkPopover" class="status-popover absolute bottom-10 left-1/2 -translate-x-1/2 w-72 bg-gray-900 border border-gray-700 rounded-xl shadow-xl p-4">
        <div class="flex items-center justify-between mb-3">
          <span class="font-medium text-white">Network Status</span>
          <button @click="showNetworkPopover = false" class="p-1 hover:bg-gray-700 rounded">
            <span class="material-symbols-outlined text-gray-400 text-sm">close</span>
          </button>
        </div>
        
        <div class="space-y-3 text-sm">
          <div class="flex items-center justify-between">
            <span class="text-gray-400">Connection</span>
            <span class="text-gray-200">{{ connectionType }}</span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-gray-400">Download Speed</span>
            <span class="text-gray-200">{{ downloadSpeed }}</span>
          </div>
          
          <div class="border-t border-gray-700 pt-3 mt-3">
            <p class="text-gray-400 mb-2">Services</p>
            <div class="space-y-1.5">
              <div v-for="service in services" :key="service.name" class="flex items-center justify-between">
                <span class="text-gray-300">{{ service.name }}</span>
                <span v-if="service.online" class="text-green-400 flex items-center gap-1">
                  <span class="material-symbols-outlined text-sm">check</span>
                  Connected
                </span>
                <span v-else class="text-red-400 flex items-center gap-1">
                  <span class="material-symbols-outlined text-sm">close</span>
                  Offline
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
    
    <!-- Right Section: Storage Info -->
    <div 
      class="status-section storage-info flex items-center gap-3 px-3 py-1 rounded hover:bg-white/5 cursor-pointer transition-colors"
      @click="toggleStoragePopover"
      @contextmenu.prevent="showStorageMenu"
      title="Storage usage"
    >
      <span class="material-symbols-outlined text-gray-400 text-sm">hard_drive_2</span>
      <span class="text-gray-400">{{ storageUsed }} / {{ storageTotal }}</span>
      
      <!-- Mini Progress Bar -->
      <div class="mini-progress-bar w-10 h-1 bg-gray-700 rounded-full overflow-hidden">
        <div 
          :class="['h-full rounded-full transition-all', storageProgressColor]"
          :style="{ width: storagePercent + '%' }"
        ></div>
      </div>
    </div>
    
    <!-- Storage Popover -->
    <Transition name="slide-up">
      <div v-if="showStoragePopover" class="status-popover absolute bottom-10 right-3 w-72 bg-gray-900 border border-gray-700 rounded-xl shadow-xl p-4">
        <div class="flex items-center justify-between mb-3">
          <span class="font-medium text-white">Storage Details</span>
          <button @click="showStoragePopover = false" class="p-1 hover:bg-gray-700 rounded">
            <span class="material-symbols-outlined text-gray-400 text-sm">close</span>
          </button>
        </div>
        
        <div class="space-y-3 text-sm">
          <div class="flex items-center justify-between">
            <span class="text-gray-400">Library Size</span>
            <span class="text-gray-200">{{ storageUsed }}</span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-gray-400">Available Space</span>
            <span class="text-gray-200">{{ storageAvailable }}</span>
          </div>
          
          <div class="h-2 bg-gray-700 rounded-full overflow-hidden">
            <div 
              :class="['h-full rounded-full transition-all', storageProgressColor]"
              :style="{ width: storagePercent + '%' }"
            ></div>
          </div>
          
          <div class="border-t border-gray-700 pt-3 space-y-1.5">
            <p class="text-gray-400 mb-2">Breakdown</p>
            <div v-for="format in storageBreakdown" :key="format.name" class="flex items-center justify-between">
              <span class="text-gray-300">{{ format.name }}</span>
              <span class="text-gray-400">{{ format.size }} ({{ format.percent }}%)</span>
            </div>
          </div>
          
          <div class="pt-2">
            <p class="text-gray-500 text-xs truncate mb-2">{{ storagePath }}</p>
            <button @click="changeLocation" class="w-full py-1.5 bg-primary/20 text-primary hover:bg-primary/30 rounded-lg text-sm">
              Change Location
            </button>
          </div>
        </div>
      </div>
    </Transition>
    
    <!-- Collapse Button -->
    <button 
      @click="collapse" 
      class="ml-3 p-1 hover:bg-white/5 rounded transition-colors"
      title="Collapse status bar"
    >
      <span class="material-symbols-outlined text-gray-500 text-sm">keyboard_arrow_down</span>
    </button>
  </div>
  
  <!-- Collapsed State: Floating Button -->
  <Transition name="fade">
    <button 
      v-if="isCollapsed"
      @click="expand"
      class="status-collapsed fixed bottom-4 right-4 w-10 h-10 bg-gray-800 border border-gray-700 rounded-full shadow-lg flex items-center justify-center hover:bg-gray-700 transition-colors z-[50]"
      title="Show status bar"
    >
      <span class="material-symbols-outlined text-gray-400">keyboard_arrow_up</span>
    </button>
  </Transition>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useGlobalTasks } from '../composables/useGlobalTasks'
import { useToast } from '../composables/useToast'
import { getStorageStats, type StorageStats } from '../api/storage'
import { getServiceStatuses } from '../api/accounts'
import { saveSetting } from '../api/settings'
import { open } from '@tauri-apps/plugin-dialog'

// State
const isCollapsed = ref(false)
const showSyncPopover = ref(false)
const showNetworkPopover = ref(false)
const showStoragePopover = ref(false)

// Global Tasks Integration
const { activeTasks, overallProgress, hasActiveTasks, addTask } = useGlobalTasks()
const router = useRouter()
const toast = useToast()

let storageInterval: ReturnType<typeof setInterval> | undefined
const isRefreshing = ref(false)

// Sync Status (Computed from global tasks)
type SyncState = 'syncing' | 'idle' | 'error' | 'paused'
const syncState = computed((): SyncState => {
  if (activeTasks.value.some(t => t.type === 'sync' && t.status === 'running')) return 'syncing'
  if (hasActiveTasks.value) return 'syncing'
  return 'idle' 
})

const syncService = computed(() => {
  const syncTask = activeTasks.value.find(t => t.type === 'sync') || activeTasks.value[0]
  if (!syncTask) return 'Spotify'
  return syncTask.service || syncTask.name.replace(/^Syncing\s+/, '') || 'Service'
})

const syncProgress = computed(() => overallProgress.value)
const syncCurrent = computed(() => {
   return activeTasks.value.reduce((acc, t) => acc + (t.current || 0), 0)
})
const syncTotal = computed(() => {
   return activeTasks.value.reduce((acc, t) => acc + (t.total || 0), 0)
})
const syncETA = ref('Calculating...')
const syncErrorMessage = ref('Connection timeout')
const lastSyncTime = ref('Just now')

const syncTooltip = computed(() => {
  switch (syncState.value) {
    case 'syncing': return `Processing ${activeTasks.value.length} tasks: ${Math.round(syncProgress.value)}%`
    case 'idle': return 'All services synced'
    case 'error': return 'Click to view sync error'
    case 'paused': return 'Sync paused'
    default: return ''
  }
})

// Network
const isOnline = ref(true)
const connectionType = ref('Wi-Fi')
const downloadSpeed = ref('12.3 MB/s')
const services = ref([
  { name: 'Spotify', online: true },
  { name: 'Qobuz', online: true },
  { name: 'Tidal', online: false },
])

// Active Operations
const activeOperations = computed(() => activeTasks.value.map(t => t.name))

// Storage
const storageUsed = ref('0 B')
const storageTotal = ref('0 B')
const storageAvailable = ref('0 B')
const storagePercent = ref(0)
const storagePath = ref('Not set')
const storageBreakdown = ref<{ name: string; size: string; percent: number }[]>([])

// Format helper
function formatBytes(bytes: number, decimals = 2) {
  if (bytes === 0) return '0 B'
  const k = 1024
  const dm = decimals < 0 ? 0 : decimals
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i]
}

const storageProgressColor = computed(() => {
  if (storagePercent.value > 90) return 'bg-red-500'
  if (storagePercent.value > 70) return 'bg-amber-500'
  return 'bg-green-500'
})

// Methods
function toggleSyncPopover() {
  showSyncPopover.value = !showSyncPopover.value
  showNetworkPopover.value = false
  showStoragePopover.value = false
}

function toggleNetworkPopover() {
  showNetworkPopover.value = !showNetworkPopover.value
  showSyncPopover.value = false
  showStoragePopover.value = false
  if (showNetworkPopover.value) {
    fetchServices()
  }
}

function toggleStoragePopover() {
  showStoragePopover.value = !showStoragePopover.value
  showSyncPopover.value = false
  showNetworkPopover.value = false
  if (showStoragePopover.value) {
    fetchStorage()
  }
}

async function fetchStorage() {
  try {
    const stats = await getStorageStats()
    updateStorageUI(stats)
  } catch (e) {
    console.error('Failed to fetch storage stats:', e)
  }
}

function updateStorageUI(stats: StorageStats) {
  storageUsed.value = formatBytes(stats.used_bytes)
  storageTotal.value = formatBytes(stats.total_bytes)
  storageAvailable.value = formatBytes(stats.available_bytes)
  storagePath.value = stats.path
  
  if (stats.total_bytes > 0) {
    storagePercent.value = Math.round((stats.used_bytes / stats.total_bytes) * 100)
  } else {
    storagePercent.value = 0
  }

  storageBreakdown.value = stats.breakdown.map(b => ({
    name: b.format,
    size: formatBytes(b.size_bytes),
    percent: stats.used_bytes > 0 ? Math.round((b.size_bytes / stats.used_bytes) * 100) : 0
  }))
}

async function fetchServices() {
  try {
    const statuses = await getServiceStatuses()
    services.value = statuses.map(s => ({
      name: s.name,
      online: s.connected
    }))
    
    // Simple global online check: if at least one service is configured/online
    isOnline.value = services.value.some(s => s.online)
  } catch (e) {
    console.error('Failed to fetch service statuses:', e)
  }
}

function cancelSync() {
  showSyncPopover.value = false
  toast.info('Go to Downloads to manage active syncs')
  router.push('/downloads')
}

function syncAll() {
  showSyncPopover.value = false
  toast.info('Starting sync for all services...')
  router.push('/downloads')
}

function retrySync() {
  showSyncPopover.value = false
  router.push('/downloads')
}

async function changeLocation() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Download Folder'
    })
    
    if (selected && typeof selected === 'string') {
      await saveSetting('download_path', selected)
      storagePath.value = selected
      toast.success('Download location updated')
      await fetchStorage()
    }
  } catch (e) {
    console.error('Failed to change location:', e)
    toast.error('Failed to update download location')
  }
  showStoragePopover.value = false
}

function showStorageMenu(e: MouseEvent) {
  // Context menu would go here
}

function collapse() {
  isCollapsed.value = true
  localStorage.setItem('syncify_statusbar_collapsed', 'true')
}

function expand() {
  isCollapsed.value = false
  localStorage.setItem('syncify_statusbar_collapsed', 'false')
}

// Close popovers on outside click
function handleOutsideClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (!target.closest('.status-popover') && !target.closest('.status-section')) {
    showSyncPopover.value = false
    showNetworkPopover.value = false
    showStoragePopover.value = false
  }
}

// Check online status
function checkOnline() {
  isOnline.value = navigator.onLine
}

onMounted(async () => {
  document.addEventListener('click', handleOutsideClick)
  window.addEventListener('online', checkOnline)
  window.addEventListener('offline', checkOnline)
  
  const savedCollapsed = typeof localStorage !== 'undefined' && localStorage && typeof localStorage.getItem === 'function'
    ? localStorage.getItem('syncify_statusbar_collapsed')
    : null
  if (savedCollapsed === 'true') isCollapsed.value = true

  // Initial fetch
  await Promise.all([
    fetchStorage(),
    fetchServices()
  ])

  // Periodic refresh
  storageInterval = setInterval(fetchStorage, 60000)
})

onUnmounted(() => {
  document.removeEventListener('click', handleOutsideClick)
  window.removeEventListener('online', checkOnline)
  window.removeEventListener('offline', checkOnline)
  
  if (storageInterval) {
    clearInterval(storageInterval)
  }
})

// Demo: toggle sync state
function demoSync() {
  // Use global tasks for demo
  addTask({
      id: 'demo-' + Date.now(),
      type: 'sync',
      name: 'Demo Sync Task',
      status: 'running',
      progress: 0,
      total: 100,
      current: 0
  })
}

defineExpose({ demoSync, syncState, isCollapsed })
</script>

<style scoped>
/* Slide up animation */
.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.2s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(10px);
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

/* Spin animation */
@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 1s linear infinite;
}

/* Pulse animation */
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.animate-pulse {
  animation: pulse 2s ease-in-out infinite;
}
</style>
