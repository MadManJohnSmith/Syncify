<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-[400] bg-black/60 backdrop-blur-sm flex items-start justify-center pt-[10vh] px-4"
      data-testid="search-modal-backdrop"
      @click.self="close"
    >
      <div
        class="w-full max-w-3xl max-h-[75vh] flex flex-col bg-white dark:bg-surface-dark rounded-2xl shadow-2xl border border-gray-200 dark:border-border-dark overflow-hidden"
        role="dialog"
        aria-modal="true"
        aria-label="Search"
        data-testid="search-modal"
      >
        <!-- Query -->
        <div class="flex items-center gap-3 px-5 py-4 border-b border-gray-200 dark:border-border-dark shrink-0">
          <span class="material-symbols-outlined text-[24px] text-gray-400 shrink-0">search</span>
          <input
            ref="searchInput"
            v-model="searchQuery"
            @input="handleInput"
            type="text"
            placeholder="Search tracks, artists, albums..."
            class="flex-1 bg-transparent text-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none"
            data-testid="search-modal-input"
          >
          <button
            v-if="searchQuery"
            @click="clearSearch"
            class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors shrink-0"
            aria-label="Clear search"
          >
            <span class="material-symbols-outlined text-[22px]">close</span>
          </button>
          <button
            @click="close"
            class="px-2 py-0.5 rounded bg-gray-100 dark:bg-surface-highlight text-xs text-text-secondary hover:text-white transition-colors shrink-0"
            aria-label="Close search"
          >
            Esc
          </button>
        </div>

        <!-- Body -->
        <div class="flex-1 overflow-y-auto px-5 py-4">
          <!-- Initial state -->
          <div v-if="!searchQuery && tracks.length === 0" class="flex flex-col items-center justify-center py-12 text-center text-text-secondary">
            <span class="material-symbols-outlined text-5xl mb-4 text-gray-300 dark:text-gray-700">search</span>
            <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-1">Find what you're looking for</h3>
            <p>Type to search your entire music library</p>
          </div>

          <!-- Loading state -->
          <div v-else-if="isLoading && tracks.length === 0" class="py-10 text-center text-text-secondary">
            <span class="material-symbols-outlined animate-spin text-4xl mb-4 text-primary block">progress_activity</span>
            <p>Searching for "{{ searchQuery }}"...</p>
          </div>

          <!-- Error: una búsqueda fallida no es «sin resultados» -->
          <div v-else-if="searchError" class="py-10 text-center text-text-secondary" data-testid="search-modal-error">
            <span class="material-symbols-outlined text-5xl mb-4 text-error">cloud_off</span>
            <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-1">Search failed</h3>
            <p class="mb-4">{{ searchError }}</p>
            <button
              v-if="searchQuery"
              @click="performSearch(searchQuery)"
              class="px-3 py-1.5 bg-primary text-white rounded-lg text-sm"
            >
              Try Again
            </button>
          </div>

          <!-- No results -->
          <div v-else-if="!isLoading && searchQuery && tracks.length === 0" class="py-10 text-center text-text-secondary">
            <span class="material-symbols-outlined text-5xl mb-4">search_off</span>
            <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-1">No results found</h3>
            <p>We couldn't find anything matching "{{ searchQuery }}"</p>
          </div>

          <!-- Results list -->
          <div v-else class="space-y-2 pb-2">
            <div class="flex items-center justify-between">
              <h2 class="text-sm font-bold text-gray-900 dark:text-white">Tracks ({{ tracks.length }})</h2>
            </div>

            <div class="rounded-xl border border-gray-200 dark:border-border-dark overflow-hidden">
              <div
                v-for="(track, index) in tracks"
                :key="track.id"
                @click="playTrack(track)"
                class="flex items-center gap-4 px-4 py-2.5 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors border-b border-gray-100 dark:border-gray-800 last:border-0 cursor-pointer group"
              >
                <!-- Index / Play icon -->
                <div class="w-8 flex justify-center shrink-0">
                  <span class="text-sm text-text-secondary group-hover:hidden">{{ index + 1 }}</span>
                  <span class="material-symbols-outlined text-[20px] text-primary hidden group-hover:block">play_arrow</span>
                </div>

                <!-- Art -->
                <TrackCover
                  :src="track.cover_art_url"
                  :item-id="track.id"
                  :alt="track.album_name || track.title"
                  size-class="w-10 h-10"
                />

                <!-- Info -->
                <div class="flex-1 min-w-0">
                  <p class="font-medium text-gray-900 dark:text-white truncate">{{ track.title }}</p>
                  <div class="flex items-center gap-1 text-sm text-text-secondary truncate">
                    <span class="hover:text-primary hover:underline transition-colors cursor-pointer" @click.stop="goToArtist(track)">{{ track.artist_name || 'Unknown Artist' }}</span>
                    <span>•</span>
                    <span class="hover:text-primary hover:underline transition-colors cursor-pointer" @click.stop="goToAlbum(track)">{{ track.album_name || 'Unknown Album' }}</span>
                  </div>
                </div>

                <!-- Duration -->
                <span class="text-sm text-text-secondary font-mono">{{ formatDuration(track.duration_ms) }}</span>

                <!-- Download button -->
                <button
                  @click.stop="downloadTrack(track)"
                  class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight/50 rounded-full transition-colors text-text-secondary hover:text-primary opacity-0 group-hover:opacity-100"
                  title="Download Track"
                >
                  <span class="material-symbols-outlined text-[18px]">download</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
