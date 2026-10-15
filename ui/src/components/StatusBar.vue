<template>
  <!-- PL-06: la barra inferior ya no se colapsa. Antes era `v-if="!isCollapsed"`
       con un botón flotante para recuperarla; el usuario pidió que no exista
       forma de colapsarla, así que desaparece también la persistencia de
       `syncify_statusbar_collapsed`: dejarla habría dejado a quien ya la
       tuviera colapsada sin barra y sin botón para volver.

       UI-CORR-1 (10-oct-2026): alto REAL y flexible. Antes era `h-8` fijo, y
       el mini-reproductor central (PL-08) pedía más de 32px: su barra de
       progreso y sus controles quedaban recortados por la propia barra. Ahora
       la altura mínima es `min-h-12` (48px) y crece con el contenido
       (`h-auto`), sin techo que recorte. `App.vue` reserva exactamente ese
       alto real mediante `--status-bar-h` (ver STATUS_BAR_MIN_HEIGHT_PX) en
       `#page-content`, así el padding del layout no depende de una constante
       hardcodeada. -->
  <div
    ref="statusBarEl"
    :class="[
      'status-bar fixed bottom-0 left-0 right-0 h-auto min-h-12 bg-shell border-t border-line',
      'flex items-center gap-x-3 gap-y-1 flex-wrap px-3 py-1 text-xs',
      overlayActive ? 'z-0' : (showSyncPopover || showNetworkPopover || showStoragePopover) ? 'z-[160]' : 'z-[50]'
    ]"
  >
    <!-- Left Section: Background Processes (BP-02/BP-04).
         No anuncia descargas activas: ésas se consultan en DownloadsView. -->
    <button
      class="status-section sync-status flex items-center gap-2 px-3 py-1 rounded hover:bg-elevated cursor-pointer transition-colors shrink-0"
      @click="toggleSyncPopover"
      :title="syncTooltip"
      aria-label="Background Processes"
    >
      <template v-if="syncState === 'syncing'">
        <span class="material-symbols-outlined text-info text-sm animate-spin">sync</span>
        <span class="text-info">
          <template v-if="backgroundTasks.length > 1">{{ backgroundTasks.length }} background processes</template>
          <template v-else>{{ syncService }}...</template>
        </span>
        <span v-if="backgroundHasDeterminateProgress" class="text-info/80">{{ syncProgress }}%</span>
      </template>

      <template v-else-if="syncState === 'idle'">
        <span class="material-symbols-outlined text-ok text-sm">check_circle</span>
        <span class="text-ink-2">Background Processes</span>
      </template>

      <template v-else-if="syncState === 'error'">
        <span class="material-symbols-outlined text-error text-sm">warning</span>
        <span class="text-error">Background process failed</span>
      </template>

      <template v-else-if="syncState === 'paused'">
        <span class="material-symbols-outlined text-warn text-sm">pause_circle</span>
        <span class="text-warn">Background process paused</span>
      </template>
    </button>

    
    <!-- Background Processes Popover (BP-02/BP-04)
         Renombrado desde "Sync Details": este panel ya no describe UNA
         sincronización, describe el conjunto de procesos en SEGUNDO PLANO
         (sincronización, enriquecimiento de metadatos, letras). Por eso filtra
         `download` (primer plano en DownloadsView) y `scan` / `import`
         iniciadas sincrónicamente desde la UI (AccountsView.vue:1359-1396). -->
    <Transition name="slide-up">
      <div v-if="showSyncPopover" :class="['status-popover absolute z-[160] left-3 w-64 bg-surface border border-line rounded-xl shadow-xl p-4', current ? 'bottom-26' : 'bottom-10']">
        <div class="flex items-center justify-between mb-3">
          <span class="font-medium text-ink">Background Processes</span>
          <button @click="showSyncPopover = false" class="p-1 hover:bg-elevated rounded">
            <span class="material-symbols-outlined text-ink-2 text-sm">close</span>
          </button>
        </div>

        <div v-if="taskProgressRows.length > 0" class="space-y-2">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-info text-lg animate-spin">sync</span>
            <span class="text-ink-2">{{ syncService }}</span>
          </div>
          <div v-if="taskProgressRows.length > 1" class="text-xs text-ink-3">
            {{ taskProgressRows.length }} tasks in progress
          </div>
          <div v-for="row in taskProgressRows" :key="row.id" class="space-y-1">
            <div class="flex items-center justify-between gap-2">
              <span class="text-ink-2 text-xs truncate">{{ row.name }}</span>
              <span class="text-ink-2 text-[11px] font-mono shrink-0">
                <template v-if="row.determinate">{{ row.detail }} · {{ Math.round(row.progress) }}%</template>
                <template v-else>{{ row.detail }}</template>
              </span>
            </div>
            <div v-if="row.determinate" class="h-1.5 bg-elevated rounded-full overflow-hidden">
              <div class="h-full bg-info rounded-full transition-all" :style="{ width: row.progress + '%' }"></div>
            </div>
            <div v-else class="h-1.5 bg-elevated rounded-full overflow-hidden">
              <div class="h-full w-1/3 bg-info/60 rounded-full animate-pulse"></div>
            </div>
          </div>
          <div v-if="syncETA" class="text-xs text-ink-3">ETA: ~{{ syncETA }}</div>
          <button v-if="runningSyncTask" @click="goToDownloads" class="w-full mt-2 py-1.5 bg-accent-soft text-accent hover:bg-accent/30 rounded-lg text-sm">
            Manage Sync in Downloads
          </button>
        </div>

        <div v-else-if="syncState === 'error'" class="text-sm">
          <p class="text-error mb-2">{{ syncErrorMessage }}</p>
          <button @click="retrySync" class="w-full py-1.5 bg-accent-soft text-accent hover:bg-accent/30 rounded-lg text-sm">
            Retry Sync
          </button>
        </div>

        <div v-else-if="syncState === 'paused'" class="text-sm">
          <p class="text-warn mb-2 flex items-center gap-2">
            <span class="material-symbols-outlined">pause_circle</span>
            Background process paused
          </p>
          <button @click="goToDownloads" class="w-full py-1.5 bg-accent-soft text-accent hover:bg-accent/30 rounded-lg text-sm">
            Manage in Downloads
          </button>
        </div>

        <!-- Sin procesos en segundo plano: se dice, en vez de abrir un panel
             vacío o de enseñar el estado de una descarga que no corresponde. -->
        <div v-else class="text-sm text-ink-2">
          <p>No background processes running.</p>
          <p class="text-xs text-ink-3 mt-1">Last synced: {{ lastSyncTime }}</p>
          <button @click="syncAll" class="w-full mt-3 py-1.5 bg-accent-soft text-accent hover:bg-accent/30 rounded-lg text-sm">
