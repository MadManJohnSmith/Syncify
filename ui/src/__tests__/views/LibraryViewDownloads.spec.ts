/**
 * LibraryViewDownloads.spec.ts — L14 (issue #109): descargas y calidad en la
 * biblioteca.
 *
 * Criterios cubiertos:
 * - los cinco estados de descarga de la fila, cada uno con su badge;
 * - la calidad efectiva del fichero en la fila (sin consultar por pista);
 * - filtros de estado y de calidad en la consulta del servidor;
 * - encolar usa `add_to_queue` y re-descargar usa la ruta existente
 *   `force_redownload_tracks` (no una segunda cola);
 * - borrar audio es por pista, pide confirmación y no toca la pista;
 * - una sola consulta `query_library` alimenta la página (sin N+1).
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import LibraryView from '@/views/LibraryView.vue'
import { mockInvoke, resetMocks } from '../setup'
import type { LibraryTrack } from '@/api/types'

function createTrack(id: number, overrides: Partial<LibraryTrack> = {}): LibraryTrack {
    return {
        id,
        title: `Track ${id}`,
        artist_name: 'Artist',
        artist_id: null,
        album_name: 'Album',
        album_id: null,
        duration_ms: 180000,
        isrc: `TEST${id}`,
        services: 'Spotify',
        quality: 'FLAC',
        download_status: 'not_downloaded',
        metadata_score: 80,
        lyrics_type: 'none',
        cover_art_url: null,
        track_number: null,
        disc_number: null,
        genre: null,
        bpm: null,
        musical_key: null,
        release_year: null,
        explicit: null,
        file_path: null,
        musicbrainz_id: null,
        ...overrides,
    }
}

function makePage(tracks: LibraryTrack[], hasMore = false) {
    return { tracks, total: tracks.length, offset: 0, limit: 100, has_more: hasMore }
}

type InvokeCall = { command: string; args?: Record<string, unknown> }

async function mountLibrary(
    tracks: LibraryTrack[],
    handler?: (cmd: string, args?: Record<string, unknown>) => unknown
) {
    const calls: InvokeCall[] = []
    mockInvoke((cmd, args) => {
        calls.push({ command: cmd, args })
        const extra = handler?.(cmd, args)
        if (extra !== undefined) return extra
        if (cmd === 'query_library') return makePage(tracks)
        return null
    })
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [{ path: '/library', name: 'Library', component: LibraryView }],
    })
    await router.push('/library')
    await router.isReady()
    const wrapper = mount(LibraryView, {
        attachTo: document.body,
        global: { plugins: [router] },
    })
    await flushPromises()
    await flushPromises()
    return { wrapper, calls, router }
}

async function settle(wrapper: ReturnType<typeof mount>) {
    await flushPromises()
    await wrapper.vm.$nextTick()
    await flushPromises()
}

function findPill(wrapper: ReturnType<typeof mount>, label: string) {
    return wrapper.findAll('.filter-pill').find((pill) => pill.text().includes(label))
}

function lastQuery(calls: InvokeCall[]): Record<string, unknown> {
    const queries = calls.filter((call) => call.command === 'query_library')
    expect(queries.length).toBeGreaterThan(0)
    return (queries[queries.length - 1].args?.query ?? {}) as Record<string, unknown>
}

async function openContextMenu(wrapper: ReturnType<typeof mount>, trackId: number) {
    const row = wrapper.find(`[data-track-id="${trackId}"]`)
    expect(row.exists()).toBe(true)
    await row.trigger('contextmenu')
    await settle(wrapper)
}

describe('LibraryView — descargas y calidad (L14, #109)', () => {
    beforeEach(() => {
        resetMocks()
        document.body.innerHTML = ''
    })

    afterEach(() => {
        vi.restoreAllMocks()
    })

    it('muestra los cinco estados y la calidad efectiva sin una consulta por pista', async () => {
        const tracks = [
            createTrack(1, { title: 'Downloaded', download_status: 'downloaded', quality: 'FLAC', download_quality: 'FLAC 24/96' }),
            createTrack(2, { title: 'Queued', download_status: 'queued' }),
            createTrack(3, { title: 'Downloading', download_status: 'downloading' }),
            createTrack(4, { title: 'Failed', download_status: 'failed' }),
            createTrack(5, { title: 'Idle', download_status: 'not_downloaded' }),
        ]
        const { wrapper, calls } = await mountLibrary(tracks)

        for (const status of ['downloaded', 'queued', 'downloading', 'failed', 'not_downloaded']) {
            const badge = wrapper.find(`[data-testid="track-download-badge"][data-download-status="${status}"]`)
            expect(badge.exists(), status).toBe(true)
        }

        const quality = wrapper.find('[data-testid="track-quality-badge"]')
        expect(quality.exists()).toBe(true)
        expect(quality.text()).toContain('FLAC 24/96')
        // La calidad sale de la MISMA página: una sola llamada para N filas.
        expect(calls.filter((call) => call.command === 'query_library')).toHaveLength(1)
        wrapper.unmount()
    })

    it('filtra por cada estado de descarga en el servidor', async () => {
        const tracks = [createTrack(1)]
        const { wrapper, calls } = await mountLibrary(tracks)

        const cases: Array<[string, string]> = [
            ['Downloaded', 'downloaded'],
            ['Queued', 'queued'],
            ['Downloading', 'downloading'],
            ['Failed', 'failed'],
            ['Not Downloaded', 'not_downloaded'],
        ]
        for (const [label, status] of cases) {
            const pill = findPill(wrapper, label)
            expect(pill, label).toBeTruthy()
            await pill!.trigger('click')
            await settle(wrapper)
            expect(lastQuery(calls).downloadStatuses, label).toEqual([status])
            // Deselecciona para que el siguiente caso pruebe su estado solo.
            await findPill(wrapper, label)!.trigger('click')
            await settle(wrapper)
        }
        wrapper.unmount()
    })

    it('filtra por calidad con los buckets del servidor', async () => {
        const tracks = [createTrack(1)]
        const { wrapper, calls } = await mountLibrary(tracks)

        await findPill(wrapper, 'Hi-Res (FLAC)')!.trigger('click')
        await settle(wrapper)
        expect(lastQuery(calls).qualityBuckets).toEqual(['hi_res', 'cd_quality'])
        await findPill(wrapper, 'Hi-Res (FLAC)')!.trigger('click')
        await settle(wrapper)

        await findPill(wrapper, 'Lossy (MP3)')!.trigger('click')
        await settle(wrapper)
        expect(lastQuery(calls).qualityBuckets).toEqual(['lossy'])
        wrapper.unmount()
    })

    it('encola por la cola existente y re-descarga por la ruta existente', async () => {
        const tracks = [
            createTrack(1, { title: 'Idle', download_status: 'not_downloaded' }),
            createTrack(2, { title: 'Downloaded', download_status: 'downloaded', download_quality: 'FLAC 16/44.1' }),
        ]
        const { wrapper, calls } = await mountLibrary(tracks, (cmd) => {
            if (cmd === 'add_to_queue') return 101
            if (cmd === 'force_redownload_tracks') return 1
            return undefined
        })

        const downloadButton = wrapper.find('[data-track-id="1"] button[title="Download"]')
        expect(downloadButton.exists()).toBe(true)
        await downloadButton.trigger('click')
        await settle(wrapper)
        const enqueue = calls.find((call) => call.command === 'add_to_queue')
        expect(enqueue?.args).toMatchObject({ trackId: 1 })

        // Repetir la acción sigue entrando por el MISMO comando: la
        // deduplicación la resuelve el guardrail de la cola
        // (`perform_add_to_queue`), no una ruta alternativa de la vista.
        await downloadButton.trigger('click')
        await settle(wrapper)
        const enqueueCalls = calls.filter((call) => call.command === 'add_to_queue')
        expect(enqueueCalls).toHaveLength(2)
        expect(enqueueCalls.every((call) => call.args?.trackId === 1)).toBe(true)

        await openContextMenu(wrapper, 2)
        const reDownload = document.querySelector('[data-testid="context-re-download"]') as HTMLButtonElement | null
        expect(reDownload).not.toBeNull()
        reDownload!.click()
        await settle(wrapper)

        const forceCall = calls.find((call) => call.command === 'force_redownload_tracks')
        expect(forceCall?.args).toMatchObject({ trackIds: [2] })
        // No hay una segunda ruta de descarga: los mismos comandos de la cola
        // existente, nunca un motor nuevo desde la biblioteca.
        const newDownloadRoutes = calls
            .map((call) => call.command)
            .filter((cmd) => ['queue_downloads', 'enqueue_tracks', 'download_tidal_single_track'].includes(cmd))
        expect(newDownloadRoutes).toEqual([])
        wrapper.unmount()
    })

    it('borrar audio pide confirmación, elimina el Download y conserva la pista', async () => {
        let audioDeleted = false
        const tracks = [
            createTrack(1, {
                title: 'Downloaded',
                download_status: 'downloaded',
                download_quality: 'FLAC 24/96',
                downloaded_from: 'Qobuz',
                file_path: '/music/Track 1.flac',
            }),
        ]
        const { wrapper, calls } = await mountLibrary(tracks, (cmd) => {
            if (cmd === 'query_library') {
                // Tras el borrado, la página canónica devuelve la MISMA pista
                // sin Download: el Track sigue en la biblioteca.
                return makePage(audioDeleted
                    ? [createTrack(1, { title: 'Downloaded', download_status: 'not_downloaded' })]
                    : tracks)
            }
            if (cmd === 'delete_track_audio') {
                audioDeleted = true
                return { queueEntriesDeleted: 0, downloadDeleted: true, audioFileDeleted: true }
            }
            return undefined
        })
        const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true)

        await openContextMenu(wrapper, 1)
        const deleteButton = document.querySelector('[data-testid="context-delete-audio"]') as HTMLButtonElement | null
        expect(deleteButton).not.toBeNull()
        deleteButton!.click()
        await settle(wrapper)

        expect(confirmSpy).toHaveBeenCalledWith(expect.stringContaining('Downloaded'))
        const deleteCall = calls.find((call) => call.command === 'delete_track_audio')
        expect(deleteCall?.args).toEqual({ trackId: 1 })
        // La fila vuelve a la lista (recarga de la página), no se elimina la
        // pista de la biblioteca.
        expect(wrapper.find('[data-track-id="1"]').exists()).toBe(true)
        expect(wrapper.find('[data-testid="track-download-badge"][data-download-status="not_downloaded"]').exists()).toBe(true)
        wrapper.unmount()
    })

    it('cancelar la confirmación no llama a borrar audio', async () => {
        const tracks = [createTrack(1, { download_status: 'downloaded' })]
        const { wrapper, calls } = await mountLibrary(tracks)
        vi.spyOn(window, 'confirm').mockReturnValue(false)

        await openContextMenu(wrapper, 1)
        const deleteButton = document.querySelector('[data-testid="context-delete-audio"]') as HTMLButtonElement | null
        deleteButton!.click()
        await settle(wrapper)

        expect(calls.filter((call) => call.command === 'delete_track_audio')).toEqual([])
        wrapper.unmount()
    })
})
