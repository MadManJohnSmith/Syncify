<script setup lang="ts">
/**
 * FavoritesView.vue — P16 (#147, spec #130): vista de Favoritos.
 *
 * Dos rejillas sobre los favoritos reales del backend (`get_favorites_albums`
 * / `get_favorites_artists`): álbumes (reproducir, encolar, abrir ficha) y
 * artistas (abrir detalle; sin top-tracks inventados: el detalle pide los
 * suyos al servidor, L08). El filtro por servicio viaja al backend (los
 * comandos ya filtran por nombre de servicio); la paginación también
 * (offset/limit, página completa = puede haber más).
 *
 * Los favoritos importados por sync llegan como stubs hasta que su tracklist
 * se expande; se piden (`includeStubs`) para que la vista no mienta, y
 * reproducir/encolar sobre uno sin pistas avisa en vez de fallar callendo.
 * La descarga desde favoritos NO vive aquí: es P17 (#148).
 */
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { getFavoritesAlbums, getFavoritesArtists, getAlbum } from '@/api/library'
import type { FavoriteAlbumItem, FavoriteArtistItem } from '@/api/library'
import { getServices } from '@/api/accounts'
import type { Service } from '@/api/types'

import type { PlayerTrack } from '@/composables/usePlayer'
import { usePlayer } from '@/composables/usePlayer'
import { useToast } from '@/composables/useToast'
import TrackCover from '@/components/TrackCover.vue'

defineOptions({ name: 'FavoritesView' })

const PAGE_SIZE = 100

const router = useRouter()
const player = usePlayer()
const toast = useToast()

const albums = ref<FavoriteAlbumItem[]>([])
const artists = ref<FavoriteArtistItem[]>([])
const services = ref<Service[]>([])

const service = ref('')
const loadingAlbums = ref(true)
const loadingArtists = ref(true)
const loadingMoreAlbums = ref(false)
const loadingMoreArtists = ref(false)
const albumsError = ref<string | null>(null)
const artistsError = ref<string | null>(null)
/** Una página completa devuelve `true`: puede haber más, nunca se afirma un total. */
const albumsMayHaveMore = ref(false)
const artistsMayHaveMore = ref(false)

let albumsRequestSeq = 0
let artistsRequestSeq = 0

function serviceArg(): string | undefined {
  return service.value === '' ? undefined : service.value
}

/** Un fallo de las facetas no bloquea la vista: el filtro degrada a «All». */
async function loadServices(): Promise<void> {
  try {
    services.value = await getServices()
  } catch {
    services.value = []
  }
}

async function loadAlbums(refresh: boolean): Promise<void> {
  const seq = ++albumsRequestSeq
  if (refresh) {
    loadingAlbums.value = true
    albumsError.value = null
  } else {
    loadingMoreAlbums.value = true
  }
  try {
    const page = await getFavoritesAlbums(serviceArg(), refresh ? 0 : albums.value.length, PAGE_SIZE, true)
    if (seq !== albumsRequestSeq) return
    albums.value = refresh ? page : albums.value.concat(page)
    albumsMayHaveMore.value = page.length === PAGE_SIZE
  } catch (e) {
    if (seq !== albumsRequestSeq) return
    albumsError.value = e instanceof Error ? e.message : String(e)
  } finally {
    if (seq === albumsRequestSeq) {
      loadingAlbums.value = false
      loadingMoreAlbums.value = false
    }
  }
}

async function loadArtists(refresh: boolean): Promise<void> {
  const seq = ++artistsRequestSeq
  if (refresh) {
    loadingArtists.value = true
    artistsError.value = null
  } else {
    loadingMoreArtists.value = true
  }
  try {
    const page = await getFavoritesArtists(serviceArg(), refresh ? 0 : artists.value.length, PAGE_SIZE)
    if (seq !== artistsRequestSeq) return
    artists.value = refresh ? page : artists.value.concat(page)
    artistsMayHaveMore.value = page.length === PAGE_SIZE
  } catch (e) {
    if (seq !== artistsRequestSeq) return
    artistsError.value = e instanceof Error ? e.message : String(e)
  } finally {
    if (seq === artistsRequestSeq) {
      loadingArtists.value = false
      loadingMoreArtists.value = false
    }
  }
}

function reload(): void {
  void loadAlbums(true)
  void loadArtists(true)
}

// El filtro es la única entrada compartida: cualquier cambio recarga ambas
// rejillas desde offset 0 (las respuestas viejas se descartan por seq).
watch(service, () => {
  reload()
})

// El conteo es honesto: sin total del backend, «100+» marca que la lista
// está truncada en la página cargada.
const albumsCountLabel = computed(() =>
  albumsMayHaveMore.value ? `${albums.value.length}+` : String(albums.value.length),
)
const artistsCountLabel = computed(() =>
  artistsMayHaveMore.value ? `${artists.value.length}+` : String(artists.value.length),
)

