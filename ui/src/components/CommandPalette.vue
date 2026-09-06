<template>
  <Teleport to="body">
    <Transition name="fade">
      <div 
        v-if="isOpen" 
        class="command-palette fixed inset-0 bg-black/60 flex items-start justify-center pt-[15vh] z-[200]"
        @click.self="close"
        @keydown.escape="close"
      >
        <div class="w-full max-w-2xl bg-white dark:bg-surface-dark rounded-2xl shadow-2xl overflow-hidden">
          <!-- Input Field -->
          <div class="palette-input relative border-b border-gray-200 dark:border-border-dark">
            <span class="material-symbols-outlined absolute left-5 top-1/2 -translate-y-1/2 text-gray-400 text-2xl">search</span>
            <input 
              ref="searchInput"
              v-model="query"
              type="text"
              :placeholder="placeholder"
              class="w-full h-16 pl-14 pr-12 bg-transparent text-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none"
              @keydown="handleKeydown"
            >
            <button 
              v-if="query" 
              @click="clearQuery"
              class="absolute right-4 top-1/2 -translate-y-1/2 p-1 text-gray-400 hover:text-gray-600 rounded"
            >
              <span class="material-symbols-outlined text-xl">close</span>
            </button>
          </div>
          
          <!-- Results Panel -->
          <div class="palette-results max-h-[500px] overflow-y-auto custom-scrollbar">
            <!-- Hint Bar -->
            <div v-if="!query" class="px-5 py-3 border-b border-gray-100 dark:border-border-dark flex items-center gap-4 text-xs text-gray-400">
              <span><kbd class="kbd-hint">/</kbd> Actions</span>
              <span><kbd class="kbd-hint">></kbd> Settings</span>
              <span><kbd class="kbd-hint">@</kbd> Artists</span>
              <span><kbd class="kbd-hint">#</kbd> Tags</span>
            </div>
            
            <!-- Recent Searches -->
            <div v-if="!query && recentSearches.length > 0" class="result-category">
              <div class="px-5 py-2 flex items-center justify-between">
                <span class="text-xs font-semibold text-gray-400 uppercase tracking-wide">Recent</span>
                <button @click="clearRecent" class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300">Clear</button>
              </div>
              <div 
                v-for="(item, index) in recentSearches" 
                :key="'recent-' + index"
                @click="selectRecent(item)"
                :class="[
                  'result-item recent-search flex items-center gap-3 px-5 py-2.5 cursor-pointer transition-colors',
                  selectedIndex === index ? 'bg-primary/10' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight'
                ]"
              >
                <span class="material-symbols-outlined text-gray-400 text-lg">history</span>
                <span class="text-sm text-gray-700 dark:text-gray-300">{{ item }}</span>
              </div>
            </div>
            
            <!-- Loading State -->
            <div v-if="isSearching && query" class="py-6 text-center">
              <span class="material-symbols-outlined text-2xl text-primary animate-spin">progress_activity</span>
              <p class="text-gray-500 text-sm mt-2">Searching...</p>
            </div>
            
            <!-- Tracks Results -->
            <div v-if="!isSearching && filteredTracks.length > 0" class="result-category">
              <div class="px-5 py-2">
                <span class="text-xs font-semibold text-gray-400 uppercase tracking-wide">Tracks</span>
              </div>
              <div 
                v-for="(track, index) in filteredTracks.slice(0, 5)" 
                :key="'track-' + track.id"
                @click="selectTrack(track)"
                :class="[
                  'result-item result-track flex items-center gap-3 px-5 py-2.5 cursor-pointer transition-colors',
                  getGlobalIndex('tracks', index) === selectedIndex ? 'bg-primary/10' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight'
                ]"
              >
                <div class="w-10 h-10 rounded bg-gray-200 dark:bg-surface-highlight shrink-0 overflow-hidden">
                  <div class="w-full h-full bg-gradient-to-br from-primary/20 to-primary/40 flex items-center justify-center">
                    <span class="material-symbols-outlined text-primary/60 text-lg">music_note</span>
                  </div>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white truncate" v-html="highlightMatch(track.title)"></p>
                  <p class="text-xs text-gray-500 truncate">{{ track.artist }} · {{ track.album }}</p>
                </div>
                <div class="flex items-center gap-1.5">
                  <span v-if="track.service" class="px-1.5 py-0.5 bg-gray-100 dark:bg-surface-highlight text-[10px] text-gray-500 rounded">{{ track.service }}</span>
                  <span v-if="track.quality" class="px-1.5 py-0.5 bg-purple-500/10 text-purple-500 text-[10px] font-medium rounded">{{ track.quality }}</span>
                </div>
              </div>
              <button v-if="filteredTracks.length > 5" class="w-full px-5 py-2 text-xs text-primary hover:underline text-left">
                Show {{ filteredTracks.length - 5 }} more tracks...
              </button>
            </div>
            
            <!-- Actions Results -->
            <div v-if="filteredActions.length > 0" class="result-category">
              <div class="px-5 py-2">
                <span class="text-xs font-semibold text-gray-400 uppercase tracking-wide">Actions</span>
              </div>
              <div 
                v-for="(action, index) in filteredActions" 
                :key="'action-' + action.id"
                @click="executeAction(action)"
                :class="[
                  'result-item result-action flex items-center gap-3 px-5 py-2.5 cursor-pointer transition-colors',
                  getGlobalIndex('actions', index) === selectedIndex ? 'bg-primary/10' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight'
                ]"
              >
                <div class="w-8 h-8 rounded-lg bg-blue-500/10 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-blue-500 text-lg">{{ action.icon }}</span>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white" v-html="highlightMatch(action.name)"></p>
                  <p class="text-xs text-gray-500">{{ action.description }}</p>
                </div>
                <span class="material-symbols-outlined text-gray-300 text-lg">arrow_forward</span>
              </div>
            </div>
            
            <!-- Settings Results -->
            <div v-if="filteredSettings.length > 0" class="result-category">
              <div class="px-5 py-2">
                <span class="text-xs font-semibold text-gray-400 uppercase tracking-wide">Settings</span>
              </div>
              <div 
                v-for="(setting, index) in filteredSettings" 
                :key="'setting-' + setting.id"
                @click="openSetting(setting)"
                :class="[
                  'result-item result-setting flex items-center gap-3 px-5 py-2.5 cursor-pointer transition-colors',
                  getGlobalIndex('settings', index) === selectedIndex ? 'bg-primary/10' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight'
                ]"
              >
                <div class="w-8 h-8 rounded-lg bg-gray-500/10 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-gray-500 text-lg">settings</span>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white" v-html="highlightMatch(setting.name)"></p>
                  <p class="text-xs text-gray-500">{{ setting.path }}</p>
                </div>
              </div>
            </div>
            
            <!-- Navigation Results -->
            <div v-if="filteredNav.length > 0" class="result-category">
              <div class="px-5 py-2">
                <span class="text-xs font-semibold text-gray-400 uppercase tracking-wide">Navigation</span>
              </div>
              <div 
                v-for="(nav, index) in filteredNav" 
                :key="'nav-' + nav.id"
                @click="navigate(nav)"
                :class="[
                  'result-item result-nav flex items-center gap-3 px-5 py-2.5 cursor-pointer transition-colors',
                  getGlobalIndex('nav', index) === selectedIndex ? 'bg-primary/10' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight'
                ]"
              >
                <div class="w-8 h-8 rounded-lg bg-green-500/10 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-green-500 text-lg">{{ nav.icon }}</span>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white" v-html="highlightMatch(nav.name)"></p>
                </div>
                <kbd class="kbd-hint">{{ nav.shortcut }}</kbd>
              </div>
            </div>
            
            <!-- No Results -->
            <div v-if="query && !hasResults && !isSearching" class="no-results py-12 text-center">
              <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-3 block">search_off</span>
              <p class="text-gray-500 mb-2">No results found</p>
              <p class="text-xs text-gray-400">Try different keywords, or use <kbd class="kbd-hint">/</kbd> for actions, <kbd class="kbd-hint">></kbd> for settings</p>
            </div>
            
            <!-- Empty State (when no query and no recent) -->
            <div v-if="!query && recentSearches.length === 0" class="py-12 text-center">
              <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-3 block">search</span>
              <p class="text-gray-500 mb-2">Search for tracks, actions, or settings</p>
              <p class="text-xs text-gray-400">Start typing or use prefixes for specific categories</p>
            </div>
          </div>
          
          <!-- Footer -->
          <div class="px-5 py-3 border-t border-gray-100 dark:border-border-dark flex items-center justify-between text-xs text-gray-400">
            <div class="flex items-center gap-4">
              <span><kbd class="kbd-hint">↑↓</kbd> navigate</span>
              <span><kbd class="kbd-hint">↵</kbd> select</span>
              <span><kbd class="kbd-hint">esc</kbd> close</span>
            </div>
            <span class="flex items-center gap-1">
              <span class="material-symbols-outlined text-[14px]">keyboard</span>
              <kbd class="kbd-hint">Ctrl</kbd>+<kbd class="kbd-hint">K</kbd>
            </span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { searchTracks } from '@/api/library'
