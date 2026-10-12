<template>
  <!-- Splash Screen -->
  <Transition name="fade">
    <SplashScreen
      v-if="showSplash"
      :error="splashError"
      :status-text="splashStatusText"
      :progress="splashProgress"
      @complete="handleSplashComplete"
      @ready="handleSplashComplete"
      @retry="retryInitialization"
    />
  </Transition>

  <!-- Main App -->
  <div class="bg-background-light dark:bg-background-dark text-white font-display overflow-hidden h-screen w-full flex flex-col">
      <!-- Header -->
      <header class="h-16 w-full bg-shell border-b border-border-dark flex items-center justify-between px-4 md:px-6 shrink-0 z-30 relative select-none" data-tauri-drag-region>
        <div class="flex items-center gap-2 md:gap-3">
          <!-- PL-01: hamburguesa, único acceso al menú lateral -->
          <button
            @click="toggleSidebarCollapsed"
            data-testid="nav-drawer-toggle"
            :aria-label="sidebarOverlay ? (sidebarOpenOnMobile ? 'Cerrar menú' : 'Abrir menú') : (sidebarCollapsed ? 'Expandir menú' : 'Colapsar menú')"
            :aria-expanded="sidebarOverlay ? sidebarOpenOnMobile : !sidebarCollapsed"
            aria-controls="app-nav-drawer"
            class="p-2 -ml-1 rounded-lg hover:bg-surface-dark transition-colors text-text-secondary hover:text-white"
          >
            <span class="material-symbols-outlined">menu</span>
          </button>
          <div class="w-8 h-8 bg-gradient-to-br from-primary to-purple-600 rounded-lg flex items-center justify-center shadow-lg shadow-primary/20">
            <span class="material-symbols-outlined text-white text-xl">all_inclusive</span>
          </div>
          <span class="text-lg font-bold tracking-tight text-white">Syncify</span>
        </div>

        <div class="flex items-center gap-3">
          <!-- Search Button: abre el modal de búsqueda (Ctrl+F / Ctrl+K) -->
          <button
            @click="openSearchModal"
            data-tour="search"
            class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-dark/50 border border-border-dark/50 hover:bg-surface-dark transition-colors cursor-pointer text-text-secondary hover:text-white"
          >
            <span class="material-symbols-outlined text-lg">search</span>
            <span class="text-xs hidden md:inline">Search...</span>
            <kbd class="hidden md:inline px-1.5 py-0.5 bg-surface-dark rounded text-xs text-gray-500">⌘F</kbd>
          </button>


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
                    @click="notification.actionLabel ? runHistoryAction(notification.id) : markAsRead(notification.id)"
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
        </div>

        <!-- Barra de progreso global (G-03): es `absolute` dentro de la
             cabecera, y la cabecera es la única caja de arriba a abajo, así que
             `left-0 right-0` == ancho completo de la ventana. El detalle de lo
             que se está haciendo ya NO va aquí (G-04/BP-01: fuera la píldora con
             el nombre de la tarea dominante); vive en "Background Processes", en
             la barra inferior (StatusBar.vue). -->
        <div class="absolute bottom-0 left-0 right-0 h-[2px] bg-surface-dark">
          <div
            v-if="gpHasActive && gpIndeterminate"
            class="h-full bg-primary shadow-[0_0_8px_rgba(60,131,246,0.6)] rounded-r-full animate-loading-bar"
            data-testid="global-progress-indeterminate"
          ></div>
          <div
            v-else-if="gpHasActive"
            class="h-full bg-primary shadow-[0_0_8px_rgba(60,131,246,0.6)] rounded-r-full"
            :style="{ width: gpProgress + '%' }"
            data-testid="global-progress-bar"
          ></div>
        </div>
      </header>

    <div class="flex flex-1 min-h-0 min-w-0 overflow-hidden">
    <!-- Barra lateral clasica de escritorio (PL-01, decision del usuario 9-oct-2026).
         Hermana flex de <main>, bajo la cabecera de ancho completo (G-03). Ya NO es
         un drawer modal: no hay backdrop que oscurezca la app al navegar. En pantallas
         estrechas se comporta como overlay, pero en `lg:` queda fija y siempre visible.
         Colapsa a solo iconos (w-16) guardando la preferencia en localStorage.
         El unico acceso para colapsar/expandir es la hamburguesa de la cabecera
         (peticion del usuario 12-oct-2026): la franja superior con el boton de
         colapsar se elimino, asi que la navegacion arranca arriba del todo y el
         lateral ya no reserva esos 64px. -->
    <aside
      v-if="sidebarVisible"
      id="app-nav-drawer"
      data-testid="nav-drawer"
      aria-label="Main navigation"
      :class="[
        'bg-shell border-r border-border-dark flex flex-col shrink-0 h-full',
        'fixed lg:static top-16 bottom-0 left-0 z-[180] lg:z-auto',
        sidebarCollapsed ? 'w-16' : 'w-64 max-w-[85vw]',
        sidebarOverlay && 'shadow-2xl'
      ]"
    >
      <nav
        class="flex-1 overflow-y-auto overflow-x-hidden p-2 space-y-1 flex flex-col"
        :class="sidebarCollapsed ? 'items-center' : ''"
        aria-label="Vistas"
      >
        <router-link
          to="/dashboard"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Dashboard' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/dashboard' }">dashboard</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Dashboard</span>
        </router-link>

        <router-link
          to="/library"
          data-tour="library"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Library' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/library' }">library_music</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Library</span>
        </router-link>

        <router-link
          to="/playlists"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Playlists' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/playlists' }">queue_music</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Playlists</span>
        </router-link>

        <router-link
          to="/downloads"
          data-tour="downloads"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Downloads' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/downloads' }">download</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Downloads</span>
        </router-link>

        <router-link
          to="/migration"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Migrate' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/migration' }">sync_alt</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Migrate</span>
        </router-link>

        <router-link
          to="/accounts"
          data-tour="accounts"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Accounts' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/accounts' }">manage_accounts</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Accounts</span>
        </router-link>

        <router-link
          to="/metadata"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Metadata' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/metadata' }">edit_note</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Metadata</span>
        </router-link>

        <router-link
          to="/lyrics"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Lyrics' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/lyrics' }">lyrics</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Lyrics</span>
        </router-link>

        <router-link
          to="/logs"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Logs' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/logs' }">terminal</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Logs</span>
        </router-link>

        <div class="mt-auto"></div>
        <div class="my-2 border-t border-border-dark opacity-50" :class="sidebarCollapsed ? 'w-10' : 'w-full'"></div>

        <!-- UI-CORR-4 (10-oct-2026): Settings estaba en la barra inferior
             (StatusBar), donde competía con Background Processes, Online y
             Storage: en cuanto el mini-reproductor aparecía, el botón se
             comprimía a solo icono. Vuelve al lateral, con texto cuando está
             expandida (de fácil acceso, se lee) y con `title` cuando está
             colapsada (icono + tooltip accesible, igual que el resto de
             enlaces). Queda justo encima de "Now Playing", a la izquierda. -->
        <router-link
          to="/settings"
          data-testid="nav-sidebar-settings"
          data-tour="settings"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group"
          active-class="bg-elevated !text-white"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          :title="sidebarCollapsed ? 'Settings' : undefined"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0" :class="{ 'fill-1 text-primary': routePath === '/settings' }">settings</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium whitespace-nowrap">Settings</span>
        </router-link>

        <button
          @click="openFullscreenPlayer"
          data-testid="nav-fullscreen-player"
          class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-text-secondary hover:bg-surface-dark hover:text-white transition-colors group text-left"
          :class="sidebarCollapsed ? 'justify-center w-10' : ''"
          title="Now Playing"
        >
          <span class="material-symbols-outlined group-hover:text-primary transition-colors shrink-0">album</span>
          <span v-if="!sidebarCollapsed" class="text-sm font-medium">Now Playing</span>
        </button>
      </nav>
    </aside>
    <main class="flex-1 min-w-0 min-h-0 flex flex-col overflow-hidden relative">

      <!-- Page Content. CONTRATO DE SCROLL (ola 3, virtualización):
           este NO es el elemento que hace scroll — es `overflow-hidden` a
           propósito. Cada vista trae su propio scroller (`LibraryView.vue:519`,
           `:685`, `:752`, `:802`; `MetadataView.vue:144`;
           `LyricsView.vue:181`), y es ahí donde la virtualización debe
           engancharse. Lo que este contenedor garantiza es que la vista
           ocupe el alto disponible entre cabecera y barra inferior sin pelear
           con el shell.

           UI-CORR-1 (10-oct-2026): `pb-8` reservaba SOLO 32px, pero la barra
           inferior creció a alto real (StatusBar.vue: `min-h-12` + `h-auto`)
           para que el mini-reproductor cupiera: el resultado era que la barra
           tapaba el reproductor y el final de cada vista. Ahora el reserva es
           el alto MEDIDO de la barra, que StatusBar publica en
           `--status-bar-h` al montar y en cada resize; `48px` es el mínimo de
           respaldo para el primer frame y para jsdom. El `+ 8px` es el mismo
           colchón visual de antes.
           `#page-content` + `data-testid="view-scroll-host"` son el ancla
           estable para calcular alturas, NO el scroller de cada vista. -->
      <div
        id="page-content"
        data-testid="view-scroll-host"
        class="flex-1 min-h-0 overflow-hidden relative"
        style="padding-bottom: calc(var(--status-bar-h, 48px) + 8px)"
      >
        <router-view v-slot="{ Component }">
          <transition name="fade" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </div>

      <!-- Status Bar -->
      <StatusBar />
    </main>
    </div>
    <!-- Global Components -->
    <ToastNotifications />
    <CommandPalette v-model="showCommandPalette" @close="showCommandPalette = false" />
    <SearchView v-if="showSearchModal" @close="closeSearchModal" />
    <KeyboardShortcuts @search="openSearchModal" @command-palette="showCommandPalette = true" />
    <OnboardingWizard
      v-if="showOnboarding"
      @complete="handleOnboardingComplete"
      @skip="handleOnboardingSkip"
    />
  </div>
