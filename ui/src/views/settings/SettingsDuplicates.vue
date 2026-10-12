<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <div class="pb-2 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <span class="material-symbols-outlined text-primary text-[22px]">content_copy</span>
            Duplicate Detection & Deduplication
          </h3>
          <p class="text-xs text-text-secondary mt-0.5">Configure how duplicate tracks are detected, compared, and resolved across your music collection</p>
        </div>
      </div>

      <!-- Live Library Duplicates Summary Card (DUP-01) -->
      <div class="p-4 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div class="flex items-start gap-3">
          <div class="w-10 h-10 rounded-lg flex items-center justify-center bg-primary/10 text-primary shrink-0 mt-0.5">
            <span class="material-symbols-outlined text-[24px]">library_music</span>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <span class="text-sm font-semibold text-gray-900 dark:text-white">Library Detection Status</span>
              <span v-if="isLoadingStats" class="text-xs text-text-secondary flex items-center gap-1">
                <span class="material-symbols-outlined animate-spin text-[14px]">progress_activity</span>
                Scanning...
              </span>
              <span
                v-else-if="duplicateCount !== null && duplicateCount > 0"
                class="px-2 py-0.5 text-xs font-bold rounded-full bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20"
              >
                {{ duplicateCount }} duplicate {{ duplicateCount === 1 ? 'track' : 'tracks' }} detected
              </span>
              <span
                v-else
                class="px-2 py-0.5 text-xs font-bold rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20"
              >
                No duplicates detected
              </span>
            </div>
            <p class="text-xs text-text-secondary mt-1">
              Syncify matches songs by Title and Primary Artist identity. Use the Library view to inspect duplicate clusters and apply auto-resolution.
            </p>
          </div>
        </div>

        <div class="flex items-center gap-2 shrink-0 self-end sm:self-center">
          <button
            type="button"
            @click="loadDuplicateStats"
            :disabled="isLoadingStats"
            class="px-3 py-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white border border-gray-200 dark:border-gray-700 rounded-lg hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-1 cursor-pointer disabled:opacity-50"
            title="Scan library database for current duplicate statistics"
          >
            <span class="material-symbols-outlined text-[16px]" :class="{ 'animate-spin': isLoadingStats }">refresh</span>
            Refresh
          </button>
          <button
            type="button"
            @click="navigateToLibrary"
            class="px-3 py-1.5 text-xs font-semibold bg-primary hover:bg-primary-hover text-on-accent rounded-lg transition-colors flex items-center gap-1 cursor-pointer shadow-sm shadow-primary/20"
            title="Open Library to review duplicates"
          >
            <span class="material-symbols-outlined text-[16px]">filter_list</span>
            View in Library
          </button>
        </div>
      </div>

      <!-- Settings List -->
      <div v-if="downloadSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary py-4">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading duplicate settings...</span>
      </div>

      <div v-else class="space-y-4">
        <!-- Master Toggle -->
        <div class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark flex items-center justify-between gap-4">
          <div class="flex-1 pr-2">
            <div class="flex items-center gap-2">
              <span class="text-sm font-semibold text-gray-900 dark:text-white">Enable duplicate detection</span>
              <button
                type="button"
                class="group relative inline-flex items-center text-gray-400 hover:text-primary transition-colors"
                title="Compares tracks by normalized title and primary artist ID. Detected duplicates are tagged in your library and displayed under the Duplicates filter for manual or automatic resolution."
              >
                <span class="material-symbols-outlined text-[16px]">help</span>
              </button>
            </div>
            <p class="text-xs text-text-secondary mt-0.5">
              Automatically scan and identify duplicate tracks in your library based on title and artist matching.
            </p>
          </div>
          <button
            type="button"
            role="switch"
            :aria-checked="duplicateSettings.enable_detection"
            @click="toggleDuplicateDetection"
            class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none"
            :class="duplicateSettings.enable_detection ? 'bg-primary' : 'bg-gray-200 dark:bg-surface-highlight'"
          >
            <span
              aria-hidden="true"
              class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
              :class="duplicateSettings.enable_detection ? 'translate-x-5' : 'translate-x-0'"
            />
          </button>
        </div>

        <!-- Detailed Rules (Active when enabled) -->
        <div v-if="duplicateSettings.enable_detection" class="p-5 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark space-y-5">
          <h4 class="text-xs font-semibold uppercase tracking-wider text-text-secondary pb-2 border-b border-gray-100 dark:border-border-dark/60">
            Deduplication Rules & Resolution Strategy
          </h4>

          <!-- Prefer Higher Quality -->
          <div class="flex items-center justify-between gap-4">
            <div class="flex-1 pr-2">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-gray-900 dark:text-white">Prefer higher quality when deduplicating</span>
                <button
                  type="button"
                  class="group relative inline-flex items-center text-gray-400 hover:text-primary transition-colors"
                  title="When deduplicating, tracks are compared acoustically. A 24-bit/96kHz or 16-bit/44.1kHz track is preserved over a 320 kbps or lower-quality track, ensuring maximum audio fidelity."
                >
                  <span class="material-symbols-outlined text-[16px]">help</span>
                </button>
              </div>
              <p class="text-xs text-text-secondary mt-0.5">
                Retains the copy with higher bit depth, sample rate, or bitrate when resolving duplicate pairs.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              :aria-checked="duplicateSettings.prefer_higher_quality"
              @click="togglePreferHigherQuality"
              class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none"
              :class="duplicateSettings.prefer_higher_quality ? 'bg-primary' : 'bg-gray-200 dark:bg-surface-highlight'"
            >
              <span
                aria-hidden="true"
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                :class="duplicateSettings.prefer_higher_quality ? 'translate-x-5' : 'translate-x-0'"
              />
            </button>
          </div>

          <!-- Prefer Lossless -->
          <div class="flex items-center justify-between gap-4">
            <div class="flex-1 pr-2">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-gray-900 dark:text-white">Prefer lossless formats (FLAC/ALAC)</span>
                <button
                  type="button"
                  class="group relative inline-flex items-center text-gray-400 hover:text-primary transition-colors"
                  title="Ensures lossless codec containers always take precedence over lossy compression, even if a lossy file advertises high bitrate numbers."
                >
                  <span class="material-symbols-outlined text-[16px]">help</span>
                </button>
              </div>
              <p class="text-xs text-text-secondary mt-0.5">
                Always keeps FLAC or ALAC files over lossy MP3/AAC/OGG formats during deduplication.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              :aria-checked="duplicateSettings.prefer_lossless"
              @click="togglePreferLossless"
              class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none"
              :class="duplicateSettings.prefer_lossless ? 'bg-primary' : 'bg-gray-200 dark:bg-surface-highlight'"
            >
              <span
                aria-hidden="true"
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                :class="duplicateSettings.prefer_lossless ? 'translate-x-5' : 'translate-x-0'"
              />
            </button>
          </div>

          <!-- Replace Different Source -->
          <div class="flex items-center justify-between gap-4">
            <div class="flex-1 pr-2">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-gray-900 dark:text-white">Replace same quality from different source</span>
                <button
                  type="button"
                  class="group relative inline-flex items-center text-gray-400 hover:text-primary transition-colors"
                  title="If enabled, duplicate tracks matching the exact same quality from another streaming service (such as Qobuz replacing Tidal) can replace the existing source entry."
                >
                  <span class="material-symbols-outlined text-[16px]">help</span>
                </button>
              </div>
              <p class="text-xs text-text-secondary mt-0.5">
                Allows updating tracks when another source provider offers the identical quality tier.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              :aria-checked="duplicateSettings.replace_same_quality_different_source"
              @click="toggleReplaceDifferentSource"
              class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none"
              :class="duplicateSettings.replace_same_quality_different_source ? 'bg-primary' : 'bg-gray-200 dark:bg-surface-highlight'"
            >
              <span
                aria-hidden="true"
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                :class="duplicateSettings.replace_same_quality_different_source ? 'translate-x-5' : 'translate-x-0'"
              />
            </button>
          </div>

          <!-- Delete Duplicates Immediately -->
          <div class="flex items-center justify-between gap-4">
            <div class="flex-1 pr-2">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-gray-900 dark:text-white">Delete duplicates immediately</span>
                <button
                  type="button"
                  class="group relative inline-flex items-center text-gray-400 hover:text-primary transition-colors"
                  title="If disabled, duplicate entries are unlinked from the database but files are left on disk. If enabled, redundant duplicate files are deleted or moved to trash upon resolution."
                >
                  <span class="material-symbols-outlined text-[16px]">help</span>
                </button>
              </div>
              <p class="text-xs text-text-secondary mt-0.5">
                Directly removes superseded duplicate files from disk and library database during resolution.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              :aria-checked="duplicateSettings.delete_duplicates_immediately"
              @click="toggleDeleteImmediately"
              class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none"
              :class="duplicateSettings.delete_duplicates_immediately ? 'bg-primary' : 'bg-gray-200 dark:bg-surface-highlight'"
            >
              <span
                aria-hidden="true"
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                :class="duplicateSettings.delete_duplicates_immediately ? 'translate-x-5' : 'translate-x-0'"
              />
            </button>
          </div>

          <!-- Move to Trash -->
          <div class="flex items-center justify-between gap-4">
            <div class="flex-1 pr-2">
              <div class="flex items-center gap-2">
                <span class="text-sm font-medium text-gray-900 dark:text-white">Move to trash instead of permanent deletion</span>
                <button
                  type="button"
                  class="group relative inline-flex items-center text-gray-400 hover:text-primary transition-colors"
                  title="Uses OS-level trash/recycle bin for safety, allowing you to restore removed audio files if needed instead of permanently wiping them."
                >
                  <span class="material-symbols-outlined text-[16px]">help</span>
                </button>
              </div>
              <p class="text-xs text-text-secondary mt-0.5">
                Sends removed duplicate files to the operating system recycle bin for safety.
              </p>
            </div>
            <button
              type="button"
              role="switch"
              :aria-checked="duplicateSettings.move_to_trash"
              @click="toggleMoveToTrash"
              class="relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none"
              :class="duplicateSettings.move_to_trash ? 'bg-primary' : 'bg-gray-200 dark:bg-surface-highlight'"
            >
              <span
                aria-hidden="true"
                class="pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out"
                :class="duplicateSettings.move_to_trash ? 'translate-x-5' : 'translate-x-0'"
              />
            </button>
          </div>

          <!-- Quality Difference Threshold -->
          <div class="pt-3 border-t border-gray-100 dark:border-border-dark/60">
            <div class="flex items-center justify-between mb-2">
              <div>
                <div class="flex items-center gap-2">
                  <label class="text-sm font-medium text-gray-900 dark:text-white">Quality Difference Threshold</label>
                  <button
                    type="button"
                    class="group relative inline-flex items-center text-gray-400 hover:text-primary transition-colors"
                    title="When comparing two lossy tracks, differences below this threshold (in kbps) are treated as equal quality, preventing unnecessary churn between very close bitrates."
                  >
                    <span class="material-symbols-outlined text-[16px]">help</span>
                  </button>
                </div>
                <p class="text-xs text-text-secondary mt-0.5">
                  Minimum bitrate delta (kbps) required to favor one lossy track over another.
                </p>
              </div>
              <span class="font-mono text-sm font-semibold text-primary px-2 py-0.5 rounded bg-primary/10">
                {{ duplicateSettings.quality_threshold_kbps || 64 }} kbps
              </span>
            </div>
            <input
              type="range"
              min="0"
              max="256"
              step="16"
              v-model.number="duplicateSettings.quality_threshold_kbps"
              @change="handleThresholdChange"
              class="w-full accent-primary cursor-pointer"
            />
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useDownloadSettings } from '@/composables/useDownloadSettings'

