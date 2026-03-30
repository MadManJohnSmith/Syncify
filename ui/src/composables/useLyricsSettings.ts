/**
 * Lyrics Settings Composable
 * 
 * Manages lyrics provider settings and lyrics configuration from the backend.
 */

import { ref, reactive, computed } from 'vue'
import {
    getLyricsProviders,
    updateLyricsProvider,
    reorderLyricsProviders,
    getLyricsConfig,
    updateLyricsConfig,
    testLyricsProvider,
} from '@/api/settings'
import type { LyricsProviderSetting, LyricsConfig } from '@/api/types'

export function useLyricsSettings() {
    // Loading states
    const isLoading = ref(false)
    const isSaving = ref(false)
    const error = ref<string | null>(null)

    // Provider settings
    const providers = ref<LyricsProviderSetting[]>([])

    // Lyrics config (singleton)
    const config = reactive<LyricsConfig>({
        id: 1,
        min_sync_level: 'line',
        preferred_language: 'en',
        storage_format: 'lrc',
        auto_fetch_on_import: true,
        retry_failed: true,
        retry_frequency: 'weekly',
    })

    // Ordered providers (by priority)
    const orderedProviders = computed(() => {
        return [...providers.value].sort((a, b) => a.priority - b.priority)
    })

    // Enabled providers only
    const enabledProviders = computed(() => {
        return orderedProviders.value.filter(p => p.enabled)
    })

    /**
     * Load all lyrics settings from backend
     */
    async function loadSettings() {
        isLoading.value = true
        error.value = null

        try {
            const [loadedProviders, loadedConfig] = await Promise.all([
                getLyricsProviders(),
                getLyricsConfig(),
            ])

            providers.value = loadedProviders
            Object.assign(config, loadedConfig)
        } catch (e) {
            error.value = e instanceof Error ? e.message : 'Failed to load lyrics settings'
            console.error('Failed to load lyrics settings:', e)
        } finally {
            isLoading.value = false
        }
    }

    /**
     * Toggle a provider's enabled state
     */
    async function toggleProvider(providerId: string) {
        const provider = providers.value.find(p => p.provider_id === providerId)
        if (!provider) return

        isSaving.value = true
        try {
            const updated = await updateLyricsProvider(
                providerId,
                !provider.enabled,
                provider.priority
            )

            // Update local state
            const index = providers.value.findIndex(p => p.provider_id === providerId)
            if (index !== -1) {
                providers.value[index] = updated
            }
        } catch (e) {
            console.error(`Failed to toggle provider ${providerId}:`, e)
        } finally {
            isSaving.value = false
        }
    }

    /**
     * Reorder providers (move up or down)
     */
    async function reorderProviders(newOrder: string[]) {
        isSaving.value = true
        try {
            const updated = await reorderLyricsProviders(newOrder)
            providers.value = updated
        } catch (e) {
            console.error('Failed to reorder providers:', e)
        } finally {
            isSaving.value = false
        }
    }

    /**
     * Move provider up in priority
     */
    async function moveProviderUp(providerId: string) {
        const ordered = orderedProviders.value
        const index = ordered.findIndex(p => p.provider_id === providerId)

        if (index <= 0) return // Already at top

        // Swap with previous
        const newOrder = ordered.map(p => p.provider_id)
            ;[newOrder[index], newOrder[index - 1]] = [newOrder[index - 1], newOrder[index]]

        await reorderProviders(newOrder)
    }

    /**
     * Move provider down in priority
     */
    async function moveProviderDown(providerId: string) {
        const ordered = orderedProviders.value
        const index = ordered.findIndex(p => p.provider_id === providerId)

        if (index >= ordered.length - 1) return // Already at bottom

        // Swap with next
        const newOrder = ordered.map(p => p.provider_id)
            ;[newOrder[index], newOrder[index + 1]] = [newOrder[index + 1], newOrder[index]]

        await reorderProviders(newOrder)
    }

    /**
     * Save lyrics configuration
     */
    async function saveConfig() {
        isSaving.value = true
        try {
            const updated = await updateLyricsConfig(config)
            Object.assign(config, updated)
        } catch (e) {
            console.error('Failed to save lyrics config:', e)
        } finally {
            isSaving.value = false
        }
    }

    /**
     * Update a specific config field and save
     */
    async function updateConfigField<K extends keyof LyricsConfig>(
        field: K,
        value: LyricsConfig[K]
    ) {
        config[field] = value
        await saveConfig()
    }

    /**
     * Test a provider connection
     */
    async function testProvider(providerId: string): Promise<boolean> {
        try {
            return await testLyricsProvider(providerId)
        } catch (e) {
            console.error(`Failed to test provider ${providerId}:`, e)
            return false
        }
    }

    // Sync level options for UI
    const syncLevelOptions = [
        { value: 'none', label: 'Any (including unsynced)' },
        { value: 'line', label: 'Line-level sync' },
        { value: 'word', label: 'Word-level sync' },
        { value: 'syllable', label: 'Syllable-level sync' },
    ]

    // Language options for UI
    const languageOptions = [
        { value: 'en', label: 'English' },
        { value: 'match', label: 'Match track language' },
        { value: 'multi', label: 'Multi-language' },
    ]

    // Storage format options for UI
    const storageFormatOptions = [
        { value: 'lrc', label: 'LRC file' },
        { value: 'ttml', label: 'TTML file' },
        { value: 'embedded', label: 'Embedded in tags' },
        { value: 'database', label: 'Database only' },
    ]

    // Retry frequency options for UI
    const retryFrequencyOptions = [
        { value: 'never', label: 'Never' },
        { value: 'hourly', label: 'Hourly' },
        { value: 'daily', label: 'Daily' },
        { value: 'weekly', label: 'Weekly' },
    ]

    return {
        // State
        isLoading,
        isSaving,
        error,
        providers,
        config,

        // Computed
        orderedProviders,
        enabledProviders,

        // Actions
        loadSettings,
        toggleProvider,
        reorderProviders,
        moveProviderUp,
        moveProviderDown,
        saveConfig,
        updateConfigField,
        testProvider,

        // Options for UI
        syncLevelOptions,
        languageOptions,
        storageFormatOptions,
        retryFrequencyOptions,
    }
}
