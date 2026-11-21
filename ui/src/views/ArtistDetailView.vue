<template>
  <div class="flex flex-col h-full bg-background-light dark:bg-background-dark">
    <!-- Header with back navigation -->
    <header class="sticky top-0 z-10 bg-background-light/80 dark:bg-background-dark/80 backdrop-blur-lg border-b border-gray-200 dark:border-border-dark shrink-0">
      <div class="flex items-center gap-4 px-6 py-4">
        <button 
          @click="goBack" 
          class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
        >
          <span class="material-symbols-outlined text-gray-600 dark:text-gray-400">arrow_back</span>
        </button>
        <div>
          <h1 class="text-xl font-semibold text-ink">{{ artist?.name || 'Artist' }}</h1>
          <p class="text-sm text-text-secondary">Artist</p>
        </div>
      </div>
    </header>
    
    <!-- Main content -->
    <main class="flex-1 min-h-0 flex flex-col">
      <!-- Loading state -->
      <div v-if="isLoading" class="flex items-center justify-center h-64">
        <div class="flex items-center gap-3 text-text-secondary">
          <span class="material-symbols-outlined animate-spin">progress_activity</span>
          <span>Loading artist...</span>
        </div>
      </div>
      
      <!-- Error state -->
      <div v-else-if="error" class="flex flex-col items-center justify-center h-64 text-center px-6">
        <span class="material-symbols-outlined text-4xl text-error mb-3">error</span>
        <h2 class="text-lg font-medium text-ink mb-1">Failed to load artist</h2>
        <p class="text-sm text-text-secondary mb-4">{{ error }}</p>
        <button @click="loadArtist" class="px-4 py-2 bg-primary text-on-accent rounded-lg hover:bg-primary-hover transition-colors">
          Try Again
        </button>
      </div>
      
      <!-- Artist content -->
      <div v-else-if="artist" class="flex-1 min-h-0 flex flex-col">
        <div class="p-6 pb-0 space-y-8 shrink-0">
          <!-- Artist header -->
          <div class="flex gap-6">
            <TrackCover
              :src="artist.image_url"
              :item-id="artist.id"
              :alt="artist.name"
              size-class="w-48 h-48"
              rounded="rounded-full shadow-lg"
              icon="person"
            />
            
            <!-- Artist info -->
            <div class="flex-1 flex flex-col justify-center">
              <span class="text-sm font-medium text-purple-500 uppercase tracking-wide mb-1">Artist</span>
              <h2 class="text-3xl font-bold text-ink mb-4">{{ artist.name }}</h2>
              
              <div class="flex items-center gap-6 text-sm text-text-secondary">
                <span class="flex items-center gap-1">
                  <span class="material-symbols-outlined text-[16px]">album</span>
                  {{ artist.album_count }} albums
                </span>
                <span class="flex items-center gap-1">
                  <span class="material-symbols-outlined text-[16px]">music_note</span>
                  {{ tracksTotal }} tracks
                </span>
              </div>
              
              <!-- Actions -->
              <div class="flex gap-3 mt-4 items-center">
                <button 
                  @click="downloadArtistTracks" 
                  :disabled="collectionLoading"
                  data-testid="artist-download-all"
                  class="flex items-center gap-2 px-4 py-2 bg-primary text-on-accent rounded-lg hover:bg-primary-hover transition-colors disabled:opacity-60 disabled:cursor-progress"
                >
                  <span class="material-symbols-outlined text-[18px]">download</span>
                  Download All
                </button>
                <button 
                  @click="shufflePlay"
                  :disabled="collectionLoading"
                  data-testid="artist-shuffle-play"
                  class="flex items-center gap-2 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors disabled:opacity-60 disabled:cursor-progress"
                >
                  <span class="material-symbols-outlined text-[18px]">shuffle</span>
                  Shuffle Play
                </button>
                <button 
                  @click="handleToggleFavorite"
                  class="p-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
                  :title="isFavorite ? 'Remove from favorites' : 'Add to favorites'"
                >
                  <span 
                    class="material-symbols-outlined text-[20px]"
                    :class="isFavorite ? 'text-red-500 fill-current font-variation-fill' : 'text-gray-400'"
                  >
                    favorite
                  </span>
                </button>
              </div>
            </div>
          </div>
          
          <!-- Tabs -->
          <div class="border-b border-gray-200 dark:border-border-dark">
            <div class="flex gap-6">
              <button 
                @click="activeTab = 'albums'" 
                :class="[
                  'pb-3 text-sm font-medium border-b-2 transition-colors',
                  activeTab === 'albums' 
                    ? 'border-primary text-primary' 
                    : 'border-transparent text-text-secondary hover:text-ink'
                ]"
              >
                Albums ({{ artist.album_count }})
              </button>
              <button 
                @click="activeTab = 'tracks'" 
                :class="[
                  'pb-3 text-sm font-medium border-b-2 transition-colors',
                  activeTab === 'tracks' 
                    ? 'border-primary text-primary' 
                    : 'border-transparent text-text-secondary hover:text-ink'
                ]"
              >
                All Tracks ({{ tracksTotal }})
              </button>
            </div>
          </div>
        </div>
        
        <!-- Albums tab -->
        <section v-if="activeTab === 'albums'" class="flex-1 min-h-0 overflow-y-auto p-6" data-testid="artist-albums">
          <div v-if="!artist.albums || artist.albums.length === 0" class="text-center py-8 text-text-secondary">
            <span class="material-symbols-outlined text-4xl mb-2">album</span>
            <p>No albums found for this artist</p>
          </div>
          
          <div v-else class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
            <div 
              v-for="album in artist.albums" 
              :key="album.id"
              @click="navigateToAlbum(album.id)"
              class="group cursor-pointer"
            >
              <TrackCover
                :src="album.cover_url"
                :item-id="album.id"
                :alt="album.title"
                size-class="aspect-square w-full"
                rounded="rounded-lg mb-2"
                icon="album"
              />
              <h4 class="font-medium text-ink text-sm truncate group-hover:text-primary transition-colors">{{ album.title }}</h4>
              <p class="text-xs text-text-secondary">
                {{ album.release_year || 'Unknown' }} • {{ album.track_count }} tracks
              </p>
            </div>
          </div>
        </section>
        
        <!-- Tracks tab: TODAS las pistas vivas del artista, paginadas en
             servidor y virtualizadas (L08, #105). top_tracks es la primera
             página de esta misma consulta; el resto se pide al hacer scroll. -->
        <section
          v-if="activeTab === 'tracks'"
          ref="containerRef"
          data-testid="artist-tracks-scroll"
          class="flex-1 min-h-0 overflow-y-auto custom-scrollbar border-t border-gray-200 dark:border-border-dark"
          @scroll="handleScroll"
        >
          <div v-if="tracksError" class="px-6 py-4 text-sm text-error flex items-center justify-between gap-4" data-testid="artist-tracks-error">
            <span>Could not load more tracks: {{ tracksError }}</span>
            <button
              type="button"
              data-testid="artist-tracks-retry"
              class="shrink-0 px-3 py-1 border border-gray-300 dark:border-gray-600 rounded-lg text-ink hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
              @click="retryTracks"
            >
              Try again
            </button>
          </div>
          <div v-else-if="tracks.length === 0" class="text-center py-8 text-text-secondary" data-testid="artist-tracks-empty">
            <span class="material-symbols-outlined text-4xl mb-2">music_off</span>
            <p>No tracks found for this artist</p>
          </div>

          <template v-else>
            <div :style="{ height: totalVirtualHeight + 'px', position: 'relative', width: '100%' }">
              <div :style="{ transform: `translateY(${virtualOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
                <div
                  v-for="(track, index) in visibleTracks"
                  :key="track.id"
                  data-testid="artist-track-row"
                  :data-track-id="track.id"
                  :style="{ height: TRACK_ROW_HEIGHT + 'px' }"
                  class="flex items-center gap-4 px-6 border-b border-gray-100 dark:border-gray-800 box-border group"
                >
                  <span class="w-8 text-center text-sm text-text-secondary">{{ virtualStartIndex + index + 1 }}</span>
                  <TrackCover
                    :src="coverByAlbumTitle(track.album)"
                    :item-id="track.id"
                    :alt="track.album || track.title"
                    size-class="w-9 h-9"
                  />
                  <div class="flex-1 min-w-0">
                    <p class="font-medium text-ink truncate">{{ track.title }}</p>
                    <p class="text-sm text-text-secondary truncate">
                      {{ track.album }}
                      <span
                        v-if="track.collaborators.length > 0"
                        data-testid="artist-track-collaborators"
                        class="text-purple-500"
                      > · feat. {{ track.collaborators.join(', ') }}</span>
                    </p>
                  </div>
                  <div class="flex items-center gap-1 flex-wrap w-36 shrink-0" data-testid="artist-track-sources">
                    <span
                      v-for="source in track.sources"
                      :key="source.serviceId"
                      class="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-surface-highlight text-text-secondary text-[10px] font-semibold"
                    >{{ source.serviceName }}</span>
                  </div>
                  <span class="text-sm text-text-secondary w-14 text-right">{{ formatTrackDuration(track.durationMs) }}</span>
                  <button 
                    @click="downloadTrack(track.id, track.title)" 
                    class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight/50 rounded-full transition-colors text-text-secondary hover:text-primary"
                    title="Download Track"
                  >
                    <span class="material-symbols-outlined text-[18px]">download</span>
                  </button>
                </div>
              </div>
            </div>
            <div v-if="tracksLoadingMore" class="py-2 text-center text-xs text-text-secondary">Loading more tracks…</div>
          </template>
        </section>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getArtist, queryLibraryArtistTracks, toggleArtistFavorite } from '@/api/library'
