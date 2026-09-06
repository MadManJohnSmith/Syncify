<template>
  <div class="system-tray-manager">
    <!-- This component manages system tray state from the frontend -->
    <!-- Actual tray is handled by Tauri backend -->
    
    <!-- Desktop Notification Permission Banner (if needed) -->
    <Transition name="slide-down">
      <div 
        v-if="showNotificationBanner" 
        class="fixed top-0 left-0 right-0 z-[100] bg-blue-500 text-white px-4 py-3"
      >
        <div class="max-w-screen-xl mx-auto flex items-center justify-between">
          <div class="flex items-center gap-3">
            <span class="material-symbols-outlined">notifications</span>
            <span>Enable notifications to get alerts when downloads complete</span>
          </div>
          <div class="flex items-center gap-3">
            <button @click="requestNotificationPermission" class="px-4 py-1.5 bg-white/20 hover:bg-white/30 rounded-lg text-sm font-medium">
              Enable
            </button>
            <button @click="dismissNotificationBanner" class="p-1 hover:bg-white/20 rounded">
              <span class="material-symbols-outlined text-lg">close</span>
            </button>
          </div>
        </div>
      </div>
    </Transition>
    
    <!-- Minimize to Tray Notification (one-time) -->
    <Transition name="slide-up">
      <div 
        v-if="showMinimizeHint" 
        class="fixed bottom-20 right-6 z-[100] bg-gray-800 border border-gray-700 rounded-xl shadow-xl p-4 w-80"
      >
        <div class="flex items-start gap-3">
          <span class="material-symbols-outlined text-primary text-xl">info</span>
          <div class="flex-1">
            <p class="text-sm text-white font-medium mb-1">Syncify is still running</p>
            <p class="text-xs text-gray-400">Click the tray icon to show the window. Right-click for quick actions.</p>
          </div>
          <button @click="dismissMinimizeHint" class="text-gray-500 hover:text-gray-300">
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>
        <label class="flex items-center gap-2 mt-3 text-xs text-gray-400 cursor-pointer">
          <input type="checkbox" v-model="dontShowMinimizeHint" class="w-3.5 h-3.5 rounded border-gray-600 text-primary focus:ring-primary bg-gray-700">
          Don't show this again
        </label>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// Props for tray state
const props = defineProps<{
  isDownloading?: boolean
  isSyncing?: boolean
  hasError?: boolean
  isPaused?: boolean
  downloadCount?: number
  syncService?: string
}>()

const emit = defineEmits(['show-window', 'hide-window', 'toggle-downloads', 'sync-all', 'open-settings', 'quit'])

// State
const showNotificationBanner = ref(false)
const showMinimizeHint = ref(false)
const dontShowMinimizeHint = ref(false)
const notificationsEnabled = ref(false)

// Tray state for Tauri
const trayState = ref<'default' | 'downloading' | 'syncing' | 'error' | 'paused'>('default')
let unlistenTrayAction: UnlistenFn | null = null

// Update tray state based on props
watch(() => [props.isDownloading, props.isSyncing, props.hasError, props.isPaused], () => {
  if (props.hasError) {
    trayState.value = 'error'
  } else if (props.isPaused) {
    trayState.value = 'paused'
  } else if (props.isSyncing) {
    trayState.value = 'syncing'
  } else if (props.isDownloading) {
    trayState.value = 'downloading'
  } else {
    trayState.value = 'default'
  }
  
  updateTrayIcon()
}, { immediate: true })

// Update tray icon via Tauri
async function updateTrayIcon() {
  try {
    await invoke('update_tray_icon', { state: trayState.value })
  } catch (e) {
    console.log('Tray update not available')
  }
}

// Show desktop notification
async function showNotification(title: string, body: string) {
  if (!notificationsEnabled.value) return
  
  try {
    if ('Notification' in window && Notification.permission === 'granted') {
      new Notification(title, { body, icon: '/icon.png' })
    }
  } catch (e) {
    console.log('Notification not available')
  }
}

// Request notification permission
async function requestNotificationPermission() {
  try {
    if ('Notification' in window) {
      const permission = await Notification.requestPermission()
      notificationsEnabled.value = permission === 'granted'
    }
    showNotificationBanner.value = false
    localStorage.setItem('syncify_notifications_prompted', 'true')
  } catch (e) {
    console.log('Notification permission not available')
  }
}

function dismissNotificationBanner() {
  showNotificationBanner.value = false
  localStorage.setItem('syncify_notifications_prompted', 'true')
}

function dismissMinimizeHint() {
  showMinimizeHint.value = false
  if (dontShowMinimizeHint.value) {
    localStorage.setItem('syncify_minimize_hint_dismissed', 'true')
  }
}

// Called when app minimizes to tray
function onMinimizeToTray() {
  const dismissed = localStorage.getItem('syncify_minimize_hint_dismissed')
  if (!dismissed) {
    showMinimizeHint.value = true
    setTimeout(() => {
      showMinimizeHint.value = false
    }, 5000)
  }
}

// Tray menu actions (called from Tauri)
function handleTrayAction(action: string) {
  switch (action) {
    case 'show':
      emit('show-window')
      break
    case 'hide':
      emit('hide-window')
      break
    case 'pause-downloads':
    case 'resume-downloads':
      emit('toggle-downloads')
      break
    case 'sync-all':
      emit('sync-all')
      break
    case 'settings':
      emit('open-settings')
      break
    case 'quit':
      emit('quit')
      break
  }
}

onMounted(async () => {
  // Check if notifications were prompted
  const prompted = localStorage.getItem('syncify_notifications_prompted')
  if (!prompted) {
    setTimeout(() => {
      showNotificationBanner.value = true
    }, 5000)
  }
  
  // Check notification permission
  if ('Notification' in window) {
    notificationsEnabled.value = Notification.permission === 'granted'
  }
  
  // Listen for tray events from Tauri
  try {
    unlistenTrayAction = await listen<string>('tray-action', (event) => {
      handleTrayAction(event.payload)
    })
  } catch (e) {
    console.log('Tray event listener not available')
  }
})

onUnmounted(() => {
  if (unlistenTrayAction) {
    unlistenTrayAction()
    unlistenTrayAction = null
  }
})

// Expose for external use
defineExpose({
  showNotification,
  onMinimizeToTray,
  trayState,
  notificationsEnabled
})
</script>

<style scoped>
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s ease;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-100%);
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.3s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(20px);
}
</style>
