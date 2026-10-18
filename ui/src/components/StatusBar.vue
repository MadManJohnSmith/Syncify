<template>
  <!-- PL-06: the bottom bar no longer collapses. It used to be `v-if="!isCollapsed"`
       with a floating button to bring it back; the user asked that there be no
       way to collapse it, so the persistence of `syncify_statusbar_collapsed`
       goes away too: keeping it would have left anyone who already had it
       collapsed with no bar and no button to recover it.

       UI-CORR-1 (10-oct-2026): REAL and flexible height. It used to be a fixed
       `h-8`, and the central mini player (PL-08) needed more than 32px: its
       progress bar and controls were clipped by the bar itself. Now the
       minimum height is `min-h-12` (48px) and it grows with the content
       (`h-auto`), with no ceiling to clip it. `App.vue` reserves exactly that
       real height through `--status-bar-h` (see STATUS_BAR_MIN_HEIGHT_PX) in
       `#page-content`, so the layout padding does not depend on a hardcoded
       constant. -->
  <div
    ref="statusBarEl"
    :class="[
      'status-bar fixed bottom-0 left-0 right-0 h-auto min-h-12 bg-shell border-t border-line',
      'flex items-center gap-x-3 gap-y-1 flex-wrap px-3 py-1 text-xs',
      overlayActive ? 'z-0' : (showSyncPopover || showNetworkPopover || showStoragePopover) ? 'z-[160]' : 'z-[50]'
    ]"
  >
    <!-- Left Section: Background Processes (BP-02/BP-04).
         It does not announce active downloads: those are checked in DownloadsView. -->
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
         Renamed from "Sync Details": this panel no longer describes ONE
         sync, it describes the set of BACKGROUND processes
         (sync, metadata enrichment, lyrics). That is why it filters out
         `download` (foreground in DownloadsView) and `scan` / `import`
         started synchronously from the UI (AccountsView.vue:1359-1396). -->
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

        <!-- No background processes: we say so, rather than opening an empty
             panel or showing the status of an unrelated download. -->
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
         BP-03: a THIRD progress bar ("Global Progress: covers every active
         task at once") with the name of the active task used to live here.
         It is removed. The global bar is the 2 px one in the header (App.vue)
         and the per-process detail lives in "Background Processes", above. -->
    <div class="flex items-center justify-center gap-6">
      <!-- UI-CORR-2 (10-oct-2026): the "Online" button is on the left, but
           its popover used to center on the bar (`left-1/2 -translate-x-1/2`),
           so it flew up to 300px away from the text the user had
           clicked. Now the button is the anchor itself: it lives in a
           `relative` container and the popover is positioned relative to it
           (`left-1/2 -translate-x-1/2` + `bottom-full`), so it stays centered
           OVER the button. On narrow screens the panel overflows to the
           left/right, so it is clamped to the viewport with `left-0
           right-auto` + `max-w` (see networkPopoverStyle). The `relative` creates
           the positioning context without touching the Z axis: the bar keeps
           raising its stacking (z-[160] when a popover opens), and the panel
           inherits that level instead of creating a new one. -->
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

        <!-- Network Popover: anchored to the Online button (see comment above). -->
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
    
    <!-- PL-08: Status bar center — Current track and mini player -->
    <div
      v-if="current"
      class="status-center-player flex-1 flex items-center justify-center gap-2.5 min-w-0 mx-2 text-xs"
      data-testid="status-bar-player"
    >
      <!-- PL-09: Cover with hover affordance and click to expand -->
      <button
        @click="openFullscreen"
        data-testid="status-bar-track-cover"
        class="group relative w-6 h-6 rounded overflow-hidden shrink-0 cursor-pointer focus:outline-none focus:ring-1 focus:ring-accent"
        title="Expand player (Fullscreen)"
        aria-label="Expand player (Fullscreen)"
      >
        <TrackCover
          :src="current.coverUrl"
          :item-id="current.id"
          :alt="current.album ?? current.title"
          size-class="w-6 h-6 text-[10px]"
          rounded="rounded"
        />
        <!-- Hover affordance: overlay with expand icon -->
        <div
          class="absolute inset-0 bg-black/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center text-white"
          data-testid="cover-expand-hint"
        >
          <span class="material-symbols-outlined text-[14px]">open_in_full</span>
        </div>
      </button>

      <!-- Title and artist (clickable to expand) -->
      <button
        @click="openFullscreen"
        class="flex items-center gap-1.5 min-w-0 max-w-[200px] sm:max-w-[280px] truncate text-left cursor-pointer hover:text-accent transition-colors focus:outline-none"
        title="Expand player (Fullscreen)"
        data-testid="status-bar-track-info"
      >
        <span class="font-medium text-ink truncate">{{ current.title }}</span>
        <span class="text-ink-3 shrink-0">·</span>
        <span class="text-text-secondary truncate text-[11px]">{{ current.artist }}</span>
      </button>

      <!-- Mini transport: prev, play/pause, next -->
      <div class="flex items-center gap-0.5 shrink-0">
        <button
          @click="previous"
          class="p-0.5 rounded hover:bg-elevated text-ink-2 hover:text-ink transition-colors"
          title="Previous track"
          aria-label="Previous track"
          data-testid="status-bar-prev"
        >
          <span class="material-symbols-outlined text-sm">skip_previous</span>
        </button>
        <button
          @click="toggle"
          class="p-0.5 rounded hover:bg-elevated text-ink hover:text-accent transition-colors"
          :title="isPlaying ? 'Pause' : 'Play'"
          :aria-label="isPlaying ? 'Pause' : 'Play'"
          data-testid="status-bar-toggle"
        >
          <span class="material-symbols-outlined text-sm">{{ isPlaying ? 'pause' : 'play_arrow' }}</span>
        </button>
        <button
          @click="next"
          class="p-0.5 rounded hover:bg-elevated text-ink-2 hover:text-ink transition-colors"
          title="Next track"
          aria-label="Next track"
          data-testid="status-bar-next"
        >
          <span class="material-symbols-outlined text-sm">skip_next</span>
        </button>
      </div>

      <!-- Elapsed / total time -->
      <span
        v-if="durationSec > 0"
        class="font-mono text-[10px] text-ink-3 shrink-0 tabular-nums hidden sm:inline"
        data-testid="status-bar-time"
      >
        {{ formatTime(positionSec) }} / {{ formatTime(durationSec) }}
      </span>

      <!-- Explicit button to expand to fullscreen -->
      <button
        @click="openFullscreen"
        class="p-0.5 rounded hover:bg-elevated text-ink-2 hover:text-ink transition-colors shrink-0"
        title="Fullscreen"
        aria-label="Fullscreen"
        data-testid="status-bar-expand-btn"
      >
        <span class="material-symbols-outlined text-sm">open_in_full</span>
      </button>
    </div>
    <!-- Keep the center free when nothing is playing -->
    <div v-else class="flex-1"></div>


    <!-- BP-03: Settings does NOT live here. With the mini player taking the
         center, the bottom bar only has room for Background Processes,
         Online/Network and Storage; Settings goes back to the sidebar
         (App.vue), where there is plenty of room for text + icon. The button and
         `goToSettings()` are removed instead of being left orphaned. -->

    <!-- Storage keeps the bottom-right corner and its popover anchored to
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

  <!-- PL-04, PL-05, PL-09, PL-10: Large player / fullscreen -->
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
 * UI-CORR-2 (10-oct-2026): the Network popover is anchored to the "Online" button,
 * not to the center of the bar. The panel is placed above the button
 * (`bottom-full` relative to the `relative` container that wraps it) and
 * centered on it with `left: 50%; transform: translateX(-50%)`.
 *
 * That centering would overflow on the left in narrow windows (the button
 * lives in the center of the bar and the panel is 288px wide). It is clamped
 * to the viewport width: if the panel does not fit, its width is limited to the
 * space available on each side and `left: 0` is left so it lines up with the
 * button instead of running off screen. The calculation is done in px over the
 * real width of the container, measured on the client (essential: in jsdom
 * `getBoundingClientRect` returns zeros and there is no real CSS, so without
 * this guard the computed style would come out as `left: 0` with no clamp and
 * lose the anchoring).
 */
const statusBarEl = ref<HTMLElement | null>(null)

/**
 * UI-CORR-2: position of the Network popover, recalculated on every render
 * (a function, not a `computed`). Window geometry is not reactive: if it
 * were a `computed`, it would be cached the first time and the panel would
 * stay anchored where it first appeared, not where the button is after a
 * resize. Since it is a function invoked from the template, it is re-evaluated
 * on every repaint of the component, and `showNetworkPopover` does force a new
 * one on open and close.
 */
function networkPopoverStyle(): Record<string, string> {
  // Appears above the button; the `mb-2` is the visual separation.
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
  // The panel is 288px wide (w-72); half is reserved so it does not overflow.
  const halfPanel = 288 / 2
  const roomLeft = anchorRect.left - barRect.left + (anchorRect.width / 2) - halfPanel
  const roomRight = barRect.right - (anchorRect.left + anchorRect.width / 2) - halfPanel
  if (roomLeft < 0) {
    // Does not fit on the left: align to the left edge of the bar.
    return { ...base, left: '0px', transform: 'none' }
  }
  if (roomRight < 0) {
    // Does not fit on the right: align to the right edge of the bar.
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

// BP-02: this panel is for BACKGROUND processes. An in-progress download is a
// foreground task — the user follows it in DownloadsView — and is left out.
// Same for `scan` (AccountsView.vue:1359-1396 starts it and waits for it to
// finish) and synchronous `import`. The types are those of `GlobalTask.type`
// (`useGlobalTasks.ts:15`); `sync`, `metadata`, `lyrics` are indeed tasks that
// continue through backend events.
const BACKGROUND_TASK_TYPES = new Set(['sync', 'metadata', 'lyrics'])
const backgroundTasks = computed(() => activeTasks.value.filter(t => BACKGROUND_TASK_TYPES.has(t.type)))
const backgroundAllTasks = computed(() => allTasks.value.filter(t => BACKGROUND_TASK_TYPES.has(t.type)))

// `deriveSyncState` from statusTelemetry.ts:118 has the old policy and
// counts any running task (including download). That function stays
// intact —it is outside this lane and has its own tests—, but the indicator here
// applies the BP-02 policy only to `backgroundAllTasks`.
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

// Weighting by item count; without a total, weight equals the average of the
// declared totals (or 1 if there are none). The same rule as the App.vue global
// bar is kept, without adding download bytes.
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
    // BP-02: counts background processes, not in-progress downloads.
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

// UI-CORR-1 (10-oct-2026): REAL minimum height of the bottom bar, in px.
// It is the single source that `App.vue` consumes to reserve the slot for
// `#page-content` (`padding-bottom: calc(var(--status-bar-h, 48px) + 8px)`),
// so the layout stops guessing the height with a fixed `pb-8`. The bar
// itself uses `min-h-12`; this number must match it (12 * 4px).
const STATUS_BAR_MIN_HEIGHT_PX = 48

/**
 * UI-CORR-1: publishes the real height of the bar in the CSS variable
 * `--status-bar-h` of `documentElement`. `App.vue` consumes it with
 * `calc(var(--status-bar-h, 48px) + 8px)`, so the reserved space is
 * ALWAYS the measured height and not a constant that has to be kept by hand.
 * A height of 0 is accepted (jsdom, hidden bar) by falling back to the minimum.
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

// PL-06: `collapse`/`expand` removed. With no button calling them, and no
// `isCollapsed` state to hold them up, they were left as dead code writing to
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
  // UI-CORR-1: publishes the REAL height of the bar in `--status-bar-h`, so that
  // `#page-content` (App.vue) reserves exactly that space and the central
  // player is never covered by the bar itself. The known minimum is published
  // on mount (jsdom and the first frame have no useful height) and is
  // re-measured with ResizeObserver when the content grows.
  publishStatusBarHeight()
  if (typeof ResizeObserver !== 'undefined' && statusBarEl.value) {
    statusBarHeightObserver = new ResizeObserver(() => publishStatusBarHeight())
    statusBarHeightObserver.observe(statusBarEl.value)
  }

  document.addEventListener('click', handleOutsideClick)
  window.addEventListener('online', checkOnline)
  window.addEventListener('offline', checkOnline)

  // PL-08: deleting NowPlayingBar lost its caller. Restoring
  // queue/volume/position lived in its onMounted; without it the saved state
  // was never recovered. Now the player of this bar does it, since it
  // is the only mount point of the shell.
  await player.restoreFromPersistence()

  eventBus.on(TauriEvents.DOWNLOAD_PROGRESS, handleDownloadProgress).then(unlisten => {
    if (unlisten) unlistenDownloadProgress = unlisten
  })

  // PL-06: `syncify_statusbar_collapsed` is no longer restored. If the
  // read were kept, anyone who already had it collapsed would see an empty
  // bottom bar (the one that restored it was the button this very change removes)
  // and with no way to bring it back.

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

// PL-06: `isCollapsed` is removed from the exposure; the state no longer exists.
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

/* UI-CORR-1: with `h-auto` + `min-h-12`, the centered panel is no longer
   clipped by the bar; the sections share the grown height without
   overlapping. `flex-wrap` prevents the mini player and the edge sections
   from stepping on each other horizontally in narrow windows. */
</style>

<script lang="ts">
/**
 * UI-CORR-1 (10-oct-2026): minimum height of the bottom bar, in px.
 *
 * It lives in a regular `<script>` (not `setup`) because `<script setup>` does
 * not allow `export`. It is the single source that `App.vue` consumes to reserve
 * the slot for `#page-content`, and must match the bar's `min-h-12`.
 */
export const STATUS_BAR_MIN_HEIGHT_PX = 48
</script>