</template>


<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

// Global Components
import SplashScreen from './components/SplashScreen.vue'
import StatusBar from './components/StatusBar.vue'
import ToastNotifications from './components/ToastNotifications.vue'
import CommandPalette from './components/CommandPalette.vue'
import KeyboardShortcuts from './components/KeyboardShortcuts.vue'
import SearchView from './views/SearchView.vue'
import OnboardingWizard from './components/OnboardingWizard.vue'

// Composables & APIs
import { useGlobalTasks } from './composables/useGlobalTasks'
import { useToast } from './composables/useToast'
import { useNotificationListener, NAVIGATE_EVENT } from './composables/useNotificationListener'
import { useLogs } from './composables/useLogs'
import { usePlayer } from './composables/usePlayer'
import { applyTintFromCover, resetTint } from './composables/useTint'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { TauriEvents } from './api/tauri'
import { accountsApi } from './api/accounts'
import { runHealthCheck, getDownloadSettings } from './api/settings'
import { trayApi } from './api/tray'

const route = useRoute()

// `route` puede ser undefined si el componente se monta sin router (tests del
// cascarón). Derivar el path aquí evita que la plantilla rompa al renderizar.
const routePath = computed(() => route?.path ?? '')
const router = useRouter()
const toast = useToast()
const { unreadCount, history: notificationHistory, markAsRead, markAllAsRead, clearAllHistory, runHistoryAction } = toast
const { startListening: startNotificationListening, stopListening: stopNotificationListening } = useNotificationListener()
const { initLogListeners } = useLogs()
const { current } = usePlayer()

