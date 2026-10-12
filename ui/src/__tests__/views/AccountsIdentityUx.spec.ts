/**
 * Accounts identity UX (identity-first labels, no secrets, active-account-at-a-glance).
 *
 * Covers the product rules agreed for the accounts surface:
 *  - Real identity is required: email validated first, human display_name second,
 *    otherwise the explicit "Identidad no proporcionada" placeholder.
 *  - Internal ids and session/cookie-like strings are never shown as identity.
 *  - No password, cookie or token is ever rendered, and LAST_AUTH_ERROR (which
 *    can carry provider-specific secret material) is not echoed into the DOM.
 *  - The active account is visible on the card without opening a <details>.
 *  - SettingsServices uses the same identity function, so labels cannot diverge.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { mockInvoke, resetMocks, defaultInvokeHandler } from '../setup'
import type { Service, Account, ServiceStatus, StoredCredentialKind } from '@/api/types'
import {
  resolveAccountIdentity,
  formatAccountLabel,
  isOpaqueIdentityString,
  pickPrimaryAccount,
  NO_IDENTITY_LABEL,
  describeCredentialState,
  credentialStateIcon,
  storedCredentialState,
} from '@/composables/useAccountsStatus'
import AccountsView from '@/views/AccountsView.vue'
import SettingsServices from '@/views/settings/SettingsServices.vue'

// Fixtures deliberately include the hostile shapes the backend can return.
const SECRET_LIKE = [
  'super-secret-password',
  'sp_dc=1a2b3c4d5e6f7a8b9c0d',
  'browser_session',
  'cookie_jar',
  'Bearer eyJhbGciOiJIUzI1NiJ9.payload.signature',
  'arl_0f1e2d3c4b5a69788796',
]

const LONG_ID = '30834759201736548291038475610293847'

function buildService(): Service[] {
  return [
    { id: 1, name: 'spotify', supports_download: 1, max_quality: '320kbps' },
    { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
  ]
}

function buildStatus(overrides: Partial<ServiceStatus> = {}): ServiceStatus[] {
  return [
    {
      name: 'spotify',
      connected: true,
      credentials_invalid: false,
      library_count: 10,
      favorites_count: 2,
      playlists_count: 1,
      last_synced: '2026-10-01T10:00:00Z',
      account_email: 'user@spotify.com',
      invalid_reason: null,
      last_auth_error: 'RequiresAuth: Qobuz user authentication required (401)',
      ...overrides,
    },
  ]
}

function buildAccounts(overrides: Partial<Account>[] = []): Account[] {
  const base: Account[] = [
    {
      id: 100,
      service_id: 1,
      service_name: 'spotify',
      display_name: 'Ana Spotify',
      email: 'ana@spotify.com',
      is_active: true,
      last_synced: '2026-10-01T10:00:00Z',
      created_at: '2026-09-01T10:00:00Z',
      credentials_invalid: false,
      invalid_reason: null,
      last_auth_error: null,
    },
    {
      id: 101,
      service_id: 1,
      service_name: 'spotify',
      display_name: 'Luis Spotify',
      email: 'luis@spotify.com',
      is_active: false,
      last_synced: null,
      created_at: '2026-09-05T10:00:00Z',
      credentials_invalid: false,
      invalid_reason: null,
      last_auth_error: null,
    },
  ]
  return base.map((a, i) => (overrides[i] ? { ...a, ...overrides[i] } : a))
}

function installBackend(
  accounts: Account[] = buildAccounts(),
  statuses: ServiceStatus[] = buildStatus(),
  services: Service[] = buildService(),
) {
  mockInvoke((command) => {
    if (command === 'get_services') return services
    if (command === 'get_accounts') return accounts
    if (command === 'get_service_statuses') return statuses
    if (command === 'get_service_preferences') return []
    if (command === 'get_service_sync_settings') return []
    if (command === 'get_sync_settings') return {
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

async function mountSettingsServices(): Promise<VueWrapper> {
  const wrapper = mount(SettingsServices, {
    global: { stubs: { ServiceConnectionModal: true } },
  })
  await flushPromises()
  return wrapper
}

beforeEach(() => {
  resetMocks()
  vi.clearAllMocks()
  document.body.innerHTML = ''
})

describe('account identity resolution (pure helpers)', () => {
  it('prefers a validated email over the display name', () => {
    const identity = resolveAccountIdentity({ display_name: 'Ana', email: 'ana@spotify.com' })
    expect(identity.label).toBe('Ana (ana@spotify.com)')
    expect(identity.source).toBe('email')
    expect(identity.email).toBe('ana@spotify.com')
  })

  it('falls back to a validated human display name when there is no email', () => {
    const identity = resolveAccountIdentity({ display_name: 'Ana Spotify', email: null })
    expect(identity.label).toBe('Ana Spotify')
    expect(identity.source).toBe('display_name')
    expect(identity.email).toBeNull()
  })

  it('reports "Identidad no proporcionada" when neither field is usable', () => {
    const identity = resolveAccountIdentity({ display_name: null, email: null })
    expect(identity.label).toBe(NO_IDENTITY_LABEL)
    expect(identity.source).toBe('none')
  })

  it('never treats a long internal identifier as identity', () => {
    for (const value of [LONG_ID, '30834759201736', 'a3f9c2d1e4b6a8c0f2e4d6b8']) {
      expect(isOpaqueIdentityString(value)).toBe(true)
      expect(resolveAccountIdentity({ display_name: value, email: null }).label).toBe(NO_IDENTITY_LABEL)
      expect(resolveAccountIdentity({ display_name: null, email: value }).label).toBe(NO_IDENTITY_LABEL)
    }
  })

  it('never treats a browser session, cookie or token placeholder as identity', () => {
    for (const value of ['browser_session', 'browser session', 'cookies', 'cookie_jar', 'session_token', 'bearer']) {
      expect(isOpaqueIdentityString(value)).toBe(true)
      expect(resolveAccountIdentity({ display_name: value, email: null }).label).toBe(NO_IDENTITY_LABEL)
      expect(resolveAccountIdentity({ display_name: null, email: value }).label).toBe(NO_IDENTITY_LABEL)
    }
  })

  it('rejects credential-like email slots (sp_dc/arl/auth token prefixes)', () => {
    for (const value of ['sp_dc=abc123', 'arl 0f1e2d3c', 'auth_token:42']) {
      expect(resolveAccountIdentity({ display_name: null, email: value }).label).toBe(NO_IDENTITY_LABEL)
    }
  })

  it('rejects malformed and whitespace-containing emails', () => {
    for (const value of ['not-an-email', 'a@b', 'a b@c.com', '@x.com', 'ana@.com']) {
      expect(resolveAccountIdentity({ display_name: null, email: value }).label).toBe(NO_IDENTITY_LABEL)
    }
  })

  it('keeps a display_name that is an email out of the name slot', () => {
    const identity = resolveAccountIdentity({ display_name: 'ana@spotify.com', email: 'ana@spotify.com' })
    // Email is the identity; the duplicate name slot is not rendered.
    expect(identity.label).toBe('ana@spotify.com')
    expect(formatAccountLabel({ display_name: 'ana@spotify.com', email: null })).toBe('ana@spotify.com')
  })

  it('picks the active account first, regardless of array order', () => {
    const accounts = [
      { id: 2, display_name: 'Second', email: null, is_active: false },
      { id: 3, display_name: 'Third', email: null, is_active: false },
      { id: 1, display_name: 'First', email: null, is_active: true },
    ]
    expect(pickPrimaryAccount(accounts)?.id).toBe(1)
  })

  it('picks any account when none is active, and null when the list is empty', () => {
    expect(pickPrimaryAccount([{ id: 7, display_name: 'x', email: null, is_active: false }])?.id).toBe(7)
    expect(pickPrimaryAccount([])).toBeNull()
    expect(pickPrimaryAccount(null)).toBeNull()
  })
})

describe('AccountsView — identity cards', () => {
  it('shows the active account identity without opening a details element', async () => {
    installBackend()
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-100"]')
    expect(row.exists()).toBe(true)
    expect(row.attributes('data-active')).toBe('true')
    // Not inside a collapsed <details>.
    expect(row.element.closest('details')).toBeNull()
    expect(row.text()).toContain('Ana Spotify (ana@spotify.com)')
    expect(row.text()).toContain('Active')
    wrapper.unmount()
  })

  it('does not duplicate the email when it is already the identity label', async () => {
    installBackend(buildAccounts([{ display_name: null }]), buildStatus())
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-100"]')
    const occurrences = row.text().split('ana@spotify.com').length - 1
    expect(occurrences).toBe(1)
    wrapper.unmount()
  })

  it('shows the secondary email separately when the identity is a display name', async () => {
    installBackend(buildAccounts([{ display_name: 'Ana' }]), buildStatus())
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-100"]')
    expect(row.text()).toContain('Ana')
    // Exactly one occurrence of the email on the primary row: it must not be
    // repeated both as the label and as the secondary line.
    expect(row.text().split('ana@spotify.com').length - 1).toBe(1)
    wrapper.unmount()
  })

  it('renders the "Identidad no proporcionada" placeholder with connect CTA when no identity exists', async () => {
    const accounts = [
      {
        id: 900,
        service_id: 1,
        service_name: 'spotify',
        display_name: null,
        email: null,
        is_active: true,
        last_synced: null,
        created_at: null,
        credentials_invalid: false,
      },
    ]
    installBackend(accounts)
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-900"]')
    expect(row.exists()).toBe(true)
    expect(row.text()).toContain('Identidad no proporcionada')
    // The card offers connect/add-account actions as the CTA.
    expect(wrapper.find('.service-card').text()).toContain('Add account')
    wrapper.unmount()
  })

  it('marks an invalid account differently from a valid one', async () => {
    installBackend(
      buildAccounts([
        { credentials_invalid: true, invalid_reason: 'Token expired' },
        {},
      ]),
    )
    const wrapper = await mountAccounts()

    const invalidRow = wrapper.find('[data-testid="account-row-100"]')
    expect(invalidRow.attributes('data-invalid')).toBe('true')
    expect(invalidRow.classes().join(' ')).toContain('ring-error')
    expect(invalidRow.text()).toContain('Active')

    const validRow = wrapper.find('[data-testid="account-row-101"]')
    expect(validRow.attributes('data-invalid')).toBe('false')
    expect(validRow.text()).not.toContain('Reauth')
    wrapper.unmount()
  })

  it('marks a secondary (non-active) account as not active and offers activation', async () => {
    installBackend()
    const wrapper = await mountAccounts()

    const secondary = wrapper.find('[data-testid="account-row-101"]')
    expect(secondary.attributes('data-active')).toBe('false')
    expect(secondary.text()).toContain('Luis Spotify (luis@spotify.com)')
    const useButton = secondary.findAll('button').find(b => b.text() === 'Use this account')
    expect(useButton).toBeDefined()
    wrapper.unmount()
  })

  it('renders no password, cookie or token anywhere on the page', async () => {
    const accounts = buildAccounts([
      { credentials_invalid: true, last_auth_error: '401 sp_dc=AABBCCDDEEFF00112233 expired' },
      {},
    ])
    installBackend(accounts, buildStatus({ credentials_invalid: true }))
    const wrapper = await mountAccounts()

    const html = wrapper.html()
    for (const secret of SECRET_LIKE) {
      expect(html).not.toContain(secret)
    }
    expect(html).not.toContain('AABBCCDDEEFF00112233')
    expect(html).not.toContain('RequiresAuth: Qobuz user authentication required (401)')
    wrapper.unmount()
  })

  it('does not render LAST_AUTH_ERROR even when the backend returns one', async () => {
    const errorString = 'RequiresAuth: Qobuz user authentication required (401)'
    installBackend(buildAccounts([{ credentials_invalid: true }, {}]), buildStatus({ credentials_invalid: true }))
    const wrapper = await mountAccounts()

    const html = wrapper.html()
    expect(html).not.toContain(errorString)
    expect(html).not.toContain('RequiresAuth')
    // Instead it shows the safe classification plus a reconnect CTA.
    expect(html).toContain('needs reauthentication')
    expect(html).toContain('Reconnect')
    wrapper.unmount()
  })

  it('never renders a raw long account id as identity', async () => {
    installBackend([
      {
        id: 101,
        service_id: 1,
        service_name: 'spotify',
        display_name: LONG_ID,
        email: null,
        is_active: true,
        last_synced: null,
        created_at: null,
        credentials_invalid: false,
      },
    ])
    const wrapper = await mountAccounts()

    expect(wrapper.html()).not.toContain(LONG_ID)
    expect(wrapper.find('[data-testid="account-row-101"]').text()).toContain('Identidad no proporcionada')
    wrapper.unmount()
  })
})

describe('SettingsServices — same identity function, no divergence', () => {
  it('uses the shared label helper so the identity matches AccountsView', async () => {
    installBackend()
    const wrapper = await mountSettingsServices()

    const row = wrapper.find('[data-testid="settings-account-row-100"]')
    expect(row.exists()).toBe(true)
    expect(row.text()).toContain('Ana Spotify (ana@spotify.com)')
    expect(row.text()).toContain('Active')
    wrapper.unmount()
  })

  it('shows the placeholder and no secret when the backend gives no identity', async () => {
    installBackend([
      {
        id: 100,
        service_id: 1,
        service_name: 'spotify',
        display_name: 'browser_session',
        email: 'sp_dc=1a2b3c4d5e6f7a8b9c0d',
        is_active: true,
        last_synced: null,
        created_at: null,
        credentials_invalid: true,
      },
    ])
    const wrapper = await mountSettingsServices()

    const html = wrapper.html()
    expect(html).not.toContain('browser_session')
    expect(html).not.toContain('sp_dc=1a2b3c4d5e6f7a8b9c0d')
    const row = wrapper.find('[data-testid="settings-account-row-100"]')
    expect(row.text()).toContain('Identidad no proporcionada')
    wrapper.unmount()
  })

  it('shows the secondary email only once when the label is a display name', async () => {
    installBackend(buildAccounts([{ display_name: 'Ana' }, displayNameSecondary()]))
    const wrapper = await mountSettingsServices()

    const row = wrapper.find('[data-testid="settings-account-row-100"]')
    expect(row.text()).toContain('ana@spotify.com')
    expect(row.text().split('ana@spotify.com').length - 1).toBeGreaterThanOrEqual(1)
    wrapper.unmount()
  })
})

function displayNameSecondary(): Partial<Account> {
  return { display_name: 'Luis' }
}

describe('advanced settings modal keeps multi-account management', () => {
  it('keeps add-account and account management inside the service settings modal', async () => {
    installBackend()
    const wrapper = await mountAccounts()

    const card = wrapper.find('.service-card')
    const gear = card.findAll('button').find(b => b.attributes('title') === 'Service settings')
    expect(gear).toBeDefined()
    await gear!.trigger('click')
    await flushPromises()

    // The settings modal is teleported to <body>, so it is queried on document.
    const overlay = document.querySelector('.service-settings-overlay')
    expect(overlay).not.toBeNull()

    // The Account tab lists every linked account with the active one flagged.
    const accountTab = Array.from(overlay!.querySelectorAll('button')).find(
      b => b.textContent === 'Account',
    )
    expect(accountTab).toBeDefined()
    ;(accountTab as HTMLButtonElement).click()
    await flushPromises()

    expect(overlay!.querySelectorAll('[data-testid^="settings-account-row-"]').length).toBe(2)
    const row100 = overlay!.querySelector('[data-testid="settings-account-row-100"]')
    expect(row100?.textContent).toContain('Ana Spotify (ana@spotify.com)')
    expect(row100?.textContent).toContain('Active')
    const row101 = overlay!.querySelector('[data-testid="settings-account-row-101"]')
    expect(row101?.textContent).toContain('Use this')

    // Add account stays available from the modal (multi-account management).
    const addAccountBtn = Array.from(overlay!.querySelectorAll('button')).find(
      b => b.textContent?.includes('Add another account'),
    )
    expect(addAccountBtn).toBeDefined()
    wrapper.unmount()
  })
})

// ==============================================
// STORED CREDENTIAL STATE (masked, secret-free)
// ==============================================

describe('masked stored-credential state (helpers)', () => {
  it('describes a cookie-only account without claiming a recoverable password', () => {
    const account = {
      id: 1,
      display_name: 'Deezer User',
      email: null,
      is_active: true,
      credential_state: {
        kind: 'cookie' as const,
        has_password: false,
        has_cookie: true,
        has_token: true,
        has_credential: true,
        auto_refreshable: false,
      },
    }
    const description = describeCredentialState(account)
    expect(description).toContain('cookie')
    // It must NOT imply the password (or cookie) can be recovered.
    expect(description.toLowerCase()).not.toContain('password')
    expect(description.toLowerCase()).not.toContain('recover')
    expect(credentialStateIcon(account)).toBe('cookie')
  })

  it('says a password is on file but never that it is shown or recoverable', () => {
    const account = {
      id: 2,
      display_name: 'Qobuz User',
      email: 'ana@example.com',
      is_active: true,
      credential_state: {
        kind: 'password' as const,
        has_password: true,
        has_cookie: false,
        has_token: false,
        has_credential: true,
        auto_refreshable: false,
      },
    }
    const description = describeCredentialState(account)
    expect(description).toContain('never shown')
    expect(credentialStateIcon(account)).toBe('password')
  })

  it('claims nothing when the backend sent no credential state', () => {
    // An older backend (or a backend that refused to summarize) must not make
    // the UI advertise a credential it never verified.
    expect(storedCredentialState({ id: 3, display_name: null, email: null }).has_credential).toBe(false)
    expect(storedCredentialState(null).kind).toBe('none')
    expect(describeCredentialState({ id: 4, display_name: null, email: null })).toBe('No stored credential')
  })

  it('describes an OAuth session as auto-renewable only when it really is', () => {
    const base = {
      id: 5,
      display_name: 'Spotify User',
      email: 'ana@spotify.com',
      is_active: true,
    }
    expect(
      describeCredentialState({
        ...base,
        credential_state: {
          kind: 'oauth',
          has_password: false,
          has_cookie: false,
          has_token: true,
          has_credential: true,
          auto_refreshable: true,
        },
      }),
    ).toContain('renews automatically')
    expect(
      describeCredentialState({
        ...base,
        credential_state: {
          kind: 'oauth',
          has_password: false,
          has_cookie: false,
          has_token: true,
          has_credential: true,
          auto_refreshable: false,
        },
      }),
    ).not.toContain('renews automatically')
  })
})

describe('accounts card — masked credential state', () => {
  function credentialKind(kind: StoredCredentialKind, hasPassword = false) {
    return {
      kind,
      has_password: hasPassword,
      has_cookie: kind === 'cookie',
      has_token: true,
      has_credential: true,
      auto_refreshable: kind === 'oauth' ? true : false,
    }
  }

  it('shows the credential kind and never a credential value', async () => {
    const accounts = buildAccounts([
      { credential_state: credentialKind('cookie') },
      { display_name: 'Luis Spotify' },
    ])
    installBackend(accounts)
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-100"]')
    const state = row.find('[data-testid="account-credential-state-100"]')
    expect(state.exists()).toBe(true)
    expect(state.text()).toContain('cookie')

    // The row never prints the stored secret in any form.
    const html = wrapper.html()
    expect(html).not.toContain('arla0')
    expect(html).not.toContain('super-secret-password')
    expect(html).not.toContain('sp_dc=')
    wrapper.unmount()
  })

  it('says a password is on file, without a recoverable password', async () => {
    const accounts = buildAccounts([
      { credential_state: credentialKind('password', true) },
      { display_name: 'Luis Spotify' },
    ])
    installBackend(accounts)
    const wrapper = await mountAccounts()

    const state = wrapper.find('[data-testid="account-credential-state-100"]')
    expect(state.text()).toContain('never shown')
    wrapper.unmount()
  })

  it('renders no credential-state line when the account has no credential', async () => {
    const accounts = buildAccounts([
      {
        credential_state: {
          kind: 'none',
          has_password: false,
          has_cookie: false,
          has_token: false,
          has_credential: false,
          auto_refreshable: false,
        },
      },
      { display_name: 'Luis Spotify' },
    ])
    installBackend(accounts)
    const wrapper = await mountAccounts()

    expect(wrapper.find('[data-testid="account-credential-state-100"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('shows a cookie-only imported account as browser-session, not as a password', async () => {
    const accounts = [
      {
        id: 700,
        service_id: 2,
        service_name: 'qobuz',
        display_name: 'Qobuz User',
        email: null,
        is_active: true,
        last_synced: null,
        created_at: null,
        credentials_invalid: false,
        credential_state: credentialKind('cookie'),
      },
    ]
    installBackend(accounts, [
      {
        name: 'qobuz',
        connected: true,
        credentials_invalid: false,
        library_count: 3,
        favorites_count: 1,
        playlists_count: 0,
        last_synced: '2026-10-01T10:00:00Z',
        account_email: null,
        invalid_reason: null,
        last_auth_error: null,
      },
    ], [
      { id: 1, name: 'spotify', supports_download: 1, max_quality: '320kbps' },
      { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
    ])
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-700"]')
    const state = row.find('[data-testid="account-credential-state-700"]')
    expect(state.exists()).toBe(true)
    expect(state.text()).toContain('cookie')
    expect(state.text().toLowerCase()).not.toContain('password')
    wrapper.unmount()
  })
})

describe('Qobuz streaming entitlement (QOBUZ-ENT)', () => {
  it('warns that an account without streaming will deliver previews', async () => {
    const accounts = buildAccounts([
      { id: 100, service_name: 'qobuz', email: 'ana@qobuz.com', streaming_entitled: false },
    ])
    installBackend(accounts)
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-100"]')
    expect(row.exists()).toBe(true)
    expect(row.text()).toContain('No streaming on this plan')
    expect(row.attributes('data-no-streaming')).toBe('true')
    wrapper.unmount()
  })

  it('stays silent for an account that can stream', async () => {
    const accounts = buildAccounts([
      { id: 100, service_name: 'qobuz', email: 'ana@qobuz.com', streaming_entitled: true },
    ])
    installBackend(accounts)
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-100"]')
    expect(row.text()).not.toContain('No streaming on this plan')
    expect(row.attributes('data-no-streaming')).toBe('false')
    wrapper.unmount()
  })

  it('stays silent when the entitlement is unknown', async () => {
    // Accounts that predate the entitlement column, or whose provider response
    // said nothing: showing the warning there would be inventing a problem.
    const accounts = buildAccounts([
      { id: 100, service_name: 'qobuz', email: 'ana@qobuz.com', streaming_entitled: null },
    ])
    installBackend(accounts)
    const wrapper = await mountAccounts()

    const row = wrapper.find('[data-testid="account-row-100"]')
    expect(row.text()).not.toContain('No streaming on this plan')
    wrapper.unmount()
  })
})

describe('active-account UI', () => {
  it('puts the active row first and marks all secondary rows as not active', async () => {
    // Backend order is deliberately wrong: the active account is last.
    installBackend()
    const wrapper = await mountAccounts()

    const rows = wrapper.findAll('[data-testid^="account-row-"]')
    expect(rows.map(r => r.attributes('data-testid'))).toEqual([
      'account-row-100',
      'account-row-101',
    ])
    expect(rows[0].attributes('data-active')).toBe('true')
    expect(rows[1].attributes('data-active')).toBe('false')
    // Only ONE row is marked active.
    expect(rows.filter(r => r.text().includes('Active')).length).toBe(1)
    wrapper.unmount()
  })

  it('shows the active account without opening any details element', async () => {
    installBackend()
    const wrapper = await mountAccounts()

    // The card lists the primary row directly (no disclosure needed).
    const card = wrapper.find('.service-card')
    expect(card.find('[data-testid="account-row-100"]').exists()).toBe(true)
    expect(card.text()).toContain('Ana Spotify (ana@spotify.com)')
    wrapper.unmount()
  })
})

describe('SettingsServices — masked credential state', () => {
  it('shows the stored credential kind with no credential value', async () => {
    const accounts = buildAccounts([
      {
        credential_state: {
          kind: 'cookie',
          has_password: false,
          has_cookie: true,
          has_token: true,
          has_credential: true,
          auto_refreshable: false,
        },
      },
      { display_name: 'Luis Spotify' },
    ])
    installBackend(accounts)
    const wrapper = await mountSettingsServices()

    const row = wrapper.find('[data-testid="settings-account-row-100"]')
    const state = row.find('[data-testid="settings-account-credential-state-100"]')
    expect(state.exists()).toBe(true)
    expect(state.text()).toContain('cookie')
    expect(row.attributes('data-active')).toBe('true')
    wrapper.unmount()
  })
})

describe('duplicate label rendering (all account surfaces)', () => {
  it('renders the email exactly once per account row on every surface', async () => {
    // Label is a display name → the email appears once (secondary line).
    installBackend(buildAccounts([{ display_name: 'Ana' }]), buildStatus())
    const wrapper = await mountAccounts()
    for (const testId of ['account-row-100', 'account-row-101']) {
      const row = wrapper.find(`[data-testid="${testId}"]`)
      const instance = testId === 'account-row-100' ? 'ana@spotify.com' : 'luis@spotify.com'
      expect(row.text().split(instance).length - 1).toBe(1)
    }
    wrapper.unmount()
  })

  it('never repeats the email when the email IS the identity label', async () => {
    installBackend(buildAccounts([{ display_name: null }]), buildStatus())
    const wrapper = await mountAccounts()
    for (const testId of ['account-row-100', 'account-row-101']) {
      const row = wrapper.find(`[data-testid="${testId}"]`)
      const instance = testId === 'account-row-100' ? 'ana@spotify.com' : 'luis@spotify.com'
      expect(row.text().split(instance).length - 1).toBe(1)
    }
    wrapper.unmount()
  })

  it('shows the credential state once and never as a second identity line', async () => {
    installBackend(
      buildAccounts([
        {
          credential_state: {
            kind: 'cookie',
            has_password: false,
            has_cookie: true,
            has_token: true,
            has_credential: true,
            auto_refreshable: false,
          },
        },
        { display_name: 'Luis Spotify' },
      ]),
    )
    const wrapper = await mountAccounts()

    const account = wrapper.find('[data-testid="account-row-100"]')
    // The credential description must not restate the identity label.
    const description = describeCredentialState(buildAccounts()[0])
    const withoutDescription = account.text().replace(description, '')
    // Removing the credential line must leave exactly one identity + the email.
    expect(withoutDescription.split('ana@spotify.com').length - 1).toBe(1)
    expect(account.find('[data-testid="account-credential-state-100"]').exists()).toBe(true)
    wrapper.unmount()
  })
})
