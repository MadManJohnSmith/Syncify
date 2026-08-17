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
      if (command === 'get_queue') return sampleQueue
      if (command === 'get_queue_stats') return sampleStats
      if (command === 'get_worker_status') return sampleWorker
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
})
