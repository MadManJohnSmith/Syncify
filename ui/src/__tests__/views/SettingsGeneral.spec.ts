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
    // #197 (SB02): el visor de db_location se retiró; quedan download_dir y temp_dir.
    expect(inputs.length).toBeGreaterThanOrEqual(2) // download_dir, temp_dir

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

    // #197 (SB02): the db_location viewer is gone; Browse remains on the
    // editable download directory and the derived staging path.
    const browseButtons = wrapper.findAll('button').filter(b => b.text().includes('Browse...'))
    expect(browseButtons.length).toBe(2)

    await browseButtons[0].trigger('click')
    expect(open).toHaveBeenCalledWith(expect.objectContaining({
      directory: true,
      multiple: false,
    }))
  })

  it('#197: the retired db_location viewer is not rendered and is never saved', async () => {
    const wrapper = mount(SettingsGeneral)
    await new Promise(r => setTimeout(r, 50))

    // Retired in #197 (SB02): the engine manages the DB location itself and
    // the db_location KV had no reader (AJ-01 in the matrix).
    expect(wrapper.text()).not.toContain('Library database location')

    const { invoke } = await import('@tauri-apps/api/core')
    vi.mocked(invoke).mockClear()
    const { useGeneralSettings } = await import('@/composables/useGeneralSettings')
    const general = useGeneralSettings()
    await general.loadSettings()
    await general.saveSettings()

    const batch = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === 'save_settings_batch')
    expect(batch).toBeDefined()
    const settings = (batch?.[1] as { settings?: Record<string, string> })?.settings ?? {}
    expect(settings.db_location).toBeUndefined()
  })
})
