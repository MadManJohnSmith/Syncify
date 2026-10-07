<template>
  <div v-if="modelValue" class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-xs animate-in fade-in duration-200">
    <div class="bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-2xl w-full max-w-4xl max-h-[90vh] flex flex-col shadow-2xl overflow-hidden">

      <!-- Modal Header -->
      <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0 bg-gray-50/50 dark:bg-surface-highlight/30">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
            <span class="material-symbols-outlined text-[24px]">drive_file_rename_outline</span>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-lg font-bold text-gray-900 dark:text-white">Track Version Disambiguation Repair Review</h2>
              <span class="px-2.5 py-0.5 text-xs font-semibold rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20">
                Dry-Run Plan
              </span>
            </div>
            <p class="text-xs text-text-secondary">Inspect planned renames for ambiguous album tracks. Applying renames audio + LRC sidecars atomically.</p>
          </div>
        </div>

        <button
          @click="closeModal"
          class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-white rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors"
          title="Close repair review"
        >
          <span class="material-symbols-outlined text-[20px]">close</span>
        </button>
      </div>

      <!-- Safety Banner -->
      <div class="px-6 py-3 bg-amber-500/10 dark:bg-amber-500/15 border-b border-amber-500/20 flex items-center justify-between shrink-0">
        <div class="flex items-center gap-2.5 text-amber-800 dark:text-amber-300 text-xs font-medium">
          <span class="material-symbols-outlined text-[20px] text-amber-600 dark:text-amber-400 shrink-0">shield</span>
          <span>Nothing changes until you confirm. Each rename is verified with SHA-256 and rolled back if any step fails.</span>
        </div>
        <div class="text-[11px] text-amber-700/80 dark:text-amber-400/80 font-mono">
          Safe Inspection
        </div>
      </div>

      <!-- Modal Body -->
      <div class="p-6 flex-1 overflow-y-auto custom-scrollbar space-y-6">

        <!-- Loading State -->
        <div v-if="isLoading" class="py-16 flex flex-col items-center justify-center text-center space-y-3">
          <span class="material-symbols-outlined text-4xl text-primary animate-spin">progress_activity</span>
          <p class="text-sm font-medium text-gray-900 dark:text-white">Computing disambiguation repair plan...</p>
          <p class="text-xs text-text-secondary">Hashing candidate files and correlating duplicate album tracks</p>
        </div>

        <!-- Error State -->
        <div v-else-if="errorMessage" class="p-6 rounded-xl bg-red-500/10 border border-red-500/30 text-center space-y-3">
          <span class="material-symbols-outlined text-[28px]">error</span>
          <h4 class="text-sm font-bold text-red-600 dark:text-red-400">Failed to compute disambiguation repair plan</h4>
          <p class="text-xs text-red-700 dark:text-red-300 font-mono break-all max-w-xl mx-auto">{{ errorMessage }}</p>
        </div>

        <!-- Empty State -->
        <div v-else-if="!items.length" class="py-16 flex flex-col items-center justify-center text-center space-y-3">
          <span class="material-symbols-outlined text-4xl text-green-500">task_alt</span>
          <h4 class="text-sm font-bold text-gray-900 dark:text-white">No ambiguous tracks detected</h4>
          <p class="text-xs text-text-secondary">Every duplicate title in your library already has a unique, version-aware filename.</p>
        </div>

        <!-- Plan Summary -->
        <div v-else class="space-y-4">
          <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div class="p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60">
              <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Candidates</span>
              <p class="text-xl font-bold text-gray-900 dark:text-white">{{ plan?.total_candidates ?? 0 }}</p>
            </div>
            <div class="p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60">
              <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Renamed</span>
              <p class="text-xl font-bold text-green-600 dark:text-green-400">{{ plan?.total_renamed ?? 0 }}</p>
            </div>
            <div class="p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60">
              <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Skipped</span>
              <p class="text-xl font-bold text-amber-600 dark:text-amber-400">{{ plan?.total_skipped ?? 0 }}</p>
            </div>
            <div class="p-3 rounded-xl bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60">
              <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Ready</span>
              <p class="text-xl font-bold text-primary">{{ readyCount }}</p>
            </div>
          </div>

          <!-- Report-level errors -->
          <div v-if="plan && plan.errors.length > 0" class="p-4 rounded-xl bg-red-500/10 border border-red-500/30 space-y-2">
            <h4 class="text-xs font-bold text-red-600 dark:text-red-400">Skipped candidates</h4>
            <p v-for="(err, idx) in plan.errors" :key="idx" class="text-xs text-red-700 dark:text-red-300 font-mono break-all">{{ err }}</p>
          </div>

          <div
            v-for="item in items"
            :key="`${item.track_id}-${item.target_audio_path}`"
            class="p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark space-y-3"
          >
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0 space-y-1">
                <p class="text-sm font-semibold text-gray-900 dark:text-white truncate" :title="item.display_title">
                  {{ item.display_title || '(untitled)' }}
                </p>
                <p class="text-[11px] text-text-secondary">
                  Track #{{ item.track_id }}
                  <span v-if="item.isrc"> · ISRC {{ item.isrc }}</span>
                  <span v-if="item.file_disambiguator"> · {{ item.file_disambiguator }}</span>
                </p>
              </div>
              <span
                class="px-2.5 py-0.5 text-[10px] font-semibold rounded-full border shrink-0"
                :class="statusBadgeClass(item.status)"
              >
                {{ statusLabel(item.status) }}
              </span>
            </div>

            <!-- Audio rename -->
            <div class="grid md:grid-cols-2 gap-2.5">
              <div class="p-2.5 rounded-lg bg-white dark:bg-surface-dark border border-gray-200/60 dark:border-border-dark/60 space-y-1">
                <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Current audio</span>
                <p class="text-xs text-gray-800 dark:text-gray-200 font-mono break-all">{{ item.current_audio_path }}</p>
              </div>
              <div class="p-2.5 rounded-lg bg-primary/5 border border-primary/20 space-y-1">
                <span class="font-bold text-primary uppercase tracking-wider text-[10px]">Target audio</span>
                <p class="text-xs text-gray-900 dark:text-white font-mono break-all">{{ item.target_audio_path }}</p>
              </div>
            </div>

            <!-- LRC sidecar rename -->
            <div class="grid md:grid-cols-2 gap-2.5">
              <div class="p-2.5 rounded-lg bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60 space-y-1">
                <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">Current LRC</span>
                <p class="text-xs text-gray-800 dark:text-gray-200 font-mono break-all">{{ item.current_lrc_path || '—' }}</p>
              </div>
              <div class="p-2.5 rounded-lg bg-primary/5 border border-primary/20 space-y-1">
                <span class="font-bold text-primary uppercase tracking-wider text-[10px]">Target LRC</span>
                <p class="text-xs text-gray-900 dark:text-white font-mono break-all">{{ item.target_lrc_path || '—' }}</p>
              </div>
            </div>

            <!-- Integrity hashes -->
            <div class="p-2.5 rounded-lg bg-gray-50 dark:bg-surface-highlight border border-gray-200/60 dark:border-border-dark/60 space-y-1">
              <span class="font-bold text-text-secondary uppercase tracking-wider text-[10px]">SHA-256 before</span>
              <p class="text-xs text-gray-800 dark:text-gray-200 font-mono break-all">{{ item.sha256_before || '—' }}</p>
              <p v-if="item.output_hashes?.file_hash_after" class="text-xs text-gray-800 dark:text-gray-200 font-mono break-all">
                SHA-256 after: {{ item.output_hashes.file_hash_after }}
              </p>
            </div>

            <div v-if="item.applied_actions.length > 0" class="p-2.5 rounded-lg bg-green-500/5 border border-green-500/20 space-y-1">
              <span class="font-bold text-green-700 dark:text-green-400 uppercase tracking-wider text-[10px]">Applied actions</span>
              <p v-for="(action, idx) in item.applied_actions" :key="idx" class="text-xs text-green-700 dark:text-green-300 font-mono break-all">
                {{ action }}
              </p>
            </div>

            <div v-if="item.rollback_state" class="p-2.5 rounded-lg bg-red-500/5 border border-red-500/20 space-y-1">
              <span class="font-bold text-red-600 dark:text-red-400 uppercase tracking-wider text-[10px]">Rollback</span>
              <p class="text-xs text-red-700 dark:text-red-300 font-mono break-all">{{ item.rollback_state }}</p>
            </div>
          </div>
        </div>
      </div>

      <!-- Modal Footer (Actions) -->
      <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0 bg-gray-50/50 dark:bg-surface-highlight/30">
        <div class="text-xs text-text-secondary">
          <span v-if="readyCount > 0">{{ readyCount }} rename{{ readyCount > 1 ? 's' : '' }} ready to apply</span>
          <span v-else-if="items.length > 0">No pending renames for this plan</span>
        </div>

        <div class="flex items-center gap-3">
          <button
            @click="fetchPlan"
            :disabled="isLoading"
            class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-surface-highlight/80 text-gray-800 dark:text-gray-200 text-xs font-medium rounded-lg transition-colors flex items-center gap-1.5 disabled:opacity-50"
            title="Recalculate disambiguation repair plan"
          >
            <span class="material-symbols-outlined text-[16px]" :class="{ 'animate-spin': isLoading }">sync</span>
            Re-run plan
          </button>

          <button
            @click="applyRepair"
            :disabled="isLoading || isExecuting || readyCount === 0"
            class="px-4 py-2 bg-primary hover:bg-primary-hover text-on-accent text-xs font-medium rounded-lg transition-colors flex items-center gap-1.5 disabled:opacity-50 shadow-sm shadow-primary/20"
            title="Apply the planned renames with SHA-256 verification and rollback protection"
          >
            <span class="material-symbols-outlined text-[16px]" :class="{ 'animate-spin': isExecuting }">
              {{ isExecuting ? 'progress_activity' : 'build_circle' }}
            </span>
            {{ isExecuting ? 'Applying repairs...' : 'Apply repair' }}
          </button>
        </div>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue';
