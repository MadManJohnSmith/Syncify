<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
          Metadata Sources
        </h3>
        <span class="text-xs text-text-secondary">Enriquecimiento de biblioteca</span>
      </div>
      <div class="p-3 bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl text-xs text-text-secondary">
        <p>Fuentes de metadatos canónicos para enriquecer la biblioteca. Las descargas directas de streaming obtienen sus metadatos del proveedor (Tidal/Qobuz); estos ajustes rigen la catalogación y el enriquecimiento posterior.</p>
      </div>
      <BaseToggle
        title="Enable MusicBrainz lookups"
        subtitle="Use ISRC to fetch canonical metadata during library enrichment"
        tooltip="Consulta la base de datos abierta MusicBrainz para obtener ISRC, año y etiquetas canónicas. Se ejecuta en segundo plano durante el enriquecimiento de la biblioteca."
        :checked="metadataSettings.settings.enable_musicbrainz"
        @click="metadataSettings.settings.enable_musicbrainz = !metadataSettings.settings.enable_musicbrainz; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Enable Last.fm tags"
        subtitle="Fetch genres, moods, and community tags (requires Last.fm API key)"
        tooltip="Consulta Last.fm para rellenar géneros secundarios y estados de ánimo en la biblioteca. Requiere una clave de API configurada."
        :checked="metadataSettings.settings.enable_lastfm"
        @click="metadataSettings.settings.enable_lastfm = !metadataSettings.settings.enable_lastfm; metadataSettings.saveSettings()"
      />
    </section>

    <!-- Cat B: Tagging Behavior -->
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Tagging Behavior</h3>
      <BaseToggle
        title="Overwrite existing tags on re-import"
        subtitle="Sobrescribir etiquetas locales al volver a importar pistas"
        tooltip="Al reimportar una pista que ya existía, reemplaza las etiquetas existentes con los valores más recientes."
        :checked="metadataSettings.settings.overwrite_on_reimport"
        @click="metadataSettings.settings.overwrite_on_reimport = !metadataSettings.settings.overwrite_on_reimport; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Preserve custom tags"
        subtitle="Keep user-edited tags when refreshing metadata"
        tooltip="Mantiene las ediciones manuales introducidas por el usuario (títulos, géneros personalizados) al refrescar metadatos."
        :checked="metadataSettings.settings.preserve_custom_tags"
        @click="metadataSettings.settings.preserve_custom_tags = !metadataSettings.settings.preserve_custom_tags; metadataSettings.saveSettings()"
      />

      <div class="max-w-md">
        <div class="flex items-center gap-1.5 mb-2">
          <label class="block text-sm font-medium text-gray-700 dark:text-gray-300">Multi-value separator</label>
          <span
            class="relative group/sep-tip inline-flex items-center"
            title="Separador para etiquetas de valores múltiples."
          >
            <span class="inline-flex items-center justify-center w-4 h-4 rounded-full bg-gray-200 dark:bg-surface-highlight text-gray-600 dark:text-gray-300 text-[10px] font-bold cursor-help">?</span>
            <span role="tooltip" class="absolute left-1/2 -translate-x-1/2 bottom-full mb-1.5 hidden group-hover/sep-tip:block z-40 w-56 p-2 text-xs font-normal text-white bg-gray-900 dark:bg-gray-800 rounded-lg shadow-xl pointer-events-none">
              Carácter separador utilizado al escribir etiquetas de múltiples valores (por ejemplo, múltiples artistas o géneros) en archivos de audio.
            </span>
          </span>
        </div>
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
      <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Symfonium-Specific Tags</h3>
        <span class="px-2 py-0.5 text-[10px] font-bold rounded-full bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20">
          Estándar Vorbis FLAC
        </span>
      </div>
      <p class="text-xs text-text-secondary">Etiquetas estándar para compatibilidad con reproductores como Symfonium. El motor de FLAC las incluye automáticamente cuando están disponibles.</p>
      <BaseToggle
        title="Write RELEASETYPE tag"
        subtitle="Album / Single / EP / Compilation / Live"
        tooltip="Clasifica el tipo de lanzamiento (Album, Single, EP, Compilation, Live) para ordenación por tipo en reproductores móviles."
        :checked="metadataSettings.settings.write_releasetype"
        @click="metadataSettings.settings.write_releasetype = !metadataSettings.settings.write_releasetype; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write LABEL tag"
        subtitle="Record label and organization"
        tooltip="Escribe el sello discográfico o editor en las etiquetas LABEL y RECORDLABEL."
        :checked="metadataSettings.settings.write_label"
        @click="metadataSettings.settings.write_label = !metadataSettings.settings.write_label; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write WORK and COMPOSER tags"
        subtitle="For classical music organization"
        tooltip="Escribe las etiquetas WORK y COMPOSER para la organización por movimientos y obras clásicas."
        :checked="metadataSettings.settings.write_work_composer"
        @click="metadataSettings.settings.write_work_composer = !metadataSettings.settings.write_work_composer; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write MusicBrainz IDs"
        subtitle="MUSICBRAINZ_TRACKID, MUSICBRAINZ_RELEASEID, etc."
        tooltip="Incrusta los UUIDs canónicos de MusicBrainz (Recording, Release, Artist) para vincular la pista con precisión matemática."
        :checked="metadataSettings.settings.write_musicbrainz_ids"
        @click="metadataSettings.settings.write_musicbrainz_ids = !metadataSettings.settings.write_musicbrainz_ids; metadataSettings.saveSettings()"
      />
    </section>

    <!-- Cat B: Custom App Tags -->
    <section class="space-y-4">
      <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Custom App Tags</h3>
        <span class="px-2 py-0.5 text-[10px] font-bold rounded-full bg-purple-500/10 text-purple-600 dark:text-purple-400 border border-purple-500/20">
          Auditoría Interna
        </span>
      </div>
      <p class="text-xs text-text-secondary">Etiquetas extendidas de catalogación de Syncify. Se aplican en operaciones de exportación y mantenimiento de biblioteca.</p>
      <BaseToggle
        title="Write DOWNLOAD_SOURCE tag"
        subtitle="Which service the file was downloaded from (e.g. tidal, qobuz)"
        tooltip="Guarda el identificador del servicio de descarga original en los metadatos Vorbis."
        :checked="metadataSettings.settings.write_download_source"
        @click="metadataSettings.settings.write_download_source = !metadataSettings.settings.write_download_source; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write DOWNLOAD_DATE tag"
        subtitle="Timestamp of when the track was downloaded"
        tooltip="Registra la fecha y hora exacta de descarga en formato estándar ISO."
        :checked="metadataSettings.settings.write_download_date"
        @click="metadataSettings.settings.write_download_date = !metadataSettings.settings.write_download_date; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write ONLY_AVAILABLE_ON tag"
        subtitle="Mark tracks exclusive to one service"
        tooltip="Identifica pistas exclusivas que solo existen en un único proveedor de streaming."
        :checked="metadataSettings.settings.write_only_available_on"
        @click="metadataSettings.settings.write_only_available_on = !metadataSettings.settings.write_only_available_on; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write NOT_AVAILABLE_STREAMING tag"
        subtitle="Mark tracks that are not available in streaming"
        tooltip="Señala pistas descatalogadas o no disponibles para streaming."
        :checked="metadataSettings.settings.write_not_available_streaming"
        @click="metadataSettings.settings.write_not_available_streaming = !metadataSettings.settings.write_not_available_streaming; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write METADATA_QUALITY_SCORE tag"
        subtitle="0-100 completeness score"
        tooltip="Escribe la puntuación calculada de calidad de metadatos (0-100) dentro del archivo físico."
        :checked="metadataSettings.settings.write_quality_score"
        @click="metadataSettings.settings.write_quality_score = !metadataSettings.settings.write_quality_score; metadataSettings.saveSettings()"
      />
      <BaseToggle
        title="Write LYRICS_TYPE and LYRICS_SOURCE tags"
        subtitle="Record lyrics provider and sync format"
        tooltip="Escribe si la letra es sincronizada o sin sincronizar y el proveedor que la suministró."
        :checked="metadataSettings.settings.write_lyrics_tags"
        @click="metadataSettings.settings.write_lyrics_tags = !metadataSettings.settings.write_lyrics_tags; metadataSettings.saveSettings()"
      />
    </section>

    <!-- Metadata Quality Scoring (SC-01, SC-02, SC-03, SC-04) -->
    <section class="space-y-4">
      <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <span>Metadata Quality Scoring</span>
            <span class="px-2 py-0.5 text-[10px] font-bold rounded-full bg-primary/10 text-primary border border-primary/20">
              Paridad Local y Streaming (SC-03)
            </span>
          </h3>
          <p class="text-xs text-text-secondary mt-0.5">Ponderación para evaluar la completitud de metadatos (0-100%). Aplica con la misma fórmula a descargas y a la biblioteca local.</p>
        </div>
      </div>

      <div v-if="metadataSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary py-4">
        <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
        <span class="text-sm">Loading scoring settings...</span>
      </div>
      <div v-else class="space-y-4">
        <div class="flex items-center justify-between text-xs text-text-secondary pb-1">
          <span>Ajusta la importancia relativa de cada campo (0 a 10).</span>
          <span v-if="metadataSettings.isSaving.value" class="text-primary font-medium flex items-center gap-1 animate-pulse">
            <span class="material-symbols-outlined text-[14px]">sync</span> Guardando...
          </span>
          <span v-else class="text-emerald-500 font-medium flex items-center gap-1">
            <span class="material-symbols-outlined text-[14px]">check</span> Cambios guardados
          </span>
        </div>

        <SliderInput
          label="Album Weight"
          v-model="metadataSettings.settings.weight_album"
          @update:modelValue="metadataSettings.saveDebounced()"
          subtitle="Ponderación para la presencia de Álbum"
          tooltip="Ponderación del campo Álbum. Mayor peso penaliza a las canciones sin álbum asignado."
        />
        <SliderInput
          label="ISRC Weight"
          v-model="metadataSettings.settings.weight_isrc"
          @update:modelValue="metadataSettings.saveDebounced()"
          subtitle="Ponderación para código identificador ISRC"
          tooltip="Ponderación del código internacional ISRC. Esencial para identificación canónica sin ambigüedades."
        />
        <SliderInput
          label="MusicBrainz ID Weight"
          v-model="metadataSettings.settings.weight_mb_id"
          @update:modelValue="metadataSettings.saveDebounced()"
          subtitle="Ponderación para MusicBrainz recording ID"
          tooltip="Ponderación de la coincidencia con MusicBrainz. Clave para metadatos auditados y canónicos."
        />
        <SliderInput
          label="Cover Art Weight"
          v-model="metadataSettings.settings.weight_cover"
          @update:modelValue="metadataSettings.saveDebounced()"
          subtitle="Ponderación para disponibilidad de carátula"
          tooltip="Ponderación de la disponibilidad de imagen de portada en alta calidad."
        />
        <SliderInput
          label="Release Year Weight"
          v-model="metadataSettings.settings.weight_year"
          @update:modelValue="metadataSettings.saveDebounced()"
          subtitle="Ponderación para el año de lanzamiento"
          tooltip="Ponderación del año de lanzamiento original del álbum o tema."
        />
        <SliderInput
          label="Genre Weight"
          v-model="metadataSettings.settings.weight_genre"
          @update:modelValue="metadataSettings.saveDebounced()"
          subtitle="Ponderación para metadatos de género"
          tooltip="Ponderación del género musical etiquetado."
        />

        <!-- Live Score Simulation Card (SC-02, SC-03) -->
        <div class="p-3 bg-gray-50 dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl flex items-center justify-between text-xs">
          <div>
            <span class="font-medium text-gray-900 dark:text-white block">Simulación en pista de ejemplo (Título + Artista + Álbum + Año + Portada + Género)</span>
            <span class="text-text-secondary text-[11px]">Calificación resultante: <strong>{{ simulatedScore }}%</strong> · Aplica de forma simétrica a la biblioteca local y streaming (SC-03)</span>
          </div>
          <div class="text-base font-bold text-primary px-3 py-1 bg-primary/10 rounded-lg">
            {{ simulatedScore }}%
          </div>
        </div>
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
              ? 'bg-primary text-on-accent shadow-sm'
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
              ? 'bg-primary text-on-accent shadow-sm'
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
              ? 'bg-primary text-on-accent shadow-sm'
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
          class="px-4 py-2 bg-primary hover:bg-primary/90 text-on-accent rounded-xl text-xs font-semibold shadow-sm transition-all flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
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
            class="px-4 py-2 bg-primary hover:bg-primary-hover text-on-accent text-xs font-semibold rounded-lg transition-colors flex items-center gap-1.5 shadow-xs"
          >
            <span class="material-symbols-outlined text-[16px]">build_circle</span>
            Review Repair Plan
          </button>
        </div>
      </div>
    </section>

    <!-- S143B/S159: Retroactive Track Version Disambiguation Repair -->
    <section class="space-y-4 pt-4 border-t border-gray-200 dark:border-border-dark">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2">
            <span>Track Version Disambiguation Repair</span>
            <span class="px-2 py-0.5 text-[10px] font-bold rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20">Dry-Run Plan</span>
          </h3>
          <p class="text-xs text-text-secondary">Detect duplicate album titles saved under identical filenames, then rename audio + LRC sidecars with SHA-256 verification and automatic rollback.</p>
        </div>
        <button
          @click="showDisambiguationRepairModal = true"
          class="px-4 py-2 bg-primary hover:bg-primary-hover text-on-accent text-xs font-semibold rounded-lg transition-colors flex items-center gap-1.5 shrink-0 shadow-xs"
        >
          <span class="material-symbols-outlined text-[16px]">drive_file_rename_outline</span>
          Review Renames
        </button>
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
            class="px-4 py-2 bg-primary hover:bg-primary-hover text-on-accent text-xs font-semibold rounded-lg transition-colors flex items-center gap-1.5 shadow-xs"
          >
            <span class="material-symbols-outlined text-[16px]">graphic_eq</span>
            Analyze Library BPM
          </button>
        </div>
      </div>
    </section>

    <!-- S158: Tidal Repair Review Modal -->
    <TidalRepairReviewModal v-model="showTidalRepairModal" />

    <!-- S143B/S159: Track Version Disambiguation Repair Review Modal -->
    <DisambiguationRepairReviewModal v-model="showDisambiguationRepairModal" />

    <!-- S163: Applied Repairs History Modal -->
    <RepairHistoryModal v-model="showRepairHistoryModal" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useMetadataSettings } from '@/composables/useMetadataSettings'
