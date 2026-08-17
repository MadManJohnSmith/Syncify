<template>
  <div class="h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex flex-wrap items-center justify-between gap-4 bg-white dark:bg-[#101723] shrink-0">
      <div class="flex items-center gap-4 flex-wrap">
        <h1 class="text-lg font-bold tracking-tight text-gray-900 dark:text-white">Audit & System Logs</h1>
        
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

        <span class="text-xs text-text-secondary font-mono">
          ({{ filteredLogs.length }} / {{ logs.length }} logs)
        </span>
      </div>
      
      <!-- Toolbar Controls -->
      <div class="flex items-center gap-3 flex-wrap">
        <!-- Worker Controls -->
        <button 
          @click="toggleWorkerPause" 
          class="px-3 py-1.5 rounded-lg text-xs font-medium border flex items-center gap-1.5 transition-colors"
          :class="enrichmentStatus?.is_paused ? 'border-emerald-500/40 text-emerald-400 hover:bg-emerald-500/10' : 'border-amber-500/40 text-amber-400 hover:bg-amber-500/10'"
        >
          <span class="material-symbols-outlined text-[16px]">{{ enrichmentStatus?.is_paused ? 'play_arrow' : 'pause' }}</span>
          {{ enrichmentStatus?.is_paused ? 'Resume Worker' : 'Pause Worker' }}
        </button>

        <!-- Search input -->
        <div class="relative group">
          <span class="absolute left-3 top-2 text-gray-400 material-symbols-outlined text-[18px]">search</span>
          <input 
            v-model="searchQuery"
            type="text" 
            placeholder="Search logs & events..." 
            class="pl-9 pr-4 py-1.5 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary w-48 focus:w-60 transition-all"
          >
        </div>

        <!-- Filter Level -->
        <select 
          v-model="filterLevel" 
          class="px-3 py-1.5 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary cursor-pointer font-medium"
        >
          <option value="all">All Levels</option>
          <option value="info">INFO</option>
          <option value="warn">WARN</option>
          <option value="error">ERROR</option>
          <option value="success">SUCCESS</option>
          <option value="debug">DEBUG</option>
        </select>

        <!-- Filter Provider -->
        <select 
          v-model="filterProvider" 
          class="px-3 py-1.5 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary cursor-pointer font-medium"
        >
          <option value="all">All Providers</option>
          <option value="spotify">Spotify</option>
          <option value="qobuz">Qobuz</option>
          <option value="tidal">Tidal</option>
          <option value="deezer">Deezer</option>
          <option value="apple_music">Apple Music</option>
          <option value="soundcloud">SoundCloud</option>
          <option value="downloads">Downloads</option>
          <option value="enrichment">Enrichment</option>
          <option value="system">System / Core</option>
        </select>

        <!-- Auto-scroll Toggle -->
        <button 
          @click="autoScroll = !autoScroll" 
          :class="['p-2 rounded-lg border transition-colors flex items-center justify-center', autoScroll ? 'bg-primary/10 border-primary text-primary' : 'bg-gray-100 dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-400']"
          :title="autoScroll ? 'Auto-scroll enabled' : 'Auto-scroll paused'"
        >
          <span class="material-symbols-outlined text-[18px]">vertical_align_bottom</span>
        </button>

        <!-- Copy Logs -->
        <button 
          @click="copyLogs" 
          class="p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
          title="Copy displayed logs"
        >
          <span class="material-symbols-outlined text-[18px]">content_copy</span>
        </button>

        <!-- Clear Logs -->
        <button 
          @click="clearLogs" 
          class="p-2 text-gray-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
          title="Clear displayed logs"
        >
          <span class="material-symbols-outlined text-[18px]">delete</span>
        </button>
      </div>
    </div>

    <!-- Logs Console Output -->
    <div ref="logContainerRef" class="flex-1 overflow-y-auto custom-scrollbar p-6 bg-[#0d121c] font-mono text-sm">
      <div v-if="filteredLogs.length === 0" class="text-gray-500 text-center py-12 flex flex-col items-center gap-2">
        <span class="material-symbols-outlined text-4xl text-gray-600">terminal</span>
        <p>No audit logs match the current filter or search criteria.</p>
      </div>
      <div v-else class="flex flex-col gap-1.5">
        <div 
          v-for="log in filteredLogs" 
          :key="log.id"
          class="flex items-start gap-3.5 hover:bg-white/5 px-2.5 py-1.5 rounded transition-colors group"
        >
          <!-- Timestamp -->
          <span class="text-gray-500 shrink-0 w-20 text-xs select-none">{{ log.time }}</span>

          <!-- Level Badge -->
          <span 
            class="font-bold shrink-0 w-18 text-[11px] px-1.5 py-0.5 rounded text-center uppercase tracking-wide select-none"
            :class="getLevelBadgeClass(log.level)"
          >
            {{ log.level }}
          </span>

          <!-- Provider Badge -->
          <span 
            class="shrink-0 text-xs px-2 py-0.5 rounded font-semibold flex items-center gap-1 select-none"
            :class="getProviderBadgeClass(log.provider)"
          >
            {{ log.provider }}
          </span>

          <!-- Category/Scope -->
          <span v-if="log.category && log.category !== log.provider" class="text-purple-400 shrink-0 text-xs font-medium">
            [{{ log.category }}]
          </span>

          <!-- Message -->
          <span class="text-gray-300 group-hover:text-white flex-1 break-all leading-relaxed">
            {{ log.message }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getEnrichmentStatus, pauseEnrichmentWorker, resumeEnrichmentWorker, type EnrichmentStatus } from '../api/library'
