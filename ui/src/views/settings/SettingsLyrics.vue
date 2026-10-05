<template>
  <div class="space-y-8">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Lyrics Sources (Priority Order)</h3>
      <p class="text-sm text-text-secondary">Syncify will try sources in this order. Drag a row, or use its arrow buttons, to reorder.</p>
      <div v-if="lyricsSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading lyrics providers...</span>
      </div>
<div v-else class="space-y-2">
        <DraggableItem
          v-for="(provider, i) in lyricsSettings.orderedProviders.value"
          :key="provider.provider_id"
          :text="provider.provider_name"
          :subtitle="`${provider.sync_level}-level sync`"
          :index="i + 1"
          :total="lyricsSettings.orderedProviders.value.length"
          @drag-start="handleDragStart"
          @reorder="handleDragReorder"
          @move-up="lyricsSettings.moveProviderUp(provider.provider_id)"
          @move-down="lyricsSettings.moveProviderDown(provider.provider_id)"
        >
          <template #actions>
            <button
              :disabled="!provider.enabled"
              :aria-label="provider.enabled ? `Disable ${provider.provider_name}` : `Enable ${provider.provider_name}`"
              @click.stop="lyricsSettings.toggleProvider(provider.provider_id)"
              class="p-1 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded transition-colors"
            >
              <span class="material-symbols-outlined text-[18px]" :class="provider.enabled ? 'text-emerald-500' : 'text-gray-400'">
                {{ provider.enabled ? 'toggle_on' : 'toggle_off' }}
              </span>
            </button>
          </template>
        </DraggableItem>
      </div>
    </section>

    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Lyrics Preferences</h3>
      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Minimum sync level</label>
          <select 
            :value="lyricsSettings.config.min_sync_level"
            @change="lyricsSettings.updateConfigField('min_sync_level', getEventValue($event))"
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
          >
            <option v-for="opt in lyricsSettings.syncLevelOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Preferred language</label>
          <select 
            :value="lyricsSettings.config.preferred_language"
            @change="lyricsSettings.updateConfigField('preferred_language', getEventValue($event))"
            class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
          >
            <option v-for="opt in lyricsSettings.languageOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
      </div>
    </section>
    
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Lyrics Storage</h3>
      <div>
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Storage format</label>
        <select 
          :value="lyricsSettings.config.storage_format"
          @change="lyricsSettings.updateConfigField('storage_format', getEventValue($event))"
          class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
        >
          <option v-for="opt in lyricsSettings.storageFormatOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
      </div>
      
      <BaseToggle
        title="Auto-fetch lyrics on import"
        subtitle="Automatically search for lyrics when adding new tracks"
        :checked="lyricsSettings.config.auto_fetch_on_import"
        test-id="toggle-auto-fetch-lyrics"
        @click="toggleAutoFetchLyrics"
      />
    </section>

    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Retry Behavior</h3>
      <BaseToggle
        title="Retry failed lookups"
        subtitle="Periodically retry tracks that failed to find lyrics"
        :checked="lyricsSettings.config.retry_failed"
        test-id="toggle-retry-failed"
        @click="toggleRetryFailed"
      />
      
      <div v-if="lyricsSettings.config.retry_failed">
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Retry frequency</label>
        <select 
          :value="lyricsSettings.config.retry_frequency"
          @change="lyricsSettings.updateConfigField('retry_frequency', getEventValue($event))"
          class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
        >
          <option v-for="opt in lyricsSettings.retryFrequencyOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useLyricsSettings } from '@/composables/useLyricsSettings'
import DraggableItem from '@/components/settings/DraggableItem.vue'
import BaseToggle from '@/components/settings/BaseToggle.vue'

const getEventValue = (e: any) => e.target?.value || ''

const lyricsSettings = useLyricsSettings()

const draggingIndex = ref<number | null>(null)

function handleDragStart(fromIndex: number) {
  draggingIndex.value = fromIndex
}

async function handleDragReorder(toIndex: number) {
  const fromIndex = draggingIndex.value
  draggingIndex.value = null
  if (fromIndex === null || fromIndex === toIndex) return
  const ordered = [...lyricsSettings.orderedProviders.value]
  if (fromIndex < 0 || fromIndex >= ordered.length) return
  if (toIndex < 0 || toIndex >= ordered.length) return
  const [moved] = ordered.splice(fromIndex, 1)
  ordered.splice(toIndex, 0, moved)
  await lyricsSettings.reorderProviders(ordered.map(p => p.provider_id))
}

async function toggleAutoFetchLyrics() {
  lyricsSettings.config.auto_fetch_on_import = !lyricsSettings.config.auto_fetch_on_import
  await lyricsSettings.saveConfig()
}

async function toggleRetryFailed() {
  lyricsSettings.config.retry_failed = !lyricsSettings.config.retry_failed
  await lyricsSettings.saveConfig()
}

onMounted(async () => {
  if (!lyricsSettings.orderedProviders.value || lyricsSettings.orderedProviders.value.length === 0) {
    await lyricsSettings.loadSettings()
  }
})
</script>
