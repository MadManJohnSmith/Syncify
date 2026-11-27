/**
 * AccountsSyncUnification.spec.ts
 * S126A: Unify UI of Accounts, Sync, and Import configuration
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AccountsView from '@/views/AccountsView.vue'
import LibraryView from '@/views/LibraryView.vue'
import SettingsSync from '@/views/settings/SettingsSync.vue'
import SettingsServices from '@/views/settings/SettingsServices.vue'
import { useAccountsStatus } from '@/composables/useAccountsStatus'
import { useSyncSettings } from '@/composables/useSyncSettings'
import { mockInvoke, resetMocks, defaultInvokeHandler } from '../setup'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import type { ServiceStatus, Service, Account, SyncSettings, ServiceSyncSettings, ServicePreference } from '@/api/types'

const mockPush = vi.fn()
vi.mock('vue-router', () => ({
  useRouter: () => ({
    push: mockPush,
  }),
  useRoute: () => ({
    query: {},
    params: {},
  }),
}))

describe('S126A Accounts & Sync Unification Suite', () => {
  let mockStatuses: ServiceStatus[] = []
  let mockServices: Service[] = []
  let mockAccounts: Account[] = []
  let mockGlobalSync: SyncSettings
  let mockServiceSyncs: ServiceSyncSettings[] = []
  let mockServicePrefs: ServicePreference[] = []

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()

    mockServices = [
      { id: 1, name: 'spotify', supports_download: 1, max_quality: '320kbps' },
      { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
      { id: 3, name: 'tidal', supports_download: 1, max_quality: 'lossless' },
    ]

    mockAccounts = [
      {
        id: 1,
        service_id: 1,
        service_name: 'spotify',
        display_name: 'Spotify User',
        email: 'user@spotify.com',
        is_active: true,
        last_synced: '2026-08-18T00:00:00Z',
        created_at: '2026-08-01T00:00:00Z',
        credentials_invalid: false,
      },
      {
        id: 2,
        service_id: 2,
        service_name: 'qobuz',
        display_name: 'Qobuz Expired User',
        email: 'user@qobuz.com',
        is_active: true,
        last_synced: '2026-08-17T00:00:00Z',
        created_at: '2026-08-01T00:00:00Z',
        credentials_invalid: true,
        last_auth_error: 'Token expired (401)',
      }
    ]

    mockStatuses = [
      {
        name: 'spotify',
        connected: true,
        account_email: 'user@spotify.com',
        library_count: 1540,
        favorites_count: 320,
        playlists_count: 12,
        last_synced: '2026-08-18T00:00:00Z',
        credentials_invalid: false,
      },
      {
        name: 'qobuz',
        connected: true,
        account_email: 'user@qobuz.com',
        library_count: 800,
        favorites_count: 150,
        playlists_count: 5,
        last_synced: '2026-08-17T00:00:00Z',
        credentials_invalid: true,
        last_auth_error: 'Token expired',
      },
      {
        name: 'tidal',
        connected: false,
        account_email: null,
        library_count: 0,
        favorites_count: 0,
        playlists_count: 0,
        last_synced: null,
        credentials_invalid: false,
      }
    ]

    mockGlobalSync = {
      id: 1,
      auto_sync_enabled: true,
      sync_interval_value: 6,
      sync_interval_unit: 'hours',
      sync_on_startup: true,
      background_download: true,
      max_concurrent_downloads: 3,
      rate_limit_delay_ms: 500,
      pause_on_metered: true,
      pause_on_low_battery: true,
    }

    mockServiceSyncs = [
      {
        id: 1,
        service_name: 'spotify',
        sync_favorites: true,
        sync_playlists: false,
        sync_albums: true,
        last_synced: null,
      },
      {
        id: 2,
        service_name: 'qobuz',
        sync_favorites: true,
        sync_playlists: true,
        sync_albums: true,
        last_synced: null,
      }
    ]

    mockServicePrefs = [
      { id: 1, service_name: 'qobuz', priority: 1, auto_import_enabled: true },
      { id: 2, service_name: 'spotify', priority: 2, auto_import_enabled: true },
      { id: 3, service_name: 'tidal', priority: 3, auto_import_enabled: false },
    ]

    let mockGranularPrefs: Record<string, any> = {
      spotify: {
        service_name: 'spotify',
        favorite_tracks: true,
        favorite_albums: false,
        favorite_artists: false,
        playlists: true,
        purchases: false,
        library_history: false,
        include_appearances: false,
      },
      qobuz: {
        service_name: 'qobuz',
        favorite_tracks: true,
        favorite_albums: true,
        favorite_artists: true,
        playlists: true,
        purchases: true,
        library_history: true,
        include_appearances: true,
      }
    }

    mockInvoke((command, args: any) => {
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'get_sync_settings') return mockGlobalSync
      if (command === 'get_service_sync_settings') return mockServiceSyncs
      if (command === 'get_service_preferences') return mockServicePrefs
      if (command === 'get_service_import_preferences') {
        return mockGranularPrefs[args.service] || {
          service_name: args.service,
          favorite_tracks: true,
          favorite_albums: false,
          favorite_artists: false,
          playlists: true,
          purchases: false,
          library_history: false,
          include_appearances: false,
        }
      }
      if (command === 'update_service_import_preferences') {
        mockGranularPrefs[args.preferences.service_name] = { ...args.preferences }
        return args.preferences
      }
      if (command === 'sync_service') {
        if (args.service === 'qobuz') {
          throw new Error('RequiresAuth: Qobuz user authentication required')
        }
        return {
          service: args.service,
          account_id: 1,
          success: true,
          message: 'Sync completed',
          imported_tracks_total: 10,
          favorite_tracks_total: 10,
          favorite_albums_total: 0,
          favorite_artists_total: 0,
          playlists_total: 0,
          purchases_total: 0,
          skipped_tracks_total: 0,
          errors: [],
        }
      }
      if (command === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 50, has_more: false }
      return null
    })
  })

  it('1. Accounts without valid token shows Reconnect, not Sync', async () => {
    const wrapper = mount(AccountsView)
    await flushPromises()

    // Qobuz has credentials_invalid: true
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.text().includes('Qobuz'))
    expect(qobuzCard).toBeDefined()
    expect(qobuzCard!.text()).toContain('Qobuz needs reauthentication')
    expect(qobuzCard!.text()).toContain('Reconnect')
    expect(qobuzCard!.find('button:contains("Sync")').exists()).toBe(false)
  })

  it('2. Eliminates "Import Settings" button and leaves single principal action: Sync', async () => {
    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    expect(spotifyCard).toBeDefined()

    // Main action is Sync
    const syncButton = spotifyCard!.findAll('button').find(b => b.text().includes('Sync'))
    expect(syncButton).toBeDefined()

    // "Import settings" button must NOT exist anywhere in the view
    expect(wrapper.text()).not.toContain('Import settings')
    expect(spotifyCard!.text()).not.toContain('Import settings')

    // No standalone "Favorites" button
    const actionButtons = spotifyCard!.findAll('.flex.items-center.gap-2 button')
    const hasStandaloneFavorites = actionButtons.some(b => b.text().trim() === 'Favorites')
    expect(hasStandaloneFavorites).toBe(false)
  })

  it('3. Gear icon opens service modal and loads/saves real import preferences from backend', async () => {
    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    expect(spotifyCard).toBeDefined()

    // Gear button exists on card
    const gearBtn = spotifyCard!.find('button.absolute.top-4.right-4')
    expect(gearBtn.exists()).toBe(true)

    await gearBtn.trigger('click')
    await flushPromises()

    // Modal is teleported to body and displays all required checkboxes
    const modalEl = document.body.querySelector('.service-settings-modal')
    expect(modalEl).not.toBeNull()
    expect(modalEl!.textContent).toContain('What will be imported:')
    expect(modalEl!.textContent).toContain('Favorite tracks')
    expect(modalEl!.textContent).toContain('Favorite albums')
    expect(modalEl!.textContent).toContain('Favorite artists')
    expect(modalEl!.textContent).toContain('Playlists')
    expect(modalEl!.textContent).toContain('Purchases')
    expect(modalEl!.textContent).toContain('History / Library tracks')
    expect(modalEl!.textContent).toContain('Appearances')

    // Save changes
    const saveBtn = Array.from(modalEl!.querySelectorAll('button')).find(b => b.textContent?.includes('Save Changes')) as HTMLButtonElement
    expect(saveBtn).toBeDefined()
    saveBtn.click()
    await flushPromises()
  })

  it('4. Sync invokes sync_service and never calls legacy import_qobuz_library from AccountsView', async () => {
    let invokedCommands: string[] = []
    mockInvoke((command, args: any) => {
      invokedCommands.push(command)
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'sync_service') {
        return {
          service: args.service,
          account_id: 1,
          success: true,
          message: 'Sync completed',
          imported_tracks_total: 5,
          favorite_tracks_total: 5,
          favorite_albums_total: 0,
          favorite_artists_total: 0,
          playlists_total: 0,
          purchases_total: 0,
          skipped_tracks_total: 0,
          errors: [],
        }
      }
      return null
    })

    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    const syncButton = spotifyCard!.findAll('button').find(b => b.text().includes('Sync'))
    await syncButton!.trigger('click')
    await flushPromises()

    expect(invokedCommands).toContain('sync_service')
    expect(invokedCommands).not.toContain('import_qobuz_library')
    expect(invokedCommands).not.toContain('import_spotify_library')
  })

  it('4b. R3: Apple Music and SoundCloud launch the same full sync as the rest (no partial-catalog veto)', async () => {
    mockServices.push(
      { id: 4, name: 'apple_music', supports_download: 0, max_quality: null },
      { id: 5, name: 'soundcloud', supports_download: 0, max_quality: null },
    )
    mockAccounts.push(
      { id: 4, service_id: 4, service_name: 'apple_music', display_name: 'Apple User', email: null, is_active: true, last_synced: null, created_at: '2026-08-01T00:00:00Z', credentials_invalid: false },
      { id: 5, service_id: 5, service_name: 'soundcloud', display_name: 'SoundCloud User', email: null, is_active: true, last_synced: null, created_at: '2026-08-01T00:00:00Z', credentials_invalid: false },
    )
    mockStatuses.push(
      { name: 'apple_music', connected: true, account_email: null, library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
      { name: 'soundcloud', connected: true, account_email: null, library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
    )

    const invokedSyncs: string[] = []
    mockInvoke((command, args: any) => {
      if (command === 'sync_service') {
        invokedSyncs.push(args.service)
      }
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'sync_service') {
        return {
          service: args.service,
          account_id: 1,
          success: true,
          message: 'Sync completed',
          imported_tracks_total: 3,
          favorite_tracks_total: 3,
          favorite_albums_total: 0,
          favorite_artists_total: 0,
          playlists_total: 1,
          purchases_total: 0,
          skipped_tracks_total: 0,
          errors: [],
        }
      }
      return null
    })

    const wrapper = mount(AccountsView)
    await flushPromises()

    // The partial-catalog copy is gone from every card, in any language variant
    expect(wrapper.text()).not.toContain('Full catalog sync is not available yet')
    expect(wrapper.text()).not.toContain('Unavailable')

    // Apple Music and SoundCloud show exactly the same enabled Sync button as Spotify
    const cardNames = ['Spotify', 'Apple_music', 'Soundcloud']
    for (const name of cardNames) {
      const card = wrapper.findAll('.service-card').find(c => c.text().includes(name))
      expect(card, `card for ${name}`).toBeDefined()
      const syncButton = card!.findAll('button').find(b => b.text().includes('Sync'))
      expect(syncButton, `sync button for ${name}`).toBeDefined()
      expect(syncButton!.attributes('disabled')).toBeUndefined()
    }

    // And clicking them launches the same unified sync_service flow as the rest
    const soundcloudCard = wrapper.findAll('.service-card').find(c => c.text().includes('Soundcloud'))
    const soundcloudSync = soundcloudCard!.findAll('button').find(b => b.text().includes('Sync'))
    await soundcloudSync!.trigger('click')
    await flushPromises()

    const appleCard = wrapper.findAll('.service-card').find(c => c.text().includes('Apple_music'))
    const appleSync = appleCard!.findAll('button').find(b => b.text().includes('Sync'))
    await appleSync!.trigger('click')
    await flushPromises()

    expect(invokedSyncs).toContain('soundcloud')
    expect(invokedSyncs).toContain('apple_music')
  })

  it('5. Connect Services button in empty LibraryView navigates to /accounts', async () => {
    const wrapper = mount(LibraryView)
    await flushPromises()

    expect(wrapper.text()).toContain('Your library is empty')
    const connectButton = wrapper.findAll('button').find(b => b.text().includes('Connect Services'))
    expect(connectButton).toBeDefined()

    await connectButton!.trigger('click')
    expect(mockPush).toHaveBeenCalledWith('/accounts')
  })

  it('6. Displays imported tracks and favorite tracks separately', async () => {
    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    expect(spotifyCard).toBeDefined()

    // Total library count = 1540, favorites count = 320
    expect(spotifyCard!.text()).toContain('1,540')
    expect(spotifyCard!.text()).toContain('Total Imported')
    expect(spotifyCard!.text()).toContain('320')
    expect(spotifyCard!.text()).toContain('Favorites')
  })

  it('7. If backend returns 401 / RequiresAuth on sync, displays actionable reauthentication message', async () => {
    mockInvoke((command) => {
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'sync_service') {
        throw new Error('RequiresAuth: Qobuz user authentication required (401)')
      }
      return null
    })

    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    const syncButton = spotifyCard!.findAll('button').find(b => b.text().includes('Sync'))
    await syncButton!.trigger('click')
    await flushPromises()

    // Verify actionable reauthentication message and reconnect action in DOM
    const updatedSpotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    expect(updatedSpotifyCard).toBeDefined()
    expect(updatedSpotifyCard!.text()).toContain('needs reauthentication')
    expect(updatedSpotifyCard!.text()).toContain('Reconnect Required')
    // LAST_AUTH_ERROR must NOT be echoed: the raw provider string can carry
    // credentials/cookies, so the UI shows a safe classification instead.
    expect(updatedSpotifyCard!.text()).not.toContain('RequiresAuth: Qobuz user authentication required (401)')
    expect(updatedSpotifyCard!.text()).not.toContain('RequiresAuth')
    const reconnectBtn = updatedSpotifyCard!.findAll('button').find(b => b.text().includes('Reconnect'))
    expect(reconnectBtn).toBeDefined()
  })

  it('7b. A resolved partial sync with "401" inside an item error does NOT ask for reauthentication', async () => {
    const { useGlobalTasks, resetGlobalTasks } = await import('@/composables/useGlobalTasks')
    resetGlobalTasks()
    const globalTasks = useGlobalTasks()
    globalTasks.initEventListeners()

    mockInvoke((command, args: any) => {
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'sync_service') {
        return {
          service: args.service,
          account_id: 1,
          success: false,
          message: 'Sync completed with 1 errors',
          imported_tracks_total: 0,
          favorite_tracks_total: 0,
          favorite_albums_total: 0,
          favorite_artists_total: 0,
          playlists_total: 0,
          purchases_total: 0,
          skipped_tracks_total: 0,
          errors: ['Tidal favorites error: item 4012345 could not be imported'],
        }
      }
      return null
    })

    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    const syncButton = spotifyCard!.findAll('button').find(b => b.text().includes('Sync'))
    await syncButton!.trigger('click')
    await flushPromises()

    const updatedSpotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    expect(updatedSpotifyCard!.text()).not.toContain('needs reauthentication')
    expect(updatedSpotifyCard!.text()).not.toContain('Reconnect Required')
    // The card remains syncable instead of switching to the invalid state.
    expect(updatedSpotifyCard!.findAll('button').some(b => b.text().includes('Sync'))).toBe(true)

    const task = globalTasks.tasks.value.get('sync-spotify')
    expect(task?.status).toBe('failed')
    expect(task?.requiresAuth).toBe(false)
  })

  it('7c. A sync with skipped items and imported data completes as a partial success without the connection error', async () => {
    const { useGlobalTasks, resetGlobalTasks } = await import('@/composables/useGlobalTasks')
    resetGlobalTasks()
    const globalTasks = useGlobalTasks()
    globalTasks.initEventListeners()

    mockInvoke((command, args: any) => {
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'sync_service') {
        return {
          service: args.service,
          account_id: 1,
          success: true,
          message:
            'Sync completed for Spotify with 3 skipped item(s) (1 expansion failures): 100 favorites, 2 albums, 5 playlists',
          imported_tracks_total: 42,
          favorite_tracks_total: 100,
          favorite_albums_total: 2,
          favorite_artists_total: 0,
          playlists_total: 5,
          purchases_total: 0,
          skipped_tracks_total: 0,
          tracks_changed_unique: 42,
          tracks_already_present: 10,
          errors: ['a', 'b', 'c'],
        }
      }
      return null
    })

    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    const syncButton = spotifyCard!.findAll('button').find(b => b.text().includes('Sync'))
    await syncButton!.trigger('click')
    await flushPromises()

    // The partial result is a completed sync with skipped items: it must not
    // take the "check the service connection" failure path nor prompt a
    // reconnect just because item errors exist.
    const task = globalTasks.tasks.value.get('sync-spotify')
    expect(task?.status).toBe('completed')
    expect(task?.requiresAuth).toBe(false)
    const updatedSpotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    expect(updatedSpotifyCard!.text()).not.toContain('needs reauthentication')
    expect(updatedSpotifyCard!.text()).not.toContain('Reconnect Required')
  })

  it('8. S128A: Sync button registers an active task in useGlobalTasks and updates on success', async () => {
    const { useGlobalTasks, resetGlobalTasks } = await import('@/composables/useGlobalTasks')
    resetGlobalTasks()
    const globalTasks = useGlobalTasks()
    globalTasks.initEventListeners()

    mockInvoke((command, args: any) => {
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'sync_service') {
        return {
          service: args.service,
          account_id: 1,
          success: true,
          message: 'Sync completed for Spotify: 25 tracks imported (10 favorites)',
          imported_tracks_total: 25,
          favorite_tracks_total: 10,
          favorite_albums_total: 2,
          favorite_artists_total: 3,
          playlists_total: 1,
          purchases_total: 0,
          skipped_tracks_total: 0,
          errors: [],
        }
      }
      return null
    })

    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    const syncButton = spotifyCard!.findAll('button').find(b => b.text().includes('Sync'))
    await syncButton!.trigger('click')
    await flushPromises()

    const task = globalTasks.tasks.value.get('sync-spotify')
    expect(task).toBeDefined()
    expect(task?.status).toBe('completed')
    expect(task?.importedCount).toBe(25)
    expect(task?.favoriteCount).toBe(10)
  })

  it('9. S128A: Sync receiving RequiresAuth marks service card as invalid, shows Reconnect button, and stops spinner', async () => {
    const { useGlobalTasks, resetGlobalTasks } = await import('@/composables/useGlobalTasks')
    resetGlobalTasks()
    const globalTasks = useGlobalTasks()
    globalTasks.initEventListeners()

    mockInvoke((command) => {
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'sync_service') {
        throw new Error('RequiresAuth: Spotify session expired (401)')
      }
      return null
    })

    const wrapper = mount(AccountsView)
    await flushPromises()

    const spotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    const syncButton = spotifyCard!.findAll('button').find(b => b.text().includes('Sync'))
    expect(syncButton).toBeDefined()

    await syncButton!.trigger('click')
    await flushPromises()

    // Card should now be in invalid status with Reconnect button
    const updatedSpotifyCard = wrapper.findAll('.service-card').find(c => c.text().includes('Spotify'))
    expect(updatedSpotifyCard!.text()).toContain('Reconnect Required')
    const reconnectBtn = updatedSpotifyCard!.findAll('button').find(b => b.text().includes('Reconnect'))
    expect(reconnectBtn).toBeDefined()
    // Spinner should NOT be spinning indefinitely
    expect(updatedSpotifyCard!.text()).not.toContain('Syncing...')

    const task = globalTasks.tasks.value.get('sync-spotify')
    expect(task).toBeDefined()
    expect(task?.status).toBe('requires_auth')
    expect(task?.requiresAuth).toBe(true)
  })
})

/**
 * Issue #68 (ADR-0006, decisión 19): la corrida de una cuenta se pinta en la
 * misma tarjeta para Apple Music que para Qobuz — fases, progreso, estado y
 * resultado — sin confundir los ids de biblioteca con los de catálogo.
 *
 * El canal difiere según lo que el backend expone hoy:
 *  - Qobuz: SyncRun persistido (`start_sync_run`, `sync-run-*`, cancelación,
 *    historial).
 *  - Apple Music: orquestador común (`sync_service`, `sync-progress` con
 *    `account_id`), el mismo motor que aplica colecciones y cuenta elegida
 *    desde #400.
 * La tarjeta es la misma; cambia la fuente de los eventos.
 */
