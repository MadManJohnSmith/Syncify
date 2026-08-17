/**
 * Unit tests for DownloadsView.vue
 * Tests virtual scrolling, live concurrency controls, throttling, and reactive filtering
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import DownloadsView from '../../views/DownloadsView.vue'
import { mockInvoke, resetMocks, emitMockEvent } from '../setup'
import { invoke } from '@tauri-apps/api/core'

// Generate sample queue items including a large batch
const mockQueueItems = [
  {
    id: 1,
    track_id: 101,
    target_title: 'Synchronize',
    target_artist: 'Cosmic Sound',
    target_album: 'Starlight',
    service_name: 'qobuz',
    quality_preference: 'hires',
    status: 'downloading',
    priority: 60,
    progress_percent: 45,
    error_message: null,
    created_at: '2026-08-17T06:00:00Z',
    started_at: '2026-08-17T06:01:00Z',
    completed_at: null,
  },
  {
    id: 2,
    track_id: 102,
    target_title: 'Mass Track 2',
    target_artist: 'Electronic Waves',
    target_album: 'Frequency',
    service_name: 'tidal',
    quality_preference: 'lossless',
    status: 'queued',
    priority: 50,
    progress_percent: 0,
    error_message: null,
    created_at: '2026-08-17T06:00:01Z',
    started_at: null,
    completed_at: null,
  },
  {
    id: 3,
    track_id: 103,
    target_title: 'Finished Track 3',
    target_artist: 'Audio Master',
    target_album: 'Complete Works',
    service_name: 'qobuz',
    quality_preference: 'lossless',
    status: 'complete',
    priority: 50,
    progress_percent: 100,
    error_message: null,
    created_at: '2026-08-17T05:00:00Z',
    started_at: '2026-08-17T05:01:00Z',
    completed_at: '2026-08-17T05:03:00Z',
  },
  {
    id: 4,
    track_id: 104,
    target_title: 'Corrupted Source',
    target_artist: 'Broken Beats',
    target_album: 'Error Vol 1',
    service_name: 'spotify',
    quality_preference: 'lossless',
    status: 'failed',
    priority: 50,
    progress_percent: 0,
    error_message: 'SourceNotFound: 404 Stale Stream URL',
    created_at: '2026-08-17T04:00:00Z',
    started_at: '2026-08-17T04:01:00Z',
    completed_at: null,
  }
]

const mockStats = {
  total: 4,
  queued: 1,
  downloading: 1,
  completed: 1,
  failed: 1,
  paused: 0,
}

const mockWorkerStatus = {
  running: true,
  paused: false,
  active_downloads: 1,
  max_concurrent: 3,
  is_running: true,
  is_paused: false,
}

describe('DownloadsView.vue', () => {
  beforeEach(() => {
    resetMocks()
  })

  it('renders downloads view and status cards', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      if (command === 'audit_download_queue') return {
        total_items: 4,
        ready_count: 1,
        source_locked_count: 3,
        legacy_unresolved_count: 0,
        stale_source_count: 1,
        ambiguous_source_count: 0,
        source_identity_missing_count: 0,
        completed_count: 1,
        failed_count: 1,
        downloading_count: 1,
      }
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    expect(wrapper.text()).toContain('Downloads')
    expect(wrapper.text()).toContain('3 Threads')
    expect(wrapper.text()).toContain('Synchronize')
    expect(wrapper.text()).toContain('Mass Track 2')
  })

  it('allows live concurrency switching between 1 and 5 threads', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Find concurrency buttons 1-5
    const buttons = wrapper.findAll('.downloads-page button[title*="concurrent download thread"]')
    expect(buttons.length).toBe(5)

    // Click on 5 threads
    await buttons[4].trigger('click')
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('set_max_concurrent_downloads', { max: 5 })
  })

  it('filters queue reactively by text search', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    const searchInput = wrapper.find('input[type="text"][placeholder*="Filter queue"]')
    expect(searchInput.exists()).toBe(true)

    // Filter by "Cosmic"
    await searchInput.setValue('Cosmic')
    await flushPromises()

    expect(wrapper.text()).toContain('Synchronize')
    expect(wrapper.text()).not.toContain('Mass Track 2')
  })

  it('filters sections when clicking status tabs', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Find tab buttons
    const queuedTab = wrapper.findAll('.downloads-toolbar button').find(b => b.text().includes('Queued'))
    expect(queuedTab).toBeDefined()
    await queuedTab?.trigger('click')
    await flushPromises()

    // Active downloads section should be hidden when 'queued' filter is active
    expect(wrapper.find('.active-downloads').exists()).toBe(false)
    expect(wrapper.find('.queue-section').exists()).toBe(true)
  })

  it('handles virtual rendering slice for mass batches', async () => {
    // Generate 500 queued items to test virtual list window
    const largeQueue = Array.from({ length: 500 }, (_, i) => ({
      id: i + 10,
      track_id: i + 1000,
      target_title: `Virtual Track ${i + 1}`,
      target_artist: `Artist ${i % 10}`,
      target_album: `Album ${i % 5}`,
      service_name: 'qobuz',
      quality_preference: 'hires',
      status: 'queued',
      priority: 50,
      progress_percent: 0,
      error_message: null,
      created_at: '2026-08-17T06:00:00Z',
      started_at: null,
      completed_at: null,
    }))

    mockInvoke((command) => {
      if (command === 'get_queue') return largeQueue
      if (command === 'get_queue_stats') return { ...mockStats, queued: 500, total: 500 }
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Verify virtual window only renders a small subset in DOM
    const renderedQueueItems = wrapper.findAll('.queue-item')
    expect(renderedQueueItems.length).toBeLessThan(50)
    expect(renderedQueueItems.length).toBeGreaterThan(0)
  })
})
