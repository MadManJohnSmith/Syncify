<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="modelValue" class="fixed inset-0 bg-black/50 flex items-center justify-center z-[60] p-4" @click.self="close">
        <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-4xl max-h-[90vh] flex flex-col shadow-2xl overflow-hidden">
          
          <!-- Header -->
          <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
            <div class="flex items-center gap-3">
              <div class="h-10 w-10 rounded-xl bg-purple-500/10 flex items-center justify-center">
                <span class="material-symbols-outlined text-purple-500 text-2xl">database</span>
              </div>
              <div>
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">MusicBrainz Match</h3>
                <p class="text-sm text-text-secondary">Find better metadata for "{{ track?.title }}"</p>
              </div>
            </div>
            <button @click="close" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
              <span class="material-symbols-outlined text-gray-500">close</span>
            </button>
          </div>

          <!-- Content -->
          <div class="flex-1 overflow-hidden flex flex-col md:flex-row">
            
            <!-- Search Panel -->
            <div class="w-full md:w-80 border-r border-gray-200 dark:border-border-dark flex flex-col bg-gray-50 dark:bg-surface-highlight/30 p-5 overflow-y-auto">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-4">Search Parameters</h4>
              
              <div class="space-y-4">
                <div>
                  <label class="block text-xs font-medium text-text-secondary mb-1">Artist</label>
                  <input 
                    type="text" 
                    v-model="searchParams.artist" 
                    class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
                    placeholder="Artist name"
                    @keyup.enter="search"
                  >
                </div>
                <div>
                  <label class="block text-xs font-medium text-text-secondary mb-1">Title</label>
                  <input 
                    type="text" 
                    v-model="searchParams.title" 
                    class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
                    placeholder="Track title"
                    @keyup.enter="search"
                  >
                </div>
                <div>
                  <label class="block text-xs font-medium text-text-secondary mb-1">Album</label>
                  <input 
                    type="text" 
                    v-model="searchParams.album" 
                    class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
                    placeholder="Album (optional)"
                    @keyup.enter="search"
                  >
                </div>
                
                <button 
                  @click="search"
                  :disabled="isSearching"
                  class="w-full mt-2 px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors flex items-center justify-center gap-2 disabled:opacity-50"
                >
                  <span v-if="isSearching" class="material-symbols-outlined text-[18px] animate-spin">progress_activity</span>
                  {{ isSearching ? 'Searching...' : 'Search MusicBrainz' }}
                </button>
              </div>

              <!-- Current Metadata Summary -->
              <div class="mt-8 pt-6 border-t border-gray-200 dark:border-border-dark">
                <h4 class="text-xs font-semibold text-text-secondary uppercase tracking-wider mb-3">Current Metadata</h4>
                <div class="space-y-2 text-sm">
                  <div>
                    <span class="text-xs text-text-secondary">Artist:</span>
                    <p class="font-medium text-gray-900 dark:text-white truncate">{{ track?.artist || '-' }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Title:</span>
                    <p class="font-medium text-gray-900 dark:text-white truncate">{{ track?.title || '-' }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Album:</span>
                    <p class="font-medium text-gray-900 dark:text-white truncate">{{ track?.album || '-' }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Year:</span>
                    <p class="font-medium text-gray-900 dark:text-white">{{ track?.year || '-' }}</p>
                  </div>
                </div>
              </div>
            </div>

            <!-- Results Panel -->
            <div class="flex-1 flex flex-col bg-white dark:bg-surface-dark overflow-hidden">
              <div v-if="hasSearched" class="flex-1 overflow-y-auto custom-scrollbar p-6">
                <!-- Results Header -->
                <div class="flex items-center justify-between mb-4">
                  <h4 class="font-semibold text-gray-900 dark:text-white">
                    Found {{ results.length }} matches
                  </h4>
                  <span class="text-xs text-text-secondary">Sorted by relevance</span>
                </div>

                <!-- Empty State -->
                <div v-if="results.length === 0" class="flex flex-col items-center justify-center py-12 text-center">
                  <div class="h-16 w-16 bg-gray-100 dark:bg-surface-highlight rounded-full flex items-center justify-center mb-4">
                    <span class="material-symbols-outlined text-3xl text-gray-400">search_off</span>
                  </div>
                  <h3 class="text-base font-semibold text-gray-900 dark:text-white">No matches found</h3>
                  <p class="text-sm text-text-secondary mt-1 max-w-xs">
                    Try adjusting your search terms or removing the album filter to find more results.
                  </p>
                </div>

                <!-- Results List -->
                <div v-else class="space-y-3">
                  <div 
                    v-for="result in results" 
                    :key="result.recording_id"
                    :class="[
                      'group relative p-4 rounded-xl border border-gray-200 dark:border-border-dark transition-all hover:border-primary/50 hover:shadow-md bg-white dark:bg-surface-dark',
                      selectedResult?.recording_id === result.recording_id ? 'ring-2 ring-primary border-transparent' : ''
                    ]"
                  >
                    <div class="flex items-start gap-4">
                      <!-- Score Badge -->
                      <div class="shrink-0 flex flex-col items-center gap-1">
                        <div :class="['w-10 h-10 rounded-full flex items-center justify-center font-bold text-xs', getScoreClass(result.score)]">
                          {{ result.score }}
                        </div>
                      </div>

                      <!-- Info -->
                      <div class="flex-1 min-w-0">
                        <h4 class="font-semibold text-gray-900 dark:text-white text-base mb-0.5 truncate bg-transparent">
                          {{ result.title }}
                        </h4>
                        <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-text-secondary">
                          <span class="font-medium text-gray-700 dark:text-gray-300">{{ result.artist }}</span>
                          <span class="text-gray-300 dark:text-gray-600">•</span>
                          <span>{{ result.album || 'No Album' }}</span>
                          <span v-if="result.release_date" class="px-1.5 py-0.5 bg-gray-100 dark:bg-surface-highlight rounded text-xs ml-1">
                            {{ result.release_date.substring(0, 4) }}
                          </span>
                        </div>
                        
                        <!-- Diff Preview (simplified) -->
                         <div class="mt-3 flex gap-4 text-xs">
                           <span v-if="result.title !== track?.title" class="text-success flex items-center gap-1">
                             <span class="material-symbols-outlined text-[14px]">edit</span> New Title
                           </span>
                           <span v-if="result.artist !== track?.artist" class="text-success flex items-center gap-1">
                             <span class="material-symbols-outlined text-[14px]">person</span> New Artist
                           </span>
                           <span v-if="result.album && result.album !== track?.album" class="text-success flex items-center gap-1">
                             <span class="material-symbols-outlined text-[14px]">album</span> New Album
                           </span>
                         </div>
                      </div>

                      <!-- Action -->
                      <button 
                        @click="applyMatch(result)"
                        :disabled="isApplying"
                        class="shrink-0 px-4 py-2 bg-primary/10 text-primary hover:bg-primary hover:text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed group-hover:bg-primary group-hover:text-white"
                      >
                         {{ isApplying && selectedResult?.recording_id === result.recording_id ? 'Applying...' : 'Apply Match' }}
                      </button>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Initial State -->
              <div v-else class="flex-1 flex flex-col items-center justify-center p-8 text-center bg-gray-50 dark:bg-surface-highlight/10">
                <div class="h-20 w-20 bg-purple-100 dark:bg-purple-900/20 rounded-full flex items-center justify-center mb-6">
                  <span class="material-symbols-outlined text-4xl text-purple-500">manage_search</span>
                </div>
                <h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">Search MusicBrainz</h3>
                <p class="text-text-secondary max-w-md">
                  Search the authentic MusicBrainz database to find canonical metadata for this track. Matches provided include official titles, artists, and release info.
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, reactive, watch } from 'vue'
import { metadataApi } from '@/api/metadata'
import type { LibraryTrack } from '@/api/types'

// Types
interface Props {
  modelValue: boolean
  track: LibraryTrack | any | null // Accept MetadataTrack or LibraryTrack
}

interface MatchResult {
  recording_id: string
  title: string
  artist: string
  album?: string
  release_date?: string
  score: number
  source: string
}

const props = defineProps<Props>()
const emit = defineEmits(['update:modelValue', 'saved'])

// State
const isSearching = ref(false)
const isApplying = ref(false)
const hasSearched = ref(false)
const results = ref<MatchResult[]>([])
const selectedResult = ref<MatchResult | null>(null)

const searchParams = reactive({
  artist: '',
  title: '',
  album: '',
})

// Initialize form from track
watch(() => props.modelValue, (isOpen) => {
  if (isOpen && props.track) {
    searchParams.artist = props.track.artist || props.track.artist_name || ''
    searchParams.title = props.track.title || ''
    searchParams.album = props.track.album || props.track.album_name || ''
    results.value = []
    hasSearched.value = false
    selectedResult.value = null
    
    // Auto-search if we have minimum data
    if (searchParams.artist && searchParams.title) {
      search()
    }
  }
})

function close() {
  emit('update:modelValue', false)
}

async function search() {
  if (!searchParams.title && !searchParams.artist) return
  
  isSearching.value = true
  hasSearched.value = true
  results.value = []
  
  try {
    const matches = await metadataApi.matchMusicBrainz({
      title: searchParams.title,
      artist: searchParams.artist,
      album: searchParams.album || undefined
    })
    
    // Convert to internal MatchResult type if needed (backend returns compatible struct)
    results.value = matches as unknown as MatchResult[]
  } catch (error) {
    console.error('MusicBrainz search failed:', error)
  } finally {
    isSearching.value = false
  }
}

async function applyMatch(match: MatchResult) {
  if (!props.track) return
  
  selectedResult.value = match
  isApplying.value = true
  
  try {
    await metadataApi.applyMusicBrainzMatch(props.track.id, match.recording_id)
    emit('saved', { 
      ...props.track, // Helper to return simplified logic, actual data reload is preferred
      musicbrainz_id: match.recording_id // Update ID at least
    })
    emit('update:modelValue', false)
  } catch (error) {
    console.error('Failed to apply MusicBrainz match:', error)
  } finally {
    isApplying.value = false
  }
}

function getScoreClass(score: number): string {
  if (score >= 90) return 'bg-success text-white'
  if (score >= 70) return 'bg-amber-500 text-white'
  return 'bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300'
}
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: rgba(156, 163, 175, 0.5);
  border-radius: 3px;
}
</style>
