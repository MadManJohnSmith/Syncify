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
          </select>
          
          <!-- Quality Analyzer Button -->
          <button @click="showQualityReport = true" class="p-2 bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 rounded-lg transition-colors" title="Lyrics Quality Report">
            <span class="material-symbols-outlined text-[20px]">analytics</span>
          </button>
        </div>
        
        <!-- Enhanced Batch Toolbar -->
        <Transition name="slide-down">
          <div v-if="selectedTracks.length > 0" class="batch-toolbar mt-3 p-3 bg-primary/10 rounded-lg">
            <div class="flex items-center justify-between">
              <span class="text-sm text-primary font-semibold">{{ selectedTracks.length }} track{{ selectedTracks.length > 1 ? 's' : '' }} selected</span>
              <div class="flex items-center gap-2">
                <!-- Fetch Dropdown -->
                <div class="relative">
                  <button @click="batchFetchSelectedLyrics" class="flex items-center gap-1.5 px-3 py-1.5 bg-blue-500 text-white hover:bg-blue-600 rounded-lg text-xs font-medium transition-colors">
                    <span class="material-symbols-outlined text-[14px]">download</span>
                    Fetch Lyrics
                  </button>
                  <Transition name="fade">
                    <div v-if="showFetchDropdown" class="absolute top-full left-0 mt-1 w-40 bg-white dark:bg-surface-dark rounded-lg shadow-xl border border-gray-200 dark:border-border-dark overflow-hidden z-20">
                      <button class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Prefer synced</button>
                      <button class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Unsynced OK</button>
                      <button class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Synced only</button>
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
                  <Transition name="fade">
                    <div v-if="showExportDropdown" class="absolute top-full left-0 mt-1 w-32 bg-white dark:bg-surface-dark rounded-lg shadow-xl border border-gray-200 dark:border-border-dark overflow-hidden z-20">
                      <button class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">LRC Format</button>
                      <button class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">TTML Format</button>
                      <button class="w-full px-3 py-2 text-left text-xs text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight">Plain TXT</button>
                    </div>
                  </Transition>
                </div>
                
                <button class="flex items-center gap-1.5 px-3 py-1.5 bg-purple-500/20 text-purple-400 hover:bg-purple-500/30 rounded-lg text-xs font-medium transition-colors">
                  <span class="material-symbols-outlined text-[14px]">upgrade</span>
                  Upgrade
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
      
      <!-- Track List -->
      <div class="flex-1 overflow-y-auto custom-scrollbar">
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
            <div class="w-full h-full bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-600 dark:to-gray-700 flex items-center justify-center">
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
              <div class="w-full h-full bg-gradient-to-br from-gray-300 to-gray-400 dark:from-gray-600 dark:to-gray-700 flex items-center justify-center">
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
          <div class="px-6 py-3 bg-gray-50 dark:bg-surface-highlight/30 flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span v-if="currentTrack?.lyricsStatus !== 'none'" :class="[
                'px-2 py-1 text-xs font-medium rounded',
                currentTrack?.lyricsStatus === 'synced' ? 'bg-blue-500/10 text-blue-500' : 'bg-gray-500/10 text-gray-500'
              ]">
                {{ currentTrack?.lyricsStatus === 'synced' ? 'Synced · ' + currentTrack?.syncLevel : 'Unsynced' }}
              </span>
              <span v-if="currentTrack?.source" class="px-2 py-1 bg-purple-500/10 text-purple-500 text-xs font-medium rounded">
                {{ currentTrack?.source }}
              </span>
              <span v-if="currentTrack?.language" class="px-2 py-1 bg-green-500/10 text-green-500 text-xs font-medium rounded">
                {{ currentTrack?.language }}
              </span>
            </div>
            <div class="flex items-center gap-2">
              <button @click="isEditing = !isEditing" :class="[
                'p-2 rounded-lg transition-colors',
                isEditing ? 'bg-primary text-white' : 'hover:bg-gray-200 dark:hover:bg-surface-highlight text-gray-600 dark:text-gray-400'
              ]" title="Edit">
                <span class="material-symbols-outlined text-[18px]">edit</span>
              </button>
              <button @click="showFetchDialog = true" class="p-2 hover:bg-gray-200 dark:hover:bg-surface-highlight rounded-lg transition-colors text-gray-600 dark:text-gray-400" title="Fetch">
                <span class="material-symbols-outlined text-[18px]">download</span>
              </button>
              <button class="p-2 hover:bg-gray-200 dark:hover:bg-surface-highlight rounded-lg transition-colors text-gray-600 dark:text-gray-400" title="Export">
                <span class="material-symbols-outlined text-[18px]">save</span>
              </button>
              <button @click="deleteTrackLyrics" class="p-2 hover:bg-error/10 rounded-lg transition-colors text-error" title="Delete">
                <span class="material-symbols-outlined text-[18px]">delete</span>
              </button>
            </div>
          </div>
        </div>
        
        <!-- Editor Mode -->
        <div v-if="isEditing" class="lyrics-editor flex-1 flex flex-col overflow-hidden">
          <!-- Editor Toolbar -->
          <div class="px-6 py-3 border-b border-gray-200 dark:border-border-dark flex items-center gap-3">
            <button class="flex items-center gap-1.5 px-3 py-1.5 bg-purple-500/10 text-purple-500 hover:bg-purple-500/20 rounded-lg text-sm font-medium transition-colors">
              <span class="material-symbols-outlined text-[16px]">timer</span>
              Sync Timestamps
            </button>
            <select class="px-3 py-1.5 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
              <option>LRC Format</option>
              <option>TTML Format</option>
              <option>Plain Text</option>
            </select>
            <button class="flex items-center gap-1.5 px-3 py-1.5 bg-green-500/10 text-green-500 hover:bg-green-500/20 rounded-lg text-sm font-medium transition-colors">
              <span class="material-symbols-outlined text-[16px]">check_circle</span>
              Validate
            </button>
            <div class="flex-1"></div>
            <button @click="isEditing = false" class="px-4 py-1.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
              Save Changes
            </button>
          </div>
          
          <!-- Editor Textarea -->
          <div class="flex-1 p-6 overflow-hidden">
            <textarea 
              v-model="editableLyrics"
              class="w-full h-full p-4 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-xl text-sm font-mono text-gray-900 dark:text-white resize-none focus:outline-none focus:ring-2 focus:ring-primary/50"
              placeholder="Enter lyrics here..."
            ></textarea>
          </div>
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
            <button @click="showFetchDialog = true" class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors flex items-center gap-2">
              <span class="material-symbols-outlined text-[18px]">search</span>
              Search for Lyrics
            </button>
          </div>
          
          <!-- Synced Lyrics Display -->
          <div v-else-if="currentTrack?.lyricsStatus === 'synced'" class="flex-1 overflow-y-auto custom-scrollbar p-6">
            <div class="space-y-4 max-w-2xl mx-auto">
              <div 
                v-for="(line, index) in syncedLyrics" 
                :key="index"
                :class="[
                  'synced-line flex items-start gap-4 py-2 px-4 rounded-lg transition-colors cursor-pointer',
                  activeLine === index ? 'bg-primary/10' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
                ]"
                @click="seekToLine(index)"
              >
                <span class="timestamp text-xs font-mono text-text-secondary w-16 shrink-0 pt-1">{{ line.time }}</span>
                <p :class="[
                  'text-lg transition-colors',
                  activeLine === index ? 'text-primary font-medium' : 'text-gray-700 dark:text-gray-300'
                ]">{{ line.text }}</p>
              </div>
            </div>
          </div>
          
          <!-- Unsynced Lyrics Display -->
          <div v-else class="flex-1 overflow-y-auto custom-scrollbar p-6">
            <div class="max-w-2xl mx-auto prose dark:prose-invert">
              <p v-for="(paragraph, index) in unsyncedLyrics" :key="index" class="text-lg text-gray-700 dark:text-gray-300 mb-6 leading-relaxed">
                {{ paragraph }}
              </p>
            </div>
          </div>
          
          <!-- Playback Controls (for synced lyrics) -->
          <div v-if="currentTrack?.lyricsStatus === 'synced'" class="shrink-0 px-6 py-4 border-t border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30">
            <div class="flex items-center gap-4">
              <button @click="isPlaying = !isPlaying" class="p-2 bg-primary hover:bg-primary-hover text-white rounded-full transition-colors">
                <span class="material-symbols-outlined text-[20px]">{{ isPlaying ? 'pause' : 'play_arrow' }}</span>
              </button>
              <div class="flex-1">
                <div class="relative h-1 bg-gray-300 dark:bg-gray-600 rounded-full">
                  <div class="absolute inset-y-0 left-0 bg-primary rounded-full" :style="{ width: playbackProgress + '%' }"></div>
                  <input type="range" v-model="playbackProgress" min="0" max="100" class="absolute inset-0 w-full opacity-0 cursor-pointer">
                </div>
              </div>
              <span class="text-sm font-mono text-text-secondary w-24 text-right">{{ currentTime }} / {{ totalTime }}</span>
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
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[80vh] overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Search for Lyrics</h3>
              <button @click="showFetchDialog = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6">
              <!-- Search Info -->
              <div class="mb-4 p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg flex items-center gap-3">
                <span class="material-symbols-outlined text-primary">search</span>
                <span class="text-sm text-gray-700 dark:text-gray-300">
                  Searching for: <span class="font-medium text-gray-900 dark:text-white">{{ currentTrack?.title }}</span> by <span class="font-medium text-gray-900 dark:text-white">{{ currentTrack?.artist }}</span>
                </span>
              </div>
              
              <!-- Results -->
              <div class="space-y-3">
                <div v-for="result in lyricsResults" :key="result.id" class="lyrics-result p-4 border border-gray-200 dark:border-border-dark rounded-xl hover:border-primary/50 transition-colors">
                  <div class="flex items-start justify-between mb-3">
                    <div class="flex items-center gap-2">
                      <span class="text-sm font-medium text-gray-900 dark:text-white">{{ result.source }}</span>
                      <span :class="[
                        'px-2 py-0.5 text-[10px] font-medium rounded',
                        result.type === 'synced' ? 'bg-blue-500/10 text-blue-500' : 'bg-gray-500/10 text-gray-500'
                      ]">
                        {{ result.type === 'synced' ? 'Synced (' + result.syncLevel + ')' : 'Unsynced' }}
                      </span>
                      <span class="px-2 py-0.5 bg-green-500/10 text-green-500 text-[10px] font-medium rounded">
                        {{ result.language }}
                      </span>
                    </div>
                    <span :class="[
                      'px-2 py-0.5 text-[10px] font-medium rounded',
                      result.confidence === 'high' ? 'bg-success/10 text-success' : 'bg-amber-500/10 text-amber-500'
                    ]">
                      {{ result.confidence === 'high' ? 'High match' : 'Possible match' }}
                    </span>
                  </div>
                  <p class="text-sm text-text-secondary mb-3 line-clamp-3">{{ result.preview }}</p>
                  <button class="w-full py-2 bg-primary/10 hover:bg-primary/20 text-primary rounded-lg text-sm font-medium transition-colors">
                    Use This
                  </button>
                </div>
              </div>
              
              <!-- Manual Search -->
              <div class="mt-4 pt-4 border-t border-gray-200 dark:border-border-dark">
                <button class="w-full py-3 border border-gray-200 dark:border-border-dark hover:border-primary hover:text-primary rounded-lg text-sm font-medium text-gray-600 dark:text-gray-400 transition-colors flex items-center justify-center gap-2">
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
                  <div class="h-full bg-primary rounded-full transition-all" :style="{ width: (batchProgress.current / batchProgress.total * 100) + '%' }"></div>
                </div>
              </div>
              
              <!-- Current Track -->
              <p class="text-sm text-text-secondary mb-4">
                Fetching: <span class="text-gray-900 dark:text-white font-medium">{{ batchProgress.currentTrack }}</span>
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
              </div>
              
              <!-- Cancel Button -->
              <button @click="showBatchProgress = false" class="w-full py-2.5 border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 transition-colors">
                Cancel
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
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Lyrics Quality Report</h3>
              <button @click="showQualityReport = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 overflow-y-auto custom-scrollbar max-h-[calc(85vh-140px)]">
              <!-- Stats Overview -->
              <div class="grid grid-cols-3 gap-4 mb-6">
                <div class="p-4 bg-blue-500/10 rounded-xl text-center">
                  <p class="text-2xl font-bold text-blue-500">234</p>
                  <p class="text-xs text-text-secondary mt-1">Synced (45%)</p>
                </div>
                <div class="p-4 bg-gray-500/10 rounded-xl text-center">
                  <p class="text-2xl font-bold text-gray-500">123</p>
                  <p class="text-xs text-text-secondary mt-1">Unsynced (24%)</p>
                </div>
                <div class="p-4 bg-error/10 rounded-xl text-center">
                  <p class="text-2xl font-bold text-error">163</p>
                  <p class="text-xs text-text-secondary mt-1">No Lyrics (31%)</p>
                </div>
              </div>
              
              <!-- Sync Level Breakdown -->
              <div class="mb-6">
                <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Sync Level Breakdown</h5>
                <div class="space-y-2">
                  <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Syllable-level</span>
                    <span class="px-2 py-0.5 bg-purple-500/10 text-purple-500 text-xs font-medium rounded">45 tracks</span>
                  </div>
                  <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Word-level</span>
                    <span class="px-2 py-0.5 bg-blue-500/10 text-blue-500 text-xs font-medium rounded">89 tracks</span>
                  </div>
                  <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Line-level</span>
                    <span class="px-2 py-0.5 bg-green-500/10 text-green-500 text-xs font-medium rounded">100 tracks</span>
                  </div>
                </div>
              </div>
              
              <!-- Issues Found -->
              <div class="mb-6">
                <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Issues Found</h5>
                <div class="space-y-2">
                  <button class="w-full flex items-center justify-between p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg hover:bg-amber-500/10 transition-colors">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Invalid timestamps</span>
                    <span class="px-2 py-0.5 bg-amber-500/10 text-amber-500 text-xs font-medium rounded">5 tracks</span>
                  </button>
                  <button class="w-full flex items-center justify-between p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg hover:bg-amber-500/10 transition-colors">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Missing timestamps</span>
                    <span class="px-2 py-0.5 bg-amber-500/10 text-amber-500 text-xs font-medium rounded">12 tracks</span>
                  </button>
                  <button class="w-full flex items-center justify-between p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg hover:bg-amber-500/10 transition-colors">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Language mismatch</span>
                    <span class="px-2 py-0.5 bg-amber-500/10 text-amber-500 text-xs font-medium rounded">3 tracks</span>
                  </button>
                </div>
              </div>
              
              <!-- Recommendations -->
              <div>
                <h5 class="font-semibold text-gray-900 dark:text-white mb-3">Recommendations</h5>
                <div class="space-y-3">
                  <div class="flex items-center gap-3 p-3 bg-blue-500/5 border border-blue-500/20 rounded-lg">
                    <span class="material-symbols-outlined text-blue-500">download</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Fetch lyrics for 163 tracks without lyrics</span>
                    <button class="px-3 py-1.5 bg-blue-500 text-white rounded-lg text-xs font-medium hover:bg-blue-600 transition-colors">Run</button>
                  </div>
                  <div class="flex items-center gap-3 p-3 bg-purple-500/5 border border-purple-500/20 rounded-lg">
                    <span class="material-symbols-outlined text-purple-500">upgrade</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Upgrade 123 tracks to synced lyrics</span>
                    <button class="px-3 py-1.5 bg-purple-500 text-white rounded-lg text-xs font-medium hover:bg-purple-600 transition-colors">Run</button>
                  </div>
                  <div class="flex items-center gap-3 p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg">
                    <span class="material-symbols-outlined text-amber-500">build</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300 flex-1">Fix timestamp issues for 5 tracks</span>
                    <button class="px-3 py-1.5 bg-amber-500 text-white rounded-lg text-xs font-medium hover:bg-amber-600 transition-colors">Run</button>
                  </div>
                </div>
              </div>
            </div>
            
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end">
              <button class="px-6 py-2.5 bg-purple-500 hover:bg-purple-600 text-white rounded-lg font-medium transition-colors flex items-center gap-2">
                <span class="material-symbols-outlined text-[18px]">auto_fix_high</span>
                Auto-Fix All
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
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg max-h-[85vh] overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Lyrics Sources</h3>
              <button @click="showProviderSettings = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 overflow-y-auto custom-scrollbar max-h-[calc(85vh-180px)]">
              <!-- Provider List -->
              <div class="mb-6">
                <h5 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Priority Order (drag to reorder)</h5>
                <div class="provider-list space-y-2">
                  <div v-for="provider in providers" :key="provider.id" class="flex items-center gap-3 p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg cursor-move">
                    <span class="material-symbols-outlined text-gray-400 text-[18px]">drag_indicator</span>
                    <div class="flex-1">
                      <p class="text-sm font-medium text-gray-900 dark:text-white">{{ provider.name }}</p>
                      <p class="text-xs text-text-secondary">{{ provider.format }}</p>
                    </div>
                    <label class="relative inline-flex items-center cursor-pointer">
                      <input type="checkbox" :checked="provider.enabled" class="sr-only peer">
                      <div class="w-9 h-5 bg-gray-300 dark:bg-gray-600 rounded-full peer peer-checked:bg-primary transition-colors"></div>
                      <div class="absolute left-0.5 top-0.5 w-4 h-4 bg-white rounded-full shadow peer-checked:translate-x-4 transition-transform"></div>
                    </label>
                  </div>
                </div>
              </div>
              
              <!-- Provider Settings -->
              <div class="mb-6">
                <h5 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Preferences</h5>
                <div class="space-y-4">
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">Language Preference</label>
                    <select class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
                      <option>English</option>
                      <option>Original</option>
                      <option>All Languages</option>
                    </select>
                  </div>
                  <div>
                    <label class="block text-xs text-text-secondary mb-1">Minimum Sync Level</label>
                    <select class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
                      <option>Any</option>
                      <option>Line-level</option>
                      <option>Word-level</option>
                      <option>Syllable-level</option>
                    </select>
                  </div>
                </div>
              </div>
              
              <!-- Global Settings -->
              <div class="mb-6">
                <h5 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Global Settings</h5>
                <div class="space-y-3">
                  <label class="flex items-center gap-3 cursor-pointer">
                    <input type="checkbox" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Prefer synced over unsynced</span>
                  </label>
                  <label class="flex items-center gap-3 cursor-pointer">
                    <input type="checkbox" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    <span class="text-sm text-gray-700 dark:text-gray-300">Skip tracks with only unsynced lyrics</span>
                  </label>
                </div>
              </div>
              
              <!-- Fallback -->
              <div>
                <label class="block text-xs text-text-secondary mb-1">Fallback Behavior</label>
                <select class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight text-gray-900 dark:text-white text-sm rounded-lg border-0 focus:outline-none focus:ring-2 focus:ring-primary/50">
                  <option>Use unsynced</option>
                  <option>Skip track</option>
                  <option>Prompt me</option>
                </select>
              </div>
            </div>
            
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3">
              <button @click="showProviderSettings = false" class="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg font-medium transition-colors">
                Cancel
              </button>
              <button class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium transition-colors">
                Save Settings
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Advanced Sync Editor Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showSyncEditor" class="sync-editor fixed inset-0 bg-black flex flex-col z-50">
          <!-- Header -->
          <div class="px-6 py-4 border-b border-gray-700 flex items-center justify-between bg-gray-900">
            <div class="flex items-center gap-4">
              <h3 class="text-lg font-semibold text-white">Advanced Sync Editor</h3>
              <span class="text-sm text-gray-400">{{ currentTrack?.title }} - {{ currentTrack?.artist }}</span>
            </div>
            <div class="flex items-center gap-2">
              <button class="px-4 py-2 text-gray-300 hover:bg-gray-800 rounded-lg font-medium transition-colors">
                Cancel
              </button>
              <button @click="showSyncEditor = false" class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium transition-colors">
                Save & Close
              </button>
            </div>
          </div>
          
          <!-- Waveform Display -->
          <div class="waveform-display h-32 bg-gray-900 border-b border-gray-700 relative">
            <!-- Placeholder waveform -->
            <div class="absolute inset-0 flex items-center justify-center">
              <div class="flex items-end gap-0.5 h-20">
                <div v-for="i in 80" :key="i" class="w-1 bg-primary/40 rounded-t" :style="{ height: Math.random() * 100 + '%' }"></div>
              </div>
            </div>
            <!-- Timestamp markers -->
            <div class="absolute bottom-0 left-0 right-0 h-6 flex items-center px-4">
              <div v-for="i in 10" :key="i" class="flex-1 text-center">
                <span class="text-[10px] text-gray-500">{{ i - 1 }}:00</span>
              </div>
            </div>
            <!-- Playhead -->
            <div class="absolute top-0 bottom-6 w-0.5 bg-primary" :style="{ left: playbackProgress + '%' }"></div>
          </div>
          
          <!-- Main Content -->
          <div class="flex-1 flex overflow-hidden">
            <!-- Timestamp Editor -->
            <div class="timestamp-editor flex-1 overflow-y-auto custom-scrollbar bg-gray-900 p-6">
              <div class="space-y-2 max-w-2xl mx-auto">
                <div v-for="(line, index) in syncedLyrics" :key="index" class="flex items-center gap-3 p-3 rounded-lg hover:bg-gray-800 transition-colors group">
                  <input 
                    type="text" 
                    :value="line.time" 
                    class="w-24 px-2 py-1 bg-gray-800 border border-gray-700 rounded text-sm font-mono text-primary text-center focus:outline-none focus:ring-2 focus:ring-primary/50"
                  >
                  <input 
                    type="text" 
                    :value="line.text" 
                    class="flex-1 px-3 py-1 bg-gray-800 border border-gray-700 rounded text-sm text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                  >
                  <button class="p-1.5 text-gray-500 hover:text-primary hover:bg-gray-800 rounded transition-colors opacity-0 group-hover:opacity-100">
                    <span class="material-symbols-outlined text-[18px]">play_arrow</span>
                  </button>
                  <button class="p-1.5 text-gray-500 hover:text-error hover:bg-gray-800 rounded transition-colors opacity-0 group-hover:opacity-100">
                    <span class="material-symbols-outlined text-[18px]">delete</span>
                  </button>
                </div>
                <button class="w-full py-2 border border-dashed border-gray-700 rounded-lg text-gray-500 hover:text-primary hover:border-primary transition-colors flex items-center justify-center gap-2">
                  <span class="material-symbols-outlined text-[18px]">add</span>
                  Add Line
                </button>
              </div>
            </div>
            
            <!-- Tools Sidebar -->
            <div class="w-64 border-l border-gray-700 bg-gray-900 p-4">
              <h5 class="text-sm font-medium text-gray-400 mb-4">Tools</h5>
              <div class="space-y-2">
                <button class="w-full flex items-center gap-2 px-3 py-2 bg-purple-500/10 text-purple-400 hover:bg-purple-500/20 rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[18px]">auto_awesome</span>
                  Auto-detect beats
                </button>
                <button class="w-full flex items-center gap-2 px-3 py-2 bg-blue-500/10 text-blue-400 hover:bg-blue-500/20 rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[18px]">schedule</span>
                  Shift all timestamps
                </button>
                <button class="w-full flex items-center gap-2 px-3 py-2 bg-green-500/10 text-green-400 hover:bg-green-500/20 rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[18px]">grid_on</span>
                  Snap to grid
                </button>
                <button class="w-full flex items-center gap-2 px-3 py-2 bg-amber-500/10 text-amber-400 hover:bg-amber-500/20 rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[18px]">verified</span>
                  Validate timing
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
                  <span>Remove last</span>
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
            </div>
          </div>
          
          <!-- Playback Controls -->
          <div class="px-6 py-4 border-t border-gray-700 bg-gray-900 flex items-center gap-4">
            <button @click="isPlaying = !isPlaying" class="p-3 bg-primary hover:bg-primary-hover text-white rounded-full transition-colors">
              <span class="material-symbols-outlined text-[24px]">{{ isPlaying ? 'pause' : 'play_arrow' }}</span>
            </button>
            <div class="flex-1 relative">
              <div class="h-1 bg-gray-700 rounded-full">
                <div class="h-full bg-primary rounded-full" :style="{ width: playbackProgress + '%' }"></div>
              </div>
            </div>
            <span class="text-sm font-mono text-gray-400 w-24 text-right">{{ currentTime }} / {{ totalTime }}</span>
            <div class="flex items-center gap-2">
              <button :class="['px-2 py-1 rounded text-xs font-medium transition-colors', playbackSpeed === 0.5 ? 'bg-primary text-white' : 'text-gray-400 hover:text-white']" @click="playbackSpeed = 0.5">0.5x</button>
              <button :class="['px-2 py-1 rounded text-xs font-medium transition-colors', playbackSpeed === 1 ? 'bg-primary text-white' : 'text-gray-400 hover:text-white']" @click="playbackSpeed = 1">1x</button>
              <button :class="['px-2 py-1 rounded text-xs font-medium transition-colors', playbackSpeed === 1.5 ? 'bg-primary text-white' : 'text-gray-400 hover:text-white']" @click="playbackSpeed = 1.5">1.5x</button>
            </div>
            <button class="p-2 text-gray-400 hover:text-primary rounded-lg transition-colors" title="Loop section">
              <span class="material-symbols-outlined text-[20px]">repeat</span>
            </button>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Lyrics History Panel -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showLyricsHistory" class="lyrics-history fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showLyricsHistory = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md max-h-[70vh] overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Lyrics History</h3>
              <button @click="showLyricsHistory = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <div class="p-6 space-y-3 overflow-y-auto custom-scrollbar max-h-[calc(70vh-80px)]">
              <div v-for="version in lyricsHistory" :key="version.id" class="p-4 border border-gray-200 dark:border-border-dark rounded-xl hover:border-primary/50 transition-colors">
                <div class="flex items-start justify-between mb-2">
                  <div>
                    <span class="text-sm font-medium text-gray-900 dark:text-white">{{ version.source }}</span>
                    <span :class="[
                      'ml-2 px-2 py-0.5 text-[10px] font-medium rounded',
                      version.type === 'synced' ? 'bg-blue-500/10 text-blue-500' : 'bg-gray-500/10 text-gray-500'
                    ]">{{ version.type }}</span>
                  </div>
                  <span class="text-xs text-text-secondary">{{ version.date }}</span>
                </div>
                <p class="text-xs text-text-secondary mb-3">{{ version.preview }}</p>
                <button class="w-full py-2 bg-primary/10 hover:bg-primary/20 text-primary rounded-lg text-xs font-medium transition-colors">
                  Restore This Version
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { libraryApi } from '../api/library'
import { lyricsApi } from '../api/lyrics'
import type { LibraryTrack, Lyrics } from '../api/types'

