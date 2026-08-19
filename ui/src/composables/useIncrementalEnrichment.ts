import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  EnrichmentMode,
  EnrichmentPreview,
  EnrichmentJobSummary,
} from '@/api/types'

export function useIncrementalEnrichment() {
  const selectedMode = ref<EnrichmentMode>('incomplete_only')
  const preview = ref<EnrichmentPreview | null>(null)
  const isPreviewLoading = ref(false)
  const isRunning = ref(false)
  const jobSummary = ref<EnrichmentJobSummary | null>(null)
  const errorMessage = ref<string | null>(null)
  let unlistenProgress: UnlistenFn | null = null

  const progressPercent = computed(() => {
    if (!jobSummary.value || jobSummary.value.totalTracks === 0) return 0
    return Math.min(
      100,
      Math.round((jobSummary.value.processedTracks / jobSummary.value.totalTracks) * 100)
    )
  })

  async function fetchPreview(mode?: EnrichmentMode, trackIds?: number[]) {
    const targetMode = mode || selectedMode.value
    isPreviewLoading.value = true
    errorMessage.value = null
    try {
      const res = await invoke<EnrichmentPreview>('preview_library_enrichment', {
        mode: targetMode,
        trackIds: trackIds || null,
      })
      preview.value = res
    } catch (err: unknown) {
      console.error('Failed to fetch enrichment preview:', err)
      errorMessage.value = err instanceof Error ? err.message : String(err)
    } finally {
      isPreviewLoading.value = false
    }
  }

  async function startEnrichment(mode?: EnrichmentMode, trackIds?: number[]) {
    const targetMode = mode || selectedMode.value
    isRunning.value = true
    errorMessage.value = null
    try {
      const res = await invoke<EnrichmentJobSummary>('start_library_enrichment', {
        mode: targetMode,
        trackIds: trackIds || null,
      })
      jobSummary.value = res
      await fetchPreview(targetMode, trackIds)
    } catch (err: unknown) {
      console.error('Failed to run incremental enrichment:', err)
      errorMessage.value = err instanceof Error ? err.message : String(err)
    } finally {
      isRunning.value = false
    }
  }

  async function cancelEnrichment() {
    try {
      await invoke('cancel_library_enrichment')
    } catch (err) {
      console.error('Failed to cancel enrichment:', err)
    }
  }

  async function fetchStatus() {
    try {
      const status = await invoke<EnrichmentJobSummary | null>('get_library_enrichment_status')
      if (status) {
        jobSummary.value = status
        isRunning.value = status.status === 'running' || status.status === 'queued'
      }
    } catch (err) {
      console.error('Failed to fetch enrichment status:', err)
    }
  }

  onMounted(async () => {
    try {
      unlistenProgress = await listen<EnrichmentJobSummary>('enrichment_progress', (event) => {
        jobSummary.value = event.payload
        isRunning.value = event.payload.status === 'running' || event.payload.status === 'queued'
      })
      await fetchPreview()
      await fetchStatus()
    } catch (err) {
      console.error('Failed to mount useIncrementalEnrichment:', err)
    }
  })

  onUnmounted(() => {
    if (unlistenProgress) {
      unlistenProgress()
    }
  })

  return {
    selectedMode,
    preview,
    isPreviewLoading,
    isRunning,
    jobSummary,
    errorMessage,
    progressPercent,
    fetchPreview,
    startEnrichment,
    cancelEnrichment,
    fetchStatus,
  }
}
