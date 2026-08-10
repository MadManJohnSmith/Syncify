<template>
  <div class="space-y-8">
    <!-- Loading State -->
    <div v-if="advancedSettings.isLoading.value" class="flex items-center justify-center py-12">
      <div class="animate-spin rounded-full h-8 w-8 border-2 border-primary border-t-transparent"></div>
    </div>
    <template v-else>
      <!-- Logging Section -->
      <section class="space-y-4">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
          <span class="material-symbols-outlined text-primary">description</span> Logging
        </h3>
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
          <span class="font-medium text-gray-900 dark:text-white">Log Level</span>
          <select :value="advancedSettings.settings.log_level" @change="advancedSettings.updateField('log_level', getEventValue($event))" class="px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm">
            <option v-for="opt in advancedSettings.logLevelOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
        <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
          <span class="font-medium text-gray-900 dark:text-white">Log to File</span>
          <button @click="advancedSettings.updateField('log_to_file', !advancedSettings.settings.log_to_file)" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', advancedSettings.settings.log_to_file ? 'bg-primary' : 'bg-gray-300']">
            <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', advancedSettings.settings.log_to_file ? 'translate-x-6' : 'translate-x-1']"></span>
          </button>
        </div>
      </section>

      <!-- Workers Section -->
      <section class="space-y-4">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
          <span class="material-symbols-outlined text-primary">memory</span> Workers
        </h3>
        <div class="grid grid-cols-3 gap-4">
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2">Max Downloads</label>
            <input type="number" min="1" max="10" :value="advancedSettings.settings.max_concurrent_downloads" @change="updateClamped('max_concurrent_downloads', getEventValue($event), 1, 10)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2">Max Imports</label>
            <input type="number" min="1" max="5" :value="advancedSettings.settings.max_concurrent_imports" @change="updateClamped('max_concurrent_imports', getEventValue($event), 1, 5)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2">Timeout (sec)</label>
            <input type="number" min="30" max="600" :value="advancedSettings.settings.worker_timeout_seconds" @change="updateClamped('worker_timeout_seconds', getEventValue($event), 30, 600)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
        </div>
      </section>

       <!-- Cache Section -->
       <section class="space-y-4">
         <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
           <span class="material-symbols-outlined text-primary">cached</span> Cache
         </h3>
         <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
           <div class="flex-1">
             <span class="font-medium text-gray-900 dark:text-white block">Enable Cache</span>
             <span class="text-xs text-text-secondary">Improve performance by caching images and metadata</span>
           </div>
           <button @click="advancedSettings.updateField('cache_enabled', !advancedSettings.settings.cache_enabled)" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', advancedSettings.settings.cache_enabled ? 'bg-primary' : 'bg-gray-300']">
             <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', advancedSettings.settings.cache_enabled ? 'translate-x-6' : 'translate-x-1']"></span>
           </button>
         </div>
         
         <div v-if="advancedSettings.cacheStats.value && advancedSettings.cacheStats.value.length > 0" class="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-2">
           <div v-for="stat in advancedSettings.cacheStats.value" :key="stat.cache_type" class="p-3 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg flex justify-between items-center text-sm">
             <span class="text-gray-500 capitalize">{{ stat.cache_type }}</span>
             <span class="text-white font-medium">{{ (stat.size_bytes / (1024 * 1024)).toFixed(1) }} MB</span>
           </div>
         </div>
         
         <button @click="confirmClearCache" class="px-4 py-2 border border-warning/50 bg-warning/5 text-warning hover:bg-warning/10 rounded-lg text-sm font-medium transition-colors">Clear All Cache</button>
       </section>

      <!-- Network Section -->
      <section class="space-y-4">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
          <span class="material-symbols-outlined text-primary">wifi</span> Network
        </h3>
        <div class="grid grid-cols-3 gap-4">
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2">Timeout</label>
            <input type="number" min="5" max="300" :value="advancedSettings.settings.request_timeout_seconds" @change="updateClamped('request_timeout_seconds', getEventValue($event), 5, 300)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2">Retries</label>
            <input type="number" min="0" max="10" :value="advancedSettings.settings.max_retries" @change="updateClamped('max_retries', getEventValue($event), 0, 10)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2">Delay</label>
            <input type="number" min="1" max="60" :value="advancedSettings.settings.retry_delay_seconds" @change="updateClamped('retry_delay_seconds', getEventValue($event), 1, 60)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
        </div>
      </section>

       <!-- Debug Section -->
       <section class="space-y-4">
         <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
           <span class="material-symbols-outlined text-primary">bug_report</span> Debug
         </h3>
         
         <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
           <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
             <span class="font-medium text-gray-900 dark:text-white">Debug Mode</span>
             <button @click="advancedSettings.updateField('debug_mode', !advancedSettings.settings.debug_mode)" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', advancedSettings.settings.debug_mode ? 'bg-warning' : 'bg-gray-300']">
               <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', advancedSettings.settings.debug_mode ? 'translate-x-6' : 'translate-x-1']"></span>
             </button>
           </div>
           <div class="flex items-center justify-between p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
             <span class="font-medium text-gray-900 dark:text-white">API Logs</span>
             <button @click="advancedSettings.updateField('verbose_api_logging', !advancedSettings.settings.verbose_api_logging)" :class="['relative inline-flex h-6 w-11 items-center rounded-full transition-colors', advancedSettings.settings.verbose_api_logging ? 'bg-warning' : 'bg-gray-300']">
               <span :class="['inline-block h-4 w-4 transform rounded-full bg-white transition-transform', advancedSettings.settings.verbose_api_logging ? 'translate-x-6' : 'translate-x-1']"></span>
             </button>
           </div>
         </div>

         <!-- Diagnostic Results -->
         <div v-if="advancedSettings.diagnostics.value.length > 0" class="space-y-2 mt-4">
           <div v-for="diag in advancedSettings.diagnostics.value" :key="diag.check_name" class="p-3 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg flex items-center justify-between text-sm">
             <div class="flex items-center gap-3">
               <span class="material-symbols-outlined text-lg" :class="diag.status === 'ok' ? 'text-green-500' : 'text-red-500'">
                 {{ diag.status === 'ok' ? 'check_circle' : 'error' }}
               </span>
               <span class="text-gray-900 dark:text-white">{{ diag.check_name }}</span>
             </div>
             <span class="text-text-secondary text-xs">{{ diag.message }} ({{ diag.duration_ms }}ms)</span>
           </div>
         </div>

         <div class="flex gap-3 mt-4">
           <button @click="advancedSettings.runDiagnostics()" class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">Run Diagnostics</button>
           <button @click="confirmVacuum" class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium hover:bg-gray-200 dark:hover:bg-surface-highlight/80 transition-colors">Vacuum Database</button>
           <button @click="confirmResetAdvanced" class="px-4 py-2 border border-error/50 bg-error/5 text-error rounded-lg text-sm font-medium hover:bg-error/10 transition-colors">Reset Defaults</button>
         </div>
       </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import { watch, onMounted } from 'vue'
