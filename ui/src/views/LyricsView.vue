<template>
  <div class="lyrics-page h-full flex bg-background-light dark:bg-background-dark overflow-hidden">
    
    <!-- Left Panel: Track List (35%) -->
    <div class="lyrics-track-list w-[35%] flex flex-col border-r border-gray-200 dark:border-border-dark">
      
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
            <option value="synced">Synced Lyrics</option>
            <option value="unsynced">Unsynced Lyrics</option>
            <option value="none">No Lyrics</option>
            <option value="downloaded">Downloaded Only</option>
            <option value="invalid-ts">Timestamp Issues</option>
          </select>
          
          <!-- Quality Analyzer Button -->
          <button @click="openQualityReport" class="p-2 bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 rounded-lg transition-colors" title="Lyrics Quality Report">
            <span class="material-symbols-outlined text-[20px]">analytics</span>
          </button>

          <!-- Lyrics Sources / Providers -->
          <button @click="showProviderSettings = true" class="p-2 bg-blue-500/10 text-blue-500 hover:bg-blue-500/20 rounded-lg transition-colors" title="Fuentes de letras y preferencias">
            <span class="material-symbols-outlined text-[20px]">tune</span>
          </button>
        </div>

        <!-- Stats strip -->
        <div class="mt-3 flex items-center gap-3 text-[11px] text-text-secondary">
          <span class="flex items-center gap-1"><span class="w-2 h-2 rounded-full bg-blue-500"></span>{{ lyricsStats.synced_lyrics }} synced</span>
          <span class="flex items-center gap-1"><span class="w-2 h-2 rounded-full bg-gray-500"></span>{{ unsyncedCount }} unsynced</span>
          <span class="flex items-center gap-1"><span class="w-2 h-2 rounded-full bg-error"></span>{{ noLyricsCount }} sin letra</span>
          <span class="ml-auto flex items-center gap-1"><span class="material-symbols-outlined text-[12px]">save</span>{{ lyricsStats.embedded_lyrics }} embebidas</span>
          <button
            @click="scanDiskForLyrics"
            :disabled="isHarvestingLyrics"
            class="ml-2 flex items-center gap-1 px-2 py-0.5 rounded border border-gray-300 dark:border-border-dark hover:border-primary/50 transition-colors disabled:opacity-50"
            title="Buscar letras incrustadas en los archivos FLAC y sidecars .lrc/.txt junto al audio para pistas sin letra"
          >
            <span :class="['material-symbols-outlined text-[12px]', isHarvestingLyrics && 'animate-spin']">{{ isHarvestingLyrics ? 'progress_activity' : 'folder_search' }}</span>
            Escanear disco
          </button>
        </div>
        
        <!-- Enhanced Batch Toolbar -->
        <Transition name="slide-down">
          <div v-if="selectedTracks.length > 0" class="batch-toolbar mt-3 p-3 bg-primary/10 rounded-lg">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <span class="text-sm text-primary font-semibold">{{ selectedTracks.length }} track{{ selectedTracks.length > 1 ? 's' : '' }} selected</span>
              <div class="flex flex-wrap items-center gap-2">
                <!-- Fetch Dropdown -->
                <div class="relative">
                  <button @click="showFetchDropdown = !showFetchDropdown" class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-500 text-white hover:bg-blue-600 rounded-lg text-xs font-medium transition-colors">
                    <span class="material-symbols-outlined text-[14px]">download</span>
                    Fetch Lyrics
                    <span class="material-symbols-outlined text-[12px]">expand_more</span>
                  </button>
                  <Teleport to="body">
                    <div v-if="showFetchDropdown" class="fixed inset-0 z-20" @click="showFetchDropdown = false"></div>
                  </Teleport>
                  <Transition name="fade">
                    <div v-if="showFetchDropdown" class="absolute top-full left-0 mt-1 w-44 bg-white dark:bg-surface-dark rounded-lg shadow-xl border border-gray-200 dark:border-border-dark overflow-hidden z-30">
                      <button @click="applyFetchMode('prefer_synced')" class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Prefer synced</button>
                      <button @click="applyFetchMode('any')" class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Unsynced OK</button>
                      <button @click="applyFetchMode('synced_only')" class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Synced only</button>
                    </div>
                  </Transition>
                </div>
                
                <!-- Export Dropdown -->
                <div class="relative">
                  <button @click="showExportDropdown = !showExportDropdown" class="flex items-center gap-1.5 px-3 py-1.5 bg-green-500/20 text-green-400 hover:bg-green-500/30 rounded-lg text-xs font-medium transition-colors">
                    <span class="material-symbols-outlined text-[14px]">save</span>
                    Export
                    <span class="material-symbols-outlined text-[12px]">expand_more</span>
                  </button>
                  <Teleport to="body">
                    <div v-if="showExportDropdown" class="fixed inset-0 z-20" @click="showExportDropdown = false"></div>
                  </Teleport>
                  <Transition name="fade">
                    <div v-if="showExportDropdown" class="absolute top-full left-0 mt-1 w-32 bg-white dark:bg-surface-dark rounded-lg shadow-xl border border-gray-200 dark:border-border-dark overflow-hidden z-30">
                      <button @click="exportSelectedLyrics('lrc')" class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">LRC Format</button>
                      <button @click="exportSelectedLyrics('ttml')" class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">TTML Format</button>
                      <button @click="exportSelectedLyrics('txt')" class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Plain TXT</button>
                    </div>
                  </Transition>
                </div>
                
                <button @click="upgradeSelectedToSynced" :disabled="isUpgrading" class="flex items-center gap-1.5 px-3 py-1.5 bg-purple-500/20 text-purple-400 hover:bg-purple-500/30 rounded-lg text-xs font-medium transition-colors disabled:opacity-50">
                  <span :class="['material-symbols-outlined text-[14px]', isUpgrading && 'animate-spin']">{{ isUpgrading ? 'progress_activity' : 'upgrade' }}</span>
                  Upgrade
                </button>

                <button @click="embedSelectedLyrics" :disabled="isEmbeddingBatch" class="flex items-center gap-1.5 px-3 py-1.5 bg-teal-500/20 text-teal-500 hover:bg-teal-500/30 rounded-lg text-xs font-medium transition-colors disabled:opacity-50" title="Embeber letras en los FLAC descargados">
                  <span :class="['material-symbols-outlined text-[14px]', isEmbeddingBatch && 'animate-spin']">{{ isEmbeddingBatch ? 'progress_activity' :('album') }}</span>
                  Embed
                </button>
                
                <button @click="deleteSelectedLyrics" class="flex items-center gap-1.5 px-3 py-1.5 bg-error/20 text-error hover:bg-error/30 rounded-lg text-xs font-medium transition-colors">
                  <span class="material-symbols-outlined text-[14px]">delete</span>
                  Delete
                </button>
                
                <div class="w-px h-4 bg-gray-300 dark:bg-gray-600"></div>
                <button @click="clearSelection" class="text-xs text-primary hover:underline">Clear</button>
              </div>
            </div>
          </div>
        </Transition>
      </div>
      
      <!-- Loading State -->
      <div v-if="isLoading" class="flex-1 flex items-center justify-center">
        <span class="material-symbols-outlined text-4xl text-primary animate-spin">progress_activity</span>
      </div>

      <!-- Empty Library -->
      <div v-else-if="tracks.length === 0" class="flex-1 flex items-center justify-center">
        <div class="text-center p-8">
          <span class="material-symbols-outlined text-5xl text-gray-400 mb-4">lyrics</span>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">No tracks in library</h3>
          <p class="text-text-secondary">Import music from streaming services to get started</p>
        </div>
      </div>

      <!-- Track List -->
      <div v-else class="flex-1 overflow-y-auto custom-scrollbar">
        <p v-if="filteredTracks.length === 0" class="px-4 py-6 text-center text-xs text-text-secondary italic">Sin resultados para este filtro.</p>
        <div 
          v-for="track in filteredTracks" 
          :key="track.id"
          @click="selectTrack(track)"
          :class="[
            'flex items-center gap-3 px-4 py-3 cursor-pointer transition-colors border-l-2',
            selectedTrackId === track.id ? 'bg-primary/10 border-l-primary' : 
            (!track.lyrics_type || track.lyrics_type === 'none') ? 'border-l-error/50 hover:bg-gray-50 dark:hover:bg-surface-highlight/50' : 
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
            <img v-if="track.cover_art_url" :src="track.cover_art_url" :alt="track.album_name ?? ''" class="w-full h-full object-cover" loading="lazy">
            <div v-else class="w-full h-full bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-600 dark:to-gray-700 flex items-center justify-center">
              <span class="material-symbols-outlined text-gray-500 dark:text-gray-400 text-[18px]">album</span>
            </div>
          </div>
          
          <!-- Track Info -->
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ track.title }}</p>
            <p class="text-xs text-text-secondary truncate">{{ track.artist_name || 'Unknown Artist' }}</p>
          </div>
          
          <!-- Lyrics Status -->
          <div class="flex flex-col items-end gap-1 shrink-0">
            <span :class="[
              'px-2 py-0.5 text-[10px] font-medium rounded flex items-center gap-1',
              mapLyricsType(track.lyrics_type) === 'synced' ? 'bg-blue-500/10 text-blue-500' :
              mapLyricsType(track.lyrics_type) === 'unsynced' ? 'bg-gray-500/10 text-gray-500' :
              'bg-error/10 text-error'
            ]">
              <span class="material-symbols-outlined text-[12px]">
                {{ mapLyricsType(track.lyrics_type) === 'synced' ? 'music_note' : mapLyricsType(track.lyrics_type) === 'unsynced' ? 'notes' : 'remove' }}
              </span>
              {{ mapLyricsType(track.lyrics_type) === 'synced' ? 'Synced' : mapLyricsType(track.lyrics_type) === 'unsynced' ? 'Unsynced' : 'None' }}
            </span>
          </div>
        </div>
      </div>
    </div>
    
    <!-- Right Panel: Lyrics Viewer/Editor (65%) -->
    <div class="lyrics-viewer w-[65%] flex flex-col overflow-hidden">
      
      <!-- No Selection State -->
      <div v-if="!selectedTrackId" class="flex-1 flex flex-col items-center justify-center text-center p-8">
        <div class="h-20 w-20 rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center mb-4">
          <span class="material-symbols-outlined text-5xl text-text-secondary">lyrics</span>
        </div>
        <h3 class="text-xl font-semibold text-gray-900 dark:text-white mb-2">Select a Track</h3>
        <p class="text-text-secondary max-w-md">Choose a track from the list to view and edit its lyrics</p>
      </div>
      
      <!-- Track Selected -->
      <template v-else>
        <!-- Header -->
        <div class="viewer-header shrink-0 border-b border-gray-200 dark:border-border-dark">
          <!-- Track Info Bar -->
          <div class="px-6 py-4 flex items-center gap-4">
            <div class="w-16 h-16 rounded-lg bg-gray-200 dark:bg-surface-highlight overflow-hidden shrink-0">
              <img v-if="currentTrack?.coverUrl" :src="currentTrack.coverUrl" :alt="currentTrack.album" class="w-full h-full object-cover">
              <div v-else class="w-full h-full bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-600 dark:to-gray-700 flex items-center justify-center">
                <span class="material-symbols-outlined text-2xl text-gray-500">album</span>
              </div>
            </div>
            <div class="flex-1 min-w-0">
              <h2 class="text-xl font-bold text-gray-900 dark:text-white truncate">{{ currentTrack?.title }}</h2>
              <p class="text-text-secondary truncate">{{ currentTrack?.artist }} · {{ currentTrack?.album }}</p>
            </div>
            <span class="text-lg font-mono text-text-secondary">{{ currentTrack?.duration }}</span>
          </div>
          
          <!-- Lyrics Info Bar -->
          <div class="px-6 py-3 bg-gray-50 dark:bg-surface-highlight/30 flex items-center justify-between gap-2">
            <div class="flex flex-wrap items-center gap-2 min-w-0">
              <span v-if="currentTrack?.lyricsStatus !== 'none'" :class="[
                'px-2 py-1 text-xs font-medium rounded',
                currentTrack?.lyricsStatus === 'synced' ? 'bg-blue-500/10 text-blue-500' : 'bg-gray-500/10 text-gray-500'
              ]">
                {{ currentTrack?.lyricsStatus === 'synced' ? 'Synced · ' + (currentTrack?.syncLevel || 'line') : 'Unsynced' }}
              </span>
              <span v-if="currentTrack?.source" class="px-2 py-1 bg-purple-500/10 text-purple-500 text-xs font-medium rounded truncate max-w-[160px]" :title="currentTrack?.source">
                {{ currentTrack?.source }}
              </span>
              <span v-if="currentTrack?.language" class="px-2 py-1 bg-green-500/10 text-green-500 text-xs font-medium rounded uppercase">
                {{ currentTrack?.language }}
              </span>
              <span v-if="currentLyrics?.embedded_in_file" class="flex items-center gap-1 px-2 py-1 bg-teal-500/10 text-teal-500 text-xs font-medium rounded">
                <span class="material-symbols-outlined text-[12px]">save</span> Embebida
              </span>
            </div>
            <div class="flex items-center gap-1 shrink-0">
              <button @click="startEditing" :class="[
                'p-2 rounded-lg transition-colors',
                isEditing ? 'bg-primary text-white' : 'hover:bg-gray-200 dark:hover:bg-surface-highlight text-gray-600 dark:text-gray-400'
              ]" title="Edit">
                <span class="material-symbols-outlined text-[18px]">edit</span>
              </button>
              <button @click="openFetchDialog" class="p-2 hover:bg-gray-200 dark:hover:bg-surface-highlight rounded-lg transition-colors text-gray-600 dark:text-gray-400" title="Buscar letra online">
                <span :class="['material-symbols-outlined text-[18px]', isFetching && 'animate-spin']">{{ isFetching ? 'progress_activity' : 'download' }}</span>
              </button>
              <!-- S192: associate an external .lrc/.txt file with the track -->
              <button @click="associateLyricsFile" :disabled="isImportingLyricsFile || !selectedTrackId" class="p-2 hover:bg-gray-200 dark:hover:bg-surface-highlight rounded-lg transition-colors text-gray-600 dark:text-gray-400 disabled:opacity-40 disabled:cursor-not-allowed" title="Asociar archivo .lrc / .txt">
                <span class="material-symbols-outlined text-[18px]" :class="{ 'animate-spin': isImportingLyricsFile }">{{ isImportingLyricsFile ? 'progress_activity' : 'attach_file' }}</span>
              </button>
              <button @click="embedSingleLyrics" :disabled="isEmbeddingSingle || !currentLyrics || !currentTrack?.filePath" class="p-2 hover:bg-gray-200 dark:hover:bg-surface-highlight rounded-lg transition-colors text-gray-600 dark:text-gray-400 disabled:opacity-40 disabled:cursor-not-allowed" title="Embeber letra en el archivo FLAC">
                <span :class="['material-symbols-outlined text-[18px]', isEmbeddingSingle && 'animate-spin']">{{ isEmbeddingSingle ? 'progress_activity' : 'album' }}</span>
              </button>
              <button @click="exportCurrentLyrics" :disabled="!currentLyrics" class="p-2 hover:bg-gray-200 dark:hover:bg-surface-highlight rounded-lg transition-colors text-gray-600 dark:text-gray-400 disabled:opacity-40" title="Exportar letra en su formato nativo">
                <span class="material-symbols-outlined text-[18px]">save_alt</span>
              </button>
              <button @click="openHistory" :disabled="!selectedTrackId" class="p-2 hover:bg-gray-200 dark:hover:bg-surface-highlight rounded-lg transition-colors text-gray-600 dark:text-gray-400 disabled:opacity-40" title="Versiones guardadas de esta letra">
                <span class="material-symbols-outlined text-[18px]">history</span>
              </button>
              <button @click="deleteTrackLyrics" :disabled="!currentLyrics" class="p-2 hover:bg-error/10 rounded-lg transition-colors text-error disabled:opacity-40" title="Delete">
                <span class="material-symbols-outlined text-[18px]">delete</span>
              </button>
            </div>
          </div>
          <!-- S192: transient import error banner -->
          <div v-if="importError" class="mx-6 mt-3 px-4 py-2 rounded-lg bg-red-500/10 border border-red-500/30 text-red-500 text-sm flex items-center gap-2">
            <span class="material-symbols-outlined text-[16px]">error</span>
            {{ importError }}
          </div>
          <!-- Inline action feedback -->
          <div v-if="actionMessage" :class="['mx-6 mt-3 px-4 py-2 rounded-lg text-sm flex items-center gap-2 border', actionMessageType === 'error' ? 'bg-red-500/10 border-red-500/30 text-red-500' : actionMessageType === 'success' ? 'bg-green-500/10 border-green-500/30 text-green-500' : 'bg-blue-500/10 border-blue-500/30 text-blue-500']">
            <span class="material-symbols-outlined text-[16px]">{{ actionMessageType === 'error' ? 'error' : actionMessageType === 'success' ? 'check_circle' : 'info' }}</span>
            {{ actionMessage }}
          </div>
        </div>
        
        <!-- Editor Mode -->
        <div v-if="isEditing" class="lyrics-editor flex-1 flex flex-col overflow-hidden">
          <!-- Editor Toolbar -->
          <div class="px-6 py-3 border-b border-gray-200 dark:border-border-dark flex items-center gap-3">
            <button @click="openSyncEditor" class="flex items-center gap-1.5 px-3 py-1.5 bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 rounded-lg text-sm font-medium transition-colors">
              <span class="material-symbols-outlined text-[16px]">timer</span>
              Sync Timestamps
            </button>
            <span :class="['px-2 py-1 rounded text-xs font-medium', editorFormat === 'lrc' ? 'bg-blue-500/10 text-blue-500' : 'bg-gray-500/10 text-gray-500']">
              Formato: {{ editorFormat.toUpperCase() }}
            </span>
            <button @click="validateEditor" class="flex items-center gap-1.5 px-3 py-1.5 bg-green-500/10 text-green-500 hover:bg-green-500/20 rounded-lg text-sm font-medium transition-colors">
              <span class="material-symbols-outlined text-[16px]">check_circle</span>
              Validate
            </button>
            <div class="flex-1"></div>
            <button @click="isEditing = false" class="px-4 py-1.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
              Cancel
            </button>
            <button @click="saveLyricsEdit" :disabled="isSavingEdit || !editableLyrics.trim()" class="px-4 py-1.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 flex items-center gap-1.5">
              <span :class="['material-symbols-outlined text-[14px]', isSavingEdit && 'animate-spin']">{{ isSavingEdit ? 'progress_activity' : 'check' }}</span>
              {{ isSavingEdit ? 'Saving…' : 'Save Changes' }}
            </button>
          </div>

          <!-- Validation banner -->
          <div v-if="validationMessage" :class="['mx-6 mt-3 px-4 py-2 rounded-lg text-sm flex items-start gap-2 border', validationOk ? 'bg-green-500/10 border-green-500/30 text-green-500' : 'bg-amber-500/10 border-amber-500/30 text-amber-500']">
            <span class="material-symbols-outlined text-[16px]">{{ validationOk ? 'check_circle' : 'warning' }}</span>
            <span class="whitespace-pre-line">{{ validationMessage }}</span>
          </div>
          
          <!-- Editor Textarea -->
          <div class="flex-1 p-6 pb-3 overflow-hidden">
            <textarea 
              v-model="editableLyrics"
              class="w-full h-full p-4 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-xl text-sm font-mono text-gray-900 dark:text-white resize-none focus:outline-none focus:ring-2 focus:ring-primary/50"
              placeholder="Enter lyrics here... (usa [mm:ss.xx] para letras sincronizadas — Ctrl+S guarda)"
            ></textarea>
          </div>
          <p class="px-6 pb-4 text-[11px] text-text-secondary">Ctrl+S / Cmd+S guarda los cambios.</p>
        </div>
        
        <!-- Viewer Mode -->
        <div v-else class="lyrics-content flex-1 flex flex-col overflow-hidden">
          <!-- No Lyrics State -->
          <div v-if="currentTrack?.lyricsStatus === 'none'" class="flex-1 flex flex-col items-center justify-center text-center p-8">
            <div class="h-16 w-16 rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center mb-4">
              <span class="material-symbols-outlined text-4xl text-text-secondary">lyrics</span>
            </div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">No Lyrics Available</h3>
            <p class="text-text-secondary mb-4">This track doesn't have any lyrics yet</p>
            <div class="flex items-center gap-3">
              <button @click="runAutoFetch" :disabled="isFetching" class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors flex items-center gap-2 disabled:opacity-50">
                <span :class="['material-symbols-outlined text-[18px]', isFetching && 'animate-spin']">{{ isFetching ? 'progress_activity' : 'search' }}</span>
                {{ isFetching ? 'Buscando…' : 'Search for Lyrics' }}
              </button>
              <button @click="associateLyricsFile" class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px]">attach_file</span>
                Asociar archivo
              </button>
            </div>
          </div>
          
          <!-- Synced Lyrics Display -->
          <div v-else-if="currentTrack?.lyricsStatus === 'synced'" class="flex-1 overflow-y-auto custom-scrollbar p-6" data-testid="synced-lyrics-container">
            <div class="space-y-1 max-w-2xl mx-auto">
              <div 
                v-for="(line, index) in syncedLyrics" 
                :key="index"
                :data-line-index="index"
                :ref="el => setLineRef(el, index)"
                :class="[
                  'synced-line flex items-start gap-4 py-2 px-4 rounded-lg transition-colors cursor-pointer',
                  activeLineIndex === index ? 'bg-primary/10' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
                ]"
                @click="seekToLine(index)"
              >
                <span class="timestamp text-xs font-mono text-text-secondary w-16 shrink-0 pt-1">{{ line.label }}</span>
                <p :class="[
                  'text-lg transition-colors',
                  activeLineIndex === index ? 'text-primary font-medium' : 'text-gray-700 dark:text-gray-300'
                ]">{{ line.text || '♪' }}</p>
              </div>
            </div>
          </div>
          
          <!-- Unsynced Lyrics Display -->
          <div v-else class="flex-1 overflow-y-auto custom-scrollbar p-6">
            <div class="max-w-2xl mx-auto prose dark:prose-invert">
              <p v-for="(paragraph, index) in unsyncedParagraphs" :key="index" class="text-lg text-gray-700 dark:text-gray-300 mb-6 leading-relaxed whitespace-pre-line">{{ paragraph }}</p>
            </div>
          </div>
          
          <!-- Playback Controls (for synced lyrics) -->
          <div v-if="currentTrack?.lyricsStatus === 'synced'" class="shrink-0 px-6 py-4 border-t border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30">
            <div class="flex items-center gap-4">
              <button @click="togglePlayback" class="p-2 bg-primary hover:bg-primary-hover text-white rounded-full transition-colors" title="Reproducir la pista descargada">
                <span class="material-symbols-outlined text-[20px]">{{ isPlayingAudio ? 'pause' : 'play_arrow' }}</span>
              </button>
              <div class="flex-1">
                <input type="range" :value="progressPercent" min="0" max="100" step="0.1" @input="onSeekPercent(($event.target as HTMLInputElement).valueAsNumber)" class="w-full accent-[var(--color-primary)] cursor-pointer">
              </div>
              <span class="text-sm font-mono text-text-secondary w-28 text-right">{{ currentTimeLabel }} / {{ totalTimeLabel }}</span>
              <button @click="autoScroll = !autoScroll" :class="[
                'p-2 rounded-lg transition-colors',
                autoScroll ? 'bg-primary/10 text-primary' : 'hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-600 dark:text-gray-400'
              ]" title="Auto-scroll">
                <span class="material-symbols-outlined text-[18px]">sync</span>
              </button>
            </div>
          </div>
        </div>
      </template>
    </div>
    
    <!-- Fetch Lyrics Dialog -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showFetchDialog" class="fetch-dialog fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showFetchDialog = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[80vh] overflow-hidden shadow-2xl flex flex-col">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Search for Lyrics</h3>
              <button @click="showFetchDialog = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 overflow-y-auto custom-scrollbar">
              <!-- Search Info -->
              <div class="mb-4 p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg flex items-center gap-3">
                <span class="material-symbols-outlined text-primary">search</span>
                <span class="text-sm text-gray-700 dark:text-gray-300">
                  Searching for: <span class="font-medium text-gray-900 dark:text-white">{{ currentTrack?.title }}</span> by <span class="font-medium text-gray-900 dark:text-white">{{ currentTrack?.artist }}</span>
                </span>
                <button @click="runLyricsSearch" :disabled="isSearching" class="ml-auto px-3 py-1.5 bg-primary/10 hover:bg-primary/20 text-primary rounded-lg text-xs font-medium disabled:opacity-50 flex items-center gap-1">
                  <span :class="['material-symbols-outlined text-[14px]', isSearching && 'animate-spin']">{{ isSearching ? 'progress_activity' : 'refresh' }}</span>
                  Reintentar
                </button>
              </div>

              <!-- Loading -->
              <div v-if="isSearching" class="py-10 text-center text-text-secondary text-sm flex flex-col items-center gap-2">
                <span class="material-symbols-outlined text-3xl text-primary animate-spin">progress_activity</span>
                Consultando proveedores…
              </div>

              <!-- Error -->
              <div v-else-if="searchError" class="mb-4 px-4 py-3 rounded-lg bg-error/10 border border-error/30 text-error text-sm flex items-center gap-2">
                <span class="material-symbols-outlined text-[16px]">error</span>
                {{ searchError }}
              </div>

              <!-- Empty -->
              <div v-else-if="lyricsResults.length === 0" class="py-10 text-center">
                <span class="material-symbols-outlined text-4xl text-gray-400 mb-2 block">search_off</span>
                <p class="text-sm text-text-secondary">Ningún proveedor devolvió resultados para esta pista.</p>
              </div>
              
              <!-- Results -->
              <div v-else class="space-y-3">
                <div v-for="(result, ri) in lyricsResults" :key="ri" class="lyrics-result p-4 border border-gray-200 dark:border-border-dark rounded-xl hover:border-primary/50 transition-colors">
                  <div class="flex items-start justify-between mb-3 gap-2">
                    <div class="flex flex-wrap items-center gap-2">
                      <span class="text-sm font-medium text-gray-900 dark:text-white">{{ result.source }}</span>
                      <span :class="[
                        'px-2 py-0.5 text-[10px] font-medium rounded',
                        result.sync_type === 'WORD_SYNCED' ? 'bg-purple-500/10 text-purple-500' :
                        result.sync_type === 'LINE_SYNCED' ? 'bg-blue-500/10 text-blue-500' : 'bg-gray-500/10 text-gray-500'
                      ]">
                        {{ result.sync_type === 'WORD_SYNCED' ? 'Word-synced' : result.sync_type === 'LINE_SYNCED' ? 'Synced' : 'Unsynced' }}
                      </span>
                      <span v-if="result.instrumental" class="px-2 py-0.5 bg-teal-500/10 text-teal-500 text-[10px] font-medium rounded">Instrumental</span>
                      <span v-if="result.album" class="px-2 py-0.5 bg-gray-500/10 text-gray-500 text-[10px] font-medium rounded truncate max-w-[180px]">{{ result.album }}</span>
                    </div>
                  </div>
                  <p class="text-sm text-text-secondary mb-3 line-clamp-3 whitespace-pre-line">{{ resultPreview(result) }}</p>
                  <button @click="applySearchResult(result)" :disabled="isApplyingResult" class="w-full py-2 bg-primary/10 hover:bg-primary/20 text-primary rounded-lg text-sm font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-1.5">
                    <span :class="['material-symbols-outlined text-[14px]', isApplyingResult && 'animate-spin']">{{ isApplyingResult ? 'progress_activity' : 'download_done' }}</span>
                    Use This
                  </button>
                </div>
              </div>
              
              <!-- Manual Search -->
              <div class="mt-4 pt-4 border-t border-gray-200 dark:border-border-dark">
                <button @click="enterManually" class="w-full py-3 border border-gray-200 dark:border-border-dark hover:border-primary hover:text-primary rounded-lg text-sm font-medium text-gray-600 dark:text-gray-400 transition-colors flex items-center justify-center gap-2">
                  <span class="material-symbols-outlined text-[18px]">edit</span>
                  Enter Lyrics Manually
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Batch Fetch Progress Dialog -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showBatchProgress" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Fetching Lyrics</h3>
            </div>
            
            <div class="p-6">
              <!-- Progress Bar -->
              <div class="mb-4">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-sm text-gray-700 dark:text-gray-300">Progress</span>
                  <span class="text-sm font-medium text-gray-900 dark:text-white">{{ batchProgress.current }} / {{ batchProgress.total }}</span>
                </div>
                <div class="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                  <div class="h-full bg-primary rounded-full transition-all" :style="{ width: batchProgress.total ? (batchProgress.current / batchProgress.total * 100) + '%' : '0%' }"></div>
                </div>
              </div>
              
              <!-- Current Track -->
              <p class="text-sm text-text-secondary mb-4 truncate">
                {{ batchProgress.currentTrack || '—' }}
              </p>
              
              <!-- Stats -->
              <div class="flex items-center gap-4 mb-6">
                <span class="flex items-center gap-1 text-sm text-success">
                  <span class="material-symbols-outlined text-[16px]">check_circle</span>
                  Found: {{ batchProgress.success }}
                </span>
                <span class="flex items-center gap-1 text-sm text-error">
                  <span class="material-symbols-outlined text-[16px]">cancel</span>
                  Not found: {{ batchProgress.failed }}
                </span>
                <span v-if="batchProgress.skipped > 0" class="flex items-center gap-1 text-sm text-text-secondary">
                  <span class="material-symbols-outlined text-[16px]">skip_next</span>
                  Skipped: {{ batchProgress.skipped }}
                </span>
              </div>
              
              <!-- Close Button (the backend loop continues independently) -->
              <button @click="showBatchProgress = false" class="w-full py-2.5 border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 transition-colors">
                Cerrar (el proceso continúa en segundo plano)
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
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[85vh] overflow-hidden shadow-2xl flex flex-col">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Lyrics Quality Report</h3>
              <button @click="showQualityReport = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 overflow-y-auto custom-scrollbar max-h-[calc(85vh-140px)]">
              <!-- Loading -->
              <div v-if="isReportLoading" class="py-12 text-center text-text-secondary text-sm flex flex-col items-center gap-2">
                <span class="material-symbols-outlined text-3xl text-primary animate-spin">progress_activity</span>
                Analizando biblioteca…
              </div>

              <template v-else>
                <!-- Stats Overview -->
                <div class="grid grid-cols-3 gap-4 mb-6">
                  <div class="p-4 bg-blue-500/10 rounded-xl text-center">
                    <p class="text-2xl font-bold text-blue-500">{{ reportData.synced }}</p>
                    <p class="text-xs text-text-secondary mt-1">Synced ({{ reportData.syncedPct }}%)</p>
                  </div>
                  <div class="p-4 bg-gray-500/10 rounded-xl text-center">
                    <p class="text-2xl font-bold text-gray-500">{{ reportData.unsynced }}</p>
                    <p class="text-xs text-text-secondary mt-1">Unsynced ({{ reportData.unsyncedPct }}%)</p>
                  </div>
                  <div class="p-4 bg-error/10 rounded-xl text-center">
                    <p class="text-2xl font-bold text-error">{{ reportData.none }}</p>
                    <p class="text-xs text-text-secondary mt-1">No Lyrics ({{ reportData.nonePct }}%)</p>
                  </div>
                </div>
                
                <!-- Sync Level Breakdown -->
                <div class="mb-6" v-if="reportData.withLyrics > 0">
                  <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Sync Level Breakdown</h5>
                  <div class="space-y-2">
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Syllable-level</span>
                      <span class="px-2 py-0.5 bg-purple-500/10 text-purple-500 text-xs font-medium rounded">{{ reportData.syllable }} tracks</span>
                    </div>
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Word-level</span>
                      <span class="px-2 py-0.5 bg-blue-500/10 text-blue-500 text-xs font-medium rounded">{{ reportData.word }} tracks</span>
                    </div>
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Line-level</span>
                      <span class="px-2 py-0.5 bg-green-500/10 text-green-500 text-xs font-medium rounded">{{ reportData.line }} tracks</span>
                    </div>
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Embebidas en archivo</span>
                      <span class="px-2 py-0.5 bg-teal-500/10 text-teal-500 text-xs font-medium rounded">{{ lyricsStats.embedded_lyrics }} tracks</span>
                    </div>
                  </div>
                </div>
                
                <!-- Issues Found -->
                <div class="mb-6">
                  <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Issues Found</h5>
                  <div class="space-y-2">
                    <button @click="filterByIssue('invalid-ts')" :disabled="reportData.invalidTs === 0" class="w-full flex items-center justify-between p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg hover:bg-amber-500/10 transition-colors disabled:opacity-40 disabled:cursor-default">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Timestamps inválidos o fuera de orden (LRC)</span>
                      <span class="px-2 py-0.5 bg-amber-500/10 text-amber-500 text-xs font-medium rounded">{{ reportData.invalidTs }} tracks</span>
                    </button>
                    <button @click="filterByIssue('missing-ts')" :disabled="reportData.missingTs === 0" class="w-full flex items-center justify-between p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg hover:bg-amber-500/10 transition-colors disabled:opacity-40 disabled:cursor-default">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Marcadas como sincronizadas pero sin timestamps válidos</span>
                      <span class="px-2 py-0.5 bg-amber-500/10 text-amber-500 text-xs font-medium rounded">{{ reportData.missingTs }} tracks</span>
                    </button>
                    <button @click="filterByIssue('not-embedded')" :disabled="reportData.notEmbedded === 0" class="w-full flex items-center justify-between p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg hover:bg-amber-500/10 transition-colors disabled:opacity-40 disabled:cursor-default">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Descargadas con letra aún no embebida en el FLAC</span>
                      <span class="px-2 py-0.5 bg-amber-500/10 text-amber-500 text-xs font-medium rounded">{{ reportData.notEmbedded }} tracks</span>
                    </button>
                  </div>
                </div>
                
                <!-- Recommendations -->
                <div>
                  <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Recommendations</h5>
                  <div class="space-y-3">
                    <div class="flex items-center gap-3 p-3 bg-blue-500/5 border border-blue-500/20 rounded-lg">
                      <span class="material-symbols-outlined text-blue-500">download</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Fetch lyrics for {{ reportData.none }} tracks without lyrics</span>
                      <button @click="runFetchMissing()" :disabled="reportData.none === 0 || isRunningAction" class="px-3 py-1.5 bg-blue-500 text-white rounded-lg text-xs font-medium hover:bg-blue-600 transition-colors disabled:opacity-40">Run</button>
                    </div>
                    <div class="flex items-center gap-3 p-3 bg-purple-500/5 border border-purple-500/20 rounded-lg">
                      <span class="material-symbols-outlined text-purple-500">upgrade</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Upgrade {{ reportData.unsynced }} tracks to synced lyrics</span>
                      <button @click="runUpgradeUnsynced()" :disabled="reportData.unsynced === 0 || isRunningAction" class="px-3 py-1.5 bg-purple-500 text-white rounded-lg text-xs font-medium hover:bg-purple-600 transition-colors disabled:opacity-40">Run</button>
                    </div>
                    <div class="flex items-center gap-3 p-3 bg-teal-500/5 border border-teal-500/20 rounded-lg">
                      <span class="material-symbols-outlined text-teal-500">save</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Embeber {{ reportData.notEmbedded }} letras en sus archivos</span>
                      <button @click="embedAllEligible()" :disabled="reportData.notEmbedded === 0 || isRunningAction" class="px-3 py-1.5 bg-teal-500 text-white rounded-lg text-xs font-medium hover:bg-teal-600 transition-colors disabled:opacity-40">Run</button>
                    </div>
                  </div>
                </div>
              </template>
            </div>
            
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end shrink-0">
              <button 
                @click="autoFixAll"
                :disabled="isRunningAction"
                class="px-6 py-2.5 bg-purple-500 hover:bg-purple-600 text-white rounded-lg font-medium transition-colors flex items-center gap-2 disabled:opacity-50"
              >
                <span :class="['material-symbols-outlined text-[18px]', isRunningAction && 'animate-spin']">{{ isRunningAction ? 'progress_activity' : 'auto_fix_high' }}</span>
                {{ isRunningAction ? 'Ejecutando…' : 'Auto-Fix All' }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Provider Settings Panel -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showProviderSettings" class="provider-settings fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showProviderSettings = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg max-h-[85vh] overflow-hidden shadow-2xl flex flex-col">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Lyrics Sources</h3>
              <button @click="showProviderSettings = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 overflow-y-auto custom-scrollbar max-h-[calc(85vh-140px)]">
              <!-- Provider List -->
              <div class="mb-6">
                <h5 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Priority Order (primero = mayor prioridad)</h5>
                <div class="provider-list space-y-2">
                  <div v-for="(provider, pi) in providers" :key="provider.provider_id" class="flex items-center gap-2 p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                    <div class="flex flex-col gap-0.5">
                      <button @click="moveProvider(pi, -1)" :disabled="pi === 0" class="p-0.5 text-gray-400 hover:text-primary disabled:opacity-30" title="Subir prioridad">
                        <span class="material-symbols-outlined text-[16px]">arrow_drop_up</span>
                      </button>
                      <button @click="moveProvider(pi, 1)" :disabled="pi === providers.length - 1" class="p-0.5 text-gray-400 hover:text-primary disabled:opacity-30" title="Bajar prioridad">
                        <span class="material-symbols-outlined text-[16px]">arrow_drop_down</span>
                      </button>
                    </div>
                    <div class="flex-1 min-w-0">
                      <p class="text-sm font-medium text-gray-900 dark:text-white">{{ provider.provider_name }}</p>
                      <p class="text-xs text-text-secondary">#{{ provider.priority }} · {{ provider.sync_level }}</p>
                    </div>
                    <button @click="testProvider(provider)" :disabled="testingProviderId === provider.provider_id" class="px-2 py-1 rounded text-[11px] font-medium transition-colors flex items-center gap-1"
                      :class="testResults[provider.provider_id] === true ? 'bg-success/10 text-success' : testResults[provider.provider_id] === false ? 'bg-error/10 text-error' : 'bg-gray-500/10 text-gray-500 hover:bg-gray-500/20'"
                      title="Probar conexión con el proveedor">
                      <span :class="['material-symbols-outlined text-[12px]', testingProviderId === provider.provider_id && 'animate-spin']">
                        {{ testingProviderId === provider.provider_id ? 'progress_activity' : testResults[provider.provider_id] === true ? 'check_circle' : testResults[provider.provider_id] === false ? 'cancel' : 'network_check' }}
                      </span>
                      Probar
                    </button>
                    <button role="switch" :aria-checked="provider.enabled" @click="toggleProvider(provider)" :class="['relative inline-flex items-center h-5 w-9 rounded-full transition-colors shrink-0', provider.enabled ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600']">
                      <span :class="['inline-block w-4 h-4 bg-white rounded-full shadow transition-transform', provider.enabled ? 'translate-x-4' : 'translate-x-0.5']"></span>
                    </button>
                  </div>
                  <p v-if="providers.length === 0" class="text-xs text-text-secondary italic">No hay proveedores configurados en la base de datos.</p>
                </div>
              </div>
              
              <!-- Preferences -->
              <div class="mb-6" v-if="providerForm">
                <h5 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Preferences</h5>
                <div class="grid grid-cols-2 gap-3">
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">Idioma preferido</label>
                    <select v-model="providerForm.preferred_language" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
                      <option value="">Original</option>
                      <option value="en">English</option>
                      <option value="es">Español</option>
                      <option value="any">All Languages</option>
                    </select>
                  </div>
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">Nivel mínimo de sync</label>
                    <select v-model="providerForm.min_sync_level" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
                      <option value="none">Any</option>
                      <option value="line">Line-level</option>
                      <option value="word">Word-level</option>
                      <option value="syllable">Syllable-level</option>
                    </select>
                  </div>
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">Formato de almacenamiento</label>
                    <select v-model="providerForm.storage_format" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
                      <option value="lrc">LRC</option>
                      <option value="plain">Plain text</option>
                      <option value="ttml">TTML</option>
                    </select>
                  </div>
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">Reintento de fallos</label>
                    <select v-model="providerForm.retry_frequency" class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50" :disabled="!providerForm.retry_failed">
                      <option value="always">Siempre</option>
                      <option value="daily">Diario</option>
                      <option value="weekly">Semanal</option>
                      <option value="never">Nunca</option>
                    </select>
                  </div>
                </div>
                <div class="mt-3 space-y-2">
                  <label class="flex items-center gap-3 cursor-pointer">
                    <input type="checkbox" v-model="providerForm.auto_fetch_on_import" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Buscar letra automáticamente al importar</span>
                  </label>
                  <label class="flex items-center gap-3 cursor-pointer">
                    <input type="checkbox" v-model="providerForm.retry_failed" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Reintentar pistas cuya búsqueda falló</span>
                  </label>
                </div>
              </div>
            </div>
            
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3 shrink-0">
              <button @click="showProviderSettings = false" class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg font-medium transition-colors">
                Cancel
              </button>
              <button @click="saveProviderSettings" :disabled="isSavingProviders || !providerForm" class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center gap-1.5">
                <span :class="['material-symbols-outlined text-[14px]', isSavingProviders && 'animate-spin']">{{ isSavingProviders ? 'progress_activity' : 'done' }}</span>
                {{ isSavingProviders ? 'Guardando…' : 'Save Settings' }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Advanced Sync Editor Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showSyncEditor" class="sync-editor fixed inset-0 bg-black flex flex-col z-50" tabindex="-1" @keydown="onSyncEditorKeydown">
          <!-- Header -->
          <div class="px-6 py-4 border-b border-gray-700 flex items-center justify-between bg-gray-900">
            <div class="flex items-center gap-4">
              <h3 class="text-lg font-semibold text-white">Advanced Sync Editor</h3>
              <span class="text-sm text-gray-400">{{ currentTrack?.title }} - {{ currentTrack?.artist }}</span>
            </div>
            <div class="flex items-center gap-2">
              <button @click="showSyncEditor = false" class="px-4 py-2 text-gray-300 hover:bg-gray-800 rounded-lg font-medium transition-colors">
                Cancel
              </button>
              <button @click="saveSyncEditor" :disabled="isSavingSync" class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium transition-colors disabled:opacity-50 flex items-center gap-1.5">
                <span :class="['material-symbols-outlined text-[14px]', isSavingSync && 'animate-spin']">{{ isSavingSync ? 'progress_activity' : 'save' }}</span>
                Save &amp; Close
              </button>
            </div>
          </div>
          
          <!-- Waveform Display -->
          <div class="waveform-display h-32 bg-gray-900 border-b border-gray-700 relative cursor-pointer" @click="seekFromWaveform($event)">
            <div class="absolute inset-x-0 top-0 bottom-6 flex items-end gap-0.5 px-2">
              <div v-for="(h, i) in waveformBars" :key="i" class="flex-1 bg-primary/40 rounded-t pointer-events-none" :style="{ height: h + '%' }"></div>
            </div>
            <!-- Timestamp markers -->
            <div
              v-for="(entry, i) in markedSyncEntries"
              :key="'m' + i"
              class="absolute bottom-6 w-0.5 h-4 bg-purple-400 pointer-events-none"
              :style="{ left: linePct(entry.line.t) + '%' }"
            ></div>
            <!-- Playhead -->
            <div class="absolute top-0 bottom-6 w-0.5 bg-primary pointer-events-none" :style="{ left: linePct(playbackPosition) + '%' }"></div>
            <!-- Time ruler -->
            <div class="absolute bottom-0 left-0 right-0 h-6 flex items-center px-4 pointer-events-none">
              <span v-for="t in waveRulerTicks" :key="t" class="flex-1 text-center text-[10px] text-gray-500">{{ formatClock(t) }}</span>
            </div>
          </div>
          
          <!-- Main Content -->
          <div class="flex-1 flex overflow-hidden">
            <!-- Timestamp Editor -->
            <div class="timestamp-editor flex-1 overflow-y-auto custom-scrollbar bg-gray-900 p-6">
              <div class="space-y-2 max-w-2xl mx-auto">
                <div 
                  v-for="(line, index) in syncLines" 
                  :key="index"
                  @click="syncSelectedIdx = index"
                  :class="['flex items-center gap-3 p-3 rounded-lg transition-colors group cursor-pointer', syncSelectedIdx === index ? 'bg-primary/10 ring-1 ring-primary/40' : 'hover:bg-gray-800']"
                >
                  <span class="w-6 text-center text-xs text-gray-500">{{ index + 1 }}</span>
                  <input 
                    type="text" 
                    :value="line.t !== null ? formatClock(line.t) : '—'"
                    @change="onSyncTimeInput(index, ($event.target as HTMLInputElement).value)"
                    class="w-24 px-2 py-1 bg-gray-800 border border-gray-700 rounded text-sm font-mono text-primary text-center focus:outline-none focus:ring-2 focus:ring-primary/50"
                  >
                  <input 
                    type="text" 
                    v-model="line.text"
                    class="flex-1 px-3 py-1 bg-gray-800 border border-gray-700 rounded text-sm text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                  >
                  <button @click.stop="markLineAtPlayhead(index)" class="p-1.5 text-gray-500 hover:text-primary hover:bg-gray-800 rounded transition-colors opacity-0 group-hover:opacity-100" title="Marcar en la posición actual (Space)">
                    <span class="material-symbols-outlined text-[18px]">play_arrow</span>
                  </button>
                  <button @click.stop="clearLineTimestamp(index)" class="p-1.5 text-gray-500 hover:text-error hover:bg-gray-800 rounded transition-colors opacity-0 group-hover:opacity-100" title="Quitar timestamp">
                    <span class="material-symbols-outlined text-[18px]">delete</span>
                  </button>
                </div>
                <button @click="addSyncLine" class="w-full py-2 border border-dashed border-gray-700 rounded-lg text-gray-500 hover:text-primary hover:border-primary transition-colors flex items-center justify-center gap-2">
                  <span class="material-symbols-outlined text-[18px]">add</span>
                  Add Line
                </button>
              </div>
            </div>
            
            <!-- Tools Sidebar -->
            <div class="w-64 border-l border-gray-700 bg-gray-900 p-4 overflow-y-auto custom-scrollbar">
              <h5 class="text-sm font-medium text-gray-400 mb-4">Tools</h5>
              <div class="space-y-2">
                <div class="flex items-center gap-2">
                  <input v-model.number="shiftSeconds" type="number" step="0.1" class="w-20 px-2 py-2 bg-gray-800 border border-gray-700 rounded text-sm text-white text-center focus:outline-none focus:ring-2 focus:ring-primary/50">
                  <button @click="shiftAllTimestamps" class="flex-1 flex items-center gap-2 px-3 py-2 bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 rounded-lg text-sm font-medium transition-colors">
                    <span class="material-symbols-outlined text-[18px]">schedule</span>
                    Shift all
                  </button>
                </div>
                <button @click="snapToGrid" class="w-full flex items-center gap-2 px-3 py-2 bg-green-500/10 text-green-400 hover:bg-green-500/20 rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[18px]">grid_on</span>
                  Snap to grid (0.5s)
                </button>
                <button @click="validateSyncTiming" class="w-full flex items-center gap-2 px-3 py-2 bg-amber-500/10 text-amber-400 hover:bg-amber-500/20 rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[18px]">verified</span>
                  Validate timing
                </button>
                <button @click="clearAllTimestamps" class="w-full flex items-center gap-2 px-3 py-2 bg-red-500/10 text-red-400 hover:bg-red-500/20 rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[18px]">backspace</span>
                  Clear timestamps
                </button>
              </div>
              
              <h5 class="text-sm font-medium text-gray-400 mt-6 mb-3">Keyboard Shortcuts</h5>
              <div class="space-y-2 text-xs">
                <div class="flex justify-between text-gray-500">
                  <span>Mark timestamp</span>
                  <span class="text-gray-400">Space</span>
                </div>
                <div class="flex justify-between text-gray-500">
                  <span>Next line</span>
                  <span class="text-gray-400">Enter</span>
                </div>
                <div class="flex justify-between text-gray-500">
                  <span>Remove last mark</span>
                  <span class="text-gray-400">Backspace</span>
                </div>
                <div class="flex justify-between text-gray-500">
                  <span>Navigate</span>
                  <span class="text-gray-400">↑↓</span>
                </div>
                <div class="flex justify-between text-gray-500">
                  <span>Save</span>
                  <span class="text-gray-400">Ctrl+S</span>
                </div>
              </div>

              <p class="mt-6 text-[11px] text-gray-500 leading-relaxed">
                Marca cada línea con Space mientras suena la pista descargada. Si no hay reproducción activa, las marcas usan la posición actual del reproductor.
              </p>
            </div>
          </div>
          
          <!-- Playback Controls -->
          <div class="px-6 py-4 border-t border-gray-700 bg-gray-900 flex items-center gap-4">
            <button @click="togglePlayback" class="p-3 bg-primary hover:bg-primary-hover text-white rounded-full transition-colors">
              <span class="material-symbols-outlined text-[24px]">{{ isPlayingAudio ? 'pause' : 'play_arrow' }}</span>
            </button>
            <div class="flex-1">
              <input type="range" :value="progressPercent" min="0" max="100" step="0.1" @input="onSeekPercent(($event.target as HTMLInputElement).valueAsNumber)" class="w-full cursor-pointer">
            </div>
            <span class="text-sm font-mono text-gray-400 w-24 text-right">{{ currentTimeLabel }} / {{ totalTimeLabel }}</span>
            <div class="flex items-center gap-2">
              <button v-for="r in [0.5, 1, 1.5]" :key="r" :class="['px-2 py-1 rounded text-xs font-medium transition-colors', playbackRateValue === r ? 'bg-primary text-white' : 'text-gray-400 hover:text-white']" @click="player.setRate(r)">{{ r }}x</button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Lyrics History Panel -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showLyricsHistory" class="lyrics-history fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showLyricsHistory = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md max-h-[70vh] overflow-hidden shadow-2xl flex flex-col">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between shrink-0">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Versiones guardadas</h3>
              <button @click="showLyricsHistory = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 space-y-3 overflow-y-auto custom-scrollbar max-h-[calc(70vh-80px)]">
              <p v-if="trackVersions.length === 0" class="text-sm text-text-secondary italic text-center py-6">
                Esta pista solo tiene una versión almacenada (o ninguna).
              </p>
              <div v-for="version in trackVersions" :key="version.id" :class="['p-4 border rounded-xl transition-colors', version.id === currentLyrics?.id ? 'border-primary/60 bg-primary/5' : 'border-gray-200 dark:border-border-dark hover:border-primary/50']">
                <div class="flex items-start justify-between mb-2">
                  <div>
                    <span class="text-sm font-medium text-gray-900 dark:text-white">{{ version.source || 'unknown' }}</span>
                    <span :class="[
                      'ml-2 px-2 py-0.5 text-[10px] font-medium rounded',
                      version.format === 'lrc' || version.format === 'ttml' ? 'bg-blue-500/10 text-blue-500' : 'bg-gray-500/10 text-gray-500'
                    ]">{{ version.format }} · {{ version.sync_level ?? 'none' }}</span>
                  </div>
                  <span class="text-xs text-text-secondary">{{ version.created_at?.slice(0, 19).replace('T', ' ') }}</span>
                </div>
                <p class="text-xs text-text-secondary mb-3 line-clamp-2 whitespace-pre-line">{{ version.content.slice(0, 140) }}</p>
                <button v-if="version.id !== currentLyrics?.id" @click="restoreVersion(version)" class="w-full py-2 bg-primary/10 hover:bg-primary/20 text-primary rounded-lg text-xs font-medium transition-colors">
                  Usar esta versión
                </button>
                <p v-else class="text-center text-xs text-primary font-medium py-1">Versión actual</p>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog'
import { libraryApi } from '../api/library'
import { lyricsApi } from '../api/lyrics'
import { settingsApi } from '../api/settings'
import { toolsApi } from '../api/tools'
import { usePlayer } from '../composables/usePlayer'
import type { LibraryTrack, Lyrics, LyricsConfig, LyricsProviderSetting, LyricsSearchResult } from '../api/types'

// ==============================================
// SEARCH AND FILTER
// ==============================================

const searchQuery = ref('')
const filterType = ref('all')

// Selection
const selectedTrackId = ref<number | null>(null)
const selectedTracks = ref<number[]>([])

// UI State
const isEditing = ref(false)
const autoScroll = ref(true)
const showFetchDialog = ref(false)
// S192: transient error banner for the file-association flow
const importError = ref<string | null>(null)
const showBatchProgress = ref(false)
const isLoading = ref(false)
const isFetching = ref(false)

// Additional UI State for modals
const showQualityReport = ref(false)
const showProviderSettings = ref(false)
const showSyncEditor = ref(false)
const showLyricsHistory = ref(false)
const showFetchDropdown = ref(false)
const showExportDropdown = ref(false)

// Inline feedback banner (single-slot, auto-dismissed)
const actionMessage = ref<string | null>(null)
const actionMessageType = ref<'success' | 'error' | 'info'>('info')
let actionTimer: ReturnType<typeof setTimeout> | null = null
function notify(message: string, type: 'success' | 'error' | 'info' = 'info'): void {
  actionMessage.value = message
  actionMessageType.value = type
  if (actionTimer) clearTimeout(actionTimer)
  actionTimer = setTimeout(() => { actionMessage.value = null }, 5000)
}

// Real audio player singleton (syncify-media:// protocol)
const player = usePlayer()
const isPlayingAudio = computed(() => player.isPlaying.value)
const playbackPosition = computed(() => player.positionSec.value)
const playbackRateValue = computed(() => player.playbackRate.value)
const playerTrackId = computed(() => player.current.value?.id ?? null)

// Real data refs
const tracks = ref<LibraryTrack[]>([])
const currentLyrics = ref<Lyrics | null>(null)
const allLyrics = ref<Lyrics[]>([])
const lyricsStats = ref({
  total_tracks: 0,
  with_lyrics: 0,
  synced_lyrics: 0,
  embedded_lyrics: 0
})

// Derived coverage counters for the stats strip
const unsyncedCount = computed(() => Math.max(0, lyricsStats.value.with_lyrics - lyricsStats.value.synced_lyrics))
const noLyricsCount = computed(() => Math.max(0, lyricsStats.value.total_tracks - lyricsStats.value.with_lyrics))

// ==============================================
// LRC PARSING / SERIALIZATION UTILITIES
// ==============================================

interface ParsedLine { t: number; label: string; text: string }

const TIMESTAMP_RE = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g

/** Parses every `[mm:ss.xx] text` pair; multiple timestamps per line are expanded. */
function parseLrc(content: string): ParsedLine[] {
  const out: ParsedLine[] = []
  for (const raw of (content ?? '').split(/\r?\n/)) {
    TIMESTAMP_RE.lastIndex = 0
    const stamps: number[] = []
    let m: RegExpExecArray | null
    while ((m = TIMESTAMP_RE.exec(raw)) !== null) {
      const mm = parseInt(m[1], 10)
      const ss = parseInt(m[2], 10)
      // Fraction digits vary (1–3); normalize to milliseconds first.
      const fracRaw = m[3] ?? ''
      const fracMs = parseInt((fracRaw || '0').padEnd(3, '0').slice(0, 3), 10)
      stamps.push(mm * 60 + ss + fracMs / 1000)
    }
    if (stamps.length === 0) continue
    const text = raw.replace(TIMESTAMP_RE, '').trim()
    for (const t of stamps) {
      out.push({ t, label: formatClock(t), text })
    }
  }
  out.sort((a, b) => a.t - b.t)
  return out
}

function formatClock(sec: number): string {
  const s = Math.max(0, sec)
  const mm = Math.floor(s / 60)
  const rest = s - mm * 60
  const whole = Math.floor(rest)
  const cents = Math.round((rest - whole) * 100)
  return `${String(mm).padStart(2, '0')}:${String(whole).padStart(2, '0')}.${String(Math.min(cents, 99)).padStart(2, '0')}`
}

function stripTimestamps(content: string): string {
  return (content ?? '')
    .split(/\r?\n/)
    .map(l => l.replace(TIMESTAMP_RE, '').trim())
    .filter(l => l.length > 0 && !/^\[[a-z]+:/i.test(l))
    .join('\n')
}

/** Detects structural LRC problems: malformed stamps and non-monotonic order. */
function analyzeLrc(content: string): { valid: boolean; problems: string[] } {
  const problems: string[] = []
  const lines = (content ?? '').split(/\r?\n/)
  let prev = -1
  let stamped = 0
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]
    if (!/\[\d/.test(line)) continue
    const parsed = parseLrc(line)
    if (parsed.length === 0) {
      problems.push(`Línea ${i + 1}: timestamp malformado`)
      continue
    }
    stamped++
    for (const p of parsed) {
      if (p.t < prev) {
        problems.push(`Línea ${i + 1}: timestamp retrocede (${p.label})`)
        break
      }
      prev = p.t
    }
  }
  return { valid: problems.length === 0, problems }
}

/** True when the payload looks like a TTML document. */
function isTtml(content: string): boolean {
  const c = (content ?? '').trimStart().slice(0, 400).toLowerCase()
  return c.startsWith('<?xml') || c.includes('<tt ') || c.includes('<ttml')
}

function detectFormat(content: string): 'lrc' | 'plain' | 'ttml' {
  if (isTtml(content)) return 'ttml'
  if (parseLrc(content).length > 0) return 'lrc'
  return 'plain'
}

function buildTtml(lines: string[]): string {
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const body = lines.map(l => `      <p begin="unknown">${esc(l)}</p>`).join('\n')
  return `<?xml version="1.0" encoding="UTF-8"?>\n<tt xmlns="http://www.w3.org/ns/ttml">\n  <body>\n    <div>\n${body}\n    </div>\n  </body>\n</tt>\n`
}

function safeFileName(s: string): string {
  return (s || 'unknown').replace(/[\\/:*?"<>|]/g, '_').slice(0, 120)
}

// ==============================================
// FILTERED TRACK LIST
// ==============================================

const filteredTracks = computed(() => {
  let result = [...tracks.value]
  
  if (searchQuery.value) {
    const query = searchQuery.value.toLowerCase()
    result = result.filter(t => 
      t.title.toLowerCase().includes(query) || 
      (t.artist_name?.toLowerCase().includes(query) ?? false)
    )
  }
  
  switch (filterType.value) {
    case 'synced':
      result = result.filter(t => mapLyricsType(t.lyrics_type) === 'synced')
      break
    case 'unsynced':
      result = result.filter(t => mapLyricsType(t.lyrics_type) === 'unsynced')
      break
    case 'none':
      result = result.filter(t => mapLyricsType(t.lyrics_type) === 'none')
      break
    case 'downloaded':
      result = result.filter(t => t.download_status === 'downloaded')
      break
    case 'invalid-ts':
      result = result.filter(t => invalidTimestampTrackIds.value.has(t.id))
      break
    case 'missing-ts':
      result = result.filter(t => missingTimestampTrackIds.value.has(t.id))
      break
    case 'not-embedded':
      result = result.filter(t => notEmbeddedTrackIds.value.has(t.id))
      break
  }
  
  return result
})

// Current track
const currentTrack = computed(() => {
  const track = tracks.value.find(t => t.id === selectedTrackId.value)
  if (!track) return null;
  
  // Map to expected format for template
  return {
    ...track,
    artist: track.artist_name ?? 'Unknown Artist',
    album: track.album_name ?? 'Unknown Album',
    duration: formatDuration(track.duration_ms),
    filePath: track.file_path,
    coverUrl: track.cover_art_url,
    lyricsStatus: mapLyricsType(track.lyrics_type),
    syncLevel: currentLyrics.value?.sync_level ?? '',
    source: currentLyrics.value?.source ?? '',
    language: currentLyrics.value?.language ?? ''
  }
})

// Parsed synced lyrics lines
const syncedLyrics = computed<ParsedLine[]>(() => {
  if (!currentLyrics.value || currentLyrics.value.format !== 'lrc') return []
  return parseLrc(currentLyrics.value.content)
})

// Unsynced lyrics paragraphs
const unsyncedParagraphs = computed<string[]>(() => {
  if (!currentLyrics.value) return []
  if (currentLyrics.value.format === 'lrc') return []
  const content = currentLyrics.value.format === 'ttml' ? extractTtmlText(currentLyrics.value.content) : currentLyrics.value.content
  if (content.includes('\n\n')) {
    return content.split('\n\n').map(p => p.trim()).filter(Boolean)
  }
  return content.split('\n').map(l => l.trimEnd())
})

/** Minimal TTML → text extraction for viewing/editing fallback. */
function extractTtmlText(xml: string): string {
  return xml
    .replace(/<[^p][^>]*>/gi, '')
    .replace(/<\/p>/gi, '\n')
    .replace(/<p[^>]*begin="([^"]*)"[^>]*>/gi, (_, _b) => '')
    .trim()
}

// ==============================================
// PLAYBACK INTEGRATION
// ==============================================

const effectiveDurationSec = computed(() => {
  if (player.durationSec.value > 0 && playerTrackId.value === selectedTrackId.value) return player.durationSec.value
  return (currentTrack.value?.duration_ms ?? 0) / 1000
})

const progressPercent = computed(() => {
  const dur = effectiveDurationSec.value
  if (dur <= 0) return 0
  return Math.min(100, Math.max(0, (playbackPosition.value / dur) * 100))
})

const currentTimeLabel = computed(() => formatDurationSec(playbackPosition.value))
const totalTimeLabel = computed(() => formatDurationSec(effectiveDurationSec.value))

function formatDurationSec(sec: number): string {
  const s = Math.max(0, Math.floor(sec))
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

const activeLineIndex = computed(() => {
  const lines = syncedLyrics.value
  if (lines.length === 0) return -1
  const pos = playbackPosition.value
  let idx = -1
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].t <= pos + 0.05) idx = i
    else break
  }
  return idx
})

