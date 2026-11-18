<template>
  <section class="download-jobs-panel" data-testid="download-jobs-panel">
    <div class="flex items-center justify-between mb-3 flex-wrap gap-2">
      <div class="flex items-center gap-2.5">
        <h2 class="text-base font-bold text-gray-900 dark:text-white">Download jobs</h2>
        <span class="px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs font-bold">{{ jobs.length }}</span>
        <span v-if="terminalCount > 0" class="text-xs text-text-secondary">
          {{ terminalCount }} finished
        </span>
      </div>
      <button
        type="button"
        @click="$emit('clear')"
        :disabled="terminalCount === 0 || clearing"
        data-testid="job-clear-finished"
        title="Remove finished jobs from this list; the downloads ledger stays intact"
        class="flex items-center gap-1.5 px-3 py-1.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-xs font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-xs"
      >
        <span class="material-symbols-outlined text-[15px]">delete_sweep</span>
        <span>Clear finished</span>
      </button>
    </div>

    <div v-if="jobs.length === 0" class="rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark p-5 text-center text-xs text-text-secondary" data-testid="download-jobs-empty">
      No download jobs yet. Create one from the download button or from favorites.
    </div>

    <div v-else class="flex flex-col gap-2">
      <article
        v-for="job in jobs"
        :key="job.id"
        data-testid="download-job-row"
        :data-job-id="job.id"
        class="download-job-row rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm px-4 py-3"
      >
        <div class="flex items-start justify-between gap-3 flex-wrap">
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2 flex-wrap">
              <h3 class="text-sm font-semibold text-gray-900 dark:text-white truncate">
                {{ jobTitle(job) }}
              </h3>
              <span class="text-[11px] text-gray-400 font-mono">#{{ job.id }}</span>
              <span :class="['px-2 py-0.5 rounded text-[10px] font-bold uppercase', statusMeta(job).badgeClass]" data-testid="job-status">
                {{ statusMeta(job).label }}
              </span>
              <span v-if="job.kind" class="px-2 py-0.5 rounded text-[10px] font-bold bg-gray-100 dark:bg-surface-highlight text-text-secondary uppercase">
                {{ job.kind }}
              </span>
            </div>

            <div class="mt-1 flex items-center gap-2 flex-wrap text-[11px] text-text-secondary">
              <span v-if="job.sourceService || job.destinationService">
                {{ serviceLabel(job.sourceService) || 'Library' }}
                <span class="material-symbols-outlined text-[12px] align-middle">arrow_forward</span>
                {{ serviceLabel(job.destinationService) || 'Auto' }}
              </span>
              <span class="flex items-center gap-1" data-testid="job-account">
                <span class="material-symbols-outlined text-[13px]">manage_accounts</span>
                {{ accountLabel(job) }}
              </span>
              <span class="flex items-center gap-1" data-testid="job-quality">
                <span class="material-symbols-outlined text-[13px]">high_quality</span>
                {{ qualityLabel(job.qualityPreference) }}
              </span>
            </div>
          </div>

          <div class="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              @click="$emit('cancel', job.id)"
              :disabled="busy || counts(job).queued === 0 || statusMeta(job).status !== 'running'"
              data-testid="job-cancel"
              :title="cancelTitle(job)"
              class="flex items-center gap-1 px-2.5 py-1.5 bg-gray-100 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-200 rounded-lg text-[11px] font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-xs"
            >
              <span class="material-symbols-outlined text-[14px]">cancel</span>
              <span>Cancel pending</span>
            </button>
            <button
              type="button"
              @click="$emit('retry', job.id)"
              :disabled="busy || counts(job).failed === 0"
              data-testid="job-retry"
              title="Requeue only this job's failed tracks"
              class="flex items-center gap-1 px-2.5 py-1.5 bg-amber-500/10 border border-amber-500/30 hover:bg-amber-500/20 text-amber-600 dark:text-amber-400 rounded-lg text-[11px] font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-xs"
            >
              <span class="material-symbols-outlined text-[14px]">refresh</span>
              <span>Retry failed</span>
            </button>
          </div>
        </div>

        <!-- Progress -->
        <div class="mt-2.5 flex items-center gap-3">
          <div class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden" :title="`${counts(job).percent}% of the job finished`">
            <div
              class="h-full bg-primary rounded-full transition-all duration-300"
              :style="{ width: Math.min(100, Math.max(0, counts(job).percent)) + '%' }"
              data-testid="job-progress-bar"
            ></div>
          </div>
          <span class="text-[11px] font-bold text-primary font-mono w-12 text-right" data-testid="job-percent">
            {{ counts(job).percent }}%
          </span>
        </div>

        <!-- Counters -->
        <div class="mt-2 grid grid-cols-3 sm:grid-cols-6 gap-1.5 text-center" data-testid="job-counters">
          <div class="rounded-lg bg-gray-50 dark:bg-surface-highlight/40 px-2 py-1.5">
            <div class="text-xs font-bold text-gray-900 dark:text-white" data-testid="job-counter-requested">{{ counts(job).requested.toLocaleString() }}</div>
            <div class="text-[9px] uppercase tracking-wide text-text-secondary">Requested</div>
          </div>
          <div class="rounded-lg bg-gray-50 dark:bg-surface-highlight/40 px-2 py-1.5">
            <div class="text-xs font-bold text-amber-500" data-testid="job-counter-queued">{{ counts(job).queued.toLocaleString() }}</div>
            <div class="text-[9px] uppercase tracking-wide text-text-secondary">Queued</div>
          </div>
          <div class="rounded-lg bg-gray-50 dark:bg-surface-highlight/40 px-2 py-1.5">
            <div class="text-xs font-bold text-primary" data-testid="job-counter-downloading">{{ counts(job).downloading.toLocaleString() }}</div>
            <div class="text-[9px] uppercase tracking-wide text-text-secondary">Downloading</div>
          </div>
          <div class="rounded-lg bg-gray-50 dark:bg-surface-highlight/40 px-2 py-1.5">
            <div class="text-xs font-bold text-success" data-testid="job-counter-completed">{{ counts(job).completed.toLocaleString() }}</div>
            <div class="text-[9px] uppercase tracking-wide text-text-secondary">Completed</div>
          </div>
          <div class="rounded-lg bg-gray-50 dark:bg-surface-highlight/40 px-2 py-1.5">
            <div class="text-xs font-bold text-error" data-testid="job-counter-failed">{{ counts(job).failed.toLocaleString() }}</div>
            <div class="text-[9px] uppercase tracking-wide text-text-secondary">Failed</div>
          </div>
          <div class="rounded-lg bg-gray-50 dark:bg-surface-highlight/40 px-2 py-1.5">
            <div class="text-xs font-bold text-text-secondary" data-testid="job-counter-skipped">{{ counts(job).skipped.toLocaleString() }}</div>
            <div class="text-[9px] uppercase tracking-wide text-text-secondary">Skipped</div>
          </div>
        </div>

        <p
          v-if="counts(job).downloading > 0 && statusMeta(job).status === 'running'"
          class="mt-2 text-[11px] text-amber-600 dark:text-amber-400 flex items-center gap-1"
          data-testid="job-active-notice"
        >
          <span class="material-symbols-outlined text-[13px]">info</span>
          The active download will finish; only queued rows are cancelled.
        </p>
        <p v-if="job.error" class="mt-1.5 text-[11px] text-error truncate" :title="job.error">{{ job.error }}</p>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { DownloadJob, DownloadJobProgress, DownloadJobStatus } from '@/api/downloadJobs'
