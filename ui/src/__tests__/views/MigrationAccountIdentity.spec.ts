/**
 * MigrationView — account dropdown labels use the shared identity resolver.
 *
 * The migration account pickers used to print `a.display_name || a.email ||
 * 'Cuenta N'`, which surfaces the raw backend string: an opaque provider id or a
 * `browser_session` placeholder for cookie-only imports. They now render exactly
 * what the Accounts and Settings screens render, so a user can tell which
 * account a migration reads from and writes to.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import MigrationView from '@/views/MigrationView.vue'
import { mockInvoke, resetMocks } from '../setup'

const stubs = { transition: true, teleport: true }

const statuses = [
  { name: 'spotify', connected: true, account_email: 'ana@spotify.com', library_count: 10, favorites_count: 8, playlists_count: 2, last_synced: null, credentials_invalid: false },
]

// Two accounts on spotify so the picker has to choose a label for both.
const accounts = [
  {
    id: 11,
    service_id: 1,
    service_name: 'spotify',
    display_name: 'Ana Spotify',
    email: 'ana@spotify.com',
    is_active: true,
    last_synced: null,
    created_at: null,
    credentials_invalid: false,
  },
  {
    id: 12,
    service_id: 1,
    service_name: 'spotify',
    // A cookie-only import: no email, and the backend stored the opaque marker
    // the bridge returned when it could not discover a real user.
    display_name: 'browser_session',
    email: null,
    is_active: false,
    last_synced: null,
    created_at: null,
    credentials_invalid: false,
    credential_state: {
      kind: 'cookie',
      has_password: false,
      has_cookie: true,
      has_token: true,
      has_credential: true,
      auto_refreshable: false,
    },
  },
]

function backend() {
  mockInvoke(cmd => {
    if (cmd === 'get_service_statuses') return statuses
    if (cmd === 'get_accounts') return accounts
    if (cmd === 'get_migration_destinations') return ['qobuz']
    if (cmd === 'get_migration_history') return []
    if (cmd === 'get_migration_templates') return []
    return []
  })
}

async function mountServiceMode() {
  const wrapper = mount(MigrationView, { global: { stubs } })
  await flushPromises()
  await wrapper.find('[data-testid="mode-service-to-service"]').trigger('click')
  await flushPromises()
  // Selecting the source service reveals the source-account picker when the
  // service has more than one account.
  const card = wrapper
    .findAll('button.service-card')
    .find(c => c.attributes('data-service-id') === 'spotify')
  await card!.trigger('click')
  await flushPromises()
  return wrapper
}

describe('MigrationView — account picker identity', () => {
  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
  })

  it('labels a cookie-only account with the shared placeholder, never a session marker', async () => {
    backend()
    const wrapper = await mountServiceMode()

    const picker = wrapper.find('[data-testid="svc-source-account"]')
    expect(picker.exists()).toBe(true)
    const optionTexts = picker.findAll('option').map(o => o.text())

    expect(optionTexts[0]).toContain('Ana Spotify (ana@spotify.com)')
    expect(optionTexts[0]).toContain('(activa)')
    // The opaque session marker is never shown as an identity.
    expect(optionTexts.join(' ')).not.toContain('browser_session')
    expect(optionTexts[1]).toContain('Identidad no proporcionada')
    // And no numeric fallback label either.
    expect(optionTexts.join(' ')).not.toContain('Cuenta 12')

    wrapper.unmount()
  })
})
