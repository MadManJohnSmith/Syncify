<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4">
    <!-- Backdrop -->
    <div 
      class="absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity" 
      @click="handleClose"
    ></div>

    <!-- Modal Card -->
    <div class="relative w-full max-w-[800px] flex flex-col rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl overflow-hidden transform transition-all animate-in fade-in zoom-in-95 duration-200">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-5 border-b border-gray-100 dark:border-border-dark">
        <div class="flex flex-col gap-1">
          <h2 class="text-xl font-bold tracking-tight text-gray-900 dark:text-white">Connect New Service</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400">Sync your library across platforms.</p>
        </div>
        <button 
          @click="handleClose"
          :disabled="isConnecting"
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

      <!-- Scrollable Content -->
      <div class="flex-1 overflow-y-auto p-6 sm:p-8">
        <!-- Service Grid -->
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
          <!-- Spotify -->
          <button 
            @click="connectService('spotify')"
            :disabled="isConnecting"
            :class="getButtonClass('spotify')"
          >
            <div v-if="connectingService === 'spotify'" class="absolute inset-0 flex items-center justify-center bg-white/80 dark:bg-black/80 rounded-xl">
              <span class="material-symbols-outlined text-[24px] animate-spin text-primary">progress_activity</span>
            </div>
            <div class="relative h-14 w-14 flex items-center justify-center rounded-full bg-[#1ed760]/10 text-[#1ed760] group-hover:bg-[#1ed760] group-hover:text-white transition-colors">
              <span class="material-symbols-outlined text-[32px]">library_music</span>
            </div>
            <span class="text-sm font-semibold text-gray-900 dark:text-white group-hover:text-primary transition-colors">Spotify</span>
          </button>

          <!-- Apple Music -->
          <button 
            @click="connectService('apple_music')"
            :disabled="isConnecting"
            :class="getButtonClass('apple_music')"
          >
            <div v-if="connectingService === 'apple_music'" class="absolute inset-0 flex items-center justify-center bg-white/80 dark:bg-black/80 rounded-xl">
              <span class="material-symbols-outlined text-[24px] animate-spin text-primary">progress_activity</span>
            </div>
            <div class="relative h-14 w-14 flex items-center justify-center rounded-full bg-[#fa243c]/10 text-[#fa243c] group-hover:bg-[#fa243c] group-hover:text-white transition-colors">
              <span class="material-symbols-outlined text-[32px]">music_note</span>
            </div>
            <span class="text-sm font-semibold text-gray-900 dark:text-white group-hover:text-primary transition-colors">Apple Music</span>
          </button>

          <!-- Tidal -->
          <button 
            @click="connectService('tidal')"
            :disabled="isConnecting"
            :class="getButtonClass('tidal')"
          >
            <div v-if="connectingService === 'tidal'" class="absolute inset-0 flex items-center justify-center bg-white/80 dark:bg-black/80 rounded-xl">
              <span class="material-symbols-outlined text-[24px] animate-spin text-primary">progress_activity</span>
            </div>
            <div class="absolute top-3 left-3 rounded bg-primary/20 px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider text-primary border border-primary/20">Hi-Fi</div>
            <div class="relative h-14 w-14 flex items-center justify-center rounded-full bg-white dark:bg-black text-black dark:text-white group-hover:ring-2 ring-primary transition-all">
              <span class="font-extrabold text-2xl tracking-tighter italic">T</span>
            </div>
            <span class="text-sm font-semibold text-gray-900 dark:text-white group-hover:text-primary transition-colors">Tidal</span>
          </button>

          <!-- Qobuz -->
          <button 
            @click="connectService('qobuz')"
            :disabled="isConnecting"
            :class="getButtonClass('qobuz')"
          >
            <div v-if="connectingService === 'qobuz'" class="absolute inset-0 flex items-center justify-center bg-white/80 dark:bg-black/80 rounded-xl">
              <span class="material-symbols-outlined text-[24px] animate-spin text-primary">progress_activity</span>
            </div>
            <div class="relative h-14 w-14 flex items-center justify-center rounded-full bg-blue-900/10 text-blue-900 dark:bg-blue-500/10 dark:text-blue-400 group-hover:bg-blue-600 group-hover:text-white transition-colors">
              <span class="material-symbols-outlined text-[32px]">album</span>
            </div>
            <span class="text-sm font-semibold text-gray-900 dark:text-white group-hover:text-primary transition-colors">Qobuz</span>
          </button>

          <!-- Deezer -->
          <button 
            @click="connectService('deezer')"
            :disabled="isConnecting"
            :class="getButtonClass('deezer')"
          >
            <div v-if="connectingService === 'deezer'" class="absolute inset-0 flex items-center justify-center bg-white/80 dark:bg-black/80 rounded-xl">
              <span class="material-symbols-outlined text-[24px] animate-spin text-primary">progress_activity</span>
            </div>
            <div class="relative h-14 w-14 flex items-center justify-center rounded-full bg-purple-900/10 text-purple-900 dark:bg-purple-500/10 dark:text-purple-400 group-hover:bg-purple-600 group-hover:text-white transition-colors">
              <span class="material-symbols-outlined text-[32px]">graphic_eq</span>
            </div>
            <span class="text-sm font-semibold text-gray-900 dark:text-white group-hover:text-primary transition-colors">Deezer</span>
          </button>

          <!-- SoundCloud -->
          <button 
            @click="connectService('soundcloud')"
            :disabled="isConnecting"
            :class="getButtonClass('soundcloud')"
          >
            <div v-if="connectingService === 'soundcloud'" class="absolute inset-0 flex items-center justify-center bg-white/80 dark:bg-black/80 rounded-xl">
              <span class="material-symbols-outlined text-[24px] animate-spin text-primary">progress_activity</span>
            </div>
            <div class="relative h-14 w-14 flex items-center justify-center rounded-full bg-[#ff5500]/10 text-[#ff5500] group-hover:bg-[#ff5500] group-hover:text-white transition-colors">
              <span class="material-symbols-outlined text-[32px]">cloud</span>
            </div>
            <span class="text-sm font-semibold text-gray-900 dark:text-white group-hover:text-primary transition-colors">SoundCloud</span>
          </button>
        </div>
      </div>

      <!-- Footer Action -->
      <div class="bg-gray-50 dark:bg-[#121b29]/50 border-t border-gray-100 dark:border-border-dark px-6 py-4 flex justify-end gap-3">
        <button 
          @click="handleClose"
          :disabled="isConnecting"
          class="px-4 py-2 text-sm font-medium text-gray-600 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white rounded-lg transition-colors focus:outline-none focus:ring-2 focus:ring-primary/50 disabled:opacity-50"
        >
          Cancel
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { startAuthAndSave } from '@/api/accounts';

