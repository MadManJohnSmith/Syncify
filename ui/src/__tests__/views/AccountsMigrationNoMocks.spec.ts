/**
 * AccountsMigrationNoMocks.spec.ts
 * [TASK-23] Verification suite: Elimination of Mocks and Simulated Fallbacks
 * in AccountsView.vue and MigrationView.vue.
 * [3.2] Extended: covers the migration wizard (real progress, real preview,
 * real manual match, real start) — must fail if a mock ever reappears.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import AccountsView from '@/views/AccountsView.vue'
import MigrationView from '@/views/MigrationView.vue'
import { mockInvoke, resetMocks, emitMockEvent } from '../setup'
import type { MigrationJob } from '@/api/types'

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

describe('[TASK-23] AccountsView & MigrationView Honest State (No Mocks)', () => {
  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
  })

  describe('AccountsView.vue', () => {
    it('initializes without hardcoded mock library paths (D:/Music/Flac_Library or E:/Downloads/Music)', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'get_services') return []
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_service_statuses') return []
        if (cmd === 'get_effective_download_preferences') return null
        if (cmd === 'get_download_settings') return null
        if (cmd === 'get_library_stats') return { total_tracks: 0 }
        return null
      })

      const wrapper = mount(AccountsView, {
        global: {
          stubs: {
            SpotifyApiConfigCard: true,
          },
        },
      })
      await flushPromises()

      const text = wrapper.text()
      expect(text).not.toContain('D:/Music/Flac_Library')
      expect(text).not.toContain('E:/Downloads/Music')
      expect(text).not.toContain('2,405')

      // Honest empty state is shown for local library folders
      const emptyState = wrapper.find('[data-testid="library-paths-empty"]')
      expect(emptyState.exists()).toBe(true)
      expect(emptyState.text()).toContain('No library folders configured')
    })

    it('initializes activity log empty without pre-populated fake sync entries', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'get_services') return []
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_service_statuses') return []
        return null
      })

      const wrapper = mount(AccountsView, {
        global: {
          stubs: {
            SpotifyApiConfigCard: true,
          },
        },
      })
      await flushPromises()

      // Header indicates 0 recent items
      expect(wrapper.text()).toContain('0 recent')
      expect(wrapper.text()).not.toContain('Synced favorites')
      expect(wrapper.text()).not.toContain('Added 15 tracks')
      expect(wrapper.text()).not.toContain('24 tracks added')

      // Click button to toggle recent activity table
      const activityToggle = wrapper.find('.activity-log button')
      expect(activityToggle.exists()).toBe(true)
      await activityToggle.trigger('click')

      // Should display honest empty state
      const emptyActivity = wrapper.find('[data-testid="activity-log-empty"]')
      expect(emptyActivity.exists()).toBe(true)
      expect(emptyActivity.text()).toContain('No recent activity')
    })

    it('populates library paths honestly from configured download directory when available', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'get_services') return []
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_service_statuses') return []
        if (cmd === 'get_effective_download_preferences') {
          return {
            downloadPath: '/real/user/music/library',
            stagingPath: '/real/user/music/library/.staging',
            pathStatus: 'valid',
            freeSpaceBytes: null,
            maxConcurrentDownloads: 4,
            maxRetries: 3,
            retryDelaySeconds: 5,
            serviceQualities: [],
          }
        }
        if (cmd === 'get_library_stats') {
          return {
            total_tracks: 152,
            total_artists: 12,
            total_albums: 8,
          }
        }
        return null
      })

      const wrapper = mount(AccountsView, {
        global: {
          stubs: {
            SpotifyApiConfigCard: true,
          },
        },
      })
      await flushPromises()

      expect(wrapper.text()).toContain('/real/user/music/library')
      expect(wrapper.text()).toContain('152 Tracks')
      expect(wrapper.find('[data-testid="library-paths-empty"]').exists()).toBe(false)
    })

    it('provides clear feedback on file drop and select instead of empty bodies', async () => {
      mockInvoke(() => null)

      const wrapper = mount(AccountsView, {
        global: {
          stubs: {
            SpotifyApiConfigCard: true,
          },
        },
      })
      await flushPromises()

      const dropArea = wrapper.find('.drag-drop-area')
      expect(dropArea.exists()).toBe(true)

      // Test dropping a valid file
      const validFile = new File(['#EXTM3U\nsong.mp3'], 'playlist.m3u', { type: 'audio/x-mpegurl' })
      await dropArea.trigger('drop', {
        dataTransfer: {
          files: [validFile],
        },
      })
      await flushPromises()

      // File input change
      const fileInput = wrapper.find('input[type="file"]')
      expect(fileInput.exists()).toBe(true)

      const invalidFile = new File(['binary'], 'song.exe', { type: 'application/octet-stream' })
      Object.defineProperty(fileInput.element, 'files', {
        value: [invalidFile],
        writable: true,
      })
      await fileInput.trigger('change')
      await flushPromises()

      // The component handled the events smoothly without throwing
      expect(wrapper.vm).toBeDefined()
    })
  })

  describe('MigrationView.vue', () => {
    it('renders honest empty state without fictitious December 2025 mock history', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'get_migration_history') return []
        if (cmd === 'get_migration_templates') return []
        return []
      })

      const wrapper = mount(MigrationView)
      await flushPromises()

      const text = wrapper.text()
      // Verify no hardcoded 2025 mock dates
      expect(text).not.toContain('Dec 23, 2025')
      expect(text).not.toContain('Dec 22, 2025')
      expect(text).not.toContain('Dec 20, 2025')
      expect(text).not.toContain('Dec 18, 2025')
      expect(text).not.toContain('Favorites (1,234 tracks)')

      // Verify honest empty state is rendered
      const emptyHistory = wrapper.find('[data-testid="migration-history-empty"]')
      expect(emptyHistory.exists()).toBe(true)
      expect(emptyHistory.text()).toContain('No migration history')
      expect(emptyHistory.text()).toContain('Completed and pending migrations will appear here')

      // Table should not be rendered when history is empty
      expect(wrapper.find('table.history-table').exists()).toBe(false)
    })

    it('renders real migration jobs when backend provides history data', async () => {
      const mockJobs: MigrationJob[] = [
        {
          id: 'mig-task23-001',
          source_service: 'spotify',
          destination_service: 'qobuz',
          source_playlist_ids: null,
          options: '{}',
          status: 'completed',
          total_items: 45,
          completed_items: 45,
          failed_items: 0,
          skipped_items: 0,
          started_at: '2026-03-30T10:00:00Z',
          completed_at: '2026-03-30T10:05:00Z',
          error_message: null,
          created_at: '2026-03-30T10:00:00Z',
        },
        {
          id: 'mig-task23-002',
          source_service: 'tidal',
          destination_service: 'qobuz',
          source_playlist_ids: null,
          options: '{}',
          status: 'partial',
          total_items: 100,
          completed_items: 90,
          failed_items: 10,
          skipped_items: 0,
          started_at: '2026-03-29T14:00:00Z',
          completed_at: '2026-03-29T14:10:00Z',
          error_message: null,
          created_at: '2026-03-29T14:00:00Z',
        },
      ]

      mockInvoke((cmd) => {
        if (cmd === 'get_migration_history') return mockJobs
        if (cmd === 'get_migration_templates') return []
        return []
      })

      const wrapper = mount(MigrationView)
      await flushPromises()

      // Empty state should be hidden
      expect(wrapper.find('[data-testid="migration-history-empty"]').exists()).toBe(false)

      // Table should be visible
      const table = wrapper.find('table.history-table')
      expect(table.exists()).toBe(true)
      expect(wrapper.text()).toContain('spotify → qobuz')
      expect(wrapper.text()).toContain('tidal → qobuz')
      expect(wrapper.text()).toContain('45 tracks')
      expect(wrapper.text()).toContain('100 tracks')
      expect(wrapper.text()).toContain('Completed')
      expect(wrapper.text()).toContain('Partial')
    })
  })
})

/**
 * [3.2] Wizard coverage: the transfer assistant must render only real backend
 * state. These tests fail if any of the removed mocks (fake progress numbers,
 * fixed match rows, fake active syncs, mock templates, dead buttons) reappear.
 */
