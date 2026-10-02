/**
 * Regression tests for FE-7: the status bar must derive every value from
 * real application state (sync tasks, service statuses, download progress
 * events) instead of hardcoded telemetry like 'Wi-Fi', '12.3 MB/s',
 * 'Calculating...', 'Just now' or the invented 'Connection timeout'.
 */

import { describe, it, expect } from 'vitest'
import {
  deriveSyncState,
  deriveSyncErrorMessage,
  estimateRemainingMs,
  formatDownloadSpeed,
  formatRelativeTime,
  formatRemainingDuration,
  isTerminalDownloadStatus,
  parseSqliteUtc,
  type SyncTaskLike,
} from '../statusTelemetry'

const NOW = Date.UTC(2026, 9, 1, 12, 0, 0) // 2026-10-01T12:00:00Z

function task(overrides: Partial<SyncTaskLike>): SyncTaskLike {
  return { type: 'sync', status: 'running', ...overrides }
}

describe('deriveSyncState', () => {
  it('reports idle with no tasks (real state, not a fake constant)', () => {
    expect(deriveSyncState([])).toBe('idle')
  })

  it('reports syncing while any task is running', () => {
    expect(deriveSyncState([task({ status: 'running' })])).toBe('syncing')
    expect(deriveSyncState([task({ type: 'download', status: 'running' })])).toBe('syncing')
  })

  it('reports error from a real failed sync task', () => {
    const tasks = [task({ status: 'failed' }), task({ status: 'failed', type: 'download' })]
    expect(deriveSyncState(tasks)).toBe('error')
  })

  it('reports error for requires_auth sync failures', () => {
    expect(deriveSyncState([task({ status: 'requires_auth' })])).toBe('error')
  })

  it('reports paused when only paused tasks remain', () => {
    expect(deriveSyncState([task({ status: 'paused' })])).toBe('paused')
  })

  it('keeps syncing while other tasks run even if a sync already failed', () => {
    expect(deriveSyncState([task({ status: 'failed' }), task({ status: 'running' })])).toBe('syncing')
  })

  it('ignores completed tasks', () => {
    expect(deriveSyncState([task({ status: 'completed' })])).toBe('idle')
  })
})

describe('deriveSyncErrorMessage', () => {
  it('returns null when no sync task failed (error branch unreachable)', () => {
    expect(deriveSyncErrorMessage([task({ status: 'running' })])).toBeNull()
  })

  it('returns the real error message of the latest failed sync', () => {
    const tasks: SyncTaskLike[] = [
      task({ status: 'failed', error: 'Old failure', startedAt: 1 }),
      task({ status: 'failed', error: 'Qobuz session expired', startedAt: 2 }),
    ]
    expect(deriveSyncErrorMessage(tasks)).toBe('Qobuz session expired')
  })

  it('falls back to a generic message when the task carries no error text', () => {
    expect(deriveSyncErrorMessage([task({ status: 'failed' })])).toBe('Sync failed')
  })
})

describe('formatDownloadSpeed', () => {
  it('shows an em dash when there is no active download', () => {
    expect(formatDownloadSpeed(null)).toBe('—')
    expect(formatDownloadSpeed(0)).toBe('—')
  })

  it('formats KB/s values from download progress events', () => {
    expect(formatDownloadSpeed(512.4)).toBe('512 KB/s')
  })

  it('formats MB/s values above 1024 KB/s', () => {
    expect(formatDownloadSpeed(2048)).toBe('2.0 MB/s')
  })

  it('never returns the old hardcoded fake value', () => {
    for (const v of [null, -5, 1, 999, 5000]) {
      expect(formatDownloadSpeed(v)).not.toBe('12.3 MB/s')
    }
  })
})

describe('parseSqliteUtc + formatRelativeTime', () => {
  it('parses SQLite CURRENT_TIMESTAMP values as UTC', () => {
    const parsed = parseSqliteUtc('2026-10-01 11:55:00')
    expect(parsed).not.toBeNull()
    expect(parsed!.getTime()).toBe(Date.UTC(2026, 9, 1, 11, 55, 0))
  })

  it('accepts ISO timestamps and rejects garbage', () => {
    expect(parseSqliteUtc('2026-10-01T11:55:00Z')!.getTime()).toBe(Date.UTC(2026, 9, 1, 11, 55, 0))
    expect(parseSqliteUtc('not-a-date')).toBeNull()
    expect(parseSqliteUtc('')).toBeNull()
  })

  it('formats relative times from the real last_synced value', () => {
    expect(formatRelativeTime(new Date(NOW - 30_000), NOW)).toBe('Just now')
    expect(formatRelativeTime(new Date(NOW - 5 * 60_000), NOW)).toBe('5 min ago')
    expect(formatRelativeTime(new Date(NOW - 2 * 3_600_000), NOW)).toBe('2 hours ago')
    expect(formatRelativeTime(new Date(NOW - 26 * 3_600_000), NOW)).toBe('Yesterday')
  })
})

describe('estimateRemainingMs (sync ETA)', () => {
  it('extrapolates from elapsed time and progress', () => {
    const startedAt = NOW - 100_000
    // 50% done after 100s → ~100s remaining
    expect(estimateRemainingMs(startedAt, 50, NOW)).toBeCloseTo(100_000, 0)
  })

  it('returns null when progress gives no basis for an estimate', () => {
    const startedAt = NOW - 100_000
    expect(estimateRemainingMs(startedAt, 0, NOW)).toBeNull()
    expect(estimateRemainingMs(startedAt, 100, NOW)).toBeNull()
    expect(estimateRemainingMs(NOW, 50, NOW)).toBeNull()
  })
})

describe('formatRemainingDuration', () => {
  it('formats seconds, minutes and hours', () => {
    expect(formatRemainingDuration(45_000)).toBe('45s')
    expect(formatRemainingDuration(125_000)).toBe('2m 5s')
    expect(formatRemainingDuration(3_600_000)).toBe('1h')
    expect(formatRemainingDuration(3_660_000)).toBe('1h 1m')
  })
})

describe('isTerminalDownloadStatus', () => {
  it('detects terminal statuses so the speed readout resets', () => {
    expect(isTerminalDownloadStatus('complete')).toBe(true)
    expect(isTerminalDownloadStatus('failed')).toBe(true)
    expect(isTerminalDownloadStatus('downloading')).toBe(false)
    expect(isTerminalDownloadStatus('')).toBe(false)
  })
})