Open Accounts to Sync
          </button>
        </div>
      </div>
    </Transition>


    <!-- Center Section: Network & Operations
         BP-03: aquí vivía una TERCERA barra de progreso ("Global Progress:
         covers every active task at once") con el nombre de la tarea activa.
         Se elimina. La barra global es la de 2 px de la cabecera (App.vue) y
         el detalle por proceso vive en "Background Processes", arriba. -->
    <div class="flex items-center justify-center gap-6">
      <!-- UI-CORR-2 (10-oct-2026): el botón "Online" está a la izquierda, pero
           su popover se centraba en la barra (`left-1/2 -translate-x-1/2`),
           así que volaba hasta 300px lejos del texto que el usuario había
           pulsado. Ahora el botón es el propio ancla: vive en un contenedor
           `relative` y el popover se posiciona respecto a él
           (`left-1/2 -translate-x-1/2` + `bottom-full`), así queda centrado
           SOBRE el botón. En pantalla estrecha el panel se sale por la
           izquierda/derecha, así que se clampea al viewport con `left-0
           right-auto` + `max-w` (ver networkPopoverStyle). El `relative` crea
           el contexto de posicionamiento sin tocar el eje Z: la barra sigue
           subiendo su stacking (z-[160] al abrir un popover), y el panel
           hereda ese nivel en vez de crear uno nuevo. -->
      <div class="relative">
        <div
          class="status-section network-status flex items-center gap-2 px-3 py-1 rounded hover:bg-elevated cursor-pointer transition-colors"
          @click="toggleNetworkPopover"
          data-testid="status-bar-network"
          title="Network status"
          role="button"
          tabindex="0"
          aria-haspopup="dialog"
          :aria-expanded="showNetworkPopover"
        >
          <span :class="['w-2 h-2 rounded-full', isOnline ? 'bg-ok' : 'bg-error']"></span>
          <span :class="isOnline ? 'text-ink-2' : 'text-error'">{{ isOnline ? 'Online' : 'Offline' }}</span>
        </div>

        <!-- Network Popover: anclado al botón Online (ver comentario arriba). -->
        <Transition name="slide-up">
          <div v-if="showNetworkPopover" :style="networkPopoverStyle()" class="status-popover absolute z-[160] w-72 max-w-[calc(100vw-1.5rem)] bg-surface border border-line rounded-xl shadow-xl p-4">
            <div class="flex items-center justify-between mb-3">
              <span class="font-medium text-ink">Network Status</span>
              <button @click="showNetworkPopover = false" class="p-1 hover:bg-elevated rounded">
                <span class="material-symbols-outlined text-ink-2 text-sm">close</span>
              </button>
            </div>

            <div class="space-y-3 text-sm">
              <div class="flex items-center justify-between">
                <span class="text-ink-2">Download Speed</span>
                <span class="text-ink">{{ downloadSpeed }}</span>
              </div>

              <div class="border-t border-line pt-3 mt-3">
                <p class="text-ink-2 mb-2">Services</p>
                <div class="space-y-1.5">
                  <div v-for="service in services" :key="service.name" class="flex items-center justify-between">
                    <span class="text-ink-2">{{ service.name }}</span>
                    <span v-if="service.online" class="text-ok flex items-center gap-1">
                      <span class="material-symbols-outlined text-sm">check</span>
                      Connected
                    </span>
                    <span v-else class="text-error flex items-center gap-1">
                      <span class="material-symbols-outlined text-sm">close</span>
                      Offline
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Transition>
      </div>
    </div>
    
    <!-- PL-08: Centro de la barra de estado — Pista en curso y mini reproductor -->
    <div
      v-if="current"
      class="status-center-player flex-1 flex items-center justify-center gap-2.5 min-w-0 mx-2 text-xs"
      data-testid="status-bar-player"
    >
      <!-- PL-09: Portada con señal al hacer hover y click para expandir -->
      <button
        @click="openFullscreen"
        data-testid="status-bar-track-cover"
        class="group relative w-6 h-6 rounded overflow-hidden shrink-0 cursor-pointer focus:outline-none focus:ring-1 focus:ring-accent"
        title="Expandir reproductor (Pantalla completa)"
        aria-label="Expandir reproductor (Pantalla completa)"
      >
        <TrackCover
          :src="current.coverUrl"
          :item-id="current.id"
          :alt="current.album ?? current.title"
          size-class="w-6 h-6 text-[10px]"
          rounded="rounded"
        />
        <!-- Señal visual de hover: overlay con icono expandir -->
        <div
          class="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center text-white"
          data-testid="cover-expand-hint"
        >
          <span class="material-symbols-outlined text-[14px]">open_in_full</span>
        </div>
      </button>

      <!-- Título y artista (clicable para expandir) -->
      <button
        @click="openFullscreen"
        class="flex items-center gap-1.5 min-w-0 max-w-[200px] sm:max-w-[280px] truncate text-left cursor-pointer hover:text-accent transition-colors focus:outline-none"
        title="Expandir reproductor (Pantalla completa)"
        data-testid="status-bar-track-info"
      >
        <span class="font-medium text-ink truncate">{{ current.title }}</span>
        <span class="text-ink-3 shrink-0">·</span>
        <span class="text-text-secondary truncate text-[11px]">{{ current.artist }}</span>
      </button>

      <!-- Mini transporte: prev, play/pause, next -->
      <div class="flex items-center gap-0.5 shrink-0">
        <button
          @click="previous"
          class="p-0.5 rounded hover:bg-elevated text-ink-2 hover:text-ink transition-colors"
          title="Pista anterior"
          aria-label="Pista anterior"
          data-testid="status-bar-prev"
        >
          <span class="material-symbols-outlined text-sm">skip_previous</span>
        </button>
        <button
          @click="toggle"
          class="p-0.5 rounded hover:bg-elevated text-ink hover:text-accent transition-colors"
          :title="isPlaying ? 'Pausa' : 'Reproducir'"
          :aria-label="isPlaying ? 'Pausa' : 'Reproducir'"
          data-testid="status-bar-toggle"
        >
          <span class="material-symbols-outlined text-sm">{{ isPlaying ? 'pause' : 'play_arrow' }}</span>
        </button>
        <button
          @click="next"
          class="p-0.5 rounded hover:bg-elevated text-ink-2 hover:text-ink transition-colors"
          title="Pista siguiente"
          aria-label="Pista siguiente"
          data-testid="status-bar-next"
        >
          <span class="material-symbols-outlined text-sm">skip_next</span>
        </button>
      </div>

      <!-- Tiempo transcurrido / total -->
      <span
        v-if="durationSec > 0"
        class="font-mono text-[10px] text-ink-3 shrink-0 tabular-nums hidden sm:inline"
        data-testid="status-bar-time"
      >
        {{ formatTime(positionSec) }} / {{ formatTime(durationSec) }}
      </span>

      <!-- Botón explícito para expandir a pantalla completa -->
      <button
        @click="openFullscreen"
        class="p-0.5 rounded hover:bg-elevated text-ink-2 hover:text-ink transition-colors shrink-0"
        title="Pantalla completa"
        aria-label="Pantalla completa"
        data-testid="status-bar-expand-btn"
      >
        <span class="material-symbols-outlined text-sm">open_in_full</span>
      </button>
    </div>
    <!-- Reservar el centro libre cuando no hay reproducción -->
    <div v-else class="flex-1"></div>


    <!-- BP-03: Ajustes NO vive aquí. Con el mini-reproductor ocupando el
         centro, la barra inferior solo tiene sitio para Background Processes,
         Online/Network y Storage; Settings vuelve a la barra lateral
         (App.vue), donde hay sitio de sobra para texto + icono. El botón y
         `goToSettings()` se eliminan en vez de quedar huérfanos. -->

    <!-- Storage conserva la esquina inferior derecha y su popover anclado a
         right-3. -->
    <div
      class="status-section storage-info flex items-center gap-3 px-3 py-1 rounded hover:bg-elevated cursor-pointer transition-colors"
      @click="toggleStoragePopover"
      title="Storage usage"
    >
      <span class="material-symbols-outlined text-ink-2 text-sm">hard_drive_2</span>
      <span class="text-ink-2">{{ storageUsed }} / {{ storageTotal }}</span>
      
      <!-- Mini Progress Bar -->
      <div class="mini-progress-bar w-10 h-1 bg-elevated rounded-full overflow-hidden">
        <div 
          :class="['h-full rounded-full transition-all', storageProgressColor]"
          :style="{ width: storagePercent + '%' }"
        ></div>
      </div>
    </div>
    
    <!-- Storage Popover -->
    <Transition name="slide-up">
      <div v-if="showStoragePopover" :class="['status-popover absolute z-[160] right-3 w-72 bg-surface border border-line rounded-xl shadow-xl p-4', current ? 'bottom-26' : 'bottom-10']">
        <div class="flex items-center justify-between mb-3">
          <span class="font-medium text-ink">Storage Details</span>
          <button @click="showStoragePopover = false" class="p-1 hover:bg-elevated rounded">
            <span class="material-symbols-outlined text-ink-2 text-sm">close</span>
          </button>
        </div>
        
        <div class="space-y-3 text-sm">
          <div class="flex items-center justify-between">
            <span class="text-ink-2">Library Size</span>
            <span class="text-ink">{{ storageUsed }}</span>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-ink-2">Available Space</span>
            <span class="text-ink">{{ storageAvailable }}</span>
          </div>
          
          <div class="h-2 bg-elevated rounded-full overflow-hidden">
            <div 
              :class="['h-full rounded-full transition-all', storageProgressColor]"
              :style="{ width: storagePercent + '%' }"
            ></div>
          </div>
          
          <div class="border-t border-line pt-3 space-y-1.5">
            <p class="text-ink-2 mb-2">Breakdown</p>
            <div v-for="format in storageBreakdown" :key="format.name" class="flex items-center justify-between">
              <span class="text-ink-2">{{ format.name }}</span>
              <span class="text-ink-2">{{ format.size }} ({{ format.percent }}%)</span>
            </div>
          </div>
          
          <div class="pt-2">
            <p class="text-ink-3 text-xs truncate mb-2">{{ storagePath }}</p>
            <button @click="changeLocation" class="w-full py-1.5 bg-accent-soft text-accent hover:bg-accent/30 rounded-lg text-sm">
              Change Location
            </button>
          </div>
        </div>
      </div>
    </Transition>
    

  </div>

  <!-- PL-04, PL-05, PL-09, PL-10: Reproductor grande / pantalla completa -->
  <FullscreenPlayer />
</template>


<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useGlobalTasks } from '../composables/useGlobalTasks'
import { useEventBus, TauriEvents } from '../composables/useEventBus'
import { useToast } from '../composables/useToast'
import { usePlayer } from '../composables/usePlayer'
import { getStorageStats, type StorageStats } from '../api/storage'
import { getServiceStatuses } from '../api/accounts'
import { saveSetting } from '../api/settings'
import { open } from '@tauri-apps/plugin-dialog'
import TrackCover from './TrackCover.vue'
import FullscreenPlayer from './player/FullscreenPlayer.vue'
import {
  deriveSyncErrorMessage,
  estimateRemainingMs,
  formatDownloadSpeed,
  formatRelativeTime,
  formatRemainingDuration,
  isTerminalDownloadStatus,
  parseSqliteUtc,
} from '../utils/statusTelemetry'

