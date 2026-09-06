<template>
  <div class="quick-actions-fab-container">
    <!-- Backdrop -->
    <Transition name="fade">
      <div 
        v-if="isOpen" 
        class="fab-backdrop fixed inset-0 bg-black/20 z-[80]"
        @click="close"
      ></div>
    </Transition>
    
    <!-- Radial Menu Actions -->
    <div :class="['fab-menu fixed right-6 z-[85]', current ? 'bottom-28' : 'bottom-6']">
      <TransitionGroup name="fab-action">
        <div 
          v-for="(action, index) in visibleActions" 
          v-show="isOpen"
          :key="action.id"
          class="fab-action absolute"
          :style="getActionPosition(index)"
        >
          <button 
            @click="executeAction(action)"
            @mouseenter="hoveredAction = action.id"
            @mouseleave="hoveredAction = null"
            :class="['w-11 h-11 rounded-full flex items-center justify-center shadow-lg transition-all hover:scale-110', action.bgColor]"
            :title="action.label"
          >
            <span class="material-symbols-outlined text-white text-xl">{{ action.icon }}</span>
          </button>
          
          <!-- Tooltip -->
          <Transition name="fade">
            <div 
              v-if="hoveredAction === action.id"
              class="fab-tooltip absolute right-14 top-1/2 -translate-y-1/2 px-3 py-1.5 bg-gray-900 text-white text-xs font-medium rounded-lg whitespace-nowrap shadow-lg"
            >
              {{ action.label }}
              <span v-if="action.shortcut" class="ml-2 text-gray-400">{{ action.shortcut }}</span>
            </div>
          </Transition>
        </div>
      </TransitionGroup>
    </div>
    
    <!-- Main FAB Button -->
    <button 
      @click="toggle"
      :class="[
        'quick-actions-fab fixed right-6 w-14 h-14 rounded-full flex items-center justify-center shadow-xl z-[90] transition-all',
        current ? 'bottom-28' : 'bottom-6',
        isOpen ? 'bg-gray-700 rotate-45' : 'bg-primary hover:bg-primary-hover hover:scale-110',
        feedbackState === 'success' && 'bg-green-500',
        feedbackState === 'error' && 'bg-red-500 animate-shake',
        feedbackState === 'loading' && 'bg-primary'
      ]"
      :title="isOpen ? 'Close menu' : 'Quick actions'"
    >
      <span v-if="feedbackState === 'loading'" class="animate-spin">
        <svg class="w-6 h-6 text-white" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" opacity="0.25"></circle>
          <path d="M12 2a10 10 0 0 1 10 10" stroke="currentColor" stroke-width="3" stroke-linecap="round"></path>
        </svg>
      </span>
      <span v-else-if="feedbackState === 'success'" class="material-symbols-outlined text-white text-2xl">check</span>
      <span v-else class="material-symbols-outlined text-white text-2xl transition-transform">
        {{ isOpen ? 'close' : 'bolt' }}
      </span>
    </button>
    
    <!-- Keyboard hint (when menu open) -->
    <Transition name="fade">
      <div 
        v-if="isOpen" 
        :class="['fixed right-6 z-[85] text-xs text-gray-400 text-right', current ? 'bottom-46' : 'bottom-24']"
      >
        <p>Press <kbd class="px-1.5 py-0.5 bg-gray-700 rounded text-gray-300">1</kbd>-<kbd class="px-1.5 py-0.5 bg-gray-700 rounded text-gray-300">{{ visibleActions.length }}</kbd> to select</p>
        <p>Press <kbd class="px-1.5 py-0.5 bg-gray-700 rounded text-gray-300">Esc</kbd> to close</p>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { usePlayer } from '../composables/usePlayer'
import { useEventBus } from '../composables/useEventBus'

const { current } = usePlayer()
const eventBus = useEventBus()

export type ActionCallback = ((errOrPromise?: unknown) => void) & {
  resolve: () => void
  reject: (err?: unknown) => void
  waitUntil: (p: Promise<unknown>) => void
  defer: () => void
}

export type QuickActionEvent =
  | 'download-url'
  | 'scan-folder'
  | 'scan-library'
  | 'sync-all'
  | 'new-playlist'
  | 'download-selected'
  | 'add-to-playlist'
  | 'batch-edit'
  | 'pause-all'
  | 'retry-failed'
  | 'clear-completed'
  | 'auto-fix'
  | 'fetch-metadata'
  | 'fetch-lyrics'
  | 'upgrade-lyrics'

const props = withDefaults(defineProps<{
  currentTab?: string
  selectedTracksCount?: number
  actionHandler?: (action: string) => Promise<unknown> | unknown
}>(), {
  currentTab: 'library',
  selectedTracksCount: 0,
  actionHandler: undefined
})