import { addToQueue, addBatchToQueue } from '@/api/queue'
import { useToast } from '@/composables/useToast'
import { usePlayer } from '@/composables/usePlayer'
import { useVirtualWindow } from '@/composables/useVirtualWindow'
import TrackCover from '@/components/TrackCover.vue'
import type { ArtistDetail, ArtistTrackRow } from '@/api/types'

const TRACK_PAGE_SIZE = 100
/** Página máxima que acepta el servidor: la enumeración completa va a bloque lleno. */
const FULL_COLLECTION_PAGE_SIZE = 200
const TRACK_ROW_HEIGHT = 64
const TRACK_OVERSCAN = 8
const TRACK_SCROLL_THROTTLE_MS = 120

const route = useRoute()
const router = useRouter()
const toast = useToast()
const player = usePlayer()

const artist = ref<ArtistDetail | null>(null)
const isLoading = ref(true)
const error = ref<string | null>(null)
const activeTab = ref<'albums' | 'tracks'>('albums')
const isFavorite = ref(false)

/** Todas las pistas vivas cargadas hasta ahora (no todas de golpe). */
const tracks = ref<ArtistTrackRow[]>([])
const tracksTotal = ref(0)
const tracksLoadingMore = ref(false)
const tracksError = ref<string | null>(null)
/** Enumeración completa en vuelo: Download All / Shuffle Play esperan su colección. */
const collectionLoading = ref(false)
let tracksRequestSeq = 0

