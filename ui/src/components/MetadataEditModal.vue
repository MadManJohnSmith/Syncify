<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <!-- Backdrop -->
    <div 
      class="absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity" 
      @click="handleClose"
    ></div>

    <!-- Modal Card -->
    <div class="relative w-full max-w-[600px] flex flex-col rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl overflow-hidden transform transition-all animate-in fade-in zoom-in-95 duration-200">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-5 border-b border-gray-100 dark:border-border-dark">
        <div class="flex flex-col gap-1">
          <h2 class="text-xl font-bold tracking-tight text-gray-900 dark:text-white">Edit Metadata</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400">Update track details manually.</p>
        </div>
        <button 
          @click="handleClose"
          :disabled="isSaving"
          aria-label="Close modal" 
          class="rounded-full p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-white/10 transition-colors disabled:opacity-50"
        >
          <span class="material-symbols-outlined text-[24px]">close</span>
        </button>
      </div>

      <!-- Error Banner -->
      <div v-if="error" class="mx-6 mt-4 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 text-sm flex items-center gap-2">
        <span class="material-symbols-outlined text-[18px]">error</span>
        {{ error }}
        <button @click="error = null" class="ml-auto text-red-500 hover:text-red-700">
          <span class="material-symbols-outlined text-[16px]">close</span>
        </button>
      </div>

      <!-- Form Content -->
      <form @submit.prevent="saveMetadata" class="flex-1 overflow-y-auto p-6 space-y-4">
        
        <!-- Title & Artist -->
        <div class="grid grid-cols-1 gap-4">
          <div class="form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Title</label>
            <input 
              v-model="form.title" 
              type="text" 
              required
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
            >
          </div>
          
          <div class="form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Artist</label>
            <input 
              v-model="form.artist" 
              type="text" 
              required
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
            >
          </div>
        </div>

        <!-- Album & Year -->
        <div class="grid grid-cols-4 gap-4">
          <div class="col-span-3 form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Album</label>
            <input 
              v-model="form.album" 
              type="text" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
            >
          </div>
          
          <div class="col-span-1 form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Year</label>
            <input 
              v-model="form.year" 
              type="number" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
            >
          </div>
        </div>

        <!-- Genre & ISRC -->
        <div class="grid grid-cols-2 gap-4">
          <div class="form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Genre</label>
            <input 
              v-model="form.genre" 
              type="text" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
            >
          </div>
          
          <div class="form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">ISRC</label>
            <input 
              v-model="form.isrc" 
              type="text" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all font-mono"
            >
          </div>
        </div>

        <!-- BPM & Key -->
        <div class="grid grid-cols-3 gap-4">
          <div class="form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">BPM</label>
            <input 
              v-model="form.bpm" 
              type="number" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
            >
          </div>
          
          <div class="form-group">
            <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Key</label>
            <input 
              v-model="form.musicalKey" 
              type="text" 
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
            >
          </div>

          <div class="form-group flex flex-col justify-end pb-2">
            <label class="flex items-center gap-2 cursor-pointer">
              <input v-model="form.explicit" type="checkbox" class="w-4 h-4 text-primary bg-gray-100 border-gray-300 rounded focus:ring-primary dark:focus:ring-primary dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600">
              <span class="text-sm font-medium text-gray-700 dark:text-gray-300">Explicit</span>
            </label>
          </div>
        </div>

      </form>

      <!-- Footer Action -->
      <div class="bg-gray-50 dark:bg-[#121b29]/50 border-t border-gray-100 dark:border-border-dark px-6 py-4 flex justify-end gap-3">
        <button 
          @click="handleClose"
          :disabled="isSaving"
          class="px-4 py-2 text-sm font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white rounded-lg transition-colors focus:outline-none focus:ring-2 focus:ring-primary/50 disabled:opacity-50"
        >
          Cancel
        </button>
        <button 
          @click="saveMetadata"
          :disabled="isSaving"
          class="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <span v-if="isSaving" class="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
          {{ isSaving ? 'Saving...' : 'Save Changes' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';
import { metadataApi } from '@/api/metadata';
import type { LibraryTrack } from '@/api/types';

// Define a more flexible track input type that works with both LibraryTrack and MetadataTrack
interface TrackInput {
  id: number;
  title: string;
  artist_name?: string | null;
  album_name?: string | null;
  release_year?: number | null;
  genre?: string | null;
  isrc?: string | null;
  bpm?: number | null;
  musical_key?: string | null;
  explicit?: boolean | null;
}

const props = defineProps<{
  modelValue: boolean;
  track: TrackInput | null | undefined;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
  (e: 'saved', track: LibraryTrack): void;
}>();

// Reactive state
const isSaving = ref(false);
const error = ref<string | null>(null);

const form = ref({
  title: '',
  artist: '',
  album: '',
  year: null as number | null,
  genre: '',
  isrc: '',
  bpm: null as number | null,
  musicalKey: '',
  explicit: false,
});

// Watch for track changes to update form
watch(() => props.track, (newTrack) => {
  if (newTrack) {
    form.value = {
      title: newTrack.title || '',
      artist: newTrack.artist_name || '',
      album: newTrack.album_name || '',
      year: newTrack.release_year || null,
      genre: newTrack.genre || '',
      isrc: newTrack.isrc || '',
      bpm: newTrack.bpm || null,
      musicalKey: newTrack.musical_key || '',
      explicit: newTrack.explicit || false,
    };
  }
}, { immediate: true });

// Close handler
function handleClose() {
  if (!isSaving.value) {
    emit('update:modelValue', false);
    error.value = null;
  }
}

// Save handler
async function saveMetadata() {
  if (!props.track || isSaving.value) return;
  
  isSaving.value = true;
  error.value = null;
  
  try {
    const updatedTrack = await metadataApi.updateTrackMetadata(props.track.id, {
      title: form.value.title,
      artistName: form.value.artist,
      albumName: form.value.album,
      year: form.value.year || undefined,
      genre: form.value.genre,
      isrc: form.value.isrc,
      bpm: form.value.bpm || undefined,
      musicalKey: form.value.musicalKey,
      explicit: form.value.explicit,
    });
    
    emit('saved', updatedTrack);
    handleClose();
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to save metadata';
  } finally {
    isSaving.value = false;
  }
}
</script>
