<template>
  <div class="space-y-4 pb-2" data-testid="catalog-provider-results">
    <!-- Búsqueda completa fallida: no es "sin resultados" -->
    <div v-if="error" class="py-10 text-center text-text-secondary" data-testid="catalog-search-error">
      <span class="material-symbols-outlined text-5xl mb-4 text-error">cloud_off</span>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-1">Catalog search failed</h3>
      <p class="mb-4">{{ error }}</p>
      <button
        @click="emit('retry')"
        class="px-3 py-1.5 bg-primary text-on-accent rounded-lg text-sm"
        data-testid="catalog-retry"
      >
        Try Again
      </button>
    </div>

    <!-- Espera del gateway -->
    <div
      v-else-if="loading && !response"
      class="py-10 text-center text-text-secondary"
      data-testid="catalog-search-loading"
    >
      <span class="material-symbols-outlined animate-spin text-4xl mb-4 text-primary block">progress_activity</span>
      <p>Searching providers for "{{ query }}"...</p>
    </div>

    <!-- Sin resultados en ningún proveedor -->
    <div
      v-else-if="response && response.providers.length === 0"
      class="py-10 text-center text-text-secondary"
      data-testid="catalog-no-providers"
    >
      <span class="material-symbols-outlined text-5xl mb-4">cloud_off</span>
      <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-1">No providers available</h3>
      <p>Connect a catalog provider account to search outside your library.</p>
    </div>

    <template v-else-if="response">
      <section
        v-for="provider in response.providers"
        :key="provider.provider"
        class="rounded-xl border border-gray-200 dark:border-border-dark overflow-hidden"
        data-testid="catalog-provider"
        :data-provider="provider.provider"
      >
        <header class="flex items-center justify-between gap-3 px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight/40">
          <div class="flex items-center gap-2 min-w-0">
            <h2 class="text-sm font-bold text-gray-900 dark:text-white truncate">{{ provider.provider }}</h2>
            <span
              class="text-[11px] px-1.5 py-0.5 rounded-full border"
              :class="statusClass(provider)"
              data-testid="catalog-provider-status"
            >{{ statusLabel(provider) }}</span>
          </div>
          <span v-if="provider.account_id !== null" class="text-[11px] text-text-secondary shrink-0">
            account #{{ provider.account_id }}
          </span>
        </header>

        <div class="divide-y divide-gray-100 dark:divide-gray-800">
          <!-- Pistas -->
          <div v-if="hasSection(provider.tracks)" data-testid="catalog-section-tracks">
            <h3 class="px-4 pt-3 pb-1 text-xs font-bold uppercase tracking-wide text-text-secondary">
              Tracks <span v-if="items(provider.tracks).length">({{ items(provider.tracks).length }})</span>
            </h3>
            <p
              v-if="sectionError(provider.tracks)"
              class="mx-4 mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400"
              data-testid="catalog-section-error"
            >
              <strong>{{ catalogFailureLabel(sectionError(provider.tracks)!.kind) }}:</strong>
              {{ sectionError(provider.tracks)!.message }}
            </p>
            <p v-else-if="items(provider.tracks).length === 0" class="px-4 pb-3 text-sm text-text-secondary">
              No tracks in this provider.
            </p>
            <ul v-else class="pb-2">
              <li
                v-for="track in items(provider.tracks)"
                :key="track.id"
                class="flex items-center gap-3 px-4 py-2 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors group"
                data-testid="catalog-track"
              >
                <TrackCover :src="null" :alt="track.album || track.title" size-class="w-9 h-9" />
                <div class="flex-1 min-w-0">
                  <p class="font-medium text-sm text-gray-900 dark:text-white truncate">{{ track.title }}</p>
                  <p class="text-xs text-text-secondary truncate">
                    {{ track.artist || 'Unknown Artist' }}<template v-if="track.album"> • {{ track.album }}</template>
                  </p>
                </div>
                <span class="text-xs text-text-secondary font-mono shrink-0">{{ formatDuration(track.duration_ms) }}</span>
                <div class="flex items-center gap-1 shrink-0 opacity-0 group-hover:opacity-100 focus-within:opacity-100">
                  <button
                    @click="emit('play', track, provider)"
                    class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight/60 text-text-secondary hover:text-primary"
                    title="Play from catalog"
                    data-testid="catalog-play-track"
                  >
                    <span class="material-symbols-outlined text-[18px]">play_arrow</span>
                  </button>
                  <button
                    @click="emit('queue', track, provider)"
                    class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight/60 text-text-secondary hover:text-primary"
                    title="Add to play queue"
                    data-testid="catalog-queue-track"
                  >
                    <span class="material-symbols-outlined text-[18px]">queue_music</span>
                  </button>
                </div>
              </li>
            </ul>
          </div>

          <!-- Álbumes -->
          <div v-if="hasSection(provider.albums)" data-testid="catalog-section-albums">
            <h3 class="px-4 pt-3 pb-1 text-xs font-bold uppercase tracking-wide text-text-secondary">
              Albums <span v-if="items(provider.albums).length">({{ items(provider.albums).length }})</span>
            </h3>
            <p
              v-if="sectionError(provider.albums)"
              class="mx-4 mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400"
              data-testid="catalog-section-error"
            >
              <strong>{{ catalogFailureLabel(sectionError(provider.albums)!.kind) }}:</strong>
              {{ sectionError(provider.albums)!.message }}
            </p>
            <p v-else-if="items(provider.albums).length === 0" class="px-4 pb-3 text-sm text-text-secondary">
              No albums in this provider.
            </p>
            <div v-else class="grid grid-cols-2 gap-2 px-4 pb-3">
              <button
                v-for="album in items(provider.albums)"
                :key="album.id"
                @click="emit('openAlbum', album, provider)"
                class="flex items-center gap-2 rounded-lg border border-gray-200 dark:border-border-dark p-2 text-left hover:border-primary/60 transition-colors"
                data-testid="catalog-album"
              >
                <TrackCover :src="album.cover_url" :alt="album.title" size-class="w-12 h-12" />
                <span class="min-w-0">
                  <span class="block text-sm font-medium text-gray-900 dark:text-white truncate">{{ album.title }}</span>
                  <span class="block text-xs text-text-secondary truncate">{{ album.artist || 'Unknown Artist' }}</span>
                  <span v-if="album.track_count !== null" class="block text-[11px] text-text-secondary">{{ album.track_count }} tracks</span>
                </span>
              </button>
            </div>
          </div>

          <!-- Artistas -->
          <div v-if="hasSection(provider.artists)" data-testid="catalog-section-artists">
            <h3 class="px-4 pt-3 pb-1 text-xs font-bold uppercase tracking-wide text-text-secondary">
              Artists <span v-if="items(provider.artists).length">({{ items(provider.artists).length }})</span>
            </h3>
            <p
              v-if="sectionError(provider.artists)"
              class="mx-4 mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm text-amber-600 dark:text-amber-400"
              data-testid="catalog-section-error"
            >
              <strong>{{ catalogFailureLabel(sectionError(provider.artists)!.kind) }}:</strong>
              {{ sectionError(provider.artists)!.message }}
            </p>
            <p v-else-if="items(provider.artists).length === 0" class="px-4 pb-3 text-sm text-text-secondary">
              No artists in this provider.
            </p>
            <div v-else class="flex flex-wrap gap-2 px-4 pb-3">
              <button
                v-for="artist in items(provider.artists)"
                :key="artist.id"
                @click="emit('openArtist', artist, provider)"
                class="flex items-center gap-2 rounded-full border border-gray-200 dark:border-border-dark pl-1 pr-3 py-1 hover:border-primary/60 transition-colors"
                data-testid="catalog-artist"
              >
                <TrackCover :src="artist.image_url" :alt="artist.name" size-class="w-8 h-8" rounded="rounded-full" icon="person" />
                <span class="text-sm text-gray-900 dark:text-white truncate">{{ artist.name }}</span>
                <span v-if="artist.album_count !== null" class="text-[11px] text-text-secondary">{{ artist.album_count }} albums</span>
              </button>
            </div>
          </div>
        </div>
      </section>

      <div
        v-if="response.providers.every(isEmptyAndHealthy)"
        class="py-8 text-center text-text-secondary"
        data-testid="catalog-no-results"
      >
        <span class="material-symbols-outlined text-5xl mb-3">search_off</span>
        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-1">No catalog results</h3>
        <p>No provider returned matches for "{{ response.query }}".</p>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * P10b (#409): resultados de la pestaña de proveedores.
 *
 * Cada proveedor conserva su bloque aunque una de sus tres secciones falle:
 * el fallo parcial se pinta dentro de la sección afectada y las demás siguen
 * mostrando sus resultados. Un proveedor caído no se disfraza de «sin
 * resultados» (ADR-0010, decisión 5 y enmienda 6).
 */
import TrackCover from '@/components/TrackCover.vue'
import { catalogFailureLabel } from '@/utils/catalogFailure'
import type {
  CatalogAlbum,
  CatalogArtist,
  CatalogProviderSearch,
  CatalogSearchFailure,
  CatalogSearchResponse,
  CatalogSection,
  CatalogTrack,
} from '@/api/types/catalogSearch'

defineProps<{
  response: CatalogSearchResponse | null
  loading: boolean
  error: string | null
  query: string
}>()

const emit = defineEmits<{
  play: [track: CatalogTrack, provider: CatalogProviderSearch]
  queue: [track: CatalogTrack, provider: CatalogProviderSearch]
  openAlbum: [album: CatalogAlbum, provider: CatalogProviderSearch]
  openArtist: [artist: CatalogArtist, provider: CatalogProviderSearch]
  retry: []
}>()

function sections(provider: CatalogProviderSearch) {
  return [provider.tracks, provider.albums, provider.artists]
}

function items<T>(section: CatalogSection<T>): T[] {
  return section.page?.items ?? []
}

function sectionError<T>(section: CatalogSection<T>): CatalogSearchFailure | null {
  return section.error ?? null
}

function failures(provider: CatalogProviderSearch): CatalogSearchFailure[] {
  return sections(provider)
    .map(section => section.error)
    .filter((failure): failure is CatalogSearchFailure => Boolean(failure))
}

/** `connected` | `expired` | `unavailable` | `degraded`: estado real del proveedor. */
function statusLabel(provider: CatalogProviderSearch): string {
  const errors = failures(provider)
  if (errors.length === 0) return 'connected'
  if (errors.some(error => error.kind === 'auth_expired')) return 'expired'
  if (errors.length === sections(provider).length) return 'unavailable'
  return 'degraded'
}

function statusClass(provider: CatalogProviderSearch): string {
  switch (statusLabel(provider)) {
    case 'connected':
      return 'border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
    case 'expired':
      return 'border-rose-500/30 bg-rose-500/10 text-rose-600 dark:text-rose-400'
    case 'unavailable':
      return 'border-gray-400/30 bg-gray-400/10 text-text-secondary'
    default:
      return 'border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400'
  }
}

/** Una sección vacía y sin fallo no necesita encabezado. */
function hasSection<T>(section: CatalogSection<T>): boolean {
  return Boolean(section.error) || Boolean(section.page && section.page.items.length > 0)
}

function isEmptyAndHealthy(provider: CatalogProviderSearch): boolean {
  return failures(provider).length === 0
    && sections(provider).every(section => (section.page?.items.length ?? 0) === 0)
}

function formatDuration(ms: number | null): string {
  if (!ms) return '0:00'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}
</script>