// Search and Filter
const searchQuery = ref('')
const filterType = ref('all')

// Selection
const selectedTrackId = ref<number | null>(null)
const selectedTracks = ref<number[]>([])

// UI State
const isEditing = ref(false)
const isPlaying = ref(false)
const autoScroll = ref(true)
const showFetchDialog = ref(false)
const showBatchProgress = ref(false)
const playbackProgress = ref(0)
const activeLine = ref(0)
const isLoading = ref(false)
const isFetching = ref(false)

// Additional UI State for new features
const showQualityReport = ref(false)
const showProviderSettings = ref(false)
const showSyncEditor = ref(false)
const showLyricsHistory = ref(false)
const showFetchDropdown = ref(false)
const showExportDropdown = ref(false)
const playbackSpeed = ref(1)

// Placeholder text
const currentTime = ref('0:00')
const totalTime = ref('0:00')
const editableLyrics = ref('')

// Real data refs
const tracks = ref<LibraryTrack[]>([])
const currentLyrics = ref<Lyrics | null>(null)
const lyricsStats = ref({
  total_tracks: 0,
  with_lyrics: 0,
  synced_lyrics: 0,
  embedded_lyrics: 0
})

// Providers list
const providers = ref([
  { id: 1, name: 'LRCLIB', format: 'LRC (Line/Word-level)', enabled: true },
  { id: 2, name: 'NetEase', format: 'LRC (Word-level)', enabled: true },
  { id: 3, name: 'Musixmatch', format: 'LRC (Line-level)', enabled: true },
  { id: 4, name: 'Genius', format: 'Unsynced only', enabled: false },
])

