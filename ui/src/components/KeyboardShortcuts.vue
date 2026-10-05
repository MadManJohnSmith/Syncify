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
            <div class="print-hidden px-6 py-4 border-b border-gray-200 dark:border-border-dark shrink-0">
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
            <div class="shortcuts-list flex-1 overflow-y-auto custom-scrollbar px-6 py-4">
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
            <div class="print-hidden px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-end shrink-0">
              <!-- FE-12: real print support for the cheat sheet. The
                   "Customize Shortcuts" button was removed: the shortcut
                   registry has no rebinding support, so the button was dead. -->
              <button @click="printCheatSheet" class="flex items-center gap-2 text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 transition-colors">
                <span class="material-symbols-outlined text-[18px]">print</span>
                Print Cheat Sheet
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
import { escapeHtml, escapeRegExp, highlightMatch as safeHighlightMatch } from '@/utils/sanitize'
import { useKeyboardShortcuts, registerShortcut } from '@/composables/useKeyboardShortcuts'

const router = useRouter()

// Composable integration
const {
  showShortcutsHelp,
  openShortcutsHelp,
  closeShortcutsHelp,
  toggleShortcutsHelp,
} = useKeyboardShortcuts()

// Two-way reactive connection with showShortcutsHelp
const showHelpModal = computed({
  get: () => showShortcutsHelp.value,
  set: (val: boolean) => {
    showShortcutsHelp.value = val
  }
})

const searchQuery = ref('')
const collapsedSections = ref<string[]>([])
const showFirstTimeHint = ref(true)
const hasSeenHint = ref(false)

// Shortcut definitions. Single source of truth: every entry is registered as a
// real key binding AND listed in the sheet, so the help can no longer advertise
// a combination that nobody handles (audit 8).
//
// `external: true` marks a documented shortcut handled by someone else: it is
// listed but not registered here, because this component's document listener
// runs before App's and its preventDefault() would make the owner bail out on
// `event.defaultPrevented`.
interface ShortcutDef {
  section: string
  action: string
  keys: string[]
  combo: string
  external?: boolean
  when?: () => boolean
  run: () => void
}

const tabRoutes: Array<{ keys: string[]; combo: string; route: string; action: string }> = [
  { keys: ['Ctrl', '1'], combo: 'Ctrl+1', route: '/library', action: 'Go to Library' },
  { keys: ['Ctrl', '2'], combo: 'Ctrl+2', route: '/downloads', action: 'Go to Downloads' },
  { keys: ['Ctrl', '3'], combo: 'Ctrl+3', route: '/metadata', action: 'Go to Metadata' },
  { keys: ['Ctrl', '4'], combo: 'Ctrl+4', route: '/lyrics', action: 'Go to Lyrics' },
  { keys: ['Ctrl', '5'], combo: 'Ctrl+5', route: '/accounts', action: 'Go to Accounts' },
  { keys: ['Ctrl', '6'], combo: 'Ctrl+6', route: '/migration', action: 'Go to Migration' },
  { keys: ['Ctrl', '7'], combo: 'Ctrl+7', route: '/queue', action: 'Go to Downloads / Queue' },
  { keys: ['Ctrl', '8'], combo: 'Ctrl+8', route: '/settings', action: 'Go to Settings' },
]

const shortcutDefs: ShortcutDef[] = [
  {
    section: 'Global',
    action: 'Show keyboard shortcuts',
    keys: ['?'],
    combo: '?',
    run: () => openShortcutsHelp()
  },
  {
    section: 'Global',
    action: 'Open command palette',
    keys: ['Ctrl', 'K'],
    combo: 'Ctrl+K',
    external: true,
    run: () => {}
  },
  {
    section: 'Global',
    action: 'Open Settings',
    keys: ['Ctrl', ','],
    combo: 'Ctrl+,',
    run: () => router.push('/settings')
  },
  {
    section: 'Global',
    action: 'Toggle this shortcuts sheet',
    keys: ['Ctrl', '/'],
    combo: 'Ctrl+/',
    run: () => toggleShortcutsHelp()
  },
  {
    section: 'Global',
    action: 'Toggle this shortcuts sheet',
    keys: ['Ctrl', 'H'],
    combo: 'Ctrl+H',
    run: () => toggleShortcutsHelp()
  },
  {
    section: 'Global',
    action: 'Close this sheet',
    keys: ['Escape'],
    combo: 'Escape',
    when: () => showShortcutsHelp.value,
    run: () => closeShortcutsHelp()
  },
  {
    section: 'Global',
    action: 'Reload the current view',
    keys: ['Ctrl', 'R'],
    combo: 'Ctrl+R',
    run: () => router.go(0)
  },
  ...tabRoutes.map((entry): ShortcutDef => ({
    section: 'Navigation',
    action: entry.action,
    keys: entry.keys,
    combo: entry.combo,
    run: () => router.push(entry.route)
  }))
]

const shortcutSections = computed(() => {
  const sections: Array<{ name: string; shortcuts: Array<{ action: string; keys: string[] }> }> = []
  for (const def of shortcutDefs) {
    let section = sections.find(s => s.name === def.section)
    if (!section) {
      section = { name: def.section, shortcuts: [] }
      sections.push(section)
    }
    section.shortcuts.push({ action: def.action, keys: def.keys })
  }
  return sections
})

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
  return safeHighlightMatch(text, searchQuery.value)
}

// FE-12: print only the cheat sheet. The @media print rules below hide
// everything else and force a light, ink-friendly palette.
function printCheatSheet() {
  window.print()
}

function dismissHint() {
  showFirstTimeHint.value = false
  hasSeenHint.value = true
  if (typeof localStorage !== 'undefined' && localStorage && typeof localStorage.setItem === 'function') {
    localStorage.setItem('syncify_seen_shortcut_hint', 'true')
  }
}

// Register shortcuts using the composable
shortcutDefs
  .filter(def => !def.external)
  .forEach(def => {
    registerShortcut(def.combo, def.run, {
      description: def.action,
      when: def.when
    })
  })

onMounted(() => {
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

// Expose for external control
defineExpose({
  show: () => openShortcutsHelp(),
  hide: () => closeShortcutsHelp(),
  toggle: () => toggleShortcutsHelp(),
  showHelpModal,
  showShortcutsHelp,
  highlightMatch,
  searchQuery,
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

<!-- FE-12: print stylesheet for the cheat sheet. Not scoped on purpose: the
     modal is teleported to <body> and the rules must reach the whole page. -->
<style>
@media print {
  body * {
    visibility: hidden !important;
  }

  .shortcuts-modal,
  .shortcuts-modal * {
    visibility: visible !important;
  }

  .shortcuts-modal {
    position: static !important;
    inset: auto !important;
    background: #fff !important;
    padding: 0 !important;
    color: #111 !important;
  }

  .shortcuts-modal > div {
    max-height: none !important;
    max-width: none !important;
    box-shadow: none !important;
    background: #fff !important;
  }

  .shortcuts-modal .print-hidden {
    display: none !important;
  }

  /* Show every section, not just what fits the on-screen scroll area. */
  .shortcuts-modal .shortcuts-list {
    overflow: visible !important;
    max-height: none !important;
  }

  .shortcuts-modal .keyboard-key {
    background: #fff !important;
    border-color: #999 !important;
    box-shadow: none !important;
    color: #000 !important;
  }

  .shortcuts-modal h2,
  .shortcuts-modal h3,
  .shortcuts-modal span {
    color: #111 !important;
  }
}
</style>
