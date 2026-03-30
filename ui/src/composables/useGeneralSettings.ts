import { reactive, ref } from 'vue'
import { settingsApi } from '@/api/settings'

export function useGeneralSettings() {
    const isLoading = ref(false)
    const isSaving = ref(false)

    const settings = reactive({
        start_on_boot: false,
        start_minimized: false,
        close_to_tray: true,
        auto_updates: true,
        anonymous_stats: false,
        db_location: '',
        download_dir: '',
        temp_dir: '',
    })

    async function loadSettings() {
        isLoading.value = true
        try {
            const keys = [
                'start_on_boot',
                'start_minimized',
                'close_to_tray',
                'auto_updates',
                'anonymous_stats',
                'db_location',
                'download_dir',
                'temp_dir'
            ]
            const values = await settingsApi.getSettingsByKeys(keys)

            if (values['start_on_boot']) settings.start_on_boot = values['start_on_boot'] === 'true'
            if (values['start_minimized']) settings.start_minimized = values['start_minimized'] === 'true'
            if (values['close_to_tray']) settings.close_to_tray = values['close_to_tray'] === 'true'
            if (values['auto_updates']) settings.auto_updates = values['auto_updates'] === 'true'
            if (values['anonymous_stats']) settings.anonymous_stats = values['anonymous_stats'] === 'true'

            settings.db_location = values['db_location'] || ''
            settings.download_dir = values['download_dir'] || ''
            settings.temp_dir = values['temp_dir'] || ''
        } catch (err) {
            console.error('Failed to load general settings:', err)
        } finally {
            isLoading.value = false
        }
    }

    async function saveSettings() {
        isSaving.value = true
        try {
            const batch: Record<string, string> = {
                'start_on_boot': settings.start_on_boot.toString(),
                'start_minimized': settings.start_minimized.toString(),
                'close_to_tray': settings.close_to_tray.toString(),
                'auto_updates': settings.auto_updates.toString(),
                'anonymous_stats': settings.anonymous_stats.toString(),
            }

            // Paths usually use dedicated pickers, but we save them if they changed
            if (settings.db_location) batch['db_location'] = settings.db_location
            if (settings.download_dir) batch['download_dir'] = settings.download_dir
            if (settings.temp_dir) batch['temp_dir'] = settings.temp_dir

            await settingsApi.saveSettingsBatch(batch)
        } catch (err) {
            console.error('Failed to save general settings:', err)
            throw err
        } finally {
            isSaving.value = false
        }
    }

    async function resetToDefaults() {
        settings.start_on_boot = false
        settings.start_minimized = false
        settings.close_to_tray = true
        settings.auto_updates = true
        settings.anonymous_stats = false
        await saveSettings()
    }

    return {
        settings,
        isLoading,
        isSaving,
        loadSettings,
        saveSettings,
        resetToDefaults
    }
}
