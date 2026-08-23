<template>
  <div class="keyboard-shortcuts-system">
    <!-- Keyboard Shortcuts Help Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div 
          v-if="showHelpModal" 
          class="shortcuts-modal fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8"
          @click.self="showHelpModal = false"
          @keydown.escape="showHelpModal = false"
        >
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[85vh] overflow-hidden shadow-2xl flex flex-col">
            <!-- Header -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
              <h2 class="text-xl font-bold text-gray-900 dark:text-white">Keyboard Shortcuts</h2>
              <button @click="showHelpModal = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <!-- Search -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark shrink-0">
              <div class="relative">
                <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-[20px]">search</span>
                <input 
                  v-model="searchQuery"
                  type="text"
                  placeholder="Search shortcuts..."
                  class="shortcut-search w-full pl-10 pr-4 py-2.5 bg-gray-100 dark:bg-surface-highlight rounded-xl text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50"
                  @keydown.escape.stop="searchQuery = ''"
                >
              </div>
            </div>
            
            <!-- Shortcuts List -->
            <div class="flex-1 overflow-y-auto custom-scrollbar px-6 py-4">
              <div v-for="section in filteredSections" :key="section.name" class="shortcut-section mb-6 last:mb-0">
                <!-- Section Header -->
                <button 
                  @click="toggleSection(section.name)"
                  class="w-full flex items-center justify-between py-2 group"
                >
                  <h3 class="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide">{{ section.name }}</h3>
                  <span class="material-symbols-outlined text-gray-400 transition-transform" :class="{ 'rotate-180': !collapsedSections.includes(section.name) }">
                    expand_more
                  </span>
                </button>
                
                <!-- Section Content -->
                <Transition name="accordion">
                  <div v-if="!collapsedSections.includes(section.name)" class="space-y-1 mt-2">
                    <div 
                      v-for="shortcut in section.shortcuts" 
                      :key="shortcut.action"
                      class="shortcut-row flex items-center justify-between py-2.5 px-3 rounded-lg hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
                    >
                      <span class="text-sm text-gray-700 dark:text-gray-300" v-html="highlightMatch(shortcut.action)"></span>
                      <div class="flex items-center gap-1">
                        <template v-for="(key, index) in shortcut.keys" :key="index">
                          <span v-if="index > 0" class="text-gray-400 text-xs">+</span>
                          <kbd class="keyboard-key">{{ key }}</kbd>
                        </template>
                      </div>
                    </div>
                  </div>
                </Transition>
              </div>
              
              <!-- No Results -->
              <div v-if="filteredSections.length === 0" class="text-center py-12">
                <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-2 block">search_off</span>
                <p class="text-gray-500">No shortcuts found for "{{ searchQuery }}"</p>
              </div>
            </div>
            
            <!-- Footer -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
              <button class="flex items-center gap-2 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 transition-colors">
                <span class="material-symbols-outlined text-[18px]">print</span>
                Print Cheat Sheet
              </button>
              <button class="flex items-center gap-2 text-sm text-primary hover:underline">
                <span class="material-symbols-outlined text-[18px]">settings</span>
                Customize Shortcuts
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- First-time hint (shows once) -->
    <Transition name="fade">
      <div 
        v-if="showFirstTimeHint && !hasSeenHint"
        class="fixed bottom-20 right-6 z-50 bg-primary text-white px-4 py-3 rounded-xl shadow-xl max-w-xs"
      >
        <button @click="dismissHint" class="absolute -top-2 -right-2 w-6 h-6 bg-white text-gray-500 rounded-full shadow flex items-center justify-center hover:bg-gray-100">
          <span class="material-symbols-outlined text-[14px]">close</span>
        </button>
        <p class="text-sm font-medium mb-1">💡 Pro tip</p>
        <p class="text-xs text-white/80">Press <kbd class="keyboard-key keyboard-key-sm">?</kbd> anytime to see all keyboard shortcuts</p>
        <div class="absolute -bottom-2 right-8 w-4 h-4 bg-primary transform rotate-45"></div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'

const router = useRouter()
const emit = defineEmits(['command-palette', 'search', 'refresh', 'settings'])

// State
const showHelpModal = ref(false)
const searchQuery = ref('')
const collapsedSections = ref<string[]>([])
const showFirstTimeHint = ref(true)
const hasSeenHint = ref(false)