let router: any = null
try {
  router = useRouter()
} catch {
  // Graceful fallback for non-routed test mounts
  router = null
}

function navigateToLibrary() {
  if (router && typeof router.push === 'function') {
    router.push('/library')
  }
}

const downloadSettings = useDownloadSettings()
const duplicateSettings = downloadSettings.duplicateSettings

const duplicateCount = ref<number | null>(null)
const isLoadingStats = ref(false)

async function loadDuplicateStats() {
  isLoadingStats.value = true
  try {
    const stats = await invoke<number>('get_duplicate_stats')
    duplicateCount.value = typeof stats === 'number' ? stats : 0
  } catch (err) {
    console.warn('[SettingsDuplicates] Failed to query get_duplicate_stats:', err)
  } finally {
    isLoadingStats.value = false
  }
}

async function toggleDuplicateDetection() {
  duplicateSettings.enable_detection = !duplicateSettings.enable_detection
  await downloadSettings.saveDuplicateSettings()
}

async function togglePreferHigherQuality() {
  duplicateSettings.prefer_higher_quality = !duplicateSettings.prefer_higher_quality
  await downloadSettings.saveDuplicateSettings()
}

async function togglePreferLossless() {
  duplicateSettings.prefer_lossless = !duplicateSettings.prefer_lossless
  await downloadSettings.saveDuplicateSettings()
}

async function toggleReplaceDifferentSource() {
  duplicateSettings.replace_same_quality_different_source = !duplicateSettings.replace_same_quality_different_source
  await downloadSettings.saveDuplicateSettings()
}

async function toggleDeleteImmediately() {
  duplicateSettings.delete_duplicates_immediately = !duplicateSettings.delete_duplicates_immediately
  await downloadSettings.saveDuplicateSettings()
}

async function toggleMoveToTrash() {
  duplicateSettings.move_to_trash = !duplicateSettings.move_to_trash
  await downloadSettings.saveDuplicateSettings()
}

async function handleThresholdChange() {
  await downloadSettings.saveDuplicateSettings()
}

onMounted(async () => {
  await Promise.all([
    loadDuplicateStats(),
    downloadSettings.loadSettings().catch(() => {}),
  ])
})
</script>
