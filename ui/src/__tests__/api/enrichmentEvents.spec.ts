import { describe, expect, it } from 'vitest'
import { parseEnrichmentEvent } from '@/api/enrichmentEvents'

describe('parseEnrichmentEvent', () => {
  it('acepta el evento canónico de MusicBrainz', () => {
    expect(
      parseEnrichmentEvent({
        kind: 'musicbrainz',
        status: 'processing',
        total: 120,
        current: 40,
        enriched: 38,
        failed: 2,
        currentTrack: 'Artist - Title',
      })
    ).toEqual({
      kind: 'musicbrainz',
      status: 'processing',
      total: 120,
      current: 40,
      enriched: 38,
      failed: 2,
      currentTrack: 'Artist - Title',
    })
  })

  it('normaliza contadores inválidos y currentTrack ausente', () => {
    expect(
      parseEnrichmentEvent({
        kind: 'musicbrainz',
        status: 'started',
        total: Number.NaN,
        current: -1,
        enriched: '3',
        failed: Infinity,
      })
    ).toMatchObject({
      total: 0,
      current: 0,
      enriched: 0,
      failed: 0,
      currentTrack: null,
    })
  })

  it('acepta el resumen incremental y lo normaliza', () => {
    const parsed = parseEnrichmentEvent({
      kind: 'incrementalSummary',
      jobId: 'job-1',
      mode: 'incomplete_only',
      status: 'running',
      totalTracks: 900,
      processedTracks: 3,
      modifiedTracks: 1,
      skippedCompleteTracks: 0,
      skippedPrecedenceTracks: 0,
      failedTracks: 0,
      currentTrack: 'Otra - Pista',
      currentPhase: 'Enriching',
      items: [],
      availableSources: [],
      startedAt: '2026-11-05T00:00:00Z',
      completedAt: null,
    })

    expect(parsed).toMatchObject({
      kind: 'incrementalSummary',
      jobId: 'job-1',
      status: 'running',
      totalTracks: 900,
      processedTracks: 3,
    })
  })

  it('ignora un kind desconocido', () => {
    expect(parseEnrichmentEvent({ kind: 'lastfm_genre', status: 'started' })).toBeNull()
    expect(parseEnrichmentEvent({ type: 'lastfm_genre', status: 'started' })).toBeNull()
  })

  it('ignora un status desconocido', () => {
    expect(parseEnrichmentEvent({ kind: 'musicbrainz', status: 'progress' })).toBeNull()
  })

  it('ignora payloads que no son objetos', () => {
    expect(parseEnrichmentEvent(null)).toBeNull()
    expect(parseEnrichmentEvent('musicbrainz')).toBeNull()
    expect(parseEnrichmentEvent(42)).toBeNull()
  })
})
