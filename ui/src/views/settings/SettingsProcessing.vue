<template>
  <div class="space-y-8">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Audio Processing Settings</h3>
      <div v-if="downloadSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading audio settings...</span>
      </div>
      <div v-else class="space-y-6">
        <!-- Transcoding -->
        <div class="p-4 bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark">
          <div class="flex items-center justify-between cursor-pointer" @click="toggleTranscoding">
            <div>
              <span class="block text-sm font-medium text-gray-900 dark:text-white">Enable transcoding</span>
              <span class="block text-xs text-text-secondary mt-0.5">Convert rare formats (DSD, MQA) to standard formats</span>
            </div>
            <div class="relative inline-block w-10 align-middle select-none">
              <div :class="audioProcessing.transcode_enabled ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
              <div :class="audioProcessing.transcode_enabled ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
            </div>
          </div>
          
          <div v-if="audioProcessing.transcode_enabled" class="mt-4 space-y-3 pt-4 border-t border-gray-100 dark:border-gray-700">
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Target format</label>
                <select 
                  :value="audioProcessing.transcode_format"
                  @change="setTranscodeFormat(getEventValue($event))"
                  class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
                >
                  <option value="flac">FLAC</option>
                  <option value="alac">ALAC</option>
                  <option value="mp3">MP3</option>
                  <option value="aac">AAC</option>
                </select>
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Bitrate (if lossy)</label>
                <select 
                  :value="audioProcessing.transcode_bitrate"
                  @change="setTranscodeBitrate(Number(getEventValue($event)))"
                  class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
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
        <div class="p-4 bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark space-y-3">
          <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-3">Embedding</h4>
          
          <div class="flex items-center justify-between cursor-pointer" @click="toggleEmbedLyrics">
            <span class="text-sm text-gray-700 dark:text-gray-300">Embed lyrics in audio files</span>
            <div class="relative inline-block w-10 align-middle select-none">
              <div :class="audioProcessing.embed_lyrics ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
              <div :class="audioProcessing.embed_lyrics ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
            </div>
          </div>
          
          <div class="flex items-center justify-between cursor-pointer" @click="toggleEmbedArtwork">
            <span class="text-sm text-gray-700 dark:text-gray-300">Embed album artwork</span>
            <div class="relative inline-block w-10 align-middle select-none">
              <div :class="audioProcessing.embed_artwork ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600'" class="block h-5 rounded-full transition-colors"></div>
              <div :class="audioProcessing.embed_artwork ? 'translate-x-5' : 'translate-x-0'" class="absolute top-0 left-0 w-5 h-5 bg-white rounded-full shadow transform transition-transform"></div>
            </div>
          </div>
          
          <div v-if="audioProcessing.embed_artwork" class="mt-3 pt-3 border-t border-gray-100 dark:border-gray-700">
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-2">Max artwork size (px)</label>
            <div class="flex items-center gap-3">
              <input 
                type="range" 
                min="300" max="3000" step="100"
                :value="audioProcessing.artwork_max_size"
                @input="setArtworkMaxSize(Number(getEventValue($event)))"
                class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary"
              >
              <span class="text-sm font-medium text-gray-900 dark:text-white w-16 text-right">{{ audioProcessing.artwork_max_size }}px</span>
            </div>
          </div>
        </div>
        
        <!-- ReplayGain -->
        <div class="p-4 bg-white dark:bg-surface-dark rounded-xl border border-gray-200 dark:border-border-dark">
          <h4 class="text-sm font-medium text-gray-900 dark:text-white mb-3">ReplayGain</h4>
          <div>
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">Mode</label>
            <select 
              :value="audioProcessing.replay_gain_mode"
              @change="setReplayGainMode(getEventValue($event))"
              class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm"
            >
              <option value="off">Off</option>
              <option value="track">Track gain</option>
              <option value="album">Album gain</option>
              <option value="both">Both</option>
            </select>
          </div>
          
          <div v-if="audioProcessing.replay_gain_mode !== 'off'" class="mt-3">
            <label class="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-2">Target loudness</label>
            <div class="flex items-center gap-3">
              <input 
                type="range" 
                min="-23" max="-9" step="0.5"
                :value="audioProcessing.target_loudness_lufs"
                @input="setTargetLoudness(Number(getEventValue($event)))"
                class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary"
              >
              <span class="text-sm font-medium text-gray-900 dark:text-white w-20 text-right">{{ audioProcessing.target_loudness_lufs }} LUFS</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useDownloadSettings } from '@/composables/useDownloadSettings'

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

onMounted(async () => {
  if (!downloadSettings.isLoading.value && downloadSettings.audioProcessingSettings.transcode_format === undefined) {
    await downloadSettings.loadSettings()
  }
})
</script>
