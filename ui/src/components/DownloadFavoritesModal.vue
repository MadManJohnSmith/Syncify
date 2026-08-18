<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <!-- Backdrop -->
    <div 
      class="absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity" 
      @click="handleClose"
    ></div>

    <!-- Modal Card -->
    <div class="relative w-full max-w-[620px] flex flex-col rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl overflow-hidden transform transition-all animate-in fade-in zoom-in-95 duration-200">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-5 border-b border-gray-100 dark:border-border-dark">
        <div class="flex items-center gap-3">
          <div class="h-10 w-10 flex items-center justify-center rounded-xl bg-primary/10 text-primary">
            <span class="material-symbols-outlined text-[24px]">favorite</span>
          </div>
          <div>
            <h2 class="text-lg font-bold tracking-tight text-gray-900 dark:text-white">Download Favorites</h2>
            <p class="text-xs text-gray-500 dark:text-gray-400">Batch download your library favorites with selective quality and service filters.</p>
          </div>
        </div>
        <button 
          @click="handleClose"
          :disabled="isProcessing"
          aria-label="Close modal" 
          class="rounded-full p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-white/10 transition-colors disabled:opacity-50"
        >
          <span class="material-symbols-outlined text-[20px]">close</span>
        </button>
      </div>

      <!-- Result Banners (Empty, Preflight, Enqueued) -->
      <div v-if="result" :class="[
        'mx-6 mt-4 p-4 rounded-xl border text-sm',
        result.total_candidates === 0
          ? 'bg-blue-500/10 border-blue-500/30 text-blue-900 dark:text-blue-200'
          : result.is_preflight 
            ? 'bg-amber-500/10 border-amber-500/30 text-amber-900 dark:text-amber-200' 
            : 'bg-green-500/10 border-green-500/30 text-green-700 dark:text-green-300'
      ]">
        <div class="flex items-start gap-3">
          <span class="material-symbols-outlined text-[22px] mt-0.5" :class="
            result.total_candidates === 0
              ? 'text-blue-500'
              : result.is_preflight ? 'text-amber-500' : 'text-green-500'
          ">
            {{ result.total_candidates === 0 ? 'info' : (result.is_preflight ? 'shield_lock' : 'check_circle') }}
          </span>
          <div class="flex-1">
            <div class="flex items-center justify-between">
              <p class="font-semibold mb-1" :class="
                result.total_candidates === 0
                  ? 'text-blue-900 dark:text-blue-100'
                  : result.is_preflight ? 'text-amber-900 dark:text-amber-100' : 'text-green-900 dark:text-green-200'
              ">
                {{ result.total_candidates === 0 ? 'No Matching Favorites' : (result.is_preflight ? 'Preflight Verification Guardrail' : 'Enqueued Successfully') }}
              </p>
              <span v-if="result.is_preflight" class="px-2 py-0.5 rounded text-[10px] font-bold bg-amber-500/20 text-amber-800 dark:text-amber-300 uppercase">
                Confirmation Required
              </span>
            </div>
            <p class="text-xs opacity-90 mb-2">{{ result.message }}</p>

            <div v-if="result.total_candidates > 0" class="grid grid-cols-7 gap-2 mt-2 pt-2 border-t border-current/10 text-xs">
              <div class="bg-black/5 dark:bg-white/5 rounded p-2 text-center">
                <div class="font-bold text-sm text-gray-700 dark:text-gray-200">{{ result.total_candidates }}</div>
                <div class="text-gray-500 dark:text-gray-400">Requested</div>
              </div>
              <div class="bg-black/5 dark:bg-white/5 rounded p-2 text-center">
                <div class="font-bold text-sm text-primary">{{ result.ready_exact ?? result.enqueued }}</div>
                <div class="text-gray-500 dark:text-gray-400">Ready Exact</div>
              </div>
              <div class="bg-black/5 dark:bg-white/5 rounded p-2 text-center">
                <div class="font-bold text-sm text-emerald-600 dark:text-emerald-400">{{ result.ready_fallback ?? 0 }}</div>
                <div class="text-gray-500 dark:text-gray-400">Fallback</div>
              </div>
              <div class="bg-black/5 dark:bg-white/5 rounded p-2 text-center">
                <div class="font-bold text-sm text-blue-600 dark:text-blue-400">{{ result.already_downloaded }}</div>
                <div class="text-gray-500 dark:text-gray-400">Downloaded</div>
              </div>
              <div class="bg-black/5 dark:bg-white/5 rounded p-2 text-center">
                <div class="font-bold text-sm text-purple-600 dark:text-purple-400">{{ result.already_queued }}</div>
                <div class="text-gray-500 dark:text-gray-400">In Queue</div>
              </div>
              <div class="bg-black/5 dark:bg-white/5 rounded p-2 text-center">
                <div class="font-bold text-sm text-gray-500 dark:text-gray-400">{{ result.no_download_provider ?? result.unresolved_sources ?? 0 }}</div>
                <div class="text-gray-500 dark:text-gray-400">No Provider</div>
              </div>
              <div class="bg-black/5 dark:bg-white/5 rounded p-2 text-center">
                <div class="font-bold text-sm text-amber-500">{{ (result.ambiguous_sources || 0) + (result.stale_sources || 0) }}</div>
                <div class="text-gray-500 dark:text-gray-400">Excluded</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Error Banner -->
      <div v-if="error" class="mx-6 mt-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 text-sm flex items-center gap-2">
        <span class="material-symbols-outlined text-[18px]">error</span>
        <span class="flex-1">{{ error }}</span>
        <button @click="error = null" class="text-red-500 hover:text-red-700">
          <span class="material-symbols-outlined text-[16px]">close</span>
        </button>
      </div>

      <!-- Form Content -->
      <div v-if="!result" class="p-6 space-y-5">
        <!-- Service Selection -->
        <div>
          <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2">
            Service Source
          </label>
          <div class="grid grid-cols-4 gap-2">
            <button 
              type="button"
              v-for="svc in serviceOptions" 
              :key="svc.value"
              @click="selectedService = svc.value"
              :class="[
                'flex flex-col items-center justify-center p-3 rounded-xl border text-xs font-medium transition-all gap-1.5',
                selectedService === svc.value 
                  ? 'bg-primary/10 border-primary text-primary shadow-sm font-semibold' 
                  : 'bg-gray-50 dark:bg-surface-highlight/50 border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600'
              ]"
            >
              <span class="material-symbols-outlined text-[20px]">{{ svc.icon }}</span>
              <span>{{ svc.label }}</span>
            </button>
          </div>
        </div>

        <!-- Entity Type Selection -->
        <div>
          <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2">
            Favorites Type
          </label>
          <div class="grid grid-cols-4 gap-2">
            <button 
              type="button"
              v-for="item in typeOptions" 
              :key="item.value"
              @click="selectedType = item.value"
              :class="[
                'flex flex-col items-center justify-center p-2.5 rounded-xl border text-xs font-medium transition-all gap-1',
                selectedType === item.value 
                  ? 'bg-primary/10 border-primary text-primary shadow-sm font-semibold' 
                  : 'bg-gray-50 dark:bg-surface-highlight/50 border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600'
              ]"
            >
              <span class="material-symbols-outlined text-[18px]">{{ item.icon }}</span>
              <span>{{ item.label }}</span>
            </button>
          </div>
        </div>

        <!-- Quality Preference Selection -->
        <div>
          <label class="block text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 mb-2">
            Quality Profile
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
                  ? 'bg-primary/10 border-primary text-primary shadow-sm' 
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
            Batch Limit (Controlled Sample)
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
                  ? 'bg-primary/10 border-primary text-primary shadow-sm font-semibold' 
                  : 'bg-gray-50 dark:bg-surface-highlight/50 border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:border-gray-300 dark:hover:border-gray-600'
              ]"
            >
              <span>{{ lim.label }}</span>
              <span class="text-[10px] text-text-secondary">{{ lim.sub }}</span>
            </button>
          </div>
        </div>

        <!-- Priority & Options -->
        <div class="space-y-2">
          <div class="flex items-center justify-between p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight/30 border border-gray-200 dark:border-border-dark">
            <div class="flex items-center gap-2">
              <span class="material-symbols-outlined text-[20px] text-primary">priority_high</span>
              <div>
                <span class="text-xs font-semibold text-gray-900 dark:text-white block">High Queue Priority</span>
                <span class="text-[11px] text-gray-500 dark:text-gray-400">Process favorites ahead of normal manual enqueued downloads.</span>
              </div>
            </div>
            <input 
              type="checkbox" 
              v-model="highPriority" 
              class="h-4 w-4 rounded text-primary focus:ring-primary border-gray-300 dark:border-gray-600 bg-white dark:bg-surface-dark"
            />
          </div>

          <div class="flex items-center justify-between p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight/30 border border-gray-200 dark:border-border-dark">
            <div class="flex items-center gap-2">
              <span class="material-symbols-outlined text-[20px] text-blue-500">sync</span>
              <div>
                <span class="text-xs font-semibold text-gray-900 dark:text-white block">Sync Cloud Favorites First</span>
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
        <button 
          v-if="!result || result.is_preflight"
          type="button" 
          @click="handleClose"
          :disabled="isProcessing"
          class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors disabled:opacity-50"
        >
          Cancel
        </button>

        <!-- Initial Enqueue / Preflight Button -->
        <button 
          v-if="!result"
          type="button" 
          @click="executeDownloadFavorites(false)"
          :disabled="isProcessing"
          class="flex items-center gap-2 px-5 py-2 text-sm font-semibold text-white bg-primary hover:bg-primary-hover rounded-lg shadow-sm transition-all disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <span v-if="isProcessing" class="material-symbols-outlined text-[18px] animate-spin">progress_activity</span>
          <span v-else class="material-symbols-outlined text-[18px]">{{ selectedLimit === undefined ? 'visibility' : 'download' }}</span>
          <span>{{ isProcessing ? (isSyncing ? 'Syncing Favorites...' : 'Checking Candidates...') : (selectedLimit === undefined ? 'Run Preflight Check' : 'Enqueue Downloads') }}</span>
        </button>

        <!-- Confirm Preflight Button -->
        <button 
          v-else-if="result.is_preflight"
          type="button" 
          @click="confirmMassEnqueue"
          :disabled="isProcessing || result.enqueued === 0"
          class="flex items-center gap-2 px-5 py-2 text-sm font-semibold text-white bg-amber-600 hover:bg-amber-700 rounded-lg shadow-sm transition-all disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <span v-if="isProcessing" class="material-symbols-outlined text-[18px] animate-spin">progress_activity</span>
          <span v-else class="material-symbols-outlined text-[18px]">verified</span>
          <span>{{ isProcessing ? 'Enqueuing...' : `Confirm & Enqueue (${result.enqueued} tracks)` }}</span>
        </button>

        <!-- Final Done Button -->
        <button 
          v-else
          type="button" 
          @click="handleClose"
          class="px-5 py-2 text-sm font-semibold text-white bg-primary hover:bg-primary-hover rounded-lg shadow-sm transition-all"
        >
          Done
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { downloadFavorites, syncFavorites, type DownloadFavoritesResult } from '../api/library'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'enqueued', result: DownloadFavoritesResult): void
}>()

