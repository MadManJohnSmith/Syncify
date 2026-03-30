<template>
  <div class="error-handler">
    <!-- Network Error Banner -->
    <Transition name="slide-down">
      <div v-if="networkError" class="error-banner network-error fixed top-0 left-0 right-0 z-[90] bg-red-500 text-white px-4 py-3">
        <div class="max-w-screen-xl mx-auto flex items-center justify-between">
          <div class="flex items-center gap-3">
            <span class="material-symbols-outlined">wifi_off</span>
            <span class="font-medium">You're offline. Some features are unavailable.</span>
          </div>
          <div class="flex items-center gap-3">
            <button @click="retryConnection" class="px-4 py-1.5 bg-white/20 hover:bg-white/30 rounded-lg text-sm font-medium transition-colors">
              Retry Connection
            </button>
            <button @click="dismissNetworkError" class="p-1 hover:bg-white/20 rounded">
              <span class="material-symbols-outlined text-lg">close</span>
            </button>
          </div>
        </div>
      </div>
    </Transition>
    
    <!-- Rate Limit Warning Banner -->
    <Transition name="slide-down">
      <div v-if="rateLimitWarning" class="rate-limit-warning fixed top-0 left-0 right-0 z-[90] bg-amber-500 text-white px-4 py-3">
        <div class="max-w-screen-xl mx-auto flex items-center justify-between">
          <div class="flex items-center gap-3">
            <span class="material-symbols-outlined">speed</span>
            <span v-if="!rateLimited">API rate limit approaching ({{ rateLimitPercent }}% used)</span>
            <span v-else>Rate limit reached. Resuming in {{ rateLimitCountdown }}</span>
          </div>
          <div class="flex items-center gap-2">
            <button v-if="!rateLimited" @click="pauseOperations" class="px-4 py-1.5 bg-white/20 hover:bg-white/30 rounded-lg text-sm font-medium">
              Pause Operations
            </button>
            <button @click="dismissRateLimit" class="p-1 hover:bg-white/20 rounded">
              <span class="material-symbols-outlined text-lg">close</span>
            </button>
          </div>
        </div>
      </div>
    </Transition>
    
    <!-- Service Outage Toast -->
    <Transition name="slide-up">
      <div v-if="serviceOutage" class="service-error fixed bottom-6 right-6 z-[100] bg-white dark:bg-surface-dark rounded-xl shadow-2xl border border-gray-200 dark:border-border-dark p-4 w-80">
        <div class="flex items-start gap-3">
          <div class="w-10 h-10 rounded-lg bg-red-500/10 flex items-center justify-center shrink-0">
            <span class="material-symbols-outlined text-red-500">cloud_off</span>
          </div>
          <div class="flex-1">
            <p class="font-medium text-gray-900 dark:text-white">{{ serviceOutage.name }} is unreachable</p>
            <p class="text-sm text-gray-500 mt-0.5">Service may be experiencing issues</p>
            <div class="flex items-center gap-3 mt-3">
              <button @click="retryService(serviceOutage)" class="text-sm text-primary hover:underline">Retry</button>
              <a href="#" class="text-sm text-gray-500 hover:underline flex items-center gap-1">
                Status page <span class="material-symbols-outlined text-xs">open_in_new</span>
              </a>
            </div>
          </div>
          <button @click="serviceOutage = null" class="text-gray-400 hover:text-gray-600">
            <span class="material-symbols-outlined text-lg">close</span>
          </button>
        </div>
      </div>
    </Transition>
    
    <!-- Authentication Error Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="authError" class="auth-error fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="authError = null">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="p-6 text-center">
              <div class="w-16 h-16 mx-auto rounded-full bg-amber-500/10 flex items-center justify-center mb-4">
                <span class="material-symbols-outlined text-amber-500 text-3xl">key_off</span>
              </div>
              <h3 class="text-xl font-bold text-gray-900 dark:text-white mb-2">Authentication Required</h3>
              <p class="text-gray-500">Your {{ authError.service }} session has expired. Please reconnect to continue.</p>
            </div>
            <div class="px-6 pb-6 flex flex-col gap-2">
              <button @click="reconnectService(authError)" class="w-full py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl">
                Reconnect {{ authError.service }}
              </button>
              <button @click="disconnectService(authError)" class="w-full py-3 text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-xl">
                Disconnect
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Database Error Modal (Critical) -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="databaseError" class="database-error fixed inset-0 bg-black/80 flex items-center justify-center z-[250] p-8">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl">
            <div class="p-6">
              <div class="flex items-start gap-4">
                <div class="w-12 h-12 rounded-full bg-red-500/10 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-red-500 text-2xl">database</span>
                </div>
                <div>
                  <h3 class="text-xl font-bold text-gray-900 dark:text-white mb-2">Database Error</h3>
                  <p class="text-gray-500">Syncify's database is corrupted or inaccessible. Your data may be at risk.</p>
                </div>
              </div>
              
              <!-- Error Details -->
              <button @click="showDbErrorDetails = !showDbErrorDetails" class="w-full mt-4 p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg flex items-center justify-between text-sm">
                <span class="text-gray-600 dark:text-gray-400">View Error Details</span>
                <span class="material-symbols-outlined text-gray-400">{{ showDbErrorDetails ? 'expand_less' : 'expand_more' }}</span>
              </button>
              <Transition name="accordion">
                <div v-if="showDbErrorDetails" class="error-details mt-2 p-3 bg-gray-900 rounded-lg overflow-x-auto">
                  <p class="text-xs text-gray-400 mb-1">Error Code: {{ databaseError.code }}</p>
                  <pre class="text-xs text-red-400 font-mono whitespace-pre-wrap">{{ databaseError.message }}</pre>
                  <button @click="copyErrorDetails" class="mt-2 text-xs text-primary hover:underline flex items-center gap-1">
                    <span class="material-symbols-outlined text-sm">content_copy</span>
                    Copy Error Details
                  </button>
                </div>
              </Transition>
            </div>
            
            <div class="px-6 pb-6 space-y-2">
              <button @click="repairDatabase" class="w-full py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl flex items-center justify-center gap-2">
                <span class="material-symbols-outlined">build</span>
                Repair Database
              </button>
              <button v-if="hasBackup" @click="restoreBackup" class="w-full py-3 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-xl">
                Restore from Backup
              </button>
              <button @click="resetDatabase" class="w-full py-3 text-red-500 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-xl">
                Reset Database (Data Loss)
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Crash Recovery Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="crashRecovery" class="crash-recovery fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="p-6 text-center">
              <div class="w-16 h-16 mx-auto rounded-full bg-amber-500/10 flex items-center justify-center mb-4">
                <span class="material-symbols-outlined text-amber-500 text-3xl">warning</span>
              </div>
              <h3 class="text-xl font-bold text-gray-900 dark:text-white mb-2">Syncify crashed unexpectedly</h3>
              <p class="text-gray-500">We're sorry about that. Would you like to restore your previous session?</p>
            </div>
            <div class="px-6 pb-6 space-y-2">
              <button @click="restoreSession" class="w-full py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl">
                Restore Session
              </button>
              <button @click="startFresh" class="w-full py-3 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-xl">
                Start Fresh
              </button>
              <button @click="showErrorReport = true; crashRecovery = false" class="w-full py-3 text-gray-500 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-xl flex items-center justify-center gap-2">
                <span class="material-symbols-outlined text-lg">bug_report</span>
                Send Error Report
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Error Report Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showErrorReport" class="error-report fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="showErrorReport = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Help Us Fix This</h3>
              <button @click="showErrorReport = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">What were you doing when this happened?</label>
                <textarea v-model="errorReportMessage" rows="4" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50" placeholder="I was trying to..."></textarea>
              </div>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="includeErrorLogs" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-600 dark:text-gray-400">Include error logs</span>
              </label>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="includeSystemInfo" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-600 dark:text-gray-400">Include system information</span>
              </label>
            </div>
            <div class="px-6 pb-6">
              <button @click="sendErrorReport" class="w-full py-3 bg-primary hover:bg-primary-hover text-white font-semibold rounded-xl">
                Send Report
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Report Sent Success -->
    <Transition name="slide-up">
      <div v-if="reportSent" class="fixed bottom-6 right-6 z-[100] bg-green-500 text-white rounded-xl shadow-xl p-4 flex items-center gap-3">
        <span class="material-symbols-outlined">check_circle</span>
        <span class="font-medium">Report sent successfully. Thank you!</span>
      </div>
    </Transition>
    
    <!-- Auto-Retry Indicator -->
    <Transition name="slide-up">
      <div v-if="retrying" class="fixed bottom-6 right-6 z-[100] bg-white dark:bg-surface-dark rounded-xl shadow-2xl border border-gray-200 dark:border-border-dark p-4 w-72">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-lg bg-blue-500/10 flex items-center justify-center">
            <span class="material-symbols-outlined text-blue-500 animate-spin">sync</span>
          </div>
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">Retrying in {{ retryCountdown }}s...</p>
            <p class="text-xs text-gray-500">Attempt {{ retryAttempt }} of {{ maxRetries }}</p>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'

