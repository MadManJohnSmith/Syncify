<template>
  <Teleport to="body">
    <Transition name="fade">
      <div
        v-if="modelValue"
        ref="dialogEl"
        class="fixed inset-0 z-[210] flex items-center justify-center p-4 sm:p-8"
        role="dialog"
        aria-modal="true"
        aria-labelledby="user-reports-title"
        data-testid="user-reports-modal"
        @keydown.escape.stop.prevent="requestClose"
        @keydown.tab="keepFocusInside"
      >
        <!-- Backdrop -->
        <div class="absolute inset-0 bg-black/60 backdrop-blur-xs" @click="requestClose"></div>

        <!-- Modal card -->
        <div class="relative w-full max-w-lg max-h-[90vh] flex flex-col rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl overflow-hidden">
          <!-- Header -->
          <div class="px-6 py-4 border-b border-gray-100 dark:border-border-dark flex items-start justify-between gap-3 shrink-0">
            <div class="flex items-start gap-3 min-w-0">
              <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center shrink-0">
                <span class="material-symbols-outlined text-[24px]">bug_report</span>
              </div>
              <div class="min-w-0">
                <h2 id="user-reports-title" class="text-lg font-bold text-gray-900 dark:text-white">
                  Report a bug or send feedback
                </h2>
                <p class="text-xs text-text-secondary mt-0.5">
                  Saved as files on this computer. Syncify does not send them anywhere.
                </p>
              </div>
            </div>
            <button
              @click="requestClose"
              :disabled="submitting"
              aria-label="Close"
              class="p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors disabled:opacity-50"
            >
              <span class="material-symbols-outlined text-[20px]">close</span>
            </button>
          </div>

          <!-- Mode switch -->
          <div
            class="px-6 pt-4 shrink-0"
            role="tablist"
            aria-label="Report type"
          >
            <div class="inline-flex p-1 rounded-xl bg-gray-100 dark:bg-surface-highlight gap-1">
              <button
                role="tab"
                type="button"
                data-testid="user-reports-tab-bug"
                :aria-selected="mode === 'bug'"
                :class="mode === 'bug' ? 'bg-white dark:bg-surface-dark text-gray-900 dark:text-white shadow-sm' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'"
                class="px-4 py-2 rounded-lg text-sm font-medium transition-colors flex items-center gap-2"
                @click="switchMode('bug')"
              >
                <span class="material-symbols-outlined text-[18px]">bug_report</span>
                Bug report
              </button>
              <button
                role="tab"
                type="button"
                data-testid="user-reports-tab-feedback"
                :aria-selected="mode === 'feedback'"
                :class="mode === 'feedback' ? 'bg-white dark:bg-surface-dark text-gray-900 dark:text-white shadow-sm' : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'"
                class="px-4 py-2 rounded-lg text-sm font-medium transition-colors flex items-center gap-2"
                @click="switchMode('feedback')"
              >
                <span class="material-symbols-outlined text-[18px]">chat</span>
                Feedback
              </button>
            </div>
          </div>

          <div class="overflow-y-auto custom-scrollbar px-6 py-5 flex flex-col gap-4">
            <!-- Saved confirmation -->
            <div v-if="savedPath" data-testid="user-reports-saved" class="flex items-start gap-3 p-4 rounded-xl bg-ok/10 border border-ok/30">
              <span class="material-symbols-outlined text-ok">check_circle</span>
              <div class="min-w-0">
                <p class="text-sm font-medium text-gray-900 dark:text-white">{{ savedTitle }}</p>
                <p class="text-xs text-text-secondary mt-1">
                  Written to your Syncify reports folder:
                </p>
                <p class="text-xs text-gray-700 dark:text-gray-300 mt-1 break-all font-mono" data-testid="user-reports-saved-path">
                  {{ savedPath }}
                </p>
              </div>
            </div>

            <template v-else>
              <!-- Bug report form -->
              <template v-if="mode === 'bug'">
                <div class="flex flex-col gap-1.5">
                  <label for="user-reports-bug-message" class="text-sm font-medium text-gray-700 dark:text-gray-300">
                    What went wrong?
                  </label>
                  <textarea
                    id="user-reports-bug-message"
                    v-model="bugMessage"
                    data-testid="user-reports-bug-message"
                    rows="4"
                    :disabled="submitting"
                    placeholder="Describe the bug you found..."
                    class="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-white/5 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/40 disabled:opacity-50"
                  ></textarea>
                </div>

                <div class="flex flex-col gap-1.5">
                  <label for="user-reports-bug-steps" class="text-sm font-medium text-gray-700 dark:text-gray-300">
                    Steps to reproduce <span class="text-gray-400 font-normal">(optional)</span>
                  </label>
                  <textarea
                    id="user-reports-bug-steps"
                    v-model="bugSteps"
                    data-testid="user-reports-bug-steps"
                    rows="3"
                    :disabled="submitting"
                    placeholder="1. Open the library&#10;2. Click sync"
                    class="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-white/5 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/40 disabled:opacity-50"
                  ></textarea>
                </div>

                <label
                  for="user-reports-attach-logs"
                  class="flex items-start gap-3 p-3 rounded-xl bg-gray-50 dark:bg-white/5 border border-gray-200 dark:border-border-dark cursor-pointer"
                >
                  <input
                    id="user-reports-attach-logs"
                    v-model="attachLogs"
                    data-testid="user-reports-attach-logs"
                    type="checkbox"
                    :disabled="submitting"
                    class="mt-0.5 w-4 h-4 rounded border-gray-300 dark:border-border-dark text-primary focus:ring-primary/40 disabled:opacity-50"
                  />
                  <span class="min-w-0">
                    <span class="block text-sm font-medium text-gray-900 dark:text-white">Attach the system log</span>
                    <span class="block text-xs text-text-secondary mt-0.5">
                      The sanitized in-memory log is written inside the report file on this computer. It is
                      never uploaded, but it can contain file names of your library, so uncheck it if you
                      prefer not to share them.
                    </span>
                  </span>
                </label>
              </template>

              <!-- Feedback form -->
              <template v-else>
                <div class="flex flex-col gap-1.5">
                  <label for="user-reports-feedback-type" class="text-sm font-medium text-gray-700 dark:text-gray-300">
                    Type
                  </label>
                  <select
                    id="user-reports-feedback-type"
                    v-model="feedbackType"
                    data-testid="user-reports-feedback-type"
                    :disabled="submitting"
                    class="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-white/5 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/40 disabled:opacity-50"
                  >
                    <option value="General Feedback">General feedback</option>
                    <option value="Feature Request">Feature request</option>
                    <option value="Bug Report">Bug report (no details)</option>
                  </select>
                </div>

                <div class="flex flex-col gap-1.5">
                  <label for="user-reports-feedback-message" class="text-sm font-medium text-gray-700 dark:text-gray-300">
                    Your feedback
                  </label>
                  <textarea
                    id="user-reports-feedback-message"
                    v-model="feedbackMessage"
                    data-testid="user-reports-feedback-message"
                    rows="4"
                    :disabled="submitting"
                    placeholder="What should Syncify do better?"
                    class="w-full px-3 py-2 text-sm rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-white/5 text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/40 disabled:opacity-50"
                  ></textarea>
                </div>
              </template>

              <!-- Destination note -->
              <p data-testid="user-reports-destination" class="text-xs text-text-secondary">
                Reports are saved locally as JSON files in your Syncify reports folder, next to the app logs.
                Nothing is sent to a server.
              </p>

              <!-- Validation / submit error -->
              <p v-if="error" data-testid="user-reports-error" role="alert" class="text-sm text-error flex items-start gap-2">
                <span class="material-symbols-outlined text-[16px] mt-0.5 shrink-0">error</span>
                <span>{{ error }}</span>
              </p>
            </template>
          </div>

          <!-- Footer -->
          <div class="px-6 py-4 border-t border-gray-100 dark:border-border-dark flex justify-end gap-3 shrink-0">
            <template v-if="savedPath">
              <button
                type="button"
                data-testid="user-reports-another"
                class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors"
                @click="resetForm"
              >
                Write another
              </button>
              <button
                type="button"
                data-testid="user-reports-done"
                class="px-4 py-2 text-sm font-medium bg-primary hover:bg-primary-hover text-on-accent rounded-lg transition-colors"
                @click="close"
              >
                Done
              </button>
            </template>
            <template v-else>
              <button
                type="button"
                data-testid="user-reports-cancel"
                :disabled="submitting"
                class="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors disabled:opacity-50"
                @click="requestClose"
              >
                Cancel
              </button>
              <button
                type="button"
                data-testid="user-reports-submit"
                :disabled="submitting"
                :aria-busy="submitting"
                class="px-4 py-2 text-sm font-medium bg-primary hover:bg-primary-hover text-on-accent rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                @click="submit"
              >
                <span v-if="submitting" class="material-symbols-outlined text-[16px] animate-spin">sync</span>
                {{ submitting ? 'Saving...' : submitLabel }}
              </button>
            </template>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { toolsApi, type UserReportInput } from '@/api/tools'
