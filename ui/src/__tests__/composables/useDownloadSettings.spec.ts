/**
 * Unit tests for useDownloadSettings composable
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { useDownloadSettings } from '../../composables/useDownloadSettings'
import { mockInvoke, resetMocks } from '../setup'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const sampleFolder = {
  id: 1,
  base_folder: '/Users/tardis/Music/Syncify',
  folder_template: '{AlbumArtist}/{Album}',
  file_template: '{TrackNumber:pad2} - {Title}',
  artist_separator: ', ',
  replace_spaces_with: null,
  max_path_length: 255,
  fallback_action: 'try_next',
}

const sampleQuality = [
  {
    id: 1,
    service_name: 'qobuz',
    max_quality: 'hires',
    preferred_format: 'flac',
    fallback_quality: 'high',
    fallback_format: 'mp3',
  },
  {
    id: 2,
    service_name: 'tidal',
    max_quality: 'lossless',
    preferred_format: 'flac',
    fallback_quality: 'high',
    fallback_format: 'mp3',
  }
]

const sampleKV = {
  dl_concurrent_downloads: '4',
  dl_retry_failed: '3',
  dl_retry_count: '3',
  dl_retry_delay: '5000',
  dl_download_path: '/Users/tardis/Music/Syncify',
  dl_create_artist_folder: 'true',
  dl_create_album_folder: 'true',
  dl_auto_download_favorites: 'true',
}

describe('useDownloadSettings composable', () => {
  beforeEach(() => {
    resetMocks()
    mockInvoke((command, args) => {
      if (command === 'get_folder_settings') return sampleFolder
      if (command === 'update_folder_settings') return sampleFolder
      if (command === 'get_quality_preferences') return sampleQuality
      if (command === 'update_quality_preference') {
        const p = args as {
          serviceName?: string
          maxQuality?: string
          preferredFormat?: string
          fallbackQuality?: string
          fallbackFormat?: string
        }
        return {
          id: p?.serviceName === 'tidal' ? 2 : 1,
          service_name: p?.serviceName || 'qobuz',
          max_quality: p?.maxQuality || 'hires',
          preferred_format: p?.preferredFormat || 'flac',
          fallback_quality: p?.fallbackQuality || 'high',
          fallback_format: p?.fallbackFormat || 'mp3',
        }
      }
      if (command === 'get_kv_settings') return sampleKV
      if (command === 'save_settings_batch') return null
      if (command === 'save_setting') return null
      if (command === 'set_max_concurrent_downloads') return null
      if (command === 'get_default_download_path') return '/Users/tardis/Music/Default'
      if (command === 'validate_directory_path') {
        const path = (args as { path: string })?.path || ''
        return {
          valid: true,
          exists: true,
          is_dir: true,
          is_writable: true,
          available_bytes: 100 * 1024 * 1024 * 1024,
          drive_mounted: true,
          canonical_path: path,
          error_message: null,
        }
      }
      if (command === 'get_duplicate_settings') return null
      if (command === 'get_audio_processing_settings') return null
      return null
    })
  })

  it('loads download settings from backend properly', async () => {
    const { loadSettings, downloadPath, concurrentDownloads, fallbackAction, qualityPreferences } = useDownloadSettings()

    await loadSettings()

    expect(downloadPath.value).toBe('/Users/tardis/Music/Syncify')
    expect(concurrentDownloads.value).toBe(4)
    expect(fallbackAction.value).toBe('try_next')
    expect(qualityPreferences.value.length).toBe(2)
  })

  it('sets max concurrency and persists setting', async () => {
    const { setMaxConcurrent, concurrentDownloads } = useDownloadSettings()

    await setMaxConcurrent(5)

    expect(concurrentDownloads.value).toBe(5)
    expect(invoke).toHaveBeenCalledWith('set_max_concurrent_downloads', { max: 5 })
    expect(invoke).toHaveBeenCalledWith('save_setting', { key: 'dl_concurrent_downloads', value: '5' })
  })

  it('S203: allows up to 10 concurrent threads without clamping', async () => {
    const { setMaxConcurrent, concurrentDownloads } = useDownloadSettings()

    await setMaxConcurrent(10)

    expect(concurrentDownloads.value).toBe(10)
    expect(invoke).toHaveBeenCalledWith('set_max_concurrent_downloads', { max: 10 })
    expect(invoke).toHaveBeenCalledWith('save_setting', { key: 'dl_concurrent_downloads', value: '10' })
  })

  it('S203: still clamps to the minimum of 1 thread', async () => {
    const { setMaxConcurrent, concurrentDownloads } = useDownloadSettings()

    await setMaxConcurrent(0)

    expect(concurrentDownloads.value).toBe(1)
    expect(invoke).toHaveBeenCalledWith('set_max_concurrent_downloads', { max: 1 })
  })

  it('updates fallback action setting', async () => {
    const { updateFallbackAction, fallbackAction } = useDownloadSettings()

    await updateFallbackAction('skip')

    expect(fallbackAction.value).toBe('skip')
  })

  it('browses directory using native dialog and saves path', async () => {
    const { browseDownloadDirectory, downloadPath } = useDownloadSettings()

    const chosen = await browseDownloadDirectory()

    expect(chosen).toBe('/Users/tardis/Music/Syncify')
    expect(downloadPath.value).toBe('/Users/tardis/Music/Syncify')
    expect(open).toHaveBeenCalled()
  })

  it('resets download path to system default', async () => {
    const { resetDownloadPath, downloadPath } = useDownloadSettings()

    const resetPath = await resetDownloadPath()

    expect(resetPath).toBe('/Users/tardis/Music/Default')
    expect(downloadPath.value).toBe('/Users/tardis/Music/Default')
  })

  it('updates global quality preference for all services', async () => {
    const { updateGlobalQuality, getQualityForService, loadSettings } = useDownloadSettings()
    await loadSettings()

    await updateGlobalQuality('hires', 'flac')

    expect(invoke).toHaveBeenCalledWith('update_quality_preference', expect.objectContaining({
      maxQuality: 'hires',
      preferredFormat: 'flac',
    }))
  })

  it('TASK-10: validateDirectory sets path_status to unavailable and returns valid:false when validation throws', async () => {
    const { validateDirectory, downloadDto, lastValidLibraryRoot } = useDownloadSettings()
    lastValidLibraryRoot.value = '/Users/tardis/Music/Syncify'

    mockInvoke((command) => {
      if (command === 'validate_directory_path') {
        throw new Error('OS I/O error or backend crash')
      }
      return null
    })

    const result = await validateDirectory('/invalid/corrupted/drive')

    expect(result.valid).toBe(false)
    expect(downloadDto.path_status).toBe('unavailable')
    expect(lastValidLibraryRoot.value).toBe('/Users/tardis/Music/Syncify')
    expect(result.error_message).toContain('OS I/O error')
  })

  it('TASK-10: validateDirectory does not update lastValidLibraryRoot when validation returns valid:false', async () => {
    const { validateDirectory, downloadDto, lastValidLibraryRoot } = useDownloadSettings()
    lastValidLibraryRoot.value = '/Users/tardis/Music/Valid'

    mockInvoke((command, args) => {
      if (command === 'validate_directory_path') {
        return {
          valid: false,
          exists: false,
          is_dir: false,
          is_writable: false,
          available_bytes: 0,
          drive_mounted: false,
          canonical_path: (args as any)?.path,
          error_message: 'Drive not mounted',
        }
      }
      return null
    })

    const result = await validateDirectory('/nonexistent/path')

    expect(result.valid).toBe(false)
    expect(downloadDto.path_status).toBe('unavailable')
    expect(lastValidLibraryRoot.value).toBe('/Users/tardis/Music/Valid')
  })

  describe('TASK-13: updateGlobalQuality race condition & determinism', () => {
    it('executes updateGlobalQuality sequentially across all known services in deterministic order', async () => {
      const invokedServices: string[] = []
      mockInvoke((command, args) => {
        if (command === 'get_quality_preferences') return sampleQuality
        if (command === 'update_quality_preference') {
          const p = args as { serviceName: string; maxQuality: string; preferredFormat: string }
          invokedServices.push(p.serviceName)
          return {
            id: 1,
            service_name: p.serviceName,
            max_quality: p.maxQuality,
            preferred_format: p.preferredFormat,
            fallback_quality: 'high',
            fallback_format: 'mp3',
          }
        }
        return null
      })

      const { updateGlobalQuality } = useDownloadSettings()
      const success = await updateGlobalQuality('lossless', 'flac')

      expect(success).toBe(true)
      expect(invokedServices).toEqual([
        'qobuz',
        'tidal',
        'spotify',
        'deezer',
        'apple_music',
        'soundcloud',
      ])
    })

    it('atomically updates qualityPreferences.value reflecting chosen quality across all services', async () => {
      mockInvoke((command, args) => {
        if (command === 'get_quality_preferences') return sampleQuality
        if (command === 'update_quality_preference') {
          const p = args as { serviceName: string; maxQuality: string; preferredFormat: string }
          return {
            id: 1,
            service_name: p.serviceName,
            max_quality: p.maxQuality,
            preferred_format: p.preferredFormat,
            fallback_quality: 'high',
            fallback_format: 'mp3',
          }
        }
        return null
      })

      const { updateGlobalQuality, qualityPreferences } = useDownloadSettings()
      await updateGlobalQuality('hires', 'flac')

      const expectedServices = ['qobuz', 'tidal', 'spotify', 'deezer', 'apple_music', 'soundcloud']
      expect(qualityPreferences.value.length).toBe(expectedServices.length)

      for (const svc of expectedServices) {
        const pref = qualityPreferences.value.find(q => q.service_name.toLowerCase() === svc)
        expect(pref).toBeDefined()
        expect(pref?.max_quality).toBe('hires')
        expect(pref?.preferred_format).toBe('flac')
      }
    })

    it('isUpdatingGlobalQuality guard blocks concurrent overlapping invocations', async () => {
      let resolveFirstCall: (() => void) | null = null
      const firstCallGate = new Promise<void>(resolve => {
        resolveFirstCall = resolve
      })

      let invocationCount = 0
      mockInvoke(async (command, args) => {
        if (command === 'update_quality_preference') {
          invocationCount++
          if (invocationCount === 1) {
            await firstCallGate
          }
          const p = args as any
          return {
            id: 1,
            service_name: p?.serviceName || 'qobuz',
            max_quality: p?.maxQuality || 'hires',
            preferred_format: p?.preferredFormat || 'flac',
            fallback_quality: 'high',
            fallback_format: 'mp3',
          }
        }
        return null
      })

      const { updateGlobalQuality, isUpdatingGlobalQuality } = useDownloadSettings()
      expect(isUpdatingGlobalQuality.value).toBe(false)

      const firstUpdatePromise = updateGlobalQuality('lossless', 'flac')
      expect(isUpdatingGlobalQuality.value).toBe(true)

      // Second invocation while first is actively running
      const secondUpdatePromise = updateGlobalQuality('hires', 'mp3')

      // Release first invocation gate
      resolveFirstCall!()

      const [firstResult, secondResult] = await Promise.all([firstUpdatePromise, secondUpdatePromise])

      expect(firstResult).toBe(true)
      expect(secondResult).toBe(false)
      expect(isUpdatingGlobalQuality.value).toBe(false)
      // Exactly 6 calls for the 6 services of the first invocation, none for the second
      expect(invocationCount).toBe(6)
    })

    it('resets isUpdatingGlobalQuality to false when backend throws an error', async () => {
      mockInvoke((command) => {
        if (command === 'update_quality_preference') {
          throw new Error('SQLite disk I/O failure')
        }
        return null
      })

      const { updateGlobalQuality, isUpdatingGlobalQuality } = useDownloadSettings()
      expect(isUpdatingGlobalQuality.value).toBe(false)

      await expect(updateGlobalQuality('hires', 'flac')).rejects.toThrow('SQLite disk I/O failure')
      expect(isUpdatingGlobalQuality.value).toBe(false)
    })
  })
})