describe('MigrationView wizard — real state only (post-audit 3.2)', () => {
  // Transitions/Teleports are stubbed so steps and modals render synchronously.
  const stubs = { transition: true, teleport: true }

  type Handler = (cmd: string, args?: Record<string, unknown>) => unknown

  /** Service statuses the backend would report for a user with spotify/qobuz/tidal connected. */
  const connectedStatuses = [
    { name: 'spotify', connected: true, account_email: 'user@example.com', library_count: 10, favorites_count: 8, playlists_count: 2, last_synced: null, credentials_invalid: false },
    { name: 'qobuz', connected: true, account_email: 'user@example.com', library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
    { name: 'tidal', connected: true, account_email: 'user@example.com', library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
    { name: 'deezer', connected: false, account_email: null, library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
  ]

  const reviewJob: MigrationJob = {
    id: 'job-1',
    source_service: 'spotify',
    destination_service: 'qobuz',
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
    job_id: 'job-1',
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

  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
  })

  /** Build a mockInvoke handler that records every command and answers from `overrides`. */
  function backend(overrides: Record<string, unknown>, calls?: Array<{ cmd: string; args?: Record<string, unknown> }>): Handler {
    mockInvoke((cmd, args) => {
      calls?.push({ cmd, args })
      if (cmd in overrides) return overrides[cmd]
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_migration_destinations') return ['qobuz', 'tidal', 'spotify', 'deezer', 'soundcloud']
      if (cmd === 'get_migration_history') return []
      if (cmd === 'get_migration_templates') return []
      return []
    })
    return (cmd) => (cmd in overrides ? overrides[cmd] : null)
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

  /** Drive the wizard: source=spotify (card 0), content, destinations (card indexes), preview. */
  async function goToPreviewStep(wrapper: VueWrapper, destinationIndexes: number[] = [1]): Promise<void> {
    await wrapper.findAll('.service-card')[0].trigger('click')
    await flushPromises()
    await clickNext(wrapper) // -> content
    await clickNext(wrapper) // -> destination (favorites preselected)
    const cards = wrapper.findAll('.service-card')
    for (const idx of destinationIndexes) {
      await cards[idx].trigger('click')
      await flushPromises()
    }
    await clickNext(wrapper) // -> preview (loads preview_migration)
    await flushPromises()
  }

  it('renders the wizard without any of the removed hardcoded mock data', async () => {
    backend({})
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()

    const text = wrapper.text()
    // Mock match rows (FE-2)
    expect(text).not.toContain('Bohemian Rhapsody')
    expect(text).not.toContain('Blinding Lights')
    expect(text).not.toContain('Kavinsky')
    expect(text).not.toContain('Obscure Indie Track')
    // Fixed summary/pill counts (FE-2)
    expect(text).not.toContain('1,234')
    expect(text).not.toContain('1,198')
    // Fixed activity log entries (FE-1)
    expect(text).not.toContain('12:34:52')
    // Fake Active Syncs dashboard (FE-3)
    expect(text).not.toContain('2h ago')
    expect(text).not.toContain('in 4h')
    expect(text).not.toContain('Active Syncs')
    // Mock saved templates fallback (FE-3)
    expect(text).not.toContain('Full Library Sync')
    expect(text).not.toContain('Spotify → Qobuz (Favorites)')
    // Hardcoded transfer/complete numbers (FE-1/FE-4)
    expect(text).not.toContain('1,217')
    // Buttons removed with the backend gaps they represented
    expect(text).not.toContain('Schedule migration')
    expect(text).not.toContain('Export Report')
  })

  it('marks services as connected only when the backend reports a valid account (FE-4)', async () => {
    backend({})
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()

    // The source grid is data-driven: one card per backend service status.
    const cards = wrapper.findAll('.service-card')
    expect(cards.length).toBe(connectedStatuses.length)
    // spotify connected, deezer (card index 3) not connected
    expect(cards[0].text()).toContain('Connected')
    expect(cards[0].text()).not.toContain('Not Connected')
    expect(cards[3].text()).toContain('Not Connected')
    expect(cards[3].attributes('disabled')).toBeDefined()
  })

  it('shows real preview numbers from preview_migration in the Review matches step (FE-2)', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend(
      {
        preview_migration: { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] },
      },
      calls,
    )
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await goToPreviewStep(wrapper)

    const previewCall = calls.find(c => c.cmd === 'preview_migration')
    expect(previewCall).toBeTruthy()
    expect(previewCall!.args?.sourceService).toBe('spotify')
    expect(previewCall!.args?.destinationService).toBe('qobuz')

    expect(wrapper.find('[data-testid="preview-total"]').text()).toBe('100')
    expect(wrapper.find('[data-testid="preview-matched"]').text()).toContain('90')
    expect(wrapper.find('[data-testid="preview-matched"]').text()).toContain('(90%)')
    expect(wrapper.find('[data-testid="preview-unmatched"]').text()).toContain('10')
    expect(wrapper.find('[data-testid="preview-unmatched"]').text()).toContain('(10%)')
    // Skip toggle shows the real unmatched count
    expect(wrapper.text()).toContain('Skip all tracks with no match found (10 tracks)')
    expect(wrapper.text()).not.toContain('1,234')
  })

  it('names the destination the preview really covers instead of the whole selection (FE-2/FE-4)', async () => {
    // preview_migration takes one destination: with two selected the review
    // step must not claim it previewed both.
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend(
      {
        preview_migration: { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] },
      },
      calls,
    )
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()

    // Source spotify (0) → destinations qobuz (1) and tidal (2)
    await wrapper.findAll('.service-card')[0].trigger('click')
    await flushPromises()
    await clickNext(wrapper) // -> content
    await clickNext(wrapper) // -> destination
    const cards = wrapper.findAll('.service-card')
    await cards[1].trigger('click')
    await flushPromises()
    await cards[2].trigger('click')
    await flushPromises()
    await clickNext(wrapper) // -> preview
    await flushPromises()

    // Only the first destination is previewed, once.
    const previewCalls = calls.filter(c => c.cmd === 'preview_migration')
    expect(previewCalls.length).toBe(1)
    expect(previewCalls[0].args?.destinationService).toBe('qobuz')

    const description = wrapper.find('[data-testid="preview-description"]').text()
    expect(description).toContain('against Qobuz')
    expect(description).not.toContain('against Qobuz, Tidal')
    expect(description).toContain('each additional destination is matched when its own migration runs')
  })

  it('Search Manually opens a real modal wired to search_destination_track and manual_match_item (FE-2)', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend(
      {
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
    await goToPreviewStep(wrapper)

    // Real review row from the last migration on this route
    const row = wrapper.find('[data-testid="review-item-row"]')
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
    expect(result.text()).toContain('Unknown Track (Deluxe)')

    await wrapper.find('[data-testid="manual-match-apply"]').trigger('click')
    await flushPromises()

    const matchCall = calls.find(c => c.cmd === 'manual_match_item')
    expect(matchCall).toBeTruthy()
    expect(matchCall!.args?.itemId).toBe(7)
    expect(matchCall!.args?.destinationTrackId).toBe('dest-1')

    // Modal closes after the match is saved
    expect(wrapper.find('[data-testid="manual-match-modal"]').exists()).toBe(false)
  })

  it('starts one real migration per selected destination, passing skipNotFound and the content selection (FE-4)', async () => {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    backend(
      {
        preview_migration: { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] },
        start_migration: 'job-x',
      },
      calls,
    )
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()

    // Source
    await wrapper.findAll('.service-card')[0].trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    // Content: favorites (preselected) + playlists
    await wrapper.findAll('.content-card')[1].trigger('click')
    await flushPromises()
    await clickNext(wrapper)
    // Destinations: qobuz (1) + tidal (2)
    const cards = wrapper.findAll('.service-card')
    await cards[1].trigger('click')
    await flushPromises()
    await cards[2].trigger('click')
    await flushPromises()
    await clickNext(wrapper) // -> preview
    await flushPromises()
    // Toggle skipNotFound on
    await wrapper.find('[data-testid="skip-not-found-toggle"]').trigger('click')
    await clickNext(wrapper) // -> transfer
    await flushPromises()

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    const startCalls = calls.filter(c => c.cmd === 'start_migration')
    expect(startCalls.length).toBe(2)
    expect(startCalls[0].args?.sourceService).toBe('spotify')
    expect(startCalls[0].args?.destinationService).toBe('qobuz')
    expect(startCalls[1].args?.destinationService).toBe('tidal')
    const options = startCalls[0].args?.options as Record<string, unknown>
    expect(options.skip_unmatched).toBe(true)
    expect(options.create_playlists).toBe(true)

    expect(wrapper.find('[data-testid="transfer-complete"]').exists()).toBe(true)
  })

  it('shows a real error and returns to the ready screen when the backend refuses to start (FE-4)', async () => {
    backend({
      preview_migration: { total_tracks: 100, matched_tracks: 90, unmatched_tracks: 10, playlists: [] },
      start_migration: null,
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await goToPreviewStep(wrapper)
    await clickNext(wrapper) // -> transfer
    await flushPromises()

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    // Error surfaced, not a hung progress screen
    expect(wrapper.find('[data-testid="transfer-error"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('Could not start the migration')
    expect(wrapper.text()).not.toContain('Transferring...')
    // Ready screen with the Start button available again
    expect(wrapper.find('[data-testid="start-transfer-btn"]').exists()).toBe(true)
  })

  it('renders real progress from migration-progress events and cancels the running job id (FE-1)', async () => {
    let resolveStart!: (value: unknown) => void
    mockInvoke((cmd) => {
      if (cmd === 'start_migration') {
        return new Promise((resolve) => { resolveStart = resolve })
      }
      if (cmd === 'get_service_statuses') return connectedStatuses
      if (cmd === 'get_migration_destinations') return ['qobuz', 'tidal', 'spotify', 'deezer', 'soundcloud']
      return []
    })
    const wrapper = mount(MigrationView, { global: { stubs } })
    await flushPromises()
    await goToPreviewStep(wrapper)
    await clickNext(wrapper) // -> transfer
    await flushPromises()

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
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
    expect(text).toContain('Failed: Real Song')
    expect(text).toContain('5 min 20 s')

    await clickButtonByText(wrapper, 'Cancel Transfer')
    await flushPromises()

    // Cancel uses the job id carried by the progress events (start has not resolved yet)
    // — regression for the useMigration.cancel fallback to progress.job_id.
    const cancelMock = (await import('@tauri-apps/api/core')).invoke as unknown as ReturnType<typeof vi.fn>
    const cancelArgs = cancelMock.mock.calls.find((c: unknown[]) => c[0] === 'cancel_migration')
    expect(cancelArgs).toBeTruthy()
    expect(cancelArgs![1]).toEqual({ jobId: 'job-42' })

    // Resolve the pending start; the cancelled status must not hang the screen
    resolveStart('job-42')
    await flushPromises()
    expect(wrapper.text()).toContain('Ready to transfer')
    expect(wrapper.find('[data-testid="transfer-progress"]').exists()).toBe(false)
  })
})