// The footer must not cover shell-level modals when one is open.
withDefaults(defineProps<{ overlayActive?: boolean }>(), { overlayActive: false })

// State
const showSyncPopover = ref(false)
const showNetworkPopover = ref(false)
const showStoragePopover = ref(false)

/**
 * UI-CORR-2 (10-oct-2026): el popover de Network se ancla al botón "Online",
 * no al centro de la barra. El panel se coloca sobre el botón
 * (`bottom-full` respecto al contenedor `relative` que lo envuelve) y se
 * centra en él con `left: 50%; transform: translateX(-50%)`.
 *
 * Ese centrado desbordaría por la izquierda en ventanas estrechas (el botón
 * vive en el centro de la barra y el panel mide 288px). Se clampea al ancho
 * del viewport: si el panel no cabe, se limita su ancho al espacio disponible
 * a cada lado y se deja `left: 0` para que nazca alineado al botón en vez de
 * salirse de la pantalla. El cálculo se hace en px sobre el ancho real del
 * contenedor, medido en el cliente (imprescindible: en jsdom `getBoundingClientRect`
 * devuelve ceros y no hay CSS real, así que sin este guardia el estilo
 * computado saldría como `left: 0` sin clamp y perdería el anclaje).
 */
const statusBarEl = ref<HTMLElement | null>(null)

