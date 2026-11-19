/**
 * D09 (#121): preferred download account per service.
 *
 * Ajustes must persist the choice in `service_preferred_accounts` through a
 * dedicated IPC command, and the selector only offers accounts that can
 * actually serve downloads (enabled, same service).
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { mockInvoke, resetMocks, defaultInvokeHandler } from '../setup'
import type { Account, Service } from '@/api/types'
import SettingsServices from '@/views/settings/SettingsServices.vue'

const services: Service[] = [
  { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
  { id: 3, name: 'tidal', supports_download: 1, max_quality: 'lossless' },
]

const accounts: Account[] = [
  {
    id: 11,
    service_id: 2,
    service_name: 'qobuz',
    display_name: 'Qobuz principal',
    email: 'qobuz@example.test',
    is_active: true,
    state: 'enabled',
    last_synced: null,
    created_at: null,
    credentials_invalid: false,
  },
  {
    id: 12,
    service_id: 2,
    service_name: 'qobuz',
    display_name: 'Qobuz secundaria',
    email: 'qobuz2@example.test',
    is_active: false,
    state: 'enabled',
    last_synced: null,
    created_at: null,
    credentials_invalid: false,
  },
  {
    id: 13,
    service_id: 2,
    service_name: 'qobuz',
    display_name: 'Qobuz desactivada',
    email: 'qobuz3@example.test',
    is_active: false,
    state: 'disabled',
    last_synced: null,
    created_at: null,
    credentials_invalid: false,
  },
  {
    id: 14,
    service_id: 3,
    service_name: 'tidal',
    display_name: 'Tidal principal',
    email: 'tidal@example.test',
    is_active: true,
    state: 'enabled',
    last_synced: null,
    created_at: null,
    credentials_invalid: false,
  },
]

describe('SettingsServices — preferred download account', () => {
  let invoked: Array<{ command: string; args?: Record<string, unknown> }>

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
    invoked = []

    mockInvoke((command, args) => {
      invoked.push({ command, args })
      if (command === 'get_services') return services
      if (command === 'get_accounts') return accounts
      if (command === 'get_service_statuses') return []
      if (command === 'get_service_preferences') return []
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
      if (command === 'get_service_preferred_accounts') {
        return [{ service_name: 'qobuz', account_id: 11, updated_at: '2026-11-18 10:00:00' }]
      }
      if (command === 'set_service_preferred_account') {
        const accountId = args?.accountId as number | null
        if (accountId === null) return null
        return {
          service_name: args?.serviceName,
          account_id: accountId,
          updated_at: '2026-11-18 10:05:00',
        }
      }
      return defaultInvokeHandler(command)
    })
  })

  async function mountView() {
    const wrapper = mount(SettingsServices, {
      global: { stubs: { ServiceConnectionModal: true } },
    })
    await flushPromises()
    await flushPromises()
    return wrapper
  }

  it('loads and shows the persisted preferred account for each service', async () => {
    const wrapper = await mountView()

    const qobuz = wrapper.get('[data-testid="preferred-account-qobuz"]')
    expect((qobuz.element as HTMLSelectElement).value).toBe('11')

    const tidal = wrapper.get('[data-testid="preferred-account-tidal"]')
    expect((tidal.element as HTMLSelectElement).value).toBe('')
  })

  it('offers only enabled accounts of the same service', async () => {
    const wrapper = await mountView()

    const values = wrapper
      .get('[data-testid="preferred-account-qobuz"]')
      .findAll('option')
      .map(option => option.attributes('value'))

    expect(values).toEqual(['', '11', '12'])
    expect(values).not.toContain('13') // disabled
    expect(values).not.toContain('14') // Tidal account in Qobuz selector
  })

  it('persists a new preference and clearing it', async () => {
    const wrapper = await mountView()
    const select = wrapper.get('[data-testid="preferred-account-qobuz"]')

    await select.setValue('12')
    await flushPromises()
    expect(invoked).toContainEqual({
      command: 'set_service_preferred_account',
      args: { serviceName: 'qobuz', accountId: 12 },
    })

    await select.setValue('')
    await flushPromises()
    expect(invoked).toContainEqual({
      command: 'set_service_preferred_account',
      args: { serviceName: 'qobuz', accountId: null },
    })
    expect((select.element as HTMLSelectElement).value).toBe('')
  })
})
