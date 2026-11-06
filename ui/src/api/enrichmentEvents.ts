/**
 * Contrato tipado del canal `enrichment-progress`.
 *
 * El backend define la misma forma en `src-tauri/src/events.rs`. Cualquier
 * cambio debe hacerse en ambos lados. El decoder ignora eventos desconocidos
 * para no romper la UI si un productor nuevo añade otra variante.
 */

import { normalizeEnrichmentJobSummary } from './enrichment'
import type { EnrichmentJobSummary } from './types'

export type MusicBrainzStatus = 'started' | 'processing' | 'completed'

export interface MusicBrainzProgress {
  kind: 'musicbrainz'
  status: MusicBrainzStatus
  total: number
  current: number
  enriched: number
  failed: number
  currentTrack: string | null
}

export interface IncrementalSummaryEvent extends EnrichmentJobSummary {
  kind: 'incrementalSummary'
}

export type EnrichmentEvent = MusicBrainzProgress | IncrementalSummaryEvent

const STATUSES: readonly MusicBrainzStatus[] = ['started', 'processing', 'completed']

function asCount(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0 ? value : 0
}

export function parseEnrichmentEvent(payload: unknown): EnrichmentEvent | null {
  if (payload === null || typeof payload !== 'object') return null

  const record = payload as Record<string, unknown>

  if (record.kind === 'incrementalSummary') {
    return {
      kind: 'incrementalSummary',
      ...normalizeEnrichmentJobSummary(record),
    }
  }

  if (record.kind !== 'musicbrainz') return null
  if (
    typeof record.status !== 'string' ||
    !(STATUSES as readonly string[]).includes(record.status)
  ) {
    return null
  }

  return {
    kind: 'musicbrainz',
    status: record.status as MusicBrainzStatus,
    total: asCount(record.total),
    current: asCount(record.current),
    enriched: asCount(record.enriched),
    failed: asCount(record.failed),
    currentTrack: typeof record.currentTrack === 'string' ? record.currentTrack : null,
  }
}
