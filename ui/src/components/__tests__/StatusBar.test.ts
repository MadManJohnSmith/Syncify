/**
 * Regression tests for FE-7: StatusBar must not render fake telemetry.
 *
 * Covers:
 * - the network popover shows no hardcoded 'Wi-Fi' / '12.3 MB/s' values and
 *   picks up the real download speed from `syncify:download_progress` events;
 * - a real failed sync task drives the 'error' state with the task's real
 *   error message (never the invented 'Connection timeout');
 * - 'Last synced' comes from the real account `last_synced` instead of a
 *   hardcoded 'Just now'.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

vi.mock('../../api/storage', () => ({
  getStorageStats: vi.fn().mockResolvedValue({
    used_bytes: 1024 * 1024 * 1024,
    total_bytes: 1024 * 1024 * 1024 * 10,
    available_bytes: 1024 * 1024 * 1024 * 9,
    path: '/tmp/music',
    breakdown: [],
  }),
}))

vi.mock('../../api/accounts', () => ({
  getServiceStatuses: vi.fn().mockResolvedValue([]),
}))

vi.mock('../../api/settings', () => ({
  saveSetting: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('../../composables/usePlayer', () => ({
  usePlayer: () => ({ current: { value: null } }),
}))

vi.mock('../../composables/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
}))

import StatusBar from '../StatusBar.vue'
import { useGlobalTasks, resetGlobalTasks } from '../../composables/useGlobalTasks'
import { getServiceStatuses } from '../../api/accounts'
import { emitMockEvent } from '../../__tests__/setup'
import { TauriEvents } from '../../api/tauri'

const mockedGetServiceStatuses = vi.mocked(getServiceStatuses)

function sqliteTimestamp(msAgo: number): string {
  const d = new Date(Date.now() - msAgo)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ` +
    `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}`
}

async function mountStatusBar() {
  const wrapper = mount(StatusBar)
  await flushPromises()
  return wrapper
}

beforeEach(() => {
  resetGlobalTasks()
  mockedGetServiceStatuses.mockResolvedValue([])
})

describe('StatusBar (FE-7 fake telemetry removal)', () => {
  it('never renders the hardcoded connection type or download speed', async () => {
    const wrapper = await mountStatusBar()

    // Open the network popover
    await wrapper.find('.network-status').trigger('click')
    await nextTick()

    const text = wrapper.text()
    expect(text).not.toContain('Wi-Fi')
    expect(text).not.toContain('12.3')
    // No active download → honest em dash instead of a made-up speed
    expect(text).toContain('—')
  })

  it('shows the real download speed from download progress events and resets on completion', async () => {
    const wrapper = await mountStatusBar()

    emitMockEvent(TauriEvents.DOWNLOAD_PROGRESS, {
      queue_id: 1,
      status: 'downloading',
      progress_percent: 40,
      instant_kbps: 512.6,
      average_kbps: 500,
      terminal: false,
    })
    await nextTick()

    await wrapper.find('.network-status').trigger('click')
    await nextTick()
    expect(wrapper.text()).toContain('513 KB/s')

    // Terminal event clears the readout back to the em dash
    emitMockEvent(TauriEvents.DOWNLOAD_PROGRESS, {
      queue_id: 1,
      status: 'complete',
      terminal: true,
    })
    await nextTick()
    expect(wrapper.text()).toContain('—')
  })

  it('drives the sync error state from a real failed sync task with its real message', async () => {
    const wrapper = await mountStatusBar()
    const { addTask } = useGlobalTasks()

    addTask({
      id: 'sync-qobuz',
      type: 'sync',
      name: 'Syncing Qobuz',
      status: 'failed',
      progress: 30,
      error: 'Qobuz session expired',
    })
    await nextTick()

    const text = wrapper.text()
    expect(text).toContain('Sync failed')
    expect(text).not.toContain('Connection timeout')

    // Open the sync popover: the real error message is shown, not an invented one
    await wrapper.find('.sync-status').trigger('click')
    await nextTick()
    expect(wrapper.text()).toContain('Qobuz session expired')
    expect(wrapper.text()).not.toContain('Connection timeout')
  })

  it('shows "Last synced" from the real account last_synced instead of a hardcoded value', async () => {
    mockedGetServiceStatuses.mockResolvedValue([
      {
        name: 'Spotify',
        connected: true,
        account_email: 'a@b.c',
        library_count: 0,
        favorites_count: 0,
        playlists_count: 0,
        last_synced: sqliteTimestamp(5 * 60_000),
        credentials_invalid: false,
      },
    ])

    const wrapper = await mountStatusBar()
    await wrapper.find('.sync-status').trigger('click')
    await flushPromises()

    const text = wrapper.text()
    expect(text).toContain('Last synced: 5 min ago')
    expect(text).not.toContain('Just now')
  })

  it('shows "Never" as last sync time when no account ever synced', async () => {
    const wrapper = await mountStatusBar()
    await wrapper.find('.sync-status').trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('Last synced: Never')
  })
})