/**
 * UI-CORR-2: posición del popover de Network, recalculada en cada render
 * (función, no `computed`). La geometría de la ventana no es reactiva: si
 * fuera un `computed`, se cachearía la primera vez y el panel quedaría anclado
 * donde naciera, no donde está el botón tras un cambio de tamaño. Al ser una
 * función invocada desde el template, se re-evalúa en cada repintado del
 * componente, y `showNetworkPopover` sí fuerza uno nuevo al abrir y cerrar.
 */
function networkPopoverStyle(): Record<string, string> {
  // Aparece sobre el botón; el `mb-2` es la separación visual.
  const base: Record<string, string> = {
    position: 'absolute',
    bottom: '100%',
    left: '50%',
    transform: 'translateX(-50%)',
    'margin-bottom': '0.5rem',
  }
  const host = statusBarEl.value
  if (!host) return base
  const barRect = host.getBoundingClientRect()
  const anchorRect = host.querySelector('.network-status')?.getBoundingClientRect() ?? barRect
  // El panel mide 288px (w-72); se reserva la mitad para no salirse.
  const halfPanel = 288 / 2
  const roomLeft = anchorRect.left - barRect.left + (anchorRect.width / 2) - halfPanel
  const roomRight = barRect.right - (anchorRect.left + anchorRect.width / 2) - halfPanel
  if (roomLeft < 0) {
    // No cabe a la izquierda: se alinea al borde izquierdo de la barra.
    return { ...base, left: '0px', transform: 'none' }
  }
  if (roomRight < 0) {
    // No cabe a la derecha: se alinea al borde derecho de la barra.
    return { ...base, left: 'auto', right: '0px', transform: 'none' }
  }
  return base
}

