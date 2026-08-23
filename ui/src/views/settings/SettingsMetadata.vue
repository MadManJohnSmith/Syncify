<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
        Metadata Sources
      </h3>
      <BaseToggle 
        title="Enable MusicBrainz lookups" 
        subtitle="Use ISRC to fetch canonical metadata" 
        :checked="metadataSettings.settings.enable_musicbrainz"
        @click="metadataSettings.settings.enable_musicbrainz = !metadataSettings.settings.enable_musicbrainz; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Enable Last.fm tags" 
        subtitle="Fetch genres, moods, and community tags" 
        :checked="metadataSettings.settings.enable_lastfm"
        @click="metadataSettings.settings.enable_lastfm = !metadataSettings.settings.enable_lastfm; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Enable AcoustID fingerprinting" 
        subtitle="Identify tracks with missing or incorrect metadata" 
        :checked="metadataSettings.settings.enable_acoustid"
        @click="metadataSettings.settings.enable_acoustid = !metadataSettings.settings.enable_acoustid; metadataSettings.saveSettings()"
      />
    </section>
    
    <!-- Cat B: Tagging Behavior -->
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Tagging Behavior</h3>
      <BaseToggle 
        title="Overwrite existing tags on re-import" 
        :checked="metadataSettings.settings.overwrite_on_reimport"
        @click="metadataSettings.settings.overwrite_on_reimport = !metadataSettings.settings.overwrite_on_reimport; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Preserve custom tags" 
        subtitle="Keep user-edited tags when refreshing metadata" 
        :checked="metadataSettings.settings.preserve_custom_tags"
        @click="metadataSettings.settings.preserve_custom_tags = !metadataSettings.settings.preserve_custom_tags; metadataSettings.saveSettings()"
      />
      
      <div class="max-w-md">
        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Multi-value separator</label>
        <select 
          v-model="metadataSettings.settings.multi_value_separator"
          @change="metadataSettings.saveSettings()"
          class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none"
        >
          <option value=";">Semicolon (;)</option>
          <option value="/">Forward slash (/)</option>
          <option value=";;">Double semicolon (;;)</option>
          <option value="|">Pipe (|)</option>
        </select>
      </div>
    </section>

    <!-- Cat B: Symfonium-Specific Tags -->
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Symfonium-Specific Tags</h3>
      <BaseToggle 
        title="Write RELEASETYPE tag" 
        subtitle="Album / Single / EP / Compilation / Live" 
        :checked="metadataSettings.settings.write_releasetype"
        @click="metadataSettings.settings.write_releasetype = !metadataSettings.settings.write_releasetype; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write LABEL tag" 
        :checked="metadataSettings.settings.write_label"
        @click="metadataSettings.settings.write_label = !metadataSettings.settings.write_label; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write WORK and COMPOSER tags" 
        subtitle="For classical music organization" 
        :checked="metadataSettings.settings.write_work_composer"
        @click="metadataSettings.settings.write_work_composer = !metadataSettings.settings.write_work_composer; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write MusicBrainz IDs" 
        subtitle="MUSICBRAINZ_TRACKID, MUSICBRAINZ_RELEASEID, etc." 
        :checked="metadataSettings.settings.write_musicbrainz_ids"
        @click="metadataSettings.settings.write_musicbrainz_ids = !metadataSettings.settings.write_musicbrainz_ids; metadataSettings.saveSettings()"
      />
    </section>
    
    <!-- Cat B: Custom App Tags -->
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Custom App Tags</h3>
      <BaseToggle 
        title="Write DOWNLOAD_SOURCE tag" 
        subtitle="Which service the file was downloaded from" 
        :checked="metadataSettings.settings.write_download_source"
        @click="metadataSettings.settings.write_download_source = !metadataSettings.settings.write_download_source; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write DOWNLOAD_DATE tag" 
        :checked="metadataSettings.settings.write_download_date"
        @click="metadataSettings.settings.write_download_date = !metadataSettings.settings.write_download_date; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write ONLY_AVAILABLE_ON tag" 
        subtitle="Mark tracks exclusive to one service" 
        :checked="metadataSettings.settings.write_only_available_on"
        @click="metadataSettings.settings.write_only_available_on = !metadataSettings.settings.write_only_available_on; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write NOT_AVAILABLE_STREAMING tag" 
        subtitle="Mark tracks that are not available in streaming" 
        :checked="metadataSettings.settings.write_not_available_streaming"
        @click="metadataSettings.settings.write_not_available_streaming = !metadataSettings.settings.write_not_available_streaming; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write METADATA_QUALITY_SCORE tag" 
        subtitle="0-100 completeness score" 
        :checked="metadataSettings.settings.write_quality_score"
        @click="metadataSettings.settings.write_quality_score = !metadataSettings.settings.write_quality_score; metadataSettings.saveSettings()"
      />
      <BaseToggle 
        title="Write LYRICS_TYPE and LYRICS_SOURCE tags" 
        :checked="metadataSettings.settings.write_lyrics_tags"
        @click="metadataSettings.settings.write_lyrics_tags = !metadataSettings.settings.write_lyrics_tags; metadataSettings.saveSettings()"
      />
    </section>

    <!-- Metadata Quality Scoring (Unlocked Sprint 15) -->
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
        Metadata Quality Scoring
      </h3>
      
      <div v-if="metadataSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary py-4">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading scoring settings...</span>
      </div>
      <div v-else class="space-y-4">
        <SliderInput 
          label="Album Weight" 
          v-model="metadataSettings.settings.weight_album" 
          @update:modelValue="metadataSettings.saveSettings()"
          subtitle="Weight for Album identification" 
        />
        <SliderInput 
          label="ISRC Weight" 
          v-model="metadataSettings.settings.weight_isrc" 
          @update:modelValue="metadataSettings.saveSettings()"
          subtitle="Weight for ISRC field" 
        />
        <SliderInput 
          label="MusicBrainz ID Weight" 
          v-model="metadataSettings.settings.weight_mb_id" 
          @update:modelValue="metadataSettings.saveSettings()"
          subtitle="Weight for MusicBrainz recording ID" 
        />
        <SliderInput 
          label="Cover Art Weight" 
          v-model="metadataSettings.settings.weight_cover" 
          @update:modelValue="metadataSettings.saveSettings()"
          subtitle="Weight for album artwork availability" 
        />
        <SliderInput 
          label="Release Year Weight" 
          v-model="metadataSettings.settings.weight_year" 
          @update:modelValue="metadataSettings.saveSettings()"
          subtitle="Weight for release year field" 
        />
        <SliderInput 
          label="Genre Weight" 
          v-model="metadataSettings.settings.weight_genre" 
          @update:modelValue="metadataSettings.saveSettings()"
          subtitle="Weight for genre metadata" 
        />
      </div>
    </section>

    <!-- S144: Incremental Library Enrichment Section -->
    <section class="space-y-4 pt-4 border-t border-gray-200 dark:border-border-dark">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <span class="material-symbols-outlined text-primary text-[22px]">auto_fix_high</span>
            Enrich Existing Library
          </h3>
          <p class="text-xs text-text-secondary mt-0.5">
            Process existing tracks to resolve missing metadata without renames, data degradation, or audio downloads.
          </p>
        </div>
        <button
          v-if="enrichment.isRunning.value"
          @click="enrichment.cancelEnrichment"
          class="px-3 py-1.5 bg-red-500/10 hover:bg-red-500/20 text-red-500 rounded-lg text-xs font-medium transition-colors flex items-center gap-1.5"
        >
          <span class="material-symbols-outlined text-[16px]">cancel</span>
          Cancel Job
        </button>
      </div>

      <!-- Mode Selector -->
      <div class="flex items-center gap-2 p-1 bg-gray-100 dark:bg-surface-dark rounded-xl max-w-lg border border-gray-200 dark:border-border-dark">
        <button
          type="button"
          @click="changeEnrichmentMode('incomplete_only')"
          :class="[
            'flex-1 py-1.5 px-3 rounded-lg text-xs font-medium transition-all text-center',
            enrichment.selectedMode.value === 'incomplete_only'
              ? 'bg-primary text-white shadow-sm'
              : 'text-text-secondary hover:text-gray-900 dark:hover:text-white'
          ]"
        >
          Only Incomplete
        </button>
        <button
          type="button"
          @click="changeEnrichmentMode('revalidate_all')"
          :class="[
            'flex-1 py-1.5 px-3 rounded-lg text-xs font-medium transition-all text-center',
            enrichment.selectedMode.value === 'revalidate_all'
              ? 'bg-primary text-white shadow-sm'
              : 'text-text-secondary hover:text-gray-900 dark:hover:text-white'
          ]"
        >
          Revalidate All
        </button>
        <button
          type="button"
          @click="changeEnrichmentMode('selection')"
          :class="[
            'flex-1 py-1.5 px-3 rounded-lg text-xs font-medium transition-all text-center',
            enrichment.selectedMode.value === 'selection'
              ? 'bg-primary text-white shadow-sm'
              : 'text-text-secondary hover:text-gray-900 dark:hover:text-white'
          ]"
        >
          Current Selection
        </button>
      </div>

      <!-- Preview Breakdown Card -->
      <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
        <div class="p-3.5 rounded-xl bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark">
          <span class="text-[11px] font-medium text-text-secondary block">Eligible Tracks</span>
          <span class="text-xl font-bold text-primary mt-1 block">
            {{ enrichment.preview.value?.totalEligible ?? '—' }}
          </span>
        </div>
        <div class="p-3.5 rounded-xl bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark">
          <span class="text-[11px] font-medium text-text-secondary block">Complete (Skipped)</span>
          <span class="text-xl font-bold text-emerald-500 mt-1 block">
            {{ enrichment.preview.value?.totalComplete ?? '—' }}
          </span>
        </div>
        <div class="p-3.5 rounded-xl bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark">
          <span class="text-[11px] font-medium text-text-secondary block">Precedence Protected</span>
          <span class="text-xl font-bold text-amber-500 mt-1 block">
            {{ enrichment.preview.value?.totalSkippedPrecedence ?? '—' }}
          </span>
        </div>
        <div class="p-3.5 rounded-xl bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark">
          <span class="text-[11px] font-medium text-text-secondary block">Available Sources</span>
          <span class="text-xs font-medium text-text-primary mt-1 block truncate">
            {{ (enrichment.preview.value?.availableSources ?? ['MusicBrainz', 'Qobuz', 'Spotify']).join(', ') }}
          </span>
        </div>
      </div>

      <!-- Progress Section (when running) -->
      <div v-if="enrichment.isRunning.value" class="p-4 rounded-xl bg-primary/5 border border-primary/20 space-y-3">
        <div class="flex items-center justify-between text-xs font-medium text-gray-900 dark:text-white">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined animate-spin text-primary text-[18px]">progress_activity</span>
            <span>Phase: <strong class="text-primary">{{ enrichment.jobSummary.value?.currentPhase || 'Resolving' }}</strong></span>
          </div>
          <span class="text-primary font-bold">{{ enrichment.progressPercent.value }}%</span>
        </div>
        
        <!-- Progress Bar -->
        <div class="w-full h-2 bg-gray-200 dark:bg-border-dark rounded-full overflow-hidden">
          <div 
            class="h-full bg-primary transition-all duration-300 rounded-full"
            :style="{ width: `${enrichment.progressPercent.value}%` }"
          />
        </div>

        <div class="flex items-center justify-between text-[11px] text-text-secondary">
          <span class="truncate max-w-[280px]">
            {{ enrichment.jobSummary.value?.currentTrack || 'Processing tracks...' }}
          </span>
          <span>
            {{ enrichment.jobSummary.value?.processedTracks ?? 0 }} / {{ enrichment.jobSummary.value?.totalTracks ?? 0 }}
          </span>
        </div>
      </div>

      <!-- Start Button & Final Summary Card -->
      <div class="flex items-center justify-between pt-2">
        <button
          type="button"
          @click="enrichment.startEnrichment()"
          :disabled="enrichment.isRunning.value || enrichment.isPreviewLoading.value"
          class="px-4 py-2 bg-primary hover:bg-primary/90 text-white rounded-xl text-xs font-semibold shadow-sm transition-all flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <span :class="['material-symbols-outlined text-[16px]', enrichment.isRunning.value && 'animate-spin']">
            {{ enrichment.isRunning.value ? 'progress_activity' : 'play_arrow' }}
          </span>
          {{ enrichment.isRunning.value ? 'Enriching...' : 'Start Incremental Enrichment' }}
        </button>

        <span v-if="enrichment.jobSummary.value?.status === 'completed'" class="text-xs text-emerald-500 font-medium flex items-center gap-1">
          <span class="material-symbols-outlined text-[16px]">check_circle</span>
          Enriched {{ enrichment.jobSummary.value.modifiedTracks }} tracks ({{ enrichment.jobSummary.value.skippedCompleteTracks }} skipped)
        </span>
      </div>

      <!-- Detailed Report Breakdown Modal / Expandable -->
      <div 
        v-if="enrichment.jobSummary.value && enrichment.jobSummary.value.items.length > 0 && !enrichment.isRunning.value" 
        class="p-4 rounded-xl bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark space-y-3"
      >
        <div class="flex items-center justify-between">
          <h4 class="text-xs font-semibold text-gray-900 dark:text-white flex items-center gap-1.5">
            <span class="material-symbols-outlined text-[16px] text-primary">analytics</span>
            Last Execution Summary (Job {{ enrichment.jobSummary.value.jobId.slice(0, 8) }})
          </h4>
          <span class="text-[11px] text-text-secondary">
            Status: <strong class="text-emerald-500 uppercase">{{ enrichment.jobSummary.value.status }}</strong>
          </span>
        </div>

        <div class="max-h-48 overflow-y-auto space-y-1.5 pr-1 divide-y divide-gray-100 dark:divide-border-dark">
          <div 
            v-for="item in enrichment.jobSummary.value.items.slice(0, 20)" 
            :key="item.trackId"
            class="pt-1.5 flex items-center justify-between text-xs"
          >
            <div class="flex items-center gap-2 truncate max-w-[280px]">
              <span 
                :class="[
                  'w-2 h-2 rounded-full',
                  item.status === 'persisted' || item.status === 'partial' ? 'bg-emerald-500' :
                  item.status === 'skipped_complete' ? 'bg-blue-400' :
                  item.status === 'skipped_precedence' ? 'bg-amber-400' : 'bg-red-400'
                ]"
              />
              <span class="truncate text-gray-800 dark:text-gray-200">{{ item.artistName }} - {{ item.trackTitle }}</span>
            </div>
            <div class="flex items-center gap-2 text-[11px]">
              <span v-if="item.modifiedFields.length > 0" class="px-1.5 py-0.5 bg-emerald-500/10 text-emerald-500 rounded text-[10px]">
                +{{ item.modifiedFields.join(', ') }}
              </span>
              <span v-else class="text-text-secondary text-[10px]">
                {{ item.status }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- S158: Tidal Metadata & Path Repair Dry-Run Section -->
    <section class="space-y-4 pt-4 border-t border-gray-200 dark:border-border-dark">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <span>Tidal Metadata & Path Repair</span>
            <span class="px-2 py-0.5 text-[10px] font-bold rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20">Dry-Run</span>
          </h3>
          <p class="text-xs text-text-secondary">Inspect planned non-mutating fixes for incomplete Tidal downloads, tag corrections, and ghost track cleanups.</p>
        </div>
        <div class="flex items-center gap-2 shrink-0">
          <button
            @click="showRepairHistoryModal = true"
            class="px-3.5 py-2 bg-purple-500/10 hover:bg-purple-500/20 text-purple-600 dark:text-purple-400 text-xs font-semibold rounded-lg transition-colors flex items-center gap-1.5 border border-purple-500/20"
          >
            <span class="material-symbols-outlined text-[16px]">history_edu</span>
            Repair History
          </button>
          <button
            @click="showTidalRepairModal = true"
            class="px-4 py-2 bg-primary hover:bg-primary-hover text-white text-xs font-semibold rounded-lg transition-colors flex items-center gap-1.5 shadow-xs"
          >
            <span class="material-symbols-outlined text-[16px]">build_circle</span>
            Review Repair Plan
          </button>
        </div>
      </div>
    </section>

    <!-- S173: Local BPM & TEMPO Analysis -->
    <section class="space-y-4 pt-4 border-t border-gray-200 dark:border-border-dark">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <span>Local BPM & TEMPO Analysis</span>
            <span class="px-2 py-0.5 text-[10px] font-bold rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20">Symfonium Parity</span>
          </h3>
          <p class="text-xs text-text-secondary">Accurately extract BPM and harmonic tempo from local audio without redownloading or altering audio payload.</p>
        </div>
      </div>

      <div class="p-4 bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl space-y-4">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <BaseToggle 
            title="Only analyze missing BPM" 
            subtitle="Skip tracks that already have verified BPM" 
            :checked="tempo.onlyMissing.value"
            @click="tempo.onlyMissing.value = !tempo.onlyMissing.value"
          />
          <BaseToggle 
            title="Force re-analyze existing" 
            subtitle="Override existing non-manual BPM values" 
            :checked="tempo.forceReanalyze.value"
            @click="tempo.forceReanalyze.value = !tempo.forceReanalyze.value"
          />
        </div>

        <SliderInput 
          label="Confidence Threshold (%)" 
          v-model="tempo.confidenceThreshold.value" 
          :min="10"
          :max="90"
          subtitle="Reject ambiguous rhythm or noise below this confidence (recommended: 40%)" 
        />

        <!-- Progress Box -->
        <div v-if="tempo.isAnalyzing.value && tempo.currentProgress.value" class="space-y-2 p-3 bg-white dark:bg-card-dark rounded-lg border border-gray-200 dark:border-border-dark">
          <div class="flex justify-between text-xs text-gray-700 dark:text-gray-300">
            <span class="font-medium truncate max-w-xs">Analyzing: {{ tempo.currentProgress.value.track_title }}</span>
            <span>{{ tempo.currentProgress.value.current_index }} / {{ tempo.currentProgress.value.total }}</span>
          </div>
          <div class="w-full bg-gray-200 dark:bg-gray-700 h-2 rounded-full overflow-hidden">
            <div 
              class="bg-emerald-500 h-full transition-all duration-300 rounded-full"
              :style="{ width: `${(tempo.currentProgress.value.current_index / tempo.currentProgress.value.total) * 100}%` }"
            ></div>
          </div>
        </div>

        <!-- Summary Banner -->
        <div v-if="tempo.lastSummary.value" class="p-3 bg-emerald-50 dark:bg-emerald-950/30 border border-emerald-200 dark:border-emerald-800 rounded-lg text-xs text-emerald-800 dark:text-emerald-300 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px]">check_circle</span>
            <span>Analysis complete: <strong>{{ tempo.lastSummary.value.analyzed }}</strong> tagged, <strong>{{ tempo.lastSummary.value.low_confidence }}</strong> low confidence, <strong>{{ tempo.lastSummary.value.skipped }}</strong> skipped.</span>
          </div>
        </div>

        <!-- Error Banner -->
        <div v-if="tempo.errorMessage.value" class="p-3 bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 rounded-lg text-xs text-red-800 dark:text-red-300 flex items-center justify-between">
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px]">error</span>
            <span>{{ tempo.errorMessage.value }}</span>
          </div>
        </div>

        <!-- Actions -->
        <div class="flex items-center justify-end gap-3 pt-2">
          <button
            v-if="tempo.isAnalyzing.value"
            @click="tempo.cancel()"
            class="px-4 py-2 bg-red-500/10 hover:bg-red-500/20 text-red-600 dark:text-red-400 text-xs font-semibold rounded-lg transition-colors flex items-center gap-1.5 border border-red-500/20"
          >
            <span class="material-symbols-outlined text-[16px]">cancel</span>
            Cancel
          </button>
          <button
            v-else
            @click="tempo.startAnalysis()"
            class="px-4 py-2 bg-primary hover:bg-primary-hover text-white text-xs font-semibold rounded-lg transition-colors flex items-center gap-1.5 shadow-xs"
          >
            <span class="material-symbols-outlined text-[16px]">graphic_eq</span>
            Analyze Library BPM
          </button>
        </div>
      </div>
    </section>

    <!-- S158: Tidal Repair Review Modal -->
    <TidalRepairReviewModal v-model="showTidalRepairModal" />

    <!-- S163: Applied Repairs History Modal -->
    <RepairHistoryModal v-model="showRepairHistoryModal" />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useMetadataSettings } from '@/composables/useMetadataSettings'
import { useIncrementalEnrichment } from '@/composables/useIncrementalEnrichment'
import { useTempoAnalysis } from '@/composables/useTempoAnalysis'
import type { EnrichmentMode } from '@/api/types'
import BaseToggle from './BaseToggle.vue'
import SliderInput from './SliderInput.vue'
import TidalRepairReviewModal from '@/components/TidalRepairReviewModal.vue'
import RepairHistoryModal from '@/components/RepairHistoryModal.vue'

const showTidalRepairModal = ref(false)
const showRepairHistoryModal = ref(false)
const metadataSettings = useMetadataSettings()
const enrichment = useIncrementalEnrichment()
const tempo = useTempoAnalysis()

async function changeEnrichmentMode(mode: EnrichmentMode) {
  enrichment.selectedMode.value = mode
  await enrichment.fetchPreview(mode)
}

onMounted(async () => {
  try {
    await metadataSettings.loadSettings()
  } catch (err) {
    console.error('Failed to load metadata settings in isolated component:', err)
  }
})
</script>