function toPlayerTrack(track: { id: number; title: string }, album: FavoriteAlbumItem): PlayerTrack {
  return {
    id: track.id,
    title: track.title,
    artist: album.artist || 'Unknown Artist',
    album: album.title || null,
    coverUrl: album.cover_art_url ?? null,
  }
}

async function withAlbumTracks(album: FavoriteAlbumItem): Promise<{ id: number; title: string }[] | null> {
  try {
    const detail = await getAlbum(album.id)
    const tracks = detail?.tracks ?? []
    if (tracks.length === 0) {
      toast.warning(
        'No indexed tracks',
        `"${album.title || 'This album'}" has no expanded tracklist yet. Open it to refresh.`,
      )
      return null
    }
    return tracks
  } catch (err) {
    toast.error('Failed to load album', err instanceof Error ? err.message : String(err))
    return null
  }
}

/** Reproducir el álbum: reemplaza la cola con sus pistas en orden (#131). */
async function playAlbum(album: FavoriteAlbumItem): Promise<void> {
  const tracks = await withAlbumTracks(album)
  if (!tracks) return
  try {
    await player.replaceQueueAndPlay(tracks.map((t) => toPlayerTrack(t, album)))
    toast.success('Playing album', `${album.title} · ${tracks.length} tracks queued`)
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    if (msg.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', 'One or more tracks have no available streaming provider source.')
    } else {
      toast.error('Playback error', msg)
    }
  }
}

/** Encolar el álbum al final de la cola de reproducción (no de descargas). */
async function queueAlbum(album: FavoriteAlbumItem): Promise<void> {
  const tracks = await withAlbumTracks(album)
  if (!tracks) return
  try {
    let started = false
    for (const track of tracks) {
      const outcome = await player.enqueue(toPlayerTrack(track, album))
      if (outcome === 'started') started = true
    }
    if (started) {
      toast.success('Playing album', `${album.title} · ${tracks.length} tracks queued`)
    } else {
      toast.success('Queued for playback', `${album.title} · ${tracks.length} tracks`)
    }
  } catch (err) {
    toast.error('Failed to queue tracks', err instanceof Error ? err.message : String(err))
  }
}

function openAlbum(albumId: number): void {
  void router.push({ name: 'AlbumDetail', params: { id: String(albumId) } })
}

function openArtist(artistId: number): void {
  void router.push({ name: 'ArtistDetail', params: { id: String(artistId) } })
}

onMounted(() => {
  void loadServices()
  reload()
})
</script>