/**
 * Búsqueda como modal (R17).
 *
 * Antes era una vista con su propia pestaña en la barra lateral y su ruta
 * `/search`. Ahora es una capa modal abierta desde el botón del navbar y desde
 * el atajo `Ctrl+F`, así que la única entrada posible es ella.
 */
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { searchTracks } from '@/api/library'
import { addToQueue } from '@/api/queue'
import { useToast } from '@/composables/useToast'
import { usePlayer } from '@/composables/usePlayer'
import TrackCover from '@/components/TrackCover.vue'
import type { LibraryTrack } from '@/api/types'

const emit = defineEmits<{ close: [] }>()

const router = useRouter()
const toast = useToast()
const player = usePlayer()

const searchQuery = ref('')
const tracks = ref<LibraryTrack[]>([])
const isLoading = ref(false)
/** Fallo de la última búsqueda; distingue «sin resultados» de «no se pudo buscar». */
const searchError = ref<string | null>(null)
const searchInput = ref<HTMLInputElement | null>(null)
let debounceTimer: ReturnType<typeof setTimeout> | null = null

function close() {
  emit('close')
}

async function downloadTrack(track: LibraryTrack) {
  try {
    await addToQueue({
      trackId: track.id,
      targetTitle: track.title,
      targetArtist: track.artist_name || undefined,
      targetAlbum: track.album_name || undefined,
      targetIsrc: track.isrc || undefined,
      allowFallback: true,
    })
    toast.success('Queued for download', track.title)
  } catch (error: any) {
    const errStr = String(error?.message || error || '')
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', `Track "${track.title}" has no available provider source.`)
    } else {
      toast.error(`Failed to enqueue: ${errStr}`)
    }
  }
}

function formatDuration(ms: number | null): string {
  if (!ms) return '0:00'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

function clearSearch() {
  searchQuery.value = ''
  tracks.value = []
  searchError.value = null
  searchInput.value?.focus()
}

function handleInput() {
  if (debounceTimer) clearTimeout(debounceTimer)

  const query = searchQuery.value.trim()
  searchError.value = null

  if (!query) {
    tracks.value = []
    isLoading.value = false
    return
  }

  isLoading.value = true
  debounceTimer = setTimeout(() => {
    performSearch(query)
  }, 500)
}

async function performSearch(query: string) {
  if (!query.trim()) return
  isLoading.value = true
  searchError.value = null
  try {
    const result = await searchTracks(query, 0, 50)
    tracks.value = result?.tracks ?? []
  } catch (error) {
    console.error('Search failed:', error)
    tracks.value = []
    searchError.value = String(error)
  } finally {
    isLoading.value = false
  }
}

async function playTrack(track: LibraryTrack) {
  try {
    await player.play({
      id: track.id,
      title: track.title,
      artist: track.artist_name || 'Unknown Artist',
      album: track.album_name || null,
      coverUrl: track.cover_art_url || null,
    })
  } catch (err) {
    console.error('Failed to play track:', err)
  }
}

function goToArtist(track: LibraryTrack) {
  if (!track.artist_id) return
  close()
  router.push({ name: 'ArtistDetail', params: { id: track.artist_id.toString() } })
}

function goToAlbum(track: LibraryTrack) {
  if (!track.album_id) return
  close()
  router.push({ name: 'AlbumDetail', params: { id: track.album_id.toString() } })
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.stopPropagation()
    close()
  }
}

onMounted(async () => {
  document.addEventListener('keydown', onKeydown)
  await nextTick()
  searchInput.value?.focus()
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown)
  if (debounceTimer) {
    clearTimeout(debounceTimer)
    debounceTimer = null
  }
})
</script>