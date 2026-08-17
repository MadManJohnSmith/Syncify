/**
 * Tests for Download & Staging Path Consistency across Settings views & backend APIs
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import SettingsGeneral from '../../views/settings/SettingsGeneral.vue'
import SettingsDownloads from '../../views/settings/SettingsDownloads.vue'
import { useDownloadSettings } from '../../composables/useDownloadSettings'
import { useGeneralSettings } from '../../composables/useGeneralSettings'
import { settingsApi } from '../../api/settings'
import { mockInvoke, resetMocks } from '../setup'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

describe('S121 P0 Path Consistency & Validation Suite', () => {
  let backendDb: Record<string, string> = {}
  let folderDb: any = {}

  beforeEach(() => {
    resetMocks()
    backendDb = {
      dl_download_path: 'D:\\LosslessMusic\\Syncify',
      download_dir: 'D:\\LosslessMusic\\Syncify',
      dl_temp_dir: 'D:\\LosslessMusic\\.staging',
      temp_dir: 'D:\\LosslessMusic\\.staging',
      dl_concurrent_downloads: '3',
      dl_retry_failed: 'true',
      dl_retry_count: '3',
      dl_retry_delay: '5000',
    }

    folderDb = {
      id: 1,
      base_folder: 'D:\\LosslessMusic\\Syncify',
      folder_template: '{AlbumArtist}/{Album}',
      file_template: '{TrackNumber:pad2} - {Title}',
      artist_separator: ', ',
      replace_spaces_with: null,
      max_path_length: 255,
      fallback_action: 'try_next',
    }

    mockInvoke((command, args) => {
      if (command === 'get_kv_settings') {
        const keys = (args as { keys: string[] })?.keys || []
        const res: Record<string, string> = {}
        for (const k of keys) {
          if (backendDb[k] !== undefined) res[k] = backendDb[k]
        }
        return res
      }
      if (command === 'save_settings_batch') {
        const batch = (args as { settings: Record<string, string> })?.settings || {}
        Object.assign(backendDb, batch)
        return null
      }
      if (command === 'save_setting') {
        const { key, value } = args as { key: string; value: string }
        backendDb[key] = value
        return null
      }
      if (command === 'get_folder_settings') {
        return { ...folderDb }
      }
      if (command === 'update_folder_settings') {
        const settings = (args as { settings: any })?.settings || {}
        Object.assign(folderDb, settings)
        return { ...folderDb }
      }
      if (command === 'get_download_settings') {
        return {
          download_path: folderDb.base_folder || backendDb.dl_download_path,
          temporary_root: backendDb.dl_temp_dir,
          folder_template: folderDb.folder_template,
          file_template: folderDb.file_template,
          artist_separator: folderDb.artist_separator,
          replace_spaces_with: folderDb.replace_spaces_with,
          max_path_length: folderDb.max_path_length,
          fallback_action: folderDb.fallback_action,
          max_concurrent_downloads: 3,
          retry_failed: true,
          retry_count: 3,
          retry_delay_ms: 5000,
          auto_download_favorites: false,
          organize_by_artist: true,
          organize_by_album: true,
          generate_lyrics_lrc: true,
          generate_cover_art: true,
          generate_animated_cover: true,
          generate_booklet: true,
          generate_artist_sidecars: true,
        }
      }
      if (command === 'save_download_settings') {
        const s = (args as { settings: any })?.settings || {}
        if (s.download_path) {
          folderDb.base_folder = s.download_path
          backendDb.dl_download_path = s.download_path
          backendDb.download_dir = s.download_path
        }
        if (s.temporary_root) {
          backendDb.dl_temp_dir = s.temporary_root
          backendDb.temp_dir = s.temporary_root
        }
        return { ...s }
      }
      if (command === 'validate_directory_path') {
        const path = (args as { path: string })?.path || ''
        if (!path.trim()) {
          return {
            valid: false,
            exists: false,
            is_dir: false,
            is_writable: false,
            available_bytes: 0,
            drive_mounted: false,
            canonical_path: '',
            error_message: 'Path cannot be empty',
          }
        }
        if (path.startsWith('Z:\\')) {
          return {
            valid: false,
            exists: false,
            is_dir: false,
            is_writable: false,
            available_bytes: 0,
            drive_mounted: false,
            canonical_path: path,
            error_message: 'Drive or volume not mounted',
          }
        }
        if (path.includes('restricted')) {
          return {
            valid: false,
            exists: true,
            is_dir: true,
            is_writable: false,
            available_bytes: 1024 * 1024 * 1024,
            drive_mounted: true,
            canonical_path: path,
            error_message: 'Directory is not writable: Permission denied',
          }
        }
        return {
          valid: true,
          exists: true,
          is_dir: true,
          is_writable: true,
          available_bytes: 500 * 1024 * 1024 * 1024,
          drive_mounted: true,
          canonical_path: path,
          error_message: null,
        }
      }
      if (command === 'get_default_download_path') return 'C:\\Users\\User\\Music\\Syncify'
      if (command === 'get_default_temp_path') return 'C:\\Users\\User\\AppData\\Local\\Temp\\Syncify'
      if (command === 'get_quality_preferences') return []
      if (command === 'get_duplicate_settings') return null
      if (command === 'get_audio_processing_settings') return null
      return null
    })
  })

  it('verifies General and Download Settings show the exact same value', async () => {
    const general = useGeneralSettings()
    const download = useDownloadSettings()

    await general.loadSettings()
    await download.loadSettings()

    expect(general.settings.download_dir).toBe('D:\\LosslessMusic\\Syncify')
    expect(download.downloadPath.value).toBe('D:\\LosslessMusic\\Syncify')
    expect(general.settings.download_dir).toEqual(download.downloadPath.value)
  })

  it('validates alternative drive path preservation on save -> get cycle', async () => {
    const download = useDownloadSettings()
    await download.loadSettings()

    const newDrivePath = 'F:\\HighResAudio\\SyncifyLibrary'
    download.downloadPath.value = newDrivePath
    await download.saveGeneralSettings()
    await download.saveFolderSettings()

    expect(backendDb.dl_download_path).toBe(newDrivePath)
    expect(folderDb.base_folder).toBe(newDrivePath)

    // Reload from fresh composable instance
    const freshDownload = useDownloadSettings()
    await freshDownload.loadSettings()
    expect(freshDownload.downloadPath.value).toBe(newDrivePath)

    const freshGeneral = useGeneralSettings()
    await freshGeneral.loadSettings()
    expect(freshGeneral.settings.download_dir).toBe(newDrivePath)
  })

  it('tests directory validator for unmounted drive and permission errors', async () => {
    const validRes = await settingsApi.validateDirectoryPath('D:\\LosslessMusic\\Syncify')
    expect(validRes.valid).toBe(true)
    expect(validRes.drive_mounted).toBe(true)
    expect(validRes.is_writable).toBe(true)

    const unmountedRes = await settingsApi.validateDirectoryPath('Z:\\NonExistentDrive\\Music')
    expect(unmountedRes.valid).toBe(false)
    expect(unmountedRes.drive_mounted).toBe(false)
    expect(unmountedRes.error_message).toContain('not mounted')

    const restrictedRes = await settingsApi.validateDirectoryPath('C:\\Program Files\\restricted')
    expect(restrictedRes.valid).toBe(false)
    expect(restrictedRes.is_writable).toBe(false)
    expect(restrictedRes.error_message).toContain('Permission denied')
  })

  it('verifies temporary_root propagation and staging path integrity', async () => {
    const download = useDownloadSettings()
    await download.loadSettings()

    expect(download.temporaryPath.value).toBe('D:\\LosslessMusic\\.staging')

    const customStaging = 'E:\\FastNVMe\\.staging'
    download.temporaryPath.value = customStaging
    await download.saveGeneralSettings()

    expect(backendDb.dl_temp_dir).toBe(customStaging)
    expect(backendDb.temp_dir).toBe(customStaging)

    const general = useGeneralSettings()
    await general.loadSettings()
    expect(general.settings.temp_dir).toBe(customStaging)
  })

  it('guarantees that settings loading does not overwrite freshly saved paths with defaults', async () => {
    const download = useDownloadSettings()
    await download.loadSettings()

    download.downloadPath.value = 'D:\\CustomDirectory'
    await download.saveGeneralSettings()
    await download.saveFolderSettings()

    // Simulate app reload/fresh component mount
    await download.loadSettings()
    expect(download.downloadPath.value).toBe('D:\\CustomDirectory')
    expect(download.downloadPath.value).not.toBe('C:\\Users\\User\\Music\\Syncify')
  })
})
