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
                
                <div v-if="activeTasks.length > 0" class="max-h-64 overflow-y-auto">
                  <div 
                    v-for="task in activeTasks" 
                    :key="task.id"
                    class="p-3 border-b border-gray-800 last:border-0 hover:bg-gray-800/50"
                  >
                    <div class="flex items-center justify-between mb-1">
                      <span class="text-sm text-white truncate flex-1">{{ task.name }}</span>
                      <span class="text-xs text-gray-400 ml-2">{{ formatProgress(task) }}</span>
                    </div>
                    <div v-if="task.description" class="text-xs text-gray-500 truncate mb-2">{{ task.description }}</div>
                    <div class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
                      <div 
                        class="h-full bg-primary rounded-full transition-all duration-300"
                        :style="{ width: task.progress + '%' }"
                      ></div>
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
          
          <!-- Notifications Bell -->
          <button 
            @click="showNotifications = !showNotifications"
            class="relative p-2 rounded-lg hover:bg-surface-dark transition-colors text-text-secondary hover:text-white"
          >
            <span class="material-symbols-outlined">notifications</span>
            <span v-if="notificationCount > 0" class="absolute -top-0.5 -right-0.5 w-4 h-4 bg-red-500 rounded-full text-[10px] font-bold flex items-center justify-center text-white">
              {{ notificationCount > 9 ? '9+' : notificationCount }}
            </span>
          </button>
          
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
    <ToastNotifications />
    <CommandPalette v-if="showCommandPalette" @close="showCommandPalette = false" />
    <KeyboardShortcuts />
    <HelpPanel v-if="showHelp" @close="showHelp = false" />
    <QuickActionsFab :currentTab="currentTab" />
    <OnboardingWizard v-if="showOnboarding" @complete="showOnboarding = false" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute } from 'vue-router'

// Global Components
import SplashScreen from './components/SplashScreen.vue'
import StatusBar from './components/StatusBar.vue'
import ToastNotifications from './components/ToastNotifications.vue'
import CommandPalette from './components/CommandPalette.vue'
import KeyboardShortcuts from './components/KeyboardShortcuts.vue'
import HelpPanel from './components/HelpPanel.vue'
import QuickActionsFab from './components/QuickActionsFab.vue'
import OnboardingWizard from './components/OnboardingWizard.vue'

// Composables
import { useGlobalTasks } from './composables/useGlobalTasks'
import { useToast } from './composables/useToast'
import { useNotificationListener } from './composables/useNotificationListener'
import { listen } from '@tauri-apps/api/event'

const route = useRoute()
const toast = useToast()
const { startListening: startNotificationListening } = useNotificationListener()

// Global tasks state
const {
  activeTasks,
  hasActiveTasks,
  activeTaskCount,
  overallProgress,
  initEventListeners
} = useGlobalTasks()

// Global state
const showSplash = ref(true)
const showCommandPalette = ref(false)
const showNotifications = ref(false)
const showTasksDropdown = ref(false)
const showHelp = ref(false)
const showOnboarding = ref(false)
const notificationCount = ref(3)

// Current tab for contextual FAB actions
const currentTab = computed(() => {
  const path = route.path.replace('/', '')
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
  if (task.current !== undefined && task.total !== undefined) {
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
}

// Check if first-time user
onMounted(() => {
  // Initialize global task event listeners
  initEventListeners()
  startNotificationListening()

  // Listen for missing python dependencies
  listen('python_deps_missing', (event: any) => {
    toast.error(event.payload.message)
  })
  
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
  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
      e.preventDefault()
      showCommandPalette.value = true
    }
  })
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