const hasMoreTracks = computed(() => tracks.value.length < tracksTotal.value)

// Get artist ID from route params
const artistId = Number(route.params.id)

const {
  containerRef,
  totalHeight: totalVirtualHeight,
  offsetY: virtualOffsetY,
  startIndex: virtualStartIndex,
  visibleItems: visibleTracks,
  handleScroll,
  reset: resetVirtualTracks,
} = useVirtualWindow(tracks, {
  rowHeight: TRACK_ROW_HEIGHT,
  overscan: TRACK_OVERSCAN,
  scrollThrottleMs: TRACK_SCROLL_THROTTLE_MS,
  onScrollMeasured: (scrollBottom) => {
    if (scrollBottom < 300) loadMoreTracks()
  },
})

/**
 * Pide la siguiente página del MISMO query que produjo `top_tracks`. El
 * desplazamiento sale de lo ya cargado: no se piden pistas que ya están en
 * pantalla y el servidor sigue paginando.
 */
async function loadMoreTracks(): Promise<void> {
  if (tracksLoadingMore.value || tracksError.value) return
  if (!hasMoreTracks.value) return
  const seq = ++tracksRequestSeq
  tracksLoadingMore.value = true
  try {
    const result = await queryLibraryArtistTracks({
      artistId,
      offset: tracks.value.length,
      limit: TRACK_PAGE_SIZE,
    })
    if (seq !== tracksRequestSeq) return
    const known = new Set(tracks.value.map((track) => track.id))
    for (const track of result.tracks) {
      if (!known.has(track.id)) {
        tracks.value.push(track)
        known.add(track.id)
      }
    }
    tracksTotal.value = result.total
  } catch (e) {
    if (seq !== tracksRequestSeq) return
    tracksError.value = e instanceof Error ? e.message : String(e)
  } finally {
    if (seq === tracksRequestSeq) tracksLoadingMore.value = false
  }
}