// Shortcut definitions
const shortcutSections = ref([
  {
    name: 'Global',
    shortcuts: [
      { action: 'Open command palette', keys: ['Ctrl', 'K'] },
      { action: 'Open Settings', keys: ['Ctrl', ','] },
      { action: 'Toggle help panel', keys: ['Ctrl', 'H'] },
      { action: 'Close modal/dialog', keys: ['Escape'] },
      { action: 'Refresh current view', keys: ['Ctrl', 'R'] },
      { action: 'Undo', keys: ['Ctrl', 'Z'] },
      { action: 'Redo', keys: ['Ctrl', 'Y'] },
    ]
  },
  {
    name: 'Navigation',
    shortcuts: [
      { action: 'Go to Library', keys: ['Ctrl', '1'] },
      { action: 'Go to Downloads', keys: ['Ctrl', '2'] },
      { action: 'Go to Metadata', keys: ['Ctrl', '3'] },
      { action: 'Go to Lyrics', keys: ['Ctrl', '4'] },
      { action: 'Go to Accounts', keys: ['Ctrl', '5'] },
      { action: 'Go to Migration', keys: ['Ctrl', '6'] },
      { action: 'Go to Queue', keys: ['Ctrl', '7'] },
      { action: 'Go to Settings', keys: ['Ctrl', '8'] },
    ]
  },
  {
    name: 'Selection & Actions',
    shortcuts: [
      { action: 'Select all', keys: ['Ctrl', 'A'] },
      { action: 'Download selected', keys: ['Ctrl', 'D'] },
      { action: 'Add to queue', keys: ['Ctrl', 'Q'] },
      { action: 'New playlist', keys: ['Ctrl', 'N'] },
      { action: 'Focus search', keys: ['Ctrl', 'F'] },
      { action: 'Delete selected', keys: ['Delete'] },
    ]
  },
  {
    name: 'Playback',
    shortcuts: [
      { action: 'Play / Pause', keys: ['Space'] },
      { action: 'Previous track', keys: ['Ctrl', '←'] },
      { action: 'Next track', keys: ['Ctrl', '→'] },
      { action: 'Volume up', keys: ['Ctrl', '↑'] },
      { action: 'Volume down', keys: ['Ctrl', '↓'] },
    ]
  },
  {
    name: 'Library',
    shortcuts: [
      { action: 'Navigate tracks', keys: ['↑', '↓'] },
      { action: 'Open context menu', keys: ['Enter'] },
      { action: 'Select range', keys: ['Shift', 'Click'] },
      { action: 'Multi-select', keys: ['Ctrl', 'Click'] },
      { action: 'Toggle view mode', keys: ['L'] },
    ]
  },
  {
    name: 'Downloads',
    shortcuts: [
      { action: 'Pause selected', keys: ['P'] },
      { action: 'Retry failed', keys: ['R'] },
      { action: 'Pause all', keys: ['Ctrl', 'P'] },
      { action: 'Retry all failed', keys: ['Ctrl', 'R'] },
    ]
  },
  {
    name: 'Metadata & Lyrics',
    shortcuts: [
      { action: 'Save changes', keys: ['Ctrl', 'S'] },
      { action: 'Edit mode', keys: ['Ctrl', 'E'] },
      { action: 'Fetch from MusicBrainz', keys: ['Ctrl', 'B'] },
      { action: 'Fetch from Last.fm', keys: ['Ctrl', 'L'] },
      { action: 'Navigate form fields', keys: ['Tab'] },
    ]
  },
])

// Computed
const filteredSections = computed(() => {
  if (!searchQuery.value.trim()) return shortcutSections.value
  
  const query = searchQuery.value.toLowerCase()
  return shortcutSections.value
    .map(section => ({
      ...section,
      shortcuts: section.shortcuts.filter(s => 
        s.action.toLowerCase().includes(query) ||
        s.keys.join(' ').toLowerCase().includes(query)
      )
    }))
    .filter(section => section.shortcuts.length > 0)
})

// Methods
function toggleSection(name: string) {
  const index = collapsedSections.value.indexOf(name)
  if (index === -1) {
    collapsedSections.value.push(name)
  } else {
    collapsedSections.value.splice(index, 1)
  }
}

