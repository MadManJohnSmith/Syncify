/**
 * SettingsServices.vue drag-to-reorder regression (R9).
 *
 * "Drag to reorder" was advertised but the row only had arrow buttons; the
 * drag now emits a real drop index and the new order is persisted through
 * reorder_service_priorities.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { mockInvoke, resetMocks } from '../setup'
import type { Service, Account, ServiceStatus, ServicePreference } from '@/api/types'
import { useSyncSettings } from '@/composables/useSyncSettings'
import SettingsServices from '@/views/settings/SettingsServices.vue'

function dragEventStub() {
  return { dataTransfer: { setData: () => {}, effectAllowed: '' } }
}

describe('SettingsServices — download priority drag and drop', () => {
  let invokedCommands: Array<{ command: string; args?: Record<string, unknown> }>
  let mockPreferences: ServicePreference[]

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
    invokedCommands = []

    const mockServices: Service[] = [
      { id: 1, name: 'spotify', supports_download: 1, max_quality: '320kbps' },
      { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
      { id: 3, name: 'tidal', supports_download: 1, max_quality: 'lossless' },
    ]
    const mockAccounts: Account[] = []
    const mockStatuses: ServiceStatus[] = []

    mockPreferences = [
      { id: 1, service_name: 'spotify', priority: 1, auto_import_enabled: false },
      { id: 2, service_name: 'qobuz', priority: 2, auto_import_enabled: false },
      { id: 3, service_name: 'tidal', priority: 3, auto_import_enabled: false },
    ]

    mockInvoke((command, args) => {
      invokedCommands.push({ command, args })
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'get_service_preferences') return mockPreferences
      if (command === 'get_service_sync_settings') return []
      if (command === 'get_sync_settings') {
        return {
          id: 1,
          auto_sync_enabled: true,
          sync_interval_value: 1,
          sync_interval_unit: 'hours',
          sync_on_startup: true,
          background_download: true,
          max_concurrent_downloads: 3,
          rate_limit_delay_ms: 500,
          pause_on_metered: true,
          pause_on_low_battery: true,
        }
      }
      if (command === 'reorder_service_priorities') {
        const order = (args?.serviceNames as string[]) ?? []
        mockPreferences = order.map((name, index) => ({
          id: index + 1,
          service_name: name,
          priority: index + 1,
          auto_import_enabled: false,
        }))
        return mockPreferences
      }
      if (command === 'get_download_settings') return null
      if (command === 'get_advanced_settings') {
        return {
          id: 1,
          log_level: 'info',
          log_to_file: true,
          log_file_max_size_mb: 50,
          log_file_retention_days: 30,
          max_concurrent_downloads: 3,
          max_concurrent_imports: 2,
          worker_timeout_seconds: 300,
          cache_enabled: true,
          cache_max_size_mb: 500,
          cache_ttl_hours: 168,
          fuzzy_match_threshold: 0.85,
          use_acoustic_fingerprinting: true,
          prefer_exact_matches: true,
          request_timeout_seconds: 30,
          max_retries: 3,
          retry_delay_seconds: 5,
          use_proxy: false,
          proxy_url: null,
          debug_mode: false,
          verbose_api_logging: false,
        }
      }
      return null
    })

    // useSyncSettings is a module singleton and the view only loads it when the
    // list is empty, so clear it to give each test the seeded order.
    useSyncSettings().servicePreferences.value = []
  })

  async function mountView() {
    const wrapper = mount(SettingsServices)
    await flushPromises()
    await flushPromises()
    return wrapper
  }

  function reorderCalls() {
    return invokedCommands.filter(c => c.command === 'reorder_service_priorities')
  }

  it('renders the priority list as draggable rows', async () => {
    const wrapper = await mountView()

    const rows = wrapper.findAll('[data-testid^="draggable-item-"]')
    expect(rows).toHaveLength(3)
    rows.forEach(row => expect(row.attributes('draggable')).toBe('true'))
  })

  it('persists the new priority order after a drop', async () => {
    const wrapper = await mountView()
    reorderCalls().length = 0

    const rows = wrapper.findAll('[data-testid^="draggable-item-"]')
    await rows[2].trigger('dragstart', dragEventStub())
    await rows[0].trigger('drop', dragEventStub())
    await flushPromises()

    const calls = reorderCalls()
    expect(calls).toHaveLength(1)
    expect(calls[0].args?.serviceNames).toEqual(['tidal', 'spotify', 'qobuz'])
  })

  it('does not persist when the row lands back on its own position', async () => {
    const wrapper = await mountView()
    reorderCalls().length = 0

    const rows = wrapper.findAll('[data-testid^="draggable-item-"]')
    await rows[1].trigger('dragstart', dragEventStub())
    await rows[1].trigger('drop', dragEventStub())
    await flushPromises()

    expect(reorderCalls()).toHaveLength(0)
  })

  it('moves a row down past the end by dragging onto a later row', async () => {
    const wrapper = await mountView()
    reorderCalls().length = 0

    const rows = wrapper.findAll('[data-testid^="draggable-item-"]')
    await rows[0].trigger('dragstart', dragEventStub())
    await rows[2].trigger('drop', dragEventStub())
    await flushPromises()

    expect(reorderCalls()[0].args?.serviceNames).toEqual(['qobuz', 'tidal', 'spotify'])
  })

  it('still persists a move made with the arrow buttons', async () => {
    const wrapper = await mountView()
    reorderCalls().length = 0

    await wrapper.get('button[aria-label="Move Spotify down"]').trigger('click')
    await flushPromises()

    expect(reorderCalls()[0].args?.serviceNames).toEqual(['qobuz', 'spotify', 'tidal'])
  })

  it('stops the first row from moving up and the last from moving down', async () => {
    const wrapper = await mountView()

    expect(wrapper.get('button[aria-label="Move Spotify up"]').attributes('disabled')).toBeDefined()
    expect(wrapper.get('button[aria-label="Move Tidal down"]').attributes('disabled')).toBeDefined()
  })
})