// Lyrics history
const lyricsHistory = ref<{ id: number; source: string; type: string; date: string; preview: string }[]>([])

// Batch progress
const batchProgress = ref({
  current: 0,
  total: 0,
  currentTrack: '',
  success: 0,
  failed: 0,
})

// Filtered tracks
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
      result = result.filter(t => t.lyrics_type === 'synced' || t.lyrics_type === 'timed')
      break
    case 'unsynced':
      result = result.filter(t => t.lyrics_type === 'plain')
      break
    case 'none':
      result = result.filter(t => !t.lyrics_type || t.lyrics_type === 'none')
      break
    case 'downloaded':
      result = result.filter(t => t.download_status === 'downloaded')
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
    lyricsStatus: mapLyricsType(track.lyrics_type),
    syncLevel: currentLyrics.value?.sync_level ?? '',
    source: currentLyrics.value?.source ?? '',
    language: currentLyrics.value?.language ?? ''
  }
})

// Parsed synced lyrics lines
const syncedLyrics = computed(() => {
  if (!currentLyrics.value || currentLyrics.value.format !== 'lrc') return []
  return parseLrc(currentLyrics.value.content)
})

// Unsynced lyrics paragraphs
const unsyncedLyrics = computed(() => {
  if (!currentLyrics.value) return []
  if (currentLyrics.value.format === 'lrc') return []
  // Split plain lyrics by double newlines or single if no doubles
  const content = currentLyrics.value.content
  if (content.includes('\n\n')) {
    return content.split('\n\n').filter(p => p.trim())
  }
  return content.split('\n').filter(l => l.trim())
})

