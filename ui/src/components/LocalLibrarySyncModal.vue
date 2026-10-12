<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <!-- Backdrop -->
    <div
      class="absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity"
      @click="handleClose"
    ></div>

    <!-- Modal Card -->
    <div class="relative w-full max-w-[680px] flex flex-col rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl overflow-hidden transform transition-all animate-in fade-in zoom-in-95 duration-200">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-5 border-b border-gray-100 dark:border-border-dark">
        <div class="flex flex-col gap-1">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-primary text-[24px]">folder_special</span>
            <h2 class="text-xl font-bold tracking-tight text-gray-900 dark:text-white">Local Library Sync</h2>
          </div>
          <p class="text-xs text-gray-500 dark:text-gray-400">
            Treat your local music folder as a first-class streaming service: scan, deduplicate, score metadata, and stream.
          </p>
        </div>
        <button
          @click="handleClose"
          :disabled="isSyncing"
          aria-label="Close modal"
          class="rounded-full p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-white/10 transition-colors disabled:opacity-50"
        >
          <span class="material-symbols-outlined text-[20px]">close</span>
        </button>
      </div>

      <!-- Error Banner -->
      <div v-if="error" class="mx-6 mt-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 text-xs flex items-center gap-2">
        <span class="material-symbols-outlined text-[16px]">error</span>
        <span class="flex-1">{{ error }}</span>
        <button @click="error = null" class="text-red-500 hover:text-red-700">
          <span class="material-symbols-outlined text-[14px]">close</span>
        </button>
      </div>

      <!-- Main Body -->
      <div class="p-6 flex flex-col gap-5">
        <!-- Directory path input -->
        <div class="flex flex-col gap-1.5">
          <label class="text-xs font-semibold text-gray-700 dark:text-gray-300">
            Music Folder Path
          </label>
          <div class="flex items-center gap-2">
            <input
              v-model="directoryPath"
              type="text"
              placeholder="Choose a folder or paste its path"
              :disabled="isSyncing"
              class="flex-1 px-3 py-2 text-xs rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-white/5 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/40 disabled:opacity-50"
            />
            <button
              type="button"
              data-testid="local-folder-picker"
              :disabled="isSyncing"
              class="px-3 py-2 rounded-lg border border-gray-200 dark:border-border-dark hover:bg-primary/10 text-sm text-gray-700 dark:text-white"
              @click="chooseDirectory"
            >Choose folder</button>
          </div>
        </div>

        <!-- Options -->
        <div class="flex flex-col sm:flex-row gap-4 p-3.5 rounded-xl bg-gray-50 dark:bg-white/5 border border-gray-100 dark:border-border-dark text-xs">
          <label class="flex items-center gap-2 cursor-pointer text-gray-700 dark:text-gray-300">
            <input
              type="checkbox"
              v-model="recursive"
              :disabled="isSyncing"
              class="rounded text-primary focus:ring-primary"
            />
            Scan subdirectories recursively
          </label>
          <label class="flex items-center gap-2 cursor-pointer text-gray-700 dark:text-gray-300">
            <input
              type="checkbox"
              v-model="autoResolveDuplicates"
              :disabled="isSyncing"
              class="rounded text-primary focus:ring-primary"
            />
            Auto-resolve duplicate clusters
          </label>
        </div>

        <!-- Sync Progress / Result Panel -->
        <div v-if="lastReport" class="p-4 rounded-xl bg-primary/5 border border-primary/20 flex flex-col gap-2.5">
          <div class="flex items-center justify-between text-xs font-bold text-gray-900 dark:text-white">
            <span class="flex items-center gap-1.5 text-primary">
              <span class="material-symbols-outlined text-[16px]">check_circle</span>
              Sync Report
            </span>
            <span class="text-gray-500 font-normal">{{ lastReport.directory }}</span>
          </div>

          <div class="grid grid-cols-2 sm:grid-cols-4 gap-2 text-center pt-1">
            <div class="p-2 rounded-lg bg-white dark:bg-surface-dark border border-gray-100 dark:border-border-dark">
              <div class="text-lg font-bold text-gray-900 dark:text-white">{{ lastReport.total_scanned }}</div>
              <div class="text-[10px] text-gray-500 uppercase tracking-wide">Files Scanned</div>
            </div>
            <div class="p-2 rounded-lg bg-white dark:bg-surface-dark border border-gray-100 dark:border-border-dark">
              <div class="text-lg font-bold text-green-600 dark:text-green-400">{{ lastReport.imported }}</div>
              <div class="text-[10px] text-gray-500 uppercase tracking-wide">Imported</div>
            </div>
            <div class="p-2 rounded-lg bg-white dark:bg-surface-dark border border-gray-100 dark:border-border-dark">
              <div class="text-lg font-bold text-amber-600 dark:text-amber-400">{{ lastReport.duplicates_matched }}</div>
              <div class="text-[10px] text-gray-500 uppercase tracking-wide">Streaming Linked</div>
            </div>
            <div class="p-2 rounded-lg bg-white dark:bg-surface-dark border border-gray-100 dark:border-border-dark">
              <div class="text-lg font-bold text-blue-600 dark:text-blue-400">{{ lastReport.duplicates_skipped }}</div>
              <div class="text-[10px] text-gray-500 uppercase tracking-wide">Skipped Existing</div>
            </div>
          </div>

          <div v-if="lastReport.errors && lastReport.errors.length > 0" class="text-[11px] text-red-600 dark:text-red-400">
            <span class="font-semibold">Notices:</span> {{ lastReport.errors.join('; ') }}
          </div>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="flex items-center justify-end gap-3 px-6 py-4 bg-gray-50 dark:bg-white/[0.02] border-t border-gray-100 dark:border-border-dark">
        <button
          @click="handleClose"
          :disabled="isSyncing"
          class="px-4 py-2 text-xs font-semibold rounded-lg text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-white/10 transition-colors disabled:opacity-50"
        >
          Cancel
        </button>
        <button
          @click="handleStartSync"
          :disabled="isSyncing || !directoryPath.trim()"
          class="px-5 py-2 text-xs font-semibold rounded-lg bg-primary text-white hover:bg-primary-hover transition-colors flex items-center gap-2 disabled:opacity-50"
        >
          <span v-if="isSyncing" class="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
          <span v-else class="material-symbols-outlined text-[16px]">sync</span>
          {{ isSyncing ? 'Syncing Local Library...' : 'Start Sync' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { syncLocalLibrary, type LocalSyncReport } from '../api/library';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'sync-completed', report: LocalSyncReport): void;
}>();

const directoryPath = ref('');
const recursive = ref(true);
const autoResolveDuplicates = ref(false);
const isSyncing = ref(false);
const error = ref<string | null>(null);
const lastReport = ref<LocalSyncReport | null>(null);

function handleClose() {
  if (isSyncing.value) return;
  emit('update:modelValue', false);
}

async function chooseDirectory() {
  try {
    const selected = await open({ directory: true, multiple: false, title: 'Select Music Library Folder' });
    if (typeof selected === 'string') directoryPath.value = selected;
  } catch (e) {
    error.value = `Could not open folder picker: ${String(e)}`;
  }
}

async function handleStartSync() {
  if (!directoryPath.value.trim()) return;

  isSyncing.value = true;
  error.value = null;

  try {
    const report = await syncLocalLibrary(directoryPath.value.trim(), {
      recursive: recursive.value,
      autoResolveDuplicates: autoResolveDuplicates.value,
    });

    lastReport.value = report;
    if (report.success) {
      emit('sync-completed', report);
    } else if (report.errors.length > 0) {
      error.value = report.errors[0];
    }
  } catch (e: any) {
    error.value = e?.message || String(e);
  } finally {
    isSyncing.value = false;
  }
}
</script>
