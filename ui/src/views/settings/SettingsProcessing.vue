<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span>Audio Processing & Formatting</span>
        </h3>
      </div>

      <div v-if="downloadSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary py-4">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading audio settings...</span>
      </div>
      <div v-else class="space-y-6">
        <!-- #197 (SB02): la transcodificación se oculta — la feature no
             existe aún (el worker no consume sus ajustes); reapertura en la
             matriz. -->

        <!-- Embed Settings -->
        <div class="p-4 bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark space-y-4">
          <div class="flex items-center justify-between pb-2 border-b border-gray-100 dark:border-border-dark">
            <h4 class="text-sm font-semibold text-gray-900 dark:text-white">Embedding & Tags</h4>
            <span class="text-[11px] text-text-secondary">Physical metadata inside the audio</span>
          </div>

          <BaseToggle
            title="Embed lyrics in audio files"
            subtitle="Embed synced and unsynced lyrics (LYRICS / UNSYNCEDLYRICS)"
            tooltip="Embeds lyrics inside the audio container. Direct FLAC downloads already include available lyrics automatically; this setting controls embedding in conversions and batch jobs."
            :checked="audioProcessing.embed_lyrics"
            @click="toggleEmbedLyrics"
          />

          <BaseToggle
            title="Embed album artwork"
            subtitle="Embed cover art in the image blocks of downloaded files"
            tooltip="Embeds the cover image in METADATA_BLOCK_PICTURE. Following the Symfonium standard, optimized WebP/JPEG images are preserved up to a safe limit of 800 KB."
            :checked="audioProcessing.embed_artwork"
            @click="toggleEmbedArtwork"
          />

          <!-- #197 (SB02): el tamaño máximo de portada se oculta — el worker
               fija el límite de tamaño (800 KB) y no consume el ajuste. -->
        </div>

        <!-- ReplayGain Settings -->
        <div class="p-4 bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark space-y-4">
          <div class="flex items-center justify-between pb-2 border-b border-gray-100 dark:border-border-dark">
            <div>
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white flex items-center gap-2">
                <span>ReplayGain & Normalization</span>
                <span class="px-2 py-0.5 text-[10px] font-bold rounded-full bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20">
                  Post-Processing
                </span>
              </h4>
              <p class="text-xs text-text-secondary mt-0.5">Calculates gain metadata to avoid abrupt volume jumps between songs without altering audio quality.</p>
            </div>
          </div>

          <div>
            <div class="flex items-center gap-1.5 mb-1">
              <label class="block text-xs font-medium text-gray-600 dark:text-gray-400">ReplayGain mode</label>
              <span
                class="relative group/rg-tip inline-flex items-center"
                title="Calculation mode for the ReplayGain tags."
              >
                <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/rg-tip:block z-40 w-56 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                  Off: no normalization. Track: normalizes each track individually. Album: preserves the original dynamics of the full album. Both: writes both tags.
                </span>
              </span>
            </div>
            <select
              :value="audioProcessing.replay_gain_mode"
              @change="setReplayGainMode(getEventValue($event))"
              class="w-full px-3 py-1.5 bg-gray-50 dark:bg-surface-highlight border border-gray-300 dark:border-gray-600 rounded-lg text-xs text-gray-900 dark:text-white"
            >
              <option value="off">Off (disabled)</option>
              <option value="track">Track gain</option>
              <option value="album">Album gain</option>
              <option value="both">Both (Track + Album)</option>
            </select>
          </div>

          <div v-if="audioProcessing.replay_gain_mode !== 'off'" class="pt-3 border-t border-gray-100 dark:border-gray-700">
            <div class="flex justify-between items-center text-xs mb-1">
              <div class="flex items-center gap-1.5">
                <label class="font-medium text-gray-700 dark:text-gray-300">Target loudness</label>
                <span
                  class="relative group/lufs-tip inline-flex items-center"
                  title="Target integrated loudness level per the EBU R128 standard."
                >
                  <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                  <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/lufs-tip:block z-40 w-56 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                    The EBU R128 standard value is -18 or -14 LUFS. Values closer to 0 are louder but reduce dynamic range.
                  </span>
                </span>
              </div>
              <span class="text-primary font-bold">{{ audioProcessing.target_loudness_lufs }} LUFS</span>
            </div>
            <input
              type="range"
              min="-23" max="-9" step="0.5"
              :value="audioProcessing.target_loudness_lufs"
              @input="setTargetLoudness(Number(getEventValue($event)))"
              class="w-full h-1.5 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary mt-1"
            >
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import BaseToggle from '@/components/settings/BaseToggle.vue'

const getEventValue = (e: any) => e.target?.value || ''

const downloadSettings = useDownloadSettings()
const audioProcessing = computed(() => downloadSettings.audioProcessingSettings)

async function toggleEmbedLyrics() {
  downloadSettings.audioProcessingSettings.embed_lyrics = !downloadSettings.audioProcessingSettings.embed_lyrics
  await downloadSettings.saveAudioProcessingSettings()
}

async function toggleEmbedArtwork() {
  downloadSettings.audioProcessingSettings.embed_artwork = !downloadSettings.audioProcessingSettings.embed_artwork
  await downloadSettings.saveAudioProcessingSettings()
}

async function setReplayGainMode(mode: string) {
  downloadSettings.audioProcessingSettings.replay_gain_mode = mode
  await downloadSettings.saveAudioProcessingSettings()
}

async function setTargetLoudness(lufs: number) {
  downloadSettings.audioProcessingSettings.target_loudness_lufs = lufs
  await downloadSettings.saveAudioProcessingSettings()
}
</script>