// Tinte Camaleón: publica el matiz de la carátula de la pista en reproducción
// en --tint-h/--tint-k (los tokens del tema camaleon los consumen). `current`
// es el singleton compartido de usePlayer; sin pista (o sin carátula) el tinte
// vuelve a su valor por defecto.
watch(
  () => current.value?.coverUrl ?? null,
  (coverUrl) => {
    if (coverUrl) {
      void applyTintFromCover(coverUrl)
    } else {
      resetTint()
    }
  },
  { immediate: true },
)

let unlistenPythonDeps: UnlistenFn | null = null
let unlistenTrayOpenSettings: UnlistenFn | null = null

// BP-02: la barra de arriba es SOLO de procesos en segundo plano. El
// composable `useGlobalProgressBar` suma TODAS las tareas (`:107-108`), y no
// está en los ficheros de este carril; se deriva aquí el avance ponderado de
// las tareas de fondo sin incluir descargas ni operaciones síncronas.
// `scan` se inicia/espera en AccountsView.vue:1359-1396, y `import` también
// se crea desde llamadas UI (`useGlobalTasks.ts:461-470`): ninguno es un
// proceso en segundo plano del shell. Esta política de tipos coincide con la
// del panel inferior en `StatusBar.vue`.
const { activeTasks, initEventListeners } = useGlobalTasks()
const BACKGROUND_TASK_TYPES = new Set(['sync', 'metadata', 'lyrics'])
const backgroundTasks = computed(() => activeTasks.value.filter(t => BACKGROUND_TASK_TYPES.has(t.type)))
const gpHasActive = computed(() => backgroundTasks.value.length > 0)
const gpIndeterminate = computed(() => backgroundTasks.value.every(t => !t.total || t.total <= 0))
const gpProgress = computed(() => {
  const tasks = backgroundTasks.value
  const totals = tasks.map(t => t.total ?? 0).filter(total => total > 0)
  const fallback = totals.length ? totals.reduce((sum, n) => sum + n, 0) / totals.length : 1
  let weighted = 0
  let weightSum = 0
  for (const task of tasks) {
    const weight = (task.total ?? 0) > 0 ? task.total! : fallback
    weighted += Math.min(100, Math.max(0, task.progress ?? 0)) * weight
    weightSum += weight
  }
  return weightSum ? Math.round(weighted / weightSum) : 0
})

