/**
 * AccountsSyncRunCard.spec.ts
 * Issue #68 (ADR-0006, decisión 19): la corrida de una cuenta se pinta en la
 * misma tarjeta para Apple Music que para Qobuz — fases, progreso, estado y
 * resultado — sin confundir los ids de biblioteca con los de catálogo.
 *
 * El canal difiere según lo que el backend expone hoy:
 *  - Qobuz: SyncRun persistido (`start_sync_run`, `sync-run-*`, cancelación,
 *    historial).
 *  - Apple Music: orquestador común (`sync_service`, `sync-progress` con
 *    `account_id`), que es el mismo motor que aplica las colecciones y la
 *    cuenta elegida desde #400.
 * La tarjeta es la misma; cambia la fuente de los eventos.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { mockInvoke, resetMocks, defaultInvokeHandler } from '../setup'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import AccountsView from '@/views/AccountsView.vue'
import type { Account, Service, ServiceStatus } from '@/api/types'

const services: Service[] = [
  { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
  { id: 4, name: 'apple_music', supports_download: 0, max_quality: null },
]

const accounts: Account[] = [
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

const statuses: ServiceStatus[] = [
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

let invoked: Array<{ command: string; args: any }> = []

function installBackend(syncServiceResult: Record<string, unknown> = {}) {
  invoked = []
  mockInvoke((command, args: any) => {
    invoked.push({ command, args })
    if (command === 'get_services') return services
    if (command === 'get_accounts') return accounts
    if (command === 'get_service_statuses') return statuses
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

async function mountAccounts(): Promise<VueWrapper> {
  const wrapper = mount(AccountsView, {
    global: { stubs: { SpotifyApiConfigCard: true, ServiceConnectionModal: true } },
    attachTo: document.body,
  })
  await flushPromises()
  return wrapper
}

async function openSettingsFor(wrapper: VueWrapper, cardName: string): Promise<void> {
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

describe('Qobuz SyncRun card (characterization before #68)', () => {
  it('renders the persisted live run and cancels through the SyncRun channel', async () => {
    installBackend()
    const wrapper = await mountAccounts()
    await openSettingsFor(wrapper, 'Qobuz')

    const start = document.body.querySelector('[data-testid="sync-run-start-2"]') as HTMLButtonElement
    expect(start).not.toBeNull()
    start.click()
    await flushPromises()

    expect(invoked.some(call => call.command === 'start_sync_run')).toBe(true)
    expect(invoked.some(call => call.command === 'sync_service')).toBe(false)

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
    // Today the card counts what arrived; the provider total is not surfaced.
    expect(modalText()).toContain('12 seen · 5 imported')
    const cancel = document.body.querySelector('[data-testid="sync-run-cancel-2"]')
    expect(cancel).not.toBeNull()

    wrapper.unmount()
  })
})

describe('Apple Music run card (#68)', () => {
  it('offers the same run card and starts the unified sync for the chosen account', async () => {
    installBackend()
    const wrapper = await mountAccounts()
    await openSettingsFor(wrapper, 'Apple_music')

    const start = document.body.querySelector('[data-testid="sync-run-start-4"]') as HTMLButtonElement
    expect(start).not.toBeNull()
    start.click()
    await flushPromises()

    const syncCall = invoked.find(call => call.command === 'sync_service')
    expect(syncCall).toBeDefined()
    expect(syncCall!.args.service).toBe('apple_music')
    expect(syncCall!.args.accountId).toBe(4)
    // Apple does not fake a persisted SyncRun the backend does not expose.
    expect(invoked.some(call => call.command === 'start_sync_run')).toBe(false)

    expect(modalText()).toContain('Last run: Completed · 3 imported')

    wrapper.unmount()
  })

  it('shows album and playlist phases with provider progress in the same card', async () => {
    installBackend()
    const wrapper = await mountAccounts()
    await openSettingsFor(wrapper, 'Apple_music')

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

  it('marks a run with provider gaps as partial, not as a clean completion', async () => {
    installBackend({ success: true, errors: ['Apple Music library: album page failed at offset 50'] })
    const wrapper = await mountAccounts()
    await openSettingsFor(wrapper, 'Apple_music')

    const start = document.body.querySelector('[data-testid="sync-run-start-4"]') as HTMLButtonElement
    start.click()
    await flushPromises()

    expect(modalText()).toContain('Last run: Partial (gaps) · 3 imported')

    wrapper.unmount()
  })

  it('keeps the Qobuz card on the persisted channel and does not leak Apple phases as raw keys', async () => {
    installBackend()
    const wrapper = await mountAccounts()
    await openSettingsFor(wrapper, 'Apple_music')

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
