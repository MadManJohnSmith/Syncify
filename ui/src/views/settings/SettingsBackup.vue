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
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { exportLibrary, importLibrary, type ExportLibraryResult, type ImportLibraryResult } from '@/api/library'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const isExporting = ref(false)
const isImporting = ref(false)
const importFilePath = ref('')
const lastExportResult = ref<ExportLibraryResult | null>(null)
const lastImportResult = ref<ImportLibraryResult | null>(null)

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
</script>
