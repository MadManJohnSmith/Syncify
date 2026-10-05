/**
 * Regression tests for the unified, in-app notice system (R13 + R16).
 *
 * R13: nothing may open a native dialog. confirm/alert/prompt come from
 * `@/composables/useToast` and are rendered by ToastNotifications.vue.
 * R16: every notice that can be retired on its own schedules a timer.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import ToastNotifications from '@/components/ToastNotifications.vue'
import { useToast, alert, confirm, prompt, useDialog } from '@/composables/useToast'

async function renderHost() {
    const wrapper = mount(ToastNotifications, { attachTo: document.body })
    await flushPromises()
    return wrapper
}

async function dismissAllDialogs() {
    const { dialogQueue } = useDialog()
    // dialogQueue es readonly; resolver vacía la cola en cascada.
    let guard = 0
    while (document.querySelector('.dialog-confirm') && guard++ < 20) {
        ;(document.querySelector('.dialog-confirm') as HTMLElement).click()
        await flushPromises()
    }
    void dialogQueue
}

describe('R13 — sistema unificado de avisos sin diálogos nativos', () => {
    beforeEach(async () => {
        document.body.innerHTML = ''
        const { toasts, dismiss } = useToast()
        toasts.value.forEach((t) => dismiss(t.id))
        useToast().clearAllHistory()
        await dismissAllDialogs()
    })

    afterEach(() => {
        vi.clearAllMocks()
        document.body.innerHTML = ''
    })

    it('confirm() renders an in-app dialog and resolves true on confirm', async () => {
        const wrapper = await renderHost()
        const pending = confirm('Delete this playlist?')

        await flushPromises()
        expect(document.querySelector('[role="alertdialog"]')).not.toBeNull()
        expect(document.body.textContent).toContain('Delete this playlist?')

        ;(document.querySelector('.dialog-confirm') as HTMLElement).click()
        await expect(pending).resolves.toBe(true)
        expect(document.querySelector('[role="alertdialog"]')).toBeNull()
        wrapper.unmount()
    })

    it('confirm() resolves false when cancelled', async () => {
        const wrapper = await renderHost()
        const pending = confirm('Delete this playlist?')
        await flushPromises()

        ;(document.querySelector('.dialog-cancel') as HTMLElement).click()
        await expect(pending).resolves.toBe(false)
        wrapper.unmount()
    })

    it('alert() offers no cancel button and resolves undefined', async () => {
        const wrapper = await renderHost()
        const pending = alert('Import finished')
        await flushPromises()

        expect(document.querySelector('.dialog-cancel')).toBeNull()
        ;(document.querySelector('.dialog-confirm') as HTMLElement).click()
        await expect(pending).resolves.toBeUndefined()
        wrapper.unmount()
    })

    it('prompt() returns the typed text', async () => {
        const wrapper = await renderHost()
        const pending = prompt('Playlist name', { defaultValue: 'Mi lista' })
        await flushPromises()

        const input = document.querySelector('.dialog-card input') as HTMLInputElement
        expect(input).not.toBeNull()
        input.value = 'Otra lista'
        input.dispatchEvent(new Event('input'))
        await flushPromises()

        ;(document.querySelector('.dialog-confirm') as HTMLElement).click()
        await expect(pending).resolves.toBe('Otra lista')
        wrapper.unmount()
    })

    it('prompt() resolves null when cancelled', async () => {
        const wrapper = await renderHost()
        const pending = prompt('Playlist name')
        await flushPromises()

        ;(document.querySelector('.dialog-cancel') as HTMLElement).click()
        await expect(pending).resolves.toBeNull()
        wrapper.unmount()
    })

    it('queues several dialogs and resolves them in order', async () => {
        const wrapper = await renderHost()
        const first = confirm('One?')
        const second = confirm('Two?')
        await flushPromises()

        expect(document.body.textContent).toContain('One?')
        expect(document.body.textContent).not.toContain('Two?')

        ;(document.querySelector('.dialog-confirm') as HTMLElement).click()
        await flushPromises()
        expect(document.body.textContent).toContain('Two?')

        ;(document.querySelector('.dialog-cancel') as HTMLElement).click()
        await expect(first).resolves.toBe(true)
        await expect(second).resolves.toBe(false)
        wrapper.unmount()
    })

    it('never calls the native plugin-dialog helpers', async () => {
        const dialog = await import('@tauri-apps/plugin-dialog')
        const wrapper = await renderHost()

        confirm('No native dialog?')
        await flushPromises()
        ;(document.querySelector('.dialog-confirm') as HTMLElement).click()
        await flushPromises()

        expect(dialog.confirm).not.toHaveBeenCalled()
        expect(dialog.message).not.toHaveBeenCalled()
        wrapper.unmount()
    })
})

describe('R16 — las notificaciones se van solas', () => {
    beforeEach(() => {
        vi.useFakeTimers()
        document.body.innerHTML = ''
        const { toasts, dismiss } = useToast()
        toasts.value.forEach((t) => dismiss(t.id))
        useToast().clearAllHistory()
    })

    afterEach(() => {
        vi.useRealTimers()
        document.body.innerHTML = ''
    })

    it('an error toast dismisses itself instead of waiting for a manual close', () => {
        const { error, toasts } = useToast()
        const id = error('Worker crashed', 'The download worker stopped unexpectedly.')

        expect(toasts.value.length).toBe(1)
        vi.advanceTimersByTime(7999)
        expect(toasts.value.length).toBe(1)

        vi.advanceTimersByTime(1)
        expect(toasts.value.length).toBe(0)
        void id
    })

    it('completeProgress(false) also schedules the auto-dismiss', () => {
        const { progress, toasts, completeProgress } = useToast()
        const id = progress('Downloading', 0)

        vi.advanceTimersByTime(5000)
        completeProgress(id, false, 'Download failed')

        expect(toasts.value[0].type).toBe('error')
        expect(toasts.value[0].autoDismiss).toBe(true)
        vi.advanceTimersByTime(7999)
        expect(toasts.value.length).toBe(1)
        vi.advanceTimersByTime(1)
        expect(toasts.value.length).toBe(0)
    })

    it('marks the history entry as read once the toast is gone', () => {
        const { success, history, unreadCount } = useToast()
        success('Saved')

        expect(unreadCount.value).toBe(1)
        vi.advanceTimersByTime(3000)

        expect(unreadCount.value).toBe(0)
        expect(history.value).toHaveLength(1)
        expect(history.value[0].read).toBe(true)
    })
})