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
          <label for="advanced-log-level" class="font-medium text-gray-900 dark:text-white">Log Level</label>
          <select id="advanced-log-level" :value="advancedSettings.settings.log_level" @change="advancedSettings.updateField('log_level', getEventValue($event))" class="px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm">
            <option v-for="opt in advancedSettings.logLevelOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
        <BaseToggle
          title="Log to File"
          subtitle="Write the application log next to your data directory"
          :checked="advancedSettings.settings.log_to_file"
          @click="advancedSettings.updateField('log_to_file', !advancedSettings.settings.log_to_file)"
        />
      </section>

      <!-- Workers Section -->
      <section class="space-y-4">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
          <span class="material-symbols-outlined text-primary">memory</span> Workers
        </h3>
        <div class="grid grid-cols-3 gap-4">
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2" for="advanced-max-downloads">Max Downloads</label>
            <input type="number" min="1" max="10" id="advanced-max-downloads" :value="advancedSettings.settings.max_concurrent_downloads" @change="updateClamped('max_concurrent_downloads', getEventValue($event), 1, 10)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2" for="advanced-max-imports">Max Imports</label>
            <input type="number" min="1" max="5" id="advanced-max-imports" :value="advancedSettings.settings.max_concurrent_imports" @change="updateClamped('max_concurrent_imports', getEventValue($event), 1, 5)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2" for="advanced-worker-timeout">Timeout (sec)</label>
            <input type="number" min="30" max="600" id="advanced-worker-timeout" :value="advancedSettings.settings.worker_timeout_seconds" @change="updateClamped('worker_timeout_seconds', getEventValue($event), 30, 600)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
        </div>
      </section>

       <!-- Cache Section -->
       <section class="space-y-4">
         <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
           <span class="material-symbols-outlined text-primary">cached</span> Cache
         </h3>
<BaseToggle
           title="Enable Cache"
           subtitle="Improve performance by caching images and metadata"
           :checked="advancedSettings.settings.cache_enabled"
           @click="advancedSettings.updateField('cache_enabled', !advancedSettings.settings.cache_enabled)"
         />

       </section>

      <!-- Network Section -->
      <section class="space-y-4">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
          <span class="material-symbols-outlined text-primary">wifi</span> Network
        </h3>
        <div class="grid grid-cols-3 gap-4">
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2" for="advanced-request-timeout">Timeout</label>
            <input type="number" min="5" max="300" id="advanced-request-timeout" :value="advancedSettings.settings.request_timeout_seconds" @change="updateClamped('request_timeout_seconds', getEventValue($event), 5, 300)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2" for="advanced-max-retries">Retries</label>
            <input type="number" min="0" max="10" id="advanced-max-retries" :value="advancedSettings.settings.max_retries" @change="updateClamped('max_retries', getEventValue($event), 0, 10)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
          <div class="p-4 bg-gray-50 dark:bg-surface-highlight rounded-lg">
            <label class="text-sm font-medium text-gray-900 dark:text-white block mb-2" for="advanced-retry-delay">Delay</label>
            <input type="number" min="1" max="60" id="advanced-retry-delay" :value="advancedSettings.settings.retry_delay_seconds" @change="updateClamped('retry_delay_seconds', getEventValue($event), 1, 60)" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm"/>
          </div>
        </div>
      </section>

       <!-- Debug Section -->
       <section class="space-y-4">
         <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center gap-2">
           <span class="material-symbols-outlined text-primary">bug_report</span> Debug
         </h3>
         
         <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
           <BaseToggle
             title="Debug Mode"
             :checked="advancedSettings.settings.debug_mode"
             @click="advancedSettings.updateField('debug_mode', !advancedSettings.settings.debug_mode)"
           />
           <BaseToggle
             title="API Logs"
             :checked="advancedSettings.settings.verbose_api_logging"
             @click="advancedSettings.updateField('verbose_api_logging', !advancedSettings.settings.verbose_api_logging)"
           />
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
           <button @click="advancedSettings.runDiagnostics()" class="px-4 py-2 bg-primary hover:bg-primary-hover text-on-accent rounded-lg text-sm font-medium transition-colors">Run Diagnostics</button>
           <button @click="runBatchHealthCheck" :disabled="advancedSettings.isRunningBatchHealthCheck.value" class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium hover:bg-gray-200 dark:hover:bg-surface-highlight/80 transition-colors disabled:opacity-50">Batch Health Check</button>
           <button @click="confirmVacuum" class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium hover:bg-gray-200 dark:hover:bg-surface-highlight/80 transition-colors">Vacuum Database</button>
           <button @click="confirmResetAdvanced" class="px-4 py-2 border border-error/50 bg-error/5 text-error rounded-lg text-sm font-medium hover:bg-error/10 transition-colors">Reset Defaults</button>
         </div>
       </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import { watch, onMounted } from 'vue'
// R13: confirm unificado (antes plugin-dialog nativo, fuera de la ventana).
import { confirm } from '@/composables/useToast'
import { useAdvancedSettings } from '@/composables/useAdvancedSettings'
import BaseToggle from '@/components/settings/BaseToggle.vue'

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

async function runBatchHealthCheck() {
  try {
    await advancedSettings.runBatchHealthCheck()
  } catch (e) {
    console.error('Batch health check failed:', e)
  }
}

async function confirmVacuum() {
  const confirmed = await confirm('This will compact the database file. The app may be unresponsive briefly. Continue?', {
    title: 'Vacuum Database',
    variant: 'warning'
  })
  if (confirmed !== true) return
  await advancedSettings.vacuumDatabase()
}

async function confirmResetAdvanced() {
  const confirmed = await confirm('Reset all advanced settings to their default values?', {
    title: 'Reset Advanced Settings',
    variant: 'warning'
  })
  if (confirmed !== true) return
  await advancedSettings.resetToDefaults('advanced')
}

onMounted(async () => {
  if (!advancedSettings.settings.log_level) {
    await advancedSettings.loadSettings()
  }
})
</script>
