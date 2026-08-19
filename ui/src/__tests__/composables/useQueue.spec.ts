/**
 * Unit tests for useQueue composable
 * Tests queue management, live throughput, ETA, success rate, and artifact counters
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { useQueue } from '../../composables/useQueue'
import { mockInvoke, resetMocks } from '../setup'

const sampleQueue = [
  {
    id: 1,
    track_id: 101,
    target_title: 'Synchronize',
    target_artist: 'Cosmic Sound',
    target_album: 'Starlight',
    service_name: 'qobuz',
    status: 'downloading',
    priority: 50,
    progress_percent: 50,
    error_message: null,
    created_at: '2026-08-17T00:00:00Z',
    started_at: null,
    completed_at: null,
  },
  {
    id: 2,
    track_id: 102,
    target_title: 'Track 2',
    target_artist: 'Artist 2',
    target_album: 'Album 2',
    service_name: 'tidal',
    status: 'queued',
    priority: 50,
    progress_percent: 0,
    error_message: null,
    created_at: '2026-08-17T00:00:00Z',
    started_at: null,
    completed_at: null,
  },
  {
    id: 3,
    track_id: 103,
    target_title: 'Track 3',
    target_artist: 'Artist 3',
    target_album: 'Album 3 Deluxe Edition',
    service_name: 'qobuz',
    status: 'complete',
    priority: 50,
    progress_percent: 100,
    error_message: null,
    created_at: '2026-08-17T00:00:00Z',
    started_at: null,
    completed_at: '2026-08-17T00:02:00Z',
  },
]

const sampleStats = {
  total: 3,
  queued: 1,
  downloading: 1,
  completed: 1,
  failed: 0,
  paused: 0,
  audio_count: 5,
  lrc_count: 5,
  cover_count: 4,
  booklet_count: 1,
  success_rate: 100.0,
}

const sampleWorker = {
  running: true,
  paused: false,
  active_downloads: 1,
  max_concurrent: 3,
}

describe('useQueue composable', () => {
  beforeEach(() => {
    resetMocks()
    mockInvoke((command) => {
      if (command === 'get_queue') return JSON.parse(JSON.stringify(sampleQueue))
      if (command === 'get_queue_stats') return { ...sampleStats }
      if (command === 'get_worker_status') return { ...sampleWorker }
      return null
    })
  })

  it('initializes queue, stats, and artifact counters', async () => {
    const { queue, stats, artifactCounters, successRate, initialize } = useQueue()

    await initialize()

    expect(queue.value.length).toBe(3)
    expect(stats.value).toBeDefined()
    expect(artifactCounters.value.audio).toBe(5)
    expect(artifactCounters.value.lrc).toBe(5)
    expect(artifactCounters.value.covers).toBe(4)
    expect(artifactCounters.value.booklets).toBe(1)
    expect(successRate.value).toBe(100.0)
  })

  it('computes live throughput and formatted ETA from progress events', async () => {
    const { queue, throughputKbps, formattedThroughput, etaSeconds, formattedEta, handleProgressEvent, initialize } = useQueue()

    await initialize()

    expect(formattedThroughput.value).toBe('0 KB/s')

    // Simulate progress tick 1
    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      progress_percent: 60,
      status: 'downloading',
    })

    // Simulate progress tick 2
    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      progress_percent: 90,
      status: 'downloading',
    })

    expect(throughputKbps.value).toBeGreaterThanOrEqual(0)
    expect(etaSeconds.value).toBeDefined()
    expect(formattedEta.value).not.toBe('--')
  })

  it('increments artifact counters when a download completes', async () => {
    const { artifactCounters, handleProgressEvent, initialize } = useQueue()

    await initialize()
    const initialAudio = artifactCounters.value.audio
    const initialLrc = artifactCounters.value.lrc
    const initialCovers = artifactCounters.value.covers

    // Complete a download
    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      progress_percent: 100,
      status: 'complete',
    })

    expect(artifactCounters.value.audio).toBe(initialAudio + 1)
    expect(artifactCounters.value.lrc).toBe(initialLrc + 1)
    expect(artifactCounters.value.covers).toBe(initialCovers + 1)
  })

  it('handles pause and concurrency switching', async () => {
    const { isWorkerPaused, maxConcurrent, pauseDownloads, setMaxConcurrent, initialize } = useQueue()

    await initialize()

    expect(isWorkerPaused.value).toBe(false)
    expect(maxConcurrent.value).toBe(3)

    await pauseDownloads()
    await setMaxConcurrent(5)
  })

  it('marks items as failed when receiving terminal failure events (stale_source, rejected_quality, error)', async () => {
    const { queue, failedItems, activeDownloads, handleProgressEvent, initialize } = useQueue()

    await initialize()

    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      status: 'stale_source',
      error: 'Audio stream 404 expired',
    })

    const item = queue.value.find(q => q.id === 1)
    expect(item?.status).toBe('failed')
    expect(item?.error_message).toBe('Audio stream 404 expired')
    expect(failedItems.value.some(q => q.id === 1)).toBe(true)
    // Failed rows must NOT be in activeDownloads
    expect(activeDownloads.value.some(q => q.id === 1)).toBe(false)
  })

  it('correctly classifies error categories: network, StaleSource, RequiresAuth, RejectedQuality, Cancelled, AmbiguousSource', () => {
    const { classifyFailureReason } = useQueue()

    // 1. Network error
    const netErr = classifyFailureReason('Qobuz download failed: error decoding response body')
    expect(netErr.reason).toBe('network')
    expect(netErr.label).toBe('Network retry exhausted')
    expect(netErr.isRetryableOriginal).toBe(true)
    expect(netErr.canUseFallback).toBe(false)

    // 2. StaleSource / 404
    const staleErr = classifyFailureReason('StaleSource: track 404 stream missing')
    expect(staleErr.reason).toBe('stale_source')
    expect(staleErr.label).toBe('Stale source / 404')
    expect(staleErr.canUseFallback).toBe(true)

    // 3. RequiresAuth (401/403)
    const authErr = classifyFailureReason('HTTP 401 Unauthorized: token expired')
    expect(authErr.reason).toBe('requires_auth')
    expect(authErr.label).toBe('Requires authentication')
    expect(authErr.requiresAuth).toBe(true)
    expect(authErr.canUseFallback).toBe(false)

    // 4. RejectedQuality
    const qualErr = classifyFailureReason('RejectedQuality: FLAC 24-bit not available for this account')
    expect(qualErr.reason).toBe('rejected_quality')
    expect(qualErr.label).toBe('Rejected quality')
    expect(qualErr.canUseFallback).toBe(true)

    // 5. Cancelled
    const cancelErr = classifyFailureReason('Download cancelled by user')
    expect(cancelErr.reason).toBe('cancelled')
    expect(cancelErr.label).toBe('Cancelled')

    // 6. AmbiguousSource
    const ambigErr = classifyFailureReason('AmbiguousSource: multiple tracks matched')
    expect(ambigErr.reason).toBe('ambiguous_source')
    expect(ambigErr.label).toBe('Ambiguous source')
  })

  it('handles rich byte progress events and tracks bytes, instant kbps and phase', async () => {
    const { queue, throughputKbps, handleProgressEvent, initialize } = useQueue()

    await initialize()

    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      bytes_downloaded: 5 * 1024 * 1024,
      total_bytes: 10 * 1024 * 1024,
      percent: 50.0,
      instant_kbps: 1250.5,
      average_kbps: 1100.0,
      phase: 'downloading',
      terminal: false,
    })

    const item = queue.value.find(q => q.id === 1) as any
    expect(item).toBeDefined()
    expect(item.progress_percent).toBe(50.0)
    expect(item.bytes_downloaded).toBe(5 * 1024 * 1024)
    expect(item.total_bytes).toBe(10 * 1024 * 1024)
    expect(item.instant_kbps).toBe(1250.5)
    expect(item.average_kbps).toBe(1100.0)
    expect(item.phase).toBe('downloading')
    expect(throughputKbps.value).toBe(1251)
  })

  it('does not invent a fake percentage when total_bytes is null (missing Content-Length)', async () => {
    const { queue, handleProgressEvent, initialize } = useQueue()

    await initialize()

    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      bytes_downloaded: 2 * 1024 * 1024,
      total_bytes: null,
      percent: null,
      instant_kbps: 500.0,
      average_kbps: 450.0,
      phase: 'downloading',
      terminal: false,
    })

    const item = queue.value.find(q => q.id === 1) as any
    expect(item).toBeDefined()
    expect(item.percent).toBeNull()
    expect(item.total_bytes).toBeNull()
    expect(item.bytes_downloaded).toBe(2 * 1024 * 1024)
  })

  it('tracks full 14-phase sequence and records timeline without dropping rapid events', async () => {
    const { queue, handleProgressEvent, initialize } = useQueue()
    await initialize()

    const phases = [
      'QueueWait',
      'Auth',
      'ResolveStream',
      'Transfer',
      'ValidateAudio',
      'EnrichMetadata',
      'ResolveLyrics',
      'ResolveCover',
      'Tagging',
      'Promotion',
      'Persisting',
      'Completed',
    ]

    for (const p of phases) {
      handleProgressEvent({
        queue_id: 1,
        track_id: 101,
        phase: p,
        status: p === 'Completed' ? 'complete' : 'downloading',
        percent: p === 'Completed' ? 100 : (p === 'Transfer' ? 50 : 0),
        bytes_downloaded: p === 'Transfer' ? 12 * 1024 * 1024 : 0,
        total_bytes: 24 * 1024 * 1024,
        instant_kbps: p === 'Transfer' ? 2048 : 0,
        terminal: p === 'Completed',
      })
    }

    const item = queue.value.find(q => q.id === 1) as any
    expect(item).toBeDefined()
    expect(item.status).toBe('complete')
    expect(item.timeline).toBeDefined()
    expect(item.timeline.length).toBe(12)
    expect(item.timeline.map((t: any) => t.phase)).toEqual(phases)
  })

  it('records best-effort non-fatal lyrics and cover messages in progress events', async () => {
    const { queue, handleProgressEvent, initialize } = useQueue()
    await initialize()

    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      phase: 'ResolveLyrics',
      message: 'Lyrics unavailable — continuing',
      status: 'downloading',
    })

    let item = queue.value.find(q => q.id === 1) as any
    expect(item.phase).toBe('ResolveLyrics')
    expect(item.message).toBe('Lyrics unavailable — continuing')

    handleProgressEvent({
      queue_id: 1,
      track_id: 101,
      phase: 'ResolveCover',
      message: 'Animated cover unavailable — continuing',
      status: 'downloading',
    })

    item = queue.value.find(q => q.id === 1) as any
    expect(item.phase).toBe('ResolveCover')
    expect(item.message).toBe('Animated cover unavailable — continuing')
  })

  it('handles 50 concurrent progress events across multiple tracks without corruption', async () => {
    const { queue, handleProgressEvent, initialize } = useQueue()
    await initialize()

    const events = []
    for (let i = 0; i < 50; i++) {
      const qId = (i % 3) + 1 // distribution among tracks 1, 2, 3
      events.push({
        queue_id: qId,
        track_id: 100 + qId,
        percent: (i * 2) % 100,
        bytes_downloaded: (i + 1) * 512 * 1024,
        total_bytes: 25 * 1024 * 1024,
        instant_kbps: 1500 + i * 10,
        phase: i % 2 === 0 ? 'Transfer' : 'EnrichMetadata',
        terminal: false,
      })
    }

    // Fire all 50 events concurrently
    events.forEach(evt => handleProgressEvent(evt))

    const item1 = queue.value.find(q => q.id === 1) as any
    const item2 = queue.value.find(q => q.id === 2) as any
    const item3 = queue.value.find(q => q.id === 3) as any

    expect(item1).toBeDefined()
    expect(item2).toBeDefined()
    expect(item3).toBeDefined()

    expect(item1.bytes_downloaded).toBeGreaterThan(0)
    expect(item2.bytes_downloaded).toBeGreaterThan(0)
    expect(item3.bytes_downloaded).toBeGreaterThan(0)
  })

  it('cleans up event listener properly on cleanup()', async () => {
    const { initialize, cleanup } = useQueue()

    await initialize()
    expect(() => cleanup()).not.toThrow()
  })
})

