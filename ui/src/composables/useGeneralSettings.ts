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
                'dl_download_path',
                'temp_dir',
                'dl_temp_dir'
            ]
            const values = (await settingsApi.getSettingsByKeys(keys)) || {}

            if (values['start_on_boot']) settings.start_on_boot = values['start_on_boot'] === 'true'
            if (values['start_minimized']) settings.start_minimized = values['start_minimized'] === 'true'
            if (values['close_to_tray']) settings.close_to_tray = values['close_to_tray'] === 'true'
            if (values['auto_updates']) settings.auto_updates = values['auto_updates'] === 'true'
            if (values['anonymous_stats']) settings.anonymous_stats = values['anonymous_stats'] === 'true'

            settings.db_location = values['db_location'] || ''

            // Single Source of Truth for Download & Temporary Paths
            let canonicalDownloadPath = ''
            let canonicalTempPath = ''

            try {
                const dl = await settingsApi.getDownloadSettings()
                if (dl) {
                    canonicalDownloadPath = dl.download_path || ''
                    canonicalTempPath = dl.temporary_root || ''
                }
            } catch (err) {
                console.warn('[useGeneralSettings] Failed to fetch unified download settings:', err)
            }

            if (!canonicalDownloadPath) {
                try {
                    const folder = await settingsApi.getFolderSettings()
                    if (folder?.base_folder) {
                        canonicalDownloadPath = folder.base_folder
                    }
                } catch {}
            }

            if (!canonicalDownloadPath) {
                try {
                    canonicalDownloadPath = await settingsApi.getDefaultDownloadPath()
                } catch {}
            }

            settings.download_dir = canonicalDownloadPath || values['dl_download_path'] || values['download_dir'] || ''
            settings.temp_dir = canonicalTempPath || values['dl_temp_dir'] || values['temp_dir'] || ''
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

            if (settings.db_location) batch['db_location'] = settings.db_location

            if (settings.download_dir) {
                batch['dl_download_path'] = settings.download_dir
                batch['download_dir'] = settings.download_dir

                // Keep folder_settings table synchronized as the canonical base_folder
                try {
                    const folder = await settingsApi.getFolderSettings()
                    if (folder) {
                        await settingsApi.updateFolderSettings({
                            ...folder,
                            base_folder: settings.download_dir
                        })
                    }
                } catch (e) {
                    console.warn('[useGeneralSettings] Failed to sync folder_settings base_folder:', e)
                }
            }

            if (settings.temp_dir) {
                batch['dl_temp_dir'] = settings.temp_dir
                batch['temp_dir'] = settings.temp_dir
            }

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
        try {
            settings.download_dir = await settingsApi.getDefaultDownloadPath()
        } catch {}
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
