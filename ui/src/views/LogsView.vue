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

        <!-- File Logging Active badge (confirmed by backend) -->
        <div v-if="loggingStatus?.file_logging_active" class="flex items-center gap-2 text-xs">
          <span class="px-2 py-0.5 rounded-full font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center gap-1.5" :title="loggingStatus.active_log_file_path || 'Rotating file logging active'">
            <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
            File Logging Active
          </span>
          <!-- Dev file path copy button if in dev mode -->
          <button 
            v-if="loggingStatus.is_development && loggingStatus.active_log_file_path"
            @click="copyLogPath" 
            class="px-2 py-0.5 rounded bg-gray-100 dark:bg-surface-dark border border-gray-200 dark:border-border-dark text-[11px] text-gray-400 hover:text-white flex items-center gap-1 font-mono transition-colors"
            title="Click to copy active log file path to clipboard"
          >
            <span class="material-symbols-outlined text-[13px]">description</span>
            {{ activeLogFileName }}
          </button>
        </div>

        <span class="text-xs text-text-secondary font-mono">
          ({{ filteredLogs.length }} / {{ logs.length }} logs)
        </span>
      </div>
      
      <!-- Toolbar Controls -->
      <div class="flex items-center gap-3 flex-wrap">
        <!-- Worker Controls -->
        <button 
          v-if="enrichmentStatus"
          @click="toggleWorkerPause" 
          class="px-3 py-1.5 rounded-lg text-xs font-medium border flex items-center gap-1.5 transition-colors"
          :class="enrichmentStatus.is_paused ? 'border-emerald-500/40 text-emerald-400 hover:bg-emerald-500/10' : 'border-amber-500/40 text-amber-400 hover:bg-amber-500/10'"
        >
          <span class="material-symbols-outlined text-[16px]">{{ enrichmentStatus.is_paused ? 'play_arrow' : 'pause' }}</span>
          {{ enrichmentStatus.is_paused ? 'Resume Worker' : 'Pause Worker' }}
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
          <option value="trace">TRACE</option>
        </select>

        <!-- Filter Provider / Module -->
        <select 
          v-model="filterProvider" 
          class="px-3 py-1.5 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary cursor-pointer font-medium"
        >
          <option value="all">All Modules</option>
          <option value="spotify">Spotify</option>
          <option value="qobuz">Qobuz</option>
          <option value="tidal">Tidal</option>
          <option value="deezer">Deezer</option>
          <option value="apple">Apple Music</option>
          <option value="soundcloud">SoundCloud</option>
          <option value="musicbrainz">MusicBrainz</option>
          <option value="lastfm">Last.fm</option>
          <option value="downloads">Downloads</option>
          <option value="enrichment">Enrichment</option>
          <option value="worker">Worker</option>
          <option value="database">Database</option>
          <option value="filesystem">Filesystem</option>
          <option value="security">Security</option>
          <option value="library">Library</option>
          <option value="system">System</option>
        </select>

        <!-- Auto-scroll Toggle -->
        <button 
          @click="toggleAutoScroll" 
          :class="['p-2 rounded-lg border transition-colors flex items-center justify-center', autoScroll ? 'bg-primary/10 border-primary text-primary' : 'bg-gray-100 dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-400']"
          :title="autoScroll ? 'Auto-scroll enabled' : 'Auto-scroll paused'"
        >
          <span class="material-symbols-outlined text-[18px]">vertical_align_bottom</span>
        </button>

        <!-- Copy Logs -->
        <button 
          @click="handleCopy" 
          class="p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
          title="Copy displayed logs"
        >
          <span class="material-symbols-outlined text-[18px]">content_copy</span>
        </button>

        <!-- Export Log File -->
        <button 
          @click="handleExport" 
          class="p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
          title="Export logs to file"
        >
          <span class="material-symbols-outlined text-[18px]">download</span>
        </button>

        <!-- Clear Logs -->
        <button 
          @click="handleClear" 
          class="p-2 text-gray-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
          title="Clear system and console logs"
        >
          <span class="material-symbols-outlined text-[18px]">delete</span>
        </button>
      </div>
    </div>

    <!-- Logs Console Output -->
    <div 
      ref="logContainerRef" 
      @scroll="handleScroll"
      class="flex-1 overflow-y-auto custom-scrollbar p-6 bg-[#0d121c] font-mono text-sm"
    >
      <!-- Initial Loading State -->
      <div v-if="isLoadingLogs" class="flex flex-col items-center justify-center py-20 gap-3 select-none">
        <div class="relative w-12 h-12">
          <div class="absolute inset-0 rounded-full border-4 border-primary/20"></div>
          <div class="absolute inset-0 rounded-full border-4 border-primary border-t-transparent animate-spin"></div>
        </div>
        <p class="text-sm text-gray-400 font-sans">Loading system logs...</p>
      </div>

      <!-- Honest Empty State -->
      <div v-else-if="filteredLogs.length === 0" class="text-gray-500 text-center py-16 flex flex-col items-center justify-center gap-3 select-none">
        <span class="material-symbols-outlined text-5xl text-gray-600">terminal</span>
        <p v-if="logs.length === 0" class="text-base text-gray-400 font-sans">No system logs recorded</p>
        <p v-else class="text-sm text-gray-400 font-sans">No audit logs match the current filter or search criteria.</p>
        <span v-if="logs.length === 0" class="text-xs text-gray-600 max-w-sm text-center">
          Native events and operations will appear here in real-time.
        </span>
      </div>

      <!-- Live Logs List -->
      <div v-else class="flex flex-col gap-1.5">
        <div 
          v-for="log in filteredLogs" 
          :key="log.id"
          class="flex flex-col hover:bg-white/5 px-2.5 py-1.5 rounded transition-colors group"
        >
          <div class="flex items-start gap-3.5">
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

            <!-- Expandable details toggle if details exist -->
            <button 
              v-if="log.details && Object.keys(log.details).length > 0"
              @click="toggleDetails(log.id)"
              class="text-gray-500 hover:text-gray-300 text-xs px-1 select-none"
              title="Toggle payload details"
            >
              <span class="material-symbols-outlined text-[14px]">
                {{ expandedLogs.has(log.id) ? 'expand_less' : 'expand_more' }}
              </span>
            </button>
          </div>

          <!-- Expanded Payload Details Viewer -->
          <div 
            v-if="expandedLogs.has(log.id) && log.details"
            class="mt-2 ml-20 p-2.5 rounded bg-black/40 border border-white/10 text-xs text-emerald-400 overflow-x-auto"
          >
            <pre>{{ JSON.stringify(log.details, null, 2) }}</pre>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, nextTick, watch } from 'vue'