import type { LibraryTrack } from '@/api/types'
import { escapeHtml, escapeRegex } from '@/utils/sanitize'

const router = useRouter()
const emit = defineEmits(['action', 'close'])

// State
const isOpen = ref(false)
const query = ref('')
const selectedIndex = ref(0)
const searchInput = ref<HTMLInputElement | null>(null)
const recentSearches = ref<string[]>([])
const tracks = ref<LibraryTrack[]>([])
const isSearching = ref(false)
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null

// Placeholder with context hint
const placeholder = computed(() => {
  if (query.value.startsWith('/')) return 'Search actions...'
  if (query.value.startsWith('>')) return 'Search settings...'
  if (query.value.startsWith('@')) return 'Search artists...'
  if (query.value.startsWith('#')) return 'Search tags/genres...'
  return 'Search tracks, settings, or type a command...'
})

// Search tracks from database with debounce
async function performSearch(searchQuery: string) {
  if (!searchQuery.trim() || searchQuery.startsWith('/') || searchQuery.startsWith('>')) {
    tracks.value = []
    return
  }

  // Remove prefix if searching by artist
  let cleanQuery = searchQuery
  if (cleanQuery.startsWith('@')) {
    cleanQuery = cleanQuery.slice(1)
  }

  if (!cleanQuery.trim()) {
    tracks.value = []
    return
  }

  isSearching.value = true
  try {
    // FTS5 query - append * to each word for prefix matching
    // Note: FTS5 prefix only works on unquoted tokens
    const ftsQuery = cleanQuery
      .trim()
      .split(/\s+/)
      .map(w => w.replace(/[^\w]/g, ''))
      .filter(w => w.length > 0)
      .map(w => w + '*')
      .join(' ')
    
    if (!ftsQuery) {
      tracks.value = []
      isSearching.value = false
      return
    }
    
    const results = await searchTracks(ftsQuery)
    tracks.value = results.tracks
  } catch (error) {
    console.error('Search failed:', error)
    tracks.value = []
  } finally {
    isSearching.value = false
  }
}