// R13: confirm unificado (antes plugin-dialog nativo, fuera de la ventana).
import { confirm } from '@/composables/useToast';
import { executeDisambiguationRepair, planDisambiguationRepair } from '@/api/metadata';
import { useToast } from '@/composables/useToast';
import type { DisambiguationRepairReport } from '@/api/types';

const props = defineProps<{
  modelValue: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void;
}>();

const toast = useToast();

const plan = ref<DisambiguationRepairReport | null>(null);
const isLoading = ref(false);
const isExecuting = ref(false);
const errorMessage = ref<string | null>(null);

const items = computed(() => plan.value?.items ?? []);

// Statuses reported by the backend service:
// plan -> "ready" | "already_disambiguated"; execute -> "repaired_success" |
// "file_not_found" | "repair_input_changed"
const readyCount = computed(() => items.value.filter((item) => item.status === 'ready').length);

function closeModal() {
  emit('update:modelValue', false);
}

function statusLabel(status: string): string {
  switch (status) {
    case 'ready':
      return 'Ready';
    case 'already_disambiguated':
      return 'Already Disambiguated';
    case 'repaired_success':
      return 'Repaired';
    case 'repair_input_changed':
      return 'Input Changed';
    case 'file_not_found':
      return 'File Not Found';
    default:
      return status;
  }
}

