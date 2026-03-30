<template>
  <div class="space-y-8">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Duplicate Detection</h3>
      <div v-if="downloadSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading duplicate settings...</span>
      </div>
      <div v-else>
        <div class="flex items-center justify-between py-2 cursor-pointer" @click="toggleDuplicateDetection">
          <div>
            <span class="block text-sm font-medium text-gray-900 dark:text-white">Enable duplicate detection</span>
            <span class="block text-xs text-text-secondary mt-0.5">Automatically detect duplicate tracks in your library</span>
          </div>
          <div class="relative inline-block w-10 align-middle select-none">
            <div :class="duplicateSettings.enable_detection ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
            <div :class="duplicateSettings.enable_detection ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
          </div>
        </div>
        
        <div v-if="duplicateSettings.enable_detection" class="mt-4 space-y-3">
          <div class="flex items-center justify-between py-2 cursor-pointer" @click="togglePreferHigherQuality">
            <span class="text-sm text-gray-700 dark:text-gray-300">Prefer higher quality when deduplicating</span>
            <div class="relative inline-block w-10 align-middle select-none">
              <div :class="duplicateSettings.prefer_higher_quality ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
              <div :class="duplicateSettings.prefer_higher_quality ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
            </div>
          </div>
          
          <div class="flex items-center justify-between py-2 cursor-pointer" @click="togglePreferLossless">
            <span class="text-sm text-gray-700 dark:text-gray-300">Prefer lossless over lossy formats</span>
            <div class="relative inline-block w-10 align-middle select-none">
              <div :class="duplicateSettings.prefer_lossless ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
              <div :class="duplicateSettings.prefer_lossless ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
            </div>
          </div>
          
          <div class="flex items-center justify-between py-2 cursor-pointer" @click="toggleMoveToTrash">
            <div>
              <span class="text-sm text-gray-700 dark:text-gray-300">Move duplicates to trash</span>
              <span class="block text-xs text-text-secondary">If disabled, duplicates are deleted immediately</span>
            </div>
            <div class="relative inline-block w-10 align-middle select-none">
              <div :class="duplicateSettings.move_to_trash ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
              <div :class="duplicateSettings.move_to_trash ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
            </div>
          </div>
          
          <div class="mt-4 p-3 bg-gray-50 dark:bg-surface-highlight/30 rounded-lg">
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-2">Quality threshold (kbps difference to prefer)</label>
            <div class="flex items-center gap-3">
              <input 
                type="range" 
                min="0" max="256" step="32"
                :value="duplicateSettings.quality_threshold_kbps"
                @input="updateQualityThreshold(Number(getEventValue($event)))"
                class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary"
              >
              <span class="text-sm font-medium text-gray-900 dark:text-white w-16 text-right">{{ duplicateSettings.quality_threshold_kbps }} kbps</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useDownloadSettings } from '@/composables/useDownloadSettings'

const getEventValue = (e: any) => e.target?.value || ''

const downloadSettings = useDownloadSettings()
const duplicateSettings = computed(() => downloadSettings.duplicateSettings)

async function toggleDuplicateDetection() {
  downloadSettings.duplicateSettings.enable_detection = !downloadSettings.duplicateSettings.enable_detection
  await downloadSettings.saveDuplicateSettings()
}

async function togglePreferHigherQuality() {
  downloadSettings.duplicateSettings.prefer_higher_quality = !downloadSettings.duplicateSettings.prefer_higher_quality
  await downloadSettings.saveDuplicateSettings()
}

async function togglePreferLossless() {
  downloadSettings.duplicateSettings.prefer_lossless = !downloadSettings.duplicateSettings.prefer_lossless
  await downloadSettings.saveDuplicateSettings()
}

async function toggleMoveToTrash() {
  downloadSettings.duplicateSettings.move_to_trash = !downloadSettings.duplicateSettings.move_to_trash
  await downloadSettings.saveDuplicateSettings()
}

async function updateQualityThreshold(value: number) {
  downloadSettings.duplicateSettings.quality_threshold_kbps = value
  await downloadSettings.saveDuplicateSettings()
}

onMounted(async () => {
  if (!downloadSettings.isLoading.value && !downloadSettings.duplicateSettings.enable_detection && downloadSettings.duplicateSettings.quality_threshold_kbps === undefined) {
    await downloadSettings.loadSettings()
  }
})
</script>