import { useIncrementalEnrichment } from '@/composables/useIncrementalEnrichment'
import { useTempoAnalysis } from '@/composables/useTempoAnalysis'
import type { EnrichmentMode } from '@/api/types'
import BaseToggle from '@/components/settings/BaseToggle.vue'
import SliderInput from './SliderInput.vue'
import TidalRepairReviewModal from '@/components/TidalRepairReviewModal.vue'
import DisambiguationRepairReviewModal from '@/components/DisambiguationRepairReviewModal.vue'
import RepairHistoryModal from '@/components/RepairHistoryModal.vue'

const showTidalRepairModal = ref(false)
const showDisambiguationRepairModal = ref(false)
const showRepairHistoryModal = ref(false)
const metadataSettings = useMetadataSettings()
const enrichment = useIncrementalEnrichment()
const tempo = useTempoAnalysis()

const simulatedScore = computed(() => {
  const w = metadataSettings.settings
  const base = 20
  const wAlbum = Math.max(0, w.weight_album ?? 1)
  const wIsrc = Math.max(0, w.weight_isrc ?? 1)
  const wMb = Math.max(0, w.weight_mb_id ?? 1)
  const wCover = Math.max(0, w.weight_cover ?? 1)
  const wYear = Math.max(0, w.weight_year ?? 1)
  const wGenre = Math.max(0, w.weight_genre ?? 1)
  const total = wAlbum + wIsrc + wMb + wCover + wYear + wGenre
  if (total === 0) return 100
  const earned = wAlbum + wCover + wYear + wGenre
  return Math.round(base + (earned / total) * 80)
})

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


