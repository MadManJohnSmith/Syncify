/**
 * Unit tests for LogsView.vue
 * Tests structured real-time logs, level filtering, provider filtering, and search
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import LogsView from '../../views/LogsView.vue'
import { mockInvoke, resetMocks, emitMockEvent } from '../setup'

describe('LogsView.vue', () => {
  beforeEach(() => {
    resetMocks()
    mockInvoke((command) => {
      if (command === 'get_enrichment_status') {
        return {
          is_paused: false,
          pending_count: 5,
          completed_count: 20,
          failed_count: 1,
        }
      }
      return null
    })
  })

  it('renders system logs header and initial mock logs', async () => {
    const wrapper = mount(LogsView)
    await flushPromises()

    expect(wrapper.text()).toContain('Audit & System Logs')
    expect(wrapper.text()).toContain('Application started successfully')
    expect(wrapper.text()).toContain('Spotify')
    expect(wrapper.text()).toContain('Library')
  })

  it('adds structured log entry when receiving syncify:enrichment_event', async () => {
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

  it('adds structured log entry when receiving syncify:download_progress', async () => {
    const wrapper = mount(LogsView)
    await flushPromises()

    emitMockEvent('syncify:download_progress', {
      queue_id: 12,
      track_id: 505,
      title: 'Midnight Echoes',
      service_name: 'tidal',
      status: 'complete',
      progress_percent: 100,
    })
    await flushPromises()

    expect(wrapper.text()).toContain('Downloaded "Midnight Echoes"')
    expect(wrapper.text()).toContain('Tidal')
    expect(wrapper.text()).toContain('SUCCESS')
  })

  it('filters logs by severity level (INFO, WARN, ERROR, SUCCESS)', async () => {
    const wrapper = mount(LogsView)
    await flushPromises()

    // Filter by ERROR
    const levelSelect = wrapper.find('select:has(option[value="error"])')
    expect(levelSelect.exists()).toBe(true)

    await levelSelect.setValue('error')
    await flushPromises()

    expect(wrapper.text()).toContain('Failed to download "Track 4"')
    expect(wrapper.text()).not.toContain('Application started successfully')

    // Filter by SUCCESS
    await levelSelect.setValue('success')
    await flushPromises()

    expect(wrapper.text()).toContain('Scanned 1,240 tracks')
    expect(wrapper.text()).not.toContain('Failed to download "Track 4"')
  })

  it('filters logs by provider (Spotify, Qobuz, Tidal, Deezer, System)', async () => {
    const wrapper = mount(LogsView)
    await flushPromises()

    const providerSelect = wrapper.find('select:has(option[value="spotify"])')
    expect(providerSelect.exists()).toBe(true)

    // Filter by Spotify
    await providerSelect.setValue('spotify')
    await flushPromises()

    expect(wrapper.text()).toContain('Rate limit approaching')
    expect(wrapper.text()).not.toContain('Application started successfully')

    // Filter by System
    await providerSelect.setValue('system')
    await flushPromises()

    expect(wrapper.text()).toContain('Application started successfully')
    expect(wrapper.text()).not.toContain('Rate limit approaching')
  })

  it('filters logs by text search query', async () => {
    const wrapper = mount(LogsView)
    await flushPromises()

    const searchInput = wrapper.find('input[placeholder*="Search logs"]')
    expect(searchInput.exists()).toBe(true)

    await searchInput.setValue('Rate limit')
    await flushPromises()

    expect(wrapper.text()).toContain('Rate limit approaching')
    expect(wrapper.text()).not.toContain('Application started successfully')
  })

  it('clears displayed logs when clicking delete button', async () => {
    const wrapper = mount(LogsView)
    await flushPromises()

    const deleteBtn = wrapper.find('button[title="Clear displayed logs"]')
    expect(deleteBtn.exists()).toBe(true)

    await deleteBtn.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('No audit logs match the current filter')
  })
})