/**
 * Un fallo de paginación no puede dejar la lista muerta: el early-return de
 * `loadMoreTracks` respeta `tracksError` para no reintentar en bucle desde el
 * scroll, así que el reintento es explícito.
 */
function retryTracks(): void {
  if (tracksLoadingMore.value) return
  tracksError.value = null
  void loadMoreTracks()
}

/**
 * Pide la página extra solo cuando la pestaña de pistas está abierta. Se usa
 * al entrar en la pestaña y también tras recargar el artista: un reintento
 * con la pestaña ya activa no cambia `activeTab` y dejaría las cinco filas
 * de `top_tracks` sin scroll para pedir el resto.
 */
function ensureExtraTracksPage(): void {
  if (activeTab.value === 'tracks' && hasMoreTracks.value) {
    void loadMoreTracks()
  }
}

/**
 * `top_tracks` only brings the album title; the cover is resolved against the
 * albums already present in the same response, without querying again.
 */
function coverByAlbumTitle(albumTitle: string | null | undefined): string | null {
  if (!artist.value?.albums || !albumTitle) return null
  return artist.value.albums.find(a => a.title === albumTitle)?.cover_url ?? null
}

/**
 * Colección COMPLETA de pistas vivas del artista (#336). Download All y
 * Shuffle Play operan sobre el total que anuncia la cabecera, no sobre la
 * primera página de `top_tracks`: se pagina el MISMO
 * `query_library_artist_tracks` que alimenta la lista, a bloque lleno.
 * Si el artista ya está cargado entero en pantalla, se reutiliza.
 */
async function loadArtistCollection(): Promise<ArtistTrackRow[]> {
  if (!hasMoreTracks.value && tracks.value.length > 0) return [...tracks.value]

  const collected: ArtistTrackRow[] = []
  const seen = new Set<number>()
  let offset = 0
  for (;;) {
    const page = await queryLibraryArtistTracks({
      artistId,
      offset,
      limit: FULL_COLLECTION_PAGE_SIZE,
    })
    for (const track of page.tracks) {
      if (seen.has(track.id)) continue
      seen.add(track.id)
      collected.push(track)
    }
    // Una página vacía corta también un `hasMore` incoherente: sin avance no
    // hay nada más que pedir.
    if (!page.hasMore || page.tracks.length === 0) break
    offset += page.tracks.length
  }
  return collected
}

async function downloadArtistTracks() {
  if (!artist.value) return
  const artistName = artist.value.name
  collectionLoading.value = true
  let collection: ArtistTrackRow[]
  try {
    collection = await loadArtistCollection()
  } catch (err: any) {
    const errStr = String(err?.message || err || '')
    toast.error('Failed to load the artist tracks', errStr)
    return
  } finally {
    collectionLoading.value = false
  }
  if (collection.length === 0) return

  const trackIds = collection.map(t => t.id)
  try {
    const res = await addBatchToQueue({ trackIds, allowFallback: true, jobKind: 'artist' })
    toast.success('Queued for download', `${res.added} tracks from ${artistName}`)
  } catch (err: any) {
    const errStr = String(err?.message || err || '')
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', 'One or more tracks have no available streaming provider source.')
    } else {
      toast.error('Failed to queue download', errStr)
    }
  }
}