import { useToast } from '@/composables/useToast'

export interface LogEntry {
  id: string
  time: string
  level: 'info' | 'warn' | 'error' | 'success' | 'debug'
  provider: string
  category: string
  message: string
  rawCategory: 'enrichment' | 'downloads' | 'system' | 'library'
  details?: any
}

const toast = useToast()
const searchQuery = ref('')
const filterLevel = ref<string>('all')
const filterProvider = ref<string>('all')
const autoScroll = ref(true)
const logContainerRef = ref<HTMLElement | null>(null)
const enrichmentStatus = ref<EnrichmentStatus | null>(null)

let unlistenEnrichment: UnlistenFn | null = null
let unlistenDownloads: UnlistenFn | null = null
let unlistenProgress: UnlistenFn | null = null
let unlistenNotification: UnlistenFn | null = null

let logIdCounter = 100

const logs = ref<LogEntry[]>([
  { id: '1', time: '10:42:05', level: 'info', provider: 'System', category: 'Core', message: 'Application started successfully. v2.1.0', rawCategory: 'system' },
  { id: '2', time: '10:42:08', level: 'warn', provider: 'Spotify', category: 'Enrichment', message: 'Rate limit approaching (350 req/min). Backing off for 2s.', rawCategory: 'enrichment' },
  { id: '3', time: '10:42:15', level: 'success', provider: 'Library', category: 'Scanner', message: 'Scanned 1,240 tracks in 3.5s. Identity lock verified.', rawCategory: 'library' },
  { id: '4', time: '10:45:00', level: 'error', provider: 'Qobuz', category: 'Downloads', message: 'Failed to download "Track 4". SourceNotFound: 404 Stale Stream URL.', rawCategory: 'downloads' },
  { id: '5', time: '10:46:22', level: 'info', provider: 'System', category: 'Worker', message: 'Background worker active with 3 concurrent threads.', rawCategory: 'system' },
])

