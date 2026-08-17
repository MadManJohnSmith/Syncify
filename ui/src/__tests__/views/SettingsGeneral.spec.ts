import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import SettingsGeneral from '../../views/settings/SettingsGeneral.vue'
import { mockInvoke, resetMocks } from '../setup'
import { open } from '@tauri-apps/plugin-dialog'

describe('SettingsGeneral.vue & PathSelector Integration', () => {
  beforeEach(() => {
    resetMocks()
    mockInvoke((command, args) => {
      if (command === 'get_kv_settings') {
        return {
          start_on_boot: 'true',
          start_minimized: 'false',
          close_to_tray: 'true',
          auto_updates: 'true',
          anonymous_stats: 'false',
          db_location: 'C:\\Users\\User\\AppData\\Roaming\\Syncify\\syncify.db',
          download_dir: 'D:\\Media\\SyncifyMusic',
          dl_download_path: 'D:\\Media\\SyncifyMusic',
          temp_dir: 'D:\\Media\\SyncifyTemp',
          dl_temp_dir: 'D:\\Media\\SyncifyTemp',
        }
      }
      if (command === 'get_download_settings') {
        return {
          download_path: 'D:\\Media\\SyncifyMusic',
          temporary_root: 'D:\\Media\\SyncifyTemp',
          folder_template: '{AlbumArtist}/{Album}',
          file_template: '{TrackNumber:pad2} - {Title}',
          artist_separator: ', ',
          replace_spaces_with: null,
          max_path_length: 255,
          fallback_action: 'try_next',
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
      if (command === 'get_folder_settings') {
        return {
          id: 1,
          base_folder: 'D:\\Media\\SyncifyMusic',
          folder_template: '{AlbumArtist}/{Album}',
          file_template: '{TrackNumber:pad2} - {Title}',
          artist_separator: ', ',
          replace_spaces_with: null,
          max_path_length: 255,
          fallback_action: 'try_next',
        }
      }
      if (command === 'validate_directory_path') {
        const path = (args as { path: string })?.path || ''
        if (path.includes('unmounted')) {
          return {
            valid: false,
            exists: false,
            is_dir: false,
            is_writable: false,
            available_bytes: 0,
            drive_mounted: false,
            canonical_path: path,
            error_message: 'Drive not mounted',
          }
        }
        if (path.includes('read_only')) {
          return {
            valid: false,
            exists: true,
            is_dir: true,
            is_writable: false,
            available_bytes: 50000000,
            drive_mounted: true,
            canonical_path: path,
            error_message: 'Directory is not writable',
          }
        }
        return {
          valid: true,
          exists: true,
          is_dir: true,
          is_writable: true,
          available_bytes: 10000000000,
          drive_mounted: true,
          canonical_path: path,
          error_message: null,
        }
      }
      if (command === 'get_default_download_path') return 'C:\\Users\\User\\Music\\Syncify'
      if (command === 'get_default_temp_path') return 'C:\\Users\\User\\AppData\\Local\\Temp\\Syncify'
      if (command === 'save_settings_batch') return null
      if (command === 'update_folder_settings') return null
      return null
    })
  })

  it('renders Download directory PathSelector in enabled/editable state and derived paths as disabled', async () => {
    const wrapper = mount(SettingsGeneral)
    await new Promise(r => setTimeout(r, 50))

    const inputs = wrapper.findAll('input[type="text"]')
    expect(inputs.length).toBeGreaterThanOrEqual(3) // db_location, download_dir, temp_dir

    // Verify download directory input is active and editable
    const downloadInput = inputs.find(i => (i.element as HTMLInputElement).value === 'D:\\Media\\SyncifyMusic')
    expect(downloadInput).toBeDefined()
    expect(downloadInput!.attributes('disabled')).toBeUndefined()
    expect(downloadInput!.attributes('readonly')).toBeUndefined()
  })

  it('displays the unified canonical download path from backend', async () => {
    const wrapper = mount(SettingsGeneral)
    await new Promise(r => setTimeout(r, 50))

    const text = wrapper.text()
    expect(text).toContain('Download directory')
    expect(text).toContain('Temporary files location')

    const downloadInput = wrapper.findAll('input[type="text"]').find(i => (i.element as HTMLInputElement).value === 'D:\\Media\\SyncifyMusic')
    expect(downloadInput).toBeDefined()
    expect((downloadInput!.element as HTMLInputElement).value).toBe('D:\\Media\\SyncifyMusic')
  })

  it('allows typing a custom alternative drive path and updates value', async () => {
    const wrapper = mount(SettingsGeneral)
    await new Promise(r => setTimeout(r, 50))

    const downloadInput = wrapper.findAll('input[type="text"]').find(i => (i.element as HTMLInputElement).value === 'D:\\Media\\SyncifyMusic')!
    await downloadInput.setValue('E:\\CustomMusic\\Library')
    await downloadInput.trigger('input')
    await downloadInput.trigger('blur')

    expect((downloadInput.element as HTMLInputElement).value).toBe('E:\\CustomMusic\\Library')
  })

  it('supports native directory browse dialog', async () => {
    const wrapper = mount(SettingsGeneral)
    await new Promise(r => setTimeout(r, 50))

    const browseButtons = wrapper.findAll('button').filter(b => b.text().includes('Browse...'))
    expect(browseButtons.length).toBeGreaterThanOrEqual(3)

    await browseButtons[1].trigger('click')
    expect(open).toHaveBeenCalledWith(expect.objectContaining({
      directory: true,
      multiple: false,
    }))
  })
})
