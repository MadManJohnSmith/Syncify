/**
 * Unit tests for DashboardView.vue (TASK-128)
 * Verifies zero fictitious mocks, honest empty states with CTAs,
 * real library growth derivations, and recent activity population.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import DashboardView from '@/views/DashboardView.vue'
import { mockInvoke, resetMocks } from '../setup'

const mockPush = vi.fn()

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: mockPush }),
  useRoute: () => ({ query: {}, params: {} }),
}))

describe('DashboardView.vue (TASK-128 UI Completion)', () => {
  beforeEach(() => {
    resetMocks()
    mockPush.mockReset()

    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') {
        return {
          total_tracks: 0,
          total_albums: 0,
          total_artists: 0,
          playlists: 0,
          total_downloads: 0,
          queued_downloads: 0,
          active_downloads: 0,
        }
      }
      if (command === 'get_queue_stats') {
        return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
      }
      if (command === 'get_service_statuses') return []
      if (command === 'get_metadata_stats') {
        return { total_tracks: 0, complete: 0, missing: 0, lyrics_count: 0 }
      }
      if (command === 'get_lyrics_stats') {
        return { total: 0, synchronized: 0, unsynchronized: 0, missing: 0 }
      }
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') {
        return { used_bytes: 0, available_bytes: 107374182400 }
      }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      if (command === 'check_ffmpeg') return { success: true }
      if (command === 'check_fingerprint') return { success: true }
      return null
    })
  })

  it('renders without fictitious mock data (no hardcoded Jan/Feb/Mar bars)', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    const text = wrapper.text()
    // Should NOT contain fabricated months from the former mock array
    expect(text).not.toContain('Jan')
    expect(text).not.toContain('Feb')
    expect(text).not.toContain('Mar')
    expect(text).not.toContain('Apr')
    expect(text).not.toContain('Jun')
    expect(text).not.toContain('Jul')
  })

  it('renders honest empty states with CTAs when library and queue are brand new', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    // Library growth honest empty state
    expect(wrapper.text()).toContain('No library growth data')
    expect(wrapper.text()).toContain('Track trends and history will appear here once tracks are imported.')

    // Recent activity honest empty state
    expect(wrapper.text()).toContain('No recent activity')
    expect(wrapper.text()).toContain('Activity from downloads and queue tasks will appear here.')

    // Validate CTAs are present and clickable
    const libraryCta = wrapper.findAll('button').find(b => b.text().includes('Go to Library'))
    expect(libraryCta).toBeDefined()
    await libraryCta!.trigger('click')
    expect(mockPush).toHaveBeenCalledWith('/library')

    const queueCta = wrapper.findAll('button').find(b => b.text().includes('Go to Queue'))
    expect(queueCta).toBeDefined()
    await queueCta!.trigger('click')
    expect(mockPush).toHaveBeenCalledWith('/queue')
  })

  it('derives growth stats from real library stats when historical snapshots are empty', async () => {
    mockInvoke((command) => {
      if (command === 'get_library_stats') {
        return {
          total_tracks: 100,
          total_albums: 10,
          total_artists: 5,
          playlists: 2,
          total_downloads: 60,
          queued_downloads: 0,
          active_downloads: 0,
        }
      }
      if (command === 'get_library_snapshots') return []
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('No library growth data')
    expect(wrapper.text()).toContain('Current')
    expect(wrapper.text()).toContain('Total tracks')
    expect(wrapper.text()).toContain('Downloaded')
  })

  it('renders historical snapshot growth bars when snapshots exist', async () => {
    mockInvoke((command) => {
      if (command === 'get_library_snapshots') {
        return [
          { snapshot_date: '2025-03-15', total_tracks: 50, downloaded_tracks: 20 },
          { snapshot_date: '2025-03-16', total_tracks: 80, downloaded_tracks: 40 },
        ]
      }
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('No library growth data')
    expect(wrapper.text()).toContain('03/15')
    expect(wrapper.text()).toContain('03/16')
  })

  it('populates recent activity from real queue downloads', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') {
        return [
          {
            id: 101,
            track_id: 1,
            title: 'Stairway to Heaven',
            artist: 'Led Zeppelin',
            status: 'complete',
            priority: 1,
            progress_percent: 100,
            created_at: new Date(Date.now() - 3600000).toISOString(),
            completed_at: new Date(Date.now() - 1800000).toISOString(),
          },
          {
            id: 102,
            track_id: 2,
            title: 'Bohemian Rhapsody',
            artist: 'Queen',
            status: 'downloading',
            priority: 1,
            progress_percent: 50,
            created_at: new Date(Date.now() - 60000).toISOString(),
            started_at: new Date(Date.now() - 30000).toISOString(),
          },
          {
            id: 103,
            track_id: 3,
            title: 'Hotel California',
            artist: 'Eagles',
            status: 'failed',
            priority: 1,
            progress_percent: 10,
            created_at: new Date(Date.now() - 120000).toISOString(),
            error_message: 'Stream unavailable',
          },
        ]
      }
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('No recent activity')
    expect(wrapper.text()).toContain('Downloaded: Stairway to Heaven • Led Zeppelin')
    expect(wrapper.text()).toContain('Downloading: Bohemian Rhapsody • Queen')
    expect(wrapper.text()).toContain('Failed: Hotel California • Eagles')
  })
})