// Auto-scroll the active line into view
const lineRefs = new Map<number, HTMLElement>()
function setLineRef(el: unknown, index: number): void {
  if (el instanceof HTMLElement) lineRefs.set(index, el)
}
watch(activeLineIndex, async (idx) => {
  if (!autoScroll.value || idx < 0) return
  await nextTick()
  const el = lineRefs.get(idx)
  // jsdom (tests) does not implement scrollIntoView
  if (el && typeof el.scrollIntoView === 'function') {
    el.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }
})

async function togglePlayback(): Promise<void> {
  if (!currentTrack.value) return
  if (playerTrackId.value === currentTrack.value.id) {
    player.toggle()
    return
  }
  await playCurrentTrack()
}

async function playCurrentTrack(): Promise<void> {
  const track = currentTrack.value
  if (!track) return
  if (!track.filePath) {
    notify('Esta pista no tiene archivo descargado; la vista de letra funciona sin reproducción.', 'info')
    return
  }
  try {
    await player.play({
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: track.album,
      coverUrl: track.coverUrl,
    })
  } catch (err) {
    notify(err instanceof Error ? err.message : String(err), 'error')
  }
}

function seekToLine(index: number): void {
  const line = syncedLyrics.value[index]
  if (!line) return
  player.seek(line.t)
}

