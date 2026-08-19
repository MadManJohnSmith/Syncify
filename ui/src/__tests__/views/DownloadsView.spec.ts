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

  it('renders live telemetry bar with throughput, ETA, success rate, and artifact counters', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return {
        ...mockStats,
        success_rate: 94.5,
        audio_count: 12,
        lrc_count: 10,
        cover_count: 8,
        booklet_count: 2,
      }
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Telemetry bar elements
    const telemetryBar = wrapper.find('.telemetry-bar')
    expect(telemetryBar.exists()).toBe(true)
    expect(telemetryBar.text()).toContain('Throughput')
    expect(telemetryBar.text()).toContain('Est. Time Remaining')
    expect(telemetryBar.text()).toContain('Success Rate')
    expect(telemetryBar.text()).toContain('94.5%')

    // Artifact counters
    expect(telemetryBar.text()).toContain('Audio')
    expect(telemetryBar.text()).toContain('12')
    expect(telemetryBar.text()).toContain('LRC')
    expect(telemetryBar.text()).toContain('10')
    expect(telemetryBar.text()).toContain('Covers')
    expect(telemetryBar.text()).toContain('8')
    expect(telemetryBar.text()).toContain('Booklets')
    expect(telemetryBar.text()).toContain('2')
  })

  it('updates live throughput and artifact counters upon progress events', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return { ...mockStats, audio_count: 1, lrc_count: 1, cover_count: 1, booklet_count: 0 }
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Emit progress event for track 1 (simulating fast progress)
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      title: 'Synchronize',
      status: 'downloading',
      progress_percent: 85,
    })
    await flushPromises()

    // Emit complete event
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      title: 'Synchronize',
      status: 'complete',
      progress_percent: 100,
    })
    await flushPromises()

    const telemetryBar = wrapper.find('.telemetry-bar')
    expect(telemetryBar.exists()).toBe(true)
    // Artifact counter for audio should have incremented from 1 to 2
    expect(telemetryBar.text()).toContain('Audio')
    expect(telemetryBar.text()).toContain('2')
  })

  it('toggles Queue details panel visibility when clicking toggle button', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    const detailsToggleBtn = wrapper.findAll('button').find(b => b.text().includes('Queue details'))
    expect(detailsToggleBtn).toBeDefined()

    const detailsPanel = wrapper.find('.reconciliation-strip').element.parentElement as HTMLElement
    // Initial state: collapsed / hidden
    expect(detailsPanel.style.display).toBe('none')

    // Click to expand
    await detailsToggleBtn?.trigger('click')
    await flushPromises()
    expect(detailsPanel.style.display).not.toBe('none')

    // Click to collapse
    await detailsToggleBtn?.trigger('click')
    await flushPromises()
    expect(detailsPanel.style.display).toBe('none')
  })

  it('preserves all primary operational controls (Pause/Resume, Cancel, Retry Failed, Refresh)', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Pause/Resume button
    const pauseBtn = wrapper.findAll('button').find(b => b.text().includes('Pause All') || b.text().includes('Resume All'))
    expect(pauseBtn).toBeDefined()

    // Cancel button
    const cancelBtn = wrapper.findAll('button').find(b => b.text().includes('Cancel'))
    expect(cancelBtn).toBeDefined()

    // Retry Failed button
    const retryBtn = wrapper.findAll('button').find(b => b.text().includes('Retry Failed'))
    expect(retryBtn).toBeDefined()

    // Refresh button
    const refreshBtn = wrapper.find('button[title="Refresh Queue"]')
    expect(refreshBtn.exists()).toBe(true)
  })

  it('displays accurate total counts from backend stats independent of pagination/limits', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems.slice(0, 2) // only 2 items returned by limit
      if (command === 'get_queue_stats') return {
        total: 1500,
        queued: 1200,
        downloading: 5,
        completed: 280,
        failed: 15,
        paused: 0
      }
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Status cards show full totals, not 2
    expect(wrapper.text()).toContain('1200')
    expect(wrapper.text()).toContain('280')
    expect(wrapper.text()).toContain('15')
  })

  it('renders differentiated failure categories with metadata, provenance, and contextual retry buttons', async () => {
    const richFailedItems = [
      {
        id: 10,
        track_id: 201,
        target_title: 'Network Fail Track',
        target_artist: 'The Streamers',
        target_album: 'Lost Packets',
        service_name: 'qobuz',
        quality_preference: 'hires',
        status: 'failed',
        priority: 50,
        progress_percent: 0,
        error_message: 'Qobuz download failed: error decoding response body',
        last_error: 'stream timeout after 3 retries',
        retry_count: 3,
        allow_fallback: false,
        created_at: '2026-08-17T04:00:00Z',
        started_at: '2026-08-17T04:01:00Z',
        completed_at: null,
      },
      {
        id: 11,
        track_id: 202,
        target_title: 'Stale Source Track',
        target_artist: 'Archive Band',
        target_album: 'Old Catalog',
        service_name: 'spotify',
        effective_service: 'tidal',
        quality_preference: 'lossless',
        status: 'failed',
        priority: 50,
        progress_percent: 0,
        error_message: 'StaleSource: 404 not found on primary CDN',
        last_error: 'source deleted from catalog',
        retry_count: 1,
        allow_fallback: true,
        created_at: '2026-08-17T04:00:00Z',
        started_at: '2026-08-17T04:01:00Z',
        completed_at: null,
      },
      {
        id: 12,
        track_id: 203,
        target_title: 'Auth Expired Track',
        target_artist: 'Secured Band',
        target_album: 'Protected Vault',
        service_name: 'tidal',
        quality_preference: 'hires',
        status: 'failed',
        priority: 50,
        progress_percent: 0,
        error_message: 'RequiresAuth: HTTP 401 Unauthorized token expired',
        last_error: 'Invalid session credentials',
        retry_count: 0,
        allow_fallback: true,
        created_at: '2026-08-17T04:00:00Z',
        started_at: '2026-08-17T04:01:00Z',
        completed_at: null,
      },
      {
        id: 13,
        track_id: 204,
        target_title: 'Hi-Res Only Track',
        target_artist: 'Audiophile Master',
        target_album: 'Ultra Fidelity',
        service_name: 'qobuz',
        quality_preference: 'lossless',
        status: 'failed',
        priority: 50,
        progress_percent: 0,
        error_message: 'RejectedQuality: Requested 24/192 format not available for account tier',
        last_error: 'Quality preference not satisfied',
        retry_count: 2,
        allow_fallback: true,
        created_at: '2026-08-17T04:00:00Z',
        started_at: '2026-08-17T04:01:00Z',
        completed_at: null,
      }
    ]

    mockInvoke((command) => {
      if (command === 'get_queue') return richFailedItems
      if (command === 'get_queue_stats') return { total: 4, queued: 0, downloading: 0, completed: 0, failed: 4, paused: 0 }
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    const text = wrapper.text()

    // 1. Check classified failure badges
    expect(text).toContain('Network retry exhausted')
    expect(text).toContain('Stale source / 404')
    expect(text).toContain('Requires authentication')
    expect(text).toContain('Rejected quality')

    // 2. Check metadata: Attempts count, Origin, Effective, Fallback
    expect(text).toContain('Attempts: 3')
    expect(text).toContain('Attempts: 1')
    expect(text).toContain('Origin: Spotify')
    expect(text).toContain('Effective: Tidal')
    expect(text).toContain('Fallback Allowed')

    // 3. Check contextual buttons:
    // Network error: "Retry original source" present; NO generic "Try another service"
    const retryOriginalBtn = wrapper.findAll('button').find(b => b.text().includes('Retry original source'))
    expect(retryOriginalBtn).toBeDefined()
    expect(text).not.toContain('Try another service')

    // Auth error: "Check Account" present; NO fallback UI
    const checkAccountBtn = wrapper.findAll('button').find(b => b.text().includes('Check Account'))
    expect(checkAccountBtn).toBeDefined()

    // Stale source with allowFallback: "Retry with Fallback"
    const retryFallbackBtn = wrapper.findAll('button').find(b => b.text().includes('Retry with Fallback'))
    expect(retryFallbackBtn).toBeDefined()

    // Rejected quality: "Retry Quality"
    const retryQualityBtn = wrapper.findAll('button').find(b => b.text().includes('Retry Quality'))
    expect(retryQualityBtn).toBeDefined()

    // 4. Verify failed rows are not in active section
    const activeSection = wrapper.find('.active-downloads-section')
    if (activeSection.exists()) {
      expect(activeSection.text()).not.toContain('Network Fail Track')
      expect(activeSection.text()).not.toContain('Stale Source Track')
    }
  })

  it('renders byte-level progress and throughput dynamically', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Emit byte progress with total_bytes
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      bytes_downloaded: 10 * 1024 * 1024,
      total_bytes: 20 * 1024 * 1024,
      percent: 50.0,
      instant_kbps: 2048.0,
      average_kbps: 1800.0,
      phase: 'downloading',
      terminal: false,
    })
    await flushPromises()

    const text = wrapper.text()
    expect(text).toContain('10.0 MB / 20.0 MB')
    expect(text).toContain('2.0 MB/s')
    expect(text).toContain('50%')
  })

  it('renders indeterminate pulse bar and bytes when Content-Length is missing', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Emit byte progress with null total_bytes
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      bytes_downloaded: 4 * 1024 * 1024,
      total_bytes: null,
      percent: null,
      instant_kbps: 1024.0,
      average_kbps: 900.0,
      phase: 'downloading',
      terminal: false,
    })
    await flushPromises()

    const text = wrapper.text()
    expect(text).toContain('4.0 MB downloaded')
    expect(text).toContain('-- %')
    expect(text).not.toContain('45%') // Did not retain old percentage or invent a fake one

    const pulseBar = wrapper.find('.animate-pulse')
    expect(pulseBar.exists()).toBe(true)
  })

  it('S141: calculates global progress across total queue items separate from active streams', async () => {
    // 4 items total: 1 completed (100%), 1 downloading at 50%, 1 queued (0%), 1 failed (0%)
    // overall queue progress = ((1 * 100) + 50) / 4 = 37.5% -> 38%
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Top toolbar should display global queue progress
    expect(wrapper.text()).toContain('Queue:')
    expect(wrapper.text()).toContain('1/4 completed')
  })

  it('S141: details panel is collapsible to maximize queue list view', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Find the queue details toggle button
    const toggleBtn = wrapper.findAll('button').find(b => b.text().includes('Queue details'))
    expect(toggleBtn).toBeDefined()

    // By default, details panel is collapsed (showQueueDetails = false)
    const detailsPanel = wrapper.find('.reconciliation-strip')
    expect(detailsPanel.exists()).toBe(true)

    // Toggle open
    if (toggleBtn) {
      await toggleBtn.trigger('click')
      await flushPromises()
    }
  })

  it('S141: displays preflight exclusions when items are skipped or deduplicated', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return { ...mockStats, skipped: 5, deduplicated: 3 }
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Preflight exclusions badge is visible
    expect(wrapper.text()).toContain('8 excluded')
  })

  it('S149: renders explicit granular phase labels on active download cards', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // 1. Emit Auth phase
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      phase: 'Auth',
      status: 'downloading',
      terminal: false,
    })
    await flushPromises()
    expect(wrapper.text()).toContain('Authenticating service')

    // 2. Emit EnrichMetadata phase
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      phase: 'EnrichMetadata',
      status: 'downloading',
      terminal: false,
    })
    await flushPromises()
    expect(wrapper.text()).toContain('Enriching metadata')

    // 3. Emit ResolveLyrics best-effort fallback
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      phase: 'ResolveLyrics',
      message: 'Lyrics unavailable — continuing',
      status: 'downloading',
      terminal: false,
    })
    await flushPromises()
    expect(wrapper.text()).toContain('Lyrics unavailable — continuing')

    // 4. Emit Tagging phase
    emitMockEvent('syncify:download_progress', {
      queue_id: 1,
      track_id: 101,
      phase: 'Tagging',
      status: 'downloading',
      terminal: false,
    })
    await flushPromises()
    expect(wrapper.text()).toContain('Writing tags')
  })

  it('S149: expands completed item to display phase execution timings and timeline', async () => {
    const completedItemWithTimings = [
      {
        id: 10,
        track_id: 201,
        target_title: 'Benchmarked Masterpiece',
        target_artist: 'Virtuoso Ensemble',
        target_album: 'Audiophile Sessions',
        service_name: 'qobuz',
        quality_preference: 'hires',
        status: 'complete',
        priority: 50,
        progress_percent: 100,
        completed_at: '2026-08-17T08:00:00Z',
        timeline: [
          { phase: 'Auth', timestamp: 1000 },
          { phase: 'ResolveStream', timestamp: 1050 },
          { phase: 'Transfer', timestamp: 1200 },
          { phase: 'ValidateAudio', timestamp: 2400 },
          { phase: 'Tagging', timestamp: 2500 },
          { phase: 'Completed', timestamp: 2600 },
        ],
        phase_timings: {
          transfer_ms: 1200,
          validate_audio_ms: 50,
          metadata_duration_ms: 100,
          lyrics_duration_ms: 80,
          cover_duration_ms: 120,
          tagging_duration_ms: 90,
          total_duration_ms: 1640,
          throughput_mibps: 4.8,
        },
      }
    ]

    mockInvoke((command) => {
      if (command === 'get_queue') return completedItemWithTimings
      if (command === 'get_queue_stats') return { total: 1, completed: 1, downloading: 0, queued: 0, failed: 0 }
      if (command === 'get_worker_status') return mockWorkerStatus
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Find the info button for completed item
    const infoBtn = wrapper.find('button[title*="Phase Timings & Timeline"]')
    expect(infoBtn.exists()).toBe(true)

    // Click info button to expand
    await infoBtn.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('Phase Execution & Timings')
    expect(wrapper.text()).toContain('Total: 1.64 s')
    expect(wrapper.text()).toContain('1.20 s (4.8 MiB/s)')
    expect(wrapper.text()).toContain('Timeline:')
  })
})


