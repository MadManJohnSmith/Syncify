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
          <span class="font-medium text-gray-900 dark:text-white block mb-0.5">¿Cuándo usar el procesamiento de audio?</span>
          <span>Por defecto viene desactivado porque Syncify prioriza descargas bit-perfect directas de las plataformas. Actívalo solo si tu reproductor portátil o equipo de sonido no reproduce formatos avanzados (DSD, FLAC 24-bit, MQA) y necesitas conversión a MP3 o AAC estándar.</span>
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
            tooltip="Activa la transcodificación con FFmpeg. Si se desactiva, los archivos se conservan en su formato nativo tal como se descargaron."
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
                    title="Formato destino al que convertir las pistas."
                  >
                    <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                    <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/fmt-tip:block z-40 w-52 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                      Formato final del archivo codificado (MP3, AAC, FLAC, Opus).
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
                    title="Tasa de bits objetivo para formatos con pérdida (CBR)."
                  >
                    <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                    <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/br-tip:block z-40 w-52 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                      Bitrate de compresión. 320 kbps es la máxima calidad para MP3.
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
            <span class="text-[11px] text-text-secondary">Metadatos físicos dentro del audio</span>
          </div>

          <BaseToggle
            title="Embed lyrics in audio files"
            subtitle="Incrustar letras sincronizadas y no sincronizadas (LYRICS / UNSYNCEDLYRICS)"
            tooltip="Incrusta letras dentro del contenedor de audio. Las descargas directas FLAC ya integran las letras disponibles automáticamente; este ajuste controla la incrustación en conversiones y tareas por lotes."
            :checked="audioProcessing.embed_lyrics"
            @click="toggleEmbedLyrics"
          />

          <BaseToggle
            title="Embed album artwork"
            subtitle="Incrustar carátula en los bloques de imagen de los archivos descargados"
            tooltip="Incrusta la imagen de portada en METADATA_BLOCK_PICTURE. Por estándar de Symfonium se preservan imágenes WebP/JPEG optimizadas hasta un límite seguro de 800 KB."
            :checked="audioProcessing.embed_artwork"
            @click="toggleEmbedArtwork"
          />

          <div v-if="audioProcessing.embed_artwork" class="pt-3 border-t border-gray-100 dark:border-gray-700">
            <div class="flex justify-between items-center text-xs mb-1">
              <div class="flex items-center gap-1.5">
                <label class="font-medium text-gray-700 dark:text-gray-300">Max artwork resolution</label>
                <span
                  class="relative group/art-tip inline-flex items-center"
                  title="Resolución máxima deseada para la carátula embebida."
                >
                  <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                  <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/art-tip:block z-40 w-56 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                    Dimensión de ancho/alto máxima deseada. Durante la descarga nativa, Syncify obtiene la portada original de alta resolución y limita el peso a 800 KB para máxima compatibilidad móvil.
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
                  Post-Procesamiento
                </span>
              </h4>
              <p class="text-xs text-text-secondary mt-0.5">Calcula metadatos de ganancia para evitar saltos bruscos de volumen entre canciones sin alterar la calidad del audio.</p>
            </div>
          </div>

          <div>
            <div class="flex items-center gap-1.5 mb-1">
              <label class="block text-xs font-medium text-gray-600 dark:text-gray-400">ReplayGain mode</label>
              <span
                class="relative group/rg-tip inline-flex items-center"
                title="Modo de cálculo para las etiquetas de ReplayGain."
              >
                <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/rg-tip:block z-40 w-56 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                  Off: sin normalización. Track: normaliza cada pista individualmente. Album: preserva la dinámica original del álbum completo. Both: escribe ambas etiquetas.
                </span>
              </span>
            </div>
            <select
              :value="audioProcessing.replay_gain_mode"
              @change="setReplayGainMode(getEventValue($event))"
              class="w-full px-3 py-1.5 bg-gray-50 dark:bg-surface-highlight border border-gray-300 dark:border-gray-600 rounded-lg text-xs text-gray-900 dark:text-white"
            >
              <option value="off">Off (desactivado)</option>
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
                  title="Nivel de sonoridad integrado objetivo según estándar EBU R128."
                >
                  <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
                  <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/lufs-tip:block z-40 w-56 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
                    Valor estándar EBU R128 es -18 o -14 LUFS. Los valores más cercanos a 0 son más altos pero reducen el rango dinámico.
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