function onSeekPercent(pct: number): void {
  const dur = effectiveDurationSec.value
  if (dur > 0) player.seek((pct / 100) * dur)
}

// ==============================================
// DATA LOADING
// ==============================================

async function loadTracks() {
  isLoading.value = true
  try {
    const result = await libraryApi.getLibrary(0, 500)
    tracks.value = result.tracks
  } catch (err) {
    console.error('Failed to load tracks:', err)
  } finally {
    isLoading.value = false
  }
}

async function loadLyricsStats() {
  try {
    lyricsStats.value = await lyricsApi.getLyricsStats()
  } catch (err) {
    console.error('Failed to load lyrics stats:', err)
  }
}

async function loadAllLyrics(): Promise<Lyrics[]> {
  try {
    allLyrics.value = await lyricsApi.getAllLyrics({ limit: 5000 })
  } catch (err) {
    console.error('Failed to load all lyrics:', err)
    allLyrics.value = []
  }
  return allLyrics.value
}

/** Tracks already probed on disk this session (avoids re-reading files). */
const diskProbedTracks = new Set<number>()

async function loadTrackLyrics(trackId: number) {
  try {
    currentLyrics.value = await lyricsApi.getLyrics(trackId)
    // S200: nothing in the DB → probe the local file (embedded FLAC tags or a
    // sidecar next to the audio). Persists what it finds for next time.
    // Each track is probed at most once per session.
    if (!currentLyrics.value && !diskProbedTracks.has(trackId)) {
      diskProbedTracks.add(trackId)
      try {
        const probed = await lyricsApi.probeTrackLyrics(trackId)
        if (probed && probed.content) {
          currentLyrics.value = probed
          notify(
            probed.source === 'sidecar'
              ? 'Letra encontrada como archivo .lrc junto al audio'
              : 'Letra encontrada incrustada en el archivo',
            'success'
          )
          // Refresh the list so lyrics_type / coverage reflect the find.
          await Promise.all([loadTracks(), loadLyricsStats()])
        }
      } catch {
        /* sin archivo descargado o sin letras locales — silencioso */
      }
    }
    if (currentLyrics.value) {
      editableLyrics.value = currentLyrics.value.content
    } else {
      editableLyrics.value = ''
    }
  } catch (err) {
    console.error('Failed to load lyrics for track:', trackId, err)
    currentLyrics.value = null
  }
}

