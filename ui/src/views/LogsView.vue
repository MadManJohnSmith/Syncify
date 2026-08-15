<template>
  <div class="h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex flex-wrap items-center justify-between gap-4 bg-white dark:bg-[#101723]">
      <div class="flex items-center gap-4">
        <h1 class="text-lg font-bold tracking-tight text-gray-900 dark:text-white">System Logs</h1>
        
        <!-- Status summary badge -->
        <div v-if="enrichmentStatus" class="flex items-center gap-2 text-xs">
          <span class="px-2 py-0.5 rounded-full font-medium" :class="enrichmentStatus.is_paused ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20' : 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'">
            {{ enrichmentStatus.is_paused ? 'Enrichment Paused' : 'Enrichment Active' }}
          </span>
          <span class="text-text-secondary">
            Pending: <strong class="text-white">{{ enrichmentStatus.pending_count }}</strong> |
            Done: <strong class="text-white">{{ enrichmentStatus.completed_count }}</strong>
          </span>
        </div>
      </div>
      
      <div class="flex items-center gap-3">
        <!-- Worker Controls -->
        <button 
          @click="toggleWorkerPause" 
          class="px-3 py-1.5 rounded-lg text-xs font-medium border flex items-center gap-1.5 transition-colors"
          :class="enrichmentStatus?.is_paused ? 'border-emerald-500/40 text-emerald-400 hover:bg-emerald-500/10' : 'border-amber-500/40 text-amber-400 hover:bg-amber-500/10'"
        >
          <span class="material-symbols-outlined text-[16px]">{{ enrichmentStatus?.is_paused ? 'play_arrow' : 'pause' }}</span>
          {{ enrichmentStatus?.is_paused ? 'Resume Worker' : 'Pause Worker' }}
        </button>

        <!-- Search -->
        <div class="relative group">
          <span class="absolute left-3 top-2 text-gray-400 material-symbols-outlined text-[18px]">search</span>
          <input 
            v-model="searchQuery"
            type="text" 
            placeholder="Search logs..." 
            class="pl-9 pr-4 py-1.5 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary w-56 transition-all"
          >
        </div>

        <!-- Filter Category -->
        <select 
          v-model="filterCategory" 
          class="px-3 py-1.5 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary cursor-pointer"
        >
          <option value="all">All Events</option>
          <option value="enrichment">Enrichment</option>
          <option value="downloads">Downloads</option>
          <option value="error">Errors</option>
          <option value="success">Success</option>
        </select>

        <button 
          @click="clearLogs" 
          class="p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
          title="Clear displayed logs"
        >
          <span class="material-symbols-outlined text-[18px]">delete</span>
        </button>
      </div>
    </div>

    <!-- Logs Console -->
    <div class="flex-1 overflow-y-auto custom-scrollbar p-6 bg-[#0d121c] font-mono text-sm">
      <div v-if="filteredLogs.length === 0" class="text-gray-500 text-center py-12">
        No logs match the current filter or search criteria.
      </div>
      <div v-else class="flex flex-col gap-1">
        <div 
          v-for="(log, idx) in filteredLogs" 
          :key="idx"
          class="flex items-start gap-4 hover:bg-white/5 p-1 rounded transition-colors group"
        >
          <span class="text-gray-500 shrink-0 w-20">{{ log.time }}</span>
          <span 
            class="font-bold shrink-0 w-20 text-xs px-1.5 py-0.5 rounded text-center"
            :class="getLevelBadgeClass(log.level)"
          >
            {{ log.level.toUpperCase() }}
          </span>
          <span class="text-purple shrink-0 w-28 text-xs font-semibold">{{ log.category }}</span>
          <span class="text-gray-300 group-hover:text-white flex-1 break-all">{{ log.message }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getEnrichmentStatus, pauseEnrichmentWorker, resumeEnrichmentWorker, type EnrichmentStatus } from '../api/library'

