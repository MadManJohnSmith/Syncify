<template>
  <div class="queue-view h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <!-- Loading State -->
    <div v-if="loading" class="loading flex items-center justify-center h-full">
      <div class="flex flex-col items-center gap-4">
        <div class="animate-spin w-8 h-8 border-3 border-primary border-t-transparent rounded-full"></div>
        <span class="text-text-secondary">Loading queue...</span>
      </div>
    </div>

    <!-- Main Content -->
    <template v-else>
      <!-- Header -->
      <div class="px-8 pt-8 pb-4 shrink-0">
        <div class="flex items-center justify-between mb-6">
          <div>
            <h1 class="text-3xl font-bold tracking-tight text-gray-900 dark:text-white mb-1">Queue</h1>
            <p class="text-text-secondary">Manage your download queue</p>
          </div>

          <!-- Worker Controls -->
          <div class="worker-controls flex items-center gap-4">
            <div class="flex items-center gap-2 px-4 py-2 rounded-lg bg-surface-light dark:bg-surface-dark border border-border-light dark:border-border-dark">
              <span 
                class="w-2 h-2 rounded-full" 
                :class="workerStatus.paused ? 'bg-amber-500' : 'bg-emerald-500'"
              ></span>
              <span class="text-sm font-medium text-gray-700 dark:text-gray-300">
                {{ workerStatus.paused ? 'Paused' : 'Running' }}
              </span>
              <span class="text-xs text-text-secondary">
                {{ workerStatus.active_downloads }}/{{ workerStatus.max_concurrent }}
              </span>
            </div>
            <button
              v-if="workerStatus.paused"
              @click="resumeDownloads"
              class="btn-primary px-4 py-2 rounded-lg text-sm font-medium"
            >
              Resume
            </button>
            <button
              v-else
              @click="pauseDownloads"
              class="btn-secondary px-4 py-2 rounded-lg text-sm font-medium border border-border-light dark:border-border-dark"
            >
              Pause
            </button>
          </div>
        </div>

        <!-- Stats Bar -->
        <div class="stats-bar flex items-center gap-3">
          <div
            v-for="(stat, index) in statItems"
            :key="stat.key ?? `stat-${index}`"
            class="stat-item flex items-center gap-2 px-4 py-2 rounded-lg cursor-pointer transition-colors"
            :class="[
              selectedFilter === stat.key 
                ? 'bg-primary/10 border border-primary/30' 
                : 'bg-surface-light dark:bg-surface-dark border border-border-light dark:border-border-dark hover:bg-gray-100 dark:hover:bg-gray-800'
            ]"
            @click="setFilter(stat.key)"
          >
            <span class="material-symbols-outlined text-[18px]" :class="stat.iconClass">{{ stat.icon }}</span>
            <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{{ stat.label }}</span>
            <span class="text-sm font-bold" :class="stat.countClass">{{ stat.count }}</span>
          </div>
        </div>
      </div>

      <!-- Queue List -->
      <div class="flex-1 overflow-y-auto px-8 pb-8">
        <!-- Empty State -->
        <div v-if="filteredItems.length === 0" class="empty-state flex flex-col items-center justify-center h-64 text-center">
          <span class="material-symbols-outlined text-6xl text-gray-300 dark:text-gray-600 mb-4">queue_music</span>
          <h3 class="text-lg font-medium text-gray-700 dark:text-gray-300 mb-2">No items in queue</h3>
          <p class="text-text-secondary">Add tracks from your library to start downloading</p>
        </div>

        <!-- Queue Items -->
        <div v-else class="space-y-3">
          <div
            v-for="item in filteredItems"
            :key="item.id"
            class="queue-item flex items-center gap-4 p-4 rounded-xl bg-surface-light dark:bg-surface-dark border border-border-light dark:border-border-dark"
          >
            <!-- Status Icon -->
            <div class="shrink-0">
              <span 
                class="material-symbols-outlined text-2xl"
                :class="getStatusIconClass(item.status)"
              >
                {{ getStatusIcon(item.status) }}
              </span>
            </div>

            <!-- Track Info -->
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <h4 class="font-medium text-gray-900 dark:text-white truncate">{{ item.title }}</h4>
                <!-- Failure Classification Badge for Failed Items -->
                <span 
                  v-if="item.status === 'failed'"
                  :class="['px-2 py-0.5 rounded text-[10px] font-bold uppercase flex items-center gap-1 shrink-0', classifyFailureReason(item.error_message).badgeClass]"
                >
                  <span class="material-symbols-outlined text-[13px]">{{ classifyFailureReason(item.error_message).icon }}</span>
                  {{ classifyFailureReason(item.error_message).label }}
                </span>
              </div>
              <p class="text-sm text-text-secondary truncate">{{ item.artist }}</p>
              
              <!-- Progress Bar (for downloading items) -->
              <div v-if="item.status === 'downloading'" class="progress-bar mt-2 h-2 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                <div 
                  class="progress-fill h-full bg-primary rounded-full transition-all duration-300"
                  :style="{ width: `${item.progress_percent}%` }"
                ></div>
              </div>
              
              <!-- Error Message -->
              <p v-if="item.error_message" class="text-sm text-red-500 mt-1 flex items-center gap-1">
                <span class="material-symbols-outlined text-[14px]">error</span>
                <span>{{ item.error_message }}</span>
              </p>
            </div>

            <!-- Actions -->
            <div class="shrink-0 flex items-center gap-2">
              <!-- Retry button for failed items -->
              <template v-if="item.status === 'failed'">
                <button
                  v-if="classifyFailureReason(item.error_message).reason === 'network'"
                  @click="retryItem(item.id)"
                  class="btn-icon flex items-center gap-1 px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-xs font-semibold transition-colors"
                  title="Retry original source"
                >
                  <span class="material-symbols-outlined text-[16px]">refresh</span>
                  <span>Retry original source</span>
                </button>
                <button
                  v-else-if="classifyFailureReason(item.error_message).reason === 'requires_auth'"
                  @click="router.push('/accounts')"
                  class="btn-icon flex items-center gap-1 px-3 py-1.5 rounded-lg bg-rose-600 hover:bg-rose-700 text-white text-xs font-semibold transition-colors"
                  title="Check Account"
                >
                  <span class="material-symbols-outlined text-[16px]">manage_accounts</span>
                  <span>Check Account</span>
                </button>
                <button
                  v-else
                  @click="retryItem(item.id)"
                  class="btn-icon p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                  title="Retry"
                >
                  <span class="material-symbols-outlined text-[20px] text-amber-500">refresh</span>
                </button>
              </template>
              
              <!-- Cancel button for queued/downloading -->
              <button
                v-if="item.status === 'queued' || item.status === 'downloading'"
                @click="cancelItem(item.id)"
                class="btn-icon p-2 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                title="Cancel"
              >
                <span class="material-symbols-outlined text-[20px] text-gray-400">close</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useRouter } from 'vue-router';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { TauriEvents } from '@/api/tauri';