describe('AccountsSyncRunCard: la corrida de Apple usa la misma tarjeta (#68)', () => {
  const runServices: Service[] = [
    { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
    { id: 4, name: 'apple_music', supports_download: 0, max_quality: null },
  ]

  const runAccounts: Account[] = [
    {
      id: 2,
      service_id: 2,
      service_name: 'qobuz',
      display_name: 'Qobuz User',
      email: 'qobuz@example.com',
      is_active: true,
      last_synced: null,
      created_at: '2026-08-01T00:00:00Z',
      credentials_invalid: false,
    },
    {
      id: 4,
      service_id: 4,
      service_name: 'apple_music',
      display_name: 'Apple User',
      email: null,
      is_active: true,
      last_synced: null,
      created_at: '2026-08-01T00:00:00Z',
      credentials_invalid: false,
    },
  ]

  const runStatuses: ServiceStatus[] = [
    {
      name: 'qobuz',
      connected: true,
      account_email: 'qobuz@example.com',
      library_count: 0,
      favorites_count: 0,
      playlists_count: 0,
      last_synced: null,
      credentials_invalid: false,
    },
    {
      name: 'apple_music',
      connected: true,
      account_email: null,
      library_count: 0,
      favorites_count: 0,
      playlists_count: 0,
      last_synced: null,
      credentials_invalid: false,
    },
  ]

  let runInvoked: Array<{ command: string; args: any }> = []

  function installRunBackend(syncServiceResult: Record<string, unknown> = {}) {
    runInvoked = []
    mockInvoke((command, args: any) => {
      runInvoked.push({ command, args })
      if (command === 'get_services') return runServices
      if (command === 'get_accounts') return runAccounts
      if (command === 'get_service_statuses') return runStatuses
      if (command === 'get_service_preferences') return []
      if (command === 'get_service_sync_settings') return []
      if (command === 'get_sync_settings') {
        return {
          id: 1,
          auto_sync_enabled: true,
          sync_interval_value: 24,
          sync_interval_unit: 'hours',
          sync_on_startup: false,
          background_download: false,
          max_concurrent_downloads: 3,
          rate_limit_delay_ms: 500,
          pause_on_metered: false,
          pause_on_low_battery: false,
        }
      }
      if (command === 'get_service_import_preferences') {
        return {
          service_name: args.service,
          favorite_tracks: true,
          favorite_albums: true,
          favorite_artists: false,
          playlists: true,
          purchases: false,
          library_history: false,
          include_appearances: false,
        }
      }
      if (command === 'list_sync_runs') return []
      if (command === 'start_sync_run') return { run_id: 70, status: 'running' }
      if (command === 'sync_service') {
        return {
          service: args.service,
          account_id: args.accountId,
          success: true,
          message: 'Sync completed',
          imported_tracks_total: 3,
          favorite_tracks_total: 3,
          favorite_albums_total: 0,
          favorite_artists_total: 0,
          playlists_total: 0,
          purchases_total: 0,
          skipped_tracks_total: 0,
          errors: [],
          ...syncServiceResult,
        }
      }
      if (command === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 50, has_more: false }
      return defaultInvokeHandler(command, args)
    })
  }

  async function mountRunAccounts() {
    const wrapper = mount(AccountsView, {
      global: { stubs: { SpotifyApiConfigCard: true, ServiceConnectionModal: true } },
      attachTo: document.body,
    })
    await flushPromises()
    return wrapper
  }

  async function openAccountTab(wrapper: ReturnType<typeof mount>, cardName: string) {
    const card = wrapper.findAll('.service-card').find(c => c.text().includes(cardName))
    expect(card, `card for ${cardName}`).toBeDefined()
    const gear = card!.find('button.absolute.top-4.right-4')
    expect(gear.exists()).toBe(true)
    await gear.trigger('click')
    await flushPromises()
    const accountTab = Array.from(document.body.querySelectorAll('.service-settings-modal .modal-tabs button'))
      .find(button => button.textContent?.trim() === 'Account') as HTMLButtonElement | undefined
    expect(accountTab, 'Account tab').toBeDefined()
    accountTab!.click()
    await flushPromises()
  }

  function modalText(): string {
    return document.body.querySelector('.service-settings-modal')?.textContent ?? ''
  }

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
    document.body.innerHTML = ''
  })

  it('characterizes the persisted Qobuz run: provider total, counters and cancel', async () => {
    installRunBackend()
    const wrapper = await mountRunAccounts()
    await openAccountTab(wrapper, 'Qobuz')

    const start = document.body.querySelector('[data-testid="sync-run-start-2"]') as HTMLButtonElement
    expect(start).not.toBeNull()
    start.click()
    await flushPromises()

    expect(runInvoked.some(call => call.command === 'start_sync_run')).toBe(true)
    expect(runInvoked.some(call => call.command === 'sync_service')).toBe(false)

    const eventBus = useEventBus()
    await eventBus.emit(TauriEvents.SYNC_RUN_STARTED, {
      run_id: 70,
      account_id: 2,
      phase: 'started',
      status: 'running',
      seen: 0,
      imported: 0,
    })
    await eventBus.emit(TauriEvents.SYNC_RUN_PROGRESS, {
      run_id: 70,
      account_id: 2,
      phase: 'importing_favorite_tracks',
      status: 'running',
      seen: 12,
      reported_total: 40,
      imported: 5,
      elapsed_ms: 2000,
    })
    await flushPromises()

    expect(modalText()).toContain('Importing favorites…')
    expect(modalText()).toContain('12 seen of 40 · 5 imported')
    expect(document.body.querySelector('[data-testid="sync-run-cancel-2"]')).not.toBeNull()

    wrapper.unmount()
  })

  it('starts the unified Apple run for the chosen account, never a fake SyncRun', async () => {
    installRunBackend()
    const wrapper = await mountRunAccounts()
    await openAccountTab(wrapper, 'Apple_music')

    const start = document.body.querySelector('[data-testid="sync-run-start-4"]') as HTMLButtonElement
    expect(start).not.toBeNull()
    start.click()
    await flushPromises()

    const syncCall = runInvoked.find(call => call.command === 'sync_service')
    expect(syncCall).toBeDefined()
    expect(syncCall!.args.service).toBe('apple_music')
    expect(syncCall!.args.accountId).toBe(4)
    expect(runInvoked.some(call => call.command === 'start_sync_run')).toBe(false)
    expect(modalText()).toContain('Last run: Completed · 3 imported')

    wrapper.unmount()
  })

  it('shows Apple album and playlist phases with provider progress in the same card', async () => {
    installRunBackend()
    const wrapper = await mountRunAccounts()
    await openAccountTab(wrapper, 'Apple_music')

    const eventBus = useEventBus()
    const base = { service: 'apple_music', account_id: 4, status: 'running', operation: 'sync' }

    await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
      ...base,
      phase: 'fetching_favorite_albums',
      current: 0,
      total: 12,
      message: 'Fetching Apple Music albums...',
      imported_tracks_total: 0,
      terminal: false,
    })
    await flushPromises()
    expect(modalText()).toContain('Fetching albums…')

    await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
      ...base,
      phase: 'importing_favorite_albums',
      current: 5,
      total: 12,
      message: 'Importing Apple Music albums (5/12)',
      imported_tracks_total: 41,
      terminal: false,
    })
    await flushPromises()
    expect(modalText()).toContain('Importing albums…')
    expect(modalText()).toContain('5 seen of 12 · 41 imported')

    await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
      ...base,
      phase: 'importing_playlists',
      current: 2,
      total: null,
      message: 'Importing Apple Music playlists (2)',
      imported_tracks_total: 60,
      terminal: false,
    })
    await flushPromises()
    expect(modalText()).toContain('Importing playlists…')
    // An absent provider total is unknown, never an invented denominator.
    expect(modalText()).toContain('2 seen · 60 imported')
    // The card does not offer a cancel the orchestrator cannot honour.
    expect(document.body.querySelector('[data-testid="sync-run-cancel-4"]')).toBeNull()

    await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
      ...base,
      phase: 'completed',
      status: 'completed',
      current: 60,
      total: null,
      message: 'Sync completed for Apple Music',
      imported_tracks_total: 60,
      terminal: true,
    })
    await flushPromises()
    expect(modalText()).toContain('Last run: Completed · 60 imported')

    wrapper.unmount()
  })

  it('marks an Apple run with provider gaps as partial, never a clean green', async () => {
    installRunBackend({ success: true, errors: ['Apple Music library: album page failed at offset 50'] })
    const wrapper = await mountRunAccounts()
    await openAccountTab(wrapper, 'Apple_music')

    const start = document.body.querySelector('[data-testid="sync-run-start-4"]') as HTMLButtonElement
    start.click()
    await flushPromises()

    expect(modalText()).toContain('Last run: Partial (gaps) · 3 imported')

    wrapper.unmount()
  })

  it('names Apple phases instead of leaking raw keys into the card', async () => {
    installRunBackend()
    const wrapper = await mountRunAccounts()
    await openAccountTab(wrapper, 'Apple_music')

    const eventBus = useEventBus()
    await eventBus.emit(TauriEvents.SYNC_PROGRESS, {
      service: 'apple_music',
      account_id: 4,
      status: 'running',
      operation: 'sync',
      phase: 'importing_playlists',
      current: 1,
      total: null,
      message: 'Importing Apple Music playlists (1)',
      imported_tracks_total: 2,
      terminal: false,
    })
    await flushPromises()

    expect(modalText()).not.toContain('importing_playlists')
    expect(modalText()).toContain('Importing playlists…')

    wrapper.unmount()
  })
})