const actions = ref([
  { id: 1, name: 'Download all favorites', description: 'Download your favorited tracks', icon: 'download' },
  { id: 2, name: 'Sync Spotify', description: 'Refresh your Spotify library', icon: 'sync' },
  { id: 3, name: 'Clear download queue', description: 'Remove all pending downloads', icon: 'delete_sweep' },
  { id: 4, name: 'Retry failed downloads', description: 'Retry all failed downloads', icon: 'refresh' },
  { id: 5, name: 'Fetch lyrics for all tracks', description: 'Search and download lyrics', icon: 'lyrics' },
  { id: 6, name: 'Export library', description: 'Export your library to file', icon: 'upload_file' },
])

const settings = ref([
  { id: 1, name: 'Download location', path: 'Settings → Storage' },
  { id: 2, name: 'Audio quality preferences', path: 'Settings → Audio Quality' },
  { id: 3, name: 'Lyrics providers', path: 'Settings → Lyrics' },
  { id: 4, name: 'Folder structure', path: 'Settings → Organization' },
  { id: 5, name: 'Connected accounts', path: 'Settings → Accounts' },
  { id: 6, name: 'Metadata templates', path: 'Settings → Metadata' },
])

const navigation = ref([
  { id: 1, name: 'Go to Library', route: '/library', icon: 'library_music', shortcut: 'Ctrl+1' },
  { id: 2, name: 'Go to Downloads', route: '/downloads', icon: 'download', shortcut: 'Ctrl+2' },
  { id: 3, name: 'Go to Metadata', route: '/metadata', icon: 'edit_note', shortcut: 'Ctrl+3' },
  { id: 4, name: 'Go to Lyrics', route: '/lyrics', icon: 'lyrics', shortcut: 'Ctrl+4' },
  { id: 5, name: 'Go to Accounts', route: '/accounts', icon: 'account_circle', shortcut: 'Ctrl+5' },
  { id: 6, name: 'Go to Migration', route: '/migration', icon: 'swap_horiz', shortcut: 'Ctrl+6' },
  { id: 7, name: 'Go to Queue', route: '/queue', icon: 'queue_music', shortcut: 'Ctrl+7' },
  { id: 8, name: 'Go to Settings', route: '/settings', icon: 'settings', shortcut: 'Ctrl+8' },
])

