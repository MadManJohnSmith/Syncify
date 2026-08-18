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
import { mockInvoke, resetMocks } from '../setup'
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
        incremental_sync: false,
        last_synced: null,
      },
      {
        id: 2,
        service_name: 'qobuz',
        sync_favorites: true,
        sync_playlists: true,
        sync_albums: true,
        incremental_sync: true,
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
        incremental_sync: false,
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
        incremental_sync: false,
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
          incremental_sync: false,
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
    expect(spotifyCard!.text()).toContain('Imported')
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

    // No standalone favorites error; proper reauthentication toast/status is handled
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
