<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <span>Audio Processing & Formatting</span>
        </h3>
      </div>

      <!-- AP-01 Context / Purpose Banner -->
      <div class="p-3.5 bg-blue-500/10 border border-blue-500/20 rounded-xl flex items-start gap-3 text-xs text-text-secondary">
        <span class="material-symbols-outlined text-blue-500 text-[20px] shrink-0 mt-0.5">info</span>
        <div>
          <span class="font-medium text-gray-900 dark:text-white block mb-0.5">When should you use audio processing?</span>
          <span>Disabled by default because Syncify prioritizes direct bit-perfect downloads from the platforms. Enable it only if your portable player or sound system cannot play advanced formats (DSD, 24-bit FLAC, MQA) and you need conversion to standard MP3 or AAC.</span>
        </div>
      </div>

      <div v-if="downloadSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary py-4">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading audio settings...</span>
      </div>
      <div v-else class="space-y-6">
        <!-- Transcoding -->
        <div class="p-4 bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark space-y-3">
          <BaseToggle
            title="Enable transcoding"
            subtitle="Convert rare or hi-res formats to standard consumer formats (MP3/AAC/FLAC)"
            tooltip="Enables transcoding with FFmpeg. When disabled, files are kept in their native format exactly as downloaded."
            :checked="audioProcessing.transcode_enabled"
            @click="toggleTranscoding"
          />

          <div v-if="audioProcessing.transcode_enabled" class="mt-4 space-y-4 pt-4 border-t border-gray-100 dark:border-gray-700">
            <div class="grid grid-cols-2 gap-4">
              <div>
                <div class="flex items-center gap-1.5 mb-1">
                  <label class="block text-xs font-medium text-gray-600 dark:text-gray-400">Target format</label>
                  <span
                    class="relative group/fmt-tip inline-flex items-center"
                    title="Target format to convert tracks to."
                  >
                    <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                    <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/fmt-tip:block z-40 w-52 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                      Final format of the encoded file (MP3, AAC, FLAC, Opus).
                    </span>
                  </span>
                </div>
                <select
                  :value="audioProcessing.transcode_format"
                  @change="setTranscodeFormat(getEventValue($event))"
                  class="w-full px-3 py-1.5 bg-gray-50 dark:bg-surface-highlight border border-gray-300 dark:border-gray-600 rounded-lg text-xs text-gray-900 dark:text-white"
                >
                  <option value="mp3">MP3</option>
                  <option value="aac">AAC (M4A)</option>
                  <option value="flac">FLAC</option>
                  <option value="opus">Opus</option>
                </select>
              </div>

              <div>
                <div class="flex items-center gap-1.5 mb-1">
                  <label class="block text-xs font-medium text-gray-600 dark:text-gray-400">Bitrate</label>
                  <span
                    class="relative group/br-tip inline-flex items-center"
                    title="Target bitrate for lossy formats (CBR)."
                  >
                    <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                    <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/br-tip:block z-40 w-52 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                      Compression bitrate. 320 kbps is the maximum quality for MP3.
                    </span>
                  </span>
                </div>
                <select
                  :value="audioProcessing.transcode_bitrate"
                  @change="setTranscodeBitrate(Number(getEventValue($event)))"
                  class="w-full px-3 py-1.5 bg-gray-50 dark:bg-surface-highlight border border-gray-300 dark:border-gray-600 rounded-lg text-xs text-gray-900 dark:text-white"
                >
                  <option :value="128">128 kbps</option>
                  <option :value="192">192 kbps</option>
                  <option :value="256">256 kbps</option>
                  <option :value="320">320 kbps</option>
                </select>
              </div>
            </div>
          </div>
        </div>

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

          <div v-if="audioProcessing.embed_artwork" class="pt-3 border-t border-gray-100 dark:border-gray-700">
            <div class="flex justify-between items-center text-xs mb-1">
              <div class="flex items-center gap-1.5">
                <label class="font-medium text-gray-700 dark:text-gray-300">Max artwork resolution</label>
                <span
                  class="relative group/art-tip inline-flex items-center"
                  title="Maximum desired resolution for the embedded cover art."
                >
                  <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                  <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/art-tip:block z-40 w-56 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                    Maximum desired width/height dimension. During native download, Syncify fetches the original high-resolution cover and caps the file size at 800 KB for maximum mobile compatibility.
                  </span>
                </span>
              </div>
              <span class="text-primary font-bold">{{ audioProcessing.artwork_max_size }} px</span>
            </div>
            <input
              type="range"
              min="500" max="3000" step="100"
              :value="audioProcessing.artwork_max_size"
              @input="setArtworkMaxSize(Number(getEventValue($event)))"
              class="w-full h-1.5 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary mt-1"
            >
          </div>
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

async function toggleTranscoding() {
  downloadSettings.audioProcessingSettings.transcode_enabled = !downloadSettings.audioProcessingSettings.transcode_enabled
  await downloadSettings.saveAudioProcessingSettings()
}

async function setTranscodeFormat(format: string) {
  downloadSettings.audioProcessingSettings.transcode_format = format
  await downloadSettings.saveAudioProcessingSettings()
}

async function setTranscodeBitrate(bitrate: number) {
  downloadSettings.audioProcessingSettings.transcode_bitrate = bitrate
  await downloadSettings.saveAudioProcessingSettings()
}

async function toggleEmbedLyrics() {
  downloadSettings.audioProcessingSettings.embed_lyrics = !downloadSettings.audioProcessingSettings.embed_lyrics
  await downloadSettings.saveAudioProcessingSettings()
}

async function toggleEmbedArtwork() {
  downloadSettings.audioProcessingSettings.embed_artwork = !downloadSettings.audioProcessingSettings.embed_artwork
  await downloadSettings.saveAudioProcessingSettings()
}

async function setArtworkMaxSize(size: number) {
  downloadSettings.audioProcessingSettings.artwork_max_size = size
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