<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div class="absolute inset-0 bg-black/60 backdrop-blur-sm" @click="close"></div>

        <div class="relative w-full max-w-2xl rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl overflow-hidden">
          <div class="flex items-center justify-between px-6 py-4 border-b border-gray-100 dark:border-border-dark">
            <div>
              <h2 class="text-xl font-bold text-gray-900 dark:text-white">Track Metadata</h2>
              <p class="text-sm text-gray-500 dark:text-gray-400">Read-only metadata snapshot</p>
            </div>
            <button
              class="rounded-full p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-white/10 transition-colors"
              aria-label="Close metadata modal"
              @click="close"
            >
              <span class="material-symbols-outlined text-[22px]">close</span>
            </button>
          </div>

          <div class="p-6">
            <div v-if="isLoading" class="py-8 text-center text-sm text-gray-500 dark:text-gray-400">
              Loading metadata...
            </div>

            <div v-else-if="error" class="rounded-lg border border-red-300 dark:border-red-700 bg-red-50 dark:bg-red-900/20 px-4 py-3 text-sm text-red-700 dark:text-red-300">
              {{ error }}
            </div>

            <div v-else-if="metadata" class="grid grid-cols-1 sm:grid-cols-2 gap-4 text-sm">
              <div class="meta-item sm:col-span-2">
                <span class="meta-label">Title</span>
                <span class="meta-value">{{ textOrDash(metadata.title) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Artist</span>
                <span class="meta-value">{{ textOrDash(metadata.artistName) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Album</span>
                <span class="meta-value">{{ textOrDash(metadata.albumName) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Track #</span>
                <span class="meta-value">{{ textOrDash(metadata.trackNumber) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Disc #</span>
                <span class="meta-value">{{ textOrDash(metadata.discNumber) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">ISRC</span>
                <span class="meta-value font-mono">{{ textOrDash(metadata.isrc) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">MusicBrainz ID</span>
                <span class="meta-value font-mono">{{ textOrDash(metadata.musicbrainzId) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Genre</span>
                <span class="meta-value">{{ textOrDash(metadata.genre) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Release Year</span>
                <span class="meta-value">{{ textOrDash(metadata.releaseYear) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">BPM</span>
                <span class="meta-value">{{ textOrDash(metadata.bpm) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Musical Key</span>
                <span class="meta-value">{{ textOrDash(metadata.musicalKey) }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Explicit</span>
                <span class="meta-value">{{ explicitLabel }}</span>
              </div>
              <div class="meta-item">
                <span class="meta-label">Duration (ms)</span>
                <span class="meta-value">{{ textOrDash(metadata.durationMs) }}</span>
              </div>
              <div class="meta-item sm:col-span-2">
                <span class="meta-label">File Path</span>
                <span class="meta-value font-mono break-all">{{ textOrDash(metadata.filePath) }}</span>
              </div>
            </div>

            <div v-else class="py-8 text-center text-sm text-gray-500 dark:text-gray-400">
              No metadata available.
            </div>
          </div>

          <div class="px-6 py-4 border-t border-gray-100 dark:border-border-dark flex justify-end">
            <button
              class="px-4 py-2 rounded-lg text-sm font-medium bg-gray-100 dark:bg-surface-highlight text-gray-700 dark:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
              @click="close"
            >
              Close
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { TrackMetadata } from '@/api/metadata'

const props = defineProps<{
  modelValue: boolean
  metadata: TrackMetadata | null
  isLoading: boolean
  error: string | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

const explicitLabel = computed(() => {
  if (!props.metadata || props.metadata.explicit === null) return '-'
  return props.metadata.explicit ? 'Yes' : 'No'
})

function close() {
  emit('update:modelValue', false)
}

function textOrDash(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return '-'
  const asText = String(value).trim()
  return asText.length > 0 ? asText : '-'
}
</script>

<style scoped>
.meta-item {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  border: 1px solid rgba(148, 163, 184, 0.25);
  border-radius: 0.625rem;
  padding: 0.6rem 0.75rem;
}

.meta-label {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: rgb(100 116 139);
}

.meta-value {
  color: rgb(17 24 39);
  font-weight: 500;
}

:deep(.dark) .meta-value {
  color: rgb(241 245 249);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
