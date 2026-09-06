<template>
  <div class="metadata-page h-full flex bg-background-light dark:bg-background-dark overflow-hidden">
    
    <!-- Left Panel: Track Selector (40%) -->
    <div class="track-selector w-2/5 flex flex-col border-r border-gray-200 dark:border-border-dark">
      
      <!-- Toolbar -->
      <div class="px-4 py-3 border-b border-gray-200 dark:border-border-dark shrink-0">
        <div class="flex items-center gap-3">
          <!-- Search -->
          <div class="relative flex-1">
            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-[18px]">search</span>
            <input 
              v-model="searchQuery"
              type="text" 
              placeholder="Search tracks..."
              class="w-full pl-10 pr-4 py-2 bg-gray-100 dark:bg-surface-highlight border-0 rounded-lg text-sm text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50"
            >
          </div>
          
          <!-- Filter -->
          <select v-model="filterType" class="px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
            <option value="all">All Tracks</option>
            <option value="needs_work">Needs Work</option>
            <option value="downloaded">Downloaded Only</option>
            <option value="low-quality">Low Quality (&lt;70%)</option>
            <option value="missing">Missing Metadata</option>
            <option value="no-art">No Album Art</option>
          </select>
          
          <!-- Sort -->
          <select v-model="sortBy" class="px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
            <option value="title">Title</option>
            <option value="artist">Artist</option>
            <option value="album">Album</option>
            <option value="score">Metadata Score</option>
            <option value="date">Date Added</option>
          </select>
          
          <!-- Quality Analyzer Button -->
          <button @click="showQualityReport = true" class="p-2 bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 rounded-lg transition-colors" title="Analyze Quality">
            <span class="material-symbols-outlined text-[20px]">analytics</span>
          </button>

          <!-- S158: Tidal Repair Dry-Run Review Button -->
          <button @click="showTidalRepairModal = true" class="p-2 bg-blue-500/10 text-blue-500 hover:bg-blue-500/20 rounded-lg transition-colors" title="Review Tidal Repair Dry-Run Plan">
            <span class="material-symbols-outlined text-[20px]">build_circle</span>
          </button>

          <!-- S163: Applied Repairs History Button -->
          <button @click="showRepairHistoryModal = true" class="p-2 bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 rounded-lg transition-colors" title="View Applied Repairs History">
            <span class="material-symbols-outlined text-[20px]">history_edu</span>
          </button>
        </div>
        
        <!-- Enhanced Batch Toolbar -->
        <Transition name="slide-down">
          <div v-if="selectedTracks.length > 0" class="batch-toolbar mt-3 p-3 bg-primary/10 rounded-lg">
            <div class="flex items-center justify-between">
              <span class="text-sm text-primary font-semibold">{{ selectedTracks.length }} track{{ selectedTracks.length > 1 ? 's' : '' }} selected</span>
              <div class="flex items-center gap-2">
                <button @click="showAutoFixPanel = !showAutoFixPanel" class="flex items-center gap-1.5 px-3 py-1.5 bg-purple-500/20 text-purple-400 hover:bg-purple-500/30 rounded-lg text-xs font-medium transition-colors">
                  <span class="material-symbols-outlined text-[14px]">auto_fix_high</span>
                  Auto-Fix
                </button>
                <button @click="fetchMissingArtwork()" :disabled="isFetchingArt" class="flex items-center gap-1.5 px-3 py-1.5 bg-green-500/20 text-green-400 hover:bg-green-500/30 rounded-lg text-xs font-medium transition-colors disabled:opacity-50">
                  <span :class="['material-symbols-outlined text-[14px]', isFetchingArt && 'animate-spin']">{{ isFetchingArt ? 'progress_activity' : 'image' }}</span>
                  Fetch Art
                </button>
                <button @click="exportSelectedMetadata" :disabled="isExportingMetadata" class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-500/20 text-blue-400 hover:bg-blue-500/30 rounded-lg text-xs font-medium transition-colors disabled:opacity-50">
                  <span :class="['material-symbols-outlined text-[14px]', isExportingMetadata && 'animate-spin']">{{ isExportingMetadata ? 'progress_activity' : 'download' }}</span>
                  Export
                </button>
                <div class="w-px h-4 bg-gray-300 dark:bg-gray-600"></div>
                <button @click="selectAll" class="text-xs text-primary hover:underline">Select All</button>
                <button @click="clearSelection" class="text-xs text-primary hover:underline">Clear</button>
              </div>
            </div>
          </div>
        </Transition>
      </div>
      
      <!-- Background enrichment live status -->
      <Transition name="slide-down">
        <div
          v-if="backgroundEnrichment || enrichProgress"
          :class="['mx-4 mt-3 shrink-0 rounded-lg border px-4 py-2.5 flex items-center gap-3 text-xs', backgroundEnrichment ? getEnrichmentStatusBg(backgroundEnrichment) : 'bg-primary/5 border-primary/20']"
        >
          <template v-if="enrichProgress">
            <span class="material-symbols-outlined text-[16px] text-primary animate-spin">progress_activity</span>
            <div class="flex-1 min-w-0">
              <p class="text-gray-900 dark:text-white font-medium truncate">{{ enrichProgress.currentTrack || 'Enriqueciendo…' }}</p>
              <div v-if="enrichProgress.total > 0" class="mt-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div class="h-full bg-primary rounded-full transition-all" :style="{ width: Math.round((enrichProgress.current / enrichProgress.total) * 100) + '%' }"></div>
              </div>
            </div>
            <span v-if="enrichProgress.total > 0" class="font-mono text-text-secondary shrink-0">{{ enrichProgress.current }}/{{ enrichProgress.total }}</span>
          </template>
          <template v-else-if="backgroundEnrichment">
            <span :class="['material-symbols-outlined text-[16px]', getEnrichmentStatusColor(backgroundEnrichment)]">{{ getEnrichmentIcon(backgroundEnrichment) }}</span>
            <span class="text-gray-700 dark:text-gray-300 flex-1 truncate">
              <strong>{{ getEnrichmentTitle(backgroundEnrichment) }}:</strong> {{ backgroundEnrichment.message }}
            </span>
          </template>
        </div>
      </Transition>

      <!-- Library metadata completeness strip (real get_metadata_stats data) -->
      <div
        v-if="metadataStats"
        class="mx-4 mt-3 shrink-0 flex flex-wrap items-center gap-x-4 gap-y-1 rounded-lg bg-gray-50 dark:bg-surface-highlight/40 border border-gray-200 dark:border-border-dark px-4 py-2 text-[11px] text-text-secondary"
      >
        <span class="font-medium text-gray-900 dark:text-white">{{ metadataStats.total_tracks }} pistas</span>
        <span :class="statPct(metadataStats.with_isrc, metadataStats.total_tracks) >= 90 ? 'text-success' : ''">ISRC {{ statPct(metadataStats.with_isrc, metadataStats.total_tracks) }}%</span>
        <span :class="statPct(metadataStats.with_musicbrainz_id, metadataStats.total_tracks) >= 90 ? 'text-success' : ''">MBID {{ statPct(metadataStats.with_musicbrainz_id, metadataStats.total_tracks) }}%</span>
        <span :class="statPct(metadataStats.with_art, metadataStats.total_tracks) >= 90 ? 'text-success' : ''">Carátulas {{ statPct(metadataStats.with_art, metadataStats.total_tracks) }}%</span>
        <span>Género {{ statPct(metadataStats.with_genre, metadataStats.total_tracks) }}%</span>
        <span>Año {{ statPct(metadataStats.with_year, metadataStats.total_tracks) }}%</span>
        <span class="ml-auto font-mono">Completitud media {{ metadataStats.average_completeness.toFixed(1) }}%</span>
      </div>

      <!-- Loading State -->
      <div v-if="isLoading" class="flex-1 flex items-center justify-center">
        <div class="text-center">
          <span class="material-symbols-outlined text-4xl text-primary animate-spin">progress_activity</span>
          <p class="text-text-secondary mt-2">Loading tracks...</p>
        </div>
      </div>
      
      <!-- Empty State -->
      <div v-else-if="tracks.length === 0" class="flex-1 flex items-center justify-center">
        <div class="text-center p-8">
          <span class="material-symbols-outlined text-5xl text-gray-400 mb-4">library_music</span>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">No tracks in library</h3>
          <p class="text-text-secondary">Import music from streaming services to get started</p>
        </div>
      </div>

      <!-- Track List -->
      <div v-else class="track-list flex-1 overflow-y-auto custom-scrollbar">
        <div 
          v-for="track in filteredTracks" 
          :key="track.id"
          @click="selectTrack(track)"
          :class="[
            'track-row flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors border-l-2',
            selectedTracks.includes(track.id) ? 'bg-primary/10 border-l-primary' : 
            track.issues > 0 ? 'border-l-amber-500 hover:bg-gray-50 dark:hover:bg-surface-highlight/50' : 
            'border-l-transparent hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
          ]"
        >
          <!-- Checkbox -->
          <input 
            type="checkbox" 
            :checked="selectedTracks.includes(track.id)"
            @click.stop="toggleTrackSelection(track.id)"
            class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary shrink-0"
          >
          
          <!-- Album Art -->
          <div class="w-10 h-10 rounded bg-gray-200 dark:bg-surface-highlight shrink-0 overflow-hidden">
            <img v-if="track.coverUrl" :src="track.coverUrl" :alt="track.album" class="w-full h-full object-cover" loading="lazy">
            <div v-else class="w-full h-full bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-600 dark:to-gray-700 flex items-center justify-center">
              <span class="material-symbols-outlined text-gray-500 dark:text-gray-400 text-[18px]">album</span>
            </div>
          </div>
          
          <!-- Track Info -->
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ track.title }}</p>
            <p class="text-xs text-text-secondary truncate">{{ track.artist }}</p>
          </div>
          
          <!-- Metadata Score -->
          <div class="flex items-center gap-2 shrink-0">
            <div :class="['relative w-9 h-9 rounded-full flex items-center justify-center', getScoreBackground(track.score)]">
              <svg class="absolute inset-0" viewBox="0 0 36 36">
                <circle cx="18" cy="18" r="15" fill="none" stroke="currentColor" stroke-width="3" class="opacity-20" />
                <circle 
                  cx="18" cy="18" r="15" fill="none" stroke="currentColor" stroke-width="3"
                  :stroke-dasharray="`${track.score} 100`"
                  stroke-linecap="round"
                  transform="rotate(-90 18 18)"
                />
              </svg>
              <span class="text-[10px] font-bold">{{ track.score }}%</span>
            </div>
            
            <!-- Issues Badge -->
            <span v-if="track.issues > 0" class="px-1.5 py-0.5 bg-error/10 text-error text-[10px] font-medium rounded">
              {{ track.issues }}
            </span>

            <!-- Inline Action Menu -->
            <div class="relative group/menu">
              <button @click.stop="openActionMenu(track, $event)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-700 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-200">
                <span class="material-symbols-outlined text-[20px]">more_vert</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Right Panel: Metadata Editor (60%) -->
    <div class="metadata-editor w-3/5 flex flex-col overflow-hidden">
      
      <!-- Auto-Fix Tools Panel (Collapsible) -->
      <Transition name="slide-down">
        <div v-if="showAutoFixPanel" class="autofix-panel shrink-0 border-b border-gray-200 dark:border-border-dark">
          <button @click="showAutoFixPanel = false" class="w-full px-6 py-3 flex items-center justify-between bg-purple-500/5 hover:bg-purple-500/10 transition-colors">
            <div class="flex items-center gap-2">
              <span class="material-symbols-outlined text-purple-500 text-[20px]">auto_fix_high</span>
              <span class="font-semibold text-gray-900 dark:text-white">Auto-Fix Tools</span>
              <span class="px-2 py-0.5 bg-purple-500/20 text-purple-500 text-xs font-medium rounded-full">{{ selectedTracks.length }} selected</span>
            </div>
            <span class="material-symbols-outlined text-gray-400">expand_less</span>
          </button>
          
          <div class="p-4 grid grid-cols-2 gap-3">
            <!-- MusicBrainz Lookup -->
            <div class="fix-tool-card p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark hover:border-purple-500/50 transition-colors">
              <div class="flex items-start gap-3">
                <div class="h-10 w-10 rounded-xl bg-blue-500/10 text-blue-500 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-[20px]">database</span>
                </div>
                <div class="flex-1 min-w-0">
                  <h5 class="font-medium text-gray-900 dark:text-white text-sm">MusicBrainz Lookup</h5>
                  <p class="text-xs text-text-secondary mt-0.5">Fetch canonical metadata using ISRC</p>
                  <p class="text-xs text-success mt-1">Available for {{ tracksWithIsrcNoMb }} tracks with ISRC</p>
                </div>
              </div>
              <button 
                @click="runMusicBrainzEnrichment"
                :disabled="isEnriching || tracksWithIsrcNoMb === 0"
                class="w-full mt-3 px-3 py-2 bg-blue-500/10 text-blue-500 hover:bg-blue-500/20 rounded-lg text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1"
              >
                <span v-if="isEnriching" class="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                {{ isEnriching ? 'Enriching...' : 'Fix Selected' }}
              </button>
            </div>
            
            <!-- AcoustID Fingerprint -->
            <div class="fix-tool-card p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark hover:border-purple-500/50 transition-colors">
              <div class="flex items-start gap-3">
                <div class="h-10 w-10 rounded-xl bg-green-500/10 text-green-500 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-[20px]">fingerprint</span>
                </div>
                <div class="flex-1 min-w-0">
                  <h5 class="font-medium text-gray-900 dark:text-white text-sm">AcoustID Fingerprint</h5>
                  <p class="text-xs text-text-secondary mt-0.5">Identify tracks using audio fingerprint</p>
                  <p :class="['text-xs mt-1', unidentifiedWithFiles > 0 ? 'text-amber-500' : 'text-success']">
                    {{ unidentifiedWithFiles }} pista(s) sin MBID con archivo local
                  </p>
                </div>
              </div>
              <button
                @click="batchIdentifyAcoustID"
                :disabled="isBatchIdentifying || unidentifiedWithFiles === 0"
                class="w-full mt-3 px-3 py-2 bg-green-500/10 text-green-500 hover:bg-green-500/20 rounded-lg text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1"
              >
                <span v-if="isBatchIdentifying" class="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                {{ isBatchIdentifying ? `Identificando ${batchIdentifyProgress.done}/${batchIdentifyProgress.total}…` : 'Analyze Selected' }}
              </button>
            </div>
            
            <!-- Last.fm Tags -->
            <div class="fix-tool-card p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark hover:border-purple-500/50 transition-colors">
              <div class="flex items-start gap-3">
                <div class="h-10 w-10 rounded-xl bg-red-500/10 text-red-500 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-[20px]">sell</span>
                </div>
                <div class="flex-1 min-w-0">
                  <h5 class="font-medium text-gray-900 dark:text-white text-sm">Last.fm Tags</h5>
                  <p class="text-xs text-text-secondary mt-0.5">Fetch community genre tags</p>
                  <p class="text-xs mt-1" :class="tracksWithoutGenre > 0 ? 'text-amber-500' : 'text-success'">
                    {{ tracksWithoutGenre }} pista(s) sin género · solo rellena vacíos
                  </p>
                </div>
              </div>
              <button
                @click="runLastfmEnrichment"
                :disabled="isLastfmRunning || tracksWithoutGenre === 0"
                class="w-full mt-3 px-3 py-2 bg-red-500/10 text-red-500 hover:bg-red-500/20 rounded-lg text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1"
              >
                <span v-if="isLastfmRunning" class="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                {{ isLastfmRunning ? 'Fetching…' : 'Fetch Tags' }}
              </button>
              <!-- S200: API key management (BD primero, luego env) -->
              <div class="mt-3 pt-3 border-t border-gray-200 dark:border-border-dark">
                <label class="block text-[11px] text-text-secondary mb-1">API key de Last.fm</label>
                <div class="flex gap-1.5">
                  <input
                    v-model="lastfmKeyInput"
                    type="password"
                    :placeholder="lastfmKeyStatus?.configured ? `Configurada (${lastfmKeyStatus.masked})` : 'Pega tu API key…'"
                    class="flex-1 min-w-0 px-2 py-1.5 text-xs bg-white dark:bg-surface-dark border border-gray-300 dark:border-border-dark rounded-lg focus:outline-none focus:ring-1 focus:ring-primary"
                    autocomplete="off"
                  >
                  <button
                    @click="saveLastfmKey"
                    :disabled="isSavingLastfmKey || !lastfmKeyInput.trim()"
                    class="px-2.5 py-1.5 bg-red-500/10 text-red-500 hover:bg-red-500/20 rounded-lg text-xs font-medium transition-colors disabled:opacity-50 shrink-0"
                  >
                    {{ isSavingLastfmKey ? '…' : 'Guardar' }}
                  </button>
                </div>
                <p v-if="lastfmKeyStatus?.configured" class="text-[11px] text-success mt-1">
                  ✓ Configurada ({{ lastfmKeyStatus.source === 'env' ? 'variable de entorno' : 'guardada en la app' }})
                </p>
                <p v-else class="text-[11px] text-text-secondary mt-1">
                  Consíguela gratis en last.fm/api — necesaria para los géneros.
                </p>
              </div>
            </div>
            
            <!-- Album Art Search -->
            <div class="fix-tool-card p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark hover:border-purple-500/50 transition-colors">
              <div class="flex items-start gap-3">
                <div class="h-10 w-10 rounded-xl bg-amber-500/10 text-amber-500 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-[20px]">image</span>
                </div>
                <div class="flex-1 min-w-0">
                  <h5 class="font-medium text-gray-900 dark:text-white text-sm">Album Art Search</h5>
                  <p class="text-xs text-text-secondary mt-0.5">Cover Art Archive vía ISRC → release-group</p>
                  <p :class="['text-xs mt-1', tracksWithoutArt > 0 ? 'text-error' : 'text-success']">
                    {{ tracksWithoutArt }} pista(s) sin carátula
                  </p>
                </div>
              </div>
              <button
                @click="fetchMissingArtwork()"
                :disabled="isFetchingArt || tracksWithoutArt === 0"
                class="w-full mt-3 px-3 py-2 bg-amber-500/10 text-amber-500 hover:bg-amber-500/20 rounded-lg text-xs font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1"
              >
                <span v-if="isFetchingArt" class="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                {{ isFetchingArt ? 'Buscando…' : 'Find All Missing' }}
              </button>
            </div>
            
            <!-- Fix Common Issues (full width) -->
            <div class="fix-tool-card col-span-2 p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark hover:border-purple-500/50 transition-colors">
              <div class="flex items-start gap-3">
                <div class="h-10 w-10 rounded-xl bg-purple-500/10 text-purple-500 flex items-center justify-center shrink-0">
                  <span class="material-symbols-outlined text-[20px]">build</span>
                </div>
                <div class="flex-1">
                  <h5 class="font-medium text-gray-900 dark:text-white text-sm">Fix Common Issues</h5>
                  <p class="text-xs text-text-secondary mt-0.5">
                    Auto-correct over {{ fixTargets.length }} pista(s) ({{ selectedTracks.length > 0 ? 'selección' : 'lista filtrada' }})
                  </p>
                  <div class="mt-2 grid grid-cols-2 gap-2">
                    <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300 cursor-pointer">
                      <input type="checkbox" v-model="fixOptions.trackNumbering" class="w-3 h-3 rounded border-gray-300 text-purple-500 focus:ring-purple-500">
                      Fix track numbering (01, 02...)
                    </label>
                    <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300 cursor-pointer">
                      <input type="checkbox" v-model="fixOptions.capitalizeArtists" class="w-3 h-3 rounded border-gray-300 text-purple-500 focus:ring-purple-500">
                      Capitalize artist names
                    </label>
                    <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300 cursor-pointer">
                      <input type="checkbox" v-model="fixOptions.stripJunkTitles" class="w-3 h-3 rounded border-gray-300 text-purple-500 focus:ring-purple-500">
                      Remove "(Official Audio)"
                    </label>
                    <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300 cursor-pointer">
                      <input type="checkbox" v-model="fixOptions.standardizeFeat" class="w-3 h-3 rounded border-gray-300 text-purple-500 focus:ring-purple-500">
                      Standardize feat. vs ft.
                    </label>
                  </div>
                </div>
                <button @click="applyCommonFixes" :disabled="isFixingCommon || fixTargets.length === 0" class="px-4 py-2 bg-purple-500 hover:bg-purple-600 text-white rounded-lg text-xs font-medium transition-colors shrink-0 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5 self-start">
                  <span v-if="isFixingCommon" class="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                  {{ isFixingCommon ? 'Applying…' : 'Apply Fixes' }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
      
      <!-- No Selection State -->
      <div v-if="selectedTracks.length === 0" class="flex-1 flex flex-col items-center justify-center text-center p-8">
        <div class="h-20 w-20 rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center mb-4">
          <span class="material-symbols-outlined text-5xl text-text-secondary">edit_note</span>
        </div>
        <h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">Select a Track</h3>
        <p class="text-text-secondary max-w-md">Choose a track from the list to view and edit its metadata</p>
      </div>
      
      <!-- Batch Edit Mode -->
      <div v-else-if="selectedTracks.length > 1" class="flex-1 overflow-y-auto custom-scrollbar">
        <div class="p-6">
          <!-- Batch Header -->
          <div class="mb-6 p-4 bg-primary/5 border border-primary/20 rounded-xl">
            <div class="flex items-center gap-3">
              <div class="h-12 w-12 rounded-xl bg-primary/10 flex items-center justify-center">
                <span class="material-symbols-outlined text-primary text-2xl">library_music</span>
              </div>
              <div>
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Editing {{ selectedTracks.length }} tracks</h3>
                <p class="text-sm text-text-secondary">Changes will apply to all selected tracks</p>
              </div>
            </div>
          </div>
          
          <!-- Batch Fields -->
          <div class="editor-form space-y-6">
            <div class="form-section">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-gray-400">album</span>
                Common Fields
              </h4>
              <div class="space-y-4">
                <div class="flex items-start gap-3">
                  <input type="checkbox" v-model="batchFields.album" class="mt-2.5 w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                  <div class="flex-1">
                    <label class="block text-xs text-text-secondary mb-1">Album</label>
                    <input v-model="batchEditForm.album" type="text" placeholder="Album name..." class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                </div>
                <div class="flex items-start gap-3">
                  <input type="checkbox" v-model="batchFields.year" class="mt-2.5 w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                  <div class="flex-1">
                    <label class="block text-xs text-text-secondary mb-1">Year</label>
                    <input v-model="batchEditForm.year" type="number" placeholder="2024" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                </div>
                <div class="flex items-start gap-3">
                  <input type="checkbox" v-model="batchFields.genre" class="mt-2.5 w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                  <div class="flex-1">
                    <label class="block text-xs text-text-secondary mb-1">Genre</label>
                    <input v-model="batchEditForm.genre" type="text" placeholder="Rock, Pop..." class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                </div>
              </div>
            </div>
          </div>
          
          <!-- Batch Actions -->
          <div class="mt-8 flex items-center gap-3">
            <button 
              @click="saveBatchEdits"
              :disabled="isBatchSaving || !Object.values(batchFields).some(v => v)"
              class="flex-1 px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
            >
              <span v-if="isBatchSaving" class="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
              {{ isBatchSaving ? 'Saving...' : `Save to All ${selectedTracks.length} Tracks` }}
            </button>
            <button @click="clearSelection" class="px-4 py-2.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
              Cancel
            </button>
          </div>
        </div>
      </div>
      
      <!-- Single Track Editor -->
      <div v-else class="flex-1 overflow-y-auto custom-scrollbar">
        <div class="p-6">
          <!-- Album Art -->
          <div class="flex items-start gap-6 mb-6">
            <div class="album-art-picker relative group">
              <div class="w-48 h-48 rounded-xl bg-gray-200 dark:bg-surface-highlight overflow-hidden shadow-lg">
                <img v-if="currentTrack?.coverUrl" :src="currentTrack.coverUrl" :alt="currentTrack.album" class="w-full h-full object-cover">
                <div v-else class="w-full h-full bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-600 dark:to-gray-700 flex items-center justify-center">
                  <span class="material-symbols-outlined text-6xl text-gray-500 dark:text-gray-400">album</span>
                </div>
              </div>
              <button @click="showArtPicker = true" class="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity rounded-xl flex items-center justify-center">
                <span class="text-white text-sm font-medium">Change Art</span>
              </button>
            </div>
            
            <div class="flex-1">
              <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-1">{{ currentTrack?.title }}</h2>
              <p class="text-lg text-text-secondary mb-4">{{ currentTrack?.artist }}</p>
              
              <!-- Quick Stats -->
              <div class="flex items-center gap-4">
                <div :class="['flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium', getScoreBackground(currentTrack?.score || 0)]">
                  <span class="material-symbols-outlined text-[16px]">analytics</span>
                  {{ currentTrack?.score }}% complete
                </div>
                <span v-if="currentTrack?.issues" class="flex items-center gap-1 px-3 py-1.5 bg-error/10 text-error rounded-full text-sm font-medium">
                  <span class="material-symbols-outlined text-[16px]">warning</span>
                  {{ currentTrack?.issues }} issues
                </span>
              </div>
            </div>
          </div>
          
          <!-- Editor Form -->
          <div class="editor-form space-y-6">
            
            <!-- Basic Info -->
            <div class="form-section">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-gray-400">info</span>
                Basic Info
              </h4>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Title</label>
                  <input type="text" v-model="editForm.title" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Artist</label>
                  <input type="text" v-model="editForm.artist" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Album</label>
                  <input type="text" v-model="editForm.album" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Album Artist</label>
                  <input type="text" v-model="editForm.albumArtist" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Year</label>
                  <input type="number" v-model="editForm.year" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div class="flex gap-2">
                  <div class="flex-1">
                    <label class="block text-xs text-text-secondary mb-1">Track #</label>
                    <input type="text" v-model="editForm.trackNumber" placeholder="3 / 12" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                  <div class="flex-1">
                    <label class="block text-xs text-text-secondary mb-1">Disc #</label>
                    <input type="text" v-model="editForm.discNumber" placeholder="1 / 1" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                </div>
              </div>
            </div>
            
            <!-- Details -->
            <div class="form-section">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-gray-400">tune</span>
                Details
              </h4>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Genre</label>
                  <input type="text" v-model="editForm.genre" placeholder="Rock, Pop..." class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Subgenre</label>
                  <input type="text" v-model="editForm.subgenre" placeholder="Alternative, Indie..." class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Composer</label>
                  <input type="text" v-model="editForm.composer" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Label</label>
                  <input type="text" v-model="editForm.label" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Release Type</label>
                  <select v-model="editForm.releaseType" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                    <option value="album">Album</option>
                    <option value="single">Single</option>
                    <option value="ep">EP</option>
                    <option value="compilation">Compilation</option>
                    <option value="live">Live</option>
                  </select>
                </div>
                <div class="flex items-center gap-3">
                  <label class="flex items-center gap-2 cursor-pointer">
                    <input type="checkbox" v-model="editForm.explicit" class="w-4 h-4 rounded border-gray-300 text-error focus:ring-error">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Explicit Content</span>
                  </label>
                </div>
              </div>
            </div>
            
            <!-- Audio Analysis -->
            <div class="form-section">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-gray-400">equalizer</span>
                Audio Analysis
              </h4>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-xs text-text-secondary mb-1">BPM</label>
                  <input type="number" v-model="editForm.bpm" placeholder="—" step="0.1" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                </div>
                <div>
                  <label class="block text-xs text-text-secondary mb-1">Musical Key</label>
                  <select v-model="editForm.musicalKey" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                    <option value="">Unknown</option>
                    <option value="C">C Major</option>
                    <option value="Cm">C Minor</option>
                    <option value="C#">C# Major</option>
                    <option value="C#m">C# Minor</option>
                    <option value="D">D Major</option>
                    <option value="Dm">D Minor</option>
                    <option value="D#">D# Major</option>
                    <option value="D#m">D# Minor</option>
                    <option value="E">E Major</option>
                    <option value="Em">E Minor</option>
                    <option value="F">F Major</option>
                    <option value="Fm">F Minor</option>
                    <option value="F#">F# Major</option>
                    <option value="F#m">F# Minor</option>
                    <option value="G">G Major</option>
                    <option value="Gm">G Minor</option>
                    <option value="G#">G# Major</option>
                    <option value="G#m">G# Minor</option>
                    <option value="A">A Major</option>
                    <option value="Am">A Minor</option>
                    <option value="A#">A# Major</option>
                    <option value="A#m">A# Minor</option>
                    <option value="B">B Major</option>
                    <option value="Bm">B Minor</option>
                  </select>
                </div>
              </div>
              <!-- Audio Feature Meters (Read-only from Spotify enrichment) -->
              <div v-if="currentTrack?.energy || currentTrack?.danceability" class="mt-4 p-4 bg-gray-50 dark:bg-surface-highlight/50 rounded-xl">
                <div class="grid grid-cols-3 gap-4">
                  <div v-if="currentTrack?.energy">
                    <span class="text-xs text-text-secondary">Energy</span>
                    <div class="mt-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                      <div class="h-full bg-orange-500 rounded-full" :style="{ width: `${(currentTrack.energy || 0) * 100}%` }"></div>
                    </div>
                  </div>
                  <div v-if="currentTrack?.danceability">
                    <span class="text-xs text-text-secondary">Danceability</span>
                    <div class="mt-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                      <div class="h-full bg-purple-500 rounded-full" :style="{ width: `${(currentTrack.danceability || 0) * 100}%` }"></div>
                    </div>
                  </div>
                  <div v-if="currentTrack?.valence">
                    <span class="text-xs text-text-secondary">Mood (Valence)</span>
                    <div class="mt-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                      <div class="h-full bg-yellow-500 rounded-full" :style="{ width: `${(currentTrack.valence || 0) * 100}%` }"></div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- Identifiers -->
            <div class="form-section">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-gray-400">fingerprint</span>
                Identifiers
              </h4>
              <div class="space-y-4">
                <div class="flex gap-2">
                  <div class="flex-1">
                    <label class="block text-xs text-text-secondary mb-1">ISRC</label>
                    <input type="text" v-model="editForm.isrc" readonly class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-500 dark:text-gray-400 cursor-not-allowed">
                  </div>
                  <button @click="fetchFromMusicBrainz" :disabled="!currentTrack" class="self-end px-3 py-2 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-sm font-medium transition-colors whitespace-nowrap disabled:opacity-40">
                    Fetch from MusicBrainz
                  </button>
                </div>
                <div class="grid grid-cols-2 gap-4">
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">MusicBrainz Track ID</label>
                    <input type="text" v-model="editForm.mbTrackId" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white font-mono text-xs focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">MusicBrainz Release ID</label>
                    <input type="text" v-model="editForm.mbReleaseId" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white font-mono text-xs focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">UPC (Album Barcode)</label>
                    <input type="text" v-model="editForm.upc" placeholder="000000000000" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white font-mono text-xs focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">Copyright</label>
                    <input type="text" v-model="editForm.copyright" placeholder="© 2024 Label Name" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white text-xs focus:outline-none focus:ring-2 focus:ring-primary/50">
                  </div>
                </div>
              </div>
            </div>
            
            <!-- Provider Provenance & Availability -->
            <div class="form-section">
              <div class="flex items-center justify-between mb-3">
                <h4 class="text-sm font-semibold text-gray-900 dark:text-white flex items-center gap-2">
                  <span class="material-symbols-outlined text-[18px] text-gray-400">hub</span>
                  Provider Provenance &amp; Availability
                </h4>
                <button 
                  @click="checkCurrentTrackAvailability" 
                  :disabled="isCheckingAvailability"
                  class="px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors flex items-center gap-1.5 disabled:opacity-50"
                >
                  <span :class="['material-symbols-outlined text-[16px]', isCheckingAvailability ? 'animate-spin' : '']">
                    {{ isCheckingAvailability ? 'progress_activity' : 'verified' }}
                  </span>
                  Check Availability
                </button>
              </div>

              <div class="p-4 bg-gray-50 dark:bg-surface-highlight/50 rounded-xl space-y-4">
                <!-- Provenance Overview -->
                <div class="grid grid-cols-2 gap-4 pb-3 border-b border-gray-200 dark:border-border-dark">
                  <div>
                    <span class="text-xs text-text-secondary">Historical Import Source</span>
                    <p class="text-sm font-semibold text-gray-900 dark:text-white flex items-center gap-1.5 mt-0.5">
                      <span class="material-symbols-outlined text-[16px] text-blue-400">history</span>
                      {{ currentTrack?.importedFrom || 'Library Import' }}
                    </p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Effective Download Provider</span>
                    <p class="text-sm font-semibold text-gray-900 dark:text-white flex items-center gap-1.5 mt-0.5">
                      <span class="material-symbols-outlined text-[16px] text-green-400">download_done</span>
                      {{ currentTrack?.downloadedFrom || (currentTrack?.filePath ? 'Local File' : 'Not Downloaded') }}
                    </p>
                  </div>
                </div>

                <!-- Provider Availability Matrix -->
                <div>
                  <span class="text-xs text-text-secondary font-medium">Linked Provider Sources</span>
                  <div v-if="trackSources.length > 0" class="mt-2 space-y-2">
                    <div 
                      v-for="src in trackSources" 
                      :key="src.id" 
                      class="flex items-center justify-between p-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-xs"
                    >
                      <div class="flex items-center gap-2.5">
                        <span :class="['w-6 h-6 rounded-full flex items-center justify-center font-bold text-[10px]', getServiceBadgeClass(src.serviceName)]">
                          {{ src.serviceName.charAt(0).toUpperCase() }}
                        </span>
                        <div>
                          <p class="font-medium text-gray-900 dark:text-white capitalize">{{ src.serviceName }}</p>
                          <p class="text-gray-400 font-mono text-[10px]">ID: {{ src.serviceTrackId }}</p>
                        </div>
                      </div>

                      <div class="flex items-center gap-3">
                        <span v-if="src.format" class="text-gray-400 font-mono">
                          {{ src.format }} {{ src.bitDepth ? src.bitDepth + 'b' : '' }} {{ src.sampleRate ? (src.sampleRate / 1000) + 'kHz' : '' }}
                        </span>
                        <span :class="['px-2 py-0.5 rounded-full font-medium text-[10px] border', getAvailabilityBadgeClass(src.availabilityStatus)]" :title="src.availabilityReason || src.availabilityStatus">
                          {{ formatAvailabilityLabel(src.availabilityStatus) }}
                        </span>
                      </div>
                    </div>
                  </div>
                  <div v-else class="mt-2 text-xs text-gray-400 italic">
                    No linked streaming provider sources found for this track.
                  </div>
                </div>
              </div>
            </div>

            <!-- Audio Info (Read-only) -->
            <div class="form-section">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3 flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-gray-400">graphic_eq</span>
                Audio Info
              </h4>
              <div class="p-4 bg-gray-50 dark:bg-surface-highlight/50 rounded-xl">
                <div class="grid grid-cols-3 gap-4">
                  <div>
                    <span class="text-xs text-text-secondary">Format</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">{{ currentTrack?.quality?.split(' ')[0] || '—' }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Quality</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">{{ currentTrack?.quality || '—' }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Bitrate</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">{{ formatBitrate(currentTrack?.bitrate) }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Duration</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">{{ formatDuration(currentTrack?.durationMs) }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">File Size</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">{{ formatFileSize(currentTrack?.fileSize) }}</p>
                  </div>
                  <div>
                    <span class="text-xs text-text-secondary">Sample Rate</span>
                    <p class="text-sm font-medium text-gray-900 dark:text-white">{{ formatSampleRate(currentTrack?.sampleRate) }}</p>
                  </div>
                </div>
                <div v-if="currentTrack?.filePath" class="mt-4 pt-4 border-t border-gray-200 dark:border-border-dark">
                  <span class="text-xs text-text-secondary">File Path</span>
                  <div class="flex items-center gap-2 mt-1">
                    <p class="text-sm font-mono text-gray-700 dark:text-gray-300 truncate flex-1">{{ currentTrack.filePath }}</p>
                    <button @click="openInFolder" class="p-1.5 hover:bg-gray-200 dark:hover:bg-gray-700 rounded transition-colors" title="Open Folder">
                      <span class="material-symbols-outlined text-[18px] text-gray-500">folder_open</span>
                    </button>
                  </div>
                </div>
                <div v-else class="mt-4 pt-4 border-t border-gray-200 dark:border-border-dark text-center">
                  <span class="text-xs text-text-secondary italic">No local file (streaming only)</span>
                </div>
              </div>
            </div>
          </div>

          <!-- S191: File Tag Editor (FLAC facets written by the container) -->
          <div class="form-section mt-8 p-4 border border-gray-200 dark:border-border-dark rounded-xl">
            <div class="flex items-center justify-between mb-3">
              <h4 class="text-sm font-semibold text-gray-900 dark:text-white flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px] text-gray-400">sell</span>
                Tags del archivo
              </h4>
              <button @click="readTrackFileTags" :disabled="isReadingFileTags || !currentTrack" class="px-3 py-1.5 bg-blue-500/10 text-blue-500 hover:bg-blue-500/20 rounded-lg text-xs font-medium transition-colors disabled:opacity-40 flex items-center gap-1.5">
                <span v-if="isReadingFileTags" class="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                <span v-else class="material-symbols-outlined text-[14px]">visibility</span>
                {{ isReadingFileTags ? 'Leyendo…' : 'Leer tags del archivo' }}
              </button>
            </div>
            <p v-if="!fileTagsSnapshot && !fileTagsError" class="text-xs text-text-secondary">
              Lee TODAS las etiquetas reales del archivo descargado: facetas Vorbis completas en FLAC (editables con verificación roundtrip); en M4A/MP3 y otros formatos vía ffprobe (solo lectura).
            </p>
            <div v-if="fileTagsError" class="mb-3 px-3 py-2 rounded-lg bg-error/10 border border-error/30 text-error text-xs">{{ fileTagsError }}</div>

            <template v-if="fileTagsSnapshot">
              <p class="text-xs text-text-secondary mb-3 truncate" :title="fileTagsSnapshot.file_path">{{ fileTagsSnapshot.file_path }}</p>
              <!-- Editable facets -->
              <div class="grid grid-cols-2 gap-x-4 gap-y-3 mb-4">
                <label v-for="field in editableTagFields" :key="field.key" class="text-xs">
                  <span class="text-text-secondary block mb-1">{{ field.label }}</span>
                  <input v-model="editableFileTags[field.key]" type="text" class="w-full px-2.5 py-1.5 rounded-lg border border-gray-300 dark:border-border-dark bg-transparent text-gray-900 dark:text-white focus:ring-1 focus:ring-primary outline-none" />
                </label>
                <label class="text-xs">
                  <span class="text-text-secondary block mb-1">Track #</span>
                  <input v-model="editableFileTags.track_number" type="number" min="0" class="w-full px-2.5 py-1.5 rounded-lg border border-gray-300 dark:border-border-dark bg-transparent text-gray-900 dark:text-white focus:ring-1 focus:ring-primary outline-none" />
                </label>
                <label class="text-xs">
                  <span class="text-text-secondary block mb-1">BPM</span>
                  <input v-model="editableFileTags.bpm" type="number" min="0" class="w-full px-2.5 py-1.5 rounded-lg border border-gray-300 dark:border-border-dark bg-transparent text-gray-900 dark:text-white focus:ring-1 focus:ring-primary outline-none" />
                </label>
              </div>
              <button @click="writeTrackFileTags" :disabled="isWritingFileTags || !currentTrack" class="mb-4 px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-xs font-medium transition-colors disabled:opacity-50 flex items-center gap-2">
                <span v-if="isWritingFileTags" class="material-symbols-outlined text-[14px] animate-spin">progress_activity</span>
                {{ isWritingFileTags ? 'Escribiendo y verificando…' : 'Escribir en archivo (roundtrip)' }}
              </button>
              <div v-if="tagVerification" :class="['mb-3 px-3 py-2 rounded-lg text-xs', tagVerification.tags_match ? 'bg-success/10 border border-success/30 text-success' : 'bg-error/10 border border-error/30 text-error']">
                Roundtrip {{ tagVerification.tags_match ? 'verificado ✓' : 'FALLÓ ✗' }} · cover: {{ tagVerification.cover_present ? 'presente' : 'ausente' }} · lyrics: {{ tagVerification.unsynced_lyrics_present ? 'sí' : 'no' }}
              </div>
              <!-- Raw facet dump (all container-written keys, always visible — S200) -->
              <div class="text-xs">
                <p class="text-text-secondary select-none">Todas las facetas crudas ({{ Object.keys(fileTagsSnapshot.all_tags).length }} claves)</p>
                <div class="mt-2 max-h-64 overflow-y-auto custom-scrollbar space-y-1 font-mono border-t border-gray-200 dark:border-border-dark pt-2">
                  <div v-for="(values, key) in fileTagsSnapshot.all_tags" :key="key" class="flex gap-2">
                    <span class="text-purple-500 shrink-0 w-40 truncate" :title="String(key)">{{ key }}</span>
                    <span class="text-gray-700 dark:text-gray-300 break-all" :title="values.join('; ')">{{ values.join('; ') }}</span>
                  </div>
                </div>
              </div>
            </template>
          </div>

          <!-- Actions -->
          <div class="mt-8 flex items-center gap-3">
            <button 
              @click="saveTrackMetadata"
              :disabled="isSaving"
              class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors shadow-lg shadow-primary/20 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
            >
              <span v-if="isSaving" class="material-symbols-outlined text-[16px] animate-spin">progress_activity</span>
              {{ isSaving ? 'Saving...' : 'Save Changes' }}
            </button>
            <button @click="revertChanges" class="px-5 py-2.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
              Revert Changes
            </button>
            <button @click="showEditModal = true" class="px-5 py-2.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
              Edit in Modal
            </button>
            <button @click="openComparison" :disabled="!currentTrack" class="px-5 py-2.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors disabled:opacity-40">
              Compare Sources
            </button>
            <div class="relative ml-auto">
              <button @click="showAutoFix = !showAutoFix" class="flex items-center gap-2 px-4 py-2.5 bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 rounded-lg text-sm font-medium transition-colors">
                <span class="material-symbols-outlined text-[18px]">auto_fix_high</span>
                Auto-fix
                <span class="material-symbols-outlined text-[16px]">expand_more</span>
              </button>
              
              <!-- Auto-fix Dropdown -->
              <Transition name="fade">
                <div v-if="showAutoFix" class="absolute right-0 top-full mt-2 w-56 bg-white dark:bg-surface-dark rounded-xl shadow-xl border border-gray-200 dark:border-border-dark overflow-hidden z-10">
                  <button @click="fetchFromMusicBrainz(); showAutoFix = false" class="w-full px-4 py-3 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-3">
                    <span class="material-symbols-outlined text-[18px] text-purple-500">search</span>
                    Fetch from MusicBrainz
                  </button>
                  <button class="w-full px-4 py-3 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-3 opacity-50 cursor-not-allowed" disabled>
                    <span class="material-symbols-outlined text-[18px] text-red-500">music_note</span>
                    Fetch from Last.fm (coming soon)
                  </button>
                  <button @click="identifyWithAcoustID(); showAutoFix = false" :disabled="isIdentifying || !currentTrack?.filePath" class="w-full px-4 py-3 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-3 disabled:opacity-50 disabled:cursor-not-allowed">
                    <span :class="['material-symbols-outlined text-[18px] text-blue-500', isIdentifying && 'animate-spin']">{{ isIdentifying ? 'progress_activity' : 'fingerprint' }}</span>
                    {{ isIdentifying ? 'Identifying...' : 'Identify with AcoustID' }}
                  </button>
                  <button class="w-full px-4 py-3 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-3 opacity-50 cursor-not-allowed" disabled>
                    <span class="material-symbols-outlined text-[18px] text-green-500">image</span>
                    Fetch Album Art (coming soon)
                  </button>
                </div>
              </Transition>
            </div>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Album Art Picker Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showArtPicker" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showArtPicker = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[80vh] overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Choose Album Art</h3>
              <button @click="showArtPicker = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6">
              <!-- Search -->
              <div class="relative mb-6">
                <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-[18px]">search</span>
                <input type="text" placeholder="Search for album art..." class="w-full pl-10 pr-4 py-3 bg-gray-100 dark:bg-surface-highlight border-0 rounded-xl text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50">
              </div>
              
              <!-- Art Grid -->
              <div class="grid grid-cols-4 gap-4 mb-6">
                <button v-for="i in 8" :key="i" class="aspect-square rounded-xl bg-gray-200 dark:bg-surface-highlight overflow-hidden hover:ring-2 hover:ring-primary transition-all">
                  <div class="w-full h-full bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-600 dark:to-gray-700 flex items-center justify-center">
                    <span class="material-symbols-outlined text-3xl text-gray-500">album</span>
                  </div>
                </button>
              </div>
              
              <!-- Upload -->
              <button class="w-full py-4 border-2 border-dashed border-gray-300 dark:border-border-dark rounded-xl text-gray-500 dark:text-gray-400 hover:border-primary hover:text-primary transition-all flex items-center justify-center gap-2">
                <span class="material-symbols-outlined">upload</span>
                Upload Custom Image
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Quality Report Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showQualityReport" class="quality-report fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showQualityReport = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[85vh] overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Metadata Quality Report</h3>
              <button @click="showQualityReport = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 overflow-y-auto custom-scrollbar max-h-[calc(85vh-140px)]">
              <!-- Overall Score -->
              <div class="flex items-center gap-6 mb-8">
                <div class="relative w-28 h-28">
                  <svg class="w-full h-full transform -rotate-90" viewBox="0 0 100 100">
                    <circle cx="50" cy="50" r="45" stroke="currentColor" stroke-width="8" fill="none" class="text-gray-200 dark:text-gray-700" />
                    <circle cx="50" cy="50" r="45" stroke="currentColor" stroke-width="8" fill="none" 
                      :class="qualityReportData.averageScore >= 80 ? 'text-success' : qualityReportData.averageScore >= 60 ? 'text-amber-500' : 'text-error'" 
                      stroke-dasharray="283"
                      :stroke-dashoffset="283 - (283 * qualityReportData.averageScore / 100)"
                      stroke-linecap="round"
                    />
                  </svg>
                  <div class="absolute inset-0 flex items-center justify-center">
                    <span :class="['text-3xl font-bold', qualityReportData.averageScore >= 80 ? 'text-success' : qualityReportData.averageScore >= 60 ? 'text-amber-500' : 'text-error']">{{ qualityReportData.averageScore }}%</span>
                  </div>
                </div>
                <div>
                  <h4 class="text-xl font-semibold text-gray-900 dark:text-white">Overall Quality</h4>
                  <p class="text-text-secondary mt-1">{{ qualityReportData.averageScore >= 80 ? 'Your library metadata is in great shape!' : qualityReportData.averageScore >= 60 ? 'Your library has some metadata gaps that can be improved' : 'Your library needs metadata attention' }}</p>
                  <div class="flex items-center gap-4 mt-3">
                    <span class="flex items-center gap-1 text-sm text-success"><span class="w-2 h-2 rounded-full bg-success"></span> {{ qualityReportData.complete }} complete</span>
                    <span class="flex items-center gap-1 text-sm text-amber-500"><span class="w-2 h-2 rounded-full bg-amber-500"></span> {{ qualityReportData.partial }} partial</span>
                    <span class="flex items-center gap-1 text-sm text-error"><span class="w-2 h-2 rounded-full bg-error"></span> {{ qualityReportData.poor }} poor</span>
                  </div>
                </div>
              </div>
              
              <!-- Issues Breakdown -->
              <div class="mb-8">
                <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Issues Found</h5>
                <div class="space-y-2">
                  <button @click="setQualityFilter('no-art')" class="w-full flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight/80 transition-colors">
                    <div class="flex items-center gap-3">
                      <span class="material-symbols-outlined text-amber-500 text-[20px]">image</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300">Missing album art</span>
                    </div>
                    <span class="px-2 py-0.5 bg-amber-500/10 text-amber-500 text-xs font-medium rounded">{{ qualityReportData.missingArt }} tracks</span>
                  </button>
                  <button @click="setQualityFilter('no-isrc')" class="w-full flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight/80 transition-colors">
                    <div class="flex items-center gap-3">
                      <span class="material-symbols-outlined text-blue-500 text-[20px]">qr_code</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300">No ISRC</span>
                    </div>
                    <span class="px-2 py-0.5 bg-blue-500/10 text-blue-500 text-xs font-medium rounded">{{ qualityReportData.missingIsrc }} tracks</span>
                  </button>
                  <button @click="setQualityFilter('no-genre')" class="w-full flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight/80 transition-colors">
                    <div class="flex items-center gap-3">
                      <span class="material-symbols-outlined text-purple-500 text-[20px]">sell</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300">No genre tags</span>
                    </div>
                    <span class="px-2 py-0.5 bg-purple-500/10 text-purple-500 text-xs font-medium rounded">{{ qualityReportData.missingGenre }} tracks</span>
                  </button>
                  <button @click="setQualityFilter('no-year')" class="w-full flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight/80 transition-colors">
                    <div class="flex items-center gap-3">
                      <span class="material-symbols-outlined text-green-500 text-[20px]">calendar_month</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300">Missing year</span>
                    </div>
                    <span class="px-2 py-0.5 bg-green-500/10 text-green-500 text-xs font-medium rounded">{{ qualityReportData.missingYear }} tracks</span>
                  </button>
                  <button @click="setQualityFilter('no-mb')" class="w-full flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight/80 transition-colors">
                    <div class="flex items-center gap-3">
                      <span class="material-symbols-outlined text-red-500 text-[20px]">database</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300">Missing MusicBrainz IDs</span>
                    </div>
                    <span class="px-2 py-0.5 bg-red-500/10 text-red-500 text-xs font-medium rounded">{{ qualityReportData.missingMbId }} tracks</span>
                  </button>
                </div>
              </div>
              
              <!-- Recommendations -->
              <div class="mb-6">
                <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Recommendations</h5>
                <div class="space-y-3">
                  <div class="flex items-center gap-3 p-3 bg-blue-500/5 border border-blue-500/20 rounded-lg">
                    <span class="material-symbols-outlined text-blue-500">database</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Run MusicBrainz lookup for {{ tracksWithIsrcNoMb }} tracks with ISRC</span>
                    <button @click="runMusicBrainzEnrichment(); showQualityReport = false" :disabled="isEnriching || tracksWithIsrcNoMb === 0" class="px-3 py-1.5 bg-blue-500 text-white rounded-lg text-xs font-medium hover:bg-blue-600 transition-colors disabled:opacity-40">Run</button>
                  </div>
                  <div class="flex items-center gap-3 p-3 bg-green-500/5 border border-green-500/20 rounded-lg">
                    <span class="material-symbols-outlined text-green-500">fingerprint</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Use AcoustID for {{ unidentifiedWithFiles }} unidentified track(s)</span>
                    <button @click="batchIdentifyAcoustID(); showQualityReport = false" :disabled="isBatchIdentifying || unidentifiedWithFiles === 0" class="px-3 py-1.5 bg-green-500 text-white rounded-lg text-xs font-medium hover:bg-green-600 transition-colors disabled:opacity-40">Run</button>
                  </div>
                  <div class="flex items-center gap-3 p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg">
                    <span class="material-symbols-outlined text-amber-500">image</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Fetch album art for {{ tracksWithoutArt }} track(s)</span>
                    <button @click="fetchMissingArtwork(); showQualityReport = false" :disabled="isFetchingArt || tracksWithoutArt === 0" class="px-3 py-1.5 bg-amber-500 text-white rounded-lg text-xs font-medium hover:bg-amber-600 transition-colors disabled:opacity-40">Run</button>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- Footer -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end">
              <button 
                @click="runMusicBrainzEnrichment(); showQualityReport = false"
                :disabled="isEnriching"
                class="px-6 py-2.5 bg-purple-500 hover:bg-purple-600 text-white rounded-lg font-medium transition-colors flex items-center gap-2 disabled:opacity-50"
              >
                <span :class="['material-symbols-outlined text-[18px]', isEnriching && 'animate-spin']">{{ isEnriching ? 'progress_activity' : 'auto_fix_high' }}</span>
                {{ isEnriching ? 'Enriching...' : 'Auto-Fix All Issues' }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Comparison Modal: file tags vs database -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showComparison" class="comparison-modal fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showComparison = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-3xl max-h-[85vh] overflow-hidden shadow-2xl flex flex-col">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
              <div>
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Compare Sources</h3>
                <p v-if="currentTrack" class="text-xs text-text-secondary truncate max-w-[420px]">{{ currentTrack.title }} — {{ currentTrack.artist }}</p>
              </div>
              <button @click="showComparison = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="overflow-auto custom-scrollbar flex-1">
              <!-- Loading -->
              <div v-if="isComparingSources" class="py-14 text-center text-sm text-text-secondary flex flex-col items-center gap-2">
                <span class="material-symbols-outlined text-3xl text-primary animate-spin">progress_activity</span>
                Leyendo tags del archivo…
              </div>

              <!-- Error -->
              <div v-else-if="comparisonError" class="m-6 px-4 py-3 rounded-lg bg-error/10 border border-error/30 text-error text-sm">
                {{ comparisonError }}
              </div>

              <!-- Diff table -->
              <table v-else-if="comparisonRows.length > 0" class="w-full text-sm">
                <thead class="bg-gray-50 dark:bg-surface-highlight sticky top-0">
                  <tr>
                    <th class="px-4 py-3 text-left font-medium text-text-secondary">Field</th>
                    <th class="px-4 py-3 text-left font-medium text-text-secondary">Database (editable)</th>
                    <th class="px-4 py-3 text-left font-medium text-text-secondary">File Tags (FLAC)</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-gray-200 dark:divide-border-dark">
                  <tr
                    v-for="row in comparisonRows"
                    :key="row.field"
                    :class="['hover:bg-gray-50 dark:hover:bg-surface-highlight cursor-pointer', row.differs ? 'bg-amber-500/5' : '']"
                    @click="adoptComparisonValue(row)"
                  >
                    <td class="px-4 py-3 font-medium text-gray-900 dark:text-white">{{ row.field }}</td>
                    <td :class="['px-4 py-3', row.differs ? 'text-gray-900 dark:text-white font-medium' : 'text-gray-600 dark:text-gray-400']">
                      {{ row.dbDisplay || '—' }}
                      <span v-if="row.adoptedFromFile" class="ml-2 px-1.5 py-0.5 bg-primary/10 text-primary text-[10px] rounded">desde archivo ✓</span>
                    </td>
                    <td :class="['px-4 py-3', row.differs ? 'text-amber-600 dark:text-amber-400 font-medium' : 'text-gray-600 dark:text-gray-400']">
                      {{ row.fileDisplay || '—' }}
                    </td>
                  </tr>
                </tbody>
              </table>

              <p v-else class="py-12 text-center text-sm text-text-secondary italic">
                Selecciona una pista para comparar sus fuentes.
              </p>
            </div>
            
            <!-- Footer -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-between items-center gap-3 shrink-0">
              <p class="text-xs text-text-secondary">Clic en una fila para adoptar el valor del archivo en el formulario · las filas resaltadas difieren</p>
              <button @click="showComparison = false; notifyComparisonHint()" class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium transition-colors shrink-0">
                Listo
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Toast Notifications -->
    <Teleport to="body">
      <div class="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
        <TransitionGroup name="toast">
          <div 
            v-for="toast in toasts" 
            :key="toast.id"
            :class="['flex items-center gap-3 px-4 py-3 rounded-lg shadow-lg min-w-[280px]', getToastClasses(toast.type)]"
          >
            <span class="material-symbols-outlined text-[20px]">{{ getToastIcon(toast.type) }}</span>
            <span class="text-sm font-medium flex-1">{{ toast.message }}</span>
            <button @click="removeToast(toast.id)" class="p-1 hover:bg-white/10 rounded transition-colors">
              <span class="material-symbols-outlined text-[16px]">close</span>
            </button>
          </div>
        </TransitionGroup>
      </div>
    </Teleport>


    <!-- Metadata Edit Modal -->
    <MetadataEditModal 
      v-model="showEditModal"
      :track="currentTrack"
      @saved="onTrackSaved"
    />

    <!-- MusicBrainz Match Modal -->
    <MusicBrainzMatchModal
      v-model="showMatchModal"
      :track="currentTrack"
      @saved="onTrackSaved"
    />

    <!-- S158: Tidal Repair Dry-Run Review Modal -->
    <TidalRepairReviewModal
      v-model="showTidalRepairModal"
    />

    <!-- S163: Applied Repairs History Modal -->
    <RepairHistoryModal
      v-model="showRepairHistoryModal"
    />
    <!-- Context Menu -->
    <Teleport to="body">
      <div v-if="contextMenu.visible" class="fixed inset-0 z-[100]" @click="closeContextMenu" @contextmenu.prevent="closeContextMenu">
        <div 
          class="absolute bg-white dark:bg-surface-dark rounded-lg shadow-xl border border-gray-200 dark:border-border-dark py-1 min-w-[180px]"
          :style="{ top: `${contextMenu.y}px`, left: `${contextMenu.x - 180}px` }" 
        >
          <div class="px-3 py-2 border-b border-gray-200 dark:border-border-dark mb-1">
            <p class="text-xs font-semibold text-gray-900 dark:text-white truncate max-w-[160px]">{{ contextMenu.track?.title }}</p>
            <p class="text-[10px] text-text-secondary truncate max-w-[160px]">{{ contextMenu.track?.artist }}</p>
          </div>
          
          <button @click="fetchFromMusicBrainz" class="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px] text-purple-500">search</span>
            Fetch from MusicBrainz
          </button>
          
          <button @click="showEditModal = true; contextMenu.visible = false" class="w-full px-4 py-2 text-left text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px]">edit</span>
            Edit in Modal
          </button>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted, watch } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { useRoute } from 'vue-router'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { libraryApi } from '@/api/library'
import { metadataApi } from '@/api/metadata'
import { settingsApi } from '@/api/settings'
import { toolsApi } from '@/api/tools'
import { TauriEvents } from '@/api/tauri'
import type { LibraryTrack, TrackSourceAvailability } from '@/api/types'
import MetadataEditModal from '@/components/MetadataEditModal.vue'
import MusicBrainzMatchModal from '@/components/MusicBrainzMatchModal.vue'
import TidalRepairReviewModal from '@/components/TidalRepairReviewModal.vue'
import RepairHistoryModal from '@/components/RepairHistoryModal.vue'

const route = useRoute()
const showTidalRepairModal = ref(false)
const showRepairHistoryModal = ref(false)

// ==============================================
// TYPES
// ==============================================

interface MetadataTrack {
  id: number
  title: string
  artist: string
  album: string
  albumArtist: string
  year: number | null
  trackNumber: number | null
  discNumber: number | null
  genre: string
  subgenre: string | null
  isrc: string | null
  musicbrainzId: string | null
  coverUrl: string | null
  quality: string
  score: number
  issues: number
  downloadStatus: string
  durationMs: number | null
  filePath: string | null
  importedFrom: string | null
  downloadedFrom: string | null
  availableServices: string[]
  availabilitySummary: string | null
  explicit: boolean
  bpm: number | null
  musicalKey: string | null
  energy: number | null
  danceability: number | null
  valence: number | null
  upc: string | null
  copyright: string | null
  displayTitle?: string | null
  sourceTitle?: string | null
  fileDisambiguator?: string | null
  // Audio file info
  bitrate: number | null
  sampleRate: number | null
  bitDepth: number | null
  fileFormat: string | null
  fileSize: number | null
}

interface MetadataStats {
  total_tracks: number
  with_isrc: number
  with_musicbrainz_id: number
  with_album: number
  with_art: number
  with_year: number
  with_genre: number
  average_completeness: number
  missing_art: number
  missing_year: number
  missing_genre: number
}

// ==============================================
// STATE
// ==============================================

// Loading state
const isLoading = ref(true)
const isSaving = ref(false)
const isEnriching = ref(false)
let unlistenEnrichment: UnlistenFn | null = null
const enrichProgress = ref<{ current: number; total: number; currentTrack: string } | null>(null)

// Background enrichment status
interface BackgroundEnrichmentStatus {
  type: 'musicbrainz' | 'spotify' | 'lastfm' | 'idle'
  status: 'running' | 'completed' | 'error' | 'waiting'
  pending?: number
  enriched?: number
  processed?: number
  message: string
  nextRunIn?: number
}
const backgroundEnrichment = ref<BackgroundEnrichmentStatus | null>(null)

// Search and Filter
const searchQuery = ref('')
const filterType = ref('all')
const sortBy = ref('score')

// Selection
const selectedTracks = ref<number[]>([])

// UI State - Modals
const showQualityReport = ref(false)
const showAutoFixPanel = ref(false)
const showComparison = ref(false)
const showAutoFix = ref(false)
const showArtPicker = ref(false)
const showEditModal = ref(false)
const showMatchModal = ref(false)

// Context Menu
const contextMenu = reactive({
  visible: false,
  x: 0,
  y: 0,
  track: null as MetadataTrack | null
})

// Tracks from library
const tracks = ref<MetadataTrack[]>([])
const totalTracks = ref(0)

// Metadata stats for quality report
const metadataStats = ref<MetadataStats | null>(null)

// Quality scoring weights (Sprint 15)
const scoreWeights = ref({
  weight_album: 1, weight_isrc: 1, weight_mb_id: 1,
  weight_cover: 1, weight_year: 1, weight_genre: 1,
})

// ==============================================
// DATA LOADING
// ==============================================

// Helper: calculate issues from track data
function calculateIssues(track: LibraryTrack): number {
  let issues = 0
  if (!track.album_name) issues++
  if (!track.isrc) issues++
  if (!track.musicbrainz_id) issues++
  if (!track.cover_art_url) issues++
  if (!track.release_year) issues++
  if (!track.genre) issues++
  return issues
}

// Helper: map LibraryTrack to MetadataTrack
function mapToMetadataTrack(item: LibraryTrack): MetadataTrack {
  const score = item.metadata_score ?? 0
  return {
    id: item.id,
    title: item.display_title || item.title,
    artist: item.artist_name || 'Unknown Artist',
    album: item.album_name || '',
    albumArtist: item.artist_name || '',
    displayTitle: item.display_title || null,
    sourceTitle: item.source_title || item.title,
    fileDisambiguator: item.file_disambiguator || null,
    year: item.release_year,
    trackNumber: item.track_number,
    discNumber: item.disc_number,
    genre: item.genre || '',
    subgenre: null,
    isrc: item.isrc,
    musicbrainzId: item.musicbrainz_id,
    coverUrl: item.cover_art_url,
    importedFrom: item.imported_from || null,
    downloadedFrom: item.downloaded_from || null,
    availableServices: item.available_services ? item.available_services.split(',').map(s => s.trim()).filter(Boolean) : [],
    availabilitySummary: item.availability_summary || null,
    quality: item.quality || '—',
    score: score,
    issues: calculateIssues(item),
    downloadStatus: item.download_status || 'not_downloaded',
    durationMs: item.duration_ms,
    filePath: item.file_path,
    explicit: item.explicit ?? false,
    bpm: item.bpm,
    musicalKey: item.musical_key,
    energy: null,
    danceability: null,
    valence: null,
    upc: null,
    copyright: null,
    // Audio file info
    bitrate: null,
    sampleRate: null,
    bitDepth: null,
    fileFormat: null,
    fileSize: null,
  }
}

// Load tracks from backend
async function loadTracks() {
  isLoading.value = true
  try {
    // If filtering by "missing", use specialized endpoint
    if (filterType.value === 'missing') {
       const rawTracks = await metadataApi.getTracksNeedingMetadata(500)
       tracks.value = rawTracks.map(mapToMetadataTrack)
       totalTracks.value = rawTracks.length // Estimate
    } else {
       // Otherwise load general library
       const page = await libraryApi.getLibrary(0, 500)
       tracks.value = page.tracks.map(mapToMetadataTrack)
       totalTracks.value = page.total
    }
  } catch (error) {
    console.error('Failed to load tracks:', error)
    showToast('Failed to load tracks', 'error')
  } finally {
    isLoading.value = false
  }
}

// Load metadata stats
async function loadMetadataStats() {
  try {
    const stats = await metadataApi.getMetadataStats()
    metadataStats.value = {
      total_tracks: stats.total_tracks,
      with_isrc: stats.with_isrc,
      with_musicbrainz_id: stats.with_musicbrainz_id,
      with_album: stats.with_album,
      with_art: stats.with_art,
      with_year: stats.with_year,
      with_genre: stats.with_genre,
      average_completeness: stats.average_completeness,
      // Calculate missing counts from totals
      missing_art: stats.total_tracks - stats.with_art,
      missing_year: stats.total_tracks - stats.with_year,
      missing_genre: stats.total_tracks - stats.with_genre,
    }
  } catch (error) {
    console.error('Failed to load metadata stats:', error)
  }
}

// Load scoring weights from preferences
async function loadScoreWeights() {
  try {
    const prefs = await settingsApi.getMetadataPreferences()
    scoreWeights.value = {
      weight_album: prefs.weight_album ?? 1,
      weight_isrc:  prefs.weight_isrc  ?? 1,
      weight_mb_id: prefs.weight_mb_id ?? 1,
      weight_cover: prefs.weight_cover ?? 1,
      weight_year:  prefs.weight_year  ?? 1,
      weight_genre: prefs.weight_genre  ?? 1,
    }
  } catch (error) {
    console.error('Failed to load score weights:', error)
  }
}

// Initialize on mount
onMounted(async () => {
  await Promise.all([loadTracks(), loadMetadataStats(), loadScoreWeights()])
  void loadLastfmKeyStatus()
  
  const filterParam = route.query.filter
  if (filterParam === 'needs_work') {
    filterType.value = 'needs_work'
  }

  const trackIdParam = route.query.trackId;
  if (trackIdParam) {
    const targetId = Number(trackIdParam);
    const found = tracks.value.find(t => t.id === targetId);
    if (found) selectTrack(found);
  }
  
  // Subscribe to background enrichment status events
  try {
    const { listen } = await import('@tauri-apps/api/event')
    const unlistenBg = await listen<BackgroundEnrichmentStatus>(TauriEvents.BACKGROUND_ENRICHMENT_STATUS, (event) => {
      backgroundEnrichment.value = event.payload

      // Auto-refresh tracks when enrichment completes
      if (event.payload.status === 'completed' && event.payload.enriched && event.payload.enriched > 0) {
        loadTracks()
      }
    })
    onUnmounted(() => unlistenBg())
  } catch (err) {
    console.warn('background-enrichment-status listener unavailable:', err)
  }
})

onUnmounted(() => {
  if (unlistenEnrichment) {
    unlistenEnrichment()
    unlistenEnrichment = null
  }
})

// ==============================================
// COMPUTED
// ==============================================

// Filtered tracks
const filteredTracks = computed(() => {
  let result = [...tracks.value]
  
  // Search filter
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    result = result.filter(t => 
      t.title.toLowerCase().includes(query) || 
      t.artist.toLowerCase().includes(query) ||
      t.album.toLowerCase().includes(query)
    )
  }
  
  // Type filter
  switch (filterType.value) {
    case 'downloaded':
      result = result.filter(t => t.downloadStatus === 'downloaded')
      break
    case 'needs_work':
      result = result.filter(t => t.score < 90 || t.issues > 0)
      break
    case 'low-quality':
      result = result.filter(t => t.score < 70)
      break
    case 'missing':
      result = result.filter(t => t.issues > 2)
      break
    case 'no-art':
      result = result.filter(t => !t.coverUrl)
      break
    case 'no-isrc':
      result = result.filter(t => !t.isrc)
      break
    case 'no-mb':
      result = result.filter(t => !t.musicbrainzId)
      break
    case 'no-genre':
      result = result.filter(t => !t.genre || t.genre.trim() === '')
      break
    case 'no-year':
      result = result.filter(t => !t.year)
      break
  }
  
  // Sort
  switch (sortBy.value) {
    case 'artist':
      result.sort((a, b) => a.artist.localeCompare(b.artist))
      break
    case 'album':
      result.sort((a, b) => a.album.localeCompare(b.album))
      break
    case 'score':
      result.sort((a, b) => b.score - a.score)
      break
    default:
      result.sort((a, b) => a.title.localeCompare(b.title))
  }
  
  return result
})

// Current track being edited
const currentTrack = computed(() => {
  if (selectedTracks.value.length === 1) {
    return tracks.value.find(t => t.id === selectedTracks.value[0])
  }
  return null
})

// ==============================================
// S191: File Tag Editor (FLAC roundtrip via syncify-flac-writer)
// ==============================================

interface TrackTagsSnapshot {
  track_id: number
  file_path: string
  file_format: string
  all_tags: Record<string, string[]>
  has_cover: boolean
  cover_mime?: string
}

interface TagVerification {
  file_exists: boolean
  flac_valid: boolean
  tags_match: boolean
  cover_present: boolean
  cover_size_bytes?: number
  cover_mime?: string
  lyrics_present: boolean
  synced_lyrics_present: boolean
  unsynced_lyrics_present: boolean
  bpm_present: boolean
}

const fileTagsSnapshot = ref<TrackTagsSnapshot | null>(null)
const fileTagsError = ref<string | null>(null)
const isReadingFileTags = ref(false)
const isWritingFileTags = ref(false)
const tagVerification = ref<TagVerification | null>(null)

// Human-curated facet subset; technical facets (replaygain, r128,
// musicbrainz ids) stay visible through the raw dump but are not edited here.
const editableTagFields = [
  { key: 'title', label: 'Título' },
  { key: 'artist', label: 'Artista' },
  { key: 'album', label: 'Álbum' },
  { key: 'album_artist', label: 'Album Artist' },
  { key: 'composer', label: 'Compositor' },
  { key: 'genre', label: 'Género' },
  { key: 'style', label: 'Style' },
  { key: 'mood', label: 'Mood' },
  { key: 'grouping', label: 'Grouping' },
  { key: 'language', label: 'Idioma' },
  { key: 'label', label: 'Sello' },
  { key: 'catalog_number', label: 'Catálogo' },
  { key: 'isrc', label: 'ISRC' },
  { key: 'release_year', label: 'Año' },
  { key: 'initial_key', label: 'Key' },
  { key: 'comment', label: 'Comentario' },
] as const

const editableFileTags = reactive<Record<string, string>>({})
function resetFileTagEditor() {
  fileTagsSnapshot.value = null
  fileTagsError.value = null
  tagVerification.value = null
  Object.keys(editableFileTags).forEach(k => delete editableFileTags[k])
}
watch(selectedTracks, () => resetFileTagEditor())

async function readTrackFileTags() {
  if (!currentTrack.value || isReadingFileTags.value) return
  isReadingFileTags.value = true
  fileTagsError.value = null
  tagVerification.value = null
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const snap = await invoke<TrackTagsSnapshot>('read_track_tags', { trackId: currentTrack.value.id })
    fileTagsSnapshot.value = snap
    // Prefill editable fields from the raw facets (first value wins).
    const first = (k: string) => (snap.all_tags[k] && snap.all_tags[k][0]) ?? ''
    for (const f of editableTagFields) {
      const keyMap: Record<string, string> = {
        album_artist: 'ALBUMARTIST', composer: 'COMPOSER', genre: 'GENRE',
        style: 'STYLE', mood: 'MOOD', grouping: 'GROUPING', language: 'LANGUAGE',
        label: 'LABEL', catalog_number: 'CATALOGNUMBER', isrc: 'ISRC',
        release_year: 'YEAR', initial_key: 'INITIALKEY', comment: 'COMMENT',
      }
      const rawKey = (keyMap as Record<string, string>)[f.key] ?? f.key.toUpperCase()
      editableFileTags[f.key] = first(rawKey)
    }
    editableFileTags['track_number'] = first('TRACKNUMBER')
    editableFileTags['bpm'] = first('BPM')
  } catch (err) {
    console.error('Failed to read file tags:', err)
    fileTagsError.value = err instanceof Error ? err.message : String(err)
  } finally {
    isReadingFileTags.value = false
  }
}