// Global Tasks Integration
const { activeTasks, allTasks } = useGlobalTasks()
const eventBus = useEventBus()
const router = useRouter()
const toast = useToast()

const player = usePlayer()
const current = player.current ?? ref(null)
const isPlaying = player.isPlaying ?? ref(false)
const positionSec = player.positionSec ?? ref(0)
const durationSec = player.durationSec ?? ref(0)
const toggle = player.toggle ?? (() => {})
const next = player.next ?? (() => {})
const previous = player.previous ?? (() => {})
const openFullscreen = player.openFullscreen ?? (() => {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new CustomEvent('syncify:open-fullscreen-player'))
  }
})

function formatTime(sec: number): string {
  if (!Number.isFinite(sec) || sec <= 0) return '0:00'
  const m = Math.floor(sec / 60)
  const s = Math.floor(sec % 60)
  return `${m}:${s.toString().padStart(2, '0')}`
}

let storageInterval: ReturnType<typeof setInterval> | undefined
let etaInterval: ReturnType<typeof setInterval> | undefined
let unlistenDownloadProgress: (() => void) | undefined
const isRefreshing = ref(false)

// BP-02: este panel es de procesos EN SEGUNDO PLANO. Una descarga en curso es
// tarea en primer plano —el usuario la sigue en DownloadsView— y queda fuera.
// Lo mismo para `scan` (AccountsView.vue:1359-1396 lo inicia y espera a que
// termine) e `import` síncrona. Los tipos son los de `GlobalTask.type`
// (`useGlobalTasks.ts:15`); `sync`, `metadata`, `lyrics` sí son tareas que
// continúan por eventos de backend.
const BACKGROUND_TASK_TYPES = new Set(['sync', 'metadata', 'lyrics'])
const backgroundTasks = computed(() => activeTasks.value.filter(t => BACKGROUND_TASK_TYPES.has(t.type)))
const backgroundAllTasks = computed(() => allTasks.value.filter(t => BACKGROUND_TASK_TYPES.has(t.type)))

// `deriveSyncState` de statusTelemetry.ts:118 tiene la política antigua y
// cuenta cualquier tarea running (incluida download). Esa función queda
// intacta —es ajena al carril y tiene sus pruebas—, pero el indicador aquí
// aplica la política BP-02 únicamente a `backgroundAllTasks`.
type SyncState = 'syncing' | 'idle' | 'error' | 'paused'
const syncState = computed((): SyncState => {
  const tasks = backgroundAllTasks.value
  if (tasks.some(t => t.status === 'running')) return 'syncing'
  if (tasks.some(t => t.status === 'failed' || t.status === 'requires_auth')) return 'error'
  if (tasks.some(t => t.status === 'paused')) return 'paused'
  return 'idle'
})

