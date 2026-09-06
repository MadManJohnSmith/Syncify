<template>
  <!-- Splash Screen -->
  <SplashScreen v-if="showSplash" @complete="showSplash = false" />
  
  <!-- Main App -->
  <div v-else class="bg-background-light dark:bg-background-dark text-white font-display overflow-hidden h-screen w-full flex">
    <!-- Sidebar -->
    <aside class="w-64 h-full bg-[#101723] border-r border-border-dark flex flex-col shrink-0 z-20">
      <nav class="flex-1 px-3 py-6 flex flex-col gap-1 overflow-y-auto">
        <router-link 
          to="/dashboard" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/dashboard' }">dashboard</span>
          <span class="text-sm font-medium">Dashboard</span>
        </router-link>

        <router-link 
          to="/search" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/search' }">search</span>
          <span class="text-sm font-medium">Search</span>
        </router-link>

        <router-link 
          to="/library" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/library' }">library_music</span>
          <span class="text-sm font-medium">Library</span>
        </router-link>

        <router-link 
          to="/playlists" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/playlists' }">queue_music</span>
          <span class="text-sm font-medium">Playlists</span>
        </router-link>
        
        <router-link 
          to="/downloads" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/downloads' }">download</span>
          <span class="text-sm font-medium">Downloads</span>
        </router-link>

        <router-link 
          to="/migration" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/migration' }">sync_alt</span>
          <span class="text-sm font-medium">Migrate</span>
        </router-link>

        <router-link 
          to="/accounts" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/accounts' }">manage_accounts</span>
          <span class="text-sm font-medium">Accounts</span>
        </router-link>

        <router-link 
          to="/metadata" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/metadata' }">edit_note</span>
          <span class="text-sm font-medium">Metadata</span>
        </router-link>

        <router-link 
          to="/lyrics" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/lyrics' }">lyrics</span>
          <span class="text-sm font-medium">Lyrics</span>
        </router-link>

        <div class="mt-auto"></div>
        <div class="my-2 border-t border-border-dark opacity-50"></div>

        <router-link 
          to="/logs" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/logs' }">terminal</span>
          <span class="text-sm font-medium">Logs</span>
        </router-link>

        <router-link 
          to="/settings" 
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-[#223149] !text-white"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors" :class="{ 'fill-1 text-primary': $route.path === '/settings' }">settings</span>
          <span class="text-sm font-medium">Settings</span>
        </router-link>
      </nav>
    </aside>

    <!-- Main Content -->
    <main class="flex-1 flex flex-col h-full overflow-hidden relative">
      <!-- Header -->
      <header class="h-16 w-full bg-[#101723] border-b border-border-dark flex items-center justify-between px-6 shrink-0 z-30 relative select-none" data-tauri-drag-region>
        <div class="flex items-center gap-3">
          <div class="w-8 h-8 bg-gradient-to-br from-primary to-purple-600 rounded-lg flex items-center justify-center shadow-lg shadow-primary/20">
            <span class="material-symbols-outlined text-white text-xl">all_inclusive</span>
          </div>
          <span class="text-lg font-bold tracking-tight text-white">Syncify</span>
        </div>
        
        <div class="flex items-center gap-3">
          <!-- Search Button (Ctrl+K) -->
          <button 
            @click="showCommandPalette = true"
            class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-dark/50 border border-border-dark/50 hover:bg-surface-dark transition-colors cursor-pointer text-text-secondary hover:text-white"
          >
            <span class="material-symbols-outlined text-lg">search</span>
            <span class="text-xs hidden md:inline">Search...</span>
            <kbd class="hidden md:inline px-1.5 py-0.5 bg-surface-dark rounded text-xs text-gray-500">⌘K</kbd>
          </button>
          
          <!-- Status Indicator with Dropdown -->
          <div class="relative">
            <div 
              @click="showTasksDropdown = !showTasksDropdown"
              class="status-indicator flex items-center gap-2 px-3 py-1.5 rounded-full bg-surface-dark/50 border border-border-dark/50 hover:bg-surface-dark transition-colors cursor-pointer group"
            >
              <span v-if="hasActiveTasks" class="relative flex h-2 w-2">
                <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75"></span>
                <span class="relative inline-flex rounded-full h-2 w-2 bg-primary"></span>
              </span>
              <span v-else class="relative inline-flex rounded-full h-2 w-2 bg-green-500"></span>
              <span class="text-xs font-medium text-text-secondary group-hover:text-white max-w-24 truncate">{{ taskStatusText }}</span>
              <span v-if="hasActiveTasks" class="material-symbols-outlined text-primary text-sm material-symbols-filled">bolt</span>
              <span v-else class="material-symbols-outlined text-green-500 text-sm">check_circle</span>
            </div>
            
            <!-- Tasks Dropdown -->
            <Transition name="slide-down">
              <div 
                v-if="showTasksDropdown" 
                class="tasks-dropdown absolute top-full right-0 mt-2 w-80 bg-gray-900 border border-gray-700 rounded-xl shadow-xl overflow-hidden z-50"
              >
                <div class="p-3 border-b border-gray-700 flex items-center justify-between">
                  <span class="font-medium text-white text-sm">Active Tasks</span>
                  <span v-if="hasActiveTasks" class="text-xs text-gray-400">{{ overallProgress }}% overall</span>
                </div>
                
                <div v-if="activeTasks.length > 0" class="max-h-80 overflow-y-auto">
                  <div 
                    v-for="task in activeTasks" 
                    :key="task.id"
                    class="p-3 border-b border-gray-800 last:border-0 hover:bg-gray-800/50 transition-colors"
                  >
                    <div class="flex items-center justify-between mb-1">
                      <div class="flex items-center gap-1.5 truncate flex-1">
                        <span class="text-sm font-medium text-white truncate">{{ task.name }}</span>
                        <span v-if="task.phase" class="px-1.5 py-0.5 rounded text-[10px] font-semibold bg-primary/20 text-primary uppercase shrink-0">
                          {{ task.phase }}
                        </span>
                      </div>
                      <span :class="[
                        'text-xs ml-2 shrink-0 font-medium',
                        task.status === 'requires_auth' ? 'text-red-400 font-semibold' : 'text-gray-400'
                      ]">
                        {{ formatProgress(task) }}
                      </span>
                    </div>
                    
                    <div v-if="task.description" class="text-xs text-gray-400 truncate mb-1.5">{{ task.description }}</div>

                    <div v-if="task.importedCount !== undefined || task.favoriteCount !== undefined" class="flex items-center gap-2 text-[11px] text-gray-400 mb-2">
                      <span v-if="task.importedCount !== undefined" class="flex items-center gap-0.5">
                        <span class="material-symbols-outlined text-[13px] text-primary">download</span>
                        {{ task.importedCount }} imported
                      </span>
                      <span v-if="task.favoriteCount !== undefined" class="flex items-center gap-0.5">
                        <span class="material-symbols-outlined text-[13px] text-red-400">favorite</span>
                        {{ task.favoriteCount }} favorites
                      </span>
                    </div>

                    <div class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
                      <div 
                        :class="[
                          'h-full rounded-full transition-all duration-300',
                          task.status === 'requires_auth' ? 'bg-red-500' :
                          task.status === 'failed' ? 'bg-amber-500' : 'bg-primary'
                        ]"
                        :style="{ width: (task.status === 'requires_auth' || task.status === 'failed' ? 100 : task.progress) + '%' }"
                      ></div>
                    </div>

                    <div v-if="task.status === 'requires_auth'" class="mt-2 flex items-center justify-between">
                      <span class="text-[11px] text-red-400">⚠️ Session expired</span>
                      <router-link to="/accounts" class="px-2 py-0.5 bg-red-500/20 text-red-400 hover:bg-red-500/30 rounded text-xs font-medium transition-colors">
                        Reconnect
                      </router-link>
                    </div>
                  </div>
                </div>
                
                <div v-else class="p-4 text-center text-gray-400 text-sm">
                  <span class="material-symbols-outlined text-2xl text-gray-600 block mb-2">check_circle</span>
                  No active tasks
                </div>
              </div>
            </Transition>
          </div>
          
          <!-- Single Notifications Bell -->
          <div class="relative notification-container">
            <button 
              @click="showNotifications = !showNotifications"
              class="notification-button notification-bell-btn relative p-2 rounded-lg hover:bg-surface-dark transition-colors text-text-secondary hover:text-white"
              title="Notifications"
            >
              <span class="material-symbols-outlined">notifications</span>
              <span v-if="unreadCount > 0" class="notification-badge absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] bg-red-500 rounded-full text-[10px] font-bold flex items-center justify-center text-white px-1">
                {{ unreadCount > 99 ? '99+' : unreadCount }}
              </span>
            </button>

            <!-- Notifications History Dropdown Panel -->
            <Transition name="dropdown">
              <div 
                v-if="showNotifications" 
                class="notification-dropdown absolute top-full right-0 mt-2 w-96 bg-white dark:bg-surface-dark rounded-xl shadow-2xl border border-gray-200 dark:border-border-dark overflow-hidden z-50"
              >
                <!-- Header -->
                <div class="px-4 py-3 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
                  <h4 class="font-semibold text-gray-900 dark:text-white text-sm">Notifications</h4>
                  <div class="flex gap-1">
                    <button 
                      :class="[
                        'px-2.5 py-1 rounded-lg text-xs font-medium transition-colors',
                        historyFilter === 'all' ? 'bg-primary/10 text-primary' : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight'
                      ]"
                      @click="historyFilter = 'all'"
                    >
                      All
                    </button>
                    <button 
                      :class="[
                        'px-2.5 py-1 rounded-lg text-xs font-medium transition-colors',
                        historyFilter === 'unread' ? 'bg-primary/10 text-primary' : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight'
                      ]"
                      @click="historyFilter = 'unread'"
                    >
                      Unread
                    </button>
                  </div>
                </div>
                
                <!-- Notifications List -->
                <div class="max-h-80 overflow-y-auto custom-scrollbar">
                  <div v-if="filteredNotifications.length === 0" class="p-6 text-center">
                    <span class="material-symbols-outlined text-3xl text-gray-300 dark:text-gray-600 mb-1 block">notifications_off</span>
                    <p class="text-xs text-gray-500">No notifications</p>
                  </div>
                  
                  <div 
                    v-for="notification in filteredNotifications" 
                    :key="notification.id"
                    :class="[
                      'px-4 py-2.5 border-b border-gray-100 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors cursor-pointer',
                      !notification.read && 'bg-primary/5'
                    ]"
                    @click="markAsRead(notification.id)"
                  >
                    <div class="flex items-start gap-3">
                      <div :class="[
                        'w-7 h-7 rounded-lg flex items-center justify-center shrink-0 mt-0.5',
                        notification.type === 'success' ? 'bg-success/20 text-success' :
                        notification.type === 'error' ? 'bg-error/20 text-error' :
                        notification.type === 'warning' ? 'bg-amber-500/20 text-amber-500' :
                        'bg-blue-500/20 text-blue-500'
                      ]">
                        <span class="material-symbols-outlined text-[16px]">
                          {{ notification.type === 'success' ? 'check_circle' : 
                             notification.type === 'error' ? 'error' : 
                             notification.type === 'warning' ? 'warning' : 'info' }}
                        </span>
                      </div>
                      <div class="flex-1 min-w-0">
                        <p class="text-xs text-gray-900 dark:text-white font-medium truncate">{{ notification.title }}</p>
                        <p v-if="notification.description" class="text-[11px] text-gray-500 mt-0.5 line-clamp-2">{{ notification.description }}</p>
                        <p class="text-[10px] text-gray-400 mt-0.5">{{ notification.timestamp }}</p>
                      </div>
                      <div v-if="!notification.read" class="w-1.5 h-1.5 bg-primary rounded-full shrink-0 mt-1.5"></div>
                    </div>
                  </div>
                </div>
                
                <!-- Footer -->
                <div class="px-4 py-2.5 border-t border-gray-200 dark:border-border-dark flex items-center justify-between text-xs">
                  <button @click="clearAllHistory" class="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
                    Clear All
                  </button>
                  <button @click="markAllAsRead" class="text-primary hover:underline">
                    Mark all as read
                  </button>
                </div>
              </div>
            </Transition>
          </div>
          
          <!-- Help Button -->
          <button 
            @click="showHelp = true"
            class="p-2 rounded-lg hover:bg-surface-dark transition-colors text-text-secondary hover:text-white"
          >
            <span class="material-symbols-outlined">help</span>
          </button>
        </div>
        
        <!-- Global Progress Bar -->
        <div class="absolute bottom-0 left-0 right-0 h-[2px] bg-surface-dark">
          <div 
            v-if="hasActiveTasks" 
            class="h-full bg-primary shadow-[0_0_8px_rgba(60,131,246,0.6)] rounded-r-full transition-all duration-300"
            :style="{ width: overallProgress + '%' }"
          ></div>
        </div>
      </header>

      <!-- Page Content -->
      <div class="flex-1 overflow-hidden relative pb-8">
        <router-view v-slot="{ Component }">
          <transition name="fade" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </div>
      
      <!-- Status Bar -->
      <StatusBar />
    </main>
    
    <!-- Global Components -->
    <NowPlayingBar />
    <ToastNotifications />
    <CommandPalette v-if="showCommandPalette" @close="showCommandPalette = false" />
    <KeyboardShortcuts />
    <HelpPanel v-if="showHelp" @close="showHelp = false" />
    <QuickActionsFab :currentTab="currentTab" />
    <OnboardingWizard v-if="showOnboarding" @complete="showOnboarding = false" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRoute } from 'vue-router'

