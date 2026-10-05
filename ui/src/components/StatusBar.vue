<template>
  <div v-if="!isCollapsed" :class="['status-bar fixed bottom-0 left-0 right-0 h-8 bg-[#1e1e1e] border-t border-gray-800 flex items-center px-3 text-xs', (showSyncPopover || showNetworkPopover || showStoragePopover) ? 'z-[160]' : 'z-[50]']">
    <!-- Left Section: Sync Status -->
    <div 
      class="status-section sync-status flex items-center gap-2 px-3 py-1 rounded hover:bg-white/5 cursor-pointer transition-colors"
      @click="toggleSyncPopover"
      :title="syncTooltip"
    >
      <!-- Syncing -->
      <template v-if="syncState === 'syncing'">
        <span class="material-symbols-outlined text-blue-400 text-sm animate-spin">sync</span>
        <span class="text-blue-400">
          <template v-if="activeTasks.length > 1">{{ activeTasks.length }} tasks</template>
          <template v-else>Syncing {{ syncService }}...</template>
        </span>
        <span v-if="hasDeterminateProgress" class="text-blue-300">{{ syncProgress }}%</span>
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
      <div v-if="showSyncPopover" :class="['status-popover absolute z-[160] left-3 w-64 bg-gray-900 border border-gray-700 rounded-xl shadow-xl p-4', current ? 'bottom-26' : 'bottom-10']">
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
          <div v-if="taskProgressRows.length > 1" class="text-xs text-gray-500">
            {{ taskProgressRows.length }} tasks in progress
          </div>
          <div v-for="row in taskProgressRows" :key="row.id" class="space-y-1">
            <div class="flex items-center justify-between gap-2">
              <span class="text-gray-300 text-xs truncate">{{ row.name }}</span>
              <span class="text-gray-400 text-[11px] font-mono shrink-0">
                <template v-if="row.determinate">{{ row.detail }} · {{ Math.round(row.progress) }}%</template>
                <template v-else>{{ row.detail }}</template>
              </span>
            </div>
            <div v-if="row.determinate" class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
              <div class="h-full bg-blue-500 rounded-full transition-all" :style="{ width: row.progress + '%' }"></div>
            </div>
            <div v-else class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
              <div class="h-full w-1/3 bg-blue-500/60 rounded-full animate-pulse"></div>
            </div>
          </div>
          <div v-if="syncETA" class="text-xs text-gray-500">ETA: ~{{ syncETA }}</div>
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

        <div v-else-if="syncState === 'paused'" class="text-sm">
          <p class="text-amber-400 mb-2 flex items-center gap-2">
            <span class="material-symbols-outlined">pause_circle</span>
            Sync paused
          </p>
          <button @click="goToDownloads" class="w-full py-1.5 bg-primary/20 text-primary hover:bg-primary/30 rounded-lg text-sm">
            Manage in Downloads
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
      
      <!-- Global Progress: covers every active task at once -->
      <div v-if="activeTasks.length > 0" class="flex items-center gap-3 text-gray-400">
        <span class="material-symbols-outlined text-sm animate-pulse">pending</span>
        <div class="w-44">
          <div class="flex items-center justify-between gap-2 mb-0.5">
            <span class="text-gray-300 text-[11px] truncate max-w-[120px]">{{ activeOperations[0] }}</span>
            <span class="text-gray-500 text-[11px] shrink-0">{{ activeTasks.length }} {{ activeTasks.length === 1 ? 'task' : 'tasks' }}</span>
          </div>
          <div
            class="h-1 bg-gray-700 rounded-full overflow-hidden"
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax="100"
            :aria-valuenow="hasDeterminateProgress ? syncProgress : undefined"
            :aria-label="hasDeterminateProgress ? 'Overall task progress' : 'Overall task progress (indeterminate)'"
          >
            <div
              v-if="hasDeterminateProgress"
              class="h-full bg-blue-500 rounded-full transition-all"
              :style="{ width: syncProgress + '%' }"
            ></div>
            <div v-else class="h-full w-1/3 bg-blue-500/60 rounded-full animate-pulse"></div>
          </div>
        </div>
        <span v-if="hasDeterminateProgress" class="text-gray-400 text-[11px] font-mono">{{ syncProgress }}%</span>
      </div>
    </div>
    
    <!-- Network Popover -->
    <Transition name="slide-up">
      <div v-if="showNetworkPopover" :class="['status-popover absolute z-[160] left-1/2 -translate-x-1/2 w-72 bg-gray-900 border border-gray-700 rounded-xl shadow-xl p-4', current ? 'bottom-26' : 'bottom-10']">
        <div class="flex items-center justify-between mb-3">
          <span class="font-medium text-white">Network Status</span>
          <button @click="showNetworkPopover = false" class="p-1 hover:bg-gray-700 rounded">
            <span class="material-symbols-outlined text-gray-400 text-sm">close</span>
          </button>
        </div>
        
        <div class="space-y-3 text-sm">
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
      <div v-if="showStoragePopover" :class="['status-popover absolute z-[160] right-3 w-72 bg-gray-900 border border-gray-700 rounded-xl shadow-xl p-4', current ? 'bottom-26' : 'bottom-10']">
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
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useGlobalTasks } from '../composables/useGlobalTasks'
import { useEventBus, TauriEvents } from '../composables/useEventBus'
import { useToast } from '../composables/useToast'
import { usePlayer } from '../composables/usePlayer'
import { getStorageStats, type StorageStats } from '../api/storage'
import { getServiceStatuses } from '../api/accounts'
import { saveSetting } from '../api/settings'
import { open } from '@tauri-apps/plugin-dialog'
import {
  deriveSyncState,
  deriveSyncErrorMessage,
  estimateRemainingMs,
  formatDownloadSpeed,
  formatRelativeTime,
  formatRemainingDuration,
  isTerminalDownloadStatus,
  parseSqliteUtc,
} from '../utils/statusTelemetry'