const syncService = computed(() => {
  const task = backgroundTasks.value.find(t => t.type === 'sync') || backgroundTasks.value[0]
  if (!task) return 'Background Processes'
  return task.service || task.name.replace(/^Syncing\s+/, '') || 'Background Process'
})

// Ponderación por número de ítems; sin total, peso igual al promedio de los
// totales declarados (o 1 si no hay ninguno). Se mantiene la misma regla que
// la barra global de App.vue sin sumar los bytes de descargas.
const backgroundHasDeterminateProgress = computed(() => backgroundTasks.value.some(t => (t.total ?? 0) > 0))
const syncProgress = computed(() => {
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

// One row per task: summing `current`/`total` across tasks mixed a
// thousand-file scan with a three-track sync and produced meaningless counts.
interface TaskProgressRow {
  id: string
  name: string
  detail: string
  progress: number
  determinate: boolean
}

const taskProgressRows = computed<TaskProgressRow[]>(() =>
  backgroundTasks.value.map(task => ({
    id: task.id,
    name: task.name,
    detail: (task.total ?? 0) > 0
      ? `${task.current ?? 0} / ${task.total}`
      : task.description || task.phase || 'In progress',
    progress: Math.min(100, Math.max(0, task.progress ?? 0)),
    determinate: (task.total ?? 0) > 0
  }))
)

// Real ETA estimate: extrapolate the elapsed time of the primary running sync
// task. Hidden (null) while there is no basis to estimate from.
const runningSyncTask = computed(() => backgroundTasks.value.find(t => t.type === 'sync' && t.status === 'running'))
const nowMs = ref(Date.now())
const syncETA = computed<string | null>(() => {
  const task = runningSyncTask.value
  if (!task) return null
  const remaining = estimateRemainingMs(task.startedAt, task.progress ?? 0, nowMs.value)
  return remaining === null ? null : formatRemainingDuration(remaining)
})
watch(() => syncState.value === 'syncing', (syncing) => {
  if (syncing && etaInterval === undefined) {
    etaInterval = setInterval(() => { nowMs.value = Date.now() }, 1000)
  } else if (!syncing && etaInterval !== undefined) {
    clearInterval(etaInterval)
    etaInterval = undefined
  }
}, { immediate: true })

// Real error message from the most recent failed sync task (no invented text)
const syncErrorMessage = computed(() => deriveSyncErrorMessage(backgroundAllTasks.value) ?? 'Background process failed')

const syncTooltip = computed(() => {
  switch (syncState.value) {
    // BP-02: cuenta procesos en segundo plano, no descargas en curso.
    case 'syncing': return `Processing ${backgroundTasks.value.length} background processes: ${Math.round(syncProgress.value)}%`
    case 'idle': return 'No background processes running'
    case 'error': return 'Click to view background process error'
    case 'paused': return 'Background process paused'
    default: return ''
  }
})

// Network
const isOnline = ref(true)
// Real download speed from `syncify:download_progress` events (KB/s); null
// while no download is running so the popover shows an em dash instead of a
// made-up number.
const downloadSpeedKBps = ref<number | null>(null)
const downloadSpeed = computed(() => formatDownloadSpeed(downloadSpeedKBps.value))
const services = ref<Array<{ name: string; online: boolean; lastSynced: string | null }>>([])

// Last sync time from the real `last_synced` of the connected accounts
// (refreshed with the service statuses).
const lastSyncTime = computed(() => {
  const dates = services.value
    .map(s => (s.lastSynced ? parseSqliteUtc(s.lastSynced) : null))
    .filter((d): d is Date => d !== null)
  if (dates.length === 0) return 'Never'
  const latest = Math.max(...dates.map(d => d.getTime()))
  return formatRelativeTime(new Date(latest))
})

// Storage
const storageUsed = ref('0 B')
const storageTotal = ref('0 B')
const storageAvailable = ref('0 B')
const storagePercent = ref(0)
const storagePath = ref('Not set')
const storageBreakdown = ref<{ name: string; size: string; percent: number }[]>([])

// Format helper
function formatBytes(bytes: number, decimals = 2) {
  if (bytes === 0) return '0 B'
  const k = 1024
  const dm = decimals < 0 ? 0 : decimals
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i]
}

const storageProgressColor = computed(() => {
  if (storagePercent.value > 90) return 'bg-error'
  if (storagePercent.value > 70) return 'bg-warn'
  return 'bg-ok'
})

// Methods
function toggleSyncPopover() {
  showSyncPopover.value = !showSyncPopover.value
  showNetworkPopover.value = false
  showStoragePopover.value = false
  if (showSyncPopover.value) {
    // Refresh so "Last synced" reflects the real accounts state
    fetchServices()
  }
}

function toggleNetworkPopover() {
  showNetworkPopover.value = !showNetworkPopover.value
  showSyncPopover.value = false
  showStoragePopover.value = false
  if (showNetworkPopover.value) {
    fetchServices()
  }
}

function toggleStoragePopover() {
  showStoragePopover.value = !showStoragePopover.value
  showSyncPopover.value = false
  showNetworkPopover.value = false
  if (showStoragePopover.value) {
    fetchStorage()
  }
}

async function fetchStorage() {
  try {
    const stats = await getStorageStats()
    updateStorageUI(stats)
  } catch (e) {
    console.error('Failed to fetch storage stats:', e)
  }
}

function updateStorageUI(stats: StorageStats) {
  storageUsed.value = formatBytes(stats.used_bytes)
  storageTotal.value = formatBytes(stats.total_bytes)
  storageAvailable.value = formatBytes(stats.available_bytes)
  storagePath.value = stats.path
  
  if (stats.total_bytes > 0) {
    storagePercent.value = Math.round((stats.used_bytes / stats.total_bytes) * 100)
  } else {
    storagePercent.value = 0
  }

  storageBreakdown.value = stats.breakdown.map(b => ({
    name: b.format,
    size: formatBytes(b.size_bytes),
    percent: stats.used_bytes > 0 ? Math.round((b.size_bytes / stats.used_bytes) * 100) : 0
  }))
}

async function fetchServices() {
  try {
    const statuses = await getServiceStatuses()
    services.value = statuses.map(s => ({
      name: s.name,
      online: s.connected,
      lastSynced: s.last_synced ?? null,
    }))

    // Simple global online check: if at least one service is configured/online
    isOnline.value = services.value.some(s => s.online)
  } catch (e) {
    console.error('Failed to fetch service statuses:', e)
  }
}

// Refresh the services (and therefore "Last synced") once a whole run
// finishes, not per item: batches (downloads, syncs, enrichment) complete one
// task at a time, and a flip-per-item watch fired get_service_statuses every
// few seconds for as long as a batch ran.
let servicesRefreshTimer: ReturnType<typeof setTimeout> | undefined
function scheduleFetchServices(delay = 1500) {
  if (servicesRefreshTimer) clearTimeout(servicesRefreshTimer)
  servicesRefreshTimer = setTimeout(() => {
    servicesRefreshTimer = undefined
    fetchServices()
  }, delay)
}

watch(() => backgroundTasks.value.length, (count, prev) => {
  if (prev > 0 && count === 0) {
    scheduleFetchServices(1000)
  }
})

function syncAll() {
  showSyncPopover.value = false
  toast.info('Open Accounts to start a service sync')
  router.push('/accounts')
}

function retrySync() {
  showSyncPopover.value = false
  router.push('/downloads')
}

function goToDownloads() {
  showSyncPopover.value = false
  router.push('/downloads')
}

// UI-CORR-1 (10-oct-2026): alto mínimo REAL de la barra inferior, en px.
// Es la fuente única que consume `App.vue` para reservar el hueco de
// `#page-content` (`padding-bottom: calc(var(--status-bar-h, 48px) + 8px)`),
// de modo que el layout deje de adivinar la altura con `pb-8` fijo. La barra
// en sí usa `min-h-12`; este número debe coincidir con él (12 * 4px).
const STATUS_BAR_MIN_HEIGHT_PX = 48

/**
 * UI-CORR-1: publica el alto real de la barra en la variable CSS
 * `--status-bar-h` de `documentElement`. `App.vue` la consume con
 * `calc(var(--status-bar-h, 48px) + 8px)`, así que el espacio reservado es
 * SIEMPRE el alto medido y no una constante que hay que mantener a mano. Se
 * acepta un alto de 0 (jsdom, barra oculta) cayendo en el mínimo.
 */
function publishStatusBarHeight() {
  if (typeof document === 'undefined') return
  const measured = statusBarEl.value?.offsetHeight ?? 0
  const height = measured > 0 ? measured : STATUS_BAR_MIN_HEIGHT_PX
  document.documentElement.style.setProperty('--status-bar-h', `${height}px`)
}

let statusBarHeightObserver: ResizeObserver | null = null

// Track the real download throughput reported by the backend
function handleDownloadProgress(payload: any) {
  if (!payload) return
  const status = typeof payload.status === 'string' ? payload.status : ''
  if (payload.terminal === true || isTerminalDownloadStatus(status)) {
    downloadSpeedKBps.value = null
    return
  }
  const instant = Number(payload.instant_kbps)
  const average = Number(payload.average_kbps)
  downloadSpeedKBps.value = instant > 0 ? instant : (average > 0 ? average : null)
}

async function changeLocation() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Download Folder'
    })
    
    if (selected && typeof selected === 'string') {
      await saveSetting('download_path', selected)
      storagePath.value = selected
      toast.success('Download location updated')
      await fetchStorage()
    }
  } catch (e) {
    console.error('Failed to change location:', e)
    toast.error('Failed to update download location')
  }
  showStoragePopover.value = false
}

