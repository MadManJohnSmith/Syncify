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
          <h1 class="text-xl font-semibold text-gray-900 dark:text-white">{{ album?.title || 'Album' }}</h1>
          <p class="text-sm text-text-secondary">{{ album?.artist_name }}</p>
        </div>
      </div>
    </header>
    
    <!-- Main content -->
    <main class="flex-1 overflow-y-auto">
      <!-- Loading state -->
      <div v-if="isLoading" class="flex items-center justify-center h-64">
        <div class="flex items-center gap-3 text-text-secondary">
          <span class="material-symbols-outlined animate-spin">progress_activity</span>
          <span>Loading album...</span>
        </div>
      </div>
      
      <!-- Error state -->
      <div v-else-if="error" class="flex flex-col items-center justify-center h-64 text-center px-6">
        <span class="material-symbols-outlined text-4xl text-error mb-3">error</span>
        <h2 class="text-lg font-medium text-gray-900 dark:text-white mb-1">Failed to load album</h2>
        <p class="text-sm text-text-secondary mb-4">{{ error }}</p>
        <button @click="loadAlbum" class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors">
          Try Again
        </button>
      </div>
      
      <!-- Album content -->
      <div v-else-if="album" class="p-6 space-y-8">
        <!-- Album header -->
        <div class="flex gap-6">
          <!-- Artwork placeholder -->
          <div class="w-48 h-48 bg-gradient-to-br from-primary/20 to-primary/40 rounded-xl flex items-center justify-center shadow-lg">
            <span class="material-symbols-outlined text-6xl text-primary/60">album</span>
          </div>
          
          <!-- Album info -->
          <div class="flex-1 flex flex-col justify-center">
            <span class="text-sm font-medium text-primary uppercase tracking-wide mb-1">Album</span>
            <h2 class="text-3xl font-bold text-gray-900 dark:text-white mb-2">{{ album.title }}</h2>
            <p class="text-lg text-gray-600 dark:text-gray-300 mb-4">{{ album.artist_name }}</p>
            
            <div class="flex items-center gap-4 text-sm text-text-secondary">
              <span v-if="album.release_year" class="flex items-center gap-1">
                <span class="material-symbols-outlined text-[16px]">calendar_month</span>
                {{ album.release_year }}
              </span>
              <span class="flex items-center gap-1">
                <span class="material-symbols-outlined text-[16px]">music_note</span>
                {{ album.track_count }} tracks
              </span>
              <span class="flex items-center gap-1">
                <span class="material-symbols-outlined text-[16px]">schedule</span>
                {{ formatDuration(album.total_duration_ms) }}
              </span>
              <span v-if="album.genre" class="px-2 py-0.5 bg-gray-100 dark:bg-surface-highlight rounded text-xs">
                {{ album.genre }}
              </span>
            </div>
            
            <!-- Actions -->
            <div class="flex gap-3 mt-4 items-center">
              <button class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors">
                <span class="material-symbols-outlined text-[18px]">download</span>
                Download All
              </button>
              <button class="flex items-center gap-2 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors">
                <span class="material-symbols-outlined text-[18px]">queue_music</span>
                Add to Queue
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
        
        <!-- Track list -->
        <section>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">Tracks</h3>
          
          <div v-if="!album.tracks || album.tracks.length === 0" class="text-center py-8 text-text-secondary">
            <span class="material-symbols-outlined text-4xl mb-2">music_off</span>
            <p>No tracks found for this album</p>
          </div>
          
          <div v-else class="bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark overflow-hidden">
            <div 
              v-for="(track, index) in album.tracks" 
              :key="track.id"
              class="flex items-center gap-4 px-4 py-3 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors border-b border-gray-100 dark:border-gray-800 last:border-0"
            >
              <span class="w-8 text-center text-sm text-text-secondary">{{ index + 1 }}</span>
              <div class="flex-1 min-w-0">
                <p class="font-medium text-gray-900 dark:text-white truncate">{{ track.title }}</p>
                <p class="text-sm text-text-secondary truncate">{{ track.artist_name }}</p>
              </div>
              <span class="text-sm text-text-secondary">{{ formatTrackDuration(track.duration_ms) }}</span>
              <button class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight/50 rounded-full transition-colors opacity-0 group-hover:opacity-100">
                <span class="material-symbols-outlined text-[18px] text-gray-500">more_vert</span>
              </button>
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
import { getAlbum, toggleAlbumFavorite } from '@/api/library'
import type { AlbumDetail } from '@/api/types'

const route = useRoute()
const router = useRouter()

const album = ref<AlbumDetail | null>(null)
const isLoading = ref(true)
const error = ref<string | null>(null)
const isFavorite = ref(false)

// Get album ID from route params
const albumId = Number(route.params.id)

async function handleToggleFavorite() {
  const previousState = isFavorite.value
  isFavorite.value = !previousState
  try {
    const newState = await toggleAlbumFavorite(albumId)
    isFavorite.value = newState
  } catch (err) {
    isFavorite.value = previousState
    console.error('Failed to toggle album favorite:', err)
  }
}

function goBack() {
  router.back()
}

function formatDuration(ms: number | undefined): string {
  if (!ms) return '0 min'
  const totalSeconds = Math.floor(ms / 1000)
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  
  if (hours > 0) {
    return `${hours}h ${minutes}m`
  }
  return `${minutes} min`
}

function formatTrackDuration(ms: number | null): string {
  if (!ms) return '0:00'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

async function loadAlbum() {
  isLoading.value = true
  error.value = null
  
  if (isNaN(albumId) || albumId === 0) {
    error.value = "Invalid album ID"
    isLoading.value = false
    return
  }

  try {
    const result = await getAlbum(albumId)
    if (result) {
      album.value = result
    } else {
      error.value = 'Failed to load album'
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load album'
    console.error('Failed to load album:', e)
  } finally {
    isLoading.value = false
  }
}

onMounted(() => {
  loadAlbum()
})
</script>