// Fuzzy search helper
function fuzzyMatch(text: string, query: string): boolean {
  const lowerText = text.toLowerCase()
  const lowerQuery = query.toLowerCase()
  
  // Exact match
  if (lowerText.includes(lowerQuery)) return true
  
  // Fuzzy: all query chars appear in order
  let queryIdx = 0
  for (let i = 0; i < lowerText.length && queryIdx < lowerQuery.length; i++) {
    if (lowerText[i] === lowerQuery[queryIdx]) queryIdx++
  }
  return queryIdx === lowerQuery.length
}

// Filtered results - now uses tracks from database search
const filteredTracks = computed(() => {
  if (!query.value || query.value.startsWith('/') || query.value.startsWith('>')) return []
  // Tracks are already filtered by the database search
  return tracks.value.map(t => ({
    id: t.id,
    title: t.title,
    artist: t.artist_name || 'Unknown Artist',
    album: t.album_name || 'Unknown Album',
    service: t.services || '',
    quality: t.quality || ''
  }))
})

const filteredActions = computed(() => {
  if (!query.value) return []
  let searchQuery = query.value
  if (searchQuery.startsWith('/')) searchQuery = searchQuery.slice(1)
  else if (searchQuery.startsWith('>') || searchQuery.startsWith('@') || searchQuery.startsWith('#')) return []
  return actions.value.filter(a => fuzzyMatch(a.name, searchQuery) || fuzzyMatch(a.description, searchQuery))
})

const filteredSettings = computed(() => {
  if (!query.value) return []
  let searchQuery = query.value
  if (searchQuery.startsWith('>')) searchQuery = searchQuery.slice(1)
  else if (searchQuery.startsWith('/') || searchQuery.startsWith('@') || searchQuery.startsWith('#')) return []
  return settings.value.filter(s => fuzzyMatch(s.name, searchQuery) || fuzzyMatch(s.path, searchQuery))
})

const filteredNav = computed(() => {
  if (!query.value) return []
  if (query.value.startsWith('/') || query.value.startsWith('>') || query.value.startsWith('@')) return []
  return navigation.value.filter(n => fuzzyMatch(n.name, query.value))
})

const hasResults = computed(() => {
  return filteredTracks.value.length > 0 || 
         filteredActions.value.length > 0 || 
         filteredSettings.value.length > 0 ||
         filteredNav.value.length > 0
})

const totalResults = computed(() => {
  if (!query.value) return recentSearches.value.length
  return Math.min(filteredTracks.value.length, 5) + 
         filteredActions.value.length + 
         filteredSettings.value.length +
         filteredNav.value.length
})

// Get global index for keyboard navigation
function getGlobalIndex(category: string, localIndex: number): number {
  let offset = 0
  if (category === 'tracks') return offset + localIndex
  offset += Math.min(filteredTracks.value.length, 5)
  if (category === 'actions') return offset + localIndex
  offset += filteredActions.value.length
  if (category === 'settings') return offset + localIndex
  offset += filteredSettings.value.length
  if (category === 'nav') return offset + localIndex
  return localIndex
}