// Lyrics search results
const lyricsResults = ref<{
  id: number;
  source: string;
  type: string;
  syncLevel: string;
  language: string;
  confidence: string;
  preview: string;
}[]>([])

// Helper functions
function formatDuration(ms: number | null): string {
  if (!ms) return '0:00'
  const seconds = Math.floor(ms / 1000)
  const mins = Math.floor(seconds / 60)
  const secs = seconds % 60
  return `${mins}:${secs.toString().padStart(2, '0')}`
}

function mapLyricsType(type: string | null): 'synced' | 'unsynced' | 'none' {
  if (type === 'synced' || type === 'timed' || type === 'lrc' || type === 'ttml') return 'synced'
  if (type === 'plain') return 'unsynced'
  return 'none'
}

function parseLrc(lrc: string): { time: string; text: string }[] {
  const lines: { time: string; text: string }[] = []
  for (const line of lrc.split('\n')) {
    const match = line.match(/^\[(\d{2}:\d{2}\.\d{2})\](.*)$/)
    if (match) {
      lines.push({ time: `[${match[1]}]`, text: match[2].trim() })
    }
  }
  return lines
}

// Data loading
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

async function loadTrackLyrics(trackId: number) {
  try {
    currentLyrics.value = await lyricsApi.getLyrics(trackId)
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

async function fetchLyricsForTrack() {
  if (!selectedTrackId.value) return
  isFetching.value = true
  try {
    const result = await lyricsApi.fetchLyrics(selectedTrackId.value)
    if (result) {
      currentLyrics.value = result
      editableLyrics.value = result.content
      // Refresh track list to update lyrics_type
      await loadTracks()
      await loadLyricsStats()
    }
  } catch (err) {
    console.error('Failed to fetch lyrics:', err)
  } finally {
    isFetching.value = false
    showFetchDialog.value = false
  }
}

async function batchFetchSelectedLyrics() {
  if (selectedTracks.value.length === 0) return
  showBatchProgress.value = true
  batchProgress.value = {
    current: 0,
    total: selectedTracks.value.length,
    currentTrack: '',
    success: 0,
    failed: 0
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
      }
    })
    
    // Use the new progress-enabled API
    const result = await lyricsApi.batchFetchLyricsWithProgress(selectedTracks.value)
    batchProgress.value.success = result.fetched
    batchProgress.value.failed = result.failed
    // Refresh data
    await loadTracks()
    await loadLyricsStats()
  } catch (err) {
    console.error('Batch fetch failed:', err)
  } finally {
    // Clean up listener
    if (unlisten) unlisten()
    showBatchProgress.value = false
    clearSelection()
  }
}