function highlightMatch(text: string): string {
  if (!searchQuery.value.trim()) return text
  const regex = new RegExp(`(${searchQuery.value})`, 'gi')
  return text.replace(regex, '<mark class="bg-yellow-200 dark:bg-yellow-500/30 rounded px-0.5">$1</mark>')
}

function dismissHint() {
  showFirstTimeHint.value = false
  hasSeenHint.value = true
  localStorage.setItem('syncify_seen_shortcut_hint', 'true')
}

// Global keyboard handler
function handleKeydown(event: KeyboardEvent) {
  const target = event.target as HTMLElement
  const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable
  
  // ? key - Show help (not in inputs)
  if (event.key === '?' && !isInput) {
    event.preventDefault()
    showHelpModal.value = true
    return
  }
  
  // Ctrl+H - Toggle help
  if (event.ctrlKey && event.key === 'h') {
    event.preventDefault()
    showHelpModal.value = !showHelpModal.value
    return
  }
  
  // Escape - Close modals
  if (event.key === 'Escape') {
    if (showHelpModal.value) {
      showHelpModal.value = false
      return
    }
  }
  
  // Ctrl+K - Command palette
  if (event.ctrlKey && event.key === 'k') {
    event.preventDefault()
    emit('command-palette')
    return
  }
  
  // Ctrl+, - Settings
  if (event.ctrlKey && event.key === ',') {
    event.preventDefault()
    router.push('/settings')
    return
  }
  
  // Ctrl+F - Focus search
  if (event.ctrlKey && event.key === 'f' && !isInput) {
    event.preventDefault()
    emit('search')
    return
  }
  
  // Ctrl+R - Refresh
  if (event.ctrlKey && event.key === 'r') {
    event.preventDefault()
    emit('refresh')
    return
  }
  
  // Ctrl+1 through Ctrl+8 - Tab navigation
  if (event.ctrlKey && !event.shiftKey && !event.altKey) {
    const tabRoutes = ['/library', '/downloads', '/metadata', '/lyrics', '/accounts', '/migration', '/queue', '/settings']
    const num = parseInt(event.key)
    if (num >= 1 && num <= 8) {
      event.preventDefault()
      router.push(tabRoutes[num - 1])
      return
    }
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
  hasSeenHint.value = typeof localStorage !== 'undefined' && localStorage && typeof localStorage.getItem === 'function'
    ? localStorage.getItem('syncify_seen_shortcut_hint') === 'true'
    : false
  
  // Show hint after delay on first visit
  if (!hasSeenHint.value) {
    setTimeout(() => {
      showFirstTimeHint.value = true
    }, 3000)
  }
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})

// Expose for external control
defineExpose({
  show: () => showHelpModal.value = true,
  hide: () => showHelpModal.value = false,
  toggle: () => showHelpModal.value = !showHelpModal.value
})
</script>

<style scoped>
/* Keyboard Key Styling */
.keyboard-key {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  height: 24px;
  padding: 0 8px;
  background: linear-gradient(180deg, #f8f9fa 0%, #e9ecef 100%);
  border: 1px solid #ced4da;
  border-radius: 4px;
  box-shadow: 0 2px 0 #adb5bd, inset 0 -1px 0 #dee2e6;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11px;
  font-weight: 600;
  color: #495057;
  text-transform: capitalize;
}

.dark .keyboard-key {
  background: linear-gradient(180deg, #3a3f44 0%, #2d3136 100%);
  border-color: #4a5057;
  box-shadow: 0 2px 0 #1a1d20, inset 0 -1px 0 #4a5057;
  color: #e9ecef;
}

.keyboard-key-sm {
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  font-size: 10px;
}

/* Modal Transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Accordion Transition */
.accordion-enter-active,
.accordion-leave-active {
  transition: all 0.2s ease;
  overflow: hidden;
}

.accordion-enter-from,
.accordion-leave-to {
  opacity: 0;
  max-height: 0;
}

.accordion-enter-to,
.accordion-leave-from {
  max-height: 500px;
}

/* Custom Scrollbar */
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

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(0, 0, 0, 0.2);
}

.dark .custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.2);
}

/* Focus visible for accessibility */
:deep(.focus-visible) {
  outline: 2px solid #6366f1;
  outline-offset: 2px;
}

button:focus-visible,
input:focus-visible {
  outline: 2px solid #6366f1;
  outline-offset: 2px;
}
</style>