import { formatServiceName } from '@/composables/useGlobalTasks'

const props = defineProps<{
  jobs: DownloadJob[]
  progress: Record<number, DownloadJobProgress>
  accountLabels: Record<number, string>
  busy?: boolean
  clearing?: boolean
}>()

defineEmits<{
  (e: 'cancel', jobId: number): void
  (e: 'retry', jobId: number): void
  (e: 'clear'): void
}>()

interface JobCounts {
  requested: number
  queued: number
  downloading: number
  completed: number
  failed: number
  skipped: number
  percent: number
}

const STATUS_META: Record<DownloadJobStatus, { label: string; badgeClass: string }> = {
  running: { label: 'Running', badgeClass: 'bg-primary/15 text-primary border border-primary/20' },
  completed: { label: 'Completed', badgeClass: 'bg-emerald-500/15 text-emerald-500 border border-emerald-500/20' },
  partial: { label: 'Partial', badgeClass: 'bg-amber-500/15 text-amber-500 border border-amber-500/20' },
  cancelled: { label: 'Cancelled', badgeClass: 'bg-gray-500/15 text-gray-400 border border-gray-500/20' },
  failed: { label: 'Failed', badgeClass: 'bg-error/15 text-error border border-error/20' },
}

const terminalCount = computed(
  () => props.jobs.filter((job) => isTerminal(statusMeta(job).status)).length,
)