async function deleteTrackLyrics() {
  if (!selectedTrackId.value) return
  try {
    await lyricsApi.deleteLyrics(selectedTrackId.value)
    currentLyrics.value = null
    editableLyrics.value = ''
    await loadTracks()
    await loadLyricsStats()
  } catch (err) {
    console.error('Failed to delete lyrics:', err)
  }
}

async function deleteSelectedLyrics() {
  if (selectedTracks.value.length === 0) return
  try {
    for (const trackId of selectedTracks.value) {
      await lyricsApi.deleteLyrics(trackId)
    }
    await loadTracks()
    await loadLyricsStats()
    clearSelection()
  } catch (err) {
    console.error('Failed to delete selected lyrics:', err)
  }
}

async function saveLyricsEdit() {
  if (!selectedTrackId.value || !editableLyrics.value) return
  try {
    const format = editableLyrics.value.includes('[') ? 'lrc' : 'plain'
    const result = await lyricsApi.saveLyrics({
      trackId: selectedTrackId.value,
      format: format as 'lrc' | 'plain',
      content: editableLyrics.value,
      source: 'manual'
    })
    currentLyrics.value = result
    isEditing.value = false
    await loadTracks()
  } catch (err) {
    console.error('Failed to save lyrics:', err)
  }
}

// Track selection
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

function seekToLine(index: number) {
  activeLine.value = index
}

// Watch for track selection to load lyrics
watch(selectedTrackId, (newId) => {
  if (newId) {
    loadTrackLyrics(newId)
  }
})

// Initialize
onMounted(() => {
  loadTracks()
  loadLyricsStats()
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
.line-clamp-3 {
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
