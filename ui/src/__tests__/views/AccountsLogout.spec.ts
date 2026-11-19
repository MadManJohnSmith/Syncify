/**
 * Cerrar sesión de una cuenta (issue #91, ADR-0007 decisión 9).
 *
 * Cubre el contrato de la vista:
 *  - Disconnect invoca `logout_account` para ESA cuenta (no borra la fila).
 *  - La tarjeta refleja el estado «sin sesión» en el momento, sin recargar la
 *    página: insignia Reauth y `data-invalid="true"`.
 *  - La cuenta sigue listada con su etiqueta/correo: se conserva junto con su
 *    biblioteca, y la acción destructiva sigue siendo Remove (aparte).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { mockInvoke, resetMocks, defaultInvokeHandler } from '../setup'
import type { Account, Service, ServiceStatus } from '@/api/types'
import AccountsView from '@/views/AccountsView.vue'

const ACCOUNT_ID = 210

function buildService(): Service[] {
  return [{ id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' }]
}

function buildStatus(loggedOut: boolean): ServiceStatus[] {
  return [
    {
      name: 'qobuz',
      connected: true,
      credentials_invalid: loggedOut,
      invalid_reason: loggedOut ? 'logged_out' : null,
      library_count: 3,
      favorites_count: 2,
      playlists_count: 1,
      last_synced: null,
      account_email: 'keep@qobuz.example',
      last_auth_error: loggedOut ? 'Session closed by the user' : null,
    },
  ]
}

function buildAccount(loggedOut: boolean): Account[] {
  return [
    {
      id: ACCOUNT_ID,
      service_id: 2,
      service_name: 'qobuz',
      display_name: 'Cuenta que se conserva',
      email: 'keep@qobuz.example',
      is_active: !loggedOut,
      state: 'enabled',
      last_synced: null,
      created_at: '2026-09-01T10:00:00Z',
      credentials_invalid: loggedOut,
      invalid_reason: loggedOut ? 'logged_out' : null,
      last_auth_error: loggedOut ? 'Session closed by the user' : null,
    },
  ]
}

const invokedCommands: { command: string; args: Record<string, unknown> }[] = []
let loggedOut = false

function installBackend() {
  mockInvoke((command, args) => {
    invokedCommands.push({ command, args: (args ?? {}) as Record<string, unknown> })
    if (command === 'get_services') return buildService()
    if (command === 'get_accounts') return buildAccount(loggedOut)
    if (command === 'get_service_statuses') return buildStatus(loggedOut)
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
    if (command === 'logout_account') {
      loggedOut = true
      return {
        account_id: ACCOUNT_ID,
        service_id: 2,
        service: 'qobuz',
        state: 'enabled',
        is_active: false,
        new_active_account_id: null,
        remaining_accounts: 0,
        profiles_deleted: 1,
      }
    }
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
  loggedOut = false
  document.body.innerHTML = ''
})

describe('cerrar sesión de una cuenta', () => {
  it('llama a logout_account para esa cuenta y deja la tarjeta «sin sesión» sin recargar', async () => {
    installBackend()
    const wrapper = await mountAccounts()

    const rowBefore = wrapper.find(`[data-testid="account-row-${ACCOUNT_ID}"]`)
    expect(rowBefore.exists()).toBe(true)
    expect(rowBefore.attributes('data-invalid')).toBe('false')
    expect(rowBefore.text()).toContain('Cuenta que se conserva')

    // El botón de desconectar de la tarjeta de servicio actúa sobre la cuenta
    // resuelta (la activa), no sobre el servicio entero.
    const disconnect = wrapper.findAll('button').find(button => button.text() === 'Disconnect')
    expect(disconnect).toBeDefined()
    await disconnect!.trigger('click')
    await flushPromises()

    const logout = invokedCommands.find(call => call.command === 'logout_account')
    expect(logout).toBeDefined()
    expect(logout?.args).toEqual({ accountId: ACCOUNT_ID })
    expect(invokedCommands.some(call => call.command === 'delete_account')).toBe(false)
    expect(invokedCommands.some(call => call.command === 'logout_service')).toBe(false)

    // La fila sigue en la lista (cuenta y biblioteca conservadas) y su estado
    // ya es «sin sesión»: insignia Reauth, sin participación activa.
    const rowAfter = wrapper.find(`[data-testid="account-row-${ACCOUNT_ID}"]`)
    expect(rowAfter.exists()).toBe(true)
    expect(rowAfter.attributes('data-invalid')).toBe('true')
    expect(rowAfter.attributes('data-active')).toBe('false')
    expect(rowAfter.text()).toContain('Reauth')
    expect(rowAfter.text()).toContain('Cuenta que se conserva')

    wrapper.unmount()
  })

  it('expone logout_account como wrapper propio de la API', async () => {
    const { logoutAccount } = await import('@/api/accounts')
    mockInvoke((command, args) => {
      invokedCommands.push({ command, args: (args ?? {}) as Record<string, unknown> })
      return null
    })

    await logoutAccount(ACCOUNT_ID)

    expect(invokedCommands).toContainEqual({
      command: 'logout_account',
      args: { accountId: ACCOUNT_ID },
    })
  })
})
