<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8"
      data-testid="playlist-conflict-reviews-dialog"
      @click.self="emit('close')"
    >
      <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[80vh] flex flex-col overflow-hidden shadow-2xl">
        <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-start justify-between gap-4">
          <div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Playlist conflicts</h3>
            <p class="text-xs text-gray-500 mt-1">
              {{ reviews.length }} pending. Nothing changes until you choose a side.
            </p>
          </div>
          <button
            type="button"
            aria-label="Close conflict review"
            class="p-1 rounded hover:bg-gray-100 dark:hover:bg-surface-highlight"
            @click="emit('close')"
          >
            <span class="material-symbols-outlined text-gray-500">close</span>
          </button>
        </div>

        <div v-if="!reviews.length" class="p-8 text-center text-sm text-gray-500" data-testid="playlist-conflict-reviews-empty">
          No playlist conflicts need review.
        </div>

        <div v-else class="flex-1 overflow-y-auto custom-scrollbar divide-y divide-gray-200 dark:divide-border-dark">
          <div
            v-for="review in reviews"
            :key="review.id"
            class="p-5 space-y-3"
            :data-testid="`playlist-conflict-review-${review.id}`"
          >
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="text-sm font-semibold text-gray-900 dark:text-white truncate">
                  {{ review.playlist_name }}
                  <span v-if="review.service_name" class="font-normal text-gray-500">· {{ review.service_name }}</span>
                </p>
                <p class="text-xs text-gray-500 mt-0.5">{{ kindLabel(review.conflict_kind) }}</p>
              </div>
              <span class="shrink-0 px-2 py-0.5 rounded-full text-[10px] font-medium bg-amber-100 dark:bg-amber-500/10 text-amber-700 dark:text-amber-400">
                Needs review
              </span>
            </div>

            <div class="grid grid-cols-2 gap-3 text-xs">
              <div class="p-3 rounded-lg bg-gray-50 dark:bg-surface-highlight">
                <p class="font-semibold text-gray-700 dark:text-gray-300 mb-1">Your local value</p>
                <p class="text-gray-600 dark:text-gray-400 break-words">{{ valueLabel(review, 'local') }}</p>
              </div>
              <div class="p-3 rounded-lg bg-gray-50 dark:bg-surface-highlight">
                <p class="font-semibold text-gray-700 dark:text-gray-300 mb-1">Provider value</p>
                <p class="text-gray-600 dark:text-gray-400 break-words">{{ valueLabel(review, 'provider') }}</p>
              </div>
            </div>

            <div class="flex justify-end gap-2">
              <button
                type="button"
                :data-testid="`conflict-keep-local-${review.id}`"
                :disabled="busy"
                class="px-3 py-1.5 text-xs rounded-lg border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight disabled:opacity-50"
                @click="emit('resolve', review.id, 'local')"
              >
                Keep local
              </button>
              <button
                type="button"
                :data-testid="`conflict-use-provider-${review.id}`"
                :disabled="busy"
                class="px-3 py-1.5 text-xs rounded-lg bg-primary text-on-accent hover:bg-primary-hover disabled:opacity-50"
                @click="emit('resolve', review.id, 'provider')"
              >
                Use provider
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import type {
  PlaylistConflictKind,
  PlaylistConflictResolution,
  PlaylistConflictReview,
} from '@/api/types'

defineProps<{
  open: boolean
  reviews: PlaylistConflictReview[]
  busy?: boolean
}>()

const emit = defineEmits<{
  (event: 'resolve', reviewId: number, resolution: PlaylistConflictResolution): void
  (event: 'close'): void
}>()

function kindLabel(kind: PlaylistConflictKind): string {
  return kind === 'membership' ? 'Membership conflict' : 'Name conflict'
}

function valueLabel(review: PlaylistConflictReview, side: 'local' | 'provider'): string {
  if (review.conflict_kind === 'name') {
    const value = side === 'local' ? review.local_value : review.provider_value
    return value ?? '—'
  }
  const trackIds = side === 'local' ? review.local_track_ids : review.provider_track_ids
  const decoded = trackIds ?? parseTrackIds(side === 'local' ? review.local_value : review.provider_value)
  if (!decoded) return '—'
  return `${decoded.length} track${decoded.length === 1 ? '' : 's'}`
}

function parseTrackIds(raw: string | null): number[] | null {
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) && parsed.every(id => typeof id === 'number') ? parsed : null
  } catch {
    return null
  }
}
</script>
