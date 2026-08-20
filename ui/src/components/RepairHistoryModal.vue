<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-xs animate-in fade-in duration-200">
    <div class="bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-2xl w-full max-w-4xl max-h-[90vh] flex flex-col shadow-2xl overflow-hidden">
      
      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0 bg-gray-50/50 dark:bg-surface-highlight/30">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-purple-500/10 text-purple-500 flex items-center justify-center">
            <span class="material-symbols-outlined text-[24px]">history_edu</span>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-lg font-bold text-gray-900 dark:text-white">Applied Repairs History</h2>
              <span class="px-2.5 py-0.5 text-xs font-semibold rounded-full bg-purple-500/10 text-purple-600 dark:text-purple-400 border border-purple-500/20">
                Audit Trail (Append-Only)
              </span>
            </div>
            <p class="text-xs text-text-secondary">Read-only cryptographic log of all executed repairs, hash verifications, and rollback events.</p>
          </div>
        </div>

        <button
          @click="closeModal"
          class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-white rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors"
          title="Close modal"
        >
          <span class="material-symbols-outlined text-[20px]">close</span>
        </button>
      </div>

      <!-- Controls & Filter Toolbar -->
      <div class="px-6 py-3 bg-gray-50 dark:bg-surface-highlight/20 border-b border-gray-200 dark:border-border-dark flex flex-wrap items-center justify-between gap-3 shrink-0">
        <div class="flex items-center gap-2 flex-1 min-w-[200px]">
          <div class="relative flex-1">
            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-[16px]">search</span>
            <input
              v-model="searchQuery"
              type="text"
              placeholder="Search by path, ID, or provenance..."
              class="w-full pl-9 pr-3 py-1.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-xs text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/40"
            />
          </div>
          <select
            v-model="filterResult"
            class="px-2.5 py-1.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark text-gray-900 dark:text-white text-xs rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/40"
          >
            <option value="all">All Results</option>
            <option value="success">Success Only</option>
            <option value="failed">Failed / Rollback</option>
          </select>
        </div>

        <div class="flex items-center gap-2 shrink-0">
          <button
            @click="fetchHistory"
            :disabled="isLoading"
            class="px-3 py-1.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-surface-highlight/80 text-gray-800 dark:text-gray-200 text-xs font-medium rounded-lg transition-colors flex items-center gap-1.5"
          >
            <span :class="['material-symbols-outlined text-[15px]', isLoading && 'animate-spin']">refresh</span>
            Refresh
          </button>
        </div>
      </div>

      <!-- Modal Body -->
      <div class="p-6 flex-1 overflow-y-auto custom-scrollbar space-y-6">
        
        <!-- Loading State -->
        <div v-if="isLoading" class="py-16 flex flex-col items-center justify-center text-center space-y-3">
          <span class="material-symbols-outlined text-4xl text-primary animate-spin">progress_activity</span>
          <p class="text-sm font-medium text-gray-900 dark:text-white">Loading repair history...</p>
          <p class="text-xs text-text-secondary">Retrieving audit records and cryptographic hashes</p>
        </div>

        <!-- Error State -->
        <div v-else-if="errorMessage" class="p-6 rounded-xl bg-red-500/10 border border-red-500/30 text-center space-y-3">
          <div class="w-12 h-12 rounded-full bg-red-500/20 text-red-500 flex items-center justify-center mx-auto">
            <span class="material-symbols-outlined text-[28px]">error</span>
          </div>
          <h4 class="text-sm font-bold text-red-600 dark:text-red-400">Failed to load repair history</h4>
          <p class="text-xs text-red-700 dark:text-red-300 font-mono break-all max-w-xl mx-auto">{{ errorMessage }}</p>
          <button
            @click="fetchHistory"
            class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-xs font-medium rounded-lg transition-colors inline-flex items-center gap-1.5"
          >
            <span class="material-symbols-outlined text-[16px]">refresh</span>
            Retry
          </button>
        </div>

        <!-- Empty State -->
        <div v-else-if="filteredRecords.length === 0" class="py-16 flex flex-col items-center justify-center text-center space-y-3">
          <div class="w-12 h-12 rounded-full bg-purple-500/10 text-purple-500 flex items-center justify-center">
            <span class="material-symbols-outlined text-[28px]">history_toggle_off</span>
          </div>
          <h4 class="text-base font-bold text-gray-900 dark:text-white">No Applied Repairs Found</h4>
          <p class="text-xs text-text-secondary max-w-md">
            {{ searchQuery ? 'No repair records match your filter criteria.' : 'No repairs have been executed yet. Applied repairs will automatically be logged here with full hash audit trails.' }}
          </p>
        </div>

        <!-- Records List -->
        <div v-else class="space-y-4">
          <!-- Summary Header -->
          <div class="flex items-center justify-between p-3.5 bg-gray-50 dark:bg-surface-highlight/30 rounded-xl border border-gray-200 dark:border-border-dark text-xs">
            <span class="font-bold text-gray-900 dark:text-white">
              Showing {{ filteredRecords.length }} {{ filteredRecords.length === 1 ? 'audit record' : 'audit records' }}
            </span>
            <span class="text-text-secondary flex items-center gap-1">
              <span class="material-symbols-outlined text-[15px] text-emerald-500">lock</span>
              Append-Only Immutability
            </span>
          </div>

          <!-- Cards Stack -->
          <div
            v-for="record in filteredRecords"
            :key="record.id"
            class="repair-history-card p-5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-xs space-y-4 transition-all hover:border-purple-500/40"
          >
            <!-- Header: Timestamp, Repair ID, Result -->
            <div class="flex items-start justify-between gap-3 flex-wrap">
              <div class="space-y-1">
                <div class="flex items-center gap-2 flex-wrap">
                  <!-- Result Badge -->
                  <span
                    :class="[
                      'px-2 py-0.5 text-xs font-bold rounded-md uppercase tracking-wider',
                      record.result === 'success'
                        ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20'
                        : 'bg-red-500/10 text-red-600 dark:text-red-400 border border-red-500/20'
                    ]"
                  >
                    {{ record.result }}
                  </span>

                  <!-- Repair ID -->
                  <span class="px-2 py-0.5 text-xs font-mono rounded bg-gray-100 dark:bg-surface-highlight text-gray-800 dark:text-gray-200">
                    {{ record.repair_id }}
                  </span>

                  <!-- Download ID / Track ID -->
                  <span v-if="record.download_id" class="px-2 py-0.5 text-xs font-mono rounded bg-blue-500/10 text-blue-600 dark:text-blue-400">
                    Download #{{ record.download_id }}
                  </span>
                  <span v-if="record.old_track_id && record.new_track_id" class="px-2 py-0.5 text-xs font-mono rounded bg-purple-500/10 text-purple-600 dark:text-purple-400">
                    Track #{{ record.old_track_id }} &rarr; #{{ record.new_track_id }}
                  </span>
                </div>

                <div class="text-[11px] text-text-secondary flex items-center gap-2">
                  <span class="flex items-center gap-1">
                    <span class="material-symbols-outlined text-[13px]">schedule</span>
                    {{ formatDate(record.timestamp) }}
                  </span>
                  <span>&bull;</span>
                  <span>Provenance: <strong class="text-gray-700 dark:text-gray-300 font-mono">{{ record.provenance }}</strong></span>
                </div>
              </div>

              <!-- Baseline Validation -->
              <div class="shrink-0 text-right">
                <span
                  :class="[
                    'inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium font-mono',
                    record.baseline_validation === 'valid'
                      ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                      : 'bg-amber-500/10 text-amber-600 dark:text-amber-400'
                  ]"
                >
                  <span class="material-symbols-outlined text-[13px]">{{ record.baseline_validation === 'valid' ? 'check' : 'warning' }}</span>
                  Baseline: {{ record.baseline_validation }}
                </span>
              </div>
            </div>

            <!-- Path Translation (Sanitized) -->
            <div class="p-3 bg-gray-50 dark:bg-surface-highlight/30 rounded-lg text-xs space-y-1.5">
              <div class="flex items-start gap-2">
                <span class="text-[10px] font-bold uppercase text-red-500 shrink-0 w-16">Old Path:</span>
                <span class="font-mono text-gray-700 dark:text-gray-300 break-all">{{ formatPathDisplay(record.old_path) }}</span>
              </div>
              <div class="flex items-start gap-2">
                <span class="text-[10px] font-bold uppercase text-emerald-500 shrink-0 w-16">New Path:</span>
                <span class="font-mono text-gray-900 dark:text-white font-medium break-all">{{ formatPathDisplay(record.new_path) }}</span>
              </div>
            </div>

            <!-- Cryptographic Hashes Matrix -->
            <div class="grid grid-cols-1 md:grid-cols-2 gap-3 p-3 bg-gray-50/70 dark:bg-surface-highlight/20 rounded-lg text-xs font-mono">
              <div class="space-y-1">
                <span class="text-[10px] font-bold uppercase tracking-wider text-text-secondary">File SHA-256</span>
                <div class="space-y-0.5 text-[11px]">
                  <p class="truncate"><span class="text-text-secondary">Before:</span> {{ record.input_file_hash }}</p>
                  <p class="truncate" :class="record.output_file_hash ? 'text-purple-600 dark:text-purple-400' : 'text-text-secondary'">
                    <span class="text-text-secondary">After: </span> {{ record.output_file_hash || 'N/A' }}
                  </p>
                </div>
              </div>

              <div class="space-y-1 border-t md:border-t-0 md:border-l border-gray-200 dark:border-border-dark pt-2 md:pt-0 md:pl-3">
                <span class="text-[10px] font-bold uppercase tracking-wider text-text-secondary flex items-center gap-1">
                  <span>Audio Payload (Frames)</span>
                  <span
                    v-if="record.audio_payload_hash_before && record.audio_payload_hash_after && record.audio_payload_hash_before === record.audio_payload_hash_after"
                    class="text-emerald-500 text-[10px] font-sans font-normal flex items-center gap-0.5"
                  >
                    <span class="material-symbols-outlined text-[12px]">verified</span> Invariant
                  </span>
                </span>
                <div class="space-y-0.5 text-[11px]">
                  <p class="truncate"><span class="text-text-secondary">Before:</span> {{ record.audio_payload_hash_before || 'N/A' }}</p>
                  <p class="truncate"><span class="text-text-secondary">After: </span> {{ record.audio_payload_hash_after || 'N/A' }}</p>
                </div>
              </div>
            </div>

            <!-- Actions Executed Badges -->
            <div v-if="record.actions && record.actions.length > 0" class="flex items-center gap-1.5 flex-wrap">
              <span class="text-[10px] font-bold uppercase text-text-secondary mr-1">Actions:</span>
              <span
                v-for="(action, aIdx) in record.actions"
                :key="aIdx"
                class="px-2 py-0.5 bg-gray-100 dark:bg-surface-highlight text-gray-800 dark:text-gray-200 text-[10px] font-mono rounded"
              >
                {{ action }}
              </span>
            </div>

            <!-- Rollback State if any -->
            <div v-if="record.rollback_state" class="p-2.5 bg-amber-500/10 border border-amber-500/20 rounded-lg text-xs text-amber-700 dark:text-amber-300 flex items-start gap-2">
              <span class="material-symbols-outlined text-[16px] text-amber-600 dark:text-amber-400 shrink-0">history</span>
              <div>
                <span class="font-bold">Rollback Event:</span>
                <span class="font-mono text-[11px] ml-1">{{ record.rollback_state }}</span>
              </div>
            </div>
          </div>
        </div>

      </div>

      <!-- Modal Footer -->
      <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0 bg-gray-50/50 dark:bg-surface-highlight/30">
        <div class="text-xs text-text-secondary flex items-center gap-1">
          <span class="material-symbols-outlined text-[16px] text-purple-500">security</span>
          Syncify Cryptographic Repair Audit v1.0
        </div>
        <button
          @click="closeModal"
          class="px-4 py-2 bg-gray-200 dark:bg-surface-highlight hover:bg-gray-300 dark:hover:bg-surface-highlight/80 text-gray-900 dark:text-white text-xs font-semibold rounded-lg transition-colors"
        >
          Close
        </button>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { getRepairHistory } from '@/api/metadata'
