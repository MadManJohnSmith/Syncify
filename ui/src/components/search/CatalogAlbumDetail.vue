<template>
  <div class="space-y-3 pb-2" data-testid="catalog-album-detail">
    <div class="flex items-center gap-3">
      <button
        @click="emit('close')"
        class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight text-text-secondary"
        title="Back to catalog results"
        data-testid="catalog-detail-back"
      >
        <span class="material-symbols-outlined text-[20px]">arrow_back</span>
      </button>
      <h2 class="text-sm font-bold text-gray-900 dark:text-white truncate">Album</h2>
      <span class="text-[11px] px-1.5 py-0.5 rounded-full border border-gray-300/60 dark:border-border-dark text-text-secondary">
        {{ provider.provider }}
      </span>
    </div>

    <div class="flex items-start gap-4 rounded-xl border border-gray-200 dark:border-border-dark p-4">
      <TrackCover :src="album.cover_url" :alt="album.title" size-class="w-24 h-24" />
      <div class="min-w-0">
        <h3 class="text-lg font-bold text-gray-900 dark:text-white truncate" data-testid="catalog-album-title">{{ album.title }}</h3>
        <button
          v-if="album.artist"
          @click="emit('openArtist', album.artist)"
          class="text-sm text-text-secondary hover:text-primary hover:underline transition-colors"
          data-testid="catalog-album-artist"
        >{{ album.artist }}</button>
        <p v-else class="text-sm text-text-secondary">Unknown Artist</p>
        <p class="text-xs text-text-secondary mt-1">
          <template v-if="album.track_count !== null">{{ album.track_count }} tracks</template>
          <template v-if="album.track_count !== null && releaseYear"> • </template>
          <template v-if="releaseYear">{{ releaseYear }}</template>
        </p>
      </div>
    </div>

    <div class="rounded-xl border border-gray-200 dark:border-border-dark overflow-hidden">
      <h3 class="px-4 pt-3 pb-1 text-xs font-bold uppercase tracking-wide text-text-secondary">
        Tracks <span v-if="tracks.length">({{ tracks.length }})</span>
      </h3>

      <p v-if="loading" class="px-4 pb-3 text-sm text-text-secondary" data-testid="catalog-detail-loading">
        Loading tracks...
      </p>
      <p
        v-else-if="error"
        class="mx-4 mb-3 rounded-lg border border-rose-500/30 bg-rose-500/10 px-3 py-2 text-sm text-rose-600 dark:text-rose-400"
        data-testid="catalog-detail-error"
      >{{ error }}</p>
      <p
        v-else-if="sectionError"
        class="mx-4 mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400"
        data-testid="catalog-detail-section-error"
      >
        <strong>{{ catalogFailureLabel(sectionError.kind) }}:</strong> {{ sectionError.message }}
      </p>
      <p v-else-if="tracks.length === 0" class="px-4 pb-3 text-sm text-text-secondary" data-testid="catalog-detail-empty">
        No tracks for this album were found in the provider.
      </p>
      <ul v-else class="pb-2">
        <li
          v-for="(track, index) in tracks"
          :key="track.id"
          class="flex items-center gap-3 px-4 py-2 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors group"
          data-testid="catalog-detail-track"
        >
          <span class="w-6 text-sm text-text-secondary text-right shrink-0">{{ index + 1 }}</span>
          <div class="flex-1 min-w-0">
            <p class="font-medium text-sm text-gray-900 dark:text-white truncate">{{ track.title }}</p>
            <p class="text-xs text-text-secondary truncate">{{ track.artist || album.artist || 'Unknown Artist' }}</p>
          </div>
          <span class="text-xs text-text-secondary font-mono shrink-0">{{ formatDuration(track.duration_ms) }}</span>
          <div class="flex items-center gap-1 shrink-0 opacity-0 group-hover:opacity-100 focus-within:opacity-100">
            <button
              @click="emit('play', track)"
              class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight/60 text-text-secondary hover:text-primary"
              title="Play from catalog"
              data-testid="catalog-detail-play"
            >
              <span class="material-symbols-outlined text-[18px]">play_arrow</span>
            </button>
            <button
              @click="emit('queue', track)"
              class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight/60 text-text-secondary hover:text-primary"
              title="Add to play queue"
              data-testid="catalog-detail-queue"
            >
              <span class="material-symbols-outlined text-[18px]">queue_music</span>
            </button>
          </div>
        </li>
      </ul>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Ficha de álbum del catálogo (P10b, #409): pistas del álbum abierto con
 * reproducir/encolar como entradas efímeras. No descarga: eso es P17 (#148).
 */
import { computed } from 'vue'
import TrackCover from '@/components/TrackCover.vue'
import { catalogFailureLabel } from '@/utils/catalogFailure'
import type {
  CatalogAlbum,
  CatalogProviderSearch,
  CatalogSearchFailure,
  CatalogSection,
  CatalogTrack,
} from '@/api/types/catalogSearch'

const props = defineProps<{
  album: CatalogAlbum
  provider: CatalogProviderSearch
  section: CatalogSection<CatalogTrack> | null
  loading: boolean
  error: string | null
}>()

const emit = defineEmits<{
  close: []
  play: [track: CatalogTrack]
  queue: [track: CatalogTrack]
  openArtist: [name: string]
}>()

const tracks = computed(() => props.section?.page?.items ?? [])
const sectionError = computed<CatalogSearchFailure | null>(() => props.section?.error ?? null)

/** Marca Unix de Qobuz → año de publicación; sin fecha no se inventa. */
const releaseYear = computed(() => {
  if (!props.album.released_at) return null
  const date = new Date(props.album.released_at * 1000)
  return Number.isNaN(date.getTime()) ? null : date.getFullYear().toString()
})

function formatDuration(ms: number | null): string {
  if (!ms) return '0:00'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}
</script>
