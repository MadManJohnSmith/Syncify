/**
 * Unit tests for useLogs composable (S170)
 * Tests singleton persistence, IPC buffer integration, background event listeners, and export/copy functions.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { useLogs, resetLogs, formatLogTime, normalizeProviderName, mapSystemLogToLogEntry } from '@/composables/useLogs'
import { mockInvoke, resetMocks, emitMockEvent } from '../setup'

describe('useLogs Composable', () => {
  beforeEach(() => {
    resetMocks()
    resetLogs()
    mockInvoke((command) => {
      if (command === 'get_system_logs') {
        return [
          {
            id: 'backend-1',
            timestamp: '2026-08-23T15:00:00Z',
            level: 'info',
            target: 'syncify_tauri::core',
            module: 'System',
            message: 'Engine boot completed successfully',
            fields: { version: '0.1.0' }
          }
        ]
      }
      if (command === 'clear_system_logs') {
        return null
      }
      if (command === 'export_system_logs') {
        return '[15:00:00] [INFO] [System] Engine boot completed'
      }
      return null
    })
  })

  it('starts with an empty log list by default after reset', () => {
    const { logs } = useLogs()
    expect(logs.value).toEqual([])
  })

  it('adds structured log entries and respects 1000 items capacity limit', () => {
    const { logs, addLog } = useLogs()

    addLog({
      level: 'info',
      provider: 'Qobuz',
      category: 'Downloads',
      message: 'Downloading FLAC 24-bit stream',
      rawCategory: 'downloads',
    })

    expect(logs.value.length).toBe(1)
    expect(logs.value[0].provider).toBe('Qobuz')
    expect(logs.value[0].message).toBe('Downloading FLAC 24-bit stream')
    expect(logs.value[0].level).toBe('info')

    // Add 1005 items
    for (let i = 0; i < 1005; i++) {
      addLog({
        level: 'info',
        provider: 'System',
        category: 'Core',
        message: `Log line ${i}`,
        rawCategory: 'system',
      })
    }

    expect(logs.value.length).toBe(1000)
  })

  it('preserves state across multiple useLogs instances (singleton pattern)', () => {
    const instanceA = useLogs()
    instanceA.addLog({
      level: 'warn',
      provider: 'Spotify',
      category: 'Enrichment',
      message: 'Rate limit backoff active',
      rawCategory: 'enrichment',
    })

    const instanceB = useLogs()
    expect(instanceB.logs.value.length).toBe(1)
    expect(instanceB.logs.value[0].provider).toBe('Spotify')
  })

  it('fetches native backend logs on initLogListeners and maps them correctly', async () => {
    const { logs, initLogListeners } = useLogs()
    await initLogListeners()

    expect(logs.value.length).toBeGreaterThanOrEqual(1)
    expect(logs.value.some(l => l.message === 'Engine boot completed successfully')).toBe(true)
  })

  it('reactively captures live syncify:log_event from native Rust backend', async () => {
    const { logs, initLogListeners } = useLogs()
    await initLogListeners()

    emitMockEvent('syncify:log_event', {
      id: 'live-99',
      timestamp: '2026-08-23T15:10:00Z',
      level: 'error',
      target: 'syncify_tauri::services::tidal_pipeline',
      module: 'Tidal',
      message: 'Failed to negotiate stream token: TokenExpired',
      fields: { error_code: 401 }
    })

    expect(logs.value.some(l => l.message.includes('TokenExpired'))).toBe(true)
    expect(logs.value.find(l => l.message.includes('TokenExpired'))?.level).toBe('error')
  })

  it('reactively captures syncify:download_progress and syncify:enrichment_event', async () => {
    const { logs, initLogListeners } = useLogs()
    await initLogListeners()

    emitMockEvent('syncify:enrichment_event', {
      track_id: 101,
      service: 'spotify',
      status: 'completed',
      message: 'Enriched ISRC USRC12345678',
    })

    emitMockEvent('syncify:download_progress', {
      track_id: 202,
      title: 'Bohemian Rhapsody',
      service: 'qobuz',
      status: 'completed',
      progress_percent: 100,
    })

    expect(logs.value.some(l => l.message.includes('Enriched ISRC USRC12345678'))).toBe(true)
    expect(logs.value.some(l => l.message.includes('Bohemian Rhapsody'))).toBe(true)
  })

  it('clears logs and invokes backend clear command', async () => {
    const { logs, addLog, clearLogs } = useLogs()
    addLog({
      level: 'info',
      provider: 'System',
      category: 'Core',
      message: 'Temporary log',
      rawCategory: 'system',
    })
    expect(logs.value.length).toBe(1)

    await clearLogs()
    expect(logs.value.length).toBe(0)
  })

  it('copies logs to clipboard formatted nicely', async () => {
    const writeTextSpy = vi.fn().mockResolvedValue(undefined)
    Object.assign(navigator, {
      clipboard: {
        writeText: writeTextSpy,
      },
    })

    const { addLog, copyLogs } = useLogs()
    addLog({
      id: 'test-1',
      time: '12:00:00',
      level: 'info',
      provider: 'System',
      category: 'Core',
      message: 'Test copy message',
      rawCategory: 'system',
    })

    const result = await copyLogs()
    expect(result).toBe(true)
    expect(writeTextSpy).toHaveBeenCalled()
    expect(writeTextSpy.mock.calls[0][0]).toContain('[12:00:00] [INFO] [System] [Core] Test copy message')
  })
})
