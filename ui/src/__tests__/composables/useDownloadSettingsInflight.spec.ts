/**
 * `useDownloadSettings` de-duplicates concurrent loads.
 *
 * `SettingsView` calls `loadSettings()` on mount and several settings tabs do
 * the same from their own `onMounted`, each behind an ad-hoc "already loaded?"
 * guard. When those mounts overlap the identical IPC fan-out ran once per
 * caller for a single screen.
 *
 * The guard is deliberately *inflight-only*: a caller arriving after a load has
 * settled still re-reads. Caching completed loads was tried and reverted — it
 * served a stale snapshot whenever the database changed outside this module,
 * which broke existing tests that reset the backend between cases. This test
 * pins the half that is safe.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'

const calls: string[] = []

vi.mock('@/api/settings', () => ({
    settingsApi: {
        getEffectiveDownloadPreferences: vi.fn(async () => {
            calls.push('get_effective_download_preferences')
            // Hold the promise open so overlapping callers really do overlap.
            await new Promise((resolve) => setTimeout(resolve, 10))
            return null
        }),
        getUnifiedDownloadSettings: vi.fn(async () => {
            calls.push('get_unified_download_settings')
            return null
        }),
        getDownloadSettings: vi.fn(async () => {
            calls.push('get_download_settings')
            return null
        }),
        getSettingsByKeys: vi.fn(async () => {
            calls.push('get_kv_settings')
            return {}
        }),
        getDefaultDownloadPath: vi.fn(async () => {
            calls.push('get_default_download_path')
            return ''
        }),
        getFolderSettings: vi.fn(async () => null),
        getDuplicateSettings: vi.fn(async () => null),
        getAudioProcessingSettings: vi.fn(async () => null),
        getQualityPreferences: vi.fn(async () => []),
        validateDirectoryPath: vi.fn(async () => ({ valid: true })),
    },
    deriveStagingRoot: (p: string) => `${p}/.staging`,
}))

vi.mock('@/composables/useEventBus', () => ({
    useEventBus: () => ({ on: vi.fn() }),
}))

function countOf(name: string) {
    return calls.filter((c) => c === name).length
}

describe('useDownloadSettings load de-duplication', () => {
    beforeEach(() => {
        calls.length = 0
        vi.resetModules()
    })

    it('collapses concurrent loads into a single IPC fan-out', async () => {
        const { useDownloadSettings } = await import('@/composables/useDownloadSettings')

        // Four consumers mounting at once, as SettingsView + three tabs do.
        const a = useDownloadSettings()
        const b = useDownloadSettings()
        const c = useDownloadSettings()
        const d = useDownloadSettings()

        await Promise.all([a.loadSettings(), b.loadSettings(), c.loadSettings(), d.loadSettings()])

        expect(
            countOf('get_effective_download_preferences'),
            'the canonical effective-preferences call must be issued once, not once per consumer'
        ).toBe(1)
        expect(countOf('get_unified_download_settings')).toBe(1)
    })

    it('still re-reads on a later, non-overlapping call', async () => {
        const { useDownloadSettings } = await import('@/composables/useDownloadSettings')
        const download = useDownloadSettings()

        await download.loadSettings()
        expect(countOf('get_effective_download_preferences')).toBe(1)

        // A later load must hit the backend again: this is the guarantee that
        // keeps the guard from turning into a stale-data cache.
        await download.loadSettings()
        expect(countOf('get_effective_download_preferences')).toBe(2)
    })

    it('clears the in-flight guard after settling so a failure cannot wedge it', async () => {
        const { useDownloadSettings } = await import('@/composables/useDownloadSettings')
        const download = useDownloadSettings()

        await download.loadSettings()
        expect(download.isLoading.value).toBe(false)

        // Still usable afterwards, i.e. the shared promise was released.
        await download.loadSettings()
        expect(countOf('get_effective_download_preferences')).toBe(2)
        expect(download.isLoading.value).toBe(false)
    })
})