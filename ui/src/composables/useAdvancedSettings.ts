import { ref, reactive } from 'vue'
import { settingsApi } from '@/api/settings'
import type { AdvancedSettings, CacheStats, DiagnosticResult } from '@/api/types'

/**
 * Composable for managing advanced application settings
 */
export function useAdvancedSettings() {
    const isLoading = ref(false)
    const isSaving = ref(false)
    const error = ref<string | null>(null)

    // Settings state
    const settings = reactive<AdvancedSettings>({
        id: 1,
        // Logging
        log_level: 'info',
        log_to_file: true,
        log_file_max_size_mb: 50,
        log_file_retention_days: 30,
        // Workers
        max_concurrent_downloads: 3,
        max_concurrent_imports: 2,
        worker_timeout_seconds: 300,
        // Cache
        cache_enabled: true,
        cache_max_size_mb: 500,
        cache_ttl_hours: 168,
        // Matching
        fuzzy_match_threshold: 0.85,
        use_acoustic_fingerprinting: true,
        prefer_exact_matches: true,
        // Network
        request_timeout_seconds: 30,
        max_retries: 3,
        retry_delay_seconds: 5,
        use_proxy: false,
        proxy_url: null,
        // Debug
        debug_mode: false,
        verbose_api_logging: false,
    })

    // Cache stats
    const cacheStats = ref<CacheStats[]>([])

    // Diagnostic results
    const diagnostics = ref<DiagnosticResult[]>([])
    const isRunningDiagnostics = ref(false)

    // Load settings from backend
    async function loadSettings() {
        isLoading.value = true
        error.value = null
        try {
            const data = await settingsApi.getAdvancedSettings()
            Object.assign(settings, data)
        } catch (e) {
            error.value = e instanceof Error ? e.message : 'Failed to load settings'
            console.error('Failed to load advanced settings:', e)
        } finally {
            isLoading.value = false
        }
    }

    // Save settings to backend
    async function saveSettings() {
        isSaving.value = true
        error.value = null
        try {
            const updated = await settingsApi.updateAdvancedSettings({ ...settings })
            Object.assign(settings, updated)
        } catch (e) {
            error.value = e instanceof Error ? e.message : 'Failed to save settings'
            console.error('Failed to save advanced settings:', e)
        } finally {
            isSaving.value = false
        }
    }

    // Update a single field and auto-save
    async function updateField<K extends keyof AdvancedSettings>(
        field: K,
        value: AdvancedSettings[K]
    ) {
        (settings as any)[field] = value
        await saveSettings()
    }

    // Load cache statistics
    async function loadCacheStats() {
        try {
            cacheStats.value = await settingsApi.getCacheStats()
        } catch (e) {
            console.error('Failed to load cache stats:', e)
        }
    }

    // Clear cache
    async function clearCache(cacheType?: string) {
        try {
            await settingsApi.clearCache(cacheType)
            await loadCacheStats()
        } catch (e) {
            error.value = e instanceof Error ? e.message : 'Failed to clear cache'
        }
    }

    // Vacuum database
    async function vacuumDatabase() {
        try {
            await settingsApi.vacuumDatabase()
        } catch (e) {
            error.value = e instanceof Error ? e.message : 'Failed to vacuum database'
        }
    }

    // Run diagnostics
    async function runDiagnostics() {
        isRunningDiagnostics.value = true
        try {
            diagnostics.value = await settingsApi.runDiagnostics()
        } catch (e) {
            error.value = e instanceof Error ? e.message : 'Failed to run diagnostics'
        } finally {
            isRunningDiagnostics.value = false
        }
    }

    // Reset to defaults
    async function resetToDefaults(settingsType: string = 'advanced') {
        try {
            await settingsApi.resetToDefaults(settingsType)
            await loadSettings()
        } catch (e) {
            error.value = e instanceof Error ? e.message : 'Failed to reset settings'
        }
    }

    // Options for UI dropdowns
    const logLevelOptions = [
        { value: 'trace', label: 'Trace' },
        { value: 'debug', label: 'Debug' },
        { value: 'info', label: 'Info' },
        { value: 'warn', label: 'Warning' },
        { value: 'error', label: 'Error' },
    ]

    return {
        // State
        isLoading,
        isSaving,
        error,
        settings,
        cacheStats,
        diagnostics,
        isRunningDiagnostics,
        // Actions
        loadSettings,
        saveSettings,
        updateField,
        loadCacheStats,
        clearCache,
        vacuumDatabase,
        runDiagnostics,
        resetToDefaults,
        // Options
        logLevelOptions,
    }
}
