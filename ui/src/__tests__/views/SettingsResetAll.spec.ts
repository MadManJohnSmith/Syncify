/**
 * SettingsView "Reset to Defaults" regression (audit item 4).
 *
 * The confirmation promises every setting, but the handler only restored the
 * general keys and the download paths: quality, metadata, service priorities
 * and lyrics providers kept whatever the user had set.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'

const confirmMock = vi.fn(async () => true)
vi.mock('@tauri-apps/plugin-dialog', () => ({
  confirm: (...args: unknown[]) => confirmMock(...(args as [])),
  open: vi.fn(async () => null),
}))

import { mockInvoke, resetMocks } from '../setup'
import SettingsView from '@/views/SettingsView.vue'

function seedInvoked() {
  return [] as Array<{ command: string; args?: Record<string, unknown> }>
}

describe('SettingsView — Reset to Defaults covers every category', () => {
  let invoked: ReturnType<typeof seedInvoked>

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
    confirmMock.mockResolvedValue(true)
    invoked = seedInvoked()

    mockInvoke((command, args) => {
      invoked.push({ command, args })
      if (command === 'get_services') return []
      if (command === 'get_accounts') return []
      if (command === 'get_service_statuses') return []
      if (command === 'get_service_sync_settings') return []
      if (command === 'get_sync_settings') return null
      if (command === 'get_quality_preferences') {
        return [
          { id: 1, service_name: 'spotify', max_quality: 'lossless', preferred_format: 'flac', fallback_quality: 'lossless', fallback_format: 'flac' },
          { id: 2, service_name: 'apple_music', max_quality: 'master', preferred_format: 'flac', fallback_quality: 'hifi', fallback_format: 'flac' },
        ]
      }
      if (command === 'get_service_preferences') {
        return [
          { id: 1, service_name: 'qobuz', priority: 1, auto_import_enabled: true },
          { id: 2, service_name: 'spotify', priority: 2, auto_import_enabled: false },
        ]
      }
      if (command === 'get_lyrics_providers') {
        return [
          { id: 1, provider_id: 'genius', provider_name: 'Genius', enabled: false, priority: 1, sync_level: 'none' },
          { id: 2, provider_id: 'apple_music', provider_name: 'Apple Music', enabled: true, priority: 2, sync_level: 'syllable' },
          { id: 3, provider_id: 'lrclib', provider_name: 'LRCLIB', enabled: true, priority: 3, sync_level: 'line' },
        ]
      }
      if (command === 'get_lyrics_config') return null
      if (command === 'get_metadata_preferences') return null
      if (command === 'get_advanced_settings') return null
      if (command === 'get_duplicate_settings') return null
      if (command === 'get_audio_processing_settings') return null
      if (command === 'get_folder_settings') return null
      if (command === 'get_download_settings') return null
      if (command === 'health_check') {
        return {
          database_ok: true, python_ok: true, ffmpeg_available: true,
          chromaprint_available: false, services_configured: [], errors: [],
        }
      }
      return null
    })
  })

  async function reset() {
    const wrapper = mount(SettingsView, {
      global: { stubs: { RouterLink: true } },
    })
    await flushPromises()

    const button = wrapper.findAll('button').find(b => b.text().includes('Reset to Defaults'))
    expect(button).toBeDefined()
    await button!.trigger('click')
    await flushPromises()
    await flushPromises()
    await flushPromises()
    return wrapper
  }

  function callsTo(command: string) {
    return invoked.filter(c => c.command === command)
  }

  it('asks the backend to wipe the settings tables it owns', async () => {
    await reset()

    const resets = callsTo('reset_to_defaults')
    expect(resets.length).toBeGreaterThanOrEqual(1)
    expect(resets.some(c => c.args?.settingsType === 'all')).toBe(true)
  })

  it('restores every quality cap, including services without a seeded row', async () => {
    await reset()

    const updates = callsTo('update_quality_preference')
    const byService = Object.fromEntries(
      updates.map(c => [c.args?.serviceName, c.args])
    )

    expect(byService.spotify).toMatchObject({
      maxQuality: 'high', preferredFormat: 'ogg',
      fallbackQuality: 'medium', fallbackFormat: 'ogg',
    })
    // apple_music has no seed in migration 0009, so the column defaults apply.
    expect(byService.apple_music).toMatchObject({
      maxQuality: 'lossless', preferredFormat: 'flac',
      fallbackQuality: 'high', fallbackFormat: 'mp3',
    })
  })

  it('restores the metadata preferences row', async () => {
    await reset()

    const updates = callsTo('update_metadata_preferences')
    expect(updates).toHaveLength(1)
    expect(updates[0].args?.settings).toMatchObject({
      enable_musicbrainz: true,
      enable_lastfm: false,
      enable_acoustid: false,
      preserve_custom_tags: true,
      multi_value_separator: ';',
      weight_album: 1,
    })
  })

  it('restores the seeded service priority without dropping connected services', async () => {
    await reset()

    const reorders = callsTo('reorder_service_priorities')
    expect(reorders).toHaveLength(1)
    expect(reorders[0].args?.serviceNames).toEqual(['spotify', 'qobuz'])
  })

  it('turns auto-import back off', async () => {
    await reset()

    const updates = callsTo('update_service_preference')
    expect(updates).toHaveLength(1)
    expect(updates[0].args).toMatchObject({ serviceName: 'qobuz', autoImportEnabled: false })
  })

  it('restores the seeded lyrics provider order and re-enables disabled providers', async () => {
    await reset()

    const reorders = callsTo('reorder_lyrics_providers')
    expect(reorders).toHaveLength(1)
    expect(reorders[0].args?.providerIds).toEqual(['apple_music', 'lrclib', 'genius'])

    const updates = callsTo('update_lyrics_provider')
    expect(updates.length).toBeGreaterThanOrEqual(1)
    expect(updates.some(c => c.args?.providerId === 'genius')).toBe(true)
  })

  it('does nothing when the confirmation is dismissed', async () => {
    confirmMock.mockResolvedValue(false)

    const wrapper = mount(SettingsView, { global: { stubs: { RouterLink: true } } })
    await flushPromises()
    const button = wrapper.findAll('button').find(b => b.text().includes('Reset to Defaults'))
    await button!.trigger('click')
    await flushPromises()

    expect(callsTo('reset_to_defaults')).toHaveLength(0)
  })
})