// Highlight matching text
function highlightMatch(text: string): string {
  const escapedText = escapeHtml(text || '')
  if (!query.value.trim()) return escapedText
  const cleanQuery = query.value.replace(/^[/>@#]/, '').trim()
  if (!cleanQuery) return escapedText
  const safeQuery = escapeRegex(escapeHtml(cleanQuery))
  const regex = new RegExp(`(${safeQuery})`, 'gi')
  return escapedText.replace(regex, '<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">$1</mark>')
}

// Keyboard handler
function handleKeydown(event: KeyboardEvent) {
  switch (event.key) {
    case 'ArrowDown':
    case 'Tab':
      event.preventDefault()
      if (selectedIndex.value < totalResults.value - 1) {
        selectedIndex.value++
      } else {
        selectedIndex.value = 0
      }
      break
    case 'ArrowUp':
      event.preventDefault()
      if (selectedIndex.value > 0) {
        selectedIndex.value--
      } else {
        selectedIndex.value = totalResults.value - 1
      }
      break
    case 'Enter':
      event.preventDefault()
      executeSelected()
      break
  }
}

// Execute selected result
function executeSelected() {
  if (!query.value && recentSearches.value.length > 0) {
    selectRecent(recentSearches.value[selectedIndex.value])
    return
  }
  
  let idx = selectedIndex.value
  
  // Tracks
  const trackCount = Math.min(filteredTracks.value.length, 5)
  if (idx < trackCount) {
    selectTrack(filteredTracks.value[idx])
    return
  }
  idx -= trackCount
  
  // Actions
  if (idx < filteredActions.value.length) {
    executeAction(filteredActions.value[idx])
    return
  }
  idx -= filteredActions.value.length
  
  // Settings
  if (idx < filteredSettings.value.length) {
    openSetting(filteredSettings.value[idx])
    return
  }
  idx -= filteredSettings.value.length
  
  // Navigation
  if (idx < filteredNav.value.length) {
    navigate(filteredNav.value[idx])
    return
  }
}

// Result handlers
function selectTrack(track: any) {
  addToRecent(track.title)
  close()
  router.push({ path: '/library', query: { track: track.id } })
}

function executeAction(action: any) {
  addToRecent(action.name)
  close()
  emit('action', action)
}

function openSetting(setting: any) {
  addToRecent(setting.name)
  close()
  router.push('/settings')
}

function navigate(nav: any) {
  close()
  router.push(nav.route)
}

function selectRecent(item: string) {
  query.value = item
}

// Recent searches
function addToRecent(item: string) {
  const idx = recentSearches.value.indexOf(item)
  if (idx !== -1) recentSearches.value.splice(idx, 1)
  recentSearches.value.unshift(item)
  if (recentSearches.value.length > 5) recentSearches.value.pop()
  localStorage.setItem('syncify_recent_searches', JSON.stringify(recentSearches.value))
}

function clearRecent() {
  recentSearches.value = []
  localStorage.removeItem('syncify_recent_searches')
}

// Control methods
function open() {
  isOpen.value = true
  query.value = ''
  selectedIndex.value = 0
  nextTick(() => {
    searchInput.value?.focus()
  })
}

function close() {
  isOpen.value = false
  query.value = ''
}

function clearQuery() {
  query.value = ''
  searchInput.value?.focus()
}

// Reset selection on query change and perform search
watch(query, (newQuery) => {
  selectedIndex.value = 0
  
  // Debounce database search
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
  searchDebounceTimer = setTimeout(() => {
    performSearch(newQuery)
  }, 200) // 200ms debounce
})

// Global keyboard listener for Ctrl+K
function handleGlobalKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 'k') {
    event.preventDefault()
    if (isOpen.value) {
      close()
    } else {
      open()
    }
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleGlobalKeydown)
  const stored = localStorage.getItem('syncify_recent_searches')
  if (stored) {
    try {
      recentSearches.value = JSON.parse(stored)
    } catch {}
  }
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleGlobalKeydown)
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
})

// Expose for external use
defineExpose({ open, close, isOpen })
</script>

<style scoped>
/* Keyboard hint styling */
.kbd-hint {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  background: rgba(0, 0, 0, 0.06);
  border-radius: 3px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 10px;
  font-weight: 500;
  color: #6b7280;
}

.dark .kbd-hint {
  background: rgba(255, 255, 255, 0.1);
  color: #9ca3af;
}

/* Modal transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.fade-enter-active .bg-white,
.fade-enter-active .bg-surface-dark {
  animation: slideUp 0.15s ease;
}

@keyframes slideUp {
  from {
    opacity: 0;
    transform: translateY(-10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
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

/* Result category spacing */
.result-category:not(:last-child) {
  border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  padding-bottom: 8px;
  margin-bottom: 8px;
}

.dark .result-category:not(:last-child) {
  border-bottom-color: rgba(255, 255, 255, 0.05);
}
</style>
