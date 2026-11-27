<template>
  <div class="space-y-3 pb-2" data-testid="catalog-artist-detail">
    <div class="flex items-center gap-3">
      <button
        @click="emit('close')"
        class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight text-text-secondary"
        title="Back to catalog results"
        data-testid="catalog-detail-back"
      >
        <span class="material-symbols-outlined text-[20px]">arrow_back</span>
      </button>
      <h2 class="text-sm font-bold text-gray-900 dark:text-white truncate">Artist</h2>
      <span class="text-[11px] px-1.5 py-0.5 rounded-full border border-gray-300/60 dark:border-border-dark text-text-secondary">
        {{ provider.provider }}
      </span>
    </div>

    <div class="flex items-center gap-4 rounded-xl border border-gray-200 dark:border-border-dark p-4">
      <TrackCover :src="imageUrl" :alt="name" size-class="w-20 h-20" rounded="rounded-full" icon="person" />
      <div class="min-w-0">
        <h3 class="text-lg font-bold text-gray-900 dark:text-white truncate" data-testid="catalog-artist-title">{{ name }}</h3>
        <p v-if="albumCount !== null" class="text-xs text-text-secondary mt-1">{{ albumCount }} albums</p>
      </div>
    </div>

    <div class="rounded-xl border border-gray-200 dark:border-border-dark overflow-hidden">
      <h3 class="px-4 pt-3 pb-1 text-xs font-bold uppercase tracking-wide text-text-secondary">
        Albums <span v-if="albums.length">({{ albums.length }})</span>
      </h3>

      <p v-if="loading" class="px-4 pb-3 text-sm text-text-secondary" data-testid="catalog-detail-loading">
        Loading albums...
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
      <p v-else-if="albums.length === 0" class="px-4 pb-3 text-sm text-text-secondary" data-testid="catalog-detail-empty">
        No albums for this artist were found in the provider.
      </p>
      <div v-else class="grid grid-cols-2 gap-2 px-4 pb-3">
        <button
          v-for="album in albums"
          :key="album.id"
          @click="emit('openAlbum', album)"
          class="flex items-center gap-2 rounded-lg border border-gray-200 dark:border-border-dark p-2 text-left hover:border-primary/60 transition-colors"
          data-testid="catalog-detail-album"
        >
          <TrackCover :src="album.cover_url" :alt="album.title" size-class="w-12 h-12" />
          <span class="min-w-0">
            <span class="block text-sm font-medium text-gray-900 dark:text-white truncate">{{ album.title }}</span>
            <span v-if="album.track_count !== null" class="block text-[11px] text-text-secondary">{{ album.track_count }} tracks</span>
          </span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
/**
 * Ficha de artista del catálogo (P10b, #409): álbumes del artista abierto.
 * El gateway no navega el catálogo completo; la lista sale de la búsqueda por
 * nombre, filtrada por el artista que la ficha muestra.
 */
import { computed } from 'vue'
import TrackCover from '@/components/TrackCover.vue'
import { catalogFailureLabel } from '@/utils/catalogFailure'
import type {
  CatalogAlbum,
  CatalogProviderSearch,
  CatalogSearchFailure,
  CatalogSection,
} from '@/api/types/catalogSearch'

const props = defineProps<{
  name: string
  imageUrl: string | null
  albumCount: number | null
  provider: CatalogProviderSearch
  section: CatalogSection<CatalogAlbum> | null
  loading: boolean
  error: string | null
}>()

const emit = defineEmits<{
  close: []
  openAlbum: [album: CatalogAlbum]
}>()

const albums = computed(() => props.section?.page?.items ?? [])
const sectionError = computed<CatalogSearchFailure | null>(() => props.section?.error ?? null)
</script>