async function shufflePlay() {
  if (!artist.value) {
    toast.warning('No tracks available', 'This artist has no indexed tracks to play.')
    return
  }
  const artistName = artist.value.name
  collectionLoading.value = true
  let collection: ArtistTrackRow[]
  try {
    collection = await loadArtistCollection()
  } catch (err: any) {
    const errStr = String(err?.message || err || '')
    toast.error('Failed to load the artist tracks', errStr)
    return
  } finally {
    collectionLoading.value = false
  }
  if (collection.length === 0) {
    toast.warning('No tracks available', 'This artist has no indexed tracks to play.')
    return
  }

  const shuffled = [...collection]
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]]
  }

  const toPlayerTrack = (t: { id: number; title: string; album: string | null }) => ({
    id: t.id,
    title: t.title,
    artist: artistName,
    album: t.album || null,
    coverUrl: coverByAlbumTitle(t.album),
  })

  try {
    // Phase 1: play the first track and queue the rest in listing order
    // (already shuffled above if coming from "Shuffle Play"); playNext used
    // to insert at the front and reverse the order.
    await player.replaceQueueAndPlay(shuffled.map(toPlayerTrack))
  } catch (err: any) {
    const errStr = String(err?.message || err || '')
    toast.error('Playback error', errStr)
  }
}

async function downloadTrack(trackId: number, title: string) {
  try {
    await addToQueue({
      trackId,
      targetTitle: title,
      targetArtist: artist.value?.name || undefined,
      allowFallback: true,
    })
    toast.success('Queued for download', title)
  } catch (err: any) {
    const errStr = String(err?.message || err || '')
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', `Track "${title}" has no available streaming provider source.`)
    } else {
      toast.error('Failed to queue download', errStr)
    }
  }
}

async function handleToggleFavorite() {
  const previousState = isFavorite.value
  isFavorite.value = !previousState
  try {
    const newState = await toggleArtistFavorite(artistId)
    isFavorite.value = newState
  } catch (err) {
    isFavorite.value = previousState
    console.error('Failed to toggle artist favorite:', err)
  }
}

function goBack() {
  router.back()
}

function formatTrackDuration(ms: number | null): string {
  if (!ms) return '0:00'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

function navigateToAlbum(albumId: number) {
  router.push({
    name: 'AlbumDetail',
    params: { id: albumId.toString() }
  })
}

async function loadArtist() {
  isLoading.value = true
  error.value = null
  
  if (isNaN(artistId) || artistId === 0) {
    error.value = "Invalid artist ID"
    isLoading.value = false
    return
  }

  try {
    const result = await getArtist(artistId)
    if (result) {
      artist.value = result
      // `top_tracks` ES la primera página de la consulta del detalle (L08) y
      // ya llega normalizada por `getArtist` (un solo normalizador, #336).
      tracksRequestSeq += 1
      tracks.value = result.top_tracks ?? []
      tracksTotal.value = result.tracks_total ?? tracks.value.length
      tracksError.value = null
      tracksLoadingMore.value = false
      resetVirtualTracks()
      // Si el reintento ocurre con la pestaña de pistas ya activa, el watch
      // no dispara: se pide aquí la página extra.
      ensureExtraTracksPage()
    } else {
      error.value = 'Failed to load artist'
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load artist'
    console.error('Failed to load artist:', e)
  } finally {
    isLoading.value = false
  }
}

/**
 * `top_tracks` ES la primera página de la consulta del detalle (L08), y son
 * solo cinco filas: si el viewport no desborda con cinco, no hay scroll y el
 * resto de páginas no llegaría a pedirse nunca. Al abrir la pestaña se pide
 * UNA página más; el scroll sigue paginando el resto (no se cargan todas).
 */
watch(activeTab, () => {
  ensureExtraTracksPage()
})

onMounted(() => {
  loadArtist()
})
</script>
