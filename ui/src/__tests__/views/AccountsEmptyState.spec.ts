/**
 * Accounts empty state (A02 / issue #93, ADR-0007 decision 15).
 *
 * Covers the product contract:
 *  - With zero accounts the screen itself is the empty state: six service
 *    cards with a Connect button, no separate welcome wizard.
 *  - Connect opens the in-app webview login (`start_account_login`, issue #84),
 *    not the legacy bridge/browser auth.
 *  - When the first account is saved, the same mounted view refreshes in place
 *    and shows the account; no reload or remount.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { mockInvoke, resetMocks, emitMockEvent } from '../setup'
import { TauriEvents } from '@/api/tauri'
import type { Account, Service, ServiceStatus } from '@/api/types'
import AccountsView from '@/views/AccountsView.vue'

const SERVICES: Service[] = [
  { id: 1, name: 'spotify', supports_download: 0, max_quality: 'lossy' },
  { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
  { id: 3, name: 'tidal', supports_download: 1, max_quality: 'hires' },
  { id: 4, name: 'deezer', supports_download: 1, max_quality: 'lossless' },
  { id: 5, name: 'soundcloud', supports_download: 1, max_quality: 'lossless' },
  { id: 6, name: 'apple_music', supports_download: 0, max_quality: 'lossless' },
]

function disconnectedStatuses(): ServiceStatus[] {
  return SERVICES.map(service => ({
    name: service.name,
    connected: false,
    account_email: null,
    library_count: 0,
    favorites_count: 0,
    playlists_count: 0,
    last_synced: null,
    credentials_invalid: false,
    invalid_reason: null,
    last_auth_error: null,
  }))
}

const FIRST_ACCOUNT: Account = {
  id: 701,
  service_id: 2,
  service_name: 'qobuz',
  display_name: 'Ana Qobuz',
  email: 'ana@qobuz.example',
  is_active: true,
  state: 'enabled',
  last_synced: null,
  created_at: '2026-11-18T10:00:00Z',
  credentials_invalid: false,
  invalid_reason: null,
  last_auth_error: null,
}

interface InvokeCall {
  command: string
  args?: Record<string, unknown>
}

function installBackend(state: {
  accounts: Account[]
  statuses: ServiceStatus[]
  calls: InvokeCall[]
}) {
  mockInvoke((command, args) => {
    state.calls.push({ command, args })
    if (command === 'get_services') return SERVICES
    if (command === 'get_accounts') return state.accounts
    if (command === 'get_service_statuses') return state.statuses
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
    if (command === 'get_download_settings') return {}
    if (command === 'refresh_account_identities') {
      return { checked: 0, updated: 0, unresolved: 0 }
    }
    if (command === 'start_account_login') {
      return { service: args?.service, profile_key: 'pk-empty-state' }
    }
    return []
  })
}

async function mountAccounts(): Promise<VueWrapper> {
  const wrapper = mount(AccountsView, {
    global: { stubs: { SpotifyApiConfigCard: true } },
  })
  await flushPromises()
  return wrapper
}

beforeEach(() => {
  resetMocks()
  vi.clearAllMocks()
})

describe('Accounts empty state (#93)', () => {
  it('renders the six service cards with a Connect action and no account rows', async () => {
    const state = { accounts: [], statuses: disconnectedStatuses(), calls: [] as InvokeCall[] }
    installBackend(state)

    const wrapper = await mountAccounts()

    expect(wrapper.find('[data-testid="accounts-empty-state"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="accounts-empty-state"]').text()).toContain('No services connected yet')

    const cards = wrapper.findAll('.service-card')
    expect(cards.length).toBe(6)
    for (const card of cards) {
      expect(card.text()).toContain('Not Connected')
      expect(card.text()).toContain('Connect')
    }
    expect(wrapper.findAll('[data-testid^="connect-service-"]').length).toBe(6)
    expect(wrapper.find('[data-testid^="account-row-"]').exists()).toBe(false)
  })

  it('Connect opens the in-app webview login, not the legacy bridge auth', async () => {
    const state = { accounts: [], statuses: disconnectedStatuses(), calls: [] as InvokeCall[] }
    installBackend(state)

    const wrapper = await mountAccounts()
    await wrapper.find('[data-testid="connect-service-qobuz"]').trigger('click')
    await flushPromises()

    const loginCall = state.calls.find(call => call.command === 'start_account_login')
    expect(loginCall?.args).toEqual({ service: 'qobuz' })
    expect(state.calls.some(call => call.command === 'start_auth_and_save')).toBe(false)
    expect(state.calls.some(call => call.command === 'spotify_auth_webview')).toBe(false)

    // Close the attempt so the pending promise does not outlive the test.
    emitMockEvent(TauriEvents.ACCOUNT_LOGIN_CANCELLED, { profile_key: 'pk-empty-state' })
    await flushPromises()
  })

  it('shows the first account in place when the login completes, without remounting', async () => {
    const state = {
      accounts: [] as Account[],
      statuses: disconnectedStatuses(),
      calls: [] as InvokeCall[],
    }
    installBackend(state)
    const wrapper = await mountAccounts()
    expect(wrapper.find('[data-testid="accounts-empty-state"]').exists()).toBe(true)

    await wrapper.find('[data-testid="connect-service-qobuz"]').trigger('click')
    await flushPromises()

    // The backend saves the account before announcing the completed attempt.
    state.accounts = [FIRST_ACCOUNT]
    state.statuses = disconnectedStatuses().map(status =>
      status.name === 'qobuz'
        ? { ...status, connected: true, account_email: FIRST_ACCOUNT.email }
        : status,
    )
    emitMockEvent(TauriEvents.ACCOUNT_LOGIN_COMPLETED, {
      profile_key: 'pk-empty-state',
      account_id: FIRST_ACCOUNT.id,
    })
    await flushPromises()

    expect(wrapper.find('[data-testid="accounts-empty-state"]').exists()).toBe(false)
    const row = wrapper.find(`[data-testid="account-row-${FIRST_ACCOUNT.id}"]`)
    expect(row.exists()).toBe(true)
    expect(row.text()).toContain('Ana Qobuz')

    const qobuzCard = wrapper.findAll('.service-card').find(card => card.text().includes('Qobuz'))
    expect(qobuzCard?.text()).toContain('Connected')
    expect(qobuzCard?.text()).toContain('Add account')

    wrapper.unmount()
  })
})
