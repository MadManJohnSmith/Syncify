<template>
  <div class="flex flex-col h-full bg-background-light dark:bg-background-dark">
    <!-- Header -->
    <header class="sticky top-0 z-10 bg-background-light/80 dark:bg-background-dark/80 backdrop-blur-lg border-b border-gray-200 dark:border-border-dark">
      <div class="px-6 py-4">
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-4">Search</h1>
        
        <div class="relative">
          <span class="absolute left-4 top-1/2 -translate-y-1/2 text-gray-400 material-symbols-outlined text-[24px]">search</span>
          <input 
            v-model="searchQuery"
            @input="handleInput"
            type="text" 
            placeholder="Search tracks, artists, albums..." 
            class="w-full pl-12 pr-12 py-4 bg-white dark:bg-surface-dark border-2 border-primary/20 hover:border-primary/50 focus:border-primary rounded-xl text-lg text-gray-900 dark:text-white transition-all shadow-sm"
          >
          <button 
            v-if="searchQuery" 
            @click="clearSearch"
            class="absolute right-4 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          >
            <span class="material-symbols-outlined text-[24px]">close</span>
          </button>
        </div>
      </div>
    </header>
    
    <!-- Main content -->
    <main class="flex-1 overflow-y-auto px-6 py-2">
      
      <!-- Initial state -->
      <div v-if="!searchQuery && tracks.length === 0" class="flex flex-col items-center justify-center h-full text-center text-text-secondary">
        <span class="material-symbols-outlined text-6xl mb-4 text-gray-300 dark:text-gray-700">search</span>
        <h3 class="text-xl font-medium text-gray-900 dark:text-white mb-2">Find what you're looking for</h3>
        <p>Type to search your entire music library</p>
      </div>
      
      <!-- Loading state -->
      <div v-else-if="isLoading && tracks.length === 0" class="py-8 text-center text-text-secondary">
        <span class="material-symbols-outlined animate-spin text-4xl mb-4 text-primary">progress_activity</span>
        <p>Searching for "{{ searchQuery }}"...</p>
      </div>

      <!-- No results -->
      <div v-else-if="!isLoading && searchQuery && tracks.length === 0" class="py-12 text-center text-text-secondary">
        <span class="material-symbols-outlined text-5xl mb-4">search_off</span>
        <h3 class="text-lg font-medium text-gray-900 dark:text-white mb-1">No results found</h3>
        <p>We couldn't find anything matching "{{ searchQuery }}"</p>
      </div>
      
      <!-- Results list -->
      <div v-else class="space-y-6 pb-8">
        <div class="flex items-center justify-between">
          <h2 class="text-lg font-bold text-gray-900 dark:text-white">Tracks ({{ tracks.length }})</h2>
        </div>
        
        <div class="bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark overflow-hidden">
          <div 
            v-for="(track, index) in tracks" 
            :key="track.id"
            @click="playTrack(track)"
            class="flex items-center gap-4 px-4 py-3 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors border-b border-gray-100 dark:border-gray-800 last:border-0 cursor-pointer group"
          >
            <!-- Index / Play icon -->
            <div class="w-8 flex justify-center shrink-0">
               <span class="text-sm text-text-secondary group-hover:hidden">{{ index + 1 }}</span>
               <span class="material-symbols-outlined text-[20px] text-primary hidden group-hover:block">play_arrow</span>
            </div>
            
            <!-- Art -->
            <div class="w-12 h-12 bg-gray-200 dark:bg-gray-800 rounded flex items-center justify-center overflow-hidden shrink-0">
              <img v-if="track.cover_art_url" :src="track.cover_art_url" class="w-full h-full object-cover">
              <span v-else class="material-symbols-outlined text-gray-400">music_note</span>
            </div>
            
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
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { searchTracks } from '@/api/library'
import { addToQueue } from '@/api/queue'
import { useToast } from '@/composables/useToast'
import type { LibraryTrack } from '@/api/types'

const router = useRouter()
const toast = useToast()

const searchQuery = ref('')
const tracks = ref<LibraryTrack[]>([])
const isLoading = ref(false)
let debounceTimer: ReturnType<typeof setTimeout> | null = null

async function downloadTrack(track: LibraryTrack) {
  try {
    await addToQueue({
      trackId: track.id,
      targetTitle: track.title,
      targetArtist: track.artist_name || undefined,
      targetAlbum: track.album_name || undefined,
      targetIsrc: track.isrc || undefined,
      allowFallback: false,
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
}

function handleInput() {
  if (debounceTimer) clearTimeout(debounceTimer)
  
  if (!searchQuery.value.trim()) {
    tracks.value = []
    isLoading.value = false
    return
  }
  
  isLoading.value = true
  debounceTimer = setTimeout(() => {
    performSearch(searchQuery.value)
  }, 500)
}

async function performSearch(query: string) {
  try {
    const result = await searchTracks(query, 0, 50)
    if (result && result.tracks) {
      tracks.value = result.tracks
    } else {
      tracks.value = []
    }
  } catch (error) {
    console.error('Search failed:', error)
    tracks.value = []
  } finally {
    isLoading.value = false
  }
}

function playTrack(track: LibraryTrack) {
  console.log('Playing track:', track.title)
}

function goToArtist(track: LibraryTrack) {
  if (track.artist_id) {
    router.push({ name: 'ArtistDetail', params: { id: track.artist_id.toString() } })
  }
}

function goToAlbum(track: LibraryTrack) {
  if (track.album_id) {
    router.push({ name: 'AlbumDetail', params: { id: track.album_id.toString() } })
  }
}
</script>
