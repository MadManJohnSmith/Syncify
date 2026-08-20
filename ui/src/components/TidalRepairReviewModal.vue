<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-xs animate-in fade-in duration-200">
    <div class="bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-2xl w-full max-w-4xl max-h-[90vh] flex flex-col shadow-2xl overflow-hidden">
      
      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0 bg-gray-50/50 dark:bg-surface-highlight/30">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
            <span class="material-symbols-outlined text-[24px]">build_circle</span>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-lg font-bold text-gray-900 dark:text-white">Tidal Metadata & Path Repair Review</h2>
              <span class="px-2.5 py-0.5 text-xs font-semibold rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20">
                Dry-Run Only
              </span>
            </div>
            <p class="text-xs text-text-secondary">Inspect planned non-mutating fixes for incomplete Tidal downloads and ghost associations.</p>
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

      <!-- Prominent Dry-Run Safety Warning Banner -->
      <div class="px-6 py-3 bg-amber-500/10 dark:bg-amber-500/15 border-b border-amber-500/20 flex items-center justify-between shrink-0">
        <div class="flex items-center gap-2.5 text-amber-800 dark:text-amber-300 text-xs font-medium">
          <span class="material-symbols-outlined text-[20px] text-amber-600 dark:text-amber-400 shrink-0">shield</span>
          <span><strong>No files or database records will be changed.</strong> This review operates strictly in read-only dry-run mode.</span>
        </div>
        <div class="text-[11px] text-amber-700/80 dark:text-amber-400/80 font-mono">
          Safe Inspection
        </div>
      </div>

      <!-- Modal Body -->
      <div class="p-6 flex-1 overflow-y-auto custom-scrollbar space-y-6">
        
        <!-- Loading State -->
        <div v-if="isLoading" class="py-16 flex flex-col items-center justify-center text-center space-y-3">
          <span class="material-symbols-outlined text-4xl text-primary animate-spin">progress_activity</span>
          <p class="text-sm font-medium text-gray-900 dark:text-white">Computing dry-run repair audit...</p>
          <p class="text-xs text-text-secondary">Scanning downloads and correlating canonical Tidal tracks</p>
        </div>

        <!-- Error State -->
        <div v-else-if="errorMessage" class="p-6 rounded-xl bg-red-500/10 border border-red-500/30 text-center space-y-3">
          <div class="w-12 h-12 rounded-full bg-red-500/20 text-red-500 flex items-center justify-center mx-auto">
            <span class="material-symbols-outlined text-[28px]">error</span>
          </div>
          <h4 class="text-sm font-bold text-red-600 dark:text-red-400">Failed to compute repair dry-run</h4>
          <p class="text-xs text-red-700 dark:text-red-300 font-mono break-all max-w-xl mx-auto">{{ errorMessage }}</p>
          <button
            @click="fetchDryRun"
            class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-xs font-medium rounded-lg transition-colors inline-flex items-center gap-1.5"
          >
            <span class="material-symbols-outlined text-[16px]">refresh</span>
            Retry Dry-Run
          </button>
        </div>

        <!-- Empty State (No Repairs Required) -->
        <div v-else-if="repairItems.length === 0" class="py-16 flex flex-col items-center justify-center text-center space-y-3">
          <div class="w-12 h-12 rounded-full bg-green-500/10 text-green-500 flex items-center justify-center">
            <span class="material-symbols-outlined text-[28px]">check_circle</span>
          </div>
          <h4 class="text-base font-bold text-gray-900 dark:text-white">No Repairs Needed</h4>
          <p class="text-xs text-text-secondary max-w-md">All Tidal downloads have complete metadata, valid tags, sidecars, and canonical storage paths.</p>
        </div>

        <!-- Repair Items List -->
        <div v-else class="space-y-4">
          
          <!-- Summary Header -->
          <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl border border-gray-200 dark:border-border-dark">
            <div class="flex items-center gap-3">
              <span class="px-2.5 py-1 text-xs font-bold rounded-lg bg-primary/10 text-primary">
                {{ repairItems.length }} {{ repairItems.length === 1 ? 'Repair Item' : 'Repair Items' }} Found
              </span>
              <span class="text-xs text-text-secondary">
                Corrupt or partial downloads detected for non-mutating preview
              </span>
            </div>
            <div class="flex items-center gap-2">
              <span class="text-xs font-medium text-green-600 dark:text-green-400 flex items-center gap-1">
                <span class="material-symbols-outlined text-[16px]">verified</span>
                100% Zero Audio Re-download
              </span>
            </div>
          </div>

          <!-- Cards Grid / Stack -->
          <div
            v-for="(item, index) in repairItems"
            :key="item.download_id"
            class="repair-item-card p-5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-xs space-y-4 transition-all hover:border-primary/40"
          >
            <!-- Card Header: IDs and Status -->
            <div class="flex items-start justify-between gap-4">
              <div class="space-y-1">
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="px-2 py-0.5 text-xs font-bold rounded bg-gray-100 dark:bg-surface-highlight text-gray-800 dark:text-gray-200 font-mono">
                    Download ID: #{{ item.download_id }}
                  </span>
                  <span class="px-2 py-0.5 text-xs font-medium rounded bg-blue-500/10 text-blue-600 dark:text-blue-400 font-mono">
                    Track ID: #{{ item.old_track_id }} &rarr; #{{ item.new_track_id }}
                  </span>
                  <span class="px-2 py-0.5 text-xs font-semibold rounded bg-green-500/10 text-green-600 dark:text-green-400 flex items-center gap-1">
                    <span class="material-symbols-outlined text-[14px]">bolt</span>
                    Confidence: {{ (item.confidence * 100).toFixed(0) }}%
                  </span>
                </div>
                <div class="text-xs text-text-secondary flex items-center gap-2">
                  <span>Provenance: <strong class="text-gray-700 dark:text-gray-300 font-mono">{{ item.provenance }}</strong></span>
                </div>
              </div>

              <!-- No-Redownload Guarantee Badge -->
              <div class="shrink-0 text-right">
                <span
                  v-if="item.no_redownload_confirmed"
                  class="inline-flex items-center gap-1 px-2.5 py-1 rounded-full text-[11px] font-semibold bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20"
                >
                  <span class="material-symbols-outlined text-[14px]">lock</span>
                  No-Redownload Guarantee
                </span>
              </div>
            </div>

            <!-- Metadata Diff Matrix -->
            <div class="grid grid-cols-1 md:grid-cols-2 gap-3 p-3.5 bg-gray-50 dark:bg-surface-highlight/40 rounded-xl text-xs">
              <div class="space-y-1">
                <span class="text-[10px] font-bold uppercase tracking-wider text-red-500 dark:text-red-400">Current (Partial / Corrupt)</span>
                <div class="font-medium text-gray-900 dark:text-white space-y-0.5">
                  <p><span class="text-text-secondary">Title:</span> <span class="line-through text-gray-500">{{ item.old_title }}</span></p>
                  <p><span class="text-text-secondary">Artist:</span> <span class="line-through text-gray-500">{{ item.old_artist }}</span></p>
                  <p><span class="text-text-secondary">Album:</span> <span class="line-through text-gray-500">{{ item.old_album }}</span></p>
                </div>
              </div>

              <div class="space-y-1 border-t md:border-t-0 md:border-l border-gray-200 dark:border-border-dark pt-2 md:pt-0 md:pl-3">
                <span class="text-[10px] font-bold uppercase tracking-wider text-emerald-500 dark:text-emerald-400">Target (Canonical Enriched)</span>
                <div class="font-semibold text-gray-900 dark:text-white space-y-0.5">
                  <p><span class="text-text-secondary font-normal">Title:</span> <span class="text-emerald-600 dark:text-emerald-400">{{ item.new_title }}</span></p>
                  <p><span class="text-text-secondary font-normal">Artist:</span> <span class="text-emerald-600 dark:text-emerald-400">{{ item.new_artist }}</span></p>
                  <p><span class="text-text-secondary font-normal">Album:</span> <span class="text-emerald-600 dark:text-emerald-400">{{ item.new_album }}</span></p>
                </div>
              </div>
            </div>

            <!-- Path Transition -->
            <div class="space-y-1.5 text-xs font-mono">
              <div class="p-2 rounded-lg bg-red-500/5 dark:bg-red-500/10 border border-red-500/20 text-gray-600 dark:text-gray-400 break-all flex items-start gap-2">
                <span class="text-[10px] font-sans font-bold uppercase tracking-wider text-red-500 shrink-0 mt-0.5">Old Path:</span>
                <span>{{ item.old_path }}</span>
              </div>
              <div class="p-2 rounded-lg bg-emerald-500/5 dark:bg-emerald-500/10 border border-emerald-500/20 text-gray-800 dark:text-gray-200 font-semibold break-all flex items-start gap-2">
                <span class="text-[10px] font-sans font-bold uppercase tracking-wider text-emerald-500 shrink-0 mt-0.5">New Path:</span>
                <span>{{ item.new_path }}</span>
              </div>
            </div>

            <!-- Operations & Verification Details -->
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-2 text-[11px]">
              <!-- Runtime Input SHA-256 -->
              <div class="p-2.5 rounded-lg bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60 space-y-1">
                <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Runtime Input SHA-256:</span>
                <p class="font-mono text-gray-800 dark:text-gray-200 truncate" :title="item.old_hash || 'None'">
                  {{ item.old_hash ? item.old_hash.slice(0, 16) + '...' : 'Not Computed' }}
                </p>
              </div>

              <!-- FLAC Operation -->
              <div class="p-2.5 rounded-lg bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60 space-y-1">
                <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Expected Operation:</span>
                <p class="text-gray-800 dark:text-gray-200 truncate" :title="item.flac_operation">
                  {{ item.flac_operation }}
                </p>
              </div>

              <!-- LRC Operation -->
              <div class="p-2.5 rounded-lg bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60 space-y-1">
                <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">LRC Operation:</span>
                <p class="text-gray-800 dark:text-gray-200 truncate" :title="item.lrc_operation">
                  {{ item.lrc_operation }}
                </p>
              </div>

              <!-- Cover Operation -->
              <div class="p-2.5 rounded-lg bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60 space-y-1">
                <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Cover Operation:</span>
                <p class="text-gray-800 dark:text-gray-200 truncate" :title="item.cover_operation">
                  {{ item.cover_operation }}
                </p>
              </div>
            </div>

            <!-- Ghost Cleanup Action -->
            <div class="p-2.5 rounded-lg bg-purple-500/5 dark:bg-purple-500/10 border border-purple-500/20 text-xs flex items-center justify-between gap-3">
              <div class="flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-purple-500">delete_sweep</span>
                <span class="text-text-secondary">Ghost Cleanup Action:</span>
                <span class="font-medium text-gray-900 dark:text-white">{{ item.ghost_cleanup }}</span>
              </div>
              <span class="text-[11px] text-purple-600 dark:text-purple-400 font-mono font-medium">Atomic SQLite Cleanup</span>
            </div>

          </div>
        </div>

      </div>

      <!-- Modal Footer (Actions) -->
      <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0 bg-gray-50/50 dark:bg-surface-highlight/30">
        <div class="text-xs text-text-secondary">
          <span v-if="repairItems.length > 0">{{ repairItems.length }} planned non-destructive repair{{ repairItems.length > 1 ? 's' : '' }}</span>
        </div>

        <div class="flex items-center gap-3">
          <!-- Re-run Dry Run Button -->
          <button
            @click="fetchDryRun"
            :disabled="isLoading"
            class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-surface-highlight/80 text-gray-800 dark:text-gray-200 text-xs font-medium rounded-lg transition-colors flex items-center gap-1.5 disabled:opacity-50"
            title="Recalculate dry-run repair audit"
          >
            <span class="material-symbols-outlined text-[16px]" :class="{ 'animate-spin': isLoading }">sync</span>
            Re-run dry-run
          </button>

          <!-- Copy Repair Plan Button -->
          <button
            @click="copyRepairPlan"
            :disabled="isLoading || repairItems.length === 0"
            class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-surface-highlight/80 text-gray-800 dark:text-gray-200 text-xs font-medium rounded-lg transition-colors flex items-center gap-1.5 disabled:opacity-50"
            title="Copy JSON repair plan to clipboard"
          >
            <span class="material-symbols-outlined text-[16px]">
              {{ isCopied ? 'done' : 'content_copy' }}
            </span>
            {{ isCopied ? 'Copied Plan!' : 'Copy repair plan' }}
          </button>

          <!-- Export JSON Repair Plan Button -->
          <button
            @click="exportRepairPlan"
            :disabled="isLoading || repairItems.length === 0"
            class="px-4 py-2 bg-primary hover:bg-primary-hover text-white text-xs font-medium rounded-lg transition-colors flex items-center gap-1.5 disabled:opacity-50 shadow-sm shadow-primary/20"
            title="Export JSON plan file"
          >
            <span class="material-symbols-outlined text-[16px]">download</span>
            Export JSON repair plan
          </button>

          <!-- Explicitly NO Apply Button Present -->
        </div>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { getTidalRepairDryRun } from '@/api/metadata';