import type { RepairHistoryRecord } from '@/api/types'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const records = ref<RepairHistoryRecord[]>([])
const isLoading = ref(false)
const errorMessage = ref<string | null>(null)
const searchQuery = ref('')
const filterResult = ref<'all' | 'success' | 'failed'>('all')

function closeModal() {
  emit('update:modelValue', false)
}

function formatDate(timestamp: string): string {
  if (!timestamp) return 'N/A'
  try {
    const d = new Date(timestamp)
    if (isNaN(d.getTime())) return timestamp
    return d.toLocaleString()
  } catch {
    return timestamp
  }
}

function formatPathDisplay(path: string): string {
  if (!path) return 'N/A'
  // Remove absolute home directories for UI privacy while keeping relative music paths
  const normalized = path.replace(/\\/g, '/')
  const syncifyIdx = normalized.indexOf('Syncify/')
  if (syncifyIdx !== -1) {
    return normalized.substring(syncifyIdx)
  }
  const parts = normalized.split('/')
  if (parts.length > 3) {
    return '.../' + parts.slice(-3).join('/')
  }
  return normalized
}

async function fetchHistory() {
  isLoading.value = true
  errorMessage.value = null
  try {
    const res = await getRepairHistory(200, 0)
    records.value = res || []
  } catch (err: any) {
    errorMessage.value = typeof err === 'string' ? err : (err?.message || 'Failed to fetch repair history')
  } finally {
    isLoading.value = false
  }
}

const filteredRecords = computed(() => {
  let list = records.value
  if (filterResult.value !== 'all') {
    list = list.filter(r => r.result === filterResult.value)
  }
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase().trim()
    list = list.filter(r =>
      r.repair_id.toLowerCase().includes(q) ||
      r.old_path.toLowerCase().includes(q) ||
      r.new_path.toLowerCase().includes(q) ||
      r.provenance.toLowerCase().includes(q) ||
      (r.download_id && r.download_id.toString().includes(q)) ||
      (r.input_file_hash && r.input_file_hash.toLowerCase().includes(q))
    )
  }
  return list
})

watch(() => props.modelValue, (isOpen) => {
  if (isOpen) {
    fetchHistory()
  }
}, { immediate: true })
</script>
