<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4" data-testid="download-job-modal">
    <!-- Backdrop -->
    <div
      class="absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity"
      @click="handleClose"
    ></div>

    <!-- Modal Card -->
    <div class="relative w-full max-w-[680px] flex flex-col rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl overflow-hidden transform transition-all animate-in fade-in zoom-in-95 duration-200">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-5 border-b border-gray-100 dark:border-border-dark">
        <div class="flex items-center gap-3">
          <div class="h-10 w-10 flex items-center justify-center rounded-xl bg-primary/10 text-primary">
            <span class="material-symbols-outlined text-[24px]">download_for_offline</span>
          </div>
          <div>
            <h2 class="text-lg font-bold tracking-tight text-gray-900 dark:text-white">Create Download Job</h2>
            <p class="text-xs text-gray-500 dark:text-gray-400">
              The job is created in the background; you can close this window once it starts.
            </p>
          </div>
        </div>
        <button
          @click="handleClose"
          aria-label="Close modal"
          class="rounded-full p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-white/10 transition-colors"
        >
          <span class="material-symbols-outlined text-[20px]">close</span>
        </button>
      </div>

      <!-- Error Banner -->
      <div v-if="error" class="mx-6 mt-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 text-sm flex items-center gap-2">
        <span class="material-symbols-outlined text-[18px]">error</span>
        <span class="flex-1">{{ error }}</span>
        <button @click="error = null" class="text-red-500 hover:text-red-700">
          <span class="material-symbols-outlined text-[16px]">close</span>
        </button>
      </div>

      <!-- Empty state: nothing to enqueue -->
      <div v-if="emptyMessage" class="mx-6 mt-4 p-4 rounded-xl border bg-blue-500/10 border-blue-500/30 text-blue-900 dark:text-blue-200 text-sm flex items-start gap-3">
        <span class="material-symbols-outlined text-[22px] text-blue-500 mt-0.5">info</span>
        <p>{{ emptyMessage }}</p>
      </div>

      <!-- Aggregate estimate confirmation: only for jobs above 1.000 tracks -->
      <div v-if="confirmation" class="p-6 space-y-4" data-testid="job-estimate-confirmation">
        <div class="rounded-xl bg-amber-500/10 border border-amber-500/30 p-4">
          <div class="flex items-start gap-3">
            <span class="material-symbols-outlined text-[22px] text-amber-500 mt-0.5">verified</span>
            <div class="flex-1">
              <p class="font-semibold text-amber-900 dark:text-amber-100">Confirm this large download</p>
              <p class="text-xs opacity-90 mt-1">
                This job requests {{ confirmation.estimate.total.toLocaleString() }} tracks
                (~{{ formatSizeMb(confirmation.estimate.estimatedSizeMb) }}). Confirm to create it.
              </p>
            </div>
          </div>

          <div class="grid grid-cols-2 sm:grid-cols-3 gap-2 mt-4 pt-3 border-t border-current/10 text-xs" data-testid="job-estimate-summary">
            <div class="bg-black/5 dark:bg-white/5 rounded-lg p-2 text-center">
              <div class="font-bold text-sm text-gray-700 dark:text-gray-200">{{ confirmation.estimate.total.toLocaleString() }}</div>
              <div class="text-gray-500 dark:text-gray-400">Tracks</div>
            </div>
            <div class="bg-black/5 dark:bg-white/5 rounded-lg p-2 text-center">
              <div class="font-bold text-sm text-primary">{{ confirmation.estimate.byCapacity.hires.toLocaleString() }}</div>
              <div class="text-gray-500 dark:text-gray-400">Hi-Res</div>
            </div>
            <div class="bg-black/5 dark:bg-white/5 rounded-lg p-2 text-center">
              <div class="font-bold text-sm text-emerald-600 dark:text-emerald-400">{{ confirmation.estimate.byCapacity.lossless.toLocaleString() }}</div>
              <div class="text-gray-500 dark:text-gray-400">Lossless</div>
            </div>
            <div class="bg-black/5 dark:bg-white/5 rounded-lg p-2 text-center">
              <div class="font-bold text-sm text-amber-500">{{ confirmation.estimate.byCapacity.high.toLocaleString() }}</div>
              <div class="text-gray-500 dark:text-gray-400">High</div>
            </div>
            <div class="bg-black/5 dark:bg-white/5 rounded-lg p-2 text-center">
              <div class="font-bold text-sm text-gray-500 dark:text-gray-400">{{ confirmation.estimate.byCapacity.lossy.toLocaleString() }}</div>
              <div class="text-gray-500 dark:text-gray-400">Lossy</div>
            </div>
            <div class="bg-black/5 dark:bg-white/5 rounded-lg p-2 text-center">
              <div class="font-bold text-sm text-purple-600 dark:text-purple-400">{{ confirmation.estimate.byCapacity.unknown.toLocaleString() }}</div>
              <div class="text-gray-500 dark:text-gray-400">No source</div>
            </div>
          </div>

          <div v-if="confirmation.estimate.bySource.length > 0" class="mt-3 text-[11px] text-gray-600 dark:text-gray-300 flex flex-wrap gap-x-3 gap-y-1">
            <span class="font-semibold">Sources:</span>
            <span v-for="item in confirmation.estimate.bySource" :key="`src-${item.service}`">
              {{ formatServiceName(item.service) }}: {{ item.count.toLocaleString() }}
            </span>
          </div>
          <div v-if="confirmation.estimate.byDestination.length > 0" class="mt-1 text-[11px] text-gray-600 dark:text-gray-300 flex flex-wrap gap-x-3 gap-y-1">
            <span class="font-semibold">Destinations:</span>
            <span v-for="item in confirmation.estimate.byDestination" :key="`dst-${item.service}`">
              {{ formatServiceName(item.service) }}: {{ item.count.toLocaleString() }}
            </span>
          </div>
        </div>
      </div>

      <!-- Form Content -->
      <div v-if="!confirmation" class="p-6 space-y-5">
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <!-- Source -->
          <div>
            <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2" for="job-source">
              Source service
            </label>
            <select
              id="job-source"
              v-model="selectedService"
              data-testid="job-source"
              class="w-full px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
            >
              <option v-for="svc in sourceOptions" :key="svc.value" :value="svc.value">{{ svc.label }}</option>
            </select>
            <p class="text-[11px] text-gray-500 dark:text-gray-400 mt-1">Library service the tracks come from.</p>
          </div>

          <!-- Destination -->
          <div>
            <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2" for="job-destination">
              Destination
            </label>
            <select
              id="job-destination"
              v-model="selectedDestination"
              data-testid="job-destination"
              class="w-full px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
            >
              <option value="">Automatic (resolver decides)</option>
              <option v-for="svc in destinationOptions" :key="svc.value" :value="svc.value">{{ svc.label }}</option>
            </select>
            <p class="text-[11px] text-gray-500 dark:text-gray-400 mt-1">Provider that delivers the audio.</p>
          </div>

          <!-- Account override -->
          <div>
            <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2" for="job-account">
              Account override
            </label>
            <select
              id="job-account"
              v-model="selectedAccountId"
              data-testid="job-account"
              class="w-full px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
            >
              <option :value="undefined">Automatic (preferred account)</option>
              <option v-for="account in accountOptions" :key="account.id" :value="account.id">
                {{ accountLabel(account) }}
              </option>
            </select>
            <p class="text-[11px] text-gray-500 dark:text-gray-400 mt-1">Soft preference: another account is used if it cannot serve.</p>
          </div>

          <!-- Favorites type -->
          <div>
            <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2" for="job-type">
              Type
            </label>
            <select
              id="job-type"
              v-model="selectedType"
              data-testid="job-type"
              class="w-full px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
            >
              <option v-for="item in typeOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
            </select>
          </div>
        </div>

        <!-- Quality Preference Selection -->
        <div>
          <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2">
            Quality profile
          </label>
          <div class="grid grid-cols-3 gap-2">
            <button
              type="button"
              v-for="q in qualityOptions"
              :key="q.value"
              @click="selectedQuality = q.value"
              :class="[
                'flex flex-col p-3 rounded-xl border text-left transition-all gap-1',
                selectedQuality === q.value
                  ? 'bg-accent-soft border-accent text-accent shadow-sm'
                  : 'bg-gray-50 dark:bg-surface-highlight/50 border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600'
              ]"
            >
              <div class="flex items-center justify-between">
                <span class="font-semibold text-xs text-gray-900 dark:text-white">{{ q.label }}</span>
                <span v-if="q.badge" class="px-1.5 py-0.5 rounded text-[10px] font-bold bg-primary/20 text-primary">{{ q.badge }}</span>
              </div>
              <span class="text-[11px] text-gray-500 dark:text-gray-400 leading-tight">{{ q.desc }}</span>
            </button>
          </div>
        </div>

        <!-- Batch Limit Selection -->
        <div>
          <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2">
            Limit
          </label>
          <div class="grid grid-cols-5 gap-2">
            <button
              type="button"
              v-for="lim in limitOptions"
              :key="lim.label"
              @click="selectedLimit = lim.value"
              :class="[
                'flex flex-col items-center justify-center p-2 rounded-xl border text-xs font-medium transition-all gap-0.5',
                selectedLimit === lim.value
                  ? 'bg-accent-soft border-accent text-accent shadow-sm font-semibold'
                  : 'bg-gray-50 dark:bg-surface-highlight/50 border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600'
              ]"
            >
              <span>{{ lim.label }}</span>
              <span class="text-[10px] text-text-secondary">{{ lim.sub }}</span>
            </button>
          </div>
          <p class="text-[11px] text-gray-500 dark:text-gray-400 mt-1">
            A job above {{ CONFIRM_THRESHOLD.toLocaleString() }} tracks asks for confirmation with an aggregate estimate.
          </p>
        </div>

        <!-- Priority & Options -->
        <div class="space-y-2">
          <div class="flex items-center justify-between p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight/30 border border-gray-200 dark:border-border-dark">
            <div class="flex items-center gap-2">
              <span class="material-symbols-outlined text-[20px] text-primary">priority_high</span>
              <div>
                <span class="text-xs font-semibold text-gray-900 dark:text-white block">High queue priority</span>
                <span class="text-[11px] text-gray-500 dark:text-gray-400">Process this job ahead of normal manual downloads.</span>
              </div>
            </div>
            <input
              type="checkbox"
              v-model="highPriority"
              data-testid="job-high-priority"
              class="h-4 w-4 rounded text-primary focus:ring-primary border-gray-300 dark:border-gray-600 bg-white dark:bg-surface-dark"
            />
          </div>

          <div class="flex items-center justify-between p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight/30 border border-gray-200 dark:border-border-dark">
            <div class="flex items-center gap-2">
              <span class="material-symbols-outlined text-[20px] text-blue-500">sync</span>
              <div>
                <span class="text-xs font-semibold text-gray-900 dark:text-white block">Sync cloud favorites first</span>
                <span class="text-[11px] text-gray-500 dark:text-gray-400">Fetch latest favorites from streaming services before enqueuing.</span>
              </div>
            </div>
            <input
              type="checkbox"
              v-model="syncBeforeDownload"
              class="h-4 w-4 rounded text-primary focus:ring-primary border-gray-300 dark:border-gray-600 bg-white dark:bg-surface-dark"
            />
          </div>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="flex items-center justify-end gap-3 px-6 py-4 border-t border-gray-100 dark:border-border-dark bg-gray-50/50 dark:bg-surface-dark/50">
        <template v-if="confirmation">
          <button
            type="button"
            @click="confirmation = null"
            class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
          >
            Back
          </button>
          <button
            type="button"
            @click="confirmAndStart"
            data-testid="job-confirm"
            class="flex items-center gap-2 px-5 py-2 text-sm font-semibold text-white bg-amber-600 hover:bg-amber-700 rounded-lg shadow-sm transition-all"
          >
            <span class="material-symbols-outlined text-[18px]">verified</span>
            <span>Confirm and create job ({{ confirmation.estimate.total.toLocaleString() }})</span>
          </button>
        </template>
        <template v-else>
          <button
            type="button"
            @click="handleClose"
            class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
          >
            Cancel
          </button>
          <button
            type="button"
            @click="submit"
            :disabled="isEstimating || isSyncing"
            data-testid="job-submit"
            class="flex items-center gap-2 px-5 py-2 text-sm font-semibold text-on-accent bg-primary hover:bg-primary-hover rounded-lg shadow-sm transition-all disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <span v-if="isEstimating || isSyncing" class="material-symbols-outlined text-[18px] animate-spin">progress_activity</span>
            <span v-else class="material-symbols-outlined text-[18px]">download</span>
            <span>{{ isSyncing ? 'Syncing favorites...' : (isEstimating ? 'Estimating...' : 'Create download job') }}</span>
          </button>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { syncFavorites } from '../api/library'
