<template>
  <div class="tray-notification-settings space-y-6">
    <!-- Application Behavior -->
    <div class="settings-group">
      <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-4 flex items-center gap-2">
        <span class="material-symbols-outlined text-lg">desktop_windows</span>
        Application Behavior
      </h3>
      
      <div class="space-y-4">
        <!-- Close to tray -->
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Close to tray instead of exit</p>
            <p class="text-xs text-gray-500 mt-1">When you click the close button, minimize to system tray instead of quitting</p>
          </div>
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" v-model="settings.closeToTray" class="sr-only peer" @change="save">
            <div class="w-11 h-6 bg-gray-300 peer-focus:ring-2 peer-focus:ring-primary/50 rounded-full peer dark:bg-gray-600 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
        
        <!-- Start minimized -->
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Start minimized to tray</p>
            <p class="text-xs text-gray-500 mt-1">Start Syncify in the background when launching</p>
          </div>
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" v-model="settings.startMinimized" class="sr-only peer" @change="save">
            <div class="w-11 h-6 bg-gray-300 peer-focus:ring-2 peer-focus:ring-primary/50 rounded-full peer dark:bg-gray-600 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
        
        <!-- Start on boot -->
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Start on system boot</p>
            <p class="text-xs text-gray-500 mt-1">Automatically launch Syncify when you log in</p>
          </div>
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" v-model="settings.startOnBoot" class="sr-only peer" @change="save">
            <div class="w-11 h-6 bg-gray-300 peer-focus:ring-2 peer-focus:ring-primary/50 rounded-full peer dark:bg-gray-600 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
      </div>
    </div>
    
    <!-- Notifications -->
    <div class="settings-group">
      <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-4 flex items-center gap-2">
        <span class="material-symbols-outlined text-lg">notifications</span>
        Desktop Notifications
      </h3>
      
      <div class="space-y-4">
        <!-- Master toggle -->
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Show system notifications</p>
            <p class="text-xs text-gray-500 mt-1">Display notifications in your system tray</p>
          </div>
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" v-model="settings.notificationsEnabled" class="sr-only peer" @change="save">
            <div class="w-11 h-6 bg-gray-300 peer-focus:ring-2 peer-focus:ring-primary/50 rounded-full peer dark:bg-gray-600 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
        
        <!-- Notification events -->
        <div v-if="settings.notificationsEnabled" class="pl-4 border-l-2 border-primary/20 space-y-3">
          <label class="flex items-center gap-3 cursor-pointer">
            <input type="checkbox" v-model="settings.notifyDownloadComplete" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary" @change="save">
            <span class="text-sm text-gray-700 dark:text-gray-300">Download completed</span>
          </label>
          <label class="flex items-center gap-3 cursor-pointer">
            <input type="checkbox" v-model="settings.notifySyncComplete" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary" @change="save">
            <span class="text-sm text-gray-700 dark:text-gray-300">Sync completed</span>
          </label>
          <label class="flex items-center gap-3 cursor-pointer">
            <input type="checkbox" v-model="settings.notifyErrors" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary" @change="save">
            <span class="text-sm text-gray-700 dark:text-gray-300">Errors and warnings</span>
          </label>
          <label class="flex items-center gap-3 cursor-pointer">
            <input type="checkbox" v-model="settings.notifyUpdates" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary" @change="save">
            <span class="text-sm text-gray-700 dark:text-gray-300">Updates available</span>
          </label>
        </div>
        
        <!-- Sound toggle -->
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Play notification sound</p>
            <p class="text-xs text-gray-500 mt-1">Play a sound when notifications appear</p>
          </div>
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" v-model="settings.notificationSound" class="sr-only peer" @change="save">
            <div class="w-11 h-6 bg-gray-300 peer-focus:ring-2 peer-focus:ring-primary/50 rounded-full peer dark:bg-gray-600 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
        
        <!-- Show when visible -->
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Show when app is visible</p>
            <p class="text-xs text-gray-500 mt-1">Show notifications even when Syncify is in the foreground</p>
          </div>
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" v-model="settings.notifyWhenVisible" class="sr-only peer" @change="save">
            <div class="w-11 h-6 bg-gray-300 peer-focus:ring-2 peer-focus:ring-primary/50 rounded-full peer dark:bg-gray-600 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
      </div>
    </div>
    
    <!-- Tray Icon -->
    <div class="settings-group">
      <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-4 flex items-center gap-2">
        <span class="material-symbols-outlined text-lg">dock_to_right</span>
        System Tray
      </h3>
      
      <div class="space-y-4">
        <!-- Show tray icon -->
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Show tray icon</p>
            <p class="text-xs text-gray-500 mt-1">Display Syncify icon in the system tray</p>
          </div>
          <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" v-model="settings.showTrayIcon" class="sr-only peer" @change="save">
            <div class="w-11 h-6 bg-gray-300 peer-focus:ring-2 peer-focus:ring-primary/50 rounded-full peer dark:bg-gray-600 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary"></div>
          </label>
        </div>
        
        <!-- Tray icon style -->
        <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
          <p class="text-sm font-medium text-gray-900 dark:text-white mb-3">Tray icon style</p>
          <div class="flex gap-4">
            <label 
              v-for="style in iconStyles" 
              :key="style.id"
              class="flex flex-col items-center gap-2 cursor-pointer"
            >
              <div 
                :class="[
                  'w-12 h-12 rounded-xl flex items-center justify-center border-2 transition-colors',
                  settings.trayIconStyle === style.id ? 'border-primary bg-primary/10' : 'border-gray-200 dark:border-gray-600'
                ]"
              >
                <div :class="['w-6 h-6 rounded', style.bgClass]"></div>
              </div>
              <span class="text-xs text-gray-500">{{ style.label }}</span>
              <input type="radio" :value="style.id" v-model="settings.trayIconStyle" class="sr-only" @change="save">
            </label>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Keyboard Shortcuts -->
    <div class="settings-group">
      <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-4 flex items-center gap-2">
        <span class="material-symbols-outlined text-lg">keyboard</span>
        Global Shortcuts
      </h3>
      
      <div class="space-y-3">
        <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
          <span class="text-sm text-gray-700 dark:text-gray-300">Show/Hide Syncify</span>
          <div class="flex items-center gap-1">
            <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 text-xs rounded">Ctrl</kbd>
            <span class="text-gray-400">+</span>
            <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 text-xs rounded">Alt</kbd>
            <span class="text-gray-400">+</span>
            <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 text-xs rounded">S</kbd>
          </div>
        </div>
        <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
          <span class="text-sm text-gray-700 dark:text-gray-300">Quit Syncify</span>
          <div class="flex items-center gap-1">
            <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 text-xs rounded">Ctrl</kbd>
            <span class="text-gray-400">+</span>
            <kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 text-xs rounded">Q</kbd>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Test Notification -->
    <button 
      @click="testNotification" 
      class="flex items-center gap-2 px-4 py-2 text-sm text-primary hover:bg-primary/10 rounded-lg transition-colors"
    >
      <span class="material-symbols-outlined text-lg">notifications_active</span>
      Send Test Notification
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'

