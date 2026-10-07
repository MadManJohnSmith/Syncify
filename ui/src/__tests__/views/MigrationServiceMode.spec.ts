/**
 * MigrationServiceMode.spec.ts
 * Post-audit 4.2 regression suite: the migration assistant's service →
 * service mode. Pins that the mode drives the real engine end to end with a
 * selection independent of the transfer wizard, a data-driven destination
 * list, a real preview, reviewable matches with manual match, and real
 * progress through the migration-progress channel. It must fail if any of
 * that ever regresses to fixed data or shared wizard state.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import MigrationView from '@/views/MigrationView.vue'
import { mockInvoke, resetMocks, emitMockEvent } from '../setup'
import type { MigrationJob } from '@/api/types'

const stubs = { transition: true, teleport: true }

type Handler = (cmd: string, args?: Record<string, unknown>) => unknown

/** Backend statuses: spotify/qobuz/tidal connected, deezer not connected. */
const connectedStatuses = [
  { name: 'spotify', connected: true, account_email: 'user@example.com', library_count: 10, favorites_count: 8, playlists_count: 2, last_synced: null, credentials_invalid: false },
  { name: 'qobuz', connected: true, account_email: 'user@example.com', library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
  { name: 'tidal', connected: true, account_email: 'user@example.com', library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
  { name: 'deezer', connected: false, account_email: null, library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
]

/** The engine supports one destination the user has NOT connected (soundcloud),
 *  and one connected service is NOT a supported destination (deezer). */
const engineDestinations = ['qobuz', 'tidal', 'soundcloud', 'spotify']

const reviewJob: MigrationJob = {
  id: 'svc-job-1',
  source_service: 'spotify',
  destination_service: 'qobuz',
    source_account_id: 11,
    destination_account_id: 20,
  source_playlist_ids: null,
  options: '{}',
  status: 'completed',
  total_items: 1,
  completed_items: 0,
  failed_items: 0,
  skipped_items: 1,
  started_at: '2026-03-30T10:00:00Z',
  completed_at: '2026-03-30T10:05:00Z',
  error_message: null,
  created_at: '2026-03-30T10:00:00Z',
}

const reviewItem = {
  id: 7,
  job_id: 'svc-job-1',
  source_track_id: 'src-1',
  source_track_title: 'Unknown Track',
  source_track_artist: 'Unknown Artist',
  source_track_album: null,
  source_playlist_id: null,
  source_playlist_name: null,
  destination_track_id: null,
  match_confidence: null,
  match_method: null,
  status: 'skipped',
  error_message: null,
  processed_at: null,
  created_at: '2026-03-30T10:00:00Z',
}

function backend(
  overrides: Record<string, unknown>,
  calls?: Array<{ cmd: string; args?: Record<string, unknown> }>,
  destinationList: string[] = engineDestinations,
): void {
  mockInvoke((cmd, args) => {
    calls?.push({ cmd, args })
    if (cmd in overrides) return overrides[cmd]
    if (cmd === 'get_service_statuses') return connectedStatuses
    if (cmd === 'get_migration_destinations') return destinationList
    if (cmd === 'get_migration_history') return []
    if (cmd === 'get_migration_templates') return []
    return []
  })
}

async function clickButtonByText(wrapper: VueWrapper, text: string): Promise<void> {
  const button = wrapper.findAll('button').find(b => b.text().trim() === text)
  expect(button, `button "${text}" should exist`).toBeTruthy()
  await button!.trigger('click')
  await flushPromises()
}

async function clickNext(wrapper: VueWrapper): Promise<void> {
  const button = wrapper.findAll('button').find(b => b.text().includes('Next'))
  expect(button, 'button "Next" should exist').toBeTruthy()
  await button!.trigger('click')
  await flushPromises()
}

async function enterServiceMode(wrapper: VueWrapper): Promise<void> {
  await wrapper.find('[data-testid="mode-service-to-service"]').trigger('click')
  await flushPromises()
}

/** Source = spotify card, then Next. */
async function selectServiceSource(wrapper: VueWrapper, serviceId: string): Promise<void> {
  const card = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === serviceId)
  expect(card, `source card "${serviceId}" should exist`).toBeTruthy()
  await card!.trigger('click')
  await flushPromises()
  await clickNext(wrapper)
}

describe('MigrationView service → service mode (post-audit 4.2)', () => {
  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
  })

  it('selects active and single accounts explicitly for migration', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    const accounts = [
      { id: 11, service_id: 1, service_name: 'spotify', display_name: 'Old', email: 'old@x', is_active: false },
      { id: 12, service_id: 1, service_name: 'spotify', display_name: 'Active', email: 'new@x', is_active: true },
      { id: 20, service_id: 2, service_name: 'qobuz', display_name: 'Only', email: 'only@x', is_active: true },
    ]
    backend({ get_accounts: accounts, preview_migration: { total_tracks: 0, matched_tracks: 0, unmatched_tracks: 0, playlists: [] } }, calls)
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    expect((wrapper.find('[data-testid="svc-source-account"]').element as HTMLSelectElement).value).toBe('12')
    await clickNext(wrapper)
    expect(wrapper.find('[data-testid="svc-destination-account"]').exists()).toBe(false)
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await clickNext(wrapper)
    expect(calls.find(c => c.cmd === 'preview_migration')?.args).toMatchObject({ sourceAccountId: 12, destinationAccountId: 20 })
  })

  it('keeps a single-account source scoped to its own library', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend({
      get_accounts: [
        { id: 1, service_id: 1, service_name: 'spotify', display_name: 'Only source', is_active: true },
        { id: 2, service_id: 2, service_name: 'qobuz', display_name: 'Only destination', is_active: true },
      ],
      preview_migration: { total_tracks: 0, matched_tracks: 0, unmatched_tracks: 0, playlists: [] },
    }, calls)
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    expect(wrapper.find('[data-testid="svc-source-account"]').exists()).toBe(false)
    await clickNext(wrapper)
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await clickNext(wrapper)
    expect(calls.find(c => c.cmd === 'preview_migration')?.args).toMatchObject({ sourceAccountId: 1, destinationAccountId: 2 })
    wrapper.unmount()
  })

  it('allows same-service transfer only between distinct selected accounts', async () => {
    const accounts = [
      { id: 11, service_id: 1, service_name: 'spotify', display_name: 'A', is_active: true },
      { id: 12, service_id: 1, service_name: 'spotify', display_name: 'B', is_active: false },
    ]
    backend({ get_accounts: accounts }, undefined, ['spotify'])
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper)
    const destination = wrapper.find('[data-service-id="spotify"]')
    expect(destination.attributes('disabled')).toBeUndefined()
    await destination.trigger('click')
    const selector = wrapper.find('[data-testid="svc-destination-account"]')
    expect((selector.element as HTMLSelectElement).value).toBe('12')
    await selector.setValue('11')
    expect(wrapper.findAll('button').find(b => b.text().includes('Next'))?.attributes('disabled')).toBeDefined()
  })

  it('requires explicit source and destination for same-service preview, even with multiple accounts', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    const accounts = [
      { id: 11, service_id: 1, service_name: 'spotify', display_name: 'A', is_active: true },
      { id: 12, service_id: 1, service_name: 'spotify', display_name: 'B', is_active: false },
    ]
    backend({ get_accounts: accounts, preview_migration: { total_tracks: 0, matched_tracks: 0, unmatched_tracks: 0, playlists: [] } }, calls, ['spotify'])
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    const source = wrapper.find('[data-testid="svc-source-account"]')
    // A missing source must not silently mean the active account: it would read
    // the entire legacy service mirror, including playlists.
    await source.setValue('')
    expect(wrapper.findAll('button').find(b => b.text().includes('Next'))?.attributes('disabled')).toBeDefined()
    await source.setValue('11')
    await clickNext(wrapper)
    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper)
    expect(calls.find(c => c.cmd === 'preview_migration')?.args).toMatchObject({ sourceAccountId: 11, destinationAccountId: 12 })
    wrapper.unmount()
  })

  it('exposes the service → service mode next to the transfer wizard and keeps their selections independent', async () => {
    backend({})
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()

    // Transfer wizard is the default mode; the service mode is hidden.
    expect(wrapper.find('[data-testid="mode-service-to-service"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="svc-source-step"]').exists()).toBe(false)

    await enterServiceMode(wrapper)
    expect(wrapper.find('[data-testid="svc-source-step"]').exists()).toBe(true)

    // Selecting a source in the service mode must NOT touch the transfer wizard.
    await selectServiceSource(wrapper, 'spotify')
    await wrapper.find('[data-testid="mode-transfer"]').trigger('click')
    await flushPromises()

    // Back in the transfer wizard the source step has no selected card.
    const selectedCards = wrapper.findAll('.service-card .top-3.right-3')
    expect(selectedCards.length).toBe(0)
    // ...and the transfer wizard's Next stays disabled (nothing selected there).
    const next = wrapper.findAll('button').find(b => b.text().includes('Next'))
    expect(next!.attributes('disabled')).toBeDefined()
  })

  it('offers only the engine-supported destinations that are actually connected (data-driven list)', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend({}, calls)
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')

    const step = wrapper.find('[data-testid="svc-destination-step"]')
    expect(step.exists()).toBe(true)
    // deezer: connected but not engine-supported → absent.
    // soundcloud: engine-supported but not connected → absent.
    // spotify: supported but it is the source → disabled card.
    const ids = step.findAll('.service-card').map(c => c.attributes('data-service-id'))
    expect(ids).toEqual(['spotify', 'qobuz', 'tidal'])

    const tidalCard = step.findAll('.service-card').find(c => c.attributes('data-service-id') === 'tidal')!
    await tidalCard.trigger('click')
    await flushPromises()

    // The mode drives the engine with its own selection.
    await clickNext(wrapper)
    const previewCall = calls.find(c => c.cmd === 'preview_migration')
    expect(previewCall).toBeTruthy()
    expect(previewCall!.args?.sourceService).toBe('spotify')
    expect(previewCall!.args?.destinationService).toBe('tidal')
  })

  it('shows the honest empty state when the engine supports no connected destination', async () => {
    backend({}, undefined, ['soundcloud'])
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')

    const step = wrapper.find('[data-testid="svc-destination-step"]')
    expect(step.text()).toContain('No supported destination services available')
    const next = wrapper.findAll('button').find(b => b.text().includes('Next'))
    expect(next!.attributes('disabled')).toBeDefined()
  })

  it('previews with real match counts from preview_migration', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend(
      { preview_migration: { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] } },
      calls,
    )
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)

    expect(wrapper.find('[data-testid="svc-review-step"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="svc-preview-total"]').text()).toBe('100')
    expect(wrapper.find('[data-testid="svc-preview-matched"]').text()).toContain('90')
    expect(wrapper.find('[data-testid="svc-preview-matched"]').text()).toContain('(90%)')
    expect(wrapper.find('[data-testid="svc-preview-unmatched"]').text()).toContain('10')
    // The skip toggle carries the real unmatched count.
    expect(wrapper.text()).toContain('Skip all tracks with no match found (10 tracks)')
    expect(wrapper.text()).not.toContain('1,234')
  })

  it('surfaces a preview error with retry instead of fabricated counts when the backend refuses', async () => {
    // The backend rejects preview_migration when no destination account is
    // available to match against (real preview_migration behavior).
    mockInvoke((cmd) => {
      if (cmd === 'preview_migration') throw new Error('No connected qobuz account is available to preview matches')
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_migration_destinations') return engineDestinations
      if (cmd === 'get_migration_history') return []
      if (cmd === 'get_migration_templates') return []
      return []
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)

    expect(wrapper.find('[data-testid="svc-preview-error"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('Could not load the match preview')
    expect(wrapper.text()).not.toContain('(85%)')
  })

  it('lists reviewable matches and wires manual match to the engine', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend(
      {
        get_accounts: [
          { id: 11, service_id: 1, service_name: 'spotify', display_name: 'Source', is_active: true },
          { id: 20, service_id: 2, service_name: 'qobuz', display_name: 'Destination', is_active: true },
        ],
        get_migration_history: [reviewJob],
        get_migration_items_by_status: [reviewItem],
        preview_migration: { total_tracks: 1, matched_tracks: 0, unmatched_tracks: 1, playlists: [] },
        search_destination_track: [
          { track_id: 'dest-1', title: 'Unknown Track (Deluxe)', artist: 'Unknown Artist', album: 'Deluxe Edition', duration_ms: 200000, quality: 'FLAC', confidence: 0.9 },
        ],
        manual_match_item: 'Item matched',
      },
      calls,
    )
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)

    // Real review row from the last migration on this exact route.
    const row = wrapper.find('[data-testid="svc-review-item-row"]')
    expect(row.exists()).toBe(true)
    expect(row.text()).toContain('Unknown Track')

    await clickButtonByText(wrapper, 'Search Manually')
    const modal = wrapper.find('[data-testid="manual-match-modal"]')
    expect(modal.exists()).toBe(true)

    await wrapper.find('[data-testid="manual-match-search-input"]').setValue('Unknown Track Unknown Artist')
    await wrapper.find('[data-testid="manual-match-search-btn"]').trigger('click')
    await flushPromises()

    const searchCall = calls.find(c => c.cmd === 'search_destination_track')
    expect(searchCall).toBeTruthy()
    expect(searchCall!.args?.service).toBe('qobuz')
    expect(searchCall!.args?.query).toBe('Unknown Track Unknown Artist')

    const result = wrapper.find('[data-testid="manual-match-result"]')
    expect(result.exists()).toBe(true)

    await wrapper.find('[data-testid="manual-match-apply"]').trigger('click')
    await flushPromises()

    const matchCall = calls.find(c => c.cmd === 'manual_match_item')
    expect(matchCall).toBeTruthy()
    expect(matchCall!.args?.itemId).toBe(7)
    expect(matchCall!.args?.destinationTrackId).toBe('dest-1')

    // The review list reloads so the new match shows up.
    const itemCalls = calls.filter(c => c.cmd === 'get_migration_items_by_status')
    expect(itemCalls.length).toBeGreaterThanOrEqual(2)
    expect(wrapper.find('[data-testid="manual-match-modal"]').exists()).toBe(false)
  })

  it('allows a valid inactive account when the service active status is invalid', async () => {
    const statuses = connectedStatuses.map(s => s.name === 'spotify'
      ? { ...s, connected: false, credentials_invalid: true } : s)
    const accounts = [
      { id: 11, service_id: 1, service_name: 'spotify', display_name: 'Expired', is_active: true, credentials_invalid: true },
      { id: 12, service_id: 1, service_name: 'spotify', display_name: 'Valid', is_active: false, credentials_invalid: false },
      { id: 20, service_id: 2, service_name: 'qobuz', display_name: 'Only', is_active: true },
    ]
    backend({ get_accounts: accounts, get_service_statuses: statuses,
      preview_migration: { total_tracks: 0, matched_tracks: 0, unmatched_tracks: 0, playlists: [] } })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    const source = wrapper.find('[data-service-id="spotify"]')
    expect(source.attributes('disabled')).toBeUndefined()
    await source.trigger('click')
    expect((wrapper.find('[data-testid="svc-source-account"]').element as HTMLSelectElement).value).toBe('12')
    wrapper.unmount()
  })

  it('does not surface a historical job from another account in review', async () => {
    const accounts = [
      { id: 11, service_id: 1, service_name: 'spotify', display_name: 'Source A', is_active: true },
      { id: 12, service_id: 1, service_name: 'spotify', display_name: 'Source B', is_active: false },
      { id: 20, service_id: 2, service_name: 'qobuz', display_name: 'Destination', is_active: true },
    ]
    backend({ get_accounts: accounts, get_migration_history: [reviewJob], get_migration_items_by_status: [reviewItem],
      preview_migration: { total_tracks: 0, matched_tracks: 0, unmatched_tracks: 0, playlists: [] } })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await wrapper.find('[data-testid="svc-source-account"]').setValue('12')
    await clickNext(wrapper)
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await clickNext(wrapper)
    expect(wrapper.find('[data-testid="svc-review-item-row"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('starts one real migration with the mode selection and renders progress from migration-progress events', async () => {
    let resolveStart!: (value: unknown) => void
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    mockInvoke((cmd, args) => {
      calls.push({ cmd, args })
      if (cmd === 'start_migration') return new Promise((resolve) => { resolveStart = resolve })
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_migration_destinations') return engineDestinations
      if (cmd === 'preview_migration') return { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] }
      return []
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper) // review (preview)
    // Skip unmatched off→on passes the real selection through.
    await wrapper.find('[data-testid="svc-skip-not-found-toggle"]').trigger('click')
    await clickNext(wrapper) // migrate

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    const startCall = calls.find(c => c.cmd === 'start_migration')
    expect(startCall).toBeTruthy()
    expect(startCall!.args?.sourceService).toBe('spotify')
    expect(startCall!.args?.destinationService).toBe('qobuz')
    const options = startCall!.args?.options as Record<string, unknown>
    expect(options.skip_unmatched).toBe(true)
    expect(options.create_playlists).toBe(false)
    expect(wrapper.find('[data-testid="transfer-progress"]').exists()).toBe(true)

    emitMockEvent('migration-progress', {
      job_id: 'job-42',
      current_item: 37,
      total_items: 100,
      current_track: 'Real Song',
      status: 'running',
      completed_count: 34,
      failed_count: 2,
      skipped_count: 1,
      percent: 37,
      speed: 12.5,
      eta: '5 min 20 s',
      current_action: "Adding 'Real Song' to Qobuz favorites...",
    })
    await flushPromises()

    const text = wrapper.text()
    expect(text).toContain('37 / 100')
    expect(text).toContain('37%')
    expect(text).toContain("Adding 'Real Song' to Qobuz favorites...")

    // Cancel uses the job id carried by the progress events.
    await clickButtonByText(wrapper, 'Cancel Transfer')
    await flushPromises()
    const cancelCall = calls.find(c => c.cmd === 'cancel_migration')
    expect(cancelCall).toBeTruthy()
    expect(cancelCall!.args?.jobId).toBe('job-42')

    resolveStart('job-42')
    await flushPromises()
    // Cancelled: no completed screen, the ready screen is back.
    expect(wrapper.find('[data-testid="transfer-complete"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="start-transfer-btn"]').exists()).toBe(true)
  })

  it('returns to the ready screen with the real error when the backend refuses to start', async () => {
    backend({
      preview_migration: { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] },
      start_migration: null,
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    await clickNext(wrapper)

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="transfer-error"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('Could not start the migration')
    expect(wrapper.text()).not.toContain('Transferring...')
    expect(wrapper.find('[data-testid="start-transfer-btn"]').exists()).toBe(true)
  })

  it('stays cancelled when the engine reports the cancelled job as finished, without claiming a transfer', async () => {
    let resolveStart!: (value: unknown) => void
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    mockInvoke((cmd, args) => {
      calls.push({ cmd, args })
      if (cmd === 'start_migration') return new Promise((resolve) => { resolveStart = resolve })
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_migration_destinations') return engineDestinations
      if (cmd === 'preview_migration') return { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] }
      return []
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    await clickNext(wrapper)

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    emitMockEvent('migration-progress', {
      job_id: 'job-cancel',
      current_item: 12,
      total_items: 100,
      current_track: 'Real Song',
      status: 'running',
      completed_count: 10,
      failed_count: 1,
      skipped_count: 1,
      percent: 12,
      speed: 12.5,
      eta: '6 min 40 s',
      current_action: 'Searching for match',
    })
    await flushPromises()

    await clickButtonByText(wrapper, 'Cancel Transfer')
    await flushPromises()
    expect(calls.find(c => c.cmd === 'cancel_migration')!.args?.jobId).toBe('job-cancel')

    // The engine closes the job it stopped early: the last event says
    // cancelled, with the real counts, never a completed migration.
    emitMockEvent('migration-progress', {
      job_id: 'job-cancel',
      current_item: 12,
      total_items: 100,
      current_track: 'Migration cancelled',
      status: 'cancelled',
      completed_count: 10,
      failed_count: 1,
      skipped_count: 1,
      percent: 12,
      speed: 12.5,
      eta: '0 s',
      current_action: 'Migration cancelled',
    })
    resolveStart('job-cancel')
    await flushPromises()

    expect(wrapper.find('[data-testid="transfer-complete"]').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('Transfer complete!')
    expect(wrapper.find('[data-testid="start-transfer-btn"]').exists()).toBe(true)
  })

  it('starts each migration from zero instead of showing the previous job numbers', async () => {
    let resolveStart!: (value: unknown) => void
    const starts: unknown[] = []
    mockInvoke((cmd) => {
      if (cmd === 'start_migration') {
        starts.push(cmd)
        return new Promise((resolve) => { resolveStart = resolve })
      }
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_migration_destinations') return engineDestinations
      if (cmd === 'preview_migration') return { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] }
      return []
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()

    // First job: real numbers arrive through migration-progress events.
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    let qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    await clickNext(wrapper)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    emitMockEvent('migration-progress', {
      job_id: 'job-first',
      current_item: 73,
      total_items: 100,
      current_track: 'First Song',
      status: 'running',
      completed_count: 70,
      failed_count: 1,
      skipped_count: 2,
      percent: 73,
      speed: 20,
      eta: '2 min',
      current_action: 'Transferring First Song',
    })
    await flushPromises()
    expect(wrapper.text()).toContain('73 / 100')
    resolveStart('job-first')
    emitMockEvent('migration-progress', {
      job_id: 'job-first',
      current_item: 100,
      total_items: 100,
      current_track: 'Migration complete',
      status: 'completed',
      completed_count: 97,
      failed_count: 1,
      skipped_count: 2,
      percent: 100,
      speed: 20,
      eta: '0 s',
      current_action: 'Migration complete',
    })
    await flushPromises()
    expect(wrapper.find('[data-testid="transfer-complete"]').exists()).toBe(true)

    // Second job: nothing of the finished one may leak into its progress screen.
    await clickButtonByText(wrapper, 'New Migration')
    await selectServiceSource(wrapper, 'spotify')
    qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    await clickNext(wrapper)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    expect(starts.length).toBe(2)
    expect(wrapper.text()).toContain('0 / 0 tracks')
    expect(wrapper.text()).toContain('0%')
    expect(wrapper.text()).not.toContain('73 / 100')
    expect(wrapper.text()).not.toContain('97 tracks')

    // The engine's first event of the new job carries the new job's id.
    emitMockEvent('migration-progress', {
      job_id: 'job-second',
      current_item: 0,
      total_items: 42,
      current_track: 'Starting migration...',
      status: 'running',
      completed_count: 0,
      failed_count: 0,
      skipped_count: 0,
      percent: 0,
      speed: 0,
      eta: 'calculating...',
      current_action: 'Starting migration...',
    })
    await flushPromises()
    expect(wrapper.text()).toContain('0 / 42 tracks')
  })

  it('never cancels the previous job when a new migration has not reported its id yet', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    let startResolve!: (value: unknown) => void
    let pendingStart = true
    mockInvoke((cmd, args) => {
      calls.push({ cmd, args })
      if (cmd === 'start_migration') {
        pendingStart = true
        return new Promise((resolve) => { startResolve = resolve })
      }
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_migration_destinations') return engineDestinations
      if (cmd === 'preview_migration') return { total_tracks: 10, matched_tracks: 8, unmatched_tracks: 2, playlists: [] }
      return []
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)

    // First job runs to completion and reports its real job id.
    await selectServiceSource(wrapper, 'spotify')
    let qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    await clickNext(wrapper)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    emitMockEvent('migration-progress', {
      job_id: 'job-done',
      current_item: 10,
      total_items: 10,
      current_track: 'Migration complete',
      status: 'completed',
      completed_count: 10,
      failed_count: 0,
      skipped_count: 0,
      percent: 100,
      speed: 30,
      eta: '0 s',
      current_action: 'Migration complete',
    })
    pendingStart = false
    startResolve('job-done')
    await flushPromises()
    expect(wrapper.find('[data-testid="transfer-complete"]').exists()).toBe(true)

    // Second job starts; its invoke has not resolved yet, so no event of the
    // new job has arrived and only the finished job's id is known.
    await clickButtonByText(wrapper, 'New Migration')
    await selectServiceSource(wrapper, 'spotify')
    qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    await clickNext(wrapper)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    expect(pendingStart).toBe(true)

    await clickButtonByText(wrapper, 'Cancel Transfer')
    await flushPromises()

    const cancelCalls = calls.filter(c => c.cmd === 'cancel_migration')
    // Nothing to cancel yet: the honest outcome is no cancel call at all, never
    // one aimed at the finished job (cancel_migration would silently no-op).
    expect(cancelCalls.length).toBe(0)

    // Once the engine reports the running job, cancelling targets that job.
    emitMockEvent('migration-progress', {
      job_id: 'job-running',
      current_item: 1,
      total_items: 10,
      current_track: 'Real Song',
      status: 'running',
      completed_count: 0,
      failed_count: 0,
      skipped_count: 0,
      percent: 10,
      speed: 5,
      eta: '2 min',
      current_action: 'Transferring Real Song',
    })
    await flushPromises()
    await clickButtonByText(wrapper, 'Cancel Transfer')
    await flushPromises()
    expect(calls.filter(c => c.cmd === 'cancel_migration').map(c => c.args?.jobId)).toEqual(['job-running'])
  })

  it('reloads the reviewable matches of the job that just ran when going back to the review step', async () => {
    let startResolve!: (value: unknown) => void
    const finishedJob: MigrationJob = { ...reviewJob, id: 'svc-job-2', status: 'completed', completed_items: 2, skipped_items: 0 }
    const finishedItem = { ...reviewItem, id: 8, job_id: 'svc-job-2', status: 'transferred', destination_track_id: 'dest-9' }
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    mockInvoke((cmd, args) => {
      calls.push({ cmd, args })
      if (cmd === 'start_migration') return new Promise((resolve) => { startResolve = resolve })
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_accounts') return [
        { id: 11, service_id: 1, service_name: 'spotify', is_active: true },
        { id: 20, service_id: 2, service_name: 'qobuz', is_active: true },
      ]
      if (cmd === 'get_migration_destinations') return engineDestinations
      if (cmd === 'preview_migration') return { total_tracks: 1, matched_tracks: 1, unmatched_tracks: 0, playlists: [] }
      // The job only exists once it has run: history starts empty.
      if (cmd === 'get_migration_history') return calls.some(c => c.cmd === 'start_migration') ? [finishedJob] : []
      if (cmd === 'get_migration_items_by_status') return [finishedItem]
      return []
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await enterServiceMode(wrapper)
    await selectServiceSource(wrapper, 'spotify')
    const qobuzCard = wrapper.findAll('.service-card').find(c => c.attributes('data-service-id') === 'qobuz')!
    await qobuzCard.trigger('click')
    await flushPromises()
    await clickNext(wrapper)

    // Before the job runs there is nothing to review.
    expect(wrapper.find('[data-testid="svc-review-step"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="svc-review-item-row"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('This will be the first migration')

    await clickNext(wrapper)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    emitMockEvent('migration-progress', {
      job_id: 'svc-job-2',
      current_item: 1,
      total_items: 1,
      current_track: 'Migration complete',
      status: 'completed',
      completed_count: 1,
      failed_count: 0,
      skipped_count: 0,
      percent: 100,
      speed: 12,
      eta: '0 s',
      current_action: 'Migration complete',
    })
    startResolve('svc-job-2')
    await flushPromises()

    // Going back reviews the matches of the job that just ran.
    const backButton = wrapper.findAll('button').find(b => b.text().includes('Back'))
    expect(backButton, 'Back button should exist').toBeTruthy()
    await backButton!.trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-testid="svc-review-step"]').exists()).toBe(true)
    const itemCalls = calls.filter(c => c.cmd === 'get_migration_items_by_status')
    expect(itemCalls[itemCalls.length - 1].args?.jobId).toBe('svc-job-2')
    const row = wrapper.find('[data-testid="svc-review-item-row"]')
    expect(row.exists()).toBe(true)
    expect(row.text()).toContain('Unknown Track')
  })
})