import {
  createDownloadJob,
  estimateDownloadJob,
  type DownloadJobEstimate,
  type DownloadJobRequest,
} from '../api/downloadJobs'
import { accountsApi } from '../api/accounts'
import type { Account } from '../api/types'
import { useToast } from '../composables/useToast'
import { formatServiceName } from '../composables/useGlobalTasks'

/** Jobs above this count confirm with an aggregate estimate first. */
const CONFIRM_THRESHOLD = 1000

export interface DownloadJobCreated {
  job_id: number
  total: number
  message: string
}

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'enqueued', result: DownloadJobCreated): void
}>()

const toast = useToast()

const selectedService = ref<string>('all')
const selectedDestination = ref<string>('')
const selectedAccountId = ref<number | undefined>(undefined)
const selectedType = ref<string>('all')
const selectedQuality = ref<string>('lossless')
const selectedLimit = ref<number | undefined>(5)
const highPriority = ref<boolean>(true)
const syncBeforeDownload = ref<boolean>(false)

const isEstimating = ref<boolean>(false)
const isSyncing = ref<boolean>(false)
const error = ref<string | null>(null)
const emptyMessage = ref<string | null>(null)
const confirmation = ref<{ request: DownloadJobRequest; estimate: DownloadJobEstimate } | null>(null)
const accounts = ref<Account[]>([])