async function writeTrackFileTags() {
  if (!currentTrack.value || !fileTagsSnapshot.value || isWritingFileTags.value) return
  isWritingFileTags.value = true
  fileTagsError.value = null
  tagVerification.value = null
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const opt = (v: string | undefined): string | undefined => {
      const t = (v ?? '').trim()
      return t.length > 0 ? t : undefined
    }
    const num = (v: string | undefined): number | undefined => {
      const n = parseInt(v ?? '', 10)
      return Number.isFinite(n) && n > 0 ? n : undefined
    }
    const verification = await invoke<TagVerification>('write_track_tags', {
      trackId: currentTrack.value.id,
      metadata: {
        title: opt(editableFileTags['title']) ?? currentTrack.value.title,
        artist: opt(editableFileTags['artist']) ?? currentTrack.value.artist,
        album: opt(editableFileTags['album']) ?? currentTrack.value.album ?? '',
        album_artist: opt(editableFileTags['album_artist']),
        composer: opt(editableFileTags['composer']),
        genre: opt(editableFileTags['genre']),
        style: opt(editableFileTags['style']),
        mood: opt(editableFileTags['mood']),
        grouping: opt(editableFileTags['grouping']),
        language: opt(editableFileTags['language']),
        copyright: opt(editableFileTags['copyright']),
        label: opt(editableFileTags['label']),
        catalog_number: opt(editableFileTags['catalog_number']),
        isrc: opt(editableFileTags['isrc']),
        release_year: opt(editableFileTags['release_year']),
        comment: opt(editableFileTags['comment']),
        track_number: num(editableFileTags['track_number']),
        track_total: undefined,
        disc_number: undefined,
        disc_total: undefined,
        bpm: num(editableFileTags['bpm']),
        initial_key: opt(editableFileTags['initial_key']),
      }
    })
    tagVerification.value = verification
    // Re-read so the raw dump reflects what actually landed in the file.
    await readTrackFileTags()
    tagVerification.value = verification
  } catch (err) {
    console.error('Failed to write file tags:', err)
    fileTagsError.value = err instanceof Error ? err.message : String(err)
  } finally {
    isWritingFileTags.value = false
  }
}