defineProps<{
  modelValue: boolean
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'connected', service: string, displayName: string): void
}>();

// Reactive state
const isConnecting = ref(false);
const connectingService = ref<string | null>(null);
const error = ref<string | null>(null);

// Button class helper
const baseButtonClass = 'group relative flex flex-col items-center justify-center gap-4 rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-[#121b29] p-6 transition-all duration-200 hover:border-primary hover:bg-white dark:hover:bg-[#1a2639] hover:shadow-glow focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 dark:focus:ring-offset-surface-dark';

function getButtonClass(service: string): string {
  if (isConnecting.value && connectingService.value !== service) {
    return baseButtonClass + ' opacity-50 cursor-not-allowed';
  }
  return baseButtonClass + ' disabled:opacity-50 disabled:cursor-not-allowed';
}

// Close handler
function handleClose() {
  if (!isConnecting.value) {
    emit('update:modelValue', false);
  }
}

// Connect to a service
async function connectService(serviceName: string) {
  if (isConnecting.value) return;
  
  isConnecting.value = true;
  connectingService.value = serviceName;
  error.value = null;
  
  try {
    const result = await startAuthAndSave(serviceName);
    
    if (result.success) {
      const displayName = result.data?.display_name as string || serviceName;
      emit('connected', serviceName, displayName);
      emit('update:modelValue', false);
    } else {
      error.value = result.error || `Failed to connect to ${serviceName}`;
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : `Failed to connect to ${serviceName}`;
  } finally {
    isConnecting.value = false;
    connectingService.value = null;
  }
}
</script>