// State
const isCollapsed = ref(false)
const showSyncPopover = ref(false)
const showNetworkPopover = ref(false)
const showStoragePopover = ref(false)

// Global Tasks Integration
const { activeTasks, allTasks, aggregateProgress, hasDeterminateProgress } = useGlobalTasks()
const eventBus = useEventBus()
const router = useRouter()
const toast = useToast()
const { current } = usePlayer()

let storageInterval: ReturnType<typeof setInterval> | undefined
let etaInterval: ReturnType<typeof setInterval> | undefined
let unlistenDownloadProgress: (() => void) | undefined
const isRefreshing = ref(false)

// Sync Status (Derived from real global task events — sync progress, failures
// and pauses are reported by the sync/import event listeners in useGlobalTasks)
type SyncState = 'syncing' | 'idle' | 'error' | 'paused'
const syncState = computed((): SyncState => deriveSyncState(allTasks.value))

const syncService = computed(() => {
  const syncTask = activeTasks.value.find(t => t.type === 'sync') || activeTasks.value[0]
  if (!syncTask) return 'Spotify'
  return syncTask.service || syncTask.name.replace(/^Syncing\s+/, '') || 'Service'
})

const syncProgress = computed(() => aggregateProgress.value)

// One row per task: summing `current`/`total` across tasks mixed a
// thousand-file scan with a three-track sync and produced meaningless counts.
interface TaskProgressRow {
  id: string
  name: string
  detail: string
  progress: number
  determinate: boolean
}

const taskProgressRows = computed<TaskProgressRow[]>(() =>
  activeTasks.value.map(task => ({
    id: task.id,
    name: task.name,
    detail: (task.total ?? 0) > 0
      ? `${task.current ?? 0} / ${task.total}`
      : task.description || task.phase || 'In progress',
    progress: Math.min(100, Math.max(0, task.progress ?? 0)),
    determinate: (task.total ?? 0) > 0
  }))
)