import { useToast } from '@/composables/useToast'

const props = defineProps<{
  /** Whether the modal is visible. */
  modelValue: boolean
  /** Which form opens first: a bug report (default) or the feedback form. */
  initialMode?: 'bug' | 'feedback'
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'saved', filePath: string): void
}>()

const toast = useToast()

const dialogEl = ref<HTMLElement | null>(null)
let previouslyFocused: HTMLElement | null = null

const mode = ref<'bug' | 'feedback'>(props.initialMode === 'feedback' ? 'feedback' : 'bug')

const bugMessage = ref('')
const bugSteps = ref('')
// Opt-in that stays true by default, matching the behaviour the help panel had
// before it was removed: the user sees the checkbox and can uncheck it.
const attachLogs = ref(true)

const feedbackType = ref('General Feedback')
const feedbackMessage = ref('')

const submitting = ref(false)
const error = ref('')
const savedPath = ref('')
const savedKind = ref<'bug_report' | 'feedback'>('bug_report')

const submitLabel = computed(() => (mode.value === 'bug' ? 'Submit report' : 'Send feedback'))
const savedTitle = computed(() => (savedKind.value === 'bug_report' ? 'Bug report saved' : 'Feedback saved'))

/** Reset every field so each submission starts from a clean form. */
function resetForm() {
  bugMessage.value = ''
  bugSteps.value = ''
  attachLogs.value = true
  feedbackType.value = 'General Feedback'
  feedbackMessage.value = ''
  error.value = ''
  savedPath.value = ''
  submitting.value = false
}