const emit = defineEmits(['save'])

// Settings state
const settings = ref({
  // Application behavior
  closeToTray: true,
  startMinimized: false,
  startOnBoot: false,
  
  // Notifications
  notificationsEnabled: true,
  notifyDownloadComplete: true,
  notifySyncComplete: true,
  notifyErrors: true,
  notifyUpdates: true,
  notificationSound: false,
  notifyWhenVisible: false,
  
  // Tray icon
  showTrayIcon: true,
  trayIconStyle: 'color',
})

// Icon style options
const iconStyles = [
  { id: 'color', label: 'Color', bgClass: 'bg-gradient-to-br from-primary to-blue-600' },
  { id: 'white', label: 'White', bgClass: 'bg-white' },
  { id: 'dark', label: 'Dark', bgClass: 'bg-gray-800' },
]

// Save settings
async function save() {
  localStorage.setItem('syncify_tray_settings', JSON.stringify(settings.value))
  emit('save', settings.value)
  
  // Update Tauri tray if available
  try {
    await invoke('update_tray_settings', { settings: settings.value })
  } catch (e) {
    console.log('Tray settings update not available')
  }
}

// Test notification
function testNotification() {
  if ('Notification' in window && Notification.permission === 'granted') {
    new Notification('Test Notification', {
      body: 'This is a test notification from Syncify!',
      icon: '/icon.png'
    })
  } else {
    alert('Notifications are not enabled or supported.')
  }
}

// Load settings
onMounted(() => {
  const saved = localStorage.getItem('syncify_tray_settings')
  if (saved) {
    try {
      Object.assign(settings.value, JSON.parse(saved))
    } catch {}
  }
})

defineExpose({ settings, save })
</script>

<style scoped>
/* Toggle switch styling handled by Tailwind peer classes */
</style>