// Global Components
import SplashScreen from './components/SplashScreen.vue'
import StatusBar from './components/StatusBar.vue'
import ToastNotifications from './components/ToastNotifications.vue'
import CommandPalette from './components/CommandPalette.vue'
import KeyboardShortcuts from './components/KeyboardShortcuts.vue'
import HelpPanel from './components/HelpPanel.vue'
import QuickActionsFab from './components/QuickActionsFab.vue'
import NowPlayingBar from './components/NowPlayingBar.vue'
import OnboardingWizard from './components/OnboardingWizard.vue'

// Composables
import { useGlobalTasks } from './composables/useGlobalTasks'
import { useToast } from './composables/useToast'
import { useNotificationListener } from './composables/useNotificationListener'
import { useLogs } from './composables/useLogs'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

const route = useRoute()
const toast = useToast()
const { unreadCount, history: notificationHistory, markAsRead, markAllAsRead, clearAllHistory } = toast
const { startListening: startNotificationListening, stopListening: stopNotificationListening } = useNotificationListener()
const { initLogListeners } = useLogs()

let unlistenPythonDeps: UnlistenFn | null = null

// Global tasks state
const {
  activeTasks,
  hasActiveTasks,
  activeTaskCount,
  overallProgress,
  initEventListeners
} = useGlobalTasks()