function statusBadgeClass(status: string): string {
  switch (status) {
    case 'repaired_success':
      return 'bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20';
    case 'already_disambiguated':
      return 'bg-gray-500/10 text-gray-600 dark:text-gray-300 border-gray-500/20';
    case 'repair_input_changed':
      return 'bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20';
    case 'file_not_found':
      return 'bg-red-500/10 text-red-600 dark:text-red-400 border-red-500/20';
    default:
      return 'bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/20';
  }
}

async function fetchPlan() {
  isLoading.value = true;
  errorMessage.value = null;
  try {
    plan.value = await planDisambiguationRepair();
  } catch (err) {
    plan.value = null;
    errorMessage.value = typeof err === 'string' ? err : String(err);
  } finally {
    isLoading.value = false;
  }
}

async function applyRepair() {
  const currentPlan = plan.value;
  if (!currentPlan || currentPlan.items.length === 0) {
    toast.error('Nothing to apply', 'Re-run the plan to refresh candidates');
    return;
  }

  const confirmed = await confirm(
    `Rename ${readyCount.value} track${readyCount.value > 1 ? 's' : ''} and update the library database? Files changed outside this plan are rolled back automatically.`,
    { title: 'Apply Disambiguation Repair', variant: 'warning' }
  );
  if (confirmed !== true) return;

  isExecuting.value = true;
  try {
    const report = await executeDisambiguationRepair(currentPlan, true);
    plan.value = report;
    if (report.errors.length > 0) {
      toast.warning(
        `Repaired ${report.total_renamed} of ${report.total_candidates} tracks`,
        `${report.total_skipped} skipped · ${report.errors[0]}`
      );
    } else {
      toast.success(
        `Repaired ${report.total_renamed} of ${report.total_candidates} tracks`,
        `${report.total_skipped} skipped`
      );
    }
    await fetchPlan();
  } catch (err) {
    toast.error('Disambiguation repair failed', typeof err === 'string' ? err : String(err));
  } finally {
    isExecuting.value = false;
  }
}

watch(
  () => props.modelValue,
  (newVal) => {
    if (newVal) {
      plan.value = null;
      fetchPlan();
    }
  },
  { immediate: true }
);
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(156, 163, 175, 0.4);
  border-radius: 3px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(156, 163, 175, 0.6);
}
</style>