function switchMode(next: 'bug' | 'feedback') {
  if (mode.value === next) return
  mode.value = next
  // A confirmation belongs to the submission that produced it; switching forms
  // would otherwise leave a stale "saved" banner with the wrong wording.
  savedPath.value = ''
  error.value = ''
}

watch(() => props.modelValue, async (open) => {
  if (open) {
    mode.value = props.initialMode === 'feedback' ? 'feedback' : 'bug'
    resetForm()
    previouslyFocused = document.activeElement as HTMLElement | null
    await nextTick()
    // Teleport + Transition can delay the real DOM insertion until the
    // following microtask; focusing the selected tab also gives the dialog a
    // useful keyboard starting point.
    await nextTick()
    dialogEl.value?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"]')?.focus()
  } else if (previouslyFocused) {
    previouslyFocused.focus()
    previouslyFocused = null
  }
}, { immediate: true })

const focusableSelector = 'button:not(:disabled), textarea:not(:disabled), select:not(:disabled), input:not(:disabled)'

function keepFocusInside(event: KeyboardEvent) {
  const dialog = dialogEl.value
  if (!dialog) return
  const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(focusableSelector))
  if (!focusable.length) return
  const first = focusable[0]
  const last = focusable[focusable.length - 1]
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

/** Surface the backend error message instead of a generic failure. */
function describeError(e: unknown): string {
  const message = e instanceof Error ? e.message : typeof e === 'string' ? e : ''
  return message || 'Could not save the report. Check the app log directory permissions and try again.'
}

async function submit() {
  if (submitting.value) return
  error.value = ''

  const report: UserReportInput =
    mode.value === 'bug'
      ? {
          kind: 'bug_report',
          message: bugMessage.value.trim(),
          steps_to_reproduce: bugSteps.value.trim(),
          attach_logs: attachLogs.value,
        }
      : {
          kind: 'feedback',
          message: feedbackMessage.value.trim(),
          feedback_type: feedbackType.value,
        }

  // The backend rejects empty free text; validating here avoids a useless write
  // and gives the message inline instead of as a failed command.
  if (!report.message) {
    error.value =
      mode.value === 'bug'
        ? 'Describe the bug before saving the report.'
        : 'Write your feedback before sending it.'
    return
  }

  submitting.value = true
  try {
    const saved = await toolsApi.saveUserReport(report)
    // The path is what proves to the user where the report ended up; the
    // backend owns it (timestamped JSON under the reports folder).
    savedPath.value = saved.filePath
    savedKind.value = mode.value === 'bug' ? 'bug_report' : 'feedback'
    emit('saved', saved.filePath)
    toast.success(mode.value === 'bug' ? 'Bug report saved locally' : 'Feedback saved locally')
  } catch (e) {
    error.value = describeError(e)
  } finally {
    submitting.value = false
  }
}

function close() {
  emit('update:modelValue', false)
}

/** Closing is blocked while a write is in flight so a report is never lost. */
function requestClose() {
  if (submitting.value) return
  close()
}
</script>
