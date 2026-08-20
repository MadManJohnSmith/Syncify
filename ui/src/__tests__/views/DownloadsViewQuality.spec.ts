/**
 * DownloadsViewQuality.spec.ts
 * 
 * Comprehensive tests for Quality Policy decision rendering, physical quality representation,
 * accessibility text, and provider/quality fallback transparency.
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import DownloadsView from '@/views/DownloadsView.vue'
import LibraryView from '@/views/LibraryView.vue'
import { mockInvoke, resetMocks } from '../setup'

const mockQualityQueueItems = [
  // 1. Completed with Quality Fallback: Tidal returned AAC 320 kbps when Hi-Res Lossless was requested
  {
    id: 1,
    track_id: 101,
    target_title: 'Tidal AAC Fallback Track',
    target_artist: 'Artist One',
    target_album: 'Album One',
    service_name: 'tidal',
    service: 'tidal',
    quality_preference: 'HI_RES_LOSSLESS',
    requested_quality: 'HI_RES_LOSSLESS',
    effective_quality: '320kbps',
    requested_format: 'FLAC',
    effective_format: 'AAC',
    quality_decision: 'CompletedWithQualityFallback',
    provider_fallback_used: 0,
    quality_fallback_used: 1,
    decision_reason: 'Provider returned AAC; lossy fallback is enabled',
    status: 'complete',
    priority: 50,
    progress_percent: 100,
    error_message: null,
    created_at: '2026-08-20T10:00:00Z',
    started_at: '2026-08-20T10:01:00Z',
    completed_at: '2026-08-20T10:02:00Z',
  },
  // 2. Completed with Provider Fallback: Spotify requested -> Qobuz provided FLAC 16-bit / 44.1 kHz
  {
    id: 2,
    track_id: 102,
    target_title: 'Spotify to Qobuz Provider Fallback Track',
    target_artist: 'Artist Two',
    target_album: 'Album Two',
    service_name: 'spotify',
    original_service: 'spotify',
    effective_service: 'qobuz',
    quality_preference: 'LOSSLESS',
    requested_quality: 'LOSSLESS',
    effective_quality: 'FLAC 16-bit / 44.1 kHz',
    requested_format: 'FLAC',
    effective_format: 'FLAC',
    quality_decision: 'CompletedWithProviderFallback',
    provider_fallback_used: 1,
    quality_fallback_used: 0,
    status: 'complete',
    priority: 50,
    progress_percent: 100,
    error_message: null,
    created_at: '2026-08-20T10:00:00Z',
    started_at: '2026-08-20T10:01:00Z',
    completed_at: '2026-08-20T10:02:00Z',
  },
  // 3. Rejected Quality: Tidal returned AAC 320 kbps when Lossless was requested under strict policy -> No file saved
  {
    id: 3,
    track_id: 103,
    target_title: 'Strict Quality Rejection Track',
    target_artist: 'Artist Three',
    target_album: 'Album Three',
    service_name: 'tidal',
    service: 'tidal',
    quality_preference: 'LOSSLESS',
    requested_quality: 'LOSSLESS',
    effective_quality: '320',
    requested_format: 'FLAC',
    effective_format: 'AAC',
    quality_decision: 'RejectedQuality',
    provider_fallback_used: 0,
    quality_fallback_used: 0,
    decision_reason: 'Provider returned AAC; lossy fallback is disabled',
    status: 'failed',
    priority: 50,
    progress_percent: 0,
    error_message: 'Quality rejection: requested_lossless_but_received_aac',
    last_error: 'Quality rejection: requested_lossless_but_received_aac',
    created_at: '2026-08-20T10:00:00Z',
    started_at: '2026-08-20T10:01:00Z',
    completed_at: null,
  }
]

const mockStats = {
  total: 3,
  queued: 0,
  downloading: 0,
  completed: 2,
  failed: 1,
  paused: 0,
}

const mockWorkerStatus = {
  running: true,
  paused: false,
  active_downloads: 0,
  max_concurrent: 3,
  is_running: true,
  is_paused: false,
}

describe('DownloadsView Quality Decision & Policy Transparency', () => {
  beforeEach(() => {
    resetMocks()
  })

  it('renders completed item with quality fallback decision details', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQualityQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      if (command === 'audit_download_queue') return {
        total_items: 3,
        ready_count: 0,
        source_locked_count: 3,
        legacy_unresolved_count: 0,
        stale_source_count: 0,
        ambiguous_source_count: 0,
        source_identity_missing_count: 0,
        completed_count: 2,
        failed_count: 1,
        downloading_count: 0,
      }
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Expand details for track 1 (id: 1)
    const infoButtons = wrapper.findAll('.completed-section button[title*="Phase Timings"]')
    expect(infoButtons.length).toBeGreaterThan(0)
    await infoButtons[0].trigger('click')
    await flushPromises()

    const text = wrapper.text()
    // 1. Requested quality
    expect(text).toContain('Requested:')
    expect(text).toContain('Hi-Res Lossless')

    // 2. Provider
    expect(text).toContain('Provider:')
    expect(text).toContain('Tidal')

    // 3. Received quality
    expect(text).toContain('Received:')
    expect(text).toContain('AAC 320 kbps')

    // 4. Result: Completed with quality fallback
    expect(text).toContain('Completed with quality fallback')

    // 5. Reason
    expect(text).toContain('Reason:')
    expect(text).toContain('Provider returned AAC; lossy fallback is enabled')

    // 6. Accessible aria-label present
    const decisionBox = wrapper.find('.quality-decision-box')
    expect(decisionBox.exists()).toBe(true)
    expect(decisionBox.attributes('aria-label')).toContain('Quality decision for Tidal AAC Fallback Track')
  })

  it('renders completed item with provider fallback without quality loss', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQualityQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      if (command === 'audit_download_queue') return { total_items: 3 }
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    // Expand details for track 2 (id: 2)
    const infoButtons = wrapper.findAll('.completed-section button[title*="Phase Timings"]')
    expect(infoButtons.length).toBeGreaterThan(1)
    await infoButtons[1].trigger('click')
    await flushPromises()

    const text = wrapper.text()
    // 1. Requested quality
    expect(text).toContain('Requested:')
    expect(text).toContain('Lossless')

    // 2. Original source
    expect(text).toContain('Original source:')
    expect(text).toContain('Spotify')

    // 3. Provider selected
    expect(text).toContain('Provider selected:')
    expect(text).toContain('Qobuz')

    // 4. Result: Completed with provider fallback
    expect(text).toContain('Completed with provider fallback')

    // 5. Physical Quality specification
    expect(text).toContain('Quality:')
    expect(text).toContain('FLAC 16-bit / 44.1 kHz')
  })

  it('renders rejected quality failure card with "No file was saved"', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') return mockQualityQueueItems
      if (command === 'get_queue_stats') return mockStats
      if (command === 'get_worker_status') return mockWorkerStatus
      if (command === 'audit_download_queue') return { total_items: 3 }
      return []
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    const text = wrapper.text()
    // 1. Track title
    expect(text).toContain('Strict Quality Rejection Track')

    // 2. Requested: Lossless
    expect(text).toContain('Requested:')
    expect(text).toContain('Lossless')

    // 3. Provider: Tidal
    expect(text).toContain('Provider:')
    expect(text).toContain('Tidal')

    // 4. Received: AAC 320 kbps
    expect(text).toContain('Received:')
    expect(text).toContain('AAC 320 kbps')

    // 5. Result: Rejected quality
    expect(text).toContain('Rejected quality')

    // 6. Explicit text: No file was saved
    expect(text).toContain('No file was saved')
  })

  it('renders LibraryView physical quality badges (Hi-Res gold, Lossless silver, AAC amber)', async () => {
    const mockLibraryTracks = [
      {
        id: 201,
        title: 'Hi-Res Audio Master',
        artist_name: 'Studio Artist',
        album_name: 'Studio Master Album',
        quality: '24/96',
        download_status: 'downloaded',
        downloaded_from: 'qobuz',
        metadata_score: 95,
        duration_ms: 240000,
        services: 'qobuz',
      },
      {
        id: 202,
        title: 'CD Quality Lossless',
        artist_name: 'CD Artist',
        album_name: 'CD Album',
        quality: '16/44.1',
        download_status: 'downloaded',
        downloaded_from: 'tidal',
        metadata_score: 90,
        duration_ms: 210000,
        services: 'tidal',
      },
      {
        id: 203,
        title: 'AAC Fallback Audio',
        artist_name: 'AAC Artist',
        album_name: 'AAC Album',
        quality: 'AAC 320',
        download_status: 'downloaded',
        downloaded_from: 'tidal',
        metadata_score: 85,
        duration_ms: 195000,
        services: 'tidal',
      },
    ]

    mockInvoke((command) => {
      if (command === 'get_library') {
        return {
          tracks: mockLibraryTracks,
          total: 3,
          offset: 0,
          limit: 50,
          has_more: false,
        }
      }
      return []
    })

    const wrapper = mount(LibraryView)
    await flushPromises()

    const text = wrapper.text()
    expect(text).toContain('Hi-Res Audio Master')
    expect(text).toContain('24/96')
    expect(text).toContain('CD Quality Lossless')
    expect(text).toContain('16/44.1')
    expect(text).toContain('AAC Fallback Audio')
    expect(text).toContain('AAC 320')

    // Verify CSS classes assigned by getQualityStyle
    const qualitySpans = wrapper.findAll('.track-cell span.rounded-full')
    const goldSpan = qualitySpans.find(s => s.text().includes('24/96'))
    expect(goldSpan?.classes()).toContain('text-quality-gold')

    const silverSpan = qualitySpans.find(s => s.text().includes('16/44.1'))
    expect(silverSpan?.classes()).toContain('text-quality-silver')

    const amberSpan = qualitySpans.find(s => s.text().includes('AAC 320'))
    expect(amberSpan?.classes()).toContain('text-amber-500')
  })
})