import { getEnrichmentStatus, pauseEnrichmentWorker, resumeEnrichmentWorker, type EnrichmentStatus } from '../api/library'
import { getLoggingStatus, type LoggingStatus } from '@/api/logs'
import { useLogs, type LogEntry } from '@/composables/useLogs'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const isLoadingLogs = ref(true)
const searchQuery = ref('')
const filterLevel = ref<string>('all')
const filterProvider = ref<string>('all')
const autoScroll = ref(true)
const logContainerRef = ref<HTMLElement | null>(null)
const enrichmentStatus = ref<EnrichmentStatus | null>(null)
const loggingStatus = ref<LoggingStatus | null>(null)
const expandedLogs = ref<Set<string>>(new Set())

const activeLogFileName = computed(() => {
  if (!loggingStatus.value?.active_log_file_path) return 'syncify-dev.log'
  const parts = loggingStatus.value.active_log_file_path.split(/[/\\]/)
  return parts[parts.length - 1] || 'syncify-dev.log'
})

async function copyLogPath() {
  if (!loggingStatus.value?.active_log_file_path) return
  try {
    if (navigator.clipboard) {
      await navigator.clipboard.writeText(loggingStatus.value.active_log_file_path)
      toast.success('Path Copied', 'Log file path copied to clipboard')
    }
  } catch (e) {
    console.error('Failed to copy log path:', e)
  }
}

// Use singleton global logs state
const { 
  logs, 
  clearLogs, 
  copyLogs, 
  exportLogsFile, 
  getLevelBadgeClass, 
  getProviderBadgeClass,
  fetchLogs
} = useLogs()

const filteredLogs = computed(() => {
  return logs.value.filter(log => {
    // Level filter
    if (filterLevel.value !== 'all') {
      const targetLevel = filterLevel.value.toLowerCase()
      const currentLevel = (log.level || '').toLowerCase()
      if (currentLevel !== targetLevel) return false
    }

    // Provider / Module filter
    if (filterProvider.value !== 'all') {
      const p = filterProvider.value.toLowerCase()
      const logProv = (log.provider || '').toLowerCase()
      const logCat = (log.category || '').toLowerCase()
      const logRaw = (log.rawCategory || '').toLowerCase()
      const matches = logProv.includes(p) || logCat.includes(p) || logRaw.includes(p)
      if (!matches) return false
    }

    // Search query
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLowerCase()
      const match = (log.message || '').toLowerCase().includes(q) ||
                    (log.category || '').toLowerCase().includes(q) ||
                    (log.provider || '').toLowerCase().includes(q) ||
                    (log.level || '').toLowerCase().includes(q)
      if (!match) return false
    }

    return true
  })
})

function toggleDetails(id: string) {
  if (expandedLogs.value.has(id)) {
    expandedLogs.value.delete(id)
  } else {
    expandedLogs.value.add(id)
  }
}

function toggleAutoScroll() {
  autoScroll.value = !autoScroll.value
  if (autoScroll.value && logContainerRef.value) {
    logContainerRef.value.scrollTop = 0
  }
}

function handleScroll() {
  if (!logContainerRef.value) return
  // If user scrolls down/away from top (since logs are unshifted to top, scrollTop > 30px means user is reviewing history)
  if (logContainerRef.value.scrollTop > 30) {
    if (autoScroll.value) {
      autoScroll.value = false
    }
  } else if (logContainerRef.value.scrollTop <= 5) {
    if (!autoScroll.value) {
      autoScroll.value = true
    }
  }
}

// Watch logs changes and auto-scroll to top if enabled
watch(() => logs.value.length, () => {
  if (autoScroll.value) {
    nextTick(() => {
      if (logContainerRef.value) {
        logContainerRef.value.scrollTop = 0
      }
    })
  }
})

async function handleClear() {
  await clearLogs()
}

async function handleCopy() {
  await copyLogs(filteredLogs.value)
}

async function handleExport() {
  await exportLogsFile(filteredLogs.value)
}

async function fetchStatus() {
  try {
    enrichmentStatus.value = await getEnrichmentStatus()
  } catch (e) {
    console.error('Failed to fetch enrichment status:', e)
  }
  try {
    loggingStatus.value = await getLoggingStatus()
  } catch (e) {
    console.error('Failed to fetch logging status:', e)
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
  isLoadingLogs.value = true
  try {
    await Promise.all([
      fetchStatus(),
      fetchLogs({ limit: 500 })
    ])
  } catch (e) {
    console.error('Failed to initialize logs view:', e)
  } finally {
    isLoadingLogs.value = false
  }
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