const sourceOptions = [
  { value: 'all', label: 'All services' },
  { value: 'qobuz', label: 'Qobuz' },
  { value: 'tidal', label: 'Tidal' },
  { value: 'deezer', label: 'Deezer' },
  { value: 'soundcloud', label: 'SoundCloud' },
  { value: 'spotify', label: 'Spotify' },
  { value: 'apple_music', label: 'Apple Music' },
]

const destinationOptions = [
  { value: 'qobuz', label: 'Qobuz' },
  { value: 'tidal', label: 'Tidal' },
  { value: 'deezer', label: 'Deezer' },
  { value: 'soundcloud', label: 'SoundCloud' },
  { value: 'amazon', label: 'Amazon Music' },
]

const typeOptions = [
  { value: 'all', label: 'All items' },
  { value: 'tracks', label: 'Tracks' },
  { value: 'albums', label: 'Albums' },
  { value: 'artists', label: 'Artists' },
]

const qualityOptions = [
  { value: 'lossless', label: 'Lossless FLAC', desc: '16-bit / 44.1kHz CD quality', badge: 'Recommended' },
  { value: 'hires', label: 'Hi-Res FLAC', desc: '24-bit / up to 192kHz studio master', badge: 'Hi-Fi' },
  { value: 'high', label: 'Standard', desc: '320kbps MP3 / high AAC', badge: 'Compact' },
]

