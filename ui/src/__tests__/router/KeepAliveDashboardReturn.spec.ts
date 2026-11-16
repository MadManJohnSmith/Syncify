// FE-PERF-01 (#49): comportamiento externo del KeepAlive acotado.
//
// Criterio de aceptación: volver a Dashboard NO repite su montaje ni sus
// llamadas. Se prueba montando la App real con el router real y contando las
// invocaciones IPC de los comandos que sólo Dashboard lanza
// (`create_library_snapshot`, `get_library_stats`): si la vista se remontara,
// la segunda visita los repetiría.
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
    if (cmd === 'get_library_stats')
      return {
        total_tracks: 1, total_albums: 1, total_artists: 1,
        playlists: 0, total_downloads: 0, queued_downloads: 0, active_downloads: 0,
      }
    if (cmd === 'get_service_statuses' || cmd === 'get_accounts') return []
    if (cmd === 'get_library') return { tracks: [], total: 0 }
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

    // Primera visita: Dashboard monta y lanza snapshot + stats (una vez).
    expect(countCalls('create_library_snapshot')).toBe(1)
    expect(countCalls('get_library_stats')).toBe(1)
    expect(countCalls('get_top_artists')).toBe(1)

    await navigate(router, '/library')
    await navigate(router, '/downloads')
    await navigate(router, '/dashboard')
    await flushPromises()

    // Segunda visita: la vista se reactiva del KeepAlive; ningún comando de
    // Dashboard se repite (el montaje no vuelve a correr).
    expect(countCalls('create_library_snapshot')).toBe(1)
    expect(countCalls('get_library_stats')).toBe(1)
    expect(countCalls('get_top_artists')).toBe(1)
    expect(countCalls('get_metadata_stats')).toBe(1)
    expect(countCalls('get_lyrics_stats')).toBe(1)
    wrapper.unmount()
  })

  it('LibraryView incluida conserva su instancia: volver no vuelve a pedir la biblioteca', async () => {
    const { wrapper, router } = await mountAppWithRouter()

    await navigate(router, '/library')
    const libraryCallsOnFirstVisit = countCalls('get_library')
    expect(libraryCallsOnFirstVisit).toBeGreaterThan(0)

    await navigate(router, '/downloads')
    await navigate(router, '/library')
    await flushPromises()

    // Reactivación desde cache: ni un `get_library` más.
    expect(countCalls('get_library')).toBe(libraryCallsOnFirstVisit)
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
