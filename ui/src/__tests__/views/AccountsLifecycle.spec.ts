/**
 * Account lifecycle (issue #85, ADR-0007 decision 3).
 *
 * Covers the card-level contract:
 *  - The lifecycle state is visible on the card (Enabled/Active, Disabled,
 *    Archived badges) and persists in the row's data-state attribute.
 *  - Disabling/archiving stops the account via `set_account_state`; the
 *    backend, not the UI, is what halts sync and downloads.
 *  - Archived rows are hidden from the main list but stay reachable under
 *    "Manage accounts" so unarchive remains possible.
 *  - Remove opens a dialog that explains consequences; audio is only deleted
 *    with the separate checkbox (never implicit), and the deletion goes to
 *    `delete_account` (which preserves canonical tracks, sources and files).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { mockInvoke, resetMocks, defaultInvokeHandler } from '../setup'
import type { Account, Service, ServiceStatus } from '@/api/types'
import AccountsView from '@/views/AccountsView.vue'

function buildService(): Service[] {
  return [{ id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' }]
}

function buildStatus(): ServiceStatus[] {
  return [
    {
      name: 'qobuz',
      connected: true,
      credentials_invalid: false,
      library_count: 3,
      favorites_count: 2,
      playlists_count: 1,
      last_synced: null,
      account_email: null,
      invalid_reason: null,
      last_auth_error: null,
    },
  ]
}

function buildAccounts(): Account[] {
  return [
    {
      id: 200,
      service_id: 2,
      service_name: 'qobuz',
      display_name: 'Enabled One',
      email: 'enabled@qobuz.example',
      is_active: true,
      state: 'enabled',
      last_synced: null,
      created_at: '2026-09-01T10:00:00Z',
      credentials_invalid: false,
      invalid_reason: null,
      last_auth_error: null,
    },
    {
      id: 201,
      service_id: 2,
      service_name: 'qobuz',
      display_name: 'Disabled One',
      email: 'disabled@qobuz.example',
      is_active: false,
      state: 'disabled',
      last_synced: null,
      created_at: '2026-09-02T10:00:00Z',
      credentials_invalid: false,
      invalid_reason: null,
      last_auth_error: null,
    },
    {
      id: 202,
      service_id: 2,
      service_name: 'qobuz',
      display_name: 'Archived One',
      email: 'archived@qobuz.example',
      is_active: false,
      state: 'archived',
      last_synced: null,
      created_at: '2026-09-03T10:00:00Z',
      credentials_invalid: false,
      invalid_reason: null,
      last_auth_error: null,
    },
  ]
}

const invokedCommands: { command: string; args: Record<string, unknown> }[] = []

function installBackend(accounts: Account[]) {
  mockInvoke((command, args) => {
    invokedCommands.push({ command, args: (args ?? {}) as Record<string, unknown> })
    if (command === 'get_services') return buildService()
    if (command === 'get_accounts') return accounts
    if (command === 'get_service_statuses') return buildStatus()
    if (command === 'get_service_preferences') return []
    if (command === 'get_service_sync_settings') return []
    if (command === 'get_sync_settings') {
      return {
        id: 1,
        auto_sync_enabled: false,
        sync_interval_value: 1,
        sync_interval_unit: 'hours',
        sync_on_startup: false,
        background_download: false,
        max_concurrent_downloads: 3,
        rate_limit_delay_ms: 500,
        pause_on_metered: false,
        pause_on_low_battery: false,
      }
    }
    if (command === 'get_download_settings') return {}
    return defaultInvokeHandler(command)
  })
}

async function mountAccounts(): Promise<VueWrapper> {
  const wrapper = mount(AccountsView, {
    global: { stubs: { SpotifyApiConfigCard: true, ServiceConnectionModal: true } },
    attachTo: document.body,
  })
  await flushPromises()
  return wrapper
}

beforeEach(() => {
  resetMocks()
  vi.clearAllMocks()
  invokedCommands.length = 0
  document.body.innerHTML = ''
})

describe('lifecycle states on the account card', () => {
  it('shows the state badge and data-state for disabled and archived rows', async () => {
    installBackend(buildAccounts())
    const wrapper = await mountAccounts()

    const enabledRow = wrapper.find('[data-testid="account-row-200"]')
    expect(enabledRow.exists()).toBe(true)
    expect(enabledRow.attributes('data-state')).toBe('enabled')
    expect(enabledRow.text()).toContain('Active')

    const disabledRow = wrapper.find('[data-testid="account-row-201"]')
    expect(disabledRow.exists()).toBe(true)
    expect(disabledRow.attributes('data-state')).toBe('disabled')
    expect(disabledRow.find('[data-testid="account-state-badge"]').text()).toBe('Disabled')

    wrapper.unmount()
  })

  it('hides archived rows from the main list but keeps them under Manage accounts', async () => {
    installBackend(buildAccounts())
    const wrapper = await mountAccounts()

    expect(wrapper.find('[data-testid="account-row-202"]').exists()).toBe(false)
    const manageRow = wrapper.find('[data-testid="manage-account-row-202"]')
    expect(manageRow.exists()).toBe(true)
    expect(manageRow.text()).toContain('Archived')
    expect(manageRow.text()).toContain('Enable')
    wrapper.unmount()
  })

  it('offers Disable and Archive on enabled rows and Enable on stopped rows', async () => {
    installBackend(buildAccounts())
    const wrapper = await mountAccounts()

    expect(wrapper.find('[data-testid="account-row-200"]').find('[data-testid="account-disable"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="account-row-200"]').find('[data-testid="account-archive"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="account-row-201"]').find('[data-testid="account-enable"]').exists()).toBe(true)

    await wrapper.find('[data-testid="account-row-200"] [data-testid="account-disable"]').trigger('click')
    await flushPromises()
    const transition = invokedCommands.find(c => c.command === 'set_account_state')
    expect(transition).toBeDefined()
    expect(transition?.args).toMatchObject({ accountId: 200, newState: 'disabled' })
    wrapper.unmount()
  })
})

describe('the delete dialog', () => {
  it('explains consequences, keeps audio unless the checkbox is set, and calls delete_account', async () => {
    installBackend(buildAccounts().filter(a => a.id === 201))
    const stopped = await mountAccounts()
    const removeButton = stopped
      .find('[data-testid="account-row-201"]')
      .findAll('button')
      .find(b => b.text() === 'Remove')
    expect(removeButton).toBeDefined()
    await removeButton!.trigger('click')
    await flushPromises()

    // El diálogo vive en un Teleport: se busca en el documento, no en el wrapper.
    const dialogEl = document.body.querySelector('[data-testid="delete-account-dialog"]')
    expect(dialogEl).not.toBeNull()
    const dialogText = dialogEl!.textContent ?? ''
    expect(dialogText).toContain('Kept')
    expect(dialogText).toContain('Deleted')

    // Sin casilla: el audio se conserva.
    ;(dialogEl!.querySelector('[data-testid="confirm-delete-account"]') as HTMLButtonElement).click()
    await flushPromises()
    const deletion = invokedCommands.find(c => c.command === 'delete_account')
    expect(deletion).toBeDefined()
    expect(deletion?.args).toMatchObject({ accountId: 201, deleteAudio: false })
    expect(document.body.querySelector('[data-testid="delete-account-dialog"]')).toBeNull()
    stopped.unmount()
  })
})
