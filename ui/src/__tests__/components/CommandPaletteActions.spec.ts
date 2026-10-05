/**
 * Audit 2 regression: the command palette actions must run real operations
 * instead of only emitting `action` and closing (App.vue never listened to it).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import CommandPalette from '@/components/CommandPalette.vue'
import ToastNotifications from '@/components/ToastNotifications.vue'
import { useToast } from '@/composables/useToast'
import * as libraryApi from '@/api/library'
import * as accountsApi from '@/api/accounts'
import * as queueApi from '@/api/queue'
import * as lyricsApi from '@/api/lyrics'

const push = vi.fn()

vi.mock('vue-router', () => ({
    useRouter: () => ({ push })
}))

vi.mock('@/api/library', () => ({
    searchTracks: vi.fn(),
    getFavoriteTracks: vi.fn(),
    queueDownloads: vi.fn(),
    exportLibrary: vi.fn()
}))

vi.mock('@/api/accounts', () => ({
    syncService: vi.fn()
}))

vi.mock('@/api/queue', () => ({
    clearQueue: vi.fn(),
    retryAllFailed: vi.fn()
}))

vi.mock('@/api/lyrics', () => ({
    fetchMissingLyrics: vi.fn()
}))

async function openPaletteWith(query: string) {
    const host = mount(ToastNotifications, { attachTo: document.body })
    const wrapper = mount(CommandPalette, { attachTo: document.body })
    wrapper.vm.open()
    await flushPromises()

    const input = document.body.querySelector('.palette-input input') as HTMLInputElement
    input.value = query
    input.dispatchEvent(new Event('input'))
    await vi.advanceTimersByTimeAsync(250)
    await flushPromises()
    return { wrapper, host }
}

function clickAction() {
    const action = document.body.querySelector('.result-action') as HTMLElement
    expect(action).not.toBeNull()
    action.click()
}

describe('CommandPalette — actions run real operations (audit 2)', () => {
    beforeEach(() => {
        vi.useFakeTimers({ shouldAdvanceTime: true })
        const { toasts, dismiss } = useToast()
        toasts.value.forEach((t) => dismiss(t.id))
        useToast().clearAllHistory()
        vi.mocked(libraryApi.searchTracks).mockResolvedValue({
            tracks: [], total: 0, offset: 0, limit: 10, has_more: false
        } as any)
    })

    afterEach(async () => {
        await vi.advanceTimersByTimeAsync(500)
        vi.useRealTimers()
        vi.clearAllMocks()
        document.body.innerHTML = ''
    })

    it('collects every favorite page before queueing downloads', async () => {
        vi.mocked(libraryApi.getFavoriteTracks)
            .mockResolvedValueOnce({
                tracks: [{ id: 1 }, { id: 2 }], total: 3, offset: 0, limit: 200, has_more: true
            } as any)
            .mockResolvedValueOnce({
                tracks: [{ id: 3 }], total: 3, offset: 2, limit: 200, has_more: false
            } as any)
        vi.mocked(libraryApi.queueDownloads).mockResolvedValue('queued' as any)

        const { wrapper, host } = await openPaletteWith('Download all favorites')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        expect(libraryApi.getFavoriteTracks).toHaveBeenCalledTimes(2)
        expect(libraryApi.queueDownloads).toHaveBeenCalledWith([1, 2, 3])
        expect(useToast().toasts.value.some(t => t.type === 'success')).toBe(true)

        wrapper.unmount()
        host.unmount()
    })

    it('syncs Spotify through syncService and reports the imported count', async () => {
        vi.mocked(accountsApi.syncService).mockResolvedValue({
            success: true, message: 'done', imported_tracks_total: 12
        } as any)

        const { wrapper, host } = await openPaletteWith('Sync Spotify')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        expect(accountsApi.syncService).toHaveBeenCalledWith('spotify')
        expect(useToast().toasts.value[0].description).toContain('12')

        wrapper.unmount()
        host.unmount()
    })

    it('asks for confirmation in-app before clearing the download queue', async () => {
        vi.mocked(queueApi.clearQueue).mockResolvedValue(7 as any)

        const { wrapper, host } = await openPaletteWith('Clear download queue')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        expect(queueApi.clearQueue).not.toHaveBeenCalled()
        const dialog = document.body.querySelector('[role="alertdialog"]')
        expect(dialog).not.toBeNull()

        // Cancel first: nothing must happen.
        ;(document.body.querySelector('.dialog-cancel') as HTMLElement).click()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()
        expect(queueApi.clearQueue).not.toHaveBeenCalled()

        wrapper.unmount()
        host.unmount()
    })

    it('clears the queue once the confirmation is accepted', async () => {
        vi.mocked(queueApi.clearQueue).mockResolvedValue(7 as any)

        const { wrapper, host } = await openPaletteWith('Clear download queue')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()
        ;(document.body.querySelector('.dialog-confirm') as HTMLElement).click()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        expect(queueApi.clearQueue).toHaveBeenCalledWith()
        expect(useToast().toasts.value[0].description).toContain('7')

        wrapper.unmount()
        host.unmount()
    })

    it('retries failed downloads', async () => {
        vi.mocked(queueApi.retryAllFailed).mockResolvedValue('ok' as any)

        const { wrapper, host } = await openPaletteWith('Retry failed downloads')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        expect(queueApi.retryAllFailed).toHaveBeenCalled()

        wrapper.unmount()
        host.unmount()
    })

    it('fetches missing lyrics for the whole library', async () => {
        vi.mocked(lyricsApi.fetchMissingLyrics).mockResolvedValue({ fetched: 5, failed: 1, skipped: 2 })

        const { wrapper, host } = await openPaletteWith('Fetch lyrics for all tracks')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        expect(lyricsApi.fetchMissingLyrics).toHaveBeenCalled()
        expect(useToast().toasts.value[0].description).toContain('5 fetched')

        wrapper.unmount()
        host.unmount()
    })

    it('exports the library without opening a native save dialog', async () => {
        vi.mocked(libraryApi.exportLibrary).mockResolvedValue({
            file_path: '/home/user/Downloads/Syncify_Backup_20260101_000000.json',
            tracks_count: 42, albums_count: 0, artists_count: 0, playlists_count: 0,
            file_size_bytes: 10, checksum: ''
        } as any)

        const { wrapper, host } = await openPaletteWith('Export library')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        expect(libraryApi.exportLibrary).toHaveBeenCalledWith()
        expect(useToast().toasts.value[0].description).toContain('42')

        wrapper.unmount()
        host.unmount()
    })

    it('surfaces a backend failure as an error toast instead of doing nothing', async () => {
        vi.mocked(queueApi.retryAllFailed).mockRejectedValue(new Error('worker offline'))

        const { wrapper, host } = await openPaletteWith('Retry failed downloads')
        clickAction()
        await vi.advanceTimersByTimeAsync(10)
        await flushPromises()

        const toasts = useToast().toasts.value
        expect(toasts[0].type).toBe('error')
        expect(toasts[0].description).toContain('worker offline')

        wrapper.unmount()
        host.unmount()
    })
})