const selectedService = ref<string>('all')
const selectedType = ref<string>('all')
const selectedQuality = ref<string>('lossless')
const selectedLimit = ref<number | undefined>(5)
const highPriority = ref<boolean>(true)
const syncBeforeDownload = ref<boolean>(false)

const isProcessing = ref<boolean>(false)
const isSyncing = ref<boolean>(false)
const error = ref<string | null>(null)
const result = ref<DownloadFavoritesResult | null>(null)

const serviceOptions = [
  { value: 'all', label: 'All Services', icon: 'hub' },
  { value: 'qobuz', label: 'Qobuz', icon: 'album' },
  { value: 'spotify', label: 'Spotify', icon: 'library_music' },
  { value: 'tidal', label: 'Tidal', icon: 'waves' },
]

const typeOptions = [
  { value: 'all', label: 'All Items', icon: 'select_all' },
  { value: 'tracks', label: 'Tracks', icon: 'music_note' },
  { value: 'albums', label: 'Albums', icon: 'album' },
  { value: 'artists', label: 'Artists', icon: 'person' },
]

const qualityOptions = [
  { value: 'lossless', label: 'Lossless FLAC', desc: '16-bit / 44.1kHz CD Quality', badge: 'Recommended' },
  { value: 'hires', label: 'Hi-Res FLAC', desc: '24-bit / up to 192kHz Studio Master', badge: 'Hi-Fi' },
  { value: 'standard', label: 'Standard', desc: '320kbps MP3 / High AAC', badge: 'Compact' },
]