/** S200: sweep the disk for lyrics this app never saw (sidecars + embedded). */
const isHarvestingLyrics = ref(false)
async function scanDiskForLyrics() {
  if (isHarvestingLyrics.value) return
  isHarvestingLyrics.value = true
  try {
    const r = await lyricsApi.harvestMissingLyrics()
    const found = r.sidecar_found + r.embedded_found
    notify(
      found > 0
        ? `Escaneo: ${found} letras recuperadas (${r.sidecar_found} sidecar, ${r.embedded_found} incrustadas) de ${r.scanned} pistas`
        : `Escaneo completo: ninguna letra nueva en ${r.scanned} pistas`,
      found > 0 ? 'success' : 'info'
    )
    await Promise.all([loadTracks(), loadLyricsStats()])
  } catch (err) {
    console.error('Harvest failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isHarvestingLyrics.value = false
  }
}

// ==============================================
// SINGLE-TRACK ACTIONS
// ==============================================

/** Direct auto-fetch (best-effort, first acceptable provider hit). */
async function runAutoFetch() {
  if (!selectedTrackId.value) return
  isFetching.value = true
  try {
    const result = await lyricsApi.fetchLyrics(selectedTrackId.value)
    if (result) {
      currentLyrics.value = result
      editableLyrics.value = result.content
      notify(`Letra obtenida desde ${result.source ?? 'proveedor'}`, 'success')
    } else {
      notify('Ningún proveedor tiene letra para esta pista.', 'error')
    }
    await Promise.all([loadTracks(), loadLyricsStats()])
  } catch (err) {
    console.error('Failed to fetch lyrics:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isFetching.value = false
  }
}

/** S192: associate an external .lrc / .txt file with the selected track. */
const isImportingLyricsFile = ref(false)
async function associateLyricsFile() {
  if (!selectedTrackId.value || isImportingLyricsFile.value) return
  try {
    const selection = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: 'Letras', extensions: ['lrc', 'txt'] }]
    })
    if (!selection || Array.isArray(selection)) return
    isImportingLyricsFile.value = true
    const result = await lyricsApi.importLyricsFile(selectedTrackId.value, selection)
    currentLyrics.value = result
    editableLyrics.value = result.content
    isEditing.value = false
    await Promise.all([loadTracks(), loadLyricsStats()])
    notify('Archivo de letras asociado correctamente', 'success')
  } catch (err) {
    console.error('Failed to associate lyrics file:', err)
    importError.value = err instanceof Error ? err.message : String(err)
    setTimeout(() => { importError.value = null }, 6000)
  } finally {
    isImportingLyricsFile.value = false
  }
}

