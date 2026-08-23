import { ref, onUnmounted, getCurrentInstance } from 'vue'
import {
  analyzeLibraryBpm,
  cancelBpmAnalysis,
  listenBpmProgress,
  type BpmProgressEvent,
  type BpmAnalysisBatchSummary,
} from '@/api/tempo'
import type { UnlistenFn } from '@tauri-apps/api/event'

export function useTempoAnalysis() {
  const isAnalyzing = ref(false)
  const onlyMissing = ref(true)
  const confidenceThreshold = ref(40) // 0 - 100%
  const forceReanalyze = ref(false)
  
  const currentProgress = ref<BpmProgressEvent | null>(null)
  const lastSummary = ref<BpmAnalysisBatchSummary | null>(null)
  const errorMessage = ref<string | null>(null)
  let unlistenFn: UnlistenFn | null = null

  async function startAnalysis(trackIds?: number[]) {
    if (isAnalyzing.value) return

    isAnalyzing.value = true
    errorMessage.value = null
    currentProgress.value = null
    lastSummary.value = null

    try {
      if (!unlistenFn) {
        unlistenFn = await listenBpmProgress((evt) => {
          currentProgress.value = evt
        })
      }

      const summary = await analyzeLibraryBpm({
        only_missing: onlyMissing.value,
        confidence_threshold: confidenceThreshold.value / 100.0,
        force: forceReanalyze.value,
        track_ids: trackIds,
      })

      lastSummary.value = summary
    } catch (err: any) {
      errorMessage.value = err?.message || String(err)
    } finally {
      isAnalyzing.value = false
    }
  }

  async function cancel() {
    try {
      await cancelBpmAnalysis()
    } catch (err) {
      console.error('Failed to cancel BPM analysis:', err)
    }
  }

  if (getCurrentInstance()) {
    onUnmounted(() => {
      if (unlistenFn) {
        unlistenFn()
        unlistenFn = null
      }
    })
  }

  return {
    isAnalyzing,
    onlyMissing,
    confidenceThreshold,
    forceReanalyze,
    currentProgress,
    lastSummary,
    errorMessage,
    startAnalysis,
    cancel,
  }
}
