import { onScopeDispose, ref, type Ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { TauriEvents } from '@/api/tauri'
import { parseEnrichmentEvent, type MusicBrainzProgress } from '@/api/enrichmentEvents'

const INITIAL_PROGRESS: MusicBrainzProgress = {
  kind: 'musicbrainz',
  status: 'started',
  total: 0,
  current: 0,
  enriched: 0,
  failed: 0,
  currentTrack: 'Enriching…',
}

export interface UseEnrichmentProgress {
  progress: Ref<MusicBrainzProgress | null>
  start: () => Promise<void>
  stop: () => void
}

/**
 * Suscribe la vista al contrato `enrichment-progress` y limpia la escucha al
 * desmontar. La vista solo lee `progress`; no conoce el payload IPC.
 */
export function useEnrichmentProgress(): UseEnrichmentProgress {
  const progress = ref<MusicBrainzProgress | null>(null)
  let unlisten: UnlistenFn | null = null
  let generation = 0

  function stop(): void {
    generation += 1
    unlisten?.()
    unlisten = null
    progress.value = null
  }

  async function start(): Promise<void> {
    stop()
    const current = generation
    progress.value = { ...INITIAL_PROGRESS }

    const off = await listen<unknown>(TauriEvents.ENRICHMENT_PROGRESS, (event) => {
      const parsed = parseEnrichmentEvent(event.payload)
      if (parsed?.kind === 'musicbrainz') progress.value = parsed
    })

    if (current !== generation) {
      off()
      return
    }
    unlisten = off
  }

  onScopeDispose(stop)

  return { progress, start, stop }
}