const emit = defineEmits<{
  (e: 'download-url', callback: ActionCallback): void
  (e: 'scan-folder', callback: ActionCallback): void
  (e: 'scan-library', callback: ActionCallback): void
  (e: 'sync-all', callback: ActionCallback): void
  (e: 'new-playlist', callback: ActionCallback): void
  (e: 'download-selected', callback: ActionCallback): void
  (e: 'add-to-playlist', callback: ActionCallback): void
  (e: 'batch-edit', callback: ActionCallback): void
  (e: 'pause-all', callback: ActionCallback): void
  (e: 'retry-failed', callback: ActionCallback): void
  (e: 'clear-completed', callback: ActionCallback): void
  (e: 'auto-fix', callback: ActionCallback): void
  (e: 'fetch-metadata', callback: ActionCallback): void
  (e: 'fetch-lyrics', callback: ActionCallback): void
  (e: 'upgrade-lyrics', callback: ActionCallback): void
  (e: 'action', action: string, callback: ActionCallback): void
}>()

// State
const isOpen = ref(false)
const hoveredAction = ref<string | null>(null)
const feedbackState = ref<'idle' | 'loading' | 'success' | 'error'>('idle')

// Action definitions
interface QuickAction {
  id: string
  label: string
  icon: string
  bgColor: string
  shortcut?: string
  event: string
  tabs?: string[]
  conditional?: () => boolean
}

const allActions: QuickAction[] = [
  { id: 'download-url', label: 'Download from URL', icon: 'link', bgColor: 'bg-blue-500', shortcut: '1', event: 'download-url' },
  { id: 'scan-folder', label: 'Scan Local Folder', icon: 'folder_open', bgColor: 'bg-green-500', shortcut: '2', event: 'scan-folder' },
  { id: 'sync-all', label: 'Sync All Services', icon: 'sync', bgColor: 'bg-purple-500', shortcut: '3', event: 'sync-all' },
  { id: 'new-playlist', label: 'New Playlist', icon: 'playlist_add', bgColor: 'bg-orange-500', shortcut: '4', event: 'new-playlist' },
  { id: 'download-selected', label: 'Download Selected', icon: 'download', bgColor: 'bg-teal-500', shortcut: '5', event: 'download-selected', tabs: ['library'], conditional: () => props.selectedTracksCount > 0 },
  { id: 'add-to-playlist', label: 'Add to Playlist', icon: 'playlist_add_check', bgColor: 'bg-pink-500', event: 'add-to-playlist', tabs: ['library'], conditional: () => props.selectedTracksCount > 0 },
  { id: 'batch-edit', label: 'Batch Edit Metadata', icon: 'edit_note', bgColor: 'bg-amber-500', event: 'batch-edit', tabs: ['library'], conditional: () => props.selectedTracksCount > 0 },
  { id: 'pause-all', label: 'Pause All Downloads', icon: 'pause', bgColor: 'bg-gray-500', event: 'pause-all', tabs: ['downloads'] },
  { id: 'retry-failed', label: 'Retry Failed', icon: 'refresh', bgColor: 'bg-red-500', event: 'retry-failed', tabs: ['downloads'] },
  { id: 'clear-completed', label: 'Clear Completed', icon: 'delete_sweep', bgColor: 'bg-gray-600', event: 'clear-completed', tabs: ['downloads'] },
  { id: 'auto-fix', label: 'Auto-Fix All Issues', icon: 'auto_fix_high', bgColor: 'bg-cyan-500', event: 'auto-fix', tabs: ['metadata'] },
  { id: 'fetch-metadata', label: 'Fetch Missing Metadata', icon: 'library_music', bgColor: 'bg-indigo-500', event: 'fetch-metadata', tabs: ['metadata'] },
  { id: 'fetch-lyrics', label: 'Fetch Missing Lyrics', icon: 'lyrics', bgColor: 'bg-rose-500', event: 'fetch-lyrics', tabs: ['lyrics'] },
  { id: 'upgrade-lyrics', label: 'Upgrade to Synced', icon: 'timer', bgColor: 'bg-violet-500', event: 'upgrade-lyrics', tabs: ['lyrics'] },
]

// Computed: visible actions based on current tab
const visibleActions = computed(() => {
  return allActions
    .filter(action => {
      // Show if no tab restriction or matches current tab
      const tabMatch = !action.tabs || action.tabs.includes(props.currentTab)
      // Check conditional
      const conditionMet = !action.conditional || action.conditional()
      return tabMatch && conditionMet
    })
    .sort((a, b) => {
      // Prioritize tab-specific actions over global actions
      const aTabSpecific = a.tabs && a.tabs.includes(props.currentTab) ? 1 : 0
      const bTabSpecific = b.tabs && b.tabs.includes(props.currentTab) ? 1 : 0
      return bTabSpecific - aTabSpecific
    })
    .slice(0, 7)
})