const limitOptions = [
  { value: 5, label: '5', sub: 'Sample' },
  { value: 25, label: '25', sub: 'Quick' },
  { value: 50, label: '50', sub: 'Medium' },
  { value: 100, label: '100', sub: 'Large' },
  { value: undefined, label: 'All', sub: '10k+' },
]

const accountOptions = computed(() => {
  if (!selectedDestination.value) return accounts.value
  return accounts.value.filter(
    (account) => account.service_name?.toLowerCase() === selectedDestination.value,
  )
})

watch(selectedDestination, () => {
  if (
    selectedAccountId.value !== undefined
    && !accountOptions.value.some((account) => account.id === selectedAccountId.value)
  ) {
    selectedAccountId.value = undefined
  }
})

function accountLabel(account: Account): string {
  const identity = account.display_name || account.email || `Account #${account.id}`
  const service = formatServiceName(account.service_name)
  return `${identity} (${service})`
}

function formatSizeMb(sizeMb: number): string {
  if (!sizeMb || sizeMb <= 0) return '0 MB'
  if (sizeMb >= 1024) return `${(sizeMb / 1024).toFixed(2)} GB`
  return `${Math.round(sizeMb)} MB`
}

function resetForm() {
  selectedService.value = 'all'
  selectedDestination.value = ''
  selectedAccountId.value = undefined
  selectedType.value = 'all'
  selectedQuality.value = 'lossless'
  selectedLimit.value = 5
  highPriority.value = true
  syncBeforeDownload.value = false
  isEstimating.value = false
  isSyncing.value = false
  error.value = null
  emptyMessage.value = null
  confirmation.value = null
}