import {
  getQueue as fetchQueueFromApi,
  getQueueStats as fetchStatsFromApi,
  getWorkerStatus as fetchWorkerFromApi,
  pauseDownloads as apiPauseDownloads,
  resumeDownloads as apiResumeDownloads,
  retryItem as apiRetryItem,
  cancelItem as apiCancelItem,
  classifyFailureReason,
} from '@/api/queue';

const router = useRouter();

interface QueueItem {
  id: number;
  track_id: number;
  title: string;
  artist: string;
  status: 'queued' | 'downloading' | 'complete' | 'failed' | 'cancelled';
  priority: number;
  progress_percent: number;
  error_message: string | null;
  created_at: string;
}

interface QueueStats {
  queued: number;
  downloading: number;
  complete: number;
  failed: number;
  cancelled: number;
}

interface WorkerStatus {
  running: boolean;
  paused: boolean;
  active_downloads: number;
  max_concurrent: number;
}

interface ProgressEvent {
  queue_id: number;
  track_id: number;
  title: string;
  artist: string;
  status: string;
  progress_percent: number;
  message: string | null;
}

const loading = ref(true);
const queueItems = ref<QueueItem[]>([]);
const stats = ref<QueueStats>({ queued: 0, downloading: 0, complete: 0, failed: 0, cancelled: 0 });
const workerStatus = ref<WorkerStatus>({ running: true, paused: true, active_downloads: 0, max_concurrent: 3 });
const selectedFilter = ref<string | null>(null);


let unlistenProgress: UnlistenFn | null = null;

const totalItems = computed(() => 
  stats.value.queued + stats.value.downloading + stats.value.complete + stats.value.failed + stats.value.cancelled
);

