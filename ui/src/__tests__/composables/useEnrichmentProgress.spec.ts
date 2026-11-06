import { describe, expect, it, vi, beforeEach } from 'vitest'
import { effectScope } from 'vue'

const listeners: Array<(event: { payload: unknown }) => void> = []

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (_event: string, handler: (event: { payload: unknown }) => void) => {
    listeners.push(handler)
    return () => {
      const index = listeners.indexOf(handler)
      if (index >= 0) listeners.splice(index, 1)
    }
  }),
}))

import { useEnrichmentProgress } from '@/composables/useEnrichmentProgress'

describe('useEnrichmentProgress', () => {
  beforeEach(() => {
    listeners.length = 0
  })

  it('arranca con progreso inicial y lo sustituye por el evento tipado', async () => {
    const scope = effectScope()
    const useProgress = scope.run(() => useEnrichmentProgress())
    expect(useProgress).toBeDefined()

    await useProgress!.start()
    expect(useProgress!.progress.value?.total).toBe(0)

    listeners[0]({
      payload: {
        kind: 'musicbrainz',
        status: 'processing',
        total: 10,
        current: 4,
        enriched: 4,
        failed: 0,
        currentTrack: 'Artist - Title',
      },
    })

    expect(useProgress!.progress.value).toMatchObject({
      kind: 'musicbrainz',
      status: 'processing',
      total: 10,
      current: 4,
    })

    useProgress!.stop()
    expect(useProgress!.progress.value).toBeNull()
    scope.stop()
  })

  it('ignora eventos que no son del contrato', async () => {
    const scope = effectScope()
    const useProgress = scope.run(() => useEnrichmentProgress())!

    await useProgress.start()
    listeners[0]({ payload: { kind: 'lastfm_genre', current: 99 } })

    expect(useProgress.progress.value?.total).toBe(0)
    scope.stop()
  })

  it('desuscribe al cerrar el scope', async () => {
    const scope = effectScope()
    const useProgress = scope.run(() => useEnrichmentProgress())!

    await useProgress.start()
    expect(listeners).toHaveLength(1)

    scope.stop()
    expect(listeners).toHaveLength(0)
  })
})