const limitOptions = [
  { value: 5, label: '5', sub: 'Sample' },
  { value: 25, label: '25', sub: 'Quick' },
  { value: 50, label: '50', sub: 'Medium' },
  { value: 100, label: '100', sub: 'Large' },
  { value: undefined, label: 'All', sub: '10k+' },
]

function handleClose() {
  if (isProcessing.value) return
  error.value = null
  result.value = null
  emit('update:modelValue', false)
}

async function executeDownloadFavorites(isConfirmed: boolean = false) {
  isProcessing.value = true
  error.value = null
  result.value = null

  try {
    const priority = highPriority.value ? 60 : 50
    const serviceParam = selectedService.value === 'all' ? undefined : selectedService.value
    const typeParam = selectedType.value === 'all' ? undefined : selectedType.value

    if (syncBeforeDownload.value && !isConfirmed) {
      isSyncing.value = true
      try {
        if (serviceParam) {
          await syncFavorites(serviceParam, typeParam)
        } else {
          // Sync all connected services
          for (const s of ['qobuz', 'spotify', 'tidal']) {
            try {
              await syncFavorites(s, typeParam)
            } catch (e) {
              console.warn(`Optional sync for ${s} skipped:`, e)
            }
          }
        }
      } catch (syncErr: any) {
        console.warn('Pre-download sync encountered notice:', syncErr)
      } finally {
        isSyncing.value = false
      }
    }

    // Run with dry_run = true if mass batch and not confirmed
    const dryRunParam = (selectedLimit.value === undefined && !isConfirmed) ? true : false

    console.debug('[DownloadFavoritesModal] Invoking downloadFavorites with params:', {
      service: serviceParam,
      itemType: typeParam,
      qualityPreference: selectedQuality.value,
      priority,
      limit: selectedLimit.value,
      dryRun: dryRunParam,
    })

    const res = await downloadFavorites(
      serviceParam,
      typeParam,
      selectedQuality.value,
      priority,
      selectedLimit.value,
      dryRunParam
    )

    result.value = res
    if (!res.is_preflight) {
      emit('enqueued', res)
    }
  } catch (err: any) {
    console.error('Failed to download favorites:', err)
    error.value = typeof err === 'string' ? err : (err.message || 'Failed to process favorites download.')
  } finally {
    isProcessing.value = false
    isSyncing.value = false
  }
}

async function confirmMassEnqueue() {
  await executeDownloadFavorites(true)
}
</script>