const filteredLogs = computed(() => {
  return logs.value.filter(log => {
    // Level filter
    if (filterLevel.value !== 'all' && log.level.toLowerCase() !== filterLevel.value.toLowerCase()) {
      return false
    }

    // Provider filter
    if (filterProvider.value !== 'all') {
      const p = filterProvider.value.toLowerCase()
      const logProv = log.provider.toLowerCase()
      const logCat = log.category.toLowerCase()
      const logRaw = log.rawCategory.toLowerCase()
      const matchesProv = logProv.includes(p) || logCat.includes(p) || logRaw.includes(p)
      if (!matchesProv) return false
    }

    // Search query
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase()
      const match = log.message.toLowerCase().includes(q) ||
                    log.category.toLowerCase().includes(q) ||
                    log.provider.toLowerCase().includes(q) ||
                    log.level.toLowerCase().includes(q)
      if (!match) return false
    }

    return true
  })
})

function getLevelBadgeClass(level: string): string {
  switch (level.toLowerCase()) {
    case 'error': return 'bg-red-500/20 text-red-400 border border-red-500/30'
    case 'warn': return 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
    case 'success': return 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
    case 'info': return 'bg-blue-500/20 text-blue-400 border border-blue-500/30'
    case 'debug': return 'bg-purple-500/20 text-purple-400 border border-purple-500/30'
    default: return 'bg-gray-500/20 text-gray-400'
  }
}

function getProviderBadgeClass(provider: string): string {
  const p = (provider || '').toLowerCase()
  if (p.includes('spotify')) return 'bg-[#1ed760]/10 text-[#1ed760] border border-[#1ed760]/20'
  if (p.includes('qobuz')) return 'bg-[#1a8fe3]/10 text-[#1a8fe3] border border-[#1a8fe3]/20'
  if (p.includes('tidal')) return 'bg-[#00d4aa]/10 text-[#00d4aa] border border-[#00d4aa]/20'
  if (p.includes('deezer')) return 'bg-[#ff0092]/10 text-[#ff0092] border border-[#ff0092]/20'
  if (p.includes('apple')) return 'bg-[#fa2d48]/10 text-[#fa2d48] border border-[#fa2d48]/20'
  if (p.includes('soundcloud')) return 'bg-[#ff5500]/10 text-[#ff5500] border border-[#ff5500]/20'
  if (p.includes('download')) return 'bg-primary/10 text-primary border border-primary/20'
  if (p.includes('enrichment')) return 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
  return 'bg-gray-500/10 text-gray-400 border border-gray-500/20'
}

function normalizeProviderName(raw: string | undefined): string {
  if (!raw) return 'System'
  const lower = raw.toLowerCase()
  if (lower.includes('spotify')) return 'Spotify'
  if (lower.includes('qobuz')) return 'Qobuz'
  if (lower.includes('tidal')) return 'Tidal'
  if (lower.includes('deezer')) return 'Deezer'
  if (lower.includes('apple')) return 'Apple Music'
  if (lower.includes('soundcloud')) return 'SoundCloud'
  if (lower.includes('enrichment')) return 'Enrichment'
  if (lower.includes('download')) return 'Downloads'
  return raw.charAt(0).toUpperCase() + raw.slice(1)
}

function addLog(entry: Omit<LogEntry, 'id' | 'time'> & { time?: string }) {
  const timeStr = entry.time || new Date().toTimeString().split(' ')[0]
  const idStr = String(++logIdCounter)

  logs.value.unshift({
    id: idStr,
    time: timeStr,
    level: entry.level,
    provider: normalizeProviderName(entry.provider),
    category: entry.category,
    message: entry.message,
    rawCategory: entry.rawCategory,
    details: entry.details,
  })

  if (logs.value.length > 1000) {
    logs.value.pop()
  }

  if (autoScroll.value) {
    nextTick(() => {
      if (logContainerRef.value) {
        logContainerRef.value.scrollTop = 0
      }
    })
  }
}

function clearLogs() {
  logs.value = []
  toast.success('Logs Cleared', 'Console display cleared')
}

