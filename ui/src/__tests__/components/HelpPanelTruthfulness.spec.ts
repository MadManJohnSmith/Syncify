/**
 * R19 regression: the Help modal must not promise things the app does not do.
 * It used to hardcode "v2.1.0 • December 2024" while the app is 0.3.0, and to
 * offer Community Forum / Email Support / Join Discord / full changelog links
 * that were all `href="#"`.
 *
 * The panel is rendered through <Teleport to="body">, so every assertion goes
 * through document.body, not through the wrapper.
 */
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import HelpPanel from '@/components/HelpPanel.vue'
import { mockInvoke, resetMocks } from '../setup'

vi.mock('@tauri-apps/api/app', () => ({
    getVersion: vi.fn(async () => '0.3.0')
}))

const REAL_VERSION = '0.3.0'

async function openHelp() {
    const wrapper = mount(HelpPanel, { attachTo: document.body, props: { modelValue: true } })
    await flushPromises()
    return wrapper
}

function clickButtonContaining(text: string) {
    const buttons = Array.from(document.querySelectorAll('button')) as HTMLButtonElement[]
    const target = buttons.find(b => (b.textContent || '').includes(text))
    expect(target, `no button containing "${text}"`).toBeDefined()
    target!.click()
}

describe('HelpPanel — la ayuda describe la app real (R19)', () => {
    beforeEach(() => {
        resetMocks()
        document.body.innerHTML = ''
    })

    afterEach(() => {
        vi.clearAllMocks()
        document.body.innerHTML = ''
    })

    it('shows the real app version in the What\'s New header, not a hardcoded one', async () => {
        const wrapper = await openHelp()
        clickButtonContaining('Contact')
        await flushPromises()
        clickButtonContaining("What's New")
        await flushPromises()

        expect(document.body.textContent).toContain(REAL_VERSION)
        expect(document.body.textContent).not.toContain('2.1.0')
        expect(document.body.textContent).not.toContain('December 2024')

        wrapper.unmount()
    })

    it('has no dead "#" links left in the panel', async () => {
        const wrapper = await openHelp()

        expect(document.querySelectorAll('a')).toHaveLength(0)
        for (const dead of ['Community Forum', 'Email Support', 'Join Discord', 'View full changelog']) {
            expect(document.body.textContent).not.toContain(dead)
        }

        wrapper.unmount()
    })

    it('tells the user reports are saved as files, because there is nowhere to send them', async () => {
        const wrapper = await openHelp()
        clickButtonContaining('Contact')
        await flushPromises()

        expect(document.body.textContent).toContain('reports folder')

        wrapper.unmount()
    })

    it('names the sync switch that actually exists', async () => {
        const wrapper = await openHelp()
        clickButtonContaining('FAQ')
        await flushPromises()
        clickButtonContaining('sync favorites automatically')
        await flushPromises()

        expect(document.body.textContent).toContain('Sync favorites')
        // Settings > Sync has no "Continuous Sync" toggle.
        expect(document.body.textContent).not.toContain('Continuous Sync')

        wrapper.unmount()
    })

    it('writes a bug report through save_user_report and shows the returned path', async () => {
        const savedPath = '/home/user/.local/share/com.syncify.app/logs/reports/bug_2026.json'
        mockInvoke((command: string) => {
            if (command === 'save_user_report') return { file_path: savedPath, created_at: '2026-01-01T00:00:00Z' }
            if (command === 'get_service_statuses') return []
            return {}
        })

        const wrapper = await openHelp()
        clickButtonContaining('Contact')
        await flushPromises()
        clickButtonContaining('Report a Bug')
        await flushPromises()

        const textareas = Array.from(document.querySelectorAll('textarea')) as HTMLTextAreaElement[]
        expect(textareas.length).toBeGreaterThanOrEqual(2)
        textareas[0].value = 'The queue stalls at 0%'
        textareas[0].dispatchEvent(new Event('input'))
        textareas[1].value = '1. Start a download'
        textareas[1].dispatchEvent(new Event('input'))
        await flushPromises()

        clickButtonContaining('Submit Report')
        await flushPromises()
        await flushPromises()

        expect(document.body.textContent).toContain(savedPath)

        wrapper.unmount()
    })
})