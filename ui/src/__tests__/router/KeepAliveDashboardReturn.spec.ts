// FE-PERF-01 (#49) + FE-PERF-04 (#51): comportamiento externo del KeepAlive
// acotado sobre el contrato unificado del Dashboard.
//
// Criterio de aceptación: volver a Dashboard NO repite su montaje ni sus
// llamadas. Se prueba montando la App real con el router real y contando las
// invocaciones IPC: la lectura unificada (`get_dashboard_snapshot`) debe
// correr UNA vez, y los comandos que el snapshot cubre (`create_library_snapshot`,
// `get_library_stats`, `get_metadata_stats`, `get_lyrics_stats`) NO se invocan
// como llamadas separadas. `get_top_artists` sigue siendo suya (el snapshot no
// lo cubre). Si la vista se remontara, la segunda visita repetiría todo.
//
// También se comprueba el límite de retención por el lado del comportamiento:
// LibraryView (incluida) conserva su instancia al volver (no repite
// `get_library`), y AlbumDetailView (excluida) SÍ remonta al reentrar.
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory, type Router } from 'vue-router'
import { routes } from '@/main'
import App from '@/App.vue'
import { mockInvoke, resetMocks } from '../setup'

const ipcCalls: string[] = []

function stubBackend() {
  mockInvoke((cmd: string) => {
    ipcCalls.push(cmd)
    if (cmd === 'get_worker_status')
      return { running: false, paused: false, active_downloads: 0, max_concurrent: 3 }
    if (cmd === 'get_queue_stats')
      return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
    if (cmd === 'get_dashboard_snapshot')
      return {
        stats: {
          total_tracks: 1, total_artists: 1, total_albums: 1, total_downloads: 0,
          queued_downloads: 0, active_downloads: 0, library_entries: 0,
          playlists: 0, services_with_data: 0,
        },
        snapshot: {
          id: 1, snapshot_date: '2026-01-01', total_tracks: 1, total_albums: 1,
          total_artists: 1, total_size_bytes: 0, tracks_with_lyrics: 0,
          tracks_lossless: 0, tracks_hires: 0, downloaded_tracks: 0,
        },
        snapshots: [],
        metadata: {
          total_tracks: 1, with_isrc: 0, with_musicbrainz_id: 0, with_album: 0,
          with_year: 0, with_genre: 0, with_art: 0, average_completeness: 0,
        },
        lyrics: { total_tracks: 1, with_lyrics: 0, synced_lyrics: 0, embedded_lyrics: 0 },
      }
    // Una cuenta en el arranque: con cero, la App aterriza en /accounts al
    // iniciar (#93, ADR-0007 decisión 15) y este escenario de KeepAlive de
    // Dashboard no es el primer arranque.
    if (cmd === 'get_service_statuses') return []
    if (cmd === 'get_accounts') return [{ id: 1, service_id: 2, is_active: true }]
    if (cmd === 'query_library') return { tracks: [], total: 0 }
    return []
  })
}

function countCalls(cmd: string): number {
  return ipcCalls.filter((c) => c === cmd).length
}

async function mountAppWithRouter(): Promise<{ wrapper: ReturnType<typeof mount>; router: Router }> {
  const router = createRouter({ history: createMemoryHistory(), routes })
  const wrapper = mount(App, { global: { plugins: [router] } })
  await router.push('/')
  await router.isReady()
  await flushPromises()
  return { wrapper, router }
}

async function navigate(router: Router, path: string) {
  await router.push(path)
  await flushPromises()
}

describe('KeepAlive acotado — volver no remonta (FE-PERF-01)', () => {
  beforeEach(() => {
    resetMocks()
    ipcCalls.length = 0
    stubBackend()
  })

  it('volver a Dashboard no repite su montaje ni sus llamadas', async () => {
    const { wrapper, router } = await mountAppWithRouter()

    // Primera visita: Dashboard monta y lanza la lectura unificada (una vez).
    // FE-PERF-04: los recuentos equivalentes van en `get_dashboard_snapshot`;
    // sus comandos predecesores no se invocan como llamadas separadas.
    expect(countCalls('get_dashboard_snapshot')).toBe(1)
    expect(countCalls('create_library_snapshot')).toBe(0)
    expect(countCalls('get_library_stats')).toBe(0)
    expect(countCalls('get_metadata_stats')).toBe(0)
    expect(countCalls('get_lyrics_stats')).toBe(0)
    expect(countCalls('get_top_artists')).toBe(1)

    await navigate(router, '/library')
    await navigate(router, '/downloads')
    await navigate(router, '/dashboard')
    await flushPromises()

    // Segunda visita: la vista se reactiva del KeepAlive; ningún comando de
    // Dashboard se repite (el montaje no vuelve a correr).
    expect(countCalls('get_dashboard_snapshot')).toBe(1)
    expect(countCalls('create_library_snapshot')).toBe(0)
    expect(countCalls('get_library_stats')).toBe(0)
    expect(countCalls('get_top_artists')).toBe(1)
    wrapper.unmount()
  })

  it('LibraryView incluida conserva su instancia: volver no vuelve a pedir la biblioteca', async () => {
    const { wrapper, router } = await mountAppWithRouter()

    await navigate(router, '/library')
    const libraryCallsOnFirstVisit = countCalls('query_library')
    expect(libraryCallsOnFirstVisit).toBeGreaterThan(0)

    await navigate(router, '/downloads')
    await navigate(router, '/library')
    await flushPromises()

    // Reactivación desde cache: ni un `get_library` más.
    expect(countCalls('query_library')).toBe(libraryCallsOnFirstVisit)
    wrapper.unmount()
  })

  it('AlbumDetailView está excluida del cache: remonta al reentrar', async () => {
    const { wrapper, router } = await mountAppWithRouter()

    await navigate(router, '/album/1')
    const callsAtFirstVisit = ipcCalls.length
    await navigate(router, '/dashboard')
    await navigate(router, '/album/1')
    await flushPromises()

    // Fuera del include: la segunda visita remonta la vista de nuevo y hace
    // al menos una llamada IPC nueva de carga.
    expect(ipcCalls.length).toBeGreaterThan(callsAtFirstVisit)
    wrapper.unmount()
  })
})
