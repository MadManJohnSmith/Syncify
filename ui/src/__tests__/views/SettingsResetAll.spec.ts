/**
 * SettingsView "Reset to Defaults" contract (SB00, #196).
 *
 * El reset completo es una operación del backend: `reset_to_defaults('all')`
 * restaura TODAS las superficies tipadas y la KV de ajustes dentro de una
 * transacción (`settings_defaults::reset_all_settings`). La UI solo pide el
 * reset y recarga; ya no compensa en cliente re-sembrando quality, metadata,
 * prioridades de servicio ni proveedores de letras.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'

const confirmMock = vi.fn(async () => true)
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
}))
// R13: la confirmación llega por el diálogo unificado de la app, no por
// plugin-dialog; se intercepta en su nuevo origen dejando el resto intacto.
vi.mock('@/composables/useToast', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/composables/useToast')>()
  return {
    ...actual,
    confirm: (...args: unknown[]) => confirmMock(...(args as [])),
  }
})

import { mockInvoke, resetMocks } from '../setup'
import SettingsView from '@/views/SettingsView.vue'

describe('SettingsView — Reset to Defaults es backend puro (SB00)', () => {
  let invoked: Array<{ command: string; args?: Record<string, unknown> }>

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
    confirmMock.mockResolvedValue(true)
    invoked = []

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
        ]
      }
      if (command === 'get_service_preferences') {
        return [
          { id: 1, service_name: 'qobuz', priority: 1, auto_import_enabled: true },
        ]
      }
      if (command === 'get_lyrics_providers') return []
      if (command === 'get_lyrics_config') return null
      if (command === 'get_metadata_preferences') return null
      if (command === 'get_advanced_settings') return null
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

  it('asks the backend for the full reset', async () => {
    await reset()

    const resets = callsTo('reset_to_defaults')
    expect(resets.length).toBeGreaterThanOrEqual(1)
    expect(resets.some(c => c.args?.settingsType === 'all')).toBe(true)
  })

  it('does not re-seed quality, metadata, priorities or lyrics from the client', async () => {
    await reset()

    // SB00: la compensación en cliente se retiró. Ninguna de estas llamadas
    // es parte del reset; si reaparecen, el reset volvió a depender de la UI.
    expect(callsTo('update_quality_preference')).toHaveLength(0)
    expect(callsTo('update_metadata_preferences')).toHaveLength(0)
    expect(callsTo('update_service_preference')).toHaveLength(0)
    expect(callsTo('reorder_service_priorities')).toHaveLength(0)
    expect(callsTo('reorder_lyrics_providers')).toHaveLength(0)
    expect(callsTo('update_lyrics_provider')).toHaveLength(0)
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