/** Embed the current lyrics into the downloaded FLAC. */
const isEmbeddingSingle = ref(false)
async function embedSingleLyrics() {
  if (!selectedTrackId.value || !currentLyrics.value || isEmbeddingSingle.value) return
  isEmbeddingSingle.value = true
  try {
    await lyricsApi.embedLyrics(selectedTrackId.value)
    currentLyrics.value = await lyricsApi.getLyrics(selectedTrackId.value)
    await loadLyricsStats()
    notify('Letra embebida y verificada en el archivo', 'success')
  } catch (err) {
    console.error('Failed to embed lyrics:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isEmbeddingSingle.value = false
  }
}

/** Export the current lyrics in their native format (.lrc/.txt/.ttml). */
async function exportCurrentLyrics() {
  const track = currentTrack.value
  const lyr = currentLyrics.value
  if (!track || !lyr) return
  const libTrack = tracks.value.find(t => t.id === track.id)
  if (!libTrack) return
  await exportLyricsRecords([{ track: libTrack, lyrics: lyr }], detectExportFormat(lyr.format))
}

async function deleteTrackLyrics() {
  if (!selectedTrackId.value) return
  try {
    await lyricsApi.deleteLyrics(selectedTrackId.value)
    currentLyrics.value = null
    editableLyrics.value = ''
    await Promise.all([loadTracks(), loadLyricsStats()])
    notify('Letra eliminada', 'success')
  } catch (err) {
    console.error('Failed to delete lyrics:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  }
}

// ==============================================
// EDITOR
// ==============================================

const editableLyrics = ref('')
const isSavingEdit = ref(false)
const validationMessage = ref<string | null>(null)
const validationOk = ref(false)

const editorFormat = computed<'lrc' | 'plain' | 'ttml'>(() => detectFormat(editableLyrics.value))

function startEditing() {
  validationMessage.value = null
  isEditing.value = true
}

function validateEditor() {
  const fmt = editorFormat.value
  if (fmt !== 'lrc') {
    validationOk.value = true
    validationMessage.value = fmt === 'plain'
      ? 'Texto plano válido: no lleva timestamps.'
      : 'Documento TTML detectado: se guarda tal cual.'
    return
  }
  const analysis = analyzeLrc(editableLyrics.value)
  validationOk.value = analysis.valid
  validationMessage.value = analysis.valid
    ? `LRC válido · ${parseLrc(editableLyrics.value).length} líneas sincronizadas`
    : `Problemas encontrados:\n· ${analysis.problems.slice(0, 8).join('\n· ')}`
}

async function saveLyricsEdit() {
  if (!selectedTrackId.value || !editableLyrics.value.trim()) return
  isSavingEdit.value = true
  try {
    const fmt = detectFormat(editableLyrics.value)
    const prevSync = currentLyrics.value?.sync_level
    const result = await lyricsApi.saveLyrics({
      trackId: selectedTrackId.value,
      format: fmt,
      content: editableLyrics.value,
      syncLevel: fmt === 'lrc' ? (prevSync === 'word' ? 'word' : 'line') : 'none',
      source: currentLyrics.value?.source === 'manual' ? 'manual' : (currentLyrics.value?.source ?? 'manual'),
      language: currentLyrics.value?.language ?? undefined,
    })
    currentLyrics.value = result
    isEditing.value = false
    await Promise.all([loadTracks(), loadLyricsStats()])
    notify('Letra guardada', 'success')
  } catch (err) {
    console.error('Failed to save lyrics:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isSavingEdit.value = false
  }
}

function onGlobalKeydown(e: KeyboardEvent): void {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    if (showSyncEditor.value) {
      e.preventDefault()
      void saveSyncEditor()
    } else if (isEditing.value) {
      e.preventDefault()
      void saveLyricsEdit()
    }
  }
  if (showSyncEditor.value) {
    onSyncEditorShortcuts(e)
  }
}

// ==============================================
// FETCH DIALOG
// ==============================================

const isSearching = ref(false)
const searchError = ref<string | null>(null)
const isApplyingResult = ref(false)
const lyricsResults = ref<LyricsSearchResult[]>([])

function openFetchDialog() {
  lyricsResults.value = []
  searchError.value = null
  showFetchDialog.value = true
  void runLyricsSearch()
}

async function runLyricsSearch() {
  const track = currentTrack.value
  if (!track) return
  isSearching.value = true
  searchError.value = null
  try {
    lyricsResults.value = await lyricsApi.searchLyrics({
      title: track.title,
      artist: track.artist,
      album: track.album === 'Unknown Album' ? undefined : track.album,
      durationMs: track.duration_ms ?? undefined,
    })
  } catch (err) {
    console.error('Lyrics search failed:', err)
    searchError.value = err instanceof Error ? err.message : String(err)
  } finally {
    isSearching.value = false
  }
}

function resultPreview(result: LyricsSearchResult): string {
  const content = result.synced_lyrics ?? result.plain_lyrics ?? ''
  if (content) return stripTimestamps(content).split('\n').slice(0, 3).join('\n')
  return result.instrumental ? 'Pista instrumental (sin texto vocal).' : 'Sin vista previa disponible.'
}

async function applySearchResult(result: LyricsSearchResult) {
  if (!selectedTrackId.value || isApplyingResult.value) return
  isApplyingResult.value = true
  try {
    const content = result.synced_lyrics || result.plain_lyrics || ''
    if (!content) {
      if (result.instrumental) {
        const saved = await lyricsApi.saveLyrics({
          trackId: selectedTrackId.value,
          format: 'plain',
          content: '♪ Instrumental ♪',
          syncLevel: 'none',
          source: result.source,
        })
        currentLyrics.value = saved
      } else {
        notify('El resultado no contiene contenido utilizable.', 'error')
        return
      }
    } else {
      const saved = await lyricsApi.saveLyrics({
        trackId: selectedTrackId.value,
        format: result.synced_lyrics ? 'lrc' : 'plain',
        content,
        syncLevel: result.sync_type === 'WORD_SYNCED' ? 'word' : result.synced_lyrics ? 'line' : 'none',
        source: result.source,
      })
      currentLyrics.value = saved
    }
    editableLyrics.value = currentLyrics.value?.content ?? ''
    showFetchDialog.value = false
    await Promise.all([loadTracks(), loadLyricsStats()])
    notify(`Letra aplicada desde ${result.source}`, 'success')
  } catch (err) {
    console.error('Failed to apply search result:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isApplyingResult.value = false
  }
}

function enterManually() {
  showFetchDialog.value = false
  editableLyrics.value = ''
  startEditing()
}

// ==============================================
// BATCH OPERATIONS
// ==============================================

type FetchMode = 'prefer_synced' | 'any' | 'synced_only'

const batchProgress = ref({
  current: 0,
  total: 0,
  currentTrack: '',
  success: 0,
  failed: 0,
  skipped: 0,
})
const isUpgrading = ref(false)
const isEmbeddingBatch = ref(false)

/** Persists the user's sync preference and starts the batch fetch. */
async function applyFetchMode(mode: FetchMode) {
  showFetchDropdown.value = false
  const levels: Record<FetchMode, string> = { prefer_synced: 'line', any: 'none', synced_only: 'word' }
  if (providerForm.value) {
    providerForm.value.min_sync_level = levels[mode]
    try {
      const saved = await settingsApi.updateLyricsConfig(providerForm.value)
      providerForm.value = { ...saved }
    } catch (err) {
      console.warn('Failed to persist lyrics preference:', err)
    }
  }
  await batchFetchSelectedLyrics(mode)
}

async function batchFetchSelectedLyrics(mode: FetchMode = 'prefer_synced') {
  const targets = selectedTracks.value.length > 0 ? [...selectedTracks.value] : selectedTrackId.value ? [selectedTrackId.value] : []
  if (targets.length === 0) return
  showBatchProgress.value = true
  batchProgress.value = {
    current: 0,
    total: targets.length,
    currentTrack: '',
    success: 0,
    failed: 0,
    skipped: 0,
  }
  
  // Subscribe to progress events
  let unlisten: UnlistenFn | null = null
  try {
    unlisten = await listen<{
      status: string
      current: number
      total: number
      track: string
      message: string
    }>('lyrics-fetch-progress', (event) => {
      const payload = event.payload
      batchProgress.value.current = payload.current
      batchProgress.value.total = payload.total
      batchProgress.value.currentTrack = payload.track
      if (payload.status === 'found') {
        batchProgress.value.success++
      } else if (payload.status === 'error' || payload.status === 'not_found') {
        batchProgress.value.failed++
      } else if (payload.status === 'skipped') {
        batchProgress.value.skipped++
      }
    })
    
    const result = await lyricsApi.batchFetchLyricsWithProgress(targets)
    batchProgress.value.success = result.fetched
    batchProgress.value.failed = result.failed
    batchProgress.value.skipped = result.skipped

    // Enforce "Synced only": drop any freshly fetched unsynced payloads.
    if (mode === 'synced_only') {
      let dropped = 0
      for (const id of targets) {
        const lyr = await lyricsApi.getLyrics(id)
        if (lyr && lyr.format !== 'lrc' && lyr.source !== 'manual' && lyr.source !== 'manual_import') {
          await lyricsApi.deleteLyrics(id)
          dropped++
        }
      }
      if (dropped > 0) {
        notify(`${dropped} resultado(s) unsynced descartados por la preferencia «Synced only»`, 'info')
      }
    }

    await refreshAfterBatch()
  } catch (err) {
    console.error('Batch fetch failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    // Clean up listener
    if (unlisten) unlisten()
    showBatchProgress.value = false
    clearSelection()
  }
}

async function refreshAfterBatch() {
  await Promise.all([loadTracks(), loadLyricsStats()])
  if (selectedTrackId.value) await loadTrackLyrics(selectedTrackId.value)
}

/** Re-fetches unsynced tracks aiming for a synced payload. */
async function upgradeSelectedToSynced() {
  const targets = selectedTracks.value.length > 0 ? [...selectedTracks.value] : []
  if (targets.length === 0) return
  isUpgrading.value = true
  let upgraded = 0
  let alreadySynced = 0
  let failed = 0
  try {
    for (const id of targets) {
      const cur = await lyricsApi.getLyrics(id)
      if (cur && (cur.format === 'lrc' || cur.format === 'ttml')) {
        alreadySynced++
        continue
      }
      const res = await lyricsApi.fetchLyrics(id)
      const after = res ?? await lyricsApi.getLyrics(id)
      if (after && (after.format === 'lrc' || after.format === 'ttml')) upgraded++
      else failed++
    }
    await refreshAfterBatch()
    notify(`Upgrade terminado: ${upgraded} sincronizadas, ${alreadySynced} ya lo estaban, ${failed} sin cambios`, upgraded > 0 ? 'success' : 'info')
  } catch (err) {
    console.error('Upgrade failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isUpgrading.value = false
    clearSelection()
  }
}

async function embedSelectedLyrics() {
  if (selectedTracks.value.length === 0) return
  isEmbeddingBatch.value = true
  try {
    const res = await lyricsApi.batchEmbedLyrics(selectedTracks.value)
    await loadLyricsStats()
    notify(`Embed: ${res.embedded} ok, ${res.failed} fallidos, ${res.skipped} omitidos`, res.embedded > 0 ? 'success' : 'info')
  } catch (err) {
    console.error('Batch embed failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isEmbeddingBatch.value = false
  }
}

async function deleteSelectedLyrics() {
  if (selectedTracks.value.length === 0) return
  try {
    for (const trackId of selectedTracks.value) {
      await lyricsApi.deleteLyrics(trackId)
    }
    await refreshAfterBatch()
    notify('Letras eliminadas de las pistas seleccionadas', 'success')
    clearSelection()
  } catch (err) {
    console.error('Failed to delete selected lyrics:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  }
}

// ==============================================
// EXPORT (single + batch, native formats)
// ==============================================

interface ExportItem { track: LibraryTrack; lyrics: Lyrics }

async function collectSelectedForExport(): Promise<ExportItem[]> {
  const ids = selectedTracks.value.length > 0 ? [...selectedTracks.value] : selectedTrackId.value ? [selectedTrackId.value] : []
  const items: ExportItem[] = []
  for (const id of ids) {
    const track = tracks.value.find(t => t.id === id)
    if (!track) continue
    const lyr = await lyricsApi.getLyrics(id)
    if (lyr) items.push({ track, lyrics: lyr })
  }
  return items
}

function buildExportContent(item: ExportItem, fmt: 'lrc' | 'txt' | 'ttml'): string | null {
  if (fmt === 'lrc') {
    return item.lyrics.format === 'lrc' ? item.lyrics.content : null
  }
  if (fmt === 'ttml') {
    if (item.lyrics.format === 'ttml') return item.lyrics.content
    const lines = item.lyrics.format === 'lrc'
      ? parseLrc(item.lyrics.content).map(l => l.text)
      : stripTimestamps(item.lyrics.content).split('\n')
    return buildTtml(lines.filter(Boolean))
  }
  // txt
  return item.lyrics.format === 'lrc' ? stripTimestamps(item.lyrics.content) : extractTtmlText(item.lyrics.content) || item.lyrics.content
}

/** Native export format for a stored lyrics record. */
function detectExportFormat(format: Lyrics['format']): 'lrc' | 'txt' | 'ttml' {
  if (format === 'lrc') return 'lrc'
  if (format === 'ttml') return 'ttml'
  return 'txt'
}

/** Writes the given records to disk through the dialog-resolved destinations. */
async function exportLyricsRecords(items: ExportItem[], fmt: 'lrc' | 'txt' | 'ttml'): Promise<void> {
  const ext = fmt === 'txt' ? 'txt' : fmt
  const built: { name: string; content: string }[] = []
  let skipped = 0
  for (const item of items) {
    const content = buildExportContent(item, fmt)
    if (!content || !content.trim()) { skipped++; continue }
    built.push({
      name: `${safeFileName(item.track.artist_name ?? '')} - ${safeFileName(item.track.title)}.${ext}`,
      content,
    })
  }
  if (built.length === 0) {
    notify(`Ninguna pista tiene letra en formato ${fmt.toUpperCase()} exportable.`, 'info')
    return
  }

  try {
    if (built.length === 1) {
      const target = await saveDialog({
        defaultPath: built[0].name,
        filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
      })
      if (!target) return
      await toolsApi.writeTextFile(target, built[0].content)
      notify(`Exportado: ${target}`, 'success')
    } else {
      const dir = await openDialog({ directory: true, multiple: false })
      if (!dir || Array.isArray(dir)) return
      let written = 0
      for (const f of built) {
        await toolsApi.writeTextFile(`${dir}/${f.name}`, f.content)
        written++
      }
      notify(`${written} archivos exportados a ${dir}${skipped ? ` (${skipped} omitidos)` : ''}`, 'success')
    }
  } catch (err) {
    console.error('Lyrics export failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  }
}

async function exportSelectedLyrics(fmt: 'lrc' | 'txt' | 'ttml') {
  showExportDropdown.value = false
  const items = await collectSelectedForExport()
  if (items.length === 0) {
    notify('Selecciona pistas con letra para exportar.', 'info')
    return
  }
  await exportLyricsRecords(items, fmt)
}

// ==============================================
// QUALITY REPORT (real numbers)
// ==============================================

const isReportLoading = ref(false)
const isRunningAction = ref(false)

const invalidTimestampTrackIds = computed(() => {
  const ids = new Set<number>()
  for (const lyr of allLyrics.value) {
    if (lyr.format !== 'lrc') continue
    const analysis = analyzeLrc(lyr.content)
    if (!analysis.valid) ids.add(lyr.track_id)
  }
  return ids
})

const missingTimestampTrackIds = computed(() => {
  const ids = new Set<number>()
  for (const lyr of allLyrics.value) {
    if (lyr.format !== 'lrc') continue
    if (parseLrc(lyr.content).length === 0) ids.add(lyr.track_id)
  }
  return ids
})

const notEmbeddedTrackIds = computed(() => {
  const ids = new Set<number>()
  const downloaded = new Map(tracks.value.map(t => [t.id, t.download_status === 'downloaded']))
  for (const lyr of allLyrics.value) {
    if (lyr.embedded_in_file) continue
    if (downloaded.get(lyr.track_id)) ids.add(lyr.track_id)
  }
  return ids
})

async function openQualityReport() {
  showQualityReport.value = true
  isReportLoading.value = true
  await Promise.all([loadAllLyrics(), loadLyricsStats()])
  isReportLoading.value = false
}

const reportData = computed(() => {
  const total = lyricsStats.value.total_tracks
  const synced = lyricsStats.value.synced_lyrics
  const withLyrics = lyricsStats.value.with_lyrics
  const unsynced = Math.max(0, withLyrics - synced)
  const none = Math.max(0, total - withLyrics)
  const pct = (n: number) => (total > 0 ? Math.round((n / total) * 100) : 0)

  const levelOf = (l: Lyrics) => l.sync_level ?? 'line'
  const syncedRecords = allLyrics.value.filter(l => l.format === 'lrc' || l.format === 'ttml')

  return {
    total,
    withLyrics,
    synced,
    unsynced,
    none,
    syncedPct: pct(synced),
    unsyncedPct: pct(unsynced),
    nonePct: pct(none),
    syllable: syncedRecords.filter(l => levelOf(l) === 'syllable').length,
    word: syncedRecords.filter(l => levelOf(l) === 'word').length,
    line: syncedRecords.filter(l => levelOf(l) === 'line' || levelOf(l) === 'none').length,
    invalidTs: invalidTimestampTrackIds.value.size,
    missingTs: missingTimestampTrackIds.value.size,
    notEmbedded: notEmbeddedTrackIds.value.size,
  }
})

function filterByIssue(kind: 'invalid-ts' | 'missing-ts' | 'not-embedded') {
  filterType.value = kind
  showQualityReport.value = false
}

async function runFetchMissing(limit = 100) {
  isRunningAction.value = true
  try {
    const res = await lyricsApi.fetchMissingLyrics(limit)
    await refreshAfterBatch()
    notify(`Fetch masivo: ${res.fetched} encontradas, ${res.failed} sin resultado`, res.fetched > 0 ? 'success' : 'info')
  } catch (err) {
    console.error('fetch_missing_lyrics failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isRunningAction.value = false
  }
}

async function runUpgradeUnsynced(cap = 100) {
  isRunningAction.value = true
  try {
    const targets = tracks.value
      .filter(t => mapLyricsType(t.lyrics_type) === 'unsynced')
      .slice(0, cap)
      .map(t => t.id)
    let upgraded = 0
    for (const id of targets) {
      const res = await lyricsApi.fetchLyrics(id)
      const after = res ?? await lyricsApi.getLyrics(id)
      if (after && (after.format === 'lrc' || after.format === 'ttml')) upgraded++
    }
    await refreshAfterBatch()
    notify(`Upgrade: ${upgraded}/${targets.length} pistas ahora sincronizadas`, upgraded > 0 ? 'success' : 'info')
  } catch (err) {
    console.error('Upgrade unsynced failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isRunningAction.value = false
  }
}

async function embedAllEligible(cap = 100) {
  isRunningAction.value = true
  try {
    const targets = [...notEmbeddedTrackIds.value].slice(0, cap)
    const res = await lyricsApi.batchEmbedLyrics(targets)
    await loadAllLyrics()
    await loadLyricsStats()
    notify(`Embed: ${res.embedded} ok, ${res.failed} fallidos`, res.embedded > 0 ? 'success' : 'info')
  } catch (err) {
    console.error('Massive embed failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isRunningAction.value = false
  }
}

async function autoFixAll() {
  isRunningAction.value = true
  try {
    const fetched = await lyricsApi.fetchMissingLyrics(50)
    await loadTracks()
    const upgraded = await runUpgradeUnsyncedQuiet(50)
    await loadAllLyrics()
    await loadLyricsStats()
    notify(`Auto-Fix: ${fetched.fetched} letras nuevas, ${upgraded} upgrades a synced`, 'success')
  } catch (err) {
    console.error('Auto-fix failed:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isRunningAction.value = false
  }
}

async function runUpgradeUnsyncedQuiet(cap: number): Promise<number> {
  const targets = tracks.value
    .filter(t => mapLyricsType(t.lyrics_type) === 'unsynced')
    .slice(0, cap)
    .map(t => t.id)
  let upgraded = 0
  for (const id of targets) {
    const res = await lyricsApi.fetchLyrics(id)
    const after = res ?? await lyricsApi.getLyrics(id)
    if (after && (after.format === 'lrc' || after.format === 'ttml')) upgraded++
  }
  return upgraded
}

// ==============================================
// PROVIDER SETTINGS (real backend config)
// ==============================================

const providers = ref<LyricsProviderSetting[]>([])
const providerForm = ref<LyricsConfig | null>(null)
const testingProviderId = ref<string | null>(null)
const testResults = ref<Record<string, boolean>>({})
const isSavingProviders = ref(false)

function defaultConfig(): LyricsConfig {
  return {
    id: 1,
    min_sync_level: 'none',
    preferred_language: '',
    storage_format: 'lrc',
    auto_fetch_on_import: false,
    retry_failed: false,
    retry_frequency: 'always',
  }
}

async function loadProviderSettings() {
  try {
    providers.value = await settingsApi.getLyricsProviders()
  } catch (err) {
    console.error('Failed to load lyrics providers:', err)
    providers.value = []
  }
  try {
    providerForm.value = { ...defaultConfig(), ...(await settingsApi.getLyricsConfig()) }
  } catch (err) {
    console.warn('Lyrics config unavailable, using defaults:', err)
    providerForm.value = defaultConfig()
  }
}

async function toggleProvider(provider: LyricsProviderSetting) {
  const next = !provider.enabled
  provider.enabled = next
  try {
    const saved = await settingsApi.updateLyricsProvider(provider.provider_id, next, provider.priority)
    Object.assign(provider, saved)
  } catch (err) {
    provider.enabled = !next
    console.error('Failed to toggle provider:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  }
}

async function moveProvider(index: number, dir: -1 | 1) {
  const target = index + dir
  if (target < 0 || target >= providers.value.length) return
  const ids = providers.value.map(p => p.provider_id)
  const tmp = ids[index]
  ids[index] = ids[target]
  ids[target] = tmp
  try {
    providers.value = await settingsApi.reorderLyricsProviders(ids)
  } catch (err) {
    console.error('Failed to reorder providers:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  }
}

async function testProvider(provider: LyricsProviderSetting) {
  testingProviderId.value = provider.provider_id
  try {
    testResults.value[provider.provider_id] = await settingsApi.testLyricsProvider(provider.provider_id)
  } catch {
    testResults.value[provider.provider_id] = false
  } finally {
    testingProviderId.value = null
  }
}

async function saveProviderSettings() {
  if (!providerForm.value) return
  isSavingProviders.value = true
  try {
    providerForm.value = { ...(await settingsApi.updateLyricsConfig(providerForm.value)) }
    notify('Preferencias de letras guardadas', 'success')
    showProviderSettings.value = false
  } catch (err) {
    console.error('Failed to save lyrics config:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isSavingProviders.value = false
  }
}

// ==============================================
// HISTORY (stored versions across formats)
// ==============================================

const trackVersions = computed<Lyrics[]>(() => {
  if (!selectedTrackId.value) return []
  return allLyrics.value
    .filter(l => l.track_id === selectedTrackId.value)
    .sort((a, b) => (b.created_at ?? '').localeCompare(a.created_at ?? ''))
})

async function openHistory() {
  showLyricsHistory.value = true
  if (allLyrics.value.length === 0) await loadAllLyrics()
}

async function restoreVersion(version: Lyrics) {
  currentLyrics.value = version
  editableLyrics.value = version.content
  isEditing.value = false
  showLyricsHistory.value = false
  notify(`Versión ${version.format} de ${version.created_at?.slice(0, 10) ?? 'archivo'} activada`, 'success')
}

// ==============================================
// ADVANCED SYNC EDITOR
// ==============================================

interface SyncLine { t: number | null; text: string }
interface MarkedSyncEntry { line: SyncLine & { t: number }; index: number }
const syncLines = ref<SyncLine[]>([])
const syncSelectedIdx = ref(0)
const shiftSeconds = ref(0)
const isSavingSync = ref(false)
const waveformBars = Array.from({ length: 80 }, (_, i) => 25 + Math.abs(Math.sin(i * 1.7) * 55) + (i % 7) * 3)

/** Marked lines with their position in the full list, so timestamps can be cleared safely. */
const markedSyncEntries = computed<MarkedSyncEntry[]>(() => {
  const out: MarkedSyncEntry[] = []
  syncLines.value.forEach((line, index) => {
    if (line.t !== null) out.push({ line: line as SyncLine & { t: number }, index })
  })
  return out
})

const waveRulerTicks = computed(() => {
  const dur = Math.max(effectiveDurationSec.value, 1)
  const n = 8
  return Array.from({ length: n }, (_, i) => (dur * i) / (n - 1))
})

function linePct(sec: number): number {
  const dur = effectiveDurationSec.value
  if (dur <= 0) return 0
  return Math.min(100, Math.max(0, (sec / dur) * 100))
}

function openSyncEditor() {
  if (!currentTrack.value) return
  const existing = syncedLyrics.value
  if (existing.length > 0) {
    syncLines.value = existing.map(l => ({ t: l.t, text: l.text }))
  } else {
    const source = editableLyrics.value.trim().length > 0
      ? stripTimestamps(editableLyrics.value).split('\n')
      : ['']
    syncLines.value = source.map(l => ({ t: null, text: l }))
  }
  syncSelectedIdx.value = 0
  showSyncEditor.value = true
}

function addSyncLine() {
  syncLines.value.push({ t: null, text: '' })
  syncSelectedIdx.value = syncLines.value.length - 1
}

function markLineAtPlayhead(index?: number) {
  const idx = index ?? syncSelectedIdx.value
  const line = syncLines.value[idx]
  if (!line) return
  line.t = Math.max(0, Math.round(playbackPosition.value * 100) / 100)
  if (idx < syncLines.value.length - 1) syncSelectedIdx.value = idx + 1
}

function clearLineTimestamp(index: number) {
  const line = syncLines.value[index]
  if (line) line.t = null
}

function clearAllTimestamps() {
  for (const l of syncLines.value) l.t = null
  notify('Todos los timestamps fueron limpiados', 'info')
}

function shiftAllTimestamps() {
  const delta = Number(shiftSeconds.value)
  if (!Number.isFinite(delta) || delta === 0) return
  for (const l of syncLines.value) {
    if (l.t !== null) l.t = Math.max(0, Math.round((l.t + delta) * 100) / 100)
  }
}

function snapToGrid() {
  for (const l of syncLines.value) {
    if (l.t !== null) l.t = Math.round(l.t * 2) / 2
  }
}

function validateSyncTiming() {
  const marked = markedSyncEntries.value
  if (marked.length === 0) {
    notify('No hay timestamps marcados todavía', 'info')
    return
  }
  let bad = 0
  for (let i = 1; i < marked.length; i++) {
    if (marked[i].line.t < marked[i - 1].line.t) bad++
  }
  notify(bad === 0
    ? `Timing válido: ${marked.length} marcas en orden monótono ✓`
    : `${bad} salto(s) hacia atrás detectados — revisa las líneas marcadas`,
    bad === 0 ? 'success' : 'error')
}

function onSyncTimeInput(index: number, value: string) {
  const line = syncLines.value[index]
  if (!line) return
  const m = value.trim().match(/^(\d{1,2}):(\d{2})(?:\.(\d{1,3}))?$/)
  if (!m) {
    notify(`Timestamp inválido: "${value}" (usa mm:ss.xx)`, 'error')
    return
  }
  const fracRaw = m[3] ?? '0'
  const frac = parseInt(fracRaw.padEnd(3, '0').slice(0, 3), 10) / 1000
  line.t = parseInt(m[1], 10) * 60 + parseInt(m[2], 10) + frac
}

function onSyncEditorKeydown(_e: KeyboardEvent) {
  // Handled centrally in onGlobalKeydown (see below) so shortcuts work
  // even when focus sits on an inner input.
}

function onSyncEditorShortcuts(e: KeyboardEvent) {
  const tag = (e.target as HTMLElement)?.tagName
  const inTextField = tag === 'INPUT' || tag === 'TEXTAREA'

  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') return // handled elsewhere
  // Never hijack typing keys while the user edits a field
  if (inTextField && e.key !== 'Escape') {
    if (e.key === ' ' || e.key === 'Enter' || e.key === 'Backspace' || e.key.startsWith('Arrow')) return
  }
  switch (e.key) {
    case ' ':
      e.preventDefault()
      markLineAtPlayhead()
      break
    case 'Enter':
      e.preventDefault()
      syncSelectedIdx.value = Math.min(syncSelectedIdx.value + 1, syncLines.value.length - 1)
      break
    case 'Backspace': {
      e.preventDefault()
      const entries = markedSyncEntries.value
      if (entries.length > 0) {
        syncLines.value[entries[entries.length - 1].index].t = null
      }
      break
    }
    case 'ArrowDown':
      e.preventDefault()
      syncSelectedIdx.value = Math.min(syncSelectedIdx.value + 1, syncLines.value.length - 1)
      break
    case 'ArrowUp':
      e.preventDefault()
      syncSelectedIdx.value = Math.max(syncSelectedIdx.value - 1, 0)
      break
  }
}

function seekFromWaveform(e: MouseEvent) {
  const target = e.currentTarget as HTMLElement
  const rect = target.getBoundingClientRect()
  const ratio = rect.width > 0 ? (e.clientX - rect.left) / rect.width : 0
  onSeekPercent(ratio * 100)
}

async function saveSyncEditor() {
  if (!selectedTrackId.value || isSavingSync.value) return
  const marked = markedSyncEntries.value
  if (marked.length === 0) {
    notify('Marca al menos una línea antes de guardar', 'info')
    return
  }
  isSavingSync.value = true
  try {
    const ordered = [...marked].sort((a, b) => a.line.t - b.line.t)
    const content = ordered.map(e => `[${formatClock(e.line.t)}]${e.line.text}`).join('\n')
    const result = await lyricsApi.saveLyrics({
      trackId: selectedTrackId.value,
      format: 'lrc',
      content,
      syncLevel: currentLyrics.value?.sync_level === 'word' ? 'word' : 'line',
      source: 'manual',
      language: currentLyrics.value?.language ?? undefined,
    })
    currentLyrics.value = result
    editableLyrics.value = result.content
    showSyncEditor.value = false
    isEditing.value = false
    await Promise.all([loadTracks(), loadLyricsStats()])
    notify(`Sync guardado: ${ordered.length} líneas marcadas`, 'success')
  } catch (err) {
    console.error('Failed to save sync:', err)
    notify(err instanceof Error ? err.message : String(err), 'error')
  } finally {
    isSavingSync.value = false
  }
}

// ==============================================
// SELECTION + HELPERS
// ==============================================

function selectTrack(track: LibraryTrack) {
  selectedTrackId.value = track.id
  isEditing.value = false
  loadTrackLyrics(track.id)
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

function mapLyricsType(type: string | null): 'synced' | 'unsynced' | 'none' {
  if (type === 'synced' || type === 'timed' || type === 'lrc' || type === 'ttml') return 'synced'
  if (type === 'plain') return 'unsynced'
  return 'none'
}

function formatDuration(ms: number | null): string {
  if (!ms) return '0:00'
  return formatDurationSec(ms / 1000)
}

// Watch for track selection to load lyrics
watch(selectedTrackId, (newId) => {
  if (newId) {
    loadTrackLyrics(newId)
  }
})

// Initialize
onMounted(async () => {
  window.addEventListener('keydown', onGlobalKeydown)
  await Promise.all([loadTracks(), loadLyricsStats(), loadProviderSettings()])
})

onUnmounted(() => {
  window.removeEventListener('keydown', onGlobalKeydown)
  if (actionTimer) clearTimeout(actionTimer)
})
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

/* Line clamp utility */
.line-clamp-2 {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.line-clamp-3 {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