function isTerminal(status: DownloadJobStatus): boolean {
  return status !== 'running'
}

function statusMeta(job: DownloadJob): { status: DownloadJobStatus; label: string; badgeClass: string } {
  const event = props.progress[job.id]
  // A persisted terminal state (cancel/retry command) wins over a stale
  // progress sample still in flight.
  const status = (job.status && job.status !== 'running' ? job.status : event?.status || job.status || 'running') as DownloadJobStatus
  const meta = STATUS_META[status] || STATUS_META.running
  return { status, ...meta }
}

function counts(job: DownloadJob): JobCounts {
  const event = props.progress[job.id]
  const requested = event?.total || job.requestedCount || job.estimatedCount || 0
  const completed = Math.max(event?.completed ?? 0, job.completedCount || 0)
  const failed = Math.max(event?.failed ?? 0, job.failedCount || 0)
  const skipped = Math.max(event?.skipped ?? 0, job.skippedCount || 0)
  // Without a live event (for example, a job inherited after a restart) the
  // persisted counters still let the panel show the pending work as queued.
  const pending = Math.max(0, requested - completed - failed - skipped)
  const queued = event?.queued ?? (statusMeta(job).status === 'running' ? pending : 0)
  const downloading = event?.downloading ?? 0
  const finished = completed + failed + (event?.cancelled ?? 0) + skipped
  const percent = event
    ? Math.round(event.percent)
    : requested > 0
      ? Math.min(100, Math.round((finished / requested) * 100))
      : 0
  return { requested, queued, downloading, completed, failed, skipped, percent }
}

function jobTitle(job: DownloadJob): string {
  if (job.label) return job.label
  const source = job.sourceService ? ` from ${formatServiceName(job.sourceService)}` : ''
  return `${job.kind || 'download'} job${source}`
}

function accountLabel(job: DownloadJob): string {
  const id = job.preferredAccountId
  if (id != null && props.accountLabels[id]) return props.accountLabels[id]
  return 'Automatic'
}

function qualityLabel(quality: string | null | undefined): string {
  if (!quality) return 'Default'
  if (quality === 'hires') return 'Hi-Res lossless'
  if (quality === 'lossless') return 'Lossless'
  if (quality === 'high') return 'High (lossy)'
  if (quality === 'any') return 'Any'
  return quality
}

function serviceLabel(service: string | null | undefined): string {
  return service ? formatServiceName(service) : ''
}

function cancelTitle(job: DownloadJob): string {
  const jobCounts = counts(job)
  if (jobCounts.downloading > 0) {
    return 'Only queued rows are cancelled; the active download will finish'
  }
  if (jobCounts.queued === 0) return 'No queued tracks to cancel'
  return 'Cancel the queued tracks of this job'
}
</script>