// Error States
const networkError = ref(false)
const serviceOutage = ref<{ name: string; id: string } | null>(null)
const authError = ref<{ service: string; id: string } | null>(null)
const databaseError = ref<{ code: string; message: string } | null>(null)
const crashRecovery = ref(false)
const showErrorReport = ref(false)
const showDbErrorDetails = ref(false)
const hasBackup = ref(true)

// Rate Limit
const rateLimitWarning = ref(false)
const rateLimited = ref(false)
const rateLimitPercent = ref(85)
const rateLimitCountdown = ref('9:45')

// Retry State
const retrying = ref(false)
const retryCountdown = ref(5)
const retryAttempt = ref(1)
const maxRetries = ref(3)

// Error Report
const errorReportMessage = ref('')
const includeErrorLogs = ref(true)
const includeSystemInfo = ref(true)
const reportSent = ref(false)

// Methods
function dismissNetworkError() {
  networkError.value = false
}

function retryConnection() {
  // Simulate retry
  networkError.value = false
}

function dismissRateLimit() {
  rateLimitWarning.value = false
}

function pauseOperations() {
  rateLimitWarning.value = false
}

function retryService(service: any) {
  serviceOutage.value = null
  // Retry service connection
}

function reconnectService(auth: any) {
  authError.value = null
  // Open OAuth flow
}

