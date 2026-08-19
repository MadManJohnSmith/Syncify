/**
 * Unit tests for downloadPhase.ts (S149)
 * Tests 14-phase UI formatting, best-effort message extraction, speed formatting, and error classifications
 */

import { describe, it, expect } from 'vitest'
import { formatDownloadPhase, formatSpeed, formatDurationMs } from '../../utils/downloadPhase'

describe('downloadPhase.ts UI contract', () => {
  it('formats all standard 14 phases correctly', () => {
    expect(formatDownloadPhase('Queued')).toBe('Queued')
    expect(formatDownloadPhase('QueueWait')).toBe('Waiting for slot')
    expect(formatDownloadPhase('Auth')).toBe('Authenticating service')
    expect(formatDownloadPhase('ResolveStream')).toBe('Resolving stream')
    expect(formatDownloadPhase('searching')).toBe('Resolving stream')
    expect(formatDownloadPhase('Transfer')).toBe('Downloading')
    expect(formatDownloadPhase('downloading')).toBe('Downloading')
    expect(formatDownloadPhase('ValidateAudio')).toBe('Validating audio')
    expect(formatDownloadPhase('EnrichMetadata')).toBe('Enriching metadata')
    expect(formatDownloadPhase('ResolveLyrics')).toBe('Searching synchronized lyrics')
    expect(formatDownloadPhase('ResolveCover')).toBe('Downloading animated cover')
    expect(formatDownloadPhase('Tagging')).toBe('Writing tags')
    expect(formatDownloadPhase('Promotion')).toBe('Moving to library')
    expect(formatDownloadPhase('Persisting')).toBe('Persisting record')
    expect(formatDownloadPhase('Completed')).toBe('Completed')
    expect(formatDownloadPhase('Cancelled')).toBe('Cancelled')
  })

  it('formats Transfer phase with percent and throughput speed', () => {
    const formatted = formatDownloadPhase('Transfer', {
      percent: 42,
      instantKbps: 4.8 * 1024,
    })
    expect(formatted).toBe('Downloading 42% — 4.8 MiB/s')

    const formattedKb = formatDownloadPhase('Transfer', {
      percent: 85,
      instantKbps: 512,
    })
    expect(formattedKb).toBe('Downloading 85% — 512 KiB/s')

    const formattedNoSpeed = formatDownloadPhase('Transfer', {
      percent: 15,
    })
    expect(formattedNoSpeed).toBe('Downloading 15%')
  })

  it('formats best-effort non-fatal lyrics and cover events', () => {
    const lyricsFallback = formatDownloadPhase('ResolveLyrics', {
      message: 'Lyrics unavailable — continuing',
    })
    expect(lyricsFallback).toBe('Lyrics unavailable — continuing')

    const coverFallback = formatDownloadPhase('ResolveCover', {
      message: 'Animated cover unavailable — continuing',
    })
    expect(coverFallback).toBe('Animated cover unavailable — continuing')
  })

  it('formats classified failure reasons in Failed status', () => {
    // Auth error
    expect(formatDownloadPhase('Failed', { errorMessage: 'HTTP 401 Unauthorized token expired' }))
      .toBe('Failed: Requires authentication')

    // Entitlement error
    expect(formatDownloadPhase('Failed', { errorMessage: 'Track unavailable on current subscription tier' }))
      .toBe('Failed: Entitlement restricted')

    // Stale source / 404
    expect(formatDownloadPhase('Failed', { errorMessage: 'StaleSource: track 404 stream missing' }))
      .toBe('Failed: Stale source / 404')

    // Quality rejected
    expect(formatDownloadPhase('Failed', { errorMessage: 'RejectedQuality: FLAC 24-bit unavailable' }))
      .toBe('Failed: Rejected quality')

    // Audio validation error
    expect(formatDownloadPhase('Failed', { errorMessage: 'Audio validation failed: invalid flac header' }))
      .toBe('Failed: Audio validation failed')

    // Tagging error
    expect(formatDownloadPhase('Failed', { errorMessage: 'Tagging error: failed to write id3 metadata' }))
      .toBe('Failed: Tagging failed')

    // Filesystem error
    expect(formatDownloadPhase('Failed', { errorMessage: 'Filesystem error: disk full permission denied' }))
      .toBe('Failed: Filesystem error')

    // Network error
    expect(formatDownloadPhase('Failed', { errorMessage: 'Network retry exhausted: connection reset by peer' }))
      .toBe('Failed: Network retry exhausted')
  })

  it('formats duration helper correctly', () => {
    expect(formatDurationMs(45)).toBe('45 ms')
    expect(formatDurationMs(1500)).toBe('1.50 s')
    expect(formatDurationMs(null)).toBe('--')
  })

  it('formats speed helper correctly', () => {
    expect(formatSpeed(500)).toBe('500 KiB/s')
    expect(formatSpeed(2048)).toBe('2.0 MiB/s')
    expect(formatSpeed(0)).toBe('')
  })
})