function handleClose() {
  error.value = null
  emptyMessage.value = null
  confirmation.value = null
  emit('update:modelValue', false)
}

function buildRequest(): DownloadJobRequest {
  return {
    kind: 'favorites',
    sourceService: selectedService.value === 'all' ? undefined : selectedService.value,
    destinationService: selectedDestination.value || undefined,
    preferredAccountId: selectedAccountId.value,
    qualityPreference: selectedQuality.value,
    priority: highPriority.value ? 60 : 50,
    allowFallback: true,
    limit: selectedLimit.value,
    filter: {
      itemType: selectedType.value === 'all'
        ? undefined
        : (selectedType.value as DownloadJobRequest['filter']['itemType']),
    },
  }
}

async function syncFavoritesIfRequested(serviceParam: string | undefined, typeParam: string): Promise<void> {
  if (!syncBeforeDownload.value) return
  isSyncing.value = true
  try {
    if (serviceParam) {
      await syncFavorites(serviceParam, typeParam)
      return
    }
    for (const service of ['qobuz', 'spotify', 'tidal']) {
      try {
        await syncFavorites(service, typeParam)
      } catch (syncError) {
        console.warn(`Optional sync for ${service} skipped:`, syncError)
      }
    }
  } catch (syncError) {
    console.warn('Pre-download sync encountered notice:', syncError)
  } finally {
    isSyncing.value = false
  }
}

function startJob(request: DownloadJobRequest, estimate: DownloadJobEstimate) {
  const promise = createDownloadJob(request)
  // The modal closes before the promise resolves; the job continues in the
  // background and the panel picks it up through the job-progress event.
  handleClose()

  promise
    .then((created) => {
      const result: DownloadJobCreated = {
        job_id: created.jobId,
        total: estimate.total,
        message: `Download job #${created.jobId} created with ${estimate.total.toLocaleString()} candidate tracks (~${formatSizeMb(estimate.estimatedSizeMb)}).`,
      }
      emit('enqueued', result)
      toast.success('Download job created', result.message)
    })
    .catch((createError) => {
      const message = typeof createError === 'string' ? createError : (createError?.message || 'Failed to create the download job.')
      toast.error('Could not create the download job', message)
    })
}

async function submit() {
  if (isEstimating.value) return
  isEstimating.value = true
  error.value = null
  emptyMessage.value = null
  confirmation.value = null

  const request = buildRequest()
  const serviceParam = request.sourceService
  const typeParam = request.filter.itemType || ''

  try {
    await syncFavoritesIfRequested(serviceParam, typeParam)
    const estimate = await estimateDownloadJob(request)

    // The user may have closed the modal while the estimate was running.
    if (!props.modelValue) return

    if (estimate.total === 0) {
      emptyMessage.value = 'No tracks match these criteria. Adjust the source, type or limit and try again.'
      return
    }

    if (estimate.total > CONFIRM_THRESHOLD) {
      confirmation.value = { request, estimate }
      return
    }

    startJob(request, estimate)
  } catch (estimateError: any) {
    console.error('Failed to estimate download job:', estimateError)
    error.value = typeof estimateError === 'string'
      ? estimateError
      : (estimateError?.message || 'Failed to estimate the download job.')
  } finally {
    isEstimating.value = false
    isSyncing.value = false
  }
}

function confirmAndStart() {
  if (!confirmation.value) return
  const { request, estimate } = confirmation.value
  startJob(request, estimate)
}

async function loadAccounts() {
  try {
    accounts.value = await accountsApi.getAccounts()
  } catch (loadError) {
    // The override is optional: without the account list the job still works
    // with the account resolved by the backend.
    console.warn('Could not load accounts for the account override:', loadError)
  }
}

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return
    resetForm()
    void loadAccounts()
  },
  { immediate: true },
)
</script>
