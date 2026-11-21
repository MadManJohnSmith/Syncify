/**
 * LibraryViewPerformance.spec.ts — L13 (#110): regresión de rendimiento de la
 * biblioteca.
 *
 * Cubre el contrato medible del issue:
 * - un scroll sostenido pide como máximo una página por límite (el caso de
 *   319 llamadas en 4 minutos queda acotado a una por página);
 * - una ráfaga de filtros asentada hace UNA consulta, y volver a un filtro ya
 *   visitado se sirve de la caché sin consultar;
 * - la cabecera conserva los contadores de la primera página al paginar;
 * - lista, grid y grupos solo renderizan la ventana visible con overscan;
 * - la búsqueda espera 250 ms desde la última tecla;
 * - el panel de detalle hace una consulta al abrirse, no una por fila.
 *
 * Sin red ni reloj real: `Date` se falsea para el throttle del scroll y el
 * debounce de búsqueda usa fake timers de Vitest.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import LibraryView from '@/views/LibraryView.vue'
import { mockInvoke, resetMocks } from '../setup'
import { resetLibraryPageInvalidationForTests } from '@/composables/libraryCacheInvalidation'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { libraryApi } from '@/api/library'
import type { LibraryTrack } from '@/api/types'

function createTestTrack(id: number, overrides: Partial<LibraryTrack> = {}): LibraryTrack {
    return {
        id,
        title: `Track ${id}`,
        artist_name: `Artist ${id}`,
        artist_id: null,
        album_name: `Album ${id}`,
        album_id: id,
        duration_ms: 180000,
        isrc: `TEST${id}`,
        services: 'Spotify',
        quality: '320kbps',
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

interface QueryLog {
    offset: number
    text: string | null
    downloadStatuses?: string[]
    sort?: string
    direction?: string
}

function counts(total: number, downloaded = 42) {
    return {
        total,
        favorites: 0,
        downloaded,
        inPlaylists: 0,
        albums: 0,
        artists: 0,
        byService: [],
        byAccount: [],
    }
}

async function mountLibrary() {
    const router = createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/library', name: 'Library', component: LibraryView },
            { path: '/library/albums', name: 'LibraryAlbums', component: { template: '<div />' } },
            { path: '/library/artists', name: 'LibraryArtists', component: { template: '<div />' } },
            { path: '/album/:id', name: 'AlbumDetail', component: { template: '<div />' } },
        ],
    })
    await router.push({ path: '/library' })
    await router.isReady()
    const wrapper = mount(LibraryView, { global: { plugins: [router] } })
    await flushPromises()
    await flushPromises()
    return { wrapper, router }
}

function listScroller(wrapper: ReturnType<typeof mount>) {
    const scroller = wrapper.find('.custom-scrollbar[style*="contain: strict"]')
    expect(scroller.exists()).toBe(true)
    const el = scroller.element as HTMLElement
    Object.defineProperty(el, 'scrollHeight', { value: 2000, configurable: true })
    Object.defineProperty(el, 'clientHeight', { value: 600, configurable: true })
    Object.defineProperty(el, 'scrollTop', { value: 1350, writable: true, configurable: true })
    return { scroller, el }
}

beforeEach(() => {
    resetMocks()
    resetLibraryPageInvalidationForTests()
    document.body.innerHTML = ''
})

afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
})

describe('L13 — scroll acotado (caso 319 llamadas en 4 minutos)', () => {
    it('320 eventos de scroll en 4 minutos piden una página por límite, no una consulta por evento', async () => {
        const queries: QueryLog[] = []
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                const query = (args?.query ?? {}) as { offset: number; limit?: number }
                queries.push(query as unknown as QueryLog)
                const offset = Number(query.offset ?? 0)
                const page = Array.from({ length: 100 }, (_, index) =>
                    createTestTrack(offset + index + 1))
                return {
                    tracks: page,
                    total: 600,
                    offset,
                    limit: 100,
                    has_more: offset < 500,
                    counts: offset === 0 ? counts(600) : null,
                }
            }
            return null
        })

        // Reloj falseado ANTES del montaje: Vue fija el sello de sus
        // listeners con `Date.now()` y Test Utils marca el evento con el
        // mismo reloj; falsearlo después haría que Vue descartara el evento
        // como duplicado. No se espera tiempo real: 4 min / 0,75 s = 320
        // eventos sobre el throttle de 120 ms.
        vi.useFakeTimers({ toFake: ['Date'] })
        const base = Date.UTC(2026, 10, 19, 10, 0, 0)
        vi.setSystemTime(base)
        const { wrapper } = await mountLibrary()
        const { scroller, el } = listScroller(wrapper)

        for (let index = 0; index < 320; index++) {
            vi.setSystemTime(base + index * 750)
            await scroller.trigger('scroll')
            await flushPromises()
        }
        vi.useRealTimers()

        const offsets = queries.map(query => query.offset)
        expect(offsets).toEqual([0, 100, 200, 300, 400, 500])
        expect(new Set(offsets).size).toBe(offsets.length)
        expect(queries.length).toBeLessThanOrEqual(6)
        wrapper.unmount()
    })

    it('los contadores de la cabecera salen de la primera página y se conservan al paginar', async () => {
        const queries: QueryLog[] = []
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                const query = (args?.query ?? {}) as { offset: number }
                queries.push(query as unknown as QueryLog)
                const offset = Number(query.offset ?? 0)
                return {
                    tracks: Array.from({ length: 100 }, (_, index) => createTestTrack(offset + index + 1)),
                    total: 200,
                    offset,
                    limit: 100,
                    has_more: offset === 0,
                    // Contrato del servidor: los contadores solo viajan en la
                    // primera página; al paginar llegan como null.
                    counts: offset === 0 ? counts(200, 42) : null,
                }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        const chip = () => wrapper.find('[data-testid="library-chip-downloaded"]')
        expect(chip().text()).toContain('42')

        const { scroller } = listScroller(wrapper)
        await scroller.trigger('scroll')
        await flushPromises()

        expect(queries.map(query => query.offset)).toEqual([0, 100])
        // Sin recálculo: el chip conserva el contador de la primera página.
        expect(chip().text()).toContain('42')
        wrapper.unmount()
    })
})

describe('L13 — filtros: una consulta por estado asentado y caché por clave', () => {
    it('una ráfaga síncrona de 20 cambios de filtro deja una sola consulta', async () => {
        const queries: QueryLog[] = []
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as QueryLog)
                return {
                    tracks: [createTestTrack(1, { download_status: 'downloaded' })],
                    total: 1,
                    offset: 0,
                    limit: 100,
                    has_more: false,
                    counts: counts(1, 1),
                }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        const initial = queries.length
        const vm = wrapper.vm as unknown as { toggleFilter: (id: string) => void }

        // 21 alternancias: el estado final (solo Descargadas) es distinto del
        // inicial, así que la ráfaga debe producir exactamente UNA consulta.
        for (let index = 0; index < 21; index++) {
            vm.toggleFilter(index % 2 === 0 ? 'downloaded' : 'favorites')
        }
        await flushPromises()

        expect(queries.length).toBe(initial + 1)
        wrapper.unmount()
    })

    it('volver a un filtro ya visitado se sirve de la caché: cero consultas nuevas', async () => {
        const queries: QueryLog[] = []
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as QueryLog)
                return {
                    tracks: [createTestTrack(1)],
                    total: 1,
                    offset: 0,
                    limit: 100,
                    has_more: false,
                    counts: counts(1),
                }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        const afterInitial = queries.length
        const vm = wrapper.vm as unknown as { toggleFilter: (id: string) => void }

        vm.toggleFilter('downloaded')
        await flushPromises()
        expect(queries.length).toBe(afterInitial + 1)
        expect(queries[queries.length - 1].downloadStatuses).toEqual(['downloaded'])

        vm.toggleFilter('downloaded')
        await flushPromises()
        // El estado vacío ya está cacheado: no hay consulta nueva.
        expect(queries.length).toBe(afterInitial + 1)
        wrapper.unmount()
    })

    it('la búsqueda espera 250 ms desde la última tecla y reutiliza la página cacheada', async () => {
        const queries: QueryLog[] = []
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                const query = (args?.query ?? {}) as QueryLog
                queries.push(query)
                return {
                    tracks: [createTestTrack(1, { title: query.text ? 'Found' : 'Initial' })],
                    total: 1,
                    offset: 0,
                    limit: 100,
                    has_more: false,
                    counts: counts(1),
                }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        const initial = queries.length
        const input = wrapper.find('input[placeholder^="Filter by title"]')

        vi.useFakeTimers()
        await input.setValue('mid')
        await input.setValue('midn')
        await input.setValue('midnight')
        await vi.advanceTimersByTimeAsync(249)
        expect(queries.length).toBe(initial)

        await vi.advanceTimersByTimeAsync(1)
        expect(queries.length).toBe(initial + 1)
        expect(queries[queries.length - 1].text).toBe('midnight')

        // Volver al texto vacío: la primera página sigue en la caché.
        await input.setValue('')
        await vi.advanceTimersByTimeAsync(250)
        expect(queries.length).toBe(initial + 1)
        expect(wrapper.text()).toContain('Initial')

        vi.useRealTimers()
        wrapper.unmount()
    })
})

describe('L13 — invalidación explícita de la caché por evento', () => {
    function mountCounting(titlePrefix: string) {
        const queries: QueryLog[] = []
        mockInvoke((cmd, args) => {
            if (cmd === 'query_library') {
                queries.push((args?.query ?? {}) as QueryLog)
                return {
                    tracks: [createTestTrack(queries.length, { title: `${titlePrefix} ${queries.length}` })],
                    total: 1,
                    offset: 0,
                    limit: 100,
                    has_more: false,
                    counts: counts(1),
                }
            }
            if (cmd === 'toggle_favorite') return true
            return null
        })
        return queries
    }

    it('un evento de sync invalida la caché y relee una vez', async () => {
        const queries = mountCounting('Version')
        const { wrapper } = await mountLibrary()
        const afterMount = queries.length
        expect(wrapper.text()).toContain(`Version ${afterMount}`)

        vi.useFakeTimers()
        const eventBus = useEventBus()
        await eventBus.emit(TauriEvents.SYNC_COMPLETE, { service: 'tidal' })
        await flushPromises()
        await vi.advanceTimersByTimeAsync(350)
        await flushPromises()

        expect(queries.length).toBe(afterMount + 1)
        expect(wrapper.text()).toContain(`Version ${afterMount + 1}`)
        vi.useRealTimers()
        wrapper.unmount()
    })

    it('solo el evento terminal de descarga invalida; el progreso intermedio no', async () => {
        const queries = mountCounting('Version')
        const { wrapper } = await mountLibrary()
        const afterMount = queries.length
        const eventBus = useEventBus()

        vi.useFakeTimers()
        await eventBus.emit(TauriEvents.DOWNLOAD_PROGRESS, { status: 'downloading', terminal: false })
        await flushPromises()
        await vi.advanceTimersByTimeAsync(400)
        await flushPromises()
        expect(queries.length).toBe(afterMount)

        await eventBus.emit(TauriEvents.DOWNLOAD_PROGRESS, { status: 'complete', terminal: true })
        await flushPromises()
        await vi.advanceTimersByTimeAsync(400)
        await flushPromises()
        expect(queries.length).toBe(afterMount + 1)
        vi.useRealTimers()
        wrapper.unmount()
    })

    it('una mutación de favorito desde la API invalida la caché de la vista', async () => {
        const queries = mountCounting('Version')
        const { wrapper } = await mountLibrary()
        const afterMount = queries.length

        vi.useFakeTimers()
        await libraryApi.toggleTrackFavorite(1)
        await flushPromises()
        await vi.advanceTimersByTimeAsync(400)
        await flushPromises()

        expect(queries.length).toBe(afterMount + 1)
        vi.useRealTimers()
        wrapper.unmount()
    })
})

describe('L13 — virtualización de lista, grid y grupos', () => {
    it('el grid renderiza solo la ventana visible de baldosas', async () => {
        const tracks = Array.from({ length: 120 }, (_, index) => createTestTrack(index + 1))
        mockInvoke((cmd) => {
            if (cmd === 'query_library') {
                return { tracks, total: tracks.length, offset: 0, limit: 100, has_more: false, counts: counts(tracks.length) }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        await wrapper.findAll('.view-toggle button')[1].trigger('click')
        await flushPromises()

        const tiles = wrapper.findAll('.grid-tile')
        expect(tiles.length).toBeGreaterThan(0)
        expect(tiles.length).toBeLessThan(120)
        // La fila visible está alineada con la altura del spacer.
        expect(wrapper.find('[data-testid="library-grid"]').exists()).toBe(true)
        wrapper.unmount()
    })

    it('los grupos se virtualizan: 200 artistas no renderizan 200 cabeceras', async () => {
        const tracks = Array.from({ length: 200 }, (_, index) =>
            createTestTrack(index + 1, {
                artist_name: `Artist ${index + 1}`,
                album_name: 'Shared Album',
                album_id: 1,
            }))
        mockInvoke((cmd) => {
            if (cmd === 'query_library') {
                return { tracks, total: tracks.length, offset: 0, limit: 100, has_more: false, counts: counts(tracks.length) }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        const vm = wrapper.vm as unknown as { groupBy: string }
        vm.groupBy = 'artist'
        await flushPromises()

        const headers = wrapper.findAll('.group-header')
        expect(headers.length).toBeGreaterThan(0)
        expect(headers.length).toBeLessThan(200)
        expect(wrapper.find('[data-testid="library-groups"]').exists()).toBe(true)
        wrapper.unmount()
    })

    it('un grupo expandido con 300 pistas solo renderiza su ventana visible', async () => {
        const tracks = [
            ...Array.from({ length: 300 }, (_, index) =>
                createTestTrack(index + 1, {
                    artist_name: 'Big Artist',
                    album_name: 'Big Album',
                    album_id: 1,
                })),
            ...Array.from({ length: 50 }, (_, index) =>
                createTestTrack(1000 + index, {
                    artist_name: `Small Artist ${index + 1}`,
                    album_name: 'Small Album',
                    album_id: 2,
                })),
        ]
        mockInvoke((cmd) => {
            if (cmd === 'query_library') {
                return { tracks, total: tracks.length, offset: 0, limit: 100, has_more: false, counts: counts(tracks.length) }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        const vm = wrapper.vm as unknown as { groupBy: string }
        vm.groupBy = 'artist'
        await flushPromises()

        const firstHeader = wrapper.findAll('.group-header')[0]
        expect(firstHeader.text()).toContain('Big Artist')
        await firstHeader.trigger('click')
        await flushPromises()

        const renderedTracks = wrapper.findAll('.group-track')
        expect(renderedTracks.length).toBeGreaterThan(0)
        expect(renderedTracks.length).toBeLessThan(300)
        wrapper.unmount()
    })
})

describe('L13 — el panel hace una consulta al abrirse, no una por fila', () => {
    it('200 filas no disparan consultas por fila y el panel consulta una vez', async () => {
        const commands: string[] = []
        const tracks = Array.from({ length: 200 }, (_, index) => createTestTrack(index + 1))
        mockInvoke((cmd, args) => {
            commands.push(cmd)
            if (cmd === 'query_library') {
                return { tracks, total: tracks.length, offset: 0, limit: 100, has_more: false, counts: counts(tracks.length) }
            }
            if (cmd === 'get_library_track_detail') {
                const trackId = Number((args as { trackId?: number })?.trackId ?? 1)
                return {
                    track: createTestTrack(trackId),
                    sources: [],
                    memberships: [],
                    download: null,
                }
            }
            return null
        })

        const { wrapper } = await mountLibrary()
        expect(commands.filter(command => command === 'get_library_track_detail')).toHaveLength(0)

        await wrapper.get('[data-testid="track-detail-open-1"]').trigger('click')
        await flushPromises()

        expect(commands.filter(command => command === 'get_library_track_detail')).toHaveLength(1)
        expect(wrapper.find('[data-testid="track-detail-panel"]').exists()).toBe(true)
        wrapper.unmount()
    })
})