// Global state
const showSplash = ref(true)
const splashError = ref<string | null>(null)
const splashStatusText = ref('Initializing...')
const splashProgress = ref(15)
const showCommandPalette = ref(false)
const showSearchModal = ref(false)
const showNotifications = ref(false)
const showOnboarding = ref(false)
const historyFilter = ref<'all' | 'unread'>('all')

// --- Barra lateral (PL-01) ------------------------------------------------
// Durante el onboarding el sidebar debe existir: el tour busca sus objetivos con
// `document.querySelector` sobre `[data-tour]` (`OnboardingWizard.vue:606`) y,
// si el elemento no existe, se salta ese paso. `sidebarVisible` lo cubre.

// Barra lateral clasica: siempre visible en escritorio, colapsable a iconos.
// En pantallas estrechas se monta como overlay y empieza cerrada (sidebarOverlay),
// asi que durante el onboarding se fuerza visible. El estado colapsado se persiste.
const SIDEBAR_COLLAPSED_KEY = 'syncify.sidebarCollapsed.v1'
// jsdom (vitest) no implementa matchMedia; se trata como escritorio ancho.
const isNarrowViewport = () => {
  try {
    return window.matchMedia?.('(max-width: 1023px)').matches ?? false
  } catch {
    return false
  }
}

const sidebarCollapsed = ref(false)
const sidebarOverlay = ref(false)
const sidebarOpenOnMobile = ref(false)

function readCollapsedPreference(): boolean {
  try {
    return window.localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === 'true'
  } catch {
    return false
  }
}

function toggleSidebarCollapsed() {
  if (sidebarOverlay.value) {
    sidebarOpenOnMobile.value = !sidebarOpenOnMobile.value
    return
  }
  sidebarCollapsed.value = !sidebarCollapsed.value
  try {
    window.localStorage.setItem(SIDEBAR_COLLAPSED_KEY, String(sidebarCollapsed.value))
  } catch {
    /* modo privado: la preferencia solo vive en memoria */
  }
}

