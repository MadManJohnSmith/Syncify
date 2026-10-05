import { reactive, ref } from 'vue'
import { settingsApi, deriveStagingRoot } from '@/api/settings'
import { trayApi, type TraySettings } from '@/api/tray'
import { useDownloadSettings } from '@/composables/useDownloadSettings'

export function useGeneralSettings() {
    const isLoading = ref(false)
    const isSaving = ref(false)
    const downloadSettings = useDownloadSettings()

    const settings = reactive({
        start_on_boot: false,
        start_minimized: false,
        close_to_tray: true,
        db_location: '',
        download_dir: '',
        temp_dir: '',
    })

    /**
     * Push the persisted startup preferences into the backend tray state (IN-5):
     * the tray only learns these values through update_tray_settings, which is
     * also what registers the OS autostart entry.
     */
    async function syncTraySettings(closeToTray: boolean): Promise<void> {
        try {
            const current: TraySettings = await trayApi.getTraySettings()
            await trayApi.updateTraySettings({
                ...current,
                closeToTray,
                startOnBoot: settings.start_on_boot,
                startMinimized: settings.start_minimized,
            })
        } catch (err) {
            console.warn('[useGeneralSettings] Failed to sync tray settings:', err)
        }
    }

    async function loadSettings() {
        isLoading.value = true
        try {
            const keys = [
                'start_on_boot',
                'start_minimized',
                'close_to_tray',
                'db_location',
                'download_dir',
                'dl_download_path',
                'temp_dir',
                'dl_temp_dir'
            ]
            const [rawValues, unifiedDto] = await Promise.all([
                settingsApi.getSettingsByKeys(keys).catch(() => ({} as Record<string, string>)),
                settingsApi.getUnifiedDownloadSettings().catch(() => null),
            ])
            const values = (rawValues || {}) as Record<string, string>

            if (values['start_on_boot']) settings.start_on_boot = values['start_on_boot'] === 'true'
            if (values['start_minimized']) settings.start_minimized = values['start_minimized'] === 'true'
            if (values['close_to_tray']) settings.close_to_tray = values['close_to_tray'] === 'true'

            // Keep the tray behavior in sync with the persisted preference (IN-5).
            void syncTraySettings(settings.close_to_tray)

            settings.db_location = values['db_location'] || ''

            // Single Source of Truth for Download & Temporary Paths via unified contract
            let canonicalDownloadPath = unifiedDto?.library_root || ''
            let canonicalTempPath = unifiedDto?.staging_root || ''

            if (!canonicalDownloadPath) {
                try {
                    const dl = await settingsApi.getDownloadSettings()
                    if (dl) {
                        canonicalDownloadPath = dl.download_path || ''
                        canonicalTempPath = dl.temporary_root || ''
                    }
                } catch (err) {
                    console.warn('[useGeneralSettings] Failed to fetch download settings:', err)
                }
            }

            if (!canonicalDownloadPath) {
                try {
                    canonicalDownloadPath = await settingsApi.getDefaultDownloadPath()
                } catch {}
            }

            const finalDownloadPath = canonicalDownloadPath || values['dl_download_path'] || values['download_dir'] || ''
            const finalTempPath = canonicalTempPath || deriveStagingRoot(finalDownloadPath) || values['dl_temp_dir'] || values['temp_dir'] || ''

            settings.download_dir = finalDownloadPath
            settings.temp_dir = finalTempPath

            // Synchronize global singleton
            downloadSettings.downloadDto.library_root = finalDownloadPath
            downloadSettings.downloadDto.staging_root = finalTempPath
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

            const staging = settings.temp_dir || deriveStagingRoot(settings.download_dir)
            if (staging) {
                settings.temp_dir = staging
                batch['dl_temp_dir'] = staging
                batch['temp_dir'] = staging
            }

            await settingsApi.saveSettingsBatch(batch)

            // Apply the close-to-tray behavior to the tray immediately (IN-5).
            await syncTraySettings(settings.close_to_tray)

            // Re-read and propagate to all views
            await loadSettings()
            await downloadSettings.loadSettings()
        } catch (err) {
            console.error('Failed to save general settings:', err)
            throw err
        } finally {
            isSaving.value = false
        }
    }

    /**
     * Restore every backend-owned settings table (advanced, sync, folders,
     * duplicates, audio processing and lyrics config) and then the general
     * keys and download paths.
     *
     * The confirmation in SettingsView promises *all* settings; without the
     * backend call only this composable's own keys were being restored and the
     * rest stayed exactly as the user had left them.
     */
    async function resetToDefaults() {
        await settingsApi.resetToDefaults('all')

        settings.start_on_boot = false
        settings.start_minimized = false
        settings.close_to_tray = true
        try {
            settings.download_dir = await settingsApi.getDefaultDownloadPath()
            settings.temp_dir = deriveStagingRoot(settings.download_dir)
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
