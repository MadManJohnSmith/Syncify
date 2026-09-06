import { ref, computed, onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { TauriEvents } from '@/api/tauri'
import {
  previewLibraryEnrichment,
  startLibraryEnrichment,
  cancelLibraryEnrichment,
  getLibraryEnrichmentStatus,
} from '@/api/enrichment'
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
      preview.value = await previewLibraryEnrichment(targetMode, trackIds || null)
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
      jobSummary.value = await startLibraryEnrichment(targetMode, trackIds || null)
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
      await cancelLibraryEnrichment()
    } catch (err) {
      console.error('Failed to cancel enrichment:', err)
    }
  }

  async function fetchStatus() {
    try {
      const status = await getLibraryEnrichmentStatus()
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
      unlistenProgress = await listen<EnrichmentJobSummary>(TauriEvents.ENRICHMENT_PROGRESS_ALT, (event) => {
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
