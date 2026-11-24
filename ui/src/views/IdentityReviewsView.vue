<script setup lang="ts">
/**
 * IdentityReviewsView.vue — T08b (#383).
 *
 * Human surface for `track_identity_reviews` (0115): the sync imported an
 * ambiguous provider item as its own track and froze both sides of the
 * comparison. This view lists the pending reviews, shows the provider item
 * next to the local candidates, and closes one only when a person presses an
 * action:
 *
 * - merge: the imported track is folded into the selected canonical track;
 *   sources, library entries, favorites, playlist entries and edition links
 *   are preserved;
 * - separate: the two recordings are different; nothing is touched;
 * - dismiss: the evidence no longer applies; nothing is touched.
 *
 * Merge is a two-step action (select a candidate → confirm) because it deletes
 * the imported track row after moving its data.
 */
import { computed, onMounted, ref } from 'vue'
import {
  getPendingTrackIdentityReviews,
  resolveTrackIdentityReview,
  type IdentityReviewCandidate,
  type IdentityReviewResolution,
  type TrackIdentityReview,
} from '@/api/trackIdentityReviews'
import { TauriError } from '@/api/tauri'

defineOptions({ name: 'IdentityReviewsView' })

const reviews = ref<TrackIdentityReview[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const selectedId = ref<number | null>(null)
const selectedCandidateId = ref<number | null>(null)
const confirmingMerge = ref(false)
const resolving = ref(false)
const resolutionError = ref<string | null>(null)
const notice = ref<string | null>(null)

const selectedReview = computed(
  () => reviews.value.find((review) => review.id === selectedId.value) ?? null,
)

const selectedCandidate = computed(
  () =>
    selectedReview.value?.candidates.find(
      (candidate) => candidate.track_id === selectedCandidateId.value,
    ) ?? null,
)

const canMerge = computed(
  () => selectedReview.value !== null && selectedCandidate.value !== null && !resolving.value,
)

function errorMessage(cause: unknown): string {
  if (cause instanceof TauriError) return cause.message
  if (cause instanceof Error) return cause.message
  return String(cause)
}

function evidence(value: string | null | undefined): string {
  const trimmed = value?.trim()
  return trimmed ? trimmed : '—'
}

function formatDuration(ms: number | null): string {
  if (ms === null || ms <= 0) return '—'
  const totalSeconds = Math.round(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${String(seconds).padStart(2, '0')}`
}

function formatDelta(ms: number | null): string {
  if (ms === null) return 'duration not published'
  if (ms === 0) return 'exact duration'
  return `±${(ms / 1000).toFixed(1)} s`
}

function reasonLabel(reason: string): string {
  return reason === 'fingerprint_multiple_candidates'
    ? 'Several local tracks match artist, title and duration'
    : reason
}

function candidateLabel(candidate: IdentityReviewCandidate): string {
  const artist = evidence(candidate.artist)
  return `${evidence(candidate.title)} — ${artist}`
}

function serviceAccountLabel(review: TrackIdentityReview): string {
  if (review.account_name) return `${review.service_name} · ${review.account_name}`
  return review.service_name
}

function selectReview(review: TrackIdentityReview): void {
  selectedId.value = review.id
  selectedCandidateId.value = null
  confirmingMerge.value = false
  resolutionError.value = null
  notice.value = null
}

function reconcileSelection(): void {
  if (reviews.value.length === 0) {
    selectedId.value = null
    selectedCandidateId.value = null
    confirmingMerge.value = false
    return
  }
  if (!reviews.value.some((review) => review.id === selectedId.value)) {
    selectReview(reviews.value[0])
  }
}

async function load(): Promise<void> {
  loading.value = true
  error.value = null
  resolutionError.value = null
  notice.value = null
  try {
    reviews.value = await getPendingTrackIdentityReviews()
    reconcileSelection()
  } catch (cause) {
    error.value = errorMessage(cause)
  } finally {
    loading.value = false
  }
}

function noticeFor(
  resolution: IdentityReviewResolution,
  result: Awaited<ReturnType<typeof resolveTrackIdentityReview>>,
  review: TrackIdentityReview,
  candidate: IdentityReviewCandidate | null,
): string {
  if (resolution === 'merged') {
    const summary = result.merge
    const moved = summary
      ? ` ${summary.sources_moved} source(s) and ${summary.memberships_moved} membership(s) moved.`
      : ''
    return `Merged “${evidence(candidate?.title)}” into track #${result.merged_track_id}.${moved}`
  }
  if (resolution === 'separate') {
    return `Review #${review.id} closed as separate. No track was changed.`
  }
  return `Review #${review.id} dismissed. No track was changed.`
}

async function applyResolution(resolution: IdentityReviewResolution): Promise<void> {
  const review = selectedReview.value
  if (!review || resolving.value) return
  if (resolution === 'merged' && selectedCandidateId.value === null) {
    resolutionError.value = 'Select the canonical track this item belongs to.'
    return
  }

  const chosenCandidate = selectedCandidate.value
  resolving.value = true
  resolutionError.value = null
  notice.value = null
  try {
    const result = await resolveTrackIdentityReview(
      review.id,
      resolution,
      resolution === 'merged' ? selectedCandidateId.value : null,
    )
    const message = noticeFor(resolution, result, review, chosenCandidate)
    confirmingMerge.value = false
    await load()
    notice.value = message
  } catch (cause) {
    resolutionError.value = errorMessage(cause)
  } finally {
    resolving.value = false
  }
}

onMounted(load)
</script>

<template>
  <div class="identity-reviews h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <header class="px-8 pt-8 pb-5 flex items-start justify-between gap-4 shrink-0">
      <div class="flex items-center gap-4 min-w-0">
        <div class="h-12 w-12 rounded-full bg-gradient-to-br from-amber-500 to-orange-400 text-white flex items-center justify-center shrink-0">
          <span class="material-symbols-outlined text-[26px]" aria-hidden="true">fact_check</span>
        </div>
        <div class="min-w-0">
          <h1 class="text-3xl font-bold tracking-tight text-gray-900 dark:text-white mb-1">Identity reviews</h1>
          <p class="text-text-secondary">
            Ambiguous imports the sync did not merge. Choose what each provider item really is.
          </p>
        </div>
      </div>
      <div class="flex items-center gap-3 shrink-0">
        <span class="px-3 py-1 rounded-full text-xs font-medium bg-amber-500/10 text-amber-600 dark:text-amber-400">
          {{ reviews.length }} pending
        </span>
        <button
          type="button"
          data-testid="identity-reviews-refresh"
          class="px-3.5 py-2 rounded-lg text-sm font-medium bg-gray-100 hover:bg-gray-200 dark:bg-surface-highlight dark:hover:bg-white/10 text-gray-700 dark:text-gray-200 transition-colors"
          :disabled="loading || resolving"
          @click="load"
        >
          <span class="material-symbols-outlined text-[18px] align-middle mr-1">refresh</span>
          Refresh
        </button>
      </div>
    </header>

    <div class="flex-1 min-h-0 px-8 pb-8">
      <p
        v-if="resolutionError"
        data-testid="identity-resolution-error"
        class="mb-3 text-sm text-red-600 dark:text-red-400 rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2"
        role="alert"
      >
        {{ resolutionError }}
      </p>
      <p
        v-if="notice"
        data-testid="identity-resolution-notice"
        class="mb-3 text-sm text-green-600 dark:text-green-400 rounded-lg border border-green-500/30 bg-green-500/10 px-3 py-2"
        role="status"
      >
        {{ notice }}
      </p>

      <div
        v-if="loading"
        data-testid="identity-reviews-loading"
        class="h-full flex items-center justify-center text-text-secondary"
        role="status"
        aria-live="polite"
      >
        <span class="material-symbols-outlined animate-spin mr-3">progress_activity</span>
        Loading identity reviews…
      </div>

      <div
        v-else-if="error"
        data-testid="identity-reviews-error"
        class="rounded-xl border border-red-500/30 bg-red-500/10 px-5 py-4"
        role="alert"
      >
        <p class="font-semibold text-red-600 dark:text-red-400">The review queue could not be loaded.</p>
        <p class="text-sm text-text-secondary mt-1">{{ error }}</p>
        <button
          type="button"
          data-testid="identity-reviews-retry"
          class="mt-3 px-3 py-1.5 rounded-lg text-sm font-semibold bg-primary text-on-accent"
          @click="load"
        >
          Retry
        </button>
      </div>

      <div
        v-else-if="reviews.length === 0"
        data-testid="identity-reviews-empty"
        class="h-full flex items-center justify-center"
      >
        <div class="max-w-md text-center space-y-3">
          <span class="material-symbols-outlined text-5xl text-gray-300 dark:text-gray-600" aria-hidden="true">task_alt</span>
          <h2 class="text-xl font-bold text-ink">No ambiguous identities</h2>
          <p class="text-sm text-text-secondary">
            Every imported track was linked unambiguously. New reviews appear here after a sync
            finds more than one local track that matches an artist, title and duration.
          </p>
        </div>
      </div>

      <div v-else class="h-full min-h-0 grid grid-cols-1 lg:grid-cols-[minmax(16rem,22rem)_1fr] gap-5">
        <aside class="min-h-0 overflow-y-auto custom-scrollbar space-y-2" aria-label="Pending identity reviews">
          <button
            v-for="review in reviews"
            :key="review.id"
            type="button"
            :data-testid="`identity-review-item-${review.id}`"
            :aria-current="review.id === selectedId ? 'true' : undefined"
            :class="[
              'w-full text-left px-4 py-3 rounded-xl border transition-colors',
              review.id === selectedId
                ? 'border-primary/60 bg-primary/10'
                : 'border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark hover:border-primary/40'
            ]"
            @click="selectReview(review)"
          >
            <div class="flex items-start justify-between gap-2">
              <p class="font-semibold text-sm text-gray-900 dark:text-white truncate">
                {{ evidence(review.incoming_title) }}
              </p>
              <span class="text-[10px] uppercase tracking-wide text-text-secondary shrink-0">
                #{{ review.id }}
              </span>
            </div>
            <p class="text-xs text-text-secondary truncate mt-0.5">
              {{ evidence(review.incoming_artist) }} · {{ serviceAccountLabel(review) }}
            </p>
            <p class="text-[11px] text-amber-600 dark:text-amber-400 mt-1">
              {{ review.candidates.length }} local candidate{{ review.candidates.length === 1 ? '' : 's' }}
            </p>
          </button>
        </aside>

        <section
          v-if="selectedReview"
          data-testid="identity-review-detail"
          class="min-h-0 overflow-y-auto custom-scrollbar rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark p-5"
          :aria-label="`Identity review ${selectedReview.id}`"
        >
          <div class="flex items-start justify-between gap-4 border-b border-gray-200 dark:border-border-dark pb-4">
            <div>
              <p class="text-xs uppercase tracking-wide text-text-secondary">Review #{{ selectedReview.id }}</p>
              <h2 class="text-lg font-semibold text-gray-900 dark:text-white mt-0.5">
                {{ evidence(selectedReview.incoming_title) }} — {{ evidence(selectedReview.incoming_artist) }}
              </h2>
              <p class="text-xs text-text-secondary mt-1">
                {{ serviceAccountLabel(selectedReview) }} · item {{ selectedReview.service_track_id }}
              </p>
            </div>
            <span class="px-2.5 py-1 rounded-full text-[11px] font-medium bg-amber-500/10 text-amber-600 dark:text-amber-400 shrink-0">
              pending
            </span>
          </div>

          <p class="text-sm text-text-secondary mt-3">
            {{ reasonLabel(selectedReview.reason) }}. Nothing is merged until you confirm it.
          </p>

          <div class="grid grid-cols-1 xl:grid-cols-2 gap-4 mt-5">
            <article class="rounded-xl border border-gray-200 dark:border-border-dark p-4 bg-gray-50/60 dark:bg-surface-highlight/20">
              <h3 class="text-xs uppercase tracking-wide text-text-secondary mb-3">Provider item</h3>
              <dl class="space-y-2 text-sm">
                <div class="flex justify-between gap-3">
                  <dt class="text-text-secondary">Title</dt>
                  <dd class="text-gray-900 dark:text-white text-right">{{ evidence(selectedReview.incoming_title) }}</dd>
                </div>
                <div class="flex justify-between gap-3">
                  <dt class="text-text-secondary">Artist</dt>
                  <dd class="text-gray-900 dark:text-white text-right">{{ evidence(selectedReview.incoming_artist) }}</dd>
                </div>
                <div class="flex justify-between gap-3">
                  <dt class="text-text-secondary">Album</dt>
                  <dd class="text-gray-900 dark:text-white text-right">{{ evidence(selectedReview.incoming_album) }}</dd>
                </div>
                <div class="flex justify-between gap-3">
                  <dt class="text-text-secondary">Duration</dt>
                  <dd class="text-gray-900 dark:text-white text-right">{{ formatDuration(selectedReview.incoming_duration_ms) }}</dd>
                </div>
                <div class="flex justify-between gap-3">
                  <dt class="text-text-secondary">ISRC</dt>
                  <dd class="text-gray-900 dark:text-white text-right font-mono text-xs">{{ evidence(selectedReview.incoming_isrc) }}</dd>
                </div>
              </dl>
              <p v-if="selectedReview.linked_track_id !== null" class="text-[11px] text-text-secondary mt-3">
                Imported as track #{{ selectedReview.linked_track_id }}. A merge folds that row into the track you choose.
              </p>
              <p v-else class="text-[11px] text-text-secondary mt-3">
                This item is no longer linked to a library track; only “Keep separate” or “Dismiss” can close it.
              </p>
            </article>

            <article class="rounded-xl border border-gray-200 dark:border-border-dark p-4">
              <h3 class="text-xs uppercase tracking-wide text-text-secondary mb-3">Local candidates</h3>
              <p v-if="selectedReview.candidates.length === 0" class="text-sm text-text-secondary">
                The frozen evidence has no candidates; do not merge it.
              </p>
              <ul v-else class="space-y-2" role="radiogroup" aria-label="Canonical track to keep">
                <li v-for="candidate in selectedReview.candidates" :key="candidate.track_id">
                  <label
                    :data-testid="`identity-candidate-${candidate.track_id}`"
                    :class="[
                      'flex items-start gap-3 rounded-lg border px-3 py-2.5 cursor-pointer transition-colors',
                      candidate.track_id === selectedCandidateId
                        ? 'border-primary/60 bg-primary/10'
                        : 'border-gray-200 dark:border-border-dark hover:border-primary/40'
                    ]"
                  >
                    <input
                      v-model="selectedCandidateId"
                      type="radio"
                      name="identity-canonical-track"
                      class="mt-1 accent-primary"
                      :value="candidate.track_id"
                    />
                    <span class="min-w-0 flex-1">
                      <span class="block text-sm font-medium text-gray-900 dark:text-white truncate">
                        {{ candidateLabel(candidate) }}
                      </span>
                      <span class="block text-xs text-text-secondary truncate">
                        {{ evidence(candidate.album) }} · {{ formatDuration(candidate.duration_ms) }} · track #{{ candidate.track_id }}
                      </span>
                      <span class="block text-[11px] text-text-secondary mt-0.5">
                        {{ formatDelta(candidate.duration_delta_ms) }}
                        <template v-if="candidate.isrc"> · ISRC {{ candidate.isrc }}</template>
                      </span>
                    </span>
                  </label>
                </li>
              </ul>
            </article>
          </div>

          <div
            v-if="confirmingMerge"
            data-testid="identity-merge-confirmation"
            class="mt-4 rounded-xl border border-amber-500/40 bg-amber-500/10 px-4 py-3"
            role="alertdialog"
            aria-labelledby="identity-merge-confirmation-title"
          >
            <p id="identity-merge-confirmation-title" class="text-sm font-semibold text-amber-700 dark:text-amber-300">
              Merge into track #{{ selectedCandidateId }}?
            </p>
            <p class="text-xs text-text-secondary mt-1">
              The imported track will be removed after its sources, library memberships, favorites
              and playlist entries move to the chosen track. This cannot be undone from this view.
            </p>
            <div class="flex gap-2 mt-3">
              <button
                type="button"
                data-testid="identity-confirm-merge"
                class="px-3 py-1.5 rounded-lg text-sm font-semibold bg-primary text-on-accent disabled:opacity-50"
                :disabled="resolving"
                @click="applyResolution('merged')"
              >
                Confirm merge
              </button>
              <button
                type="button"
                data-testid="identity-cancel-merge"
                class="px-3 py-1.5 rounded-lg text-sm font-medium bg-gray-100 hover:bg-gray-200 dark:bg-surface-highlight dark:hover:bg-white/10 text-gray-700 dark:text-gray-200"
                :disabled="resolving"
                @click="confirmingMerge = false"
              >
                Cancel
              </button>
            </div>
          </div>

          <div v-else class="flex flex-wrap items-center gap-2 mt-5">
            <button
              type="button"
              data-testid="identity-merge"
              class="px-3.5 py-2 rounded-lg text-sm font-semibold bg-primary text-on-accent disabled:opacity-50 disabled:cursor-not-allowed"
              :disabled="!canMerge"
              @click="confirmingMerge = true"
            >
              Merge into selected track
            </button>
            <button
              type="button"
              data-testid="identity-resolve-separate"
              class="px-3.5 py-2 rounded-lg text-sm font-medium bg-gray-100 hover:bg-gray-200 dark:bg-surface-highlight dark:hover:bg-white/10 text-gray-700 dark:text-gray-200 disabled:opacity-50"
              :disabled="resolving"
              @click="applyResolution('separate')"
            >
              They are different
            </button>
            <button
              type="button"
              data-testid="identity-resolve-dismissed"
              class="px-3.5 py-2 rounded-lg text-sm font-medium bg-transparent border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-text-secondary disabled:opacity-50"
              :disabled="resolving"
              @click="applyResolution('dismissed')"
            >
              Dismiss evidence
            </button>
            <span v-if="resolving" class="text-xs text-text-secondary" role="status">Saving…</span>
          </div>
        </section>
      </div>
    </div>
  </div>
</template>