import { confirm } from '@tauri-apps/plugin-dialog'
import { useAdvancedSettings } from '@/composables/useAdvancedSettings'

const getEventValue = (e: any) => e.target?.value || ''

const advancedSettings = useAdvancedSettings()

// Clamp value to [min, max] range — prevents out-of-bounds persistence
function clampValue(val: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, val))
}

// Clamped update wrappers for numeric inputs
function updateClamped(field: string, raw: string, min: number, max: number) {
  const clamped = clampValue(Number(raw) || min, min, max)
  advancedSettings.updateField(field as any, clamped)
}

async function confirmClearCache() {
  const confirmed = await confirm('This will delete all cached images and metadata. Continue?', {
    title: 'Clear Cache',
    kind: 'warning'
  })
  if (confirmed !== true) return
  await advancedSettings.clearCache()
}

async function confirmVacuum() {
  const confirmed = await confirm('This will compact the database file. The app may be unresponsive briefly. Continue?', {
    title: 'Vacuum Database',
    kind: 'warning'
  })
  if (confirmed !== true) return
  await advancedSettings.vacuumDatabase()
}

async function confirmResetAdvanced() {
  const confirmed = await confirm('Reset all advanced settings to their default values?', {
    title: 'Reset Advanced Settings',
    kind: 'warning'
  })
  if (confirmed !== true) return
  await advancedSettings.resetToDefaults('advanced')
}

onMounted(async () => {
  if (!advancedSettings.settings.log_level) {
    await advancedSettings.loadSettings()
  }
  await advancedSettings.loadCacheStats()
})
</script>