<template>
  <div class="favorites-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <header class="px-8 pt-6 pb-2 shrink-0 flex items-end justify-between flex-wrap gap-4">
      <div>
        <div class="flex items-baseline gap-4">
          <h1 class="text-3xl font-bold tracking-tight text-ink">Favorites</h1>
          <span class="text-lg text-text-secondary font-medium" data-testid="favorites-total">
            {{ albumsCountLabel }} albums · {{ artistsCountLabel }} artists
          </span>
        </div>
        <p class="text-text-secondary mt-1">Albums and artists you marked as favorites in your library and services</p>
      </div>
      <select
        v-model="service"
        data-testid="favorites-filter-service"
        aria-label="Filter by service"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
      >
        <option value="">All services</option>
        <option value="local">Local</option>
        <option v-for="s in services" :key="s.id" :value="s.name">
          {{ s.name }}
        </option>
      </select>
    </header>

    <main class="flex-1 min-h-0 overflow-y-auto custom-scrollbar">
      <!-- Albums -->
      <section class="px-8 py-4" data-testid="favorites-albums-section">
        <h2 class="text-lg font-semibold text-ink mb-3">Albums</h2>

        <div v-if="albumsError" class="py-4" data-testid="favorites-albums-error">
          <p class="text-sm font-medium text-error">Could not load favorite albums</p>
          <p class="text-xs text-text-secondary mt-1">{{ albumsError }}</p>
          <button type="button" class="mt-3 px-3 py-1.5 rounded-lg bg-primary text-on-accent text-sm" @click="reload">
            Try again
          </button>
        </div>

        <div v-else-if="loadingAlbums" class="flex items-center text-text-secondary py-4" data-testid="favorites-albums-loading">
          <span class="material-symbols-outlined animate-spin mr-2">progress_activity</span>
          Loading favorite albums…
        </div>

        <div v-else-if="albums.length === 0" class="py-8 text-center" data-testid="favorites-albums-empty">
          <span class="material-symbols-outlined text-4xl text-gray-300">album</span>
          <p class="text-sm text-text-secondary mt-2">No favorite albums match this filter.</p>
          <p class="text-xs text-text-secondary mt-1">Favorite albums from your library or sync them from a service.</p>
        </div>

        <template v-else>
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
            <div
              v-for="album in albums"
              :key="album.id"
              data-testid="favorites-album-card"
              :data-album-id="album.id"
              class="group cursor-pointer"
              @click="openAlbum(album.id)"
            >
              <TrackCover
                :src="album.cover_art_url"
                :item-id="album.id"
                :alt="album.title"
                size-class="aspect-square w-full"
                rounded="rounded-lg mb-2"
                icon="album"
              />
              <h4 class="font-medium text-ink text-sm truncate group-hover:text-primary transition-colors">
                {{ album.title || 'Untitled album' }}
              </h4>
              <p class="text-xs text-text-secondary truncate">
                {{ album.artist || 'Unknown artist' }}
                <span v-if="typeof album.total_tracks === 'number' && album.total_tracks > 0"> · {{ album.total_tracks }} tracks</span>
                <span v-else> · Tracklist not expanded</span>
              </p>
              <div class="flex items-center justify-between mt-2">
                <span class="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-surface-highlight text-text-secondary text-[10px] font-semibold" data-testid="favorites-album-service">
                  {{ album.service || 'local' }}
                </span>
                <span class="flex items-center gap-1">
                  <button
                    type="button"
                    data-testid="favorites-album-play"
                    class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight text-text-secondary hover:text-primary transition-colors"
                    title="Play album"
                    @click.stop="playAlbum(album)"
                  >
                    <span class="material-symbols-outlined text-[18px]">play_arrow</span>
                  </button>
                  <button
                    type="button"
                    data-testid="favorites-album-queue"
                    class="p-1.5 rounded-full hover:bg-gray-100 dark:hover:bg-surface-highlight text-text-secondary hover:text-primary transition-colors"
                    title="Queue album for playback"
                    @click.stop="queueAlbum(album)"
                  >
                    <span class="material-symbols-outlined text-[18px]">playlist_add</span>
                  </button>
                </span>
              </div>
            </div>
          </div>
          <div v-if="albumsMayHaveMore" class="py-3 text-center">
            <button
              type="button"
              data-testid="favorites-albums-more"
              class="px-4 py-1.5 rounded-lg border border-gray-300 dark:border-gray-600 text-sm text-ink hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
              :disabled="loadingMoreAlbums"
              @click="loadAlbums(false)"
            >
              {{ loadingMoreAlbums ? 'Loading more albums…' : 'Load more albums' }}
            </button>
          </div>
        </template>
      </section>

      <!-- Artists -->
      <section class="px-8 py-4" data-testid="favorites-artists-section">
        <h2 class="text-lg font-semibold text-ink mb-3">Artists</h2>

        <div v-if="artistsError" class="py-4" data-testid="favorites-artists-error">
          <p class="text-sm font-medium text-error">Could not load favorite artists</p>
          <p class="text-xs text-text-secondary mt-1">{{ artistsError }}</p>
          <button type="button" class="mt-3 px-3 py-1.5 rounded-lg bg-primary text-on-accent text-sm" @click="reload">
            Try again
          </button>
        </div>

        <div v-else-if="loadingArtists" class="flex items-center text-text-secondary py-4" data-testid="favorites-artists-loading">
          <span class="material-symbols-outlined animate-spin mr-2">progress_activity</span>
          Loading favorite artists…
        </div>

        <div v-else-if="artists.length === 0" class="py-8 text-center" data-testid="favorites-artists-empty">
          <span class="material-symbols-outlined text-4xl text-gray-300">person</span>
          <p class="text-sm text-text-secondary mt-2">No favorite artists match this filter.</p>
        </div>

        <template v-else>
          <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
            <div
              v-for="artist in artists"
              :key="artist.id"
              data-testid="favorites-artist-card"
              :data-artist-id="artist.id"
              class="group cursor-pointer text-center"
              @click="openArtist(artist.id)"
            >
              <TrackCover
                :src="artist.image_url"
                :item-id="artist.id"
                :alt="artist.name"
                size-class="aspect-square w-full"
                rounded="rounded-full shadow-lg mb-2"
                icon="person"
              />
              <h4 class="font-medium text-ink text-sm truncate group-hover:text-primary transition-colors">
                {{ artist.name || 'Unknown artist' }}
              </h4>
              <span
                class="inline-block mt-1 px-1.5 py-0.5 rounded bg-gray-100 dark:bg-surface-highlight text-text-secondary text-[10px] font-semibold"
                data-testid="favorites-artist-service"
              >{{ artist.service || 'local' }}</span>
            </div>
          </div>
          <div v-if="artistsMayHaveMore" class="py-3 text-center">
            <button
              type="button"
              data-testid="favorites-artists-more"
              class="px-4 py-1.5 rounded-lg border border-gray-300 dark:border-gray-600 text-sm text-ink hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
              :disabled="loadingMoreArtists"
              @click="loadArtists(false)"
            >
              {{ loadingMoreArtists ? 'Loading more artists…' : 'Load more artists' }}
            </button>
          </div>
        </template>
      </section>
    </main>
  </div>
</template>