const statItems = computed(() => [
  { key: null, label: 'Total', count: totalItems.value, icon: 'playlist_play', iconClass: 'text-gray-500', countClass: 'text-gray-700 dark:text-gray-300' },
  { key: 'queued', label: 'Queued', count: stats.value.queued, icon: 'schedule', iconClass: 'text-blue-500', countClass: 'text-blue-600' },
  { key: 'downloading', label: 'Downloading', count: stats.value.downloading, icon: 'downloading', iconClass: 'text-primary', countClass: 'text-primary' },
  { key: 'complete', label: 'Complete', count: stats.value.complete, icon: 'check_circle', iconClass: 'text-emerald-500', countClass: 'text-emerald-600' },
  { key: 'failed', label: 'Failed', count: stats.value.failed, icon: 'error', iconClass: 'text-red-500', countClass: 'text-red-600' },
]);

const filteredItems = computed(() => {
  if (!selectedFilter.value) return queueItems.value;
  return queueItems.value.filter(item => item.status === selectedFilter.value);
});

function setFilter(key: string | null) {
  selectedFilter.value = key;
}

function getStatusIcon(status: string): string {
  const icons: Record<string, string> = {
    queued: 'schedule',
    downloading: 'downloading',
    complete: 'check_circle',
    failed: 'error',
    cancelled: 'cancel',
  };
  return icons[status] || 'help';
}

function getStatusIconClass(status: string): string {
  const classes: Record<string, string> = {
    queued: 'text-blue-500',
    downloading: 'text-primary animate-pulse',
    complete: 'text-emerald-500',
    failed: 'text-red-500',
    cancelled: 'text-gray-400',
  };
  return classes[status] || 'text-gray-500';
}

async function loadData() {
  try {
    // All commands go through the api layer, which normalizes missing fields.
    const [items, queueStats, worker] = await Promise.all([
      fetchQueueFromApi(),
      fetchStatsFromApi(),
      fetchWorkerFromApi(),
    ]);
    queueItems.value = items.map((item) => ({
      id: item.id,
      track_id: item.track_id,
      title: item.title ?? '',
      artist: item.artist ?? '',
      status: item.status as QueueItem['status'],
      priority: item.priority,
      progress_percent: item.progress_percent,
      error_message: item.error_message,
      created_at: item.created_at ?? '',
    }));
    stats.value = {
      queued: queueStats.queued,
      downloading: queueStats.downloading,
      complete: queueStats.completed,
      failed: queueStats.failed,
      cancelled: queueStats.cancelled ?? 0,
    };
    workerStatus.value = worker;
  } catch (error) {
    console.error('Failed to load queue data:', error);
  } finally {
    loading.value = false;
  }
}

async function pauseDownloads() {
  try {
    await apiPauseDownloads();
    workerStatus.value.paused = true;
  } catch (error) {
    console.error('Failed to pause downloads:', error);
  }
}

async function resumeDownloads() {
  try {
    await apiResumeDownloads();
    workerStatus.value.paused = false;
  } catch (error) {
    console.error('Failed to resume downloads:', error);
  }
}

async function retryItem(queueId: number) {
  try {
    await apiRetryItem(queueId);
    await loadData();
  } catch (error) {
    console.error('Failed to retry item:', error);
  }
}

async function cancelItem(queueId: number) {
  try {
    await apiCancelItem(queueId);
    await loadData();
  } catch (error) {
    console.error('Failed to cancel item:', error);
  }
}

function handleProgressEvent(event: { payload: ProgressEvent }) {
  const { queue_id, progress_percent, status } = event.payload;
  const item = queueItems.value.find(i => i.id === queue_id);
  if (item) {
    item.progress_percent = progress_percent;
    if (status === 'completed') {
      item.status = 'complete';
    } else if (status === 'failed') {
      item.status = 'failed';
    }
  }
}

onMounted(async () => {
  await loadData();
  unlistenProgress = await listen<ProgressEvent>(TauriEvents.DOWNLOAD_PROGRESS, handleProgressEvent);
});

onUnmounted(() => {
  if (unlistenProgress) {
    unlistenProgress();
  }
});
</script>

<style scoped>
.btn-primary {
  @apply bg-primary text-white hover:bg-primary-dark transition-colors;
}

.btn-secondary {
  @apply bg-surface-light dark:bg-surface-dark text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors;
}
</style>
