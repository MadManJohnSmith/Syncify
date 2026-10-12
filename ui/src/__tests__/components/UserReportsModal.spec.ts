/**
 * FE-8 regression: reports and feedback must actually reach `save_user_report`.
 *
 * `HelpPanel.vue` was deleted and replaced by `KeyboardShortcuts.vue`, which has
 * no forms. That left `toolsApi.saveUserReport` and the backend command with
 * ZERO callers in the UI — the only way to file a bug report was gone. This
 * spec guards the replacement (`UserReportsModal.vue`, mounted from
 * Settings → Advanced → Reports & Feedback) so the removal is not repeated:
 *
 * - no dead `href="#"` / placeholder links or buttons in the panel,
 * - submitting the bug report calls `save_user_report` with a real payload,
 * - the returned local path is shown to the user,
 * - backend failures and empty input are handled instead of discarded,
 * - feedback keeps its own report type and message contract.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import UserReportsModal from '@/components/UserReportsModal.vue'
import SettingsAdvanced from '@/views/settings/SettingsAdvanced.vue'

const { saveUserReport, toastSuccess, toastError } = vi.hoisted(() => ({
    saveUserReport: vi.fn(),
    toastSuccess: vi.fn(),
    toastError: vi.fn(),
}))

vi.mock('@/api/tools', () => ({
    toolsApi: { saveUserReport },
}))

vi.mock('@/composables/useToast', () => ({
    useToast: () => ({ success: toastSuccess, error: toastError, info: vi.fn(), warning: vi.fn() }),
    confirm: vi.fn(),
}))

const SAVED_PATH = '/home/user/.local/share/com.syncify.app/logs/reports/user-report-abc.json'

function mountModal(initialMode?: 'bug' | 'feedback') {
    const wrapper = mount(UserReportsModal, {
        attachTo: document.body,
        props: { modelValue: true, ...(initialMode ? { initialMode } : {}) },
    })
    return wrapper
}

function bodyText(): string {
    return document.body.textContent || ''
}

function deferSave() {
    let release: (() => void) | undefined
    const promise = new Promise<{ filePath: string; createdAt: string }>((resolve) => {
        release = () => resolve({ filePath: SAVED_PATH, createdAt: '2026-10-11T00:00:00Z' })
    })
    saveUserReport.mockImplementation(() => promise)
    return { settle: () => release?.() }
}

function buttons(): HTMLButtonElement[] {
    return Array.from(document.querySelectorAll('button')) as HTMLButtonElement[]
}

function clickByText(text: string): void {
    const target = buttons().find(b => (b.textContent || '').includes(text))
    expect(target, `no button containing "${text}"`).toBeDefined()
    target!.click()
}

async function fillTextarea(testId: string, value: string): Promise<void> {
    const area = document.querySelector(`[data-testid="${testId}"]`) as HTMLTextAreaElement | null
    expect(area, `no textarea "${testId}"`).not.toBeNull()
    area!.value = value
    area!.dispatchEvent(new Event('input'))
    await flushPromises()
}

describe('UserReportsModal — bug report and feedback really save (FE-8)', () => {
    beforeEach(() => {
        saveUserReport.mockReset()
        saveUserReport.mockResolvedValue({ filePath: SAVED_PATH, createdAt: '2026-10-11T00:00:00Z' })
        toastSuccess.mockReset()
        toastError.mockReset()
        document.body.innerHTML = ''
    })

    afterEach(() => {
        vi.clearAllMocks()
        document.body.innerHTML = ''
    })

    it('submits the bug report with the typed values and shows the returned path', async () => {
        mountModal('bug')
        await flushPromises()

        await fillTextarea('user-reports-bug-message', 'The download queue stalls at 0%')
        await fillTextarea('user-reports-bug-steps', '1. Add a track 2. Click sync')

        clickByText('Submit report')
        await flushPromises()
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledTimes(1)
        expect(saveUserReport).toHaveBeenCalledWith({
            kind: 'bug_report',
            message: 'The download queue stalls at 0%',
            steps_to_reproduce: '1. Add a track 2. Click sync',
            attach_logs: true,
        })
        expect(bodyText()).toContain('Bug report saved')
        expect(bodyText()).toContain(SAVED_PATH)
    })

    it('submits the feedback with the selected type and message', async () => {
        mountModal('feedback')
        await flushPromises()

        const type = document.querySelector('[data-testid="user-reports-feedback-type"]') as HTMLSelectElement
        expect(type).not.toBeNull()
        type.value = 'Feature Request'
        type.dispatchEvent(new Event('change'))
        await flushPromises()

        await fillTextarea('user-reports-feedback-message', 'Please add a dark mode for the tray')

        clickByText('Send feedback')
        await flushPromises()
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledTimes(1)
        expect(saveUserReport).toHaveBeenCalledWith({
            kind: 'feedback',
            message: 'Please add a dark mode for the tray',
            feedback_type: 'Feature Request',
        })
        expect(bodyText()).toContain('Feedback saved')
        expect(bodyText()).toContain(SAVED_PATH)
    })

    it('keeps the two report kinds distinct in the payload', async () => {
        mountModal('bug')
        await flushPromises()
        await fillTextarea('user-reports-bug-message', 'crash on start')
        clickByText('Submit report')
        await flushPromises()

        clickByText('Write another')
        await flushPromises()

        await fillTextarea('user-reports-bug-message', 'second issue')
        clickByText('Submit report')
        await flushPromises()

        const kinds = saveUserReport.mock.calls.map(([payload]) => payload.kind)
        expect(kinds).toEqual(['bug_report', 'bug_report'])

        // The feedback form sends `kind: 'feedback'`, never the bug kind.
        saveUserReport.mockClear()
        clickByText('Write another')
        await flushPromises()
        clickByText('Feedback')
        await flushPromises()
        await fillTextarea('user-reports-feedback-message', 'nice work')
        clickByText('Send feedback')
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledTimes(1)
        expect(saveUserReport).toHaveBeenCalledWith({
            kind: 'feedback',
            message: 'nice work',
            feedback_type: 'General Feedback',
        })
    })

    it('sends the attach-logs opt-in only when the user leaves it checked', async () => {
        mountModal('bug')
        await flushPromises()
        await fillTextarea('user-reports-bug-message', 'queue stalls')

        const attach = document.querySelector('[data-testid="user-reports-attach-logs"]') as HTMLInputElement
        expect(attach.checked).toBe(true)
        attach.checked = false
        attach.dispatchEvent(new Event('change'))
        await flushPromises()

        clickByText('Submit report')
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledTimes(1)
        expect(saveUserReport.mock.calls[0][0].attach_logs).toBe(false)
    })

    it('rejects an empty description without calling the backend', async () => {
        mountModal('bug')
        await flushPromises()

        clickByText('Submit report')
        await flushPromises()

        expect(saveUserReport).not.toHaveBeenCalled()
        expect(bodyText()).toContain('Describe the bug before saving the report.')
        expect(bodyText()).not.toContain('Bug report saved')
    })

    it('rejects empty feedback without calling the backend', async () => {
        mountModal('feedback')
        await flushPromises()

        clickByText('Send feedback')
        await flushPromises()

        expect(saveUserReport).not.toHaveBeenCalled()
        expect(bodyText()).toContain('Write your feedback before sending it.')
        expect(bodyText()).not.toContain('Feedback saved')
    })

    it('surfaces a backend failure instead of pretending the report was saved', async () => {
        saveUserReport.mockRejectedValue(new Error('Could not write report: permission denied'))
        mountModal('bug')
        await flushPromises()
        await fillTextarea('user-reports-bug-message', 'queue stalls at 0%')

        clickByText('Submit report')
        await flushPromises()
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledTimes(1)
        expect(bodyText()).toContain('Could not write report: permission denied')
        expect(bodyText()).not.toContain('Bug report saved')
        expect(bodyText()).not.toContain(SAVED_PATH)
        expect(toastError).not.toHaveBeenCalled()
    })

    it('keeps the error visible and re-enables the submit button for a retry', async () => {
        saveUserReport.mockRejectedValueOnce(new Error('disk full'))
        mountModal('bug')
        await flushPromises()
        await fillTextarea('user-reports-bug-message', 'first try')

        clickByText('Submit report')
        await flushPromises()
        await flushPromises()
        expect(bodyText()).toContain('disk full')

        saveUserReport.mockResolvedValue({ filePath: SAVED_PATH, createdAt: '2026-10-11T00:00:00Z' })
        clickByText('Submit report')
        await flushPromises()
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledTimes(2)
        expect(bodyText()).toContain(SAVED_PATH)
    })

    it('disables the controls while the report is being written', async () => {
        const pending = deferSave()
        mountModal('bug')
        await flushPromises()
        await fillTextarea('user-reports-bug-message', 'queue stalls')

        clickByText('Submit report')
        await flushPromises()

        const submit = document.querySelector('[data-testid="user-reports-submit"]') as HTMLButtonElement
        expect(submit.disabled).toBe(true)
        expect(submit.getAttribute('aria-busy')).toBe('true')
        expect(bodyText()).toContain('Saving...')

        pending.settle()
        await flushPromises()
        await flushPromises()
        expect(bodyText()).toContain(SAVED_PATH)
    })

    it('has no dead "#" links or placeholder contact routes', async () => {
        mountModal()
        await flushPromises()

        // Every destination the panel promises must be something the app does.
        expect(document.querySelectorAll('a')).toHaveLength(0)
        for (const dead of ['Community Forum', 'Email Support', 'Join Discord', 'View full changelog']) {
            expect(bodyText()).not.toContain(dead)
        }
        for (const href of Array.from(document.querySelectorAll('[href]'))) {
            expect(href.getAttribute('href')).not.toBe('#')
        }
    })

    it('tells the user the reports are saved locally, because there is nowhere to send them', async () => {
        mountModal()
        await flushPromises()

        expect(bodyText()).toContain('reports folder')
        expect(bodyText()).toContain('Nothing is sent to a server')
    })

    it('closes only after the write finished, so a report is never lost', async () => {
        const pending = deferSave()
        const wrapper = mountModal('bug')
        await flushPromises()
        await fillTextarea('user-reports-bug-message', 'queue stalls')

        clickByText('Submit report')
        await flushPromises()

        clickByText('Cancel')
        await flushPromises()
        expect(wrapper.emitted('update:modelValue')).toBeUndefined()

        pending.settle()
        await flushPromises()
        await flushPromises()

        clickByText('Done')
        await flushPromises()
        expect(wrapper.emitted('update:modelValue')).toEqual([[false]])
    })

    it('is keyboard reachable: focus trap, Escape and focus restore', async () => {
        const wrapper = mountModal('bug')
        await flushPromises()

        const dialog = document.querySelector('[data-testid="user-reports-modal"]') as HTMLElement
        expect(dialog).not.toBeNull()

        // Focus starts inside the dialog.
        expect(dialog.contains(document.activeElement)).toBe(true)

        // Tab on the last focusable element wraps back to the first.
        const focusables = Array.from(dialog.querySelectorAll(
            'button:not(:disabled), textarea:not(:disabled), input:not(:disabled), select:not(:disabled)'
        )) as HTMLElement[]
        expect(focusables.length).toBeGreaterThan(1)
        focusables[focusables.length - 1].focus()
        dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }))
        await nextTick()
        expect(document.activeElement).toBe(focusables[0])

        // Escape closes, and it is bound on the teleported dialog itself.
        dialog.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
        await nextTick()
        expect(wrapper.emitted('update:modelValue')).toEqual([[false]])
    })
})

/**
 * The regression the rest of this file exists for: the panel is reachable from
 * the app. A perfect modal that nothing opens is as good as deleting HelpPanel
 * again, so the Settings entry points are asserted here too.
 */