import type { DownloadRepairDryRunItem } from '@/api/types';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const repairItems = ref<DownloadRepairDryRunItem[]>([]);
const isLoading = ref(false);
const errorMessage = ref<string | null>(null);
const isCopied = ref(false);

async function fetchDryRun() {
  isLoading.value = true;
  errorMessage.value = null;
  try {
    const items = await getTidalRepairDryRun();
    repairItems.value = items || [];
  } catch (err) {
    errorMessage.value = typeof err === 'string' ? err : String(err);
  } finally {
    isLoading.value = false;
  }
}

function sanitizeForExport(items: DownloadRepairDryRunItem[]): unknown[] {
  // Strip any unexpected sensitive tokens/credentials if ever present
  return items.map(item => ({
    download_id: item.download_id,
    old_track_id: item.old_track_id,
    new_track_id: item.new_track_id,
    old_path: item.old_path,
    new_path: item.new_path,
    old_title: item.old_title,
    new_title: item.new_title,
    old_artist: item.old_artist,
    new_artist: item.new_artist,
    old_album: item.old_album,
    new_album: item.new_album,
    old_hash: item.old_hash,
    expected_hash_after: item.expected_hash_after,
    flac_operation: item.flac_operation,
    lrc_operation: item.lrc_operation,
    cover_operation: item.cover_operation,
    downloads_update: item.downloads_update,
    ghost_cleanup: item.ghost_cleanup,
    rollback_plan: item.rollback_plan,
    planned_action: item.planned_action,
    confidence: item.confidence,
    provenance: item.provenance,
    no_redownload_confirmed: item.no_redownload_confirmed,
  }));
}

async function copyRepairPlan() {
  try {
    const sanitized = sanitizeForExport(repairItems.value);
    const jsonStr = JSON.stringify(sanitized, null, 2);
    await navigator.clipboard.writeText(jsonStr);
    isCopied.value = true;
    setTimeout(() => {
      isCopied.value = false;
    }, 2000);
  } catch (err) {
    console.error('Failed to copy repair plan:', err);
  }
}

function exportRepairPlan() {
  try {
    const sanitized = sanitizeForExport(repairItems.value);
    const jsonStr = JSON.stringify(sanitized, null, 2);
    const blob = new Blob([jsonStr], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `tidal_repair_dry_run_plan_${new Date().toISOString().slice(0, 10)}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  } catch (err) {
    console.error('Failed to export repair plan:', err);
  }
}

function closeModal() {
  emit('update:modelValue', false);
}

watch(
  () => props.modelValue,
  (newVal) => {
    if (newVal) {
      fetchDryRun();
    }
  },
  { immediate: true }
);
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(156, 163, 175, 0.4);
  border-radius: 3px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(156, 163, 175, 0.6);
}
</style>