/** Percentage helper for the completeness strip (0 when the library is empty). */
function statPct(part: number, total: number): number {
  if (!total) return 0
  return Math.round((part / total) * 100)
}

// Counts for auto-fix tools
const tracksWithIsrcNoMb = computed(() => {
  return tracks.value.filter(t => t.isrc && !t.musicbrainzId).length
})

const tracksWithoutArt = computed(() => {
  return tracks.value.filter(t => !t.coverUrl).length
})

const tracksWithoutGenre = computed(() => {
  return tracks.value.filter(t => !t.genre || t.genre.trim() === '').length
})

// Tracks that can actually be fingerprinted: no MBID and a local file
const unidentifiedWithFiles = computed(() => {
  return tracks.value.filter(t => !t.musicbrainzId && !!t.filePath).length
})

// ==============================================
// AUTO-FIX TOOL IMPLEMENTATIONS
// ==============================================

// ---- AcoustID batch identification ----
const isBatchIdentifying = ref(false)
const batchIdentifyProgress = ref({ done: 0, total: 0 })

async function batchIdentifyAcoustID() {
  if (isBatchIdentifying.value) return
  const selectedSet = new Set(selectedTracks.value)
  const targets = tracks.value.filter(
    t => (!selectedTracks.value.length || selectedSet.has(t.id)) && !t.musicbrainzId && !!t.filePath
  ).slice(0, 25)
  if (targets.length === 0) {
    showToast('No hay pistas elegibles para huella acústica (requiere archivo local y sin MBID)', 'warning')
    return
  }

  isBatchIdentifying.value = true
  batchIdentifyProgress.value = { done: 0, total: targets.length }
  let identified = 0
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    for (const track of targets) {
      try {
        const result = await invoke<{ success: boolean; data?: { recordings?: Array<{ id: string; title?: string; artist?: string }> } }>('identify_audio', {
          filePath: track.filePath,
        })
        const match = result?.data?.recordings?.[0]
        if (result.success && match?.id) {
          await metadataApi.updateTrackMetadata(track.id, {
            mbTrackId: match.id,
            ...(match.title ? { title: match.title } : {}),
          })
          identified++
        }
      } catch (err) {
        console.warn(`AcoustID failed for track ${track.id}:`, err)
      }
      batchIdentifyProgress.value.done++
    }
    if (identified > 0) await loadTracks()
    showToast(`AcoustID: ${identified}/${targets.length} pista(s) identificadas`, identified > 0 ? 'success' : 'info')
  } finally {
    isBatchIdentifying.value = false
    batchIdentifyProgress.value = { done: 0, total: 0 }
  }
}

