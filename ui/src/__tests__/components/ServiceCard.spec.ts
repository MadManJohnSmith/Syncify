/**
 * ServiceCard.spec.ts
 * TASK-25: Vincular Eventos y Reactividad en ServiceCard.vue y SettingsServices.vue
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import ServiceCard from '@/components/settings/ServiceCard.vue'
import SettingsServices from '@/views/settings/SettingsServices.vue'
import { mockInvoke, resetMocks } from '../setup'
import type { Service, Account, ServiceStatus, ServicePreference } from '@/api/types'

describe('ServiceCard.vue - Component Unit Tests', () => {
  it('renders connected state correctly with checkboxes and action buttons', () => {
    const wrapper = mount(ServiceCard, {
      props: {
        id: 'spotify',
        name: 'Spotify',
        icon: 'library_music',
        color: '#1ed760',
        isConnected: true,
        user: 'spotify_tester',
        status: 'Connected',
        statusType: 'success',
        enabled: true,
        autoImport: true,
      },
    })

    expect(wrapper.text()).toContain('Spotify')
    expect(wrapper.text()).toContain('Connected')
    expect(wrapper.text()).toContain('Enable for downloads')
    expect(wrapper.text()).toContain('Auto-import favorites')

    // Connected buttons
    const reauthBtn = wrapper.find('[data-testid="service-card-reauth"]')
    const disconnectBtn = wrapper.find('[data-testid="service-card-disconnect"]')
    const connectBtn = wrapper.find('[data-testid="service-card-connect"]')

    expect(reauthBtn.exists()).toBe(true)
    expect(disconnectBtn.exists()).toBe(true)
    expect(connectBtn.exists()).toBe(false)

    // Checkboxes checked state
    const enabledCheckbox = wrapper.find<HTMLInputElement>('[data-testid="service-card-enabled"]')
    const autoImportCheckbox = wrapper.find<HTMLInputElement>('[data-testid="service-card-auto-import"]')
    expect(enabledCheckbox.element.checked).toBe(true)
    expect(autoImportCheckbox.element.checked).toBe(true)
  })

  it('renders disconnected state correctly without checkboxes and only Connect button', () => {
    const wrapper = mount(ServiceCard, {
      props: {
        id: 'qobuz',
        name: 'Qobuz',
        icon: 'album',
        color: '#000000',
        isConnected: false,
      },
    })

    expect(wrapper.text()).toContain('Qobuz')
    expect(wrapper.text()).toContain('Not Connected')
    expect(wrapper.find('[data-testid="service-card-enabled"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="service-card-auto-import"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="service-card-reauth"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="service-card-disconnect"]').exists()).toBe(false)

    const connectBtn = wrapper.find('[data-testid="service-card-connect"]')
    expect(connectBtn.exists()).toBe(true)
  })

  it('emits update:enabled and toggleEnabled when "Enable for downloads" checkbox changes', async () => {
    const wrapper = mount(ServiceCard, {
      props: {
        id: 'spotify',
        serviceId: 'spotify',
        name: 'Spotify',
        icon: 'library_music',
        color: '#1ed760',
        isConnected: true,
        enabled: true,
      },
    })

    const enabledCheckbox = wrapper.find<HTMLInputElement>('[data-testid="service-card-enabled"]')
    await enabledCheckbox.setValue(false)

    expect(wrapper.emitted('update:enabled')).toBeTruthy()
    expect(wrapper.emitted('update:enabled')![0]).toEqual([false])

    expect(wrapper.emitted('toggleEnabled')).toBeTruthy()
    expect(wrapper.emitted('toggleEnabled')![0]).toEqual(['spotify', false])
  })

  it('emits update:autoImport and toggleAutoImport when "Auto-import favorites" checkbox changes', async () => {
    const wrapper = mount(ServiceCard, {
      props: {
        id: 'tidal',
        serviceId: 'tidal',
        name: 'Tidal',
        icon: 'T',
        color: '#ffffff',
        isConnected: true,
        autoImport: true,
      },
    })

    const autoImportCheckbox = wrapper.find<HTMLInputElement>('[data-testid="service-card-auto-import"]')
    await autoImportCheckbox.setValue(false)

    expect(wrapper.emitted('update:autoImport')).toBeTruthy()
    expect(wrapper.emitted('update:autoImport')![0]).toEqual([false])

    expect(wrapper.emitted('toggleAutoImport')).toBeTruthy()
    expect(wrapper.emitted('toggleAutoImport')![0]).toEqual(['tidal', false])
  })

  it('emits connect with serviceId when Connect button is clicked', async () => {
    const wrapper = mount(ServiceCard, {
      props: {
        serviceId: 'deezer',
        name: 'Deezer',
        icon: 'graphic_eq',
        color: '#a238ff',
        isConnected: false,
      },
    })

    await wrapper.find('[data-testid="service-card-connect"]').trigger('click')

    expect(wrapper.emitted('connect')).toBeTruthy()
    expect(wrapper.emitted('connect')![0]).toEqual(['deezer'])
  })

  it('emits reauth with serviceId when Re-authenticate button is clicked', async () => {
    const wrapper = mount(ServiceCard, {
      props: {
        id: 'tidal',
        name: 'Tidal',
        icon: 'T',
        color: '#ffffff',
        isConnected: true,
      },
    })

    await wrapper.find('[data-testid="service-card-reauth"]').trigger('click')

    expect(wrapper.emitted('reauth')).toBeTruthy()
    expect(wrapper.emitted('reauth')![0]).toEqual(['tidal'])
  })

  it('emits disconnect with serviceId when Disconnect button is clicked', async () => {
    const wrapper = mount(ServiceCard, {
      props: {
        name: 'Apple Music',
        serviceId: 'apple_music',
        icon: 'music_note',
        color: '#fa243c',
        isConnected: true,
      },
    })

    await wrapper.find('[data-testid="service-card-disconnect"]').trigger('click')

    expect(wrapper.emitted('disconnect')).toBeTruthy()
    expect(wrapper.emitted('disconnect')![0]).toEqual(['apple_music'])
  })

  it('resolves ID hierarchy: serviceId > id > lowercase name', async () => {
    // 1. With serviceId
    const w1 = mount(ServiceCard, {
      props: {
        serviceId: 'custom-svc',
        id: 42,
        name: 'Custom Service',
        icon: 'cloud',
        color: '#ff0000',
        isConnected: false,
      },
    })
    await w1.find('[data-testid="service-card-connect"]').trigger('click')
    expect(w1.emitted('connect')![0]).toEqual(['custom-svc'])

    // 2. With numeric id, no serviceId
    const w2 = mount(ServiceCard, {
      props: {
        id: 42,
        name: 'Custom Service',
        icon: 'cloud',
        color: '#ff0000',
        isConnected: false,
      },
    })
    await w2.find('[data-testid="service-card-connect"]').trigger('click')
    expect(w2.emitted('connect')![0]).toEqual(['42'])

    // 3. Fallback to name
    const w3 = mount(ServiceCard, {
      props: {
        name: 'SoundCloud',
        icon: 'cloud',
        color: '#ff5500',
        isConnected: false,
      },
    })
    await w3.find('[data-testid="service-card-connect"]').trigger('click')
    expect(w3.emitted('connect')![0]).toEqual(['soundcloud'])
  })
})

describe('SettingsServices.vue - Integration with ServiceCard Events', () => {
  let mockServices: Service[]
  let mockAccounts: Account[]
  let mockStatuses: ServiceStatus[]
  let mockPreferences: ServicePreference[]
  let invokedCommands: Array<{ command: string; args?: Record<string, unknown> }>

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
    invokedCommands = []

    mockServices = [
      { id: 1, name: 'spotify', supports_download: 1, max_quality: '320kbps' },
      { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
      { id: 3, name: 'tidal', supports_download: 1, max_quality: 'lossless' },
    ]

    mockAccounts = [
      {
        id: 101,
        service_id: 1,
        service_name: 'spotify',
        display_name: 'Spotify Tester',
        email: 'tester@spotify.com',
        is_active: true,
        last_synced: '2026-08-18T00:00:00Z',
        created_at: '2026-08-01T00:00:00Z',
        credentials_invalid: false,
      },
    ]

    mockStatuses = [
      {
        name: 'spotify',
        connected: true,
        account_email: 'tester@spotify.com',
        library_count: 50,
        favorites_count: 20,
        playlists_count: 5,
        last_synced: '2026-08-18T00:00:00Z',
        credentials_invalid: false,
      },
      {
        name: 'qobuz',
        connected: false,
        account_email: null,
        library_count: 0,
        favorites_count: 0,
        playlists_count: 0,
        last_synced: null,
        credentials_invalid: false,
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
      },
    ]

    mockPreferences = [
      { id: 1, service_name: 'spotify', priority: 1, auto_import_enabled: true },
      { id: 2, service_name: 'qobuz', priority: 2, auto_import_enabled: false },
      { id: 3, service_name: 'tidal', priority: 3, auto_import_enabled: true },
    ]

    mockInvoke((command, args) => {
      invokedCommands.push({ command, args })
      if (command === 'get_services') return mockServices
      if (command === 'get_accounts') return mockAccounts
      if (command === 'get_service_statuses') return mockStatuses
      if (command === 'get_service_preferences') return mockPreferences
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
      if (command === 'get_service_sync_settings') return []
      if (command === 'start_auth_and_save') {
        return { success: true, data: { display_name: 'New User' }, error: null }
      }
      if (command === 'logout_service') {
        return { success: true, data: null, error: null }
      }
      if (command === 'remove_account') return null
      if (command === 'toggle_account_active') return null
      if (command === 'update_service_preference') {
        const sName = (args as any)?.serviceName || (args as any)?.service
        const pref = mockPreferences.find(p => p.service_name === sName)
        return {
          id: pref?.id || 99,
          service_name: sName,
          priority: pref?.priority || 1,
          auto_import_enabled: (args as any)?.autoImportEnabled,
        }
      }
      return null
    })
  })

  it('handles connect event from ServiceCard and invokes start_auth_and_save', async () => {
    const wrapper = mount(SettingsServices)
    await flushPromises()

    const cards = wrapper.findAllComponents(ServiceCard)
    expect(cards.length).toBe(3)

    // Qobuz is not connected (index 1)
    const qobuzCard = cards[1]
    expect(qobuzCard.props('name')).toBe('Qobuz')
    expect(qobuzCard.props('isConnected')).toBe(false)

    // Click connect on Qobuz card
    await qobuzCard.find('[data-testid="service-card-connect"]').trigger('click')
    await flushPromises()

    const authCall = invokedCommands.find(c => c.command === 'start_auth_and_save')
    expect(authCall).toBeDefined()
    expect(authCall?.args).toEqual({ service: 'qobuz' })
  })

  it('handles disconnect event from ServiceCard and invokes logout_service and remove_account', async () => {
    const wrapper = mount(SettingsServices)
    await flushPromises()

    const cards = wrapper.findAllComponents(ServiceCard)
    // Spotify is connected (index 0)
    const spotifyCard = cards[0]
    expect(spotifyCard.props('name')).toBe('Spotify')
    expect(spotifyCard.props('isConnected')).toBe(true)

    // Click disconnect
    await spotifyCard.find('[data-testid="service-card-disconnect"]').trigger('click')
    await flushPromises()

    const logoutCall = invokedCommands.find(c => c.command === 'logout_service')
    expect(logoutCall).toBeDefined()
    expect(logoutCall?.args).toEqual({ service: 'spotify' })

    const removeCall = invokedCommands.find(c => c.command === 'remove_account')
    expect(removeCall).toBeDefined()
    expect(removeCall?.args).toEqual({ accountId: 101 })
  })

  it('handles reauth event from ServiceCard and triggers start_auth_and_save', async () => {
    const wrapper = mount(SettingsServices)
    await flushPromises()

    const cards = wrapper.findAllComponents(ServiceCard)
    const spotifyCard = cards[0]

    // Click reauth
    await spotifyCard.find('[data-testid="service-card-reauth"]').trigger('click')
    await flushPromises()

    const reauthCall = invokedCommands.find(c => c.command === 'start_auth_and_save')
    expect(reauthCall).toBeDefined()
    expect(reauthCall?.args).toEqual({ service: 'spotify' })
  })

  it('handles toggleEnabled event from ServiceCard and invokes toggle_account_active', async () => {
    const wrapper = mount(SettingsServices)
    await flushPromises()

    const cards = wrapper.findAllComponents(ServiceCard)
    const spotifyCard = cards[0]

    const enabledCheckbox = spotifyCard.find<HTMLInputElement>('[data-testid="service-card-enabled"]')
    await enabledCheckbox.setValue(false)
    await flushPromises()

    const toggleActiveCall = invokedCommands.find(c => c.command === 'toggle_account_active')
    expect(toggleActiveCall).toBeDefined()
    expect(toggleActiveCall?.args).toEqual({ accountId: 101, isActive: false })
  })

  it('handles toggleAutoImport event from ServiceCard and invokes update_service_preference', async () => {
    const wrapper = mount(SettingsServices)
    await flushPromises()

    const cards = wrapper.findAllComponents(ServiceCard)
    const spotifyCard = cards[0]

    const autoImportCheckbox = spotifyCard.find<HTMLInputElement>('[data-testid="service-card-auto-import"]')
    await autoImportCheckbox.setValue(false)
    await flushPromises()

    const updatePrefCall = invokedCommands.find(c => c.command === 'update_service_preference')
    expect(updatePrefCall).toBeDefined()
    expect(updatePrefCall?.args).toEqual({
      serviceName: 'spotify',
      autoImportEnabled: false,
    })
  })
})