async function copyLogs() {
  if (filteredLogs.value.length === 0) return
  const text = filteredLogs.value.map(l => `[${l.time}] [${l.level.toUpperCase()}] [${l.provider}] [${l.category}] ${l.message}`).join('\n')
  try {
    await navigator.clipboard.writeText(text)
    toast.success('Copied', `${filteredLogs.value.length} log lines copied to clipboard`)
  } catch {
    toast.error('Copy Failed', 'Could not copy logs to clipboard')
  }
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

  // 1. Listen for background enrichment events
  try {
    unlistenEnrichment = await listen<any>('syncify:enrichment_event', (event) => {
      const payload = event.payload
      if (!payload) return
      const level: 'info' | 'warn' | 'error' | 'success' = 
        payload.status === 'completed' ? 'success' :
        payload.status === 'failed' ? 'error' :
        payload.status === 'rate_limited' ? 'warn' : 'info'

      addLog({
        level,
        provider: normalizeProviderName(payload.service || 'Enrichment'),
        category: 'Enrichment',
        message: payload.message || `Track ${payload.track_id}: status ${payload.status}`,
        rawCategory: 'enrichment',
        details: payload,
      })

      fetchStatus()
    })
  } catch (e) {
    console.warn('Enrichment event listener not available:', e)
  }

  // 2. Listen for download progress & completion events
  try {
    unlistenDownloads = await listen<any>('syncify:download_progress', (event) => {
      const payload = event.payload
      if (!payload) return
      const status = (payload.status || '').toLowerCase()
      const title = payload.title || payload.target_title || `Track #${payload.track_id || payload.queue_id}`
      const level: 'info' | 'warn' | 'error' | 'success' = 
        status === 'complete' || status === 'completed' ? 'success' :
        status === 'failed' ? 'error' :
        status === 'paused' ? 'warn' : 'info'

      const msg = payload.message 
        ? `"${title}" - ${payload.message}` 
        : (status === 'complete' || status === 'completed' 
            ? `Downloaded "${title}" (100%) - Sidecars generated.`
            : (status === 'failed' 
                ? `Download failed for "${title}": ${payload.error || payload.error_message || 'Unknown error'}`
                : `Progress for "${title}": ${Math.round(payload.progress_percent || 0)}%`))

      addLog({
        level,
        provider: normalizeProviderName(payload.service_name || payload.service || 'Downloads'),
        category: 'Downloads',
        message: msg,
        rawCategory: 'downloads',
        details: payload,
      })
    })
  } catch (e) {
    console.warn('Download progress listener not available:', e)
  }

  // 3. Listen for general pipeline / tool progress events
  try {
    unlistenProgress = await listen<any>('syncify:progress', (event) => {
      const payload = event.payload
      if (!payload) return
      addLog({
        level: payload.status === 'completed' ? 'success' : (payload.status === 'failed' ? 'error' : 'info'),
        provider: normalizeProviderName(payload.provider || payload.operation || 'System'),
        category: payload.operation || 'Pipeline',
        message: payload.message || `Operation ${payload.operation || 'task'}: ${payload.status || 'in progress'}`,
        rawCategory: 'system',
        details: payload,
      })
    })
  } catch (e) {
    console.warn('Progress listener not available:', e)
  }

  // 4. Listen for system notifications
  try {
    unlistenNotification = await listen<any>('syncify:notification', (event) => {
      const payload = event.payload
      if (!payload) return
      addLog({
        level: payload.level || 'info',
        provider: 'System',
        category: 'Notification',
        message: payload.message || payload.title || 'System Notification',
        rawCategory: 'system',
        details: payload,
      })
    })
  } catch (e) {
    console.warn('Notification listener not available:', e)
  }
})

onUnmounted(() => {
  if (unlistenEnrichment) unlistenEnrichment()
  if (unlistenDownloads) unlistenDownloads()
  if (unlistenProgress) unlistenProgress()
  if (unlistenNotification) unlistenNotification()
})
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 8px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: #0d121c;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: #1e293b;
  border-radius: 4px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: #334155;
}
</style>

