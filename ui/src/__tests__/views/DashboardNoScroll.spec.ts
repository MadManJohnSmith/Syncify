/**
 * R12: the dashboard fits one screen (no page scroll) and R6: the free space
 * it shows comes from the real backend figure.
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

const FREE_BYTES = 107374182400 // 100 GiB

function seedLibrary() {
  return {
    total_tracks: 1200,
    total_albums: 80,
    total_artists: 45,
    playlists: 6,
    total_downloads: 900,
    queued_downloads: 4,
    active_downloads: 2,
  }
}

describe('DashboardView without page scroll and with real free space', () => {
  beforeEach(() => {
    resetMocks()
    mockPush.mockReset()

    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') return seedLibrary()
      if (command === 'get_queue_stats') {
        return { total: 12, queued: 4, downloading: 2, completed: 6, failed: 0, paused: 0 }
      }
      if (command === 'get_service_statuses') {
        return [{ name: 'spotify', connected: true, library_count: 1200 }]
      }
      if (command === 'get_metadata_stats') {
        return { total_tracks: 1200, complete: 0, missing: 0, lyrics_count: 0 }
      }
      if (command === 'get_lyrics_stats') {
        return { total: 1200, synchronized: 0, unsynchronized: 0, missing: 0 }
      }
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') {
        return {
          used_bytes: 32212254720,
          total_bytes: 161061273600,
          available_bytes: FREE_BYTES,
          breakdown: [{ format: 'FLAC', size_bytes: 32212254720 }],
        }
      }
      if (command === 'get_top_artists') return [{ name: 'Radiohead', track_count: 120 }]
      if (command === 'get_top_genres') return [{ name: 'Rock', count: 300 }]
      if (command === 'get_audio_quality_distribution') {
        return [{ label: 'FLAC', count: 900 }]
      }
      if (command === 'get_duplicate_stats') return 3
      if (command === 'get_queue') return []
      if (command === 'check_ffmpeg') return { success: true }
      if (command === 'check_fingerprint') return { success: true }
      return null
    })
  })

  it('root and grid do not scroll the page on wide screens', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    const root = wrapper.find('.stats-dashboard')
    expect(root.exists()).toBe(true)
    expect(root.classes()).toContain('overflow-hidden')
    expect(root.classes()).toContain('min-h-0')
    expect(root.classes()).not.toContain('overflow-y-auto')

    // The wrapper only overflows below the xl breakpoint, where the 12-column grid
    // that fits one screen is not active yet.
    const scroller = wrapper
      .findAll('.stats-dashboard > div')
      .find(node => node.classes().includes('overflow-y-auto'))
    expect(scroller).toBeDefined()
    expect(scroller!.classes()).toContain('xl:overflow-hidden')
    expect(scroller!.classes()).toContain('min-h-0')
  })

  it('each panel clips instead of stretching the page', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    const cards = wrapper.findAll('.stat-card')
    expect(cards.length).toBe(13)
    for (const card of cards) {
      expect(card.classes()).toContain('overflow-hidden')
      expect(card.classes()).toContain('min-h-0')
    }
  })

  it('keeps all thirteen sections visible at a glance', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    for (const panel of [
      'library-overview',
      'growth-chart',
      'storage-usage',
      'quality-distribution',
      'service-distribution',
      'queue-stats',
      'recent-activity',
      'system-diagnostics',
      'metadata-quality',
      'lyrics-coverage',
      'top-artists',
      'top-genres',
      'duplicates',
    ]) {
      expect(wrapper.find(`.${panel}`).exists(), `falta el panel ${panel}`).toBe(true)
    }
  })

  it('shows the real free space, not a zero or filler text', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    const storage = wrapper.find('.storage-usage')
    expect(storage.text()).toContain('30 GB')
    expect(storage.text()).toContain('100 GB free on disk')
    expect(storage.text()).not.toContain('0 Bytes free')
  })

  it('does not claim a full disk when the backend could not measure storage', async () => {
    mockInvoke((command) => {
      if (command === 'get_storage_stats') {
        // storage.rs leaves total_bytes = 0 when it cannot locate the disk.
        return { used_bytes: 0, total_bytes: 0, available_bytes: 0, breakdown: [] }
      }
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    const storage = wrapper.find('.storage-usage')
    expect(storage.exists()).toBe(true)
    expect(storage.text()).toContain('Free space unavailable')
    expect(storage.text()).not.toContain('0 Bytes free on disk')
  })

  it('shows a backend failure as an error, not as zeroed statistics', async () => {
    mockInvoke((command) => {
      if (command === 'get_library_stats') {
        throw new Error('Dashboard stats error (tracks): no such table: tracks')
      }
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).toContain('Failed to load dashboard')
    expect(wrapper.text()).toContain('no such table: tracks')
    // With no data the grid that would look like an empty library is not mounted.
    expect(wrapper.find('.stat-card').exists()).toBe(false)
  })
})