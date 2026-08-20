<template>
  <div class="space-y-6">
    <!-- Export Card -->
    <div class="p-6 rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm space-y-4">
      <div class="flex items-center gap-3">
        <div class="h-10 w-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
          <span class="material-symbols-outlined text-[24px]">file_upload</span>
        </div>
        <div>
          <h3 class="text-lg font-bold text-gray-900 dark:text-white">Export Library Backup</h3>
          <p class="text-sm text-text-secondary">Export your complete library, favorites, playlists, and metadata to a portable JSON backup file.</p>
        </div>
      </div>

      <div class="pt-2 flex items-center gap-3">
        <button 
          @click="handleExport"
          :disabled="isExporting"
          class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium shadow-md shadow-primary/20 transition-all disabled:opacity-50 flex items-center gap-2"
        >
          <span class="material-symbols-outlined text-[18px]" :class="{ 'animate-spin': isExporting }">upload</span>
          {{ isExporting ? 'Exporting...' : 'Export Backup File' }}
        </button>
      </div>

      <div v-if="lastExportResult" class="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-xl space-y-1">
        <p class="text-sm font-semibold text-emerald-600 dark:text-emerald-400 flex items-center gap-2">
          <span class="material-symbols-outlined text-[18px]">check_circle</span>
          Backup Exported Successfully
        </p>
        <p class="text-xs text-text-secondary font-mono truncate">Path: {{ lastExportResult.file_path }}</p>
        <p class="text-xs text-text-secondary">
          {{ lastExportResult.tracks_count }} tracks, {{ lastExportResult.albums_count }} albums, {{ lastExportResult.artists_count }} artists, {{ lastExportResult.playlists_count }} playlists ({{ (lastExportResult.file_size_bytes / 1024).toFixed(1) }} KB)
        </p>
      </div>
    </div>

    <!-- Import Card -->
    <div class="p-6 rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm space-y-4">
      <div class="flex items-center gap-3">
        <div class="h-10 w-10 rounded-xl bg-sky-500/10 text-sky-500 flex items-center justify-center">
          <span class="material-symbols-outlined text-[24px]">file_download</span>
        </div>
        <div>
          <h3 class="text-lg font-bold text-gray-900 dark:text-white">Import Library Backup</h3>
          <p class="text-sm text-text-secondary">Restore your library from a previous Syncify backup file (.json) with integrity verification and atomic rollback.</p>
        </div>
      </div>

      <div class="flex items-center gap-3">
        <input 
          type="text"
          v-model="importFilePath"
          placeholder="Enter or paste absolute backup file path..."
          class="flex-1 px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
        />
        <button 
          @click="handleImport"
          :disabled="isImporting || !importFilePath.trim()"
          class="px-5 py-2.5 bg-sky-600 hover:bg-sky-500 text-white rounded-lg text-sm font-medium transition-all disabled:opacity-50 flex items-center gap-2"
        >
          <span class="material-symbols-outlined text-[18px]" :class="{ 'animate-spin': isImporting }">download</span>
          {{ isImporting ? 'Importing...' : 'Restore Backup' }}
        </button>
      </div>

      <div v-if="lastImportResult" class="p-4 bg-emerald-500/10 border border-emerald-500/30 rounded-xl space-y-1">
        <p class="text-sm font-semibold text-emerald-600 dark:text-emerald-400 flex items-center gap-2">
          <span class="material-symbols-outlined text-[18px]">check_circle</span>
          {{ lastImportResult.message }}
        </p>
      </div>
    </div>

    <!-- S152A: Physical Library Integrity & Reconciliation Safety Gate -->
    <div class="p-6 rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm space-y-6">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="h-10 w-10 rounded-xl bg-amber-500/10 text-amber-500 flex items-center justify-center">
            <span class="material-symbols-outlined text-[24px]">verified_user</span>
          </div>
          <div>
            <h3 class="text-lg font-bold text-gray-900 dark:text-white">Physical Library Integrity & Reconciliation</h3>
            <p class="text-sm text-text-secondary">Audit and synchronize disk storage with database download records with safety gates, strict identity relinking, and atomic rollbacks.</p>
          </div>
        </div>
        <span class="px-3 py-1 text-xs font-semibold rounded-full bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20">
          S152A Safety Gate
        </span>
      </div>

      <!-- Controls Grid -->
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 p-4 bg-gray-50 dark:bg-surface-highlight rounded-xl">
        <!-- Execution Mode -->
        <div class="space-y-1.5">
          <label class="text-xs font-semibold uppercase tracking-wider text-text-secondary">Mode</label>
          <div class="flex rounded-lg bg-gray-200 dark:bg-surface-dark p-1">
            <button
              @click="reconcileOptions.dryRun = true"
              :class="['flex-1 py-1.5 text-xs font-medium rounded-md transition-all', reconcileOptions.dryRun ? 'bg-white dark:bg-surface-highlight text-primary shadow-xs' : 'text-text-secondary hover:text-white']"
            >
              DryRun (Audit)
            </button>
            <button
              @click="reconcileOptions.dryRun = false"
              :class="['flex-1 py-1.5 text-xs font-medium rounded-md transition-all', !reconcileOptions.dryRun ? 'bg-amber-600 text-white shadow-xs' : 'text-text-secondary hover:text-white']"
            >
              Apply (Execute)
            </button>
          </div>
        </div>

        <!-- Missing File Policy -->
        <div class="space-y-1.5">
          <label class="text-xs font-semibold uppercase tracking-wider text-text-secondary">Missing File Policy</label>
          <select
            v-model="reconcileOptions.missingFilePolicy"
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-xs font-medium text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
          >
            <option value="report_only">Report Only (Safe)</option>
            <option value="mark_missing">Mark Missing</option>
            <option value="delete_record">Delete Record (Destructive)</option>
          </select>
        </div>

        <!-- Orphan Policy -->
        <div class="space-y-1.5">
          <label class="text-xs font-semibold uppercase tracking-wider text-text-secondary">Orphan Policy</label>
          <select
            v-model="reconcileOptions.orphanPolicy"
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-xs font-medium text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
          >
            <option value="report_only">Report Only</option>
            <option value="relink_if_exact_identity">Relink Exact Identity (ISRC / ID)</option>
            <option value="ignore">Ignore Orphans</option>
          </select>
        </div>

        <!-- Staging Policy -->
        <div class="space-y-1.5">
          <label class="text-xs font-semibold uppercase tracking-wider text-text-secondary">Staging Policy</label>
          <select
            v-model="reconcileOptions.stagingPolicy"
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-xs font-medium text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
          >
            <option value="report_only">Report Only</option>
            <option value="purge_safe_residuals">Purge Safe Residuals</option>
          </select>
        </div>
      </div>

      <!-- Destructive Action Warning & Confirmation Gate -->
      <div v-if="!reconcileOptions.dryRun && reconcileOptions.missingFilePolicy === 'delete_record'" class="p-4 bg-red-500/10 border border-red-500/30 rounded-xl space-y-3">
        <div class="flex items-center gap-2 text-red-600 dark:text-red-400 font-semibold text-sm">
          <span class="material-symbols-outlined text-[20px]">warning</span>
          <span>Explicit Authorization Required for DeleteRecord</span>
        </div>
        <p class="text-xs text-text-secondary">
          Permanently deleting download database records cannot be undone without restoring from a backup. An automatic transactional backup will be recorded prior to mutation.
        </p>
        <label class="flex items-center gap-2 text-xs font-medium text-gray-900 dark:text-white cursor-pointer select-none">
          <input
            type="checkbox"
            v-model="reconcileOptions.confirmDelete"
            class="rounded border-gray-300 text-red-600 focus:ring-red-500"
          />
          <span>I explicitly authorize deleting orphaned/missing download records</span>
        </label>
      </div>

      <!-- Trigger Action Button -->
      <div class="flex items-center justify-between pt-2">
        <button
          @click="handleReconciliation"
          :disabled="isReconciling || (!reconcileOptions.dryRun && reconcileOptions.missingFilePolicy === 'delete_record' && !reconcileOptions.confirmDelete)"
          :class="[
            'px-5 py-2.5 rounded-lg text-sm font-medium transition-all disabled:opacity-50 flex items-center gap-2 shadow-sm',
            reconcileOptions.dryRun 
              ? 'bg-primary hover:bg-primary-hover text-white shadow-primary/20' 
              : 'bg-amber-600 hover:bg-amber-500 text-white shadow-amber-600/20'
          ]"
        >
          <span class="material-symbols-outlined text-[18px]" :class="{ 'animate-spin': isReconciling }">
            {{ isReconciling ? 'sync' : (reconcileOptions.dryRun ? 'find_in_page' : 'security') }}
          </span>
          {{ isReconciling ? 'Processing...' : (reconcileOptions.dryRun ? 'Audit Library Integrity' : 'Apply Reversible Reconciliation') }}
        </button>

        <div v-if="lastReconcileReport" class="flex items-center gap-2">
          <button
            @click="copyReportJson"
            class="px-3 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-surface-highlight/80 text-gray-700 dark:text-gray-300 rounded-lg text-xs font-medium transition-colors flex items-center gap-1.5"
            title="Copy full JSON report to clipboard"
          >
            <span class="material-symbols-outlined text-[16px]">content_copy</span>
            Copy JSON
          </button>
          <button
            @click="exportReportJson"
            class="px-3 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-surface-highlight/80 text-gray-700 dark:text-gray-300 rounded-lg text-xs font-medium transition-colors flex items-center gap-1.5"
            title="Download JSON report"
          >
            <span class="material-symbols-outlined text-[16px]">download</span>
            Download Report
          </button>
        </div>
      </div>

      <!-- Report View -->
      <div v-if="lastReconcileReport" class="p-5 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark rounded-xl space-y-4">
        <div class="flex items-center justify-between border-b border-gray-200 dark:border-border-dark pb-3">
          <div class="flex items-center gap-2">
            <span :class="['px-2.5 py-0.5 rounded-full text-xs font-bold uppercase tracking-wider', lastReconcileReport.dryRun ? 'bg-sky-500/10 text-sky-500 border border-sky-500/20' : 'bg-emerald-500/10 text-emerald-500 border border-emerald-500/20']">
              {{ lastReconcileReport.dryRun ? 'DryRun Report' : 'Applied Report' }}
            </span>
            <span class="text-xs text-text-secondary font-mono">{{ lastReconcileReport.timestamp }}</span>
          </div>
          <span v-if="lastReconcileReport.backupId" class="text-xs text-text-secondary font-mono">
            Backup ID: <strong class="text-gray-900 dark:text-white">{{ lastReconcileReport.backupId }}</strong>
          </span>
        </div>

        <!-- Metrics Strip -->
        <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <div class="p-3 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-center">
            <span class="text-[10px] uppercase font-bold text-text-secondary block">Verified Total</span>
            <span class="text-lg font-bold text-emerald-500">{{ lastReconcileReport.verifiedTotal }}</span>
          </div>
          <div class="p-3 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-center">
            <span class="text-[10px] uppercase font-bold text-text-secondary block">Missing Files</span>
            <span class="text-lg font-bold" :class="lastReconcileReport.missingFiles.length > 0 ? 'text-amber-500' : 'text-gray-500'">{{ lastReconcileReport.missingFiles.length }}</span>
          </div>
          <div class="p-3 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-center">
            <span class="text-[10px] uppercase font-bold text-text-secondary block">Exact Relinked</span>
            <span class="text-lg font-bold" :class="lastReconcileReport.relinkedOrphans > 0 ? 'text-primary' : 'text-gray-500'">{{ lastReconcileReport.relinkedOrphans }}</span>
          </div>
          <div class="p-3 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-center">
            <span class="text-[10px] uppercase font-bold text-text-secondary block">Ambiguous Orphans</span>
            <span class="text-lg font-bold" :class="lastReconcileReport.ambiguousOrphans.length > 0 ? 'text-purple-500' : 'text-gray-500'">{{ lastReconcileReport.ambiguousOrphans.length }}</span>
          </div>
        </div>

        <!-- Action Items Preview / Details -->
        <div v-if="lastReconcileReport.plannedActions && lastReconcileReport.plannedActions.length > 0" class="space-y-2">
          <h4 class="text-xs font-bold uppercase tracking-wider text-text-secondary">
            {{ lastReconcileReport.dryRun ? 'Proposed Actions Preview' : 'Executed Actions' }} ({{ lastReconcileReport.plannedActions.length }})
          </h4>
          <div class="max-h-48 overflow-y-auto space-y-1.5 pr-1">
            <div
              v-for="(action, idx) in (lastReconcileReport.dryRun ? lastReconcileReport.plannedActions : lastReconcileReport.executedActions)"
              :key="idx"
              class="p-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg flex items-center justify-between text-xs gap-2"
            >
              <div class="flex items-center gap-2 min-w-0">
                <span :class="['px-2 py-0.5 rounded text-[10px] font-mono font-bold uppercase shrink-0', 
                  action.actionType.includes('delete') ? 'bg-red-500/10 text-red-500' :
                  action.actionType.includes('relink') ? 'bg-emerald-500/10 text-emerald-500' :
                  action.actionType.includes('ambiguous') ? 'bg-purple-500/10 text-purple-500' :
                  'bg-gray-500/10 text-gray-400'
                ]">
                  {{ action.actionType }}
                </span>
                <span class="font-mono text-text-secondary truncate text-[11px]" :title="action.target">{{ action.target }}</span>
              </div>
              <span class="text-[11px] text-text-secondary shrink-0">{{ action.details }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from 'vue'
import {
  exportLibrary,
  importLibrary,
  reconcileLibraryPhysicalState,
  type ExportLibraryResult,
  type ImportLibraryResult,
  type LibraryReconciliationReport,
  type ReconciliationOptions,
} from '@/api/library'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const isExporting = ref(false)
const isImporting = ref(false)
const isReconciling = ref(false)
const importFilePath = ref('')
const lastExportResult = ref<ExportLibraryResult | null>(null)
const lastImportResult = ref<ImportLibraryResult | null>(null)
const lastReconcileReport = ref<LibraryReconciliationReport | null>(null)

const reconcileOptions = reactive<ReconciliationOptions>({
  dryRun: true,
  scope: { type: 'all' },
  missingFilePolicy: 'report_only',
  orphanPolicy: 'report_only',
  stagingPolicy: 'report_only',
  confirmDelete: false,
})

async function handleExport() {
  isExporting.value = true
  try {
    const res = await exportLibrary()
    lastExportResult.value = res
    toast.success(`Exported ${res.tracks_count} tracks to backup file`)
  } catch (e: any) {
    toast.error(`Export failed: ${e}`)
  } finally {
    isExporting.value = false
  }
}

async function handleImport() {
  if (!importFilePath.value.trim()) return
  isImporting.value = true
  try {
    const res = await importLibrary(importFilePath.value.trim())
    lastImportResult.value = res
    toast.success(res.message)
  } catch (e: any) {
    toast.error(`Import failed: ${e}`)
  } finally {
    isImporting.value = false
  }
}

async function handleReconciliation() {
  isReconciling.value = true
  try {
    const report = await reconcileLibraryPhysicalState({
      dryRun: reconcileOptions.dryRun,
      scope: reconcileOptions.scope,
      missingFilePolicy: reconcileOptions.missingFilePolicy,
      orphanPolicy: reconcileOptions.orphanPolicy,
      stagingPolicy: reconcileOptions.stagingPolicy,
      confirmDelete: reconcileOptions.confirmDelete,
    })
    lastReconcileReport.value = report
    if (report.dryRun) {
      toast.success(`Integrity audit complete: ${report.missingFiles.length} missing, ${report.orphanFiles.length} orphans`)
    } else {
      toast.success(`Reconciliation applied: ${report.purgedMissing} purged, ${report.relinkedOrphans} relinked`)
    }
  } catch (e: any) {
    toast.error(`Reconciliation failed: ${e}`)
  } finally {
    isReconciling.value = false
  }
}

function copyReportJson() {
  if (!lastReconcileReport.value) return
  navigator.clipboard.writeText(JSON.stringify(lastReconcileReport.value, null, 2))
  toast.success('Report copied to clipboard')
}

function exportReportJson() {
  if (!lastReconcileReport.value) return
  const blob = new Blob([JSON.stringify(lastReconcileReport.value, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `reconciliation_report_${lastReconcileReport.value.reportId}.json`
  a.click()
  URL.revokeObjectURL(url)
  toast.success('Report downloaded')
}
</script>