// Real ETA estimate: extrapolate the elapsed time of the primary running sync
// task. Hidden (null) while there is no basis to estimate from.
const runningSyncTask = computed(() => activeTasks.value.find(t => t.type === 'sync' && t.status === 'running'))
const nowMs = ref(Date.now())
const syncETA = computed<string | null>(() => {
  const task = runningSyncTask.value
  if (!task) return null
  const remaining = estimateRemainingMs(task.startedAt, task.progress ?? 0, nowMs.value)
  return remaining === null ? null : formatRemainingDuration(remaining)
})
watch(() => syncState.value === 'syncing', (syncing) => {
  if (syncing && etaInterval === undefined) {
    etaInterval = setInterval(() => { nowMs.value = Date.now() }, 1000)
  } else if (!syncing && etaInterval !== undefined) {
    clearInterval(etaInterval)
    etaInterval = undefined
  }
}, { immediate: true })

// Real error message from the most recent failed sync task (no invented text)
const syncErrorMessage = computed(() => deriveSyncErrorMessage(allTasks.value) ?? 'Sync failed')

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
// Real download speed from `syncify:download_progress` events (KB/s); null
// while no download is running so the popover shows an em dash instead of a
// made-up number.
const downloadSpeedKBps = ref<number | null>(null)
const downloadSpeed = computed(() => formatDownloadSpeed(downloadSpeedKBps.value))
const services = ref<Array<{ name: string; online: boolean; lastSynced: string | null }>>([])

// Last sync time from the real `last_synced` of the connected accounts
// (refreshed with the service statuses).
const lastSyncTime = computed(() => {
  const dates = services.value
    .map(s => (s.lastSynced ? parseSqliteUtc(s.lastSynced) : null))
    .filter((d): d is Date => d !== null)
  if (dates.length === 0) return 'Never'
  const latest = Math.max(...dates.map(d => d.getTime()))
  return formatRelativeTime(new Date(latest))
})

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
  if (showSyncPopover.value) {
    // Refresh so "Last synced" reflects the real accounts state
    fetchServices()
  }
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
      online: s.connected,
      lastSynced: s.last_synced ?? null,
    }))

    // Simple global online check: if at least one service is configured/online
    isOnline.value = services.value.some(s => s.online)
  } catch (e) {
    console.error('Failed to fetch service statuses:', e)
  }
}

// Refresh the services (and therefore "Last synced") once a whole run
// finishes, not per item: batches (downloads, syncs, enrichment) complete one
// task at a time, and a flip-per-item watch fired get_service_statuses every
// few seconds for as long as a batch ran.
let servicesRefreshTimer: ReturnType<typeof setTimeout> | undefined
function scheduleFetchServices(delay = 1500) {
  if (servicesRefreshTimer) clearTimeout(servicesRefreshTimer)
  servicesRefreshTimer = setTimeout(() => {
    servicesRefreshTimer = undefined
    fetchServices()
  }, delay)
}

watch(() => activeTasks.value.length, (count, prev) => {
  if (prev > 0 && count === 0) {
    scheduleFetchServices(1000)
  }
})

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

function goToDownloads() {
  showSyncPopover.value = false
  router.push('/downloads')
}

// Track the real download throughput reported by the backend
function handleDownloadProgress(payload: any) {
  if (!payload) return
  const status = typeof payload.status === 'string' ? payload.status : ''
  if (payload.terminal === true || isTerminalDownloadStatus(status)) {
    downloadSpeedKBps.value = null
    return
  }
  const instant = Number(payload.instant_kbps)
  const average = Number(payload.average_kbps)
  downloadSpeedKBps.value = instant > 0 ? instant : (average > 0 ? average : null)
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

  eventBus.on(TauriEvents.DOWNLOAD_PROGRESS, handleDownloadProgress).then(unlisten => {
    if (unlisten) unlistenDownloadProgress = unlisten
  })

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

  if (unlistenDownloadProgress) {
    unlistenDownloadProgress()
    unlistenDownloadProgress = undefined
  }
  if (etaInterval) {
    clearInterval(etaInterval)
    etaInterval = undefined
  }

  if (storageInterval) {
    clearInterval(storageInterval)
  }

  if (servicesRefreshTimer) {
    clearTimeout(servicesRefreshTimer)
    servicesRefreshTimer = undefined
  }
})

defineExpose({ syncState, isCollapsed })
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

.bottom-26 {
  bottom: 6.5rem;
}
</style>