interface LogEntry {
  time: string
  level: 'info' | 'warn' | 'error' | 'success' | 'debug'
  category: string
  message: string
  rawCategory: 'enrichment' | 'downloads' | 'system' | 'library'
}

const searchQuery = ref('')
const filterCategory = ref('all')
const enrichmentStatus = ref<EnrichmentStatus | null>(null)
let unlistenEnrichment: UnlistenFn | null = null

const logs = ref<LogEntry[]>([
  { time: '10:42:05', level: 'info', category: 'Core', message: 'Application started successfully. v2.1.0', rawCategory: 'system' },
  { time: '10:42:08', level: 'warn', category: 'Spotify', message: 'Rate limit approaching. Backing off for 2s.', rawCategory: 'enrichment' },
  { time: '10:42:15', level: 'success', category: 'Library', message: 'Scanned 1,240 tracks in 3.5s.', rawCategory: 'library' },
  { time: '10:45:00', level: 'error', category: 'Download', message: 'Failed to download "Track 4". Staging cleanup executed.', rawCategory: 'downloads' },
  { time: '10:46:22', level: 'info', category: 'System', message: 'Background workers active and responsive.', rawCategory: 'system' },
])

const filteredLogs = computed(() => {
  return logs.value.filter(log => {
    // Category filter
    if (filterCategory.value === 'enrichment' && log.rawCategory !== 'enrichment') return false
    if (filterCategory.value === 'downloads' && log.rawCategory !== 'downloads') return false
    if (filterCategory.value === 'error' && log.level !== 'error') return false
    if (filterCategory.value === 'success' && log.level !== 'success') return false

    // Search query
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase()
      const match = log.message.toLowerCase().includes(q) ||
                    log.category.toLowerCase().includes(q) ||
                    log.level.toLowerCase().includes(q)
      if (!match) return false
    }

    return true
  })
})

function getLevelBadgeClass(level: string): string {
  switch (level) {
    case 'error': return 'bg-red-500/20 text-red-400'
    case 'warn': return 'bg-amber-500/20 text-amber-400'
    case 'success': return 'bg-emerald-500/20 text-emerald-400'
    case 'info': return 'bg-blue-500/20 text-blue-400'
    default: return 'bg-gray-500/20 text-gray-400'
  }
}

function clearLogs() {
  logs.value = []
}

async function fetchStatus() {
  try {
    enrichmentStatus.value = await getEnrichmentStatus()
  } catch (e) {
    console.error('Failed to fetch enrichment status:', e)
  }
}

async function toggleWorkerPause() {
  if (!enrichmentStatus.value) return
  try {
    if (enrichmentStatus.value.is_paused) {
      await resumeEnrichmentWorker()
    } else {
      await pauseEnrichmentWorker()
    }
    await fetchStatus()
  } catch (e) {
    console.error('Failed to toggle enrichment worker:', e)
  }
}

onMounted(async () => {
  await fetchStatus()

  // Listen for real-time background enrichment events
  try {
    unlistenEnrichment = await listen<any>('syncify:enrichment_event', (event) => {
      const payload = event.payload
      const timeStr = new Date().toTimeString().split(' ')[0]
      const level: 'info' | 'warn' | 'error' | 'success' = 
        payload.status === 'completed' ? 'success' :
        payload.status === 'failed' ? 'error' :
        payload.status === 'rate_limited' ? 'warn' : 'info'

      logs.value.unshift({
        time: timeStr,
        level,
        category: `Enrichment (${payload.service})`,
        message: payload.message || `Track ${payload.track_id}: ${payload.status}`,
        rawCategory: 'enrichment',
      })

      if (logs.value.length > 500) {
        logs.value.pop()
      }

      fetchStatus()
    })
  } catch (e) {
    console.warn('Tauri event listener not available:', e)
  }
})

onUnmounted(() => {
  if (unlistenEnrichment) {
    unlistenEnrichment()
  }
})
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 10px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: #0d121c;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: #1e293b;
  border: 2px solid #0d121c;
}
</style>