// Global state
const showSplash = ref(false)
const showCommandPalette = ref(false)
const showNotifications = ref(false)
const showTasksDropdown = ref(false)
const showHelp = ref(false)
const showOnboarding = ref(false)
const historyFilter = ref<'all' | 'unread'>('all')

const filteredNotifications = computed(() => {
  if (historyFilter.value === 'unread') {
    return notificationHistory.value.filter(n => !n.read)
  }
  return notificationHistory.value
})

// Current tab for contextual FAB actions
const currentTab = computed(() => {
  const path = route?.path ? route.path.replace('/', '') : ''
  return path || 'dashboard'
})

// Task status text for indicator
const taskStatusText = computed(() => {
  if (!hasActiveTasks.value) return 'Idle'
  if (activeTaskCount.value === 1) return activeTasks.value[0]?.name || 'Running...'
  return `${activeTaskCount.value} tasks`
})

// Format task progress
function formatProgress(task: any): string {
  if (task.status === 'requires_auth') {
    return 'Auth Required'
  }
  if (task.status === 'failed') {
    return 'Failed'
  }
  if (task.current !== undefined && task.total !== undefined && task.total > 0) {
    return `${task.current}/${task.total}`
  }
  return `${task.progress}%`
}

// Close dropdown on outside click
function handleOutsideClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (!target.closest('.tasks-dropdown') && !target.closest('.status-indicator')) {
    showTasksDropdown.value = false
  }
  if (!target.closest('.notification-dropdown') && !target.closest('.notification-button')) {
    showNotifications.value = false
  }
}

