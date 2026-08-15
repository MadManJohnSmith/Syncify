<template>
  <div class="flex flex-col h-full bg-background-light dark:bg-background-dark">
    <!-- Header with back navigation -->
    <header class="sticky top-0 z-10 bg-background-light/80 dark:bg-background-dark/80 backdrop-blur-lg border-b border-gray-200 dark:border-border-dark">
      <div class="flex items-center gap-4 px-6 py-4">
        <button 
          @click="goBack" 
          class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
        >
          <span class="material-symbols-outlined text-gray-600 dark:text-gray-400">arrow_back</span>
        </button>
        <div>
          <h1 class="text-xl font-semibold text-gray-900 dark:text-white">{{ artist?.name || 'Artist' }}</h1>
          <p class="text-sm text-text-secondary">Artist</p>
        </div>
      </div>
    </header>
    
    <!-- Main content -->
    <main class="flex-1 overflow-y-auto">
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
        <h2 class="text-lg font-medium text-gray-900 dark:text-white mb-1">Failed to load artist</h2>
        <p class="text-sm text-text-secondary mb-4">{{ error }}</p>
        <button @click="loadArtist" class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors">
          Try Again
        </button>
      </div>
      
      <!-- Artist content -->
      <div v-else-if="artist" class="p-6 space-y-8">
        <!-- Artist header -->
        <div class="flex gap-6">
          <!-- Avatar placeholder -->
          <div class="w-48 h-48 bg-gradient-to-br from-purple-400/20 to-purple-600/40 rounded-full flex items-center justify-center shadow-lg">
            <span class="material-symbols-outlined text-6xl text-purple-500/60">person</span>
          </div>
          
          <!-- Artist info -->
          <div class="flex-1 flex flex-col justify-center">
            <span class="text-sm font-medium text-purple-500 uppercase tracking-wide mb-1">Artist</span>
            <h2 class="text-3xl font-bold text-gray-900 dark:text-white mb-4">{{ artist.name }}</h2>
            
            <div class="flex items-center gap-6 text-sm text-text-secondary">
              <span class="flex items-center gap-1">
                <span class="material-symbols-outlined text-[16px]">album</span>
                {{ artist.album_count }} albums
              </span>
              <span class="flex items-center gap-1">
                <span class="material-symbols-outlined text-[16px]">music_note</span>
                {{ artist.track_count }} tracks
              </span>
            </div>
            
            <!-- Actions -->
            <div class="flex gap-3 mt-4 items-center">
              <button class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors">
                <span class="material-symbols-outlined text-[18px]">download</span>
                Download All
              </button>
              <button class="flex items-center gap-2 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors">
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
                  : 'border-transparent text-text-secondary hover:text-gray-900 dark:hover:text-white'
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
                  : 'border-transparent text-text-secondary hover:text-gray-900 dark:hover:text-white'
              ]"
            >
              All Tracks ({{ artist.track_count }})
            </button>
          </div>
        </div>
        
        <!-- Albums tab -->
        <section v-if="activeTab === 'albums'">
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
              <div class="aspect-square bg-gradient-to-br from-primary/10 to-primary/30 rounded-lg mb-2 flex items-center justify-center group-hover:shadow-lg transition-shadow">
                <span class="material-symbols-outlined text-4xl text-primary/50">album</span>
              </div>
              <h4 class="font-medium text-gray-900 dark:text-white text-sm truncate group-hover:text-primary transition-colors">{{ album.title }}</h4>
              <p class="text-xs text-text-secondary">
                {{ album.release_year || 'Unknown' }} • {{ album.track_count }} tracks
              </p>
            </div>
          </div>
        </section>
        
        <!-- Tracks tab -->
        <section v-if="activeTab === 'tracks'">
          <div v-if="!artist.top_tracks || artist.top_tracks.length === 0" class="text-center py-8 text-text-secondary">
            <span class="material-symbols-outlined text-4xl mb-2">music_off</span>
            <p>No tracks found for this artist</p>
          </div>
          
          <div v-else class="bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark overflow-hidden">
            <div 
              v-for="(track, index) in artist.top_tracks" 
              :key="track.id"
              class="flex items-center gap-4 px-4 py-3 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors border-b border-gray-100 dark:border-gray-800 last:border-0"
            >
              <span class="w-8 text-center text-sm text-text-secondary">{{ index + 1 }}</span>
              <div class="flex-1 min-w-0">
                <p class="font-medium text-gray-900 dark:text-white truncate">{{ track.title }}</p>
                <p class="text-sm text-text-secondary truncate">{{ track.album }}</p>
              </div>
              <span class="text-sm text-text-secondary">{{ formatTrackDuration(track.duration_ms) }}</span>
            </div>
          </div>
        </section>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getArtist, toggleArtistFavorite } from '@/api/library'
import type { ArtistDetail } from '@/api/types'

const route = useRoute()
const router = useRouter()

const artist = ref<ArtistDetail | null>(null)
const isLoading = ref(true)
const error = ref<string | null>(null)
const activeTab = ref<'albums' | 'tracks'>('albums')
const isFavorite = ref(false)

// Get artist ID from route params
const artistId = Number(route.params.id)

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

onMounted(() => {
  loadArtist()
})
</script>