// Calculate positions for radial layout
function getActionPosition(index: number) {
  const totalActions = visibleActions.value.length
  const startAngle = -90 // Start from top
  const angleSpread = Math.min(180, totalActions * 40) // Spread angle
  const angleStep = angleSpread / (totalActions - 1 || 1)
  const angle = startAngle - (angleSpread / 2) + (angleStep * index)
  const radius = 80 // Distance from FAB center
  
  const radian = (angle * Math.PI) / 180
  const x = Math.cos(radian) * radius
  const y = Math.sin(radian) * radius
  
  return {
    transform: `translate(${x}px, ${y}px)`,
    transitionDelay: `${index * 50}ms`
  }
}

// Toggle menu
function toggle() {
  isOpen.value = !isOpen.value
}

function close() {
  isOpen.value = false
  hoveredAction.value = null
}

// Execute action
async function executeAction(action: QuickAction) {
  close()
  feedbackState.value = 'loading'
  
  try {
    if (props.actionHandler) {
      await props.actionHandler(action.event)
    } else {
      let registeredAsync = false
      let asyncPromise: Promise<unknown> | null = null
      let cbResolve: () => void
      let cbReject: (err: unknown) => void

      const completionPromise = new Promise<void>((resolve, reject) => {
        cbResolve = resolve
        cbReject = reject
      })

      const callback: ActionCallback = Object.assign(
        (errOrPromise?: unknown) => {
          registeredAsync = true
          if (errOrPromise instanceof Promise) {
            asyncPromise = errOrPromise
          } else if (errOrPromise) {
            cbReject(errOrPromise)
          } else {
            cbResolve()
          }
        },
        {
          resolve: () => {
            registeredAsync = true
            cbResolve()
          },
          reject: (err?: unknown) => {
            registeredAsync = true
            cbReject(err || new Error('Action failed'))
          },
          waitUntil: (p: Promise<unknown>) => {
            registeredAsync = true
            asyncPromise = p
          },
          defer: () => {
            registeredAsync = true
          }
        }
      )

      emit(action.event as any, callback)
      emit('action', action.event, callback)
      eventBus.emit(action.event, callback)

      if (asyncPromise) {
        await asyncPromise
      } else if (registeredAsync) {
        await completionPromise
      }
    }
    
    feedbackState.value = 'success'
    setTimeout(() => {
      feedbackState.value = 'idle'
    }, 1200)
  } catch (e) {
    feedbackState.value = 'error'
    setTimeout(() => {
      feedbackState.value = 'idle'
    }, 1500)
  }
}

// Keyboard handling
function handleKeydown(e: KeyboardEvent) {
  // "/" opens menu
  if (e.key === '/' && !e.ctrlKey && !e.metaKey) {
    const target = e.target as HTMLElement
    if (target.tagName !== 'INPUT' && target.tagName !== 'TEXTAREA') {
      e.preventDefault()
      isOpen.value = true
    }
  }
  
  // Escape closes menu
  if (e.key === 'Escape' && isOpen.value) {
    close()
  }
  
  // Number keys select actions
  if (isOpen.value && /^[1-6]$/.test(e.key)) {
    const index = parseInt(e.key) - 1
    if (index < visibleActions.value.length) {
      executeAction(visibleActions.value[index])
    }
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})

// Expose for external control
defineExpose({
  open: () => isOpen.value = true,
  close,
  toggle,
  feedbackState,
  executeAction,
  visibleActions
})
</script>

<style scoped>
/* Fade animation */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* FAB action staggered animation */
.fab-action-enter-active {
  transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.fab-action-leave-active {
  transition: all 0.2s ease-in;
}

.fab-action-enter-from,
.fab-action-leave-to {
  opacity: 0;
  transform: translate(0, 0) scale(0.3) !important;
}

/* Shake animation */
@keyframes shake {
  0%, 100% { transform: translateX(0); }
  20%, 60% { transform: translateX(-4px); }
  40%, 80% { transform: translateX(4px); }
}

.animate-shake {
  animation: shake 0.4s ease;
}

/* Spin animation */
@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 0.8s linear infinite;
}

/* FAB shadow */
.quick-actions-fab {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

.quick-actions-fab:hover {
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
}

/* Tooltip arrow */
.fab-tooltip::after {
  content: '';
  position: absolute;
  right: -4px;
  top: 50%;
  transform: translateY(-50%);
  border: 4px solid transparent;
  border-left-color: #111827;
}

.bottom-46 {
  bottom: 11.5rem;
}
</style>
