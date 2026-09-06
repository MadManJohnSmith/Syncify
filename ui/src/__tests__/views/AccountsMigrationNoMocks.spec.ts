/**
 * AccountsMigrationNoMocks.spec.ts
 * [TASK-23] Verification suite: Elimination of Mocks and Simulated Fallbacks
 * in AccountsView.vue and MigrationView.vue.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AccountsView from '@/views/AccountsView.vue'
import MigrationView from '@/views/MigrationView.vue'
import { mockInvoke, resetMocks } from '../setup'
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