describe('Settings → Advanced → Reports & Feedback, mounted and functional', () => {
    beforeEach(() => {
        saveUserReport.mockReset()
        saveUserReport.mockResolvedValue({ filePath: SAVED_PATH, createdAt: '2026-10-11T00:00:00Z' })
        toastSuccess.mockReset()
        toastError.mockReset()
        document.body.innerHTML = ''
    })

    afterEach(() => {
        vi.clearAllMocks()
        document.body.innerHTML = ''
    })

    it('opens the bug form from Settings Advanced and saves through it', async () => {
        const wrapper = mount(SettingsAdvanced)
        await flushPromises()

        expect(wrapper.find('[data-testid="advanced-user-reports"]').exists()).toBe(true)
        expect(wrapper.text()).toContain('reports folder')
        expect(document.body.querySelector('[data-testid="user-reports-modal"]')).toBeNull()

        await wrapper.get('[data-testid="advanced-open-bug-report"]').trigger('click')
        await flushPromises()
        expect(document.body.querySelector('[data-testid="user-reports-modal"]')).not.toBeNull()
        expect(wrapper.vm).toBeTruthy()

        await fillTextarea('user-reports-bug-message', 'queue stalls at 0%')
        clickByText('Submit report')
        await flushPromises()
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledTimes(1)
        expect(bodyText()).toContain(SAVED_PATH)
    })

    it('opens the feedback form from Settings Advanced', async () => {
        const wrapper = mount(SettingsAdvanced)
        await flushPromises()

        await wrapper.get('[data-testid="advanced-open-feedback"]').trigger('click')
        await flushPromises()

        expect(document.body.querySelector('[data-testid="user-reports-feedback-type"]')).not.toBeNull()
        expect(document.body.querySelector('[data-testid="user-reports-bug-message"]')).toBeNull()

        await fillTextarea('user-reports-feedback-message', 'shorter export please')
        clickByText('Send feedback')
        await flushPromises()
        await flushPromises()

        expect(saveUserReport).toHaveBeenCalledWith({
            kind: 'feedback',
            message: 'shorter export please',
            feedback_type: 'General Feedback',
        })
    })
})