// PL-06: `collapse`/`expand` eliminados. Sin botón que las llame, y sin estado
// `isCollapsed` que las sostenga, quedaban como código muerto con escritura en
// `localStorage`.

// Close popovers on outside click
function handleOutsideClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (!target.closest('.status-popover') && !target.closest('.status-section')) {
    showSyncPopover.value = false
    showNetworkPopover.value = false
    showStoragePopover.value = false
  }
}

// Check online status
function checkOnline() {
  isOnline.value = navigator.onLine
}

onMounted(async () => {
  // UI-CORR-1: publica el alto REAL de la barra en `--status-bar-h`, para que
  // `#page-content` (App.vue) reserve exactamente ese espacio y el reproductor
  // central nunca quede tapado por la propia barra. Se publica el mínimo
  // conocido al montar (jsdom y el primer frame no tienen alto útil) y se
  // re-mide con ResizeObserver cuando el contenido crece.
  publishStatusBarHeight()
  if (typeof ResizeObserver !== 'undefined' && statusBarEl.value) {
    statusBarHeightObserver = new ResizeObserver(() => publishStatusBarHeight())
    statusBarHeightObserver.observe(statusBarEl.value)
  }

  document.addEventListener('click', handleOutsideClick)
  window.addEventListener('online', checkOnline)
  window.addEventListener('offline', checkOnline)

  // PL-08: al borrar NowPlayingBar su llamador se perdio. La restauracion de
  // cola/volumen/posicion vivia en su onMounted; sin ella el estado guardado
  // no se recuperaba nunca. Ahora la hace el reproductor de esta barra, que
  // es el unico punto de montaje del shell.
  await player.restoreFromPersistence()

  eventBus.on(TauriEvents.DOWNLOAD_PROGRESS, handleDownloadProgress).then(unlisten => {
    if (unlisten) unlistenDownloadProgress = unlisten
  })

  // PL-06: ya no se restaura `syncify_statusbar_collapsed`. Si se conservara
  // la lectura, quien ya la tuviera colapsada vería una barra inferior vacía
  // (la que la restauraba era el botón que este mismo cambio elimina) y sin
  // forma de volver a sacarla.

  // Initial fetch
  await Promise.all([
    fetchStorage(),
    fetchServices()
  ])

  // Periodic refresh
  storageInterval = setInterval(fetchStorage, 60000)
})