// Global keydown listener (e.g. Ctrl+K)
function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault()
    showCommandPalette.value = true
  }
}

// Check if first-time user
onMounted(async () => {
  // Initialize global task and log event listeners
  initEventListeners()
  startNotificationListening()
  initLogListeners()

  // Listen for missing python dependencies
  try {
    unlistenPythonDeps = await listen('python_deps_missing', (event: any) => {
      toast.error(event.payload.message)
    })
  } catch (err) {
    console.warn('Failed to listen for python_deps_missing:', err)
  }
  
  // Add outside click handler
  document.addEventListener('click', handleOutsideClick)
  
  // Show splash for 2 seconds
  setTimeout(() => {
    showSplash.value = false
    
    // Check if onboarding needed (no services connected)
    const hasCompletedOnboarding = localStorage.getItem('syncify_onboarding_complete')
    if (!hasCompletedOnboarding) {
      setTimeout(() => {
        showOnboarding.value = true
      }, 500)
    }
  }, 2000)
  
  // Listen for Ctrl+K
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  unlistenPythonDeps?.()
  unlistenPythonDeps = null
  stopNotificationListening()
  document.removeEventListener('click', handleOutsideClick)
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<style>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

@keyframes loading-bar {
  0% { transform: translateX(-100%); width: 25%; }
  50% { width: 50%; }
  100% { transform: translateX(400%); width: 25%; }
}

.animate-loading-bar {
  animation: loading-bar 1.5s ease-in-out infinite;
}

/* Slide down animation */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.2s ease;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
