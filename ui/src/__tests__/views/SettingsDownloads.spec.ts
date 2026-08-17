/**
 * Unit tests for SettingsDownloads.vue
 * Tests library location, quality selection, fallback policies, concurrency controls, and persistence
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import SettingsDownloads from '../../views/settings/SettingsDownloads.vue'
import { mockInvoke, resetMocks } from '../setup'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

const mockFolderSettings = {
  id: 1,
  base_folder: '/Users/tardis/Music/Syncify',
  folder_template: '{AlbumArtist}/{Album}',
  file_template: '{TrackNumber:pad2} - {Title}',
  artist_separator: ', ',
  replace_spaces_with: null,
  max_path_length: 255,
  fallback_action: 'try_next',
}

const mockQualityPreferences = [
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
  },
  {
    id: 3,
    service_name: 'spotify',
    max_quality: 'high',
    preferred_format: 'mp3',
    fallback_quality: 'normal',
    fallback_format: 'ogg',
  }
]

const mockKvSettings = {
  dl_concurrent_downloads: '3',
  dl_retry_failed: '3',
  dl_retry_count: '3',
  dl_retry_delay: '5000',
  dl_download_path: '/Users/tardis/Music/Syncify',
  dl_create_artist_folder: 'true',
  dl_create_album_folder: 'true',
  dl_auto_download_favorites: 'false',
}

describe('SettingsDownloads.vue', () => {
  beforeEach(() => {
    resetMocks()
    mockInvoke((command) => {
      if (command === 'get_folder_settings') return mockFolderSettings
      if (command === 'update_folder_settings') return mockFolderSettings
      if (command === 'get_quality_preferences') return mockQualityPreferences
      if (command === 'update_quality_preference') return mockQualityPreferences[0]
      if (command === 'get_kv_settings') return mockKvSettings
      if (command === 'save_settings_batch') return null
      if (command === 'save_setting') return null
      if (command === 'set_max_concurrent_downloads') return null
      if (command === 'get_default_download_path') return '/Users/tardis/Music/Syncify'
      if (command === 'get_duplicate_settings') return null
      if (command === 'get_audio_processing_settings') return null
      return null
    })
  })

  it('renders all download controls and sections without disabled state', async () => {
    const wrapper = mount(SettingsDownloads)
    await flushPromises()

    // Section 1: Library location
    expect(wrapper.text()).toContain('Library & Download Location')
    const pathInput = wrapper.find('input[type="text"]')
    expect(pathInput.exists()).toBe(true)
    expect(pathInput.attributes('disabled')).toBeUndefined()
    expect((pathInput.element as HTMLInputElement).value).toBe('/Users/tardis/Music/Syncify')

    const browseBtn = wrapper.findAll('button').find(b => b.text().includes('Browse'))
    expect(browseBtn).toBeDefined()
    expect(browseBtn?.attributes('disabled')).toBeUndefined()

    // Section 2: Quality preferences
    expect(wrapper.text()).toContain('Audio Quality Preferences')
    const selects = wrapper.findAll('select')
    expect(selects.length).toBeGreaterThanOrEqual(3)

    // Section 3: Fallback policy
    expect(wrapper.text()).toContain('Service & Quality Fallback Policy')
    expect(wrapper.text()).toContain('Allow Quality Downgrade')

    // Section 4: Concurrency
    expect(wrapper.text()).toContain('Download Concurrency')
    const threadButtons = wrapper.findAll('button[title*="concurrent download thread"]')
    expect(threadButtons.length).toBe(5)
  })

  it('allows editing library path and browsing via native dialog', async () => {
    const wrapper = mount(SettingsDownloads)
    await flushPromises()

    const browseBtn = wrapper.findAll('button').find(b => b.text().includes('Browse'))
    expect(browseBtn).toBeDefined()

    await browseBtn!.trigger('click')
    await flushPromises()

    expect(open).toHaveBeenCalledWith(expect.objectContaining({
      directory: true,
      multiple: false,
    }))
  })

  it('updates fallback action and toggles downgrade allowance', async () => {
    const wrapper = mount(SettingsDownloads)
    await flushPromises()

    // Find downgrade switch
    const switchBtn = wrapper.find('button[role="switch"]')
    expect(switchBtn.exists()).toBe(true)

    // Initially true ('try_next')
    expect(switchBtn.attributes('aria-checked')).toBe('true')

    // Toggle switch to false ('skip')
    await switchBtn.trigger('click')
    await flushPromises()

    expect(switchBtn.attributes('aria-checked')).toBe('false')
  })

  it('allows changing download concurrency between 1 and 5 threads', async () => {
    const wrapper = mount(SettingsDownloads)
    await flushPromises()

    const threadButtons = wrapper.findAll('button[title*="concurrent download thread"]')
    expect(threadButtons.length).toBe(5)

    // Click 4 threads
    await threadButtons[3].trigger('click')
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('set_max_concurrent_downloads', { max: 4 })
    expect(invoke).toHaveBeenCalledWith('save_setting', { key: 'dl_concurrent_downloads', value: '4' })
  })

  it('persists settings when clicking Save Settings button', async () => {
    const wrapper = mount(SettingsDownloads)
    await flushPromises()

    const saveBtn = wrapper.findAll('button').find(b => b.text().includes('Save Settings'))
    expect(saveBtn).toBeDefined()

    await saveBtn!.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('Changes saved')
  })

  it('restores persisted settings upon remounting view', async () => {
    mockInvoke((command) => {
      if (command === 'get_folder_settings') return {
        ...mockFolderSettings,
        base_folder: '/Volumes/Audio/Lossless',
        fallback_action: 'skip',
      }
      if (command === 'get_kv_settings') return {
        ...mockKvSettings,
        dl_download_path: '/Volumes/Audio/Lossless',
        dl_concurrent_downloads: '5',
      }
      if (command === 'get_quality_preferences') return mockQualityPreferences
      return null
    })

    const wrapper = mount(SettingsDownloads)
    await flushPromises()

    const pathInput = wrapper.find('input[type="text"]')
    expect((pathInput.element as HTMLInputElement).value).toBe('/Volumes/Audio/Lossless')
    expect(wrapper.text()).toContain('5 parallel threads')
  })
})