onUnmounted(() => {
  statusBarHeightObserver?.disconnect()
  statusBarHeightObserver = null
  document.removeEventListener('click', handleOutsideClick)
  window.removeEventListener('online', checkOnline)
  window.removeEventListener('offline', checkOnline)

  if (unlistenDownloadProgress) {
    unlistenDownloadProgress()
    unlistenDownloadProgress = undefined
  }
  if (etaInterval) {
    clearInterval(etaInterval)
    etaInterval = undefined
  }

  if (storageInterval) {
    clearInterval(storageInterval)
  }

  if (servicesRefreshTimer) {
    clearTimeout(servicesRefreshTimer)
    servicesRefreshTimer = undefined
  }
})

// PL-06: `isCollapsed` sale de la exposición; el estado ya no existe.
defineExpose({ syncState })
</script>

<style scoped>
/* Slide up animation */
.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.2s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(10px);
}

/* Fade animation */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Spin animation */
@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 1s linear infinite;
}

/* Pulse animation */
@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.animate-pulse {
  animation: pulse 2s ease-in-out infinite;
}

.bottom-26 {
  bottom: 6.5rem;
}

/* UI-CORR-1: con `h-auto` + `min-h-12`, el panel centrado deja de quedar
   recortado por la barra; las secciones se reparten el alto crecido sin
   solaparse. `flex-wrap` evita que el mini-reproductor y las secciones de los
   extremos se pisen a lo ancho en ventanas estrechas. */
</style>

<script lang="ts">
/**
 * UI-CORR-1 (10-oct-2026): alto mínimo de la barra inferior, en px.
 *
 * Vive en un `<script>` normal (no `setup`) porque `<script setup>` no admite
 * `export`. Es la fuente única que consume `App.vue` para reservar el hueco de
 * `#page-content`, y debe coincidir con el `min-h-12` de la barra.
 */
export const STATUS_BAR_MIN_HEIGHT_PX = 48
</script>
