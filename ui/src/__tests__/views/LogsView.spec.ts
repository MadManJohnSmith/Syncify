/**
 * Unit tests for LogsView.vue (S170)
 * Tests real-time event logs, zero hardcoded mocks, honest empty state,
 * tab persistence across unmount/remount, level/provider filtering, and toolbar actions.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import LogsView from '../../views/LogsView.vue'
import { useLogs, resetLogs } from '@/composables/useLogs'
import { mockInvoke, resetMocks, emitMockEvent } from '../setup'

describe('LogsView.vue', () => {
  beforeEach(() => {
    resetMocks()
    resetLogs()
    mockInvoke((command) => {
      if (command === 'get_enrichment_status') {
        return {
          is_paused: false,
          pending_count: 5,
          completed_count: 20,
          failed_count: 1,
        }
      }
      if (command === 'get_system_logs') {
        return []
      }
      if (command === 'clear_system_logs') {
        return null
      }
      if (command === 'export_system_logs') {
        return '# Logs export'
      }
      return null
    })
  })

  it('renders loading state initially while logs are being fetched', async () => {
    let resolveLogsPromise: (val: any) => void
    const pendingLogsPromise = new Promise((resolve) => {
      resolveLogsPromise = resolve
    })

    mockInvoke((command) => {
      if (command === 'get_system_logs') {
        return pendingLogsPromise
      }
      return null
    })

    const wrapper = mount(LogsView)
    // Synchronously before logs promise resolves:
    expect(wrapper.text()).toContain('Loading system logs...')

    // Now resolve
    resolveLogsPromise!([])
    await flushPromises()

    expect(wrapper.text()).not.toContain('Loading system logs...')
    expect(wrapper.text()).toContain('No system logs recorded')
  })

  it('renders honest empty state when there are no logs recorded (zero mocks)', async () => {
    const wrapper = mount(LogsView)
    await flushPromises()

    expect(wrapper.text()).toContain('Audit & System Logs')
    expect(wrapper.text()).toContain('No system logs recorded')
    expect(wrapper.text()).not.toContain('Application started successfully. v2.1.0')
  })

  it('displays live logs added via background events', async () => {
    const { initLogListeners } = useLogs()
    await initLogListeners()

    const wrapper = mount(LogsView)
    await flushPromises()

    emitMockEvent('syncify:enrichment_event', {
      track_id: 204,
      service: 'qobuz',
      status: 'completed',
      message: 'Enriched metadata and album art from Qobuz API',
    })
    await flushPromises()

    expect(wrapper.text()).toContain('Enriched metadata and album art from Qobuz API')
    expect(wrapper.text()).toContain('Qobuz')
    expect(wrapper.text()).toContain('SUCCESS')
  })

  it('preserves logs across component unmount and remount (tab switching)', async () => {
    const { addLog } = useLogs()
    addLog({
      level: 'info',
      provider: 'Tidal',
      category: 'Downloads',
      message: 'Persistent log stream entry #1',
      rawCategory: 'downloads',
    })

    // Mount view on Logs tab
    let wrapper = mount(LogsView)
    await flushPromises()
    expect(wrapper.text()).toContain('Persistent log stream entry #1')

    // Navigate away (unmount LogsView)
    wrapper.unmount()

    // Receive background event while user is in another tab (e.g. Library or Settings)
    addLog({
      level: 'warn',
      provider: 'Spotify',
      category: 'Enrichment',
      message: 'Background event while user was on Settings tab',
      rawCategory: 'enrichment',
    })

    // User navigates back to Logs tab (remount LogsView)
    wrapper = mount(LogsView)
    await flushPromises()

    expect(wrapper.text()).toContain('Persistent log stream entry #1')
    expect(wrapper.text()).toContain('Background event while user was on Settings tab')
  })

  it('filters logs by severity level', async () => {
    const { addLog } = useLogs()
    addLog({ level: 'info', provider: 'System', category: 'Core', message: 'System startup OK', rawCategory: 'system' })
    addLog({ level: 'error', provider: 'Qobuz', category: 'Downloads', message: 'Download failed 404', rawCategory: 'downloads' })
    addLog({ level: 'success', provider: 'Tidal', category: 'Downloads', message: 'Download 100% complete', rawCategory: 'downloads' })

    const wrapper = mount(LogsView)
    await flushPromises()

    // Filter by ERROR
    const levelSelect = wrapper.find('select:has(option[value="error"])')
    expect(levelSelect.exists()).toBe(true)

    await levelSelect.setValue('error')
    await flushPromises()

    expect(wrapper.text()).toContain('Download failed 404')
    expect(wrapper.text()).not.toContain('System startup OK')
    expect(wrapper.text()).not.toContain('Download 100% complete')

    // Filter by SUCCESS
    await levelSelect.setValue('success')
    await flushPromises()

    expect(wrapper.text()).toContain('Download 100% complete')
    expect(wrapper.text()).not.toContain('Download failed 404')
  })

  it('R15: level "success" stays in-memory only — the disk history query never carries it', async () => {
    const { addLog } = useLogs()
    addLog({ level: 'success', provider: 'Tidal', category: 'Downloads', message: 'Download 100% complete', rawCategory: 'downloads' })

    const historyCalls: Array<Record<string, unknown> | undefined> = []
    mockInvoke((command, args) => {
      if (command === 'read_log_history') {
        historyCalls.push(args)
        return { entries: [], total: 0, offset: 0, limit: 500 }
      }
      return null
    })

    const wrapper = mount(LogsView)
    await flushPromises()

    const levelSelect = wrapper.find('select:has(option[value="success"])')
    expect(levelSelect.exists()).toBe(true)
    await levelSelect.setValue('success')

    const loadBtn = wrapper.find('button[title*="Load the log history"]')
    expect(loadBtn.exists()).toBe(true)
    await loadBtn.trigger('click')
    await flushPromises()

    // SUCCESS never reaches read_log_history: FileLogLayer does not write it
    // to disk, so the filter would always match 0 entries there.
    expect(historyCalls).toHaveLength(1)
    expect(historyCalls[0]?.level).toBeNull()
    // The in-memory SUCCESS entry is still shown through the level filter.
    expect(wrapper.text()).toContain('Download 100% complete')

    // Control: a real disk level IS forwarded to the history query.
    await levelSelect.setValue('error')
    await loadBtn.trigger('click')
    await flushPromises()
    expect(historyCalls).toHaveLength(2)
    expect(historyCalls[1]?.level).toBe('error')
  })

  it('filters logs by module/provider', async () => {
    const { addLog } = useLogs()
    addLog({ level: 'info', provider: 'Spotify', category: 'Enrichment', message: 'Spotify track resolved', rawCategory: 'enrichment' })
    addLog({ level: 'info', provider: 'Qobuz', category: 'Downloads', message: 'Qobuz stream opened', rawCategory: 'downloads' })

    const wrapper = mount(LogsView)
    await flushPromises()

    const providerSelect = wrapper.find('select:has(option[value="spotify"])')
    expect(providerSelect.exists()).toBe(true)

    await providerSelect.setValue('spotify')
    await flushPromises()

    expect(wrapper.text()).toContain('Spotify track resolved')
    expect(wrapper.text()).not.toContain('Qobuz stream opened')
  })

  it('filters logs by search query in real-time', async () => {
    const { addLog } = useLogs()
    addLog({ level: 'info', provider: 'Spotify', category: 'Enrichment', message: 'Searching for ISRC USQX92000875', rawCategory: 'enrichment' })
    addLog({ level: 'warn', provider: 'Tidal', category: 'Downloads', message: 'Bit depth downgraded to 16bit', rawCategory: 'downloads' })

    const wrapper = mount(LogsView)
    await flushPromises()

    const searchInput = wrapper.find('input[placeholder*="Search logs"]')
    expect(searchInput.exists()).toBe(true)

    await searchInput.setValue('USQX92000875')
    await flushPromises()

    expect(wrapper.text()).toContain('Searching for ISRC USQX92000875')
    expect(wrapper.text()).not.toContain('Bit depth downgraded')
  })

  it('clears displayed logs when clicking delete button', async () => {
    const { addLog } = useLogs()
    addLog({ level: 'info', provider: 'System', category: 'Core', message: 'Will be deleted', rawCategory: 'system' })

    const wrapper = mount(LogsView)
    await flushPromises()
    expect(wrapper.text()).toContain('Will be deleted')

    const deleteBtn = wrapper.find('button[title*="Clear"]')
    expect(deleteBtn.exists()).toBe(true)

    await deleteBtn.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('No system logs recorded')
    expect(wrapper.text()).not.toContain('Will be deleted')
  })

  it('S170A: displays File Logging Active and dev log file path when confirmed by backend', async () => {
    mockInvoke((command) => {
      if (command === 'get_logging_status') {
        return {
          is_development: true,
          file_logging_active: true,
          active_log_file_path: '/home/test/.local/share/com.syncify.app/logs/syncify-dev.log',
          log_dir: '/home/test/.local/share/com.syncify.app/logs',
          log_level: 'DEBUG',
          buffer_count: 5,
          retention_days: 30,
          max_file_size_mb: 50,
        }
      }
      return null
    })

    const wrapper = mount(LogsView)
    await flushPromises()

    expect(wrapper.text()).toContain('File Logging Active')
    expect(wrapper.text()).toContain('syncify-dev.log')
  })

  it('S170A: hides File Logging Active badge when backend confirms file logging is inactive', async () => {
    mockInvoke((command) => {
      if (command === 'get_logging_status') {
        return {
          is_development: false,
          file_logging_active: false,
          active_log_file_path: null,
          log_dir: '/home/test/.local/share/com.syncify.app/logs',
          log_level: 'INFO',
          buffer_count: 0,
          retention_days: 30,
          max_file_size_mb: 50,
        }
      }
      return null
    })

    const wrapper = mount(LogsView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('File Logging Active')
    expect(wrapper.text()).not.toContain('syncify-dev.log')
  })
})