function disconnectService(auth: any) {
  authError.value = null
  // Remove service
}

function repairDatabase() {
  // Attempt database repair
  databaseError.value = null
}

function restoreBackup() {
  databaseError.value = null
}

function resetDatabase() {
  if (confirm('Are you sure? This will delete all your data.')) {
    databaseError.value = null
  }
}

function copyErrorDetails() {
  if (databaseError.value) {
    navigator.clipboard.writeText(`Error Code: ${databaseError.value.code}\n${databaseError.value.message}`)
  }
}

function restoreSession() {
  crashRecovery.value = false
}

function startFresh() {
  crashRecovery.value = false
}

function sendErrorReport() {
  showErrorReport.value = false
  reportSent.value = true
  setTimeout(() => reportSent.value = false, 3000)
}

// Demo triggers (for testing)
function triggerNetworkError() {
  networkError.value = true
}

function triggerServiceOutage(name: string) {
  serviceOutage.value = { name, id: name.toLowerCase() }
}

function triggerAuthError(service: string) {
  authError.value = { service, id: service.toLowerCase() }
}

function triggerDatabaseError() {
  databaseError.value = {
    code: 'ERR_DB_CORRUPT',
    message: 'SQLITE_CORRUPT: database disk image is malformed\nat Database.exec (sqlite3.js:123)\nat syncify-core::database::init (lib.rs:45)'
  }
}

function triggerCrashRecovery() {
  crashRecovery.value = true
}

// Expose for external use
defineExpose({
  triggerNetworkError,
  triggerServiceOutage,
  triggerAuthError,
  triggerDatabaseError,
  triggerCrashRecovery,
  networkError,
  serviceOutage,
  authError,
  databaseError
})
</script>

<style scoped>
/* Animations */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.3s ease;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-100%);
}

.slide-up-enter-active,
.slide-up-leave-active {
  transition: all 0.3s ease;
}

.slide-up-enter-from,
.slide-up-leave-to {
  opacity: 0;
  transform: translateY(20px);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.accordion-enter-active,
.accordion-leave-active {
  transition: all 0.2s ease;
  overflow: hidden;
}

.accordion-enter-from,
.accordion-leave-to {
  opacity: 0;
  max-height: 0;
}

/* Spin animation */
@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 1s linear infinite;
}
</style>