function syncViewportMode() {
  const narrow = isNarrowViewport()
  sidebarOverlay.value = narrow
  if (narrow) {
    sidebarCollapsed.value = false
    sidebarOpenOnMobile.value = false
  } else {
    sidebarCollapsed.value = readCollapsedPreference()
  }
}

function handleViewportResize() {
  syncViewportMode()
}

const sidebarVisible = computed(() => {
  if (sidebarOverlay.value) return sidebarOpenOnMobile.value || showOnboarding.value
  return true
})

/**
 * PL-10: el reproductor de pantalla completa es PL-09/PL-10, carril
 * `7-reproductor`, y todavía no existe (grep `fullscreen` en `ui/src` = 0 fuera
 * de iconos de Material). Lo que sí es de este carril es el menú lateral: que
 * exista y llegue hasta ahí. Se emite un `CustomEvent` de `window` como punto
 * de enganche; si el componente de la ola 7 no aparece, el enlace es inerte
 * pero no rompe nada.
 */
function openFullscreenPlayer() {
  if (sidebarOverlay.value) sidebarOpenOnMobile.value = false
  window.dispatchEvent(new CustomEvent('syncify:open-fullscreen-player'))
}

const filteredNotifications = computed(() => {
  if (historyFilter.value === 'unread') {
    return notificationHistory.value.filter(n => !n.read)
  }
  return notificationHistory.value
})

// Cerrar al pulsar fuera: notificaciones de la campana y desplegable de
// tareas de la cabecera. La barra global solo cubre procesos en segundo
// plano; el desplegable lista TODAS las tareas activas con su progreso.
function onNotificationNavigate(e: Event) {
  const destination = (e as CustomEvent<{ route?: string }>).detail?.route
  if (destination) void router.push(destination)
}

function handleOutsideClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (!target.closest('.notification-dropdown') && !target.closest('.notification-button')) {
    showNotifications.value = false
  }
}

// Global keydown listener (e.g. Ctrl+K)
function handleKeydown(e: KeyboardEvent) {
  if (e.defaultPrevented) return
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault()
    showCommandPalette.value = true
    return
  }
  if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
    e.preventDefault()
    openSearchModal()
    return
  }
  // En pantallas estrechas Escape cierra la navegación superpuesta.
  if (e.key === 'Escape' && sidebarOverlay.value && sidebarOpenOnMobile.value) {
    sidebarOpenOnMobile.value = false
  }
}

// En pantallas estrechas, navegar cierra el sidebar superpuesto;
// en escritorio la barra lateral permanece siempre visible.
watch(() => route?.path, () => {
  if (sidebarOverlay.value) sidebarOpenOnMobile.value = false
})

function openSearchModal() {
  showSearchModal.value = true
}

function closeSearchModal() {
  showSearchModal.value = false
}

const ONBOARDING_COMPLETED_KEY = 'syncify_onboarding_completed'
const ONBOARDING_COMPLETE_KEY = 'syncify_onboarding_complete'

function handleOnboardingComplete() {
  localStorage.setItem(ONBOARDING_COMPLETED_KEY, 'true')
  localStorage.setItem(ONBOARDING_COMPLETE_KEY, 'true')
  showOnboarding.value = false
}

function handleOnboardingSkip() {
  localStorage.setItem(ONBOARDING_COMPLETED_KEY, 'true')
  localStorage.setItem(ONBOARDING_COMPLETE_KEY, 'true')
  showOnboarding.value = false
}

