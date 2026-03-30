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
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { useMetadataSettings } from '@/composables/useMetadataSettings'
import BaseToggle from './BaseToggle.vue'
import SliderInput from './SliderInput.vue'

const metadataSettings = useMetadataSettings()

onMounted(async () => {
  try {
    await metadataSettings.loadSettings()
  } catch (err) {
    console.error('Failed to load metadata settings in isolated component:', err)
  }
})
</script>
