/**
 * DownloadsViewJobsPanel.spec.ts
 * D08 (#120): DownloadsView wires the jobs panel to the IPC wrappers, updates
 * counters from `syncify:download_job_progress`, and never drops the ledger
 * when clearing finished jobs.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'

const confirmMock = vi.hoisted(() => vi.fn<(...args: unknown[]) => Promise<boolean>>())
vi.mock('@/composables/useToast', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/composables/useToast')>()
  return { ...actual, confirm: confirmMock }
})

import DownloadsView from '@/views/DownloadsView.vue'
import { emitMockEvent, mockInvoke, resetMocks } from '../setup'
import { invoke } from '@tauri-apps/api/core'

const runningJob = {
  id: 42,
  kind: 'favorites',
  label: 'Favorites',
  status: 'running',
  sourceService: 'qobuz',
  destinationService: 'tidal',
  preferredAccountId: 7,
  qualityPreference: 'lossless',
  allowFallback: 1,
  filterJson: null,
  requestedCount: 10,
  estimatedCount: 10,
  estimatedSizeMb: 350,
  completedCount: 2,
  failedCount: 1,
  skippedCount: 1,
  skipBreakdownJson: null,
  error: null,
  createdAt: '2026-11-18T10:00:00Z',
  updatedAt: '2026-11-18T10:05:00Z',
  startedAt: '2026-11-18T10:00:00Z',
  completedAt: null,
}

const queueItem = {
  id: 5,
  track_id: 105,
  target_title: 'Resolved Account Track',
  target_artist: 'The Auditors',
  target_album: 'Evidence',
  service_name: 'qobuz',
  quality_preference: 'hires',
  requested_quality: 'hires',
  effective_quality: 'lossless',
  resolved_account_id: 7,
  status: 'complete',
  priority: 50,
  progress_percent: 100,
  error_message: null,
  created_at: '2026-11-18T10:00:00Z',
  started_at: '2026-11-18T10:01:00Z',
  completed_at: '2026-11-18T10:03:00Z',
}

const activeItem = {
  id: 6,
  track_id: 106,
  target_title: 'Active Row',
  target_artist: 'The Auditors',
  target_album: 'Evidence',
  service_name: 'qobuz',
  quality_preference: 'lossless',
  requested_quality: 'lossless',
  resolved_account_id: 7,
  status: 'downloading',
  priority: 50,
  progress_percent: 25,
  error_message: null,
  created_at: '2026-11-18T10:00:00Z',
  started_at: '2026-11-18T10:01:00Z',
  completed_at: null,
}

const queueStats = { total: 2, queued: 0, downloading: 1, completed: 1, failed: 0, paused: 0 }
const workerStatus = { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }

const account = {
  id: 7,
  service_id: 1,
  service_name: 'qobuz',
  display_name: 'Studio Account',
  email: 'studio@example.com',
  is_active: true,
  last_synced: null,
  created_at: null,
}

function mockBase(
  overrides: (cmd: string, args?: Record<string, unknown>) => unknown,
  jobs: unknown[] = [runningJob],
) {
  mockInvoke((cmd, args) => {
    const custom = overrides(cmd, args)
    if (custom !== undefined) return custom
    if (cmd === 'get_queue') return [activeItem, queueItem]
    if (cmd === 'get_queue_stats') return queueStats
    if (cmd === 'get_worker_status') return workerStatus
    if (cmd === 'get_download_jobs') return jobs
    if (cmd === 'get_accounts') return [account]
    return undefined
  })
}

describe('DownloadsView — jobs panel (D08, #120)', () => {
  beforeEach(() => {
    resetMocks()
    confirmMock.mockReset()
    confirmMock.mockResolvedValue(true)
  })

  it('lists jobs and updates their counters from the job-progress event', async () => {
    mockBase(() => undefined)

    const wrapper = mount(DownloadsView)
    await flushPromises()

    const row = wrapper.find('[data-testid="download-job-row"]')
    expect(row.exists()).toBe(true)
    expect(row.text()).toContain('Favorites')
    expect(row.text()).toContain('Studio Account')
    // Persisted counters only: pending is derived until the first event.
    expect(row.find('[data-testid="job-counter-requested"]').text()).toBe('10')
    expect(row.find('[data-testid="job-counter-queued"]').text()).toBe('6')

    emitMockEvent('syncify:download_job_progress', {
      job_id: 42,
      label: 'Favorites',
      status: 'running',
      total: 10,
      queued: 4,
      downloading: 2,
      completed: 2,
      failed: 1,
      cancelled: 0,
      skipped: 1,
      percent: 40,
      terminal: false,
    })
    await flushPromises()

    const updated = wrapper.find('[data-testid="download-job-row"]')
    expect(updated.find('[data-testid="job-counter-queued"]').text()).toBe('4')
    expect(updated.find('[data-testid="job-counter-downloading"]').text()).toBe('2')
    expect(updated.find('[data-testid="job-percent"]').text()).toBe('40%')
  })

  it('cancels pending rows and retries failed rows through the job wrappers', async () => {
    mockBase((cmd) => {
      if (cmd === 'cancel_download_job') return { ...runningJob, status: 'cancelled' }
      if (cmd === 'retry_download_job_failed') return { ...runningJob, status: 'running' }
      return undefined
    })

    const wrapper = mount(DownloadsView)
    await flushPromises()

    await wrapper.find('[data-testid="job-cancel"]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('cancel_download_job', { jobId: 42 })

    await wrapper.find('[data-testid="job-retry"]').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('retry_download_job_failed', { jobId: 42 })
  })

  it('clears finished jobs without touching the downloads ledger', async () => {
    mockBase(
      (cmd) => {
        if (cmd === 'clear_download_jobs') return 1
        return undefined
      },
      [{ ...runningJob, status: 'completed' }],
    )

    const wrapper = mount(DownloadsView)
    await flushPromises()

    await wrapper.find('[data-testid="job-clear-finished"]').trigger('click')
    await flushPromises()

    expect(confirmMock).toHaveBeenCalled()
    expect(invoke).toHaveBeenCalledWith('clear_download_jobs')
    expect(invoke).not.toHaveBeenCalledWith('clear_download_history', expect.anything())
    expect(wrapper.find('[data-testid="download-job-row"]').exists()).toBe(false)
  })

  it('disables the cancel button of an active row and shows the resolved account and quality', async () => {
    mockBase(() => undefined)

    const wrapper = mount(DownloadsView)
    await flushPromises()

    const activeCancel = wrapper.find('[data-testid="active-cancel-disabled"]')
    expect(activeCancel.exists()).toBe(true)
    expect(activeCancel.attributes('disabled')).toBeDefined()
    expect(activeCancel.attributes('title')).toContain('Terminará la actual')

    const qualityRows = wrapper.findAll('[data-testid="queue-row-account-quality"]')
    expect(qualityRows.length).toBeGreaterThan(0)
    // The completed row carries the resolved account and both quality values.
    expect(wrapper.text()).toContain('Studio Account')
    expect(wrapper.text()).toContain('Requested Hi-Res Lossless')
    expect(wrapper.text()).toContain('Delivered')
  })
})