async function initializeApp() {
  splashError.value = null
  splashProgress.value = 15
  splashStatusText.value = 'Initializing system listeners...'

  try {
    // 1. Initialize global task, notification, and log listeners
    initEventListeners()
    startNotificationListening()
    initLogListeners()
    try {
      const preferences = await trayApi.getTraySettings()
      toast.applyTrayPreferences({
        notify_download_complete: preferences.notifyDownloadComplete,
        notify_sync_complete: preferences.notifySyncComplete,
        notify_updates: preferences.notifyUpdates,
      })
    } catch (error) {
      console.warn('Could not load notification preferences:', error)
    }

    // 2. Database and system health check
    splashProgress.value = 45
    splashStatusText.value = 'Checking database...'

    try {
      const health = await runHealthCheck()
      if (health && typeof health.database_ok === 'boolean' && !health.database_ok) {
        const detail = (health.errors && health.errors.length > 0)
          ? health.errors.join('; ')
          : 'Database health check failed'
        throw new Error(`Database error: ${detail}`)
      }
    } catch (err: any) {
      if (err?.message && err.message.toLowerCase().includes('database')) {
        throw err
      }
      console.warn('[App] Health check non-fatal note:', err)
    }

    // 3. Load services and download configuration
    splashProgress.value = 75
    splashStatusText.value = 'Loading services and configuration...'

    try {
      await Promise.all([
        accountsApi.getAccounts(),
        getDownloadSettings(),
      ])
    } catch (err) {
      console.warn('[App] Services/settings load non-fatal note:', err)
    }

    // 4. Finalizing
    splashProgress.value = 100
    splashStatusText.value = 'Ready!'

    // Smooth transition
    showSplash.value = false

    // Check if onboarding needed (no services connected)
    const hasCompletedOnboarding =
      localStorage.getItem(ONBOARDING_COMPLETED_KEY) === 'true' ||
      localStorage.getItem(ONBOARDING_COMPLETE_KEY) === 'true'
    if (!hasCompletedOnboarding) {
      setTimeout(() => {
        showOnboarding.value = true
      }, 500)
    }
  } catch (err: any) {
    console.error('[App] Critical initialization failure:', err)
    splashError.value = err?.message || 'Failed to initialize Syncify database and services.'
    splashStatusText.value = 'Initialization error'
  }
}

function retryInitialization() {
  splashError.value = null
  showSplash.value = true
  initializeApp()
}

function handleSplashComplete() {
  showSplash.value = false
}

// Check if first-time user
onMounted(async () => {
  // Inicializar el modo del sidebar (fijo en escritorio / overlay en movil)
  syncViewportMode()
  window.addEventListener('resize', handleViewportResize)
  window.addEventListener(NAVIGATE_EVENT, onNotificationNavigate)

  // Listen for missing python dependencies
  try {
    unlistenPythonDeps = await listen(TauriEvents.PYTHON_DEPS_MISSING, (event: any) => {
      toast.error(event.payload.message)
    })
  } catch (err) {
    console.warn('Failed to listen for python_deps_missing:', err)
  }

  // Ítem 48: el menú de bandeja pide abrir Ajustes. El backend emite el evento
  // con este nombre EXACTO; aquí solo se escucha y se navega.
  try {
    unlistenTrayOpenSettings = await listen(TauriEvents.TRAY_OPEN_SETTINGS, () => {
      void router.push('/settings')
    })
  } catch (err) {
    console.warn('Failed to listen for tray-open-settings:', err)
  }

  // Add outside click handler
  document.addEventListener('click', handleOutsideClick)

  // Listen for Ctrl+K
  document.addEventListener('keydown', handleKeydown)

  // Start reactive initialization cycle
  await initializeApp()
})

onUnmounted(() => {
  unlistenPythonDeps?.()
  unlistenPythonDeps = null
  unlistenTrayOpenSettings?.()
  unlistenTrayOpenSettings = null
  stopNotificationListening()
  window.removeEventListener('resize', handleViewportResize)
  window.removeEventListener(NAVIGATE_EVENT, onNotificationNavigate)
  document.removeEventListener('click', handleOutsideClick)
  document.removeEventListener('keydown', handleKeydown)
})

defineExpose({
  showSplash,
  splashError,
  splashStatusText,
  splashProgress,
  initializeApp,
  retryInitialization,
  showOnboarding,
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