// ---- Last.fm genre enrichment (backend fills empty genres only) ----
const isLastfmRunning = ref(false)
async function runLastfmEnrichment() {
  if (isLastfmRunning.value) return
  isLastfmRunning.value = true
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const summary = await invoke<string>('enrich_genre_lastfm')
    showToast(summary || 'Enriquecimiento de géneros finalizado', 'info')
    await loadTracks()
  } catch (error) {
    console.error('Last.fm enrichment failed:', error)
    showToast(error instanceof Error ? error.message : String(error), 'error')
  } finally {
    isLastfmRunning.value = false
  }
}

// ---- S200: Last.fm API key management ----
const lastfmKeyInput = ref('')
const isSavingLastfmKey = ref(false)
const lastfmKeyStatus = ref<Awaited<ReturnType<typeof settingsApi.getLastfmApiKeyStatus>> | null>(null)
async function loadLastfmKeyStatus() {
  try {
    lastfmKeyStatus.value = await settingsApi.getLastfmApiKeyStatus()
  } catch (err) {
    console.warn('Failed to load lastfm key status:', err)
  }
}
async function saveLastfmKey() {
  const key = lastfmKeyInput.value.trim()
  if (!key || isSavingLastfmKey.value) return
  isSavingLastfmKey.value = true
  try {
    await settingsApi.setLastfmApiKey(key)
    lastfmKeyInput.value = ''
    await loadLastfmKeyStatus()
    showToast('API key de Last.fm guardada — ya puedes pedir géneros', 'success')
  } catch (err) {
    showToast(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isSavingLastfmKey.value = false
  }
}

// ---- Cover art backfill (MB → Cover Art Archive) ----
const isFetchingArt = ref(false)
async function fetchMissingArtwork() {
  if (isFetchingArt.value) return
  isFetchingArt.value = true
  try {
    const res = await metadataApi.fetchMissingCoverArt(100)
    if (res.updated > 0) await loadTracks()
    showToast(`Carátulas: ${res.updated} actualizadas, ${res.skipped} sin imagen disponible, ${res.failed} errores`, res.updated > 0 ? 'success' : 'info')
  } catch (error) {
    console.error('Cover art backfill failed:', error)
    showToast(error instanceof Error ? error.message : String(error), 'error')
  } finally {
    isFetchingArt.value = false
  }
}

// ---- Fix Common Issues (deterministic client-side transforms) ----
const fixOptions = reactive({
  trackNumbering: true,
  capitalizeArtists: true,
  stripJunkTitles: true,
  standardizeFeat: false,
})
const isFixingCommon = ref(false)

const fixTargets = computed(() => {
  if (selectedTracks.value.length > 0) {
    return tracks.value.filter(t => selectedTracks.value.includes(t.id))
  }
  return filteredTracks.value
})

function toTitleCase(s: string): string {
  return s.replace(/\w[\w'’]*/g, w => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase())
}

const JUNK_TITLE_RE = /\s*[\(\[](official|lyric[s]?\s*video|audio|video|music\s*video|hd|hq|4k|visualizer|explicit)[^\)\]]*[\)\]]/gi

/** Computes DB field updates for one track according to the enabled fixes. */
function computeFixUpdates(track: MetadataTrack): Partial<Parameters<typeof metadataApi.updateTrackMetadata>[1]> | null {
  const updates: Record<string, unknown> = {}

  if (fixOptions.trackNumbering && track.trackNumber !== null && (!Number.isInteger(track.trackNumber) || track.trackNumber < 1)) {
    const n = Math.max(1, Math.round(track.trackNumber))
    updates.trackNumber = n
  }

  if (fixOptions.capitalizeArtists) {
    const artist = track.artist.trim()
    const fixed = artist === artist.toUpperCase() || artist === artist.toLowerCase() ? toTitleCase(artist) : artist
    if (fixed && fixed !== artist) updates.artistName = fixed
  }

  let title = track.title
  let titleChanged = false
  if (fixOptions.stripJunkTitles) {
    const cleaned = title.replace(JUNK_TITLE_RE, '').trim()
    if (cleaned && cleaned !== title) {
      title = cleaned
      titleChanged = true
    }
  }
  if (fixOptions.standardizeFeat) {
    const standardized = title.replace(/\bft\.?\b/gi, 'feat.').replace(/\bfeat\b(?!\.)/gi, 'feat.')
    if (standardized !== title) {
      title = standardized
      titleChanged = true
    }
  }
  if (titleChanged) updates.title = title

  return Object.keys(updates).length > 0 ? (updates as Parameters<typeof metadataApi.updateTrackMetadata>[1]) : null
}

async function applyCommonFixes() {
  if (isFixingCommon.value || fixTargets.value.length === 0) return
  isFixingCommon.value = true
  let changed = 0
  let failed = 0
  try {
    for (const track of fixTargets.value) {
      const updates = computeFixUpdates(track)
      if (!updates) continue
      try {
        await metadataApi.updateTrackMetadata(track.id, updates)
        changed++
      } catch {
        failed++
      }
    }
    if (changed > 0) await loadTracks()
    showToast(`Common fixes: ${changed} pista(s) corregidas${failed ? `, ${failed} errores` : ''}`, changed > 0 ? 'success' : 'info')
  } finally {
    isFixingCommon.value = false
  }
}

// ---- Metadata JSON export of the selection ----
const isExportingMetadata = ref(false)
async function exportSelectedMetadata() {
  if (isExportingMetadata.value) return
  const ids = selectedTracks.value.length > 0 ? selectedTracks.value : filteredTracks.value.map(t => t.id)
  if (ids.length === 0) {
    showToast('No hay pistas que exportar', 'info')
    return
  }
  isExportingMetadata.value = true
  try {
    const items = tracks.value.filter(t => ids.includes(t.id)).map(t => ({
      id: t.id,
      title: t.title,
      artist: t.artist,
      album: t.album,
      albumArtist: t.albumArtist,
      year: t.year,
      trackNumber: t.trackNumber,
      discNumber: t.discNumber,
      genre: t.genre,
      isrc: t.isrc,
      musicbrainzId: t.musicbrainzId,
      bpm: t.bpm,
      musicalKey: t.musicalKey,
      explicit: t.explicit,
      quality: t.quality,
      score: t.score,
      filePath: t.filePath,
    }))
    const target = await saveDialog({
      defaultPath: 'metadata-export.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
    if (!target) return
    const bytes = await toolsApi.writeTextFile(target, JSON.stringify(items, null, 2))
    showToast(`${items.length} pista(s) exportadas (${bytes} bytes) → ${target}`, 'success')
  } catch (error) {
    console.error('Metadata export failed:', error)
    showToast(error instanceof Error ? error.message : String(error), 'error')
  } finally {
    isExportingMetadata.value = false
  }
}

// ---- Open containing folder ----
async function openInFolder() {
  if (!currentTrack.value) return
  try {
    await libraryApi.showInFolder(currentTrack.value.id)
  } catch (err) {
    console.error('Failed to reveal file:', err)
    showToast(err instanceof Error ? err.message : String(err), 'error')
  }
}

// ==============================================
// EDIT FORM
// ==============================================

// Edit form - reactive to selected track
const editForm = reactive({
  title: '',
  artist: '',
  album: '',
  albumArtist: '',
  year: null as number | null,
  trackNumber: '',
  discNumber: '',
  genre: '',
  subgenre: '',
  composer: '',
  label: '',
  releaseType: 'album',
  isrc: '',
  mbTrackId: '',
  mbReleaseId: '',
  explicit: false,
  bpm: null as number | null,
  musicalKey: '',
  upc: '',
  copyright: '',
})

// Track Sources & Availability
const trackSources = ref<TrackSourceAvailability[]>([])
const isCheckingAvailability = ref(false)

// Watch for track selection to populate form
watch(currentTrack, async (track) => {
  if (track) {
    editForm.title = track.title
    editForm.artist = track.artist
    editForm.album = track.album
    editForm.albumArtist = track.albumArtist
    editForm.year = track.year
    editForm.trackNumber = track.trackNumber?.toString() || ''
    editForm.discNumber = track.discNumber?.toString() || ''
    editForm.genre = track.genre
    editForm.subgenre = track.subgenre || ''
    editForm.isrc = track.isrc || ''
    editForm.mbTrackId = track.musicbrainzId || ''
    editForm.explicit = track.explicit
    editForm.bpm = track.bpm
    editForm.musicalKey = track.musicalKey || ''
    editForm.upc = track.upc || ''
    editForm.copyright = track.copyright || ''

    try {
      trackSources.value = await libraryApi.getTrackSourcesAvailability(track.id)
    } catch {
      trackSources.value = []
    }
  } else {
    trackSources.value = []
  }
})

async function checkCurrentTrackAvailability() {
  if (!currentTrack.value) return
  isCheckingAvailability.value = true
  try {
    const updated = await libraryApi.checkTrackAvailability(currentTrack.value.id)
    trackSources.value = updated
    showToast(`Updated availability for ${updated.length} provider source(s)`, 'success')
  } catch (err: any) {
    showToast(err?.message || String(err), 'error')
  } finally {
    isCheckingAvailability.value = false
  }
}

function getServiceBadgeClass(serviceName: string): string {
  const s = (serviceName || '').toLowerCase()
  if (s === 'qobuz') return 'bg-[#00283c] text-[#00b0ea]'
  if (s === 'tidal') return 'bg-black text-white'
  if (s === 'spotify') return 'bg-[#1ed760] text-black'
  if (s === 'deezer') return 'bg-purple-600 text-white'
  return 'bg-gray-600 text-white'
}

function getAvailabilityBadgeClass(status: string): string {
  switch (status) {
    case 'available':
      return 'bg-success/10 text-success border-success/30'
    case 'stale_404':
      return 'bg-error/10 text-error border-error/30'
    case 'region_unavailable':
      return 'bg-amber-500/10 text-amber-500 border-amber-500/30'
    case 'requires_auth':
      return 'bg-blue-500/10 text-blue-500 border-blue-500/30'
    default:
      return 'bg-gray-500/10 text-gray-400 border-gray-500/30'
  }
}

function formatAvailabilityLabel(status: string): string {
  switch (status) {
    case 'available':
      return 'Available'
    case 'stale_404':
      return 'Stale (404)'
    case 'region_unavailable':
      return 'Region Restricted'
    case 'requires_auth':
      return 'Auth Required'
    default:
      return 'Unchecked'
  }
}

// Watch for filter query changes
watch(() => route.query.filter, (newFilter) => {
  if (newFilter === 'needs_work') {
    filterType.value = 'needs_work'
  } else if (newFilter === 'all' || !newFilter) {
    filterType.value = 'all'
  }
})

// Batch fields
const batchFields = reactive({
  album: false,
  year: false,
  genre: false,
})

// ==============================================
// ACTIONS
// ==============================================

function selectTrack(track: MetadataTrack) {
  if (selectedTracks.value.includes(track.id)) {
    // Already selected, do nothing (or could toggle off)
  } else {
    selectedTracks.value = [track.id]
  }
}

function toggleTrackSelection(trackId: number) {
  const index = selectedTracks.value.indexOf(trackId)
  if (index === -1) {
    selectedTracks.value.push(trackId)
  } else {
    selectedTracks.value.splice(index, 1)
  }
}

function clearSelection() {
  selectedTracks.value = []
}

function selectAll() {
  selectedTracks.value = filteredTracks.value.map(t => t.id)
}

// Save single track metadata
async function saveTrackMetadata() {
  if (!currentTrack.value) return
  
  isSaving.value = true
  try {
    const updatedTrack = await metadataApi.updateTrackMetadata(currentTrack.value.id, {
      title: editForm.title,
      artistName: editForm.artist,
      albumName: editForm.album,
      trackNumber: editForm.trackNumber ? parseInt(editForm.trackNumber) : undefined,
      discNumber: editForm.discNumber ? parseInt(editForm.discNumber) : undefined,
      isrc: editForm.isrc || undefined,
      genre: editForm.genre || undefined,
      year: editForm.year || undefined,
      bpm: editForm.bpm || undefined,
      musicalKey: editForm.musicalKey || undefined,
      explicit: editForm.explicit,
      mbTrackId: editForm.mbTrackId || undefined,
      label: editForm.label || undefined,
    })
    
    // Refresh the track in our local state using the returned data
    onTrackSaved(updatedTrack)
    
    showToast('Saved metadata successfully', 'success')
  } catch (error) {
    console.error('Failed to save metadata:', error)
    showToast('Failed to save metadata', 'error')
  } finally {
    isSaving.value = false
  }
}

// Revert changes to form
function revertChanges() {
  if (currentTrack.value) {
    editForm.title = currentTrack.value.title
    editForm.artist = currentTrack.value.artist
    editForm.album = currentTrack.value.album
    editForm.albumArtist = currentTrack.value.albumArtist
    editForm.year = currentTrack.value.year
    editForm.trackNumber = currentTrack.value.trackNumber?.toString() || ''
    editForm.discNumber = currentTrack.value.discNumber?.toString() || ''
    editForm.genre = currentTrack.value.genre
    editForm.subgenre = currentTrack.value.subgenre || ''
    editForm.isrc = currentTrack.value.isrc || ''
    editForm.mbTrackId = currentTrack.value.musicbrainzId || ''
    editForm.explicit = currentTrack.value.explicit
    editForm.bpm = currentTrack.value.bpm
    editForm.musicalKey = currentTrack.value.musicalKey || ''
    editForm.upc = currentTrack.value.upc || ''
    editForm.copyright = currentTrack.value.copyright || ''
  }
}

// Handle save from modal
function onTrackSaved(updatedTrack: LibraryTrack) {
  // Update local track list
  const index = tracks.value.findIndex(t => t.id === updatedTrack.id)
  if (index !== -1) {
    // Merge updates into our MetadataTrack shape
    const metaTrack = mapToMetadataTrack(updatedTrack)
    tracks.value[index] = metaTrack
    
    // Also update editForm if it's the current track
    if (currentTrack.value?.id === updatedTrack.id) {
       revertChanges() // This will reload form from currentTrack
    }
  }
}

// ==============================================
// AUTO-FIX TOOLS
// ==============================================

// Run MusicBrainz enrichment on selected tracks (or all if none)
async function runMusicBrainzEnrichment() {
  isEnriching.value = true
  enrichProgress.value = { current: 0, total: 0, currentTrack: '' }
  
  try {
    // Listen for progress events
    const { listen } = await import('@tauri-apps/api/event')
    unlistenEnrichment = await listen<{ status: string; total: number; current: number; enriched: number; failed: number; currentTrack?: string; message?: string }>(TauriEvents.ENRICHMENT_PROGRESS, (event) => {
      enrichProgress.value = {
        current: event.payload.current,
        total: event.payload.total,
        currentTrack: event.payload.currentTrack ?? event.payload.message ?? ''
      }
    })

    const { invoke } = await import('@tauri-apps/api/core')
    let result: { total: number; enriched: number; failed: number }

    if (selectedTracks.value.length > 0) {
      // Selection-scoped: run the single-track enrichment for each chosen track.
      const ids = [...selectedTracks.value]
      result = { total: ids.length, enriched: 0, failed: 0 }
      let done = 0
      for (const trackId of ids) {
        done++
        try {
          await invoke('enrich_metadata', { trackId })
          result.enriched++
          enrichProgress.value = { current: done, total: ids.length, currentTrack: `Track ${trackId}` }
        } catch (err) {
          console.warn(`Enrichment failed for track ${trackId}:`, err)
          result.failed++
        }
      }
    } else {
      // Global sweep: backend batches ISRC → MBID lookups with progress events.
      const raw = await invoke<{ total?: number; enriched?: number; failed?: number }>('enrich_metadata_musicbrainz', {})
      result = {
        total: raw.total ?? raw.enriched ?? 0,
        enriched: raw.enriched ?? 0,
        failed: raw.failed ?? 0,
      }
    }

    console.log(`Enriched ${result.enriched}/${result.total} tracks (${result.failed} failed)`)
    
    // Show toast
    showToast(`Enriched ${result.enriched} tracks successfully`, 'success')
    
    // Reload tracks to show updated scores
    if (result.enriched > 0 || result.failed > 0) {
      await loadTracks()
    }
  } catch (error) {
    console.error('Failed to enrich metadata:', error)
    showToast('Failed to enrich metadata', 'error')
  } finally {
    if (unlistenEnrichment) {
      unlistenEnrichment()
      unlistenEnrichment = null
    }
    isEnriching.value = false
    enrichProgress.value = null
  }
}

// Single track MusicBrainz lookup - Open Modal
function fetchFromMusicBrainz() {
  const track = contextMenu.track || currentTrack.value
  if (!track) return
  
  // If triggered from context menu, set as current track so modal works
  if (contextMenu.track) {
    selectTrack(contextMenu.track)
  }
  
  showMatchModal.value = true
  contextMenu.visible = false
}

function openActionMenu(track: MetadataTrack, event: MouseEvent) {
  event.preventDefault()
  contextMenu.track = track
  contextMenu.x = event.clientX
  contextMenu.y = event.clientY
  contextMenu.visible = true
}

// Close context menu on click outside
function closeContextMenu() {
  contextMenu.visible = false
}

// AcoustID fingerprint identification
const isIdentifying = ref(false)
async function identifyWithAcoustID() {
  if (!currentTrack.value?.filePath) {
    showToast('No audio file path available for fingerprinting', 'warning')
    return
  }
  
  isIdentifying.value = true
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const result = await invoke<{ success: boolean; data?: { recordings?: Array<{ id: string; title: string; artist: string }> }; error?: string }>('identify_audio', { 
      filePath: currentTrack.value.filePath 
    })
    
    if (result.success && result.data?.recordings && result.data.recordings.length > 0) {
      const match = result.data.recordings[0]
      editForm.mbTrackId = match.id
      if (match.title) editForm.title = match.title
      if (match.artist) editForm.artist = match.artist
      showToast('Track identified via AcoustID!', 'success')
    } else {
      showToast('Could not identify track', 'warning')
    }
  } catch (error) {
    console.error('AcoustID identification failed:', error)
    showToast('Fingerprint identification failed', 'error')
  } finally {
    isIdentifying.value = false
  }
}

// ==============================================
// BATCH EDITING
// ==============================================

// Batch edit form
const batchEditForm = reactive({
  album: '',
  year: null as number | null,
  genre: '',
})

// Save batch edits
const isBatchSaving = ref(false)
async function saveBatchEdits() {
  if (selectedTracks.value.length < 2) return
  
  isBatchSaving.value = true
  let successCount = 0
  let failCount = 0
  
  try {
    for (const trackId of selectedTracks.value) {
      const updates: Record<string, unknown> = {}
      
      if (batchFields.album && batchEditForm.album) {
        updates.albumName = batchEditForm.album
      }
      if (batchFields.year && batchEditForm.year) {
        updates.year = batchEditForm.year
      }
      if (batchFields.genre && batchEditForm.genre) {
        updates.genre = batchEditForm.genre
      }
      
      if (Object.keys(updates).length > 0) {
        try {
          await metadataApi.updateTrackMetadata(trackId, updates as Parameters<typeof metadataApi.updateTrackMetadata>[1])
          successCount++
        } catch {
          failCount++
        }
      }
    }
    
    if (successCount > 0) {
      showToast(`Updated ${successCount} tracks`, 'success')
      await loadTracks()
    }
    if (failCount > 0) {
      showToast(`Failed to update ${failCount} tracks`, 'error')
    }
    
    // Clear selection after batch edit
    clearSelection()
  } catch (error) {
    console.error('Batch edit failed:', error)
    showToast('Batch edit failed', 'error')
  } finally {
    isBatchSaving.value = false
  }
}

// ==============================================
// TOAST NOTIFICATIONS
// ==============================================

interface Toast {
  id: number
  message: string
  type: 'success' | 'error' | 'warning' | 'info'
}

const toasts = ref<Toast[]>([])
let toastIdCounter = 0

function showToast(message: string, type: 'success' | 'error' | 'warning' | 'info' = 'info') {
  const id = ++toastIdCounter
  toasts.value.push({ id, message, type })
  
  // Auto-remove after 4 seconds
  setTimeout(() => {
    removeToast(id)
  }, 4000)
}

function removeToast(id: number) {
  const index = toasts.value.findIndex(t => t.id === id)
  if (index !== -1) {
    toasts.value.splice(index, 1)
  }
}

// ==============================================
// HELPERS
// ==============================================

function getScoreBackground(score: number): string {
  if (score >= 90) return 'bg-success/10 text-success'
  if (score >= 70) return 'bg-amber-500/10 text-amber-500'
  return 'bg-error/10 text-error'
}

// Format duration from milliseconds to mm:ss
function formatDuration(ms: number | null | undefined): string {
  if (!ms) return '—'
  const totalSeconds = Math.floor(ms / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}:${seconds.toString().padStart(2, '0')}`
}

// Format bitrate to kbps
function formatBitrate(bitrate: number | null | undefined): string {
  if (!bitrate) return '—'
  return `${bitrate.toLocaleString()} kbps`
}

// Format file size from bytes to human readable
function formatFileSize(bytes: number | null | undefined): string {
  if (!bytes) return '—'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

// Format sample rate to Hz
function formatSampleRate(sampleRate: number | null | undefined): string {
  if (!sampleRate) return '—'
  return `${sampleRate.toLocaleString()} Hz`
}

function getToastClasses(type: string): string {
  switch (type) {
    case 'success': return 'bg-success text-white'
    case 'error': return 'bg-error text-white'
    case 'warning': return 'bg-amber-500 text-white'
    default: return 'bg-primary text-white'
  }
}

function getToastIcon(type: string): string {
  switch (type) {
    case 'success': return 'check_circle'
    case 'error': return 'error'
    case 'warning': return 'warning'
    default: return 'info'
  }
}

// Background enrichment status helpers
function getEnrichmentIcon(status: BackgroundEnrichmentStatus): string {
  switch (status.type) {
    case 'musicbrainz': return 'database'
    case 'spotify': return 'equalizer'
    case 'lastfm': return 'music_note'
    case 'idle': return 'schedule'
    default: return 'sync'
  }
}

function getEnrichmentTitle(status: BackgroundEnrichmentStatus): string {
  switch (status.type) {
    case 'musicbrainz': return 'MusicBrainz Enrichment'
    case 'spotify': return 'Spotify Audio Features'
    case 'lastfm': return 'Last.fm Genre Tags'
    case 'idle': return 'Background Enrichment'
    default: return 'Enrichment'
  }
}

function getEnrichmentStatusColor(status: BackgroundEnrichmentStatus): string {
  switch (status.status) {
    case 'running': return 'text-blue-500 animate-pulse'
    case 'completed': return 'text-success'
    case 'error': return 'text-error'
    default: return 'text-gray-500'
  }
}

function getEnrichmentStatusBg(status: BackgroundEnrichmentStatus): string {
  switch (status.status) {
    case 'running': return 'bg-blue-500/5'
    case 'completed': return 'bg-success/5'
    case 'error': return 'bg-error/5'
    default: return 'bg-gray-500/5'
  }
}

// Quality report computed values
const qualityReportData = computed(() => {
  const complete = tracks.value.filter(t => t.score >= 90).length
  const partial = tracks.value.filter(t => t.score >= 60 && t.score < 90).length
  const poor = tracks.value.filter(t => t.score < 60).length
  const averageScore = tracks.value.length > 0 
    ? Math.round(tracks.value.reduce((sum, t) => sum + t.score, 0) / tracks.value.length)
    : 0
  
  return {
    complete,
    partial,
    poor,
    averageScore,
    missingArt: tracks.value.filter(t => !t.coverUrl).length,
    missingIsrc: tracks.value.filter(t => !t.isrc).length,
    missingAlbum: tracks.value.filter(t => !t.album).length,
    missingGenre: tracks.value.filter(t => !t.genre || t.genre.trim() === '').length,
    missingYear: tracks.value.filter(t => !t.year).length,
    missingMbId: tracks.value.filter(t => !t.musicbrainzId).length,
  }
})

/** Jumps from a quality-report issue row to the affected subset in the list. */
function setQualityFilter(kind: 'no-isrc' | 'no-mb' | 'no-genre' | 'no-year' | 'no-art') {
  filterType.value = kind
  searchQuery.value = ''
  showQualityReport.value = false
}

// ==============================================
// COMPARISON: FILE TAGS vs DATABASE
// ==============================================

interface ComparisonRow {
  field: string
  dbValue: string
  fileValue: string
  dbDisplay: string
  fileDisplay: string
  differs: boolean
  adoptedFromFile: boolean
}

const isComparingSources = ref(false)
const comparisonError = ref<string | null>(null)
const comparisonFileTags = ref<TrackTagsSnapshot | null>(null)
const adoptedFromFileFields = ref<Set<string>>(new Set())

const COMPARISON_FIELDS: Array<{ field: string; fileKey: string; dbKey: keyof typeof editForm }> = [
  { field: 'Title', fileKey: 'TITLE', dbKey: 'title' },
  { field: 'Artist', fileKey: 'ARTIST', dbKey: 'artist' },
  { field: 'Album', fileKey: 'ALBUM', dbKey: 'album' },
  { field: 'Album Artist', fileKey: 'ALBUMARTIST', dbKey: 'albumArtist' },
  { field: 'Genre', fileKey: 'GENRE', dbKey: 'genre' },
  { field: 'Composer', fileKey: 'COMPOSER', dbKey: 'composer' },
  { field: 'Label', fileKey: 'LABEL', dbKey: 'label' },
  { field: 'Year', fileKey: 'YEAR', dbKey: 'year' },
  { field: 'Track #', fileKey: 'TRACKNUMBER', dbKey: 'trackNumber' },
  { field: 'ISRC', fileKey: 'ISRC', dbKey: 'isrc' },
  { field: 'BPM', fileKey: 'BPM', dbKey: 'bpm' },
]

function firstTag(key: string): string {
  const values = comparisonFileTags.value?.all_tags[key]
  return (values && values[0]) ? String(values[0]).trim() : ''
}

const comparisonRows = computed<ComparisonRow[]>(() => {
  if (!comparisonFileTags.value || !currentTrack.value) return []
  return COMPARISON_FIELDS.map(({ field, fileKey, dbKey }) => {
    const rawDb = editForm[dbKey]
    const dbValue = rawDb === null || rawDb === undefined ? '' : String(rawDb).trim()
    const fileValue = firstTag(fileKey)
    // Numeric fields compare by numeric value so "07" vs "7" is not a diff.
    const dbNum = Number(dbValue)
    const fileNum = Number(fileValue)
    const sameNumber = dbValue !== '' && fileValue !== '' && Number.isFinite(dbNum) && Number.isFinite(fileNum)
      && dbNum === fileNum
    const differs = !sameNumber && dbValue.toLowerCase() !== fileValue.toLowerCase()
    return {
      field,
      dbValue,
      fileValue,
      dbDisplay: dbValue,
      fileDisplay: fileValue,
      differs,
      adoptedFromFile: differs && adoptedFromFileFields.value.has(field),
    }
  })
})

async function openComparison() {
  if (!currentTrack.value) return
  showComparison.value = true
  comparisonFileTags.value = null
  comparisonError.value = null
  adoptedFromFileFields.value = new Set()
  isComparingSources.value = true
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    comparisonFileTags.value = await invoke<TrackTagsSnapshot>('read_track_tags', { trackId: currentTrack.value.id })
  } catch (err) {
    console.error('Failed to read file tags for comparison:', err)
    comparisonError.value = err instanceof Error ? err.message : String(err)
  } finally {
    isComparingSources.value = false
  }
}

/** Adopts the file-side value into the editable form for this field. */
function adoptComparisonValue(row: ComparisonRow) {
  if (!row.differs || row.fileValue === '') return
  const target = COMPARISON_FIELDS.find(f => f.field === row.field)
  if (!target) return
  switch (target.dbKey) {
    case 'year': {
      const n = parseInt(row.fileValue, 10)
      editForm.year = Number.isFinite(n) && n > 0 ? n : editForm.year
      break
    }
    case 'trackNumber': {
      const n = parseInt(row.fileValue.split('/')[0], 10)
      editForm.trackNumber = Number.isFinite(n) && n > 0 ? String(n) : editForm.trackNumber
      break
    }
    case 'bpm': {
      const n = parseFloat(row.fileValue)
      editForm.bpm = Number.isFinite(n) && n > 0 ? n : editForm.bpm
      break
    }
    default:
      (editForm as unknown as Record<string, unknown>)[target.dbKey] = row.fileValue
  }
  const next = new Set(adoptedFromFileFields.value)
  next.add(row.field)
  adoptedFromFileFields.value = next
}

function notifyComparisonHint() {
  if (adoptedFromFileFields.value.size > 0) {
    showToast(`${adoptedFromFileFields.value.size} campo(s) adoptados desde el archivo — pulsa «Save Changes» para persistir`, 'info')
  }
}
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(128, 128, 128, 0.3);
  border-radius: 4px;
}

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(128, 128, 128, 0.5);
}

/* Fade transition */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Slide down transition */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.2s ease;
}
.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}

/* Track row hover */
.track-row:hover .metadata-score {
  transform: scale(1.1);
}

/* Toast transitions */
.toast-enter-active {
  transition: all 0.3s ease;
}
.toast-leave-active {
  transition: all 0.2s ease;
}
.toast-enter-from {
  opacity: 0;
  transform: translateX(100%);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(100%);
}
.toast-move {
  transition: transform 0.3s ease;
}
</style>
