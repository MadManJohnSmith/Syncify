import { describe, it, expect, vi, beforeEach } from 'vitest'
import { useTempoAnalysis } from '@/composables/useTempoAnalysis'
import * as tempoApi from '@/api/tempo'

describe('useTempoAnalysis Composable (S173)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('initializes with default options', () => {
    const tempo = useTempoAnalysis()
    expect(tempo.isAnalyzing.value).toBe(false)
    expect(tempo.onlyMissing.value).toBe(true)
    expect(tempo.confidenceThreshold.value).toBe(40)
    expect(tempo.forceReanalyze.value).toBe(false)
    expect(tempo.currentProgress.value).toBe(null)
    expect(tempo.lastSummary.value).toBe(null)
  })

  it('runs analysis and updates summary on completion', async () => {
    const mockSummary: tempoApi.BpmAnalysisBatchSummary = {
      total: 10,
      analyzed: 8,
      skipped: 2,
      low_confidence: 0,
      failed: 0,
    }

    vi.spyOn(tempoApi, 'analyzeLibraryBpm').mockResolvedValue(mockSummary)
    vi.spyOn(tempoApi, 'listenBpmProgress').mockResolvedValue(() => {})

    const tempo = useTempoAnalysis()
    await tempo.startAnalysis()

    expect(tempoApi.analyzeLibraryBpm).toHaveBeenCalledWith({
      only_missing: true,
      confidence_threshold: 0.4,
      force: false,
      track_ids: undefined,
    })
    expect(tempo.lastSummary.value).toEqual(mockSummary)
    expect(tempo.isAnalyzing.value).toBe(false)
  })

  it('invokes cancelBpmAnalysis on cancel()', async () => {
    const cancelSpy = vi.spyOn(tempoApi, 'cancelBpmAnalysis').mockResolvedValue()
    const tempo = useTempoAnalysis()
    await tempo.cancel()
    expect(cancelSpy).toHaveBeenCalledTimes(1)
  })
})
