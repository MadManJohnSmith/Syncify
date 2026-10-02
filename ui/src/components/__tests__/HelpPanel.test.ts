/**
 * Regression tests for FE-8: HelpPanel submissions must be real.
 *
 * Covers:
 * - "Submit Report" persists a bug report via save_user_report and shows a
 *   visible confirmation with the saved file path;
 * - "Send Feedback" persists feedback (type select + message bound with
 *   v-model);
 * - "Was this helpful? Yes/No" persists an article rating and confirms;
 * - "Copy info" writes the real system info to the clipboard and confirms;
 * - System info shows the real app version / connected services, not the old
 *   hardcoded '2.1.0' / 'Windows 11' / '3 (Spotify, Qobuz, Tidal)'.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'

vi.mock('@/api/tools', () => ({
  toolsApi: { saveUserReport: vi.fn() },
}))

vi.mock('@/api/accounts', () => ({
  getServiceStatuses: vi.fn(),
}))

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn(),
}))

vi.mock('../../composables/useToast', () => ({
  useToast: () => ({
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
}))

import HelpPanel from '../HelpPanel.vue'
import { toolsApi } from '@/api/tools'
import { getServiceStatuses } from '@/api/accounts'
import { getVersion } from '@tauri-apps/api/app'

const mockedSave = vi.mocked(toolsApi.saveUserReport)
const mockedGetServiceStatuses = vi.mocked(getServiceStatuses)
const mockedGetVersion = vi.mocked(getVersion)

const SAVED = { filePath: '/logs/reports/user-report-abc.json', createdAt: '2026-10-01T00:00:00Z' }

function mountPanel() {
  return mount(HelpPanel, {
    global: { stubs: { teleport: true } },
  })
}

async function openContactTab(wrapper: ReturnType<typeof mountPanel>) {
  const tabs = wrapper.findAll('button')
  const contactTab = tabs.find(b => b.text() === 'Contact')
  expect(contactTab, 'Contact tab should exist').toBeDefined()
  await contactTab!.trigger('click')
  await nextTick()
}

async function clickButtonWithText(wrapper: ReturnType<typeof mountPanel>, text: string) {
  const btn = wrapper.findAll('button').find(b => b.text() === text)
  expect(btn, `button "${text}" should exist`).toBeDefined()
  await btn!.trigger('click')
  await nextTick()
}

async function clickButtonContainingText(wrapper: ReturnType<typeof mountPanel>, text: string) {
  const btn = wrapper.findAll('button').find(b => b.text().includes(text))
  expect(btn, `button containing "${text}" should exist`).toBeDefined()
  await btn!.trigger('click')
  await nextTick()
}

beforeEach(() => {
  vi.clearAllMocks()
  mockedSave.mockResolvedValue(SAVED)
  mockedGetVersion.mockResolvedValue('3.2.1')
  mockedGetServiceStatuses.mockResolvedValue([
    { name: 'Spotify', connected: true, account_email: 'a@b.c', library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
    { name: 'Qobuz', connected: false, account_email: null, library_count: 0, favorites_count: 0, playlists_count: 0, last_synced: null, credentials_invalid: false },
  ])
})

describe('HelpPanel (FE-8 real submissions)', () => {
  it('submits the bug report with v-model values and shows the saved path', async () => {
    const wrapper = mountPanel()
    await flushPromises()
    await wrapper.vm.open()
    await nextTick()
    await openContactTab(wrapper)
    await clickButtonContainingText(wrapper, 'Report a Bug')

    const textareas = wrapper.findAll('textarea')
    expect(textareas.length).toBe(2)
    await textareas[0].setValue('App crashes when syncing favorites')
    await textareas[1].setValue('1. Open favorites 2. Click sync')

    await clickButtonWithText(wrapper, 'Submit Report')
    await flushPromises()

    expect(mockedSave).toHaveBeenCalledTimes(1)
    expect(mockedSave).toHaveBeenCalledWith({
      kind: 'bug_report',
      message: 'App crashes when syncing favorites',
      steps_to_reproduce: '1. Open favorites 2. Click sync',
      attach_logs: true,
    })
    expect(wrapper.text()).toContain('Bug report saved')
    expect(wrapper.text()).toContain('/logs/reports/user-report-abc.json')
  })

  it('rejects an empty bug description without calling the backend', async () => {
    const wrapper = mountPanel()
    await flushPromises()
    await wrapper.vm.open()
    await nextTick()
    await openContactTab(wrapper)
    await clickButtonContainingText(wrapper, 'Report a Bug')

    await clickButtonWithText(wrapper, 'Submit Report')
    await flushPromises()

    expect(mockedSave).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('Please describe the bug before submitting.')
  })

  it('submits feedback with the selected type and message', async () => {
    const wrapper = mountPanel()
    await flushPromises()
    await wrapper.vm.open()
    await nextTick()
    await openContactTab(wrapper)
    await clickButtonContainingText(wrapper, 'Send Feedback')

    await (wrapper.find('select')).setValue('Feature Request')
    await wrapper.find('textarea').setValue('Please add OGG support')

    await clickButtonWithText(wrapper, 'Send Feedback')
    await flushPromises()

    expect(mockedSave).toHaveBeenCalledWith({
      kind: 'feedback',
      message: 'Please add OGG support',
      feedback_type: 'Feature Request',
    })
    expect(wrapper.text()).toContain('Feedback saved')
    expect(wrapper.text()).toContain('/logs/reports/user-report-abc.json')
  })

  it('persists "Was this helpful?" ratings for the open article', async () => {
    const wrapper = mountPanel()
    await flushPromises()
    await wrapper.vm.open()
    await nextTick()

    // Open an article from the Articles tab (default tab)
    const titleSpan = wrapper.findAll('span').find(s => s.text() === 'Managing playlists')
    expect(titleSpan, 'article "Managing playlists" should be listed').toBeDefined()
    await titleSpan!.trigger('click')
    await nextTick()

    await clickButtonContainingText(wrapper, 'Yes')
    await flushPromises()

    expect(mockedSave).toHaveBeenCalledWith({
      kind: 'article_feedback',
      message: '',
      article_title: 'Managing playlists',
      helpful: true,
    })
    expect(wrapper.text()).toContain('Thanks for your feedback!')
    // The rating buttons are replaced by the confirmation
    expect(wrapper.text()).not.toContain('thumb_up')
  })

  it('copies the real system info to the clipboard and confirms', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })

    const wrapper = mountPanel()
    await flushPromises()
    await wrapper.vm.open()
    await nextTick()
    await openContactTab(wrapper)

    // Expand System Information
    const sysInfoToggle = wrapper.findAll('button').find(b => b.text().includes('System Information'))
    expect(sysInfoToggle).toBeDefined()
    await sysInfoToggle!.trigger('click')
    await nextTick()

    // Real values from the backend, not the old hardcoded ones
    expect(wrapper.text()).toContain('3.2.1')
    expect(wrapper.text()).toContain('1 (Spotify)')
    expect(wrapper.text()).not.toContain('Windows 11')
    expect(wrapper.text()).not.toContain('3 (Spotify, Qobuz, Tidal)')

    await clickButtonContainingText(wrapper, 'Copy info')
    await flushPromises()

    expect(writeText).toHaveBeenCalledTimes(1)
    const copied = writeText.mock.calls[0][0] as string
    expect(copied).toContain('App Version: 3.2.1')
    expect(copied).toContain('Connected Services: 1 (Spotify)')
    expect(wrapper.text()).toContain('Copied!')
  })

  it('surfaces backend failures instead of discarding the report silently', async () => {
    mockedSave.mockRejectedValue(new Error('disk full'))

    const wrapper = mountPanel()
    await flushPromises()
    await wrapper.vm.open()
    await nextTick()
    await openContactTab(wrapper)
    await clickButtonContainingText(wrapper, 'Report a Bug')

    await wrapper.findAll('textarea')[0].setValue('Crash on import')
    await clickButtonWithText(wrapper, 'Submit Report')
    await flushPromises()

    expect(wrapper.text()).toContain('disk full')
    expect(wrapper.text()).not.toContain('Bug report saved')
  })
})
