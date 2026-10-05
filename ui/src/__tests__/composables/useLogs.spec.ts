/**
 * Unit tests for useLogs composable (S170)
 * Tests singleton persistence, IPC buffer integration, background event listeners, and export/copy functions.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
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

  it('adds structured log entries and respects the 2000 items capacity limit (R15, aligned with the native ring buffer)', () => {
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

    // Add 2005 items
    for (let i = 0; i < 2005; i++) {
      addLog({
        level: 'info',
        provider: 'System',
        category: 'Core',
        message: `Log line ${i}`,
        rawCategory: 'system',
      })
    }

    expect(logs.value.length).toBe(2000)
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

  it('R15: keeps worker progress telemetry (queue_id + progress_percent) out of the audit history', async () => {
    const { logs, initLogListeners } = useLogs()
    await initLogListeners()

    // Telemetría de progreso del worker por queue_id + progress_percent:
    // cada tick NO es una entrada de log y no debe contaminar el histórico.
    for (let pct = 0; pct <= 100; pct += 25) {
      emitMockEvent('syncify:progress', {
        queue_id: 42,
        status: 'downloading',
        progress_percent: pct,
      })
    }
    // El mismo canal para el pipeline single-track (item_id + percent): fuera.
    emitMockEvent('syncify:progress', {
      item_id: 'abc',
      status: 'downloading',
      progress_percent: 50,
    })
    // Eventos de pipeline sin porcentaje (scan/organize): siguen entrando.
    emitMockEvent('syncify:progress', {
      operation: 'scan',
      status: 'completed',
      message: 'Scan finished',
    })

    expect(logs.value.some(l => l.details?.queue_id === 42)).toBe(false)
    expect(logs.value.some(l => l.details?.item_id === 'abc')).toBe(false)
    expect(logs.value.some(l => l.message.includes('Scan finished'))).toBe(true)
  })

  it('R15: loads the on-disk history pages with stable ids and dated times', async () => {
    mockInvoke((command) => {
      if (command === 'read_log_history') {
        return {
          entries: [
            {
              id: 'hist-abc',
              timestamp: '2026-10-04T09:00:00Z',
              level: 'warn',
              target: 'syncify_tauri::worker',
              module: 'Worker',
              message: 'Old warning from disk',
              fields: null,
            },
          ],
          total: 1,
          offset: 0,
          limit: 500,
        }
      }
      return null
    })

    const { logs, loadLogHistory } = useLogs()
    const page = await loadLogHistory({ from: '2026-10-04', to: '2026-10-05' })
    expect(page.total).toBe(1)
    expect(page.entries[0].message).toBe('Old warning from disk')
    expect(logs.value.some(l => l.id === 'hist-abc')).toBe(true)
    // La entrada no es de hoy: la columna de tiempo lleva la fecha (MM-DD).
    expect(logs.value.find(l => l.id === 'hist-abc')?.time).toMatch(/^\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/)

    // Recargar la misma página no duplica la entrada (id estable).
    await loadLogHistory({ from: '2026-10-04' })
    expect(logs.value.filter(l => l.id === 'hist-abc').length).toBe(1)
  })

  it('R15: exports the history via the native dialog (path only) and export_log_range', async () => {
    const exported: Array<{ command: string; args: Record<string, unknown> | undefined }> = []
    vi.mocked(saveDialog).mockResolvedValueOnce('/tmp/syncify-history.txt')
    mockInvoke((command, args) => {
      if (command === 'export_log_range') {
        exported.push({ command, args })
        return { path: '/tmp/syncify-history.txt', exportedCount: 3 }
      }
      return null
    })

    const { exportLogHistory } = useLogs()
    const ok = await exportLogHistory({ from: '2026-10-04', to: '2026-10-05', level: 'warn' })
    expect(ok).toBe(true)
    expect(exported).toHaveLength(1)
    expect(exported[0].args?.destPath).toBe('/tmp/syncify-history.txt')
    expect(exported[0].args?.level).toBe('warn')
    expect(exported[0].args?.from).toBe('2026-10-04')
  })

  it('R15: does not invoke export_log_range when the user cancels the save dialog', async () => {
    const exported: string[] = []
    vi.mocked(saveDialog).mockResolvedValueOnce(null)
    mockInvoke((command) => {
      if (command === 'export_log_range') exported.push(command)
      return null
    })

    const { exportLogHistory } = useLogs()
    const ok = await exportLogHistory({})
    expect(ok).toBe(false)
    expect(exported).toHaveLength(0)
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
