<template>
  <div class="downloads-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    
    <!-- Page Header with Status Cards -->
    <div class="px-8 pt-8 pb-4 flex items-center justify-between shrink-0">
      <div>
        <div class="flex items-center gap-3 mb-1">
          <h1 class="text-3xl font-bold tracking-tight text-gray-900 dark:text-white">Downloads</h1>
          <div class="flex items-center gap-2 px-3 py-1 rounded-full border text-xs font-bold"
               :class="isPaused ? 'bg-amber-500/10 border-amber-500/30 text-amber-600 dark:text-amber-400' : 'bg-emerald-500/10 border-emerald-500/30 text-emerald-600 dark:text-emerald-400'">
            <span class="w-2 h-2 rounded-full" :class="isPaused ? 'bg-amber-500' : 'bg-emerald-500 animate-pulse'"></span>
            <span>Queue Worker: {{ isPaused ? 'PAUSED' : 'ACTIVE' }}</span>
          </div>
        </div>
        <p class="text-text-secondary">Track progress and manage your queue.</p>
      </div>

      <!-- Status Cards -->
      <div class="flex gap-4">
        <!-- Active Card -->
        <div class="flex items-center gap-4 px-6 py-4 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm min-w-[140px]">
          <div class="h-12 w-12 rounded-full bg-primary/10 text-primary flex items-center justify-center">
            <span class="material-symbols-outlined text-[28px] animate-spin-slow">sync</span>
          </div>
          <div>
            <p class="text-xs font-semibold text-text-secondary uppercase tracking-wider">Active</p>
            <p class="text-2xl font-bold text-gray-900 dark:text-white">{{ queueStats?.downloading || 0 }}</p>
          </div>
        </div>

        <!-- Completed Card -->
        <div class="flex items-center gap-4 px-6 py-4 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm min-w-[140px]">
          <div class="h-12 w-12 rounded-full bg-success/10 text-success flex items-center justify-center">
            <span class="material-symbols-outlined text-[28px]">check_circle</span>
          </div>
          <div>
            <p class="text-xs font-semibold text-text-secondary uppercase tracking-wider">Completed</p>
            <p class="text-2xl font-bold text-gray-900 dark:text-white">{{ queueStats?.completed || 0 }}</p>
          </div>
        </div>

        <!-- Failed Card -->
        <div class="flex items-center gap-4 px-6 py-4 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm min-w-[140px]">
          <div class="h-12 w-12 rounded-full bg-error/10 text-error flex items-center justify-center">
            <span class="material-symbols-outlined text-[28px]">cancel</span>
          </div>
          <div>
            <p class="text-xs font-semibold text-text-secondary uppercase tracking-wider">Failed</p>
            <p class="text-2xl font-bold text-gray-900 dark:text-white">{{ queueStats?.failed || 0 }}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Direct Single-Track Pipeline Section (Corte 2) -->
    <div class="single-track-section mx-8 mb-6 p-6 rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm shrink-0">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-3">
          <div class="h-10 w-10 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
            <span class="material-symbols-outlined text-[24px]">music_note</span>
          </div>
          <div>
            <h2 class="text-base font-bold text-gray-900 dark:text-white">Direct Tidal Single-Track Pipeline</h2>
            <p class="text-xs text-text-secondary">Execute complete resolution, stream extraction, bit-perfect FLAC validation & METADATA_BLOCK_PICTURE tagging.</p>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-xs px-2.5 py-1 rounded-full font-semibold"
                :class="isPaused ? 'bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20' : 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20'">
            Worker: {{ isPaused ? 'Paused (Manual Mode)' : 'Running' }}
          </span>
        </div>
      </div>

      <!-- Controls row -->
      <div class="flex flex-wrap items-center gap-3 mb-2">
        <div class="flex-1 min-w-[280px] relative">
          <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 material-symbols-outlined text-[20px]">search</span>
          <input 
            v-model="singleTrackQuery"
            type="text"
            placeholder="Track Title - Artist or Tidal Track ID (e.g. David Bowie - Heroes or The Warning - Apologize)"
            :disabled="isSingleTrackDownloading"
            class="w-full pl-10 pr-4 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-xl text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary disabled:opacity-50"
            @keyup.enter="runSingleTrackDownload"
          />
        </div>

        <select 
          v-model="singleTrackQuality"
          :disabled="isSingleTrackDownloading"
          class="px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-xl text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary disabled:opacity-50"
        >
          <option value="HI_RES_LOSSLESS">HI_RES_LOSSLESS (24-bit / up to 192kHz)</option>
          <option value="LOSSLESS">LOSSLESS (16-bit / 44.1kHz FLAC)</option>
          <option value="HIGH">HIGH (320kbps AAC)</option>
        </select>

        <button 
          @click="runSingleTrackDownload"
          :disabled="isSingleTrackDownloading || !singleTrackQuery.trim()"
          class="flex items-center gap-2 px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-xl text-sm font-semibold transition-all shadow-sm disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <span class="material-symbols-outlined text-[20px]" :class="{ 'animate-spin': isSingleTrackDownloading }">
            {{ isSingleTrackDownloading ? 'progress_activity' : 'download' }}
          </span>
          {{ isSingleTrackDownloading ? 'Processing Pipeline...' : 'Download Single Track' }}
        </button>
      </div>

      <!-- Live Step Progression Feedback -->
      <div v-if="singleTrackProgress || isSingleTrackDownloading || singleTrackResult || singleTrackError" class="mt-4 p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark">
        <!-- Progress Steps Bar -->
        <div v-if="singleTrackProgress" class="mb-3">
          <div class="flex items-center justify-between text-xs mb-2">
            <div class="flex items-center gap-2">
              <span class="font-bold uppercase tracking-wider text-primary">Stage {{ singleTrackProgress.step_number }} of {{ singleTrackProgress.total_steps }}:</span>
              <span class="font-medium text-gray-900 dark:text-white">{{ singleTrackProgress.step }}</span>
            </div>
            <span class="text-text-secondary">{{ singleTrackProgress.message || '' }}</span>
          </div>
          <div class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div 
              class="h-full bg-gradient-to-r from-primary to-blue-500 rounded-full transition-all duration-300"
              :style="{ width: `${(singleTrackProgress.step_number / singleTrackProgress.total_steps) * 100}%` }"
            ></div>
          </div>
        </div>

        <!-- Success Result Details -->
        <div v-if="singleTrackResult" class="flex flex-col gap-2 pt-2 border-t border-gray-200 dark:border-border-dark/60 text-xs">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2 text-success font-semibold">
              <span class="material-symbols-outlined text-[18px]">check_circle</span>
              <span>Successfully downloaded & verified: {{ singleTrackResult.artist }} - {{ singleTrackResult.title }} ({{ singleTrackResult.album }})</span>
            </div>
            <span class="px-2 py-0.5 rounded bg-success/10 text-success font-mono font-bold uppercase">{{ singleTrackResult.codec }} {{ singleTrackResult.bit_depth }}bit/{{ (singleTrackResult.sample_rate / 1000).toFixed(1) }}kHz</span>
          </div>
          <div class="flex flex-wrap items-center gap-4 text-text-secondary">
            <span>Size: <strong class="text-gray-900 dark:text-white">{{ formatBytes(singleTrackResult.size_bytes) }}</strong></span>
            <span>Path: <strong class="text-gray-900 dark:text-white font-mono truncate max-w-[500px]" :title="singleTrackResult.final_path">{{ singleTrackResult.final_path }}</strong></span>
            <span>Validation: <strong class="text-success">{{ singleTrackResult.flac_validation }}</strong></span>
            <span>Tagging: <strong class="text-success">{{ singleTrackResult.tagging_result }}</strong></span>
          </div>
        </div>

        <!-- Error Feedback -->
        <div v-if="singleTrackError" class="flex items-start gap-2 pt-2 border-t border-red-200 dark:border-red-900/40 text-xs text-error">
          <span class="material-symbols-outlined text-[18px] shrink-0 mt-0.5">error</span>
          <div>
            <p class="font-semibold">Pipeline Error:</p>
            <p class="font-mono mt-0.5">{{ singleTrackError }}</p>
          </div>
        </div>
      </div>
    </div>


    <!-- Global Progress Section -->
    <div class="global-progress mx-8 mb-6 p-6 rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm shrink-0">
      <!-- Large Progress Bar -->
      <div class="relative h-3 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden mb-4">
        <div 
          class="absolute inset-0 bg-gradient-to-r from-primary to-blue-400 rounded-full progress-bar-animated transition-all duration-300"
          :style="{ width: overallProgress + '%' }"
        >
          <div class="absolute inset-0 bg-white/20 progress-shine"></div>
        </div>
        <span class="absolute right-2 top-1/2 -translate-y-1/2 text-[10px] font-bold text-white drop-shadow-sm">{{ Math.round(overallProgress) }}%</span>
      </div>
      
      <!-- Stats Row -->
      <div class="progress-stats flex items-center justify-between mb-4">
        <div class="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <span class="material-symbols-outlined text-[18px] text-primary">download</span>
          <span>Downloading <strong class="text-gray-900 dark:text-white">{{ activeDownloads.length }} of {{ (activeDownloads.length + queueItems.length) }}</strong> tracks</span>
        </div>
        <!-- Speed and ETA HIDDEN until backend support -->
        <!--
        <div class="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <span class="material-symbols-outlined text-[18px] text-success">speed</span>
          <span class="font-medium text-gray-900 dark:text-white">12.3 MB/s</span>
        </div>
        <div class="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <span class="material-symbols-outlined text-[18px] text-amber-500">schedule</span>
          <span>ETA: <strong class="text-gray-900 dark:text-white">8m 34s</strong></span>
        </div>
        -->
        <!-- Bandwidth Sparkline Graph HIDDEN -->
        <!--
        <div class="bandwidth-graph flex items-center gap-2">
          <span class="text-xs text-text-secondary">Speed (60s)</span>
          <svg class="w-[200px] h-[40px]" viewBox="0 0 200 40" preserveAspectRatio="none">
             ... content ...
          </svg>
        </div>
        -->
      </div>
      
      <!-- Action Buttons -->
      <div class="flex items-center justify-end gap-3">
        <button @click="togglePause" :disabled="isProcessing" class="flex items-center gap-2 px-4 py-2 bg-gray-100 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
          <span class="material-symbols-outlined text-[18px]">{{ isPaused ? 'play_arrow' : 'pause' }}</span>
          {{ isPaused ? 'Resume All' : 'Pause All' }}
        </button>
        <button @click="clearCompleted" :disabled="isProcessing" class="flex items-center gap-2 px-4 py-2 bg-gray-100 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
          <span class="material-symbols-outlined text-[18px]">delete_sweep</span>
          Clear Completed
        </button>
        <button @click="retryFailed" :disabled="isProcessing" class="flex items-center gap-2 px-4 py-2 bg-amber-500/10 border border-amber-500/30 hover:bg-amber-500/20 text-amber-600 dark:text-amber-400 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
          <span class="material-symbols-outlined text-[18px]">refresh</span>
          Retry All Failed
        </button>
        <button @click="clearFailed" :disabled="isProcessing" class="flex items-center gap-2 px-4 py-2 bg-red-500/10 border border-red-500/30 hover:bg-red-500/20 text-red-600 dark:text-red-400 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
          <span class="material-symbols-outlined text-[18px]">delete</span>
          Clear Failed
        </button>
      </div>
    </div>

    <!-- Downloads Toolbar -->
    <div class="downloads-toolbar mx-8 mb-4 flex items-center gap-4 shrink-0">
      <!-- View Filter Dropdown -->
      <div class="relative">
        <button 
          @click="showViewDropdown = !showViewDropdown"
          class="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
        >
          <span class="material-symbols-outlined text-[18px]">filter_list</span>
          <span>{{ viewFilterLabel }}</span>
          <span class="material-symbols-outlined text-[16px] text-gray-400">expand_more</span>
        </button>
        <div v-if="showViewDropdown" class="absolute top-full left-0 mt-1 w-44 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg shadow-xl z-20 py-1">
          <button 
            v-for="option in viewFilterOptions" 
            :key="option.value"
            @click="viewFilter = option.value as typeof viewFilter; showViewDropdown = false"
            :class="['w-full px-4 py-2.5 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-3', viewFilter === option.value ? 'text-primary font-medium' : 'text-gray-700 dark:text-gray-300']"
          >
            <span class="material-symbols-outlined text-[18px]">{{ option.icon }}</span>
            {{ option.label }}
            <span v-if="viewFilter === option.value" class="ml-auto material-symbols-outlined text-[16px]">check</span>
          </button>
        </div>
      </div>
      
      <!-- Search Input -->
      <div class="relative flex-1 max-w-md">
        <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 material-symbols-outlined text-[20px]">search</span>
        <input 
          v-model="searchQuery"
          type="text" 
          placeholder="Search downloads..." 
          class="w-full pl-10 pr-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all hover:border-gray-300 dark:hover:border-gray-600"
        >
      </div>
      
      <!-- Right Action Buttons -->
      <div class="flex items-center gap-3 ml-auto">
        <div class="relative">
          <button 
            @click="showFavoritesDownloadDropdown = !showFavoritesDownloadDropdown"
            :disabled="isProcessing"
            class="flex items-center gap-2 px-4 py-2.5 bg-primary/10 border border-primary/30 hover:bg-primary/20 text-primary rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
          >
            <span class="material-symbols-outlined text-[18px]">favorite</span>
            Download Favorites
            <span class="material-symbols-outlined text-[16px]">expand_more</span>
          </button>
          <div v-if="showFavoritesDownloadDropdown" class="absolute top-full right-0 mt-1 w-48 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg shadow-xl z-20 py-1">
            <button @click="triggerDownloadFavorites('all')" class="w-full px-4 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-2 text-gray-700 dark:text-gray-200">
              <span class="material-symbols-outlined text-[16px] text-primary">all_inclusive</span>
              All Services
            </button>
            <button @click="triggerDownloadFavorites('tidal')" class="w-full px-4 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-2 text-gray-700 dark:text-gray-200">
              <span class="w-2 h-2 rounded-full bg-[#00d4aa]"></span>
              Tidal Favorites
            </button>
            <button @click="triggerDownloadFavorites('qobuz')" class="w-full px-4 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-2 text-gray-700 dark:text-gray-200">
              <span class="w-2 h-2 rounded-full bg-[#1a8fe3]"></span>
              Qobuz Favorites
            </button>
            <button @click="triggerDownloadFavorites('spotify')" class="w-full px-4 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight flex items-center gap-2 text-gray-700 dark:text-gray-200">
              <span class="w-2 h-2 rounded-full bg-[#1ed760]"></span>
              Spotify Favorites
            </button>
          </div>
        </div>

        <button @click="clearCompleted" :disabled="isProcessing" class="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors disabled:opacity-50">
          <span class="material-symbols-outlined text-[18px]">delete_sweep</span>
          Clear Completed
        </button>
        <button @click="retryFailed" :disabled="isProcessing" class="flex items-center gap-2 px-4 py-2.5 bg-amber-500/10 border border-amber-500/30 hover:bg-amber-500/20 text-amber-600 dark:text-amber-400 rounded-lg text-sm font-medium transition-colors disabled:opacity-50">
          <span class="material-symbols-outlined text-[18px]">refresh</span>
          Retry All Failed
        </button>
        <button @click="showSettingsPanel = true" class="p-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-400 dark:text-gray-500 rounded-lg transition-colors" title="Download Settings">
          <span class="material-symbols-outlined text-[20px]">settings</span>
        </button>
      </div>
    </div>

    <!-- Scrollable Content Area -->
    <div class="flex-1 overflow-y-auto custom-scrollbar px-8 pb-8">

      <!-- Active Downloads Section -->
      <div class="active-downloads mb-8">
        <div class="flex items-center gap-3 mb-4">
          <h2 class="text-lg font-bold text-gray-900 dark:text-white">Active Downloads</h2>
          <span class="px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs font-bold">{{ activeDownloads.length }}</span>
        </div>
        
        <div class="flex flex-col gap-4">
          <!-- Empty state when no active downloads -->
          <div v-if="activeDownloads.length === 0" class="rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark p-8 text-center">
            <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-2">cloud_done</span>
            <p class="text-text-secondary">No active downloads</p>
          </div>
          
          <!-- Active Download Items -->
          <div 
            v-for="item in activeDownloads" 
            :key="item.id"
            class="download-item relative overflow-hidden rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark p-5 shadow-sm"
          >
            <div class="flex items-center gap-5">
              <!-- Album Art -->
              <div :class="['w-16 h-16 rounded-lg shrink-0 flex items-center justify-center text-white/30', item.artGradient]">
                <span class="material-symbols-outlined text-3xl">album</span>
              </div>
              
              <!-- Track Info -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-3 mb-1">
                  <h3 class="text-[15px] font-semibold text-gray-900 dark:text-white truncate">{{ item.title }}</h3>
                  <span :class="['px-2 py-0.5 rounded text-[10px] font-bold uppercase', item.serviceBadgeClass]">{{ item.service }}</span>
                  <span :class="['px-2 py-0.5 rounded text-[10px] font-bold', item.qualityBadgeClass]">{{ item.quality }}</span>
                </div>
                <p class="text-[13px] text-text-secondary truncate mb-3">{{ item.artist }} • {{ item.album }}</p>
                
                <!-- Individual Progress Bar -->
                <div class="item-progress">
                  <div class="flex items-center gap-3 mb-1.5">
                    <div class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                      <div class="h-full bg-primary rounded-full transition-all duration-300" :style="{ width: item.progress + '%' }"></div>
                    </div>
                    <span class="text-xs font-bold text-primary w-10 text-right">{{ item.progress }}%</span>
                  </div>
                  <div class="flex items-center justify-between text-[11px] text-text-secondary">
                    <span>Downloading...</span>
                    <span class="flex items-center gap-1 text-primary">
                      <span class="material-symbols-outlined text-[14px] animate-spin">sync</span>
                      In progress
                    </span>
                  </div>
                </div>
              </div>
              
              <!-- Action Buttons -->
              <div class="flex items-center gap-2 shrink-0">
                <button class="p-2.5 text-gray-400 hover:text-primary hover:bg-primary/10 rounded-lg transition-all" title="Pause">
                  <span class="material-symbols-outlined text-[22px]">pause</span>
                </button>
                <button @click="cancelItem(item.id)" class="p-2.5 text-gray-400 hover:text-error hover:bg-error/10 rounded-lg transition-all" title="Cancel">
                  <span class="material-symbols-outlined text-[22px]">close</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Up Next (Queue) Section -->
      <div class="queue-section">
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-3">
            <h2 class="text-lg font-bold text-gray-900 dark:text-white">Up Next</h2>
            <span class="px-2 py-0.5 rounded-full bg-gray-100 dark:bg-surface-highlight text-text-secondary text-xs font-bold">{{ queueItems.length }}</span>
          </div>
          <div class="flex items-center gap-4">
            <button @click="clearPendingQueue" :disabled="isProcessing" class="text-sm font-medium text-primary hover:text-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Clear All</button>
            <button @click="pauseAll" :disabled="isProcessing" class="text-sm font-medium text-text-secondary hover:text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Pause All</button>
          </div>

        </div>
        
        <div class="border border-gray-200 dark:border-border-dark rounded-xl bg-white dark:bg-surface-dark overflow-hidden">
          <!-- Queue Items -->
          <div 
            v-for="(item, index) in queueItems" 
            :key="item.id"
            draggable="true"
            @dragstart="onDragStart(index)"
            @dragover="onDragOver"
            @drop="onDrop(index)"
            class="queue-item flex items-center gap-4 p-4 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors group cursor-grab active:cursor-grabbing"
          >
            <!-- Drag Handle -->
            <span class="material-symbols-outlined text-[18px] text-gray-300 dark:text-gray-600 group-hover:text-gray-400 cursor-grab">drag_indicator</span>
            
            <!-- Queue Position -->
            <span class="text-sm text-gray-400 w-6 text-center font-medium">{{ index + 1 }}</span>
            
            <!-- Album Art (smaller) -->
            <div :class="['w-10 h-10 rounded-md shrink-0 flex items-center justify-center', item.artGradient]">
              <span class="material-symbols-outlined text-xl text-white/30">music_note</span>
            </div>
            
            <!-- Track Info -->
            <div class="flex-1 min-w-0">
              <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ item.title }}</p>
              <p class="text-xs text-text-secondary truncate">{{ item.artist }} • {{ item.album }}</p>
            </div>
            
            <!-- Service Badge (small) -->
            <span :class="['px-2 py-0.5 rounded text-[9px] font-bold uppercase', item.serviceBadgeClass]">{{ item.service }}</span>
            
            <!-- Status Badge -->
            <span class="px-2.5 py-1 rounded-full text-[10px] font-bold bg-gray-100 dark:bg-surface-highlight text-text-secondary uppercase tracking-wide">Queued</span>
            
            <!-- Remove Button (on hover) -->
            <button @click="removeQueueItem(item.id)" class="opacity-0 group-hover:opacity-100 p-2 text-gray-400 hover:text-error rounded-lg transition-all" title="Remove from queue">
              <span class="material-symbols-outlined text-[18px]">close</span>
            </button>

          </div>
        </div>
      </div>

      <!-- Completed Downloads Section (Collapsible) -->
      <div v-if="viewFilter === 'all' || viewFilter === 'completed'" class="completed-section mb-8">
        <div class="flex items-center justify-between mb-4">
          <button @click="showCompleted = !showCompleted" class="flex items-center gap-3 group">
            <span :class="['material-symbols-outlined text-[20px] text-gray-400 transition-transform', showCompleted ? 'rotate-0' : '-rotate-90']">expand_more</span>
            <h2 class="text-lg font-bold text-gray-900 dark:text-white group-hover:text-primary transition-colors">Completed</h2>
            <span class="px-2 py-0.5 rounded-full bg-success/10 text-success text-xs font-bold">{{ completedItems.length }}</span>
          </button>
          <button @click="clearCompleted" class="text-sm font-medium text-text-secondary hover:text-error transition-colors">Clear All</button>

        </div>
        
        <Transition name="collapse">
          <div v-if="showCompleted" class="border border-gray-200 dark:border-border-dark rounded-xl bg-white dark:bg-surface-dark overflow-hidden">
            <!-- Completed Items (compact rows) -->
            <div 
              v-for="item in completedItems" 
              :key="item.id"
              class="completed-item flex items-center gap-4 px-4 py-2.5 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors group"
            >
              <!-- Album Art (40x40) -->
              <div :class="['w-10 h-10 rounded-md shrink-0 flex items-center justify-center', item.artGradient]">
                <span class="material-symbols-outlined text-xl text-white/30">music_note</span>
              </div>
              
              <!-- Track Info -->
              <div class="flex-1 min-w-0">
                <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ item.title }}</p>
                <p class="text-xs text-text-secondary truncate">{{ item.artist }}</p>
              </div>
              
              <!-- Service + Quality Badges (small) -->
              <span :class="['px-1.5 py-0.5 rounded text-[9px] font-bold uppercase', item.serviceBadgeClass]">{{ item.service }}</span>
              <span :class="['px-1.5 py-0.5 rounded text-[9px] font-bold', item.qualityBadgeClass]">{{ item.quality }}</span>
              
              <!-- Completion Time -->
              <span class="text-xs text-text-secondary w-16 text-right">{{ item.completedAt }}</span>
              
              <!-- Success Icon -->
              <span class="material-symbols-outlined text-[20px] text-success">check_circle</span>
              
              <!-- Hover Actions -->
              <div class="opacity-0 group-hover:opacity-100 flex items-center gap-1 transition-opacity">
                <button 
                  @click="showInFolder(item.trackId)" 
                  :disabled="!item.trackId"
                  :class="['p-1.5 rounded transition-colors', item.trackId ? 'text-gray-400 hover:text-primary' : 'text-gray-600 opacity-50 cursor-not-allowed']" 
                  title="Show in Folder"
                >
                  <span class="material-symbols-outlined text-[18px]">folder_open</span>
                </button>
                <button @click="removeQueueItem(item.id)" class="p-1.5 text-gray-400 hover:text-error rounded transition-colors" title="Remove from List">
                  <span class="material-symbols-outlined text-[18px]">close</span>
                </button>
              </div>

            </div>
            
            <!-- Show More Button -->
            <button class="w-full py-3 text-sm font-medium text-primary hover:text-primary-hover hover:bg-primary/5 transition-colors border-t border-gray-100 dark:border-border-dark">
              Show More (1,234 more)
            </button>
          </div>
        </Transition>
      </div>

      <!-- Failed Downloads Section -->
      <div v-if="viewFilter === 'all' || viewFilter === 'failed'" class="failed-section">
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-3">
            <h2 class="text-lg font-bold text-gray-900 dark:text-white">Failed</h2>
            <span class="px-2 py-0.5 rounded-full bg-error/10 text-error text-xs font-bold">{{ failedItems.length }}</span>
          </div>
          <button class="flex items-center gap-2 px-3 py-1.5 bg-amber-500/10 border border-amber-500/30 hover:bg-amber-500/20 text-amber-600 dark:text-amber-400 rounded-lg text-sm font-medium transition-colors">
            <span class="material-symbols-outlined text-[16px]">refresh</span>
            Retry All
          </button>
        </div>
        
        <div class="flex flex-col gap-4">
          <!-- Failed Item Cards -->
          <div 
            v-for="item in failedItems" 
            :key="item.id"
            class="failed-item relative overflow-hidden rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm"
          >
            <!-- Red left border -->
            <div class="absolute left-0 top-0 bottom-0 w-1 bg-error"></div>
            
            <div class="flex items-start gap-5 p-5 pl-6">
              <!-- Album Art (60x60) -->
              <div :class="['w-16 h-16 rounded-lg shrink-0 flex items-center justify-center', item.artGradient]">
                <span class="material-symbols-outlined text-3xl text-white/30">album</span>
              </div>
              
              <!-- Track Info + Error -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-3 mb-1">
                  <h3 class="text-[15px] font-semibold text-gray-900 dark:text-white truncate">{{ item.title }}</h3>
                  <span :class="['px-2 py-0.5 rounded text-[10px] font-bold uppercase', item.serviceBadgeClass]">{{ item.service }}</span>
                </div>
                <p class="text-[13px] text-text-secondary truncate mb-2">{{ item.artist }} • {{ item.album }}</p>
                
                <!-- Error Message -->
                <div class="error-message flex items-center gap-2 text-error text-sm mb-2">
                  <span class="material-symbols-outlined text-[16px]">error</span>
                  <span class="font-medium">{{ item.errorMessage }}</span>
                </div>
                
                <!-- Expandable Error Details -->
                <button 
                  @click="item.showDetails = !item.showDetails"
                  class="flex items-center gap-1 text-xs text-text-secondary hover:text-gray-300 transition-colors"
                >
                  <span class="material-symbols-outlined text-[14px]">{{ item.showDetails ? 'expand_less' : 'expand_more' }}</span>
                  {{ item.showDetails ? 'Hide details' : 'Show details' }}
                </button>
                
                <Transition name="expand">
                  <div v-if="item.showDetails" class="mt-2 p-3 rounded-lg bg-gray-100 dark:bg-gray-800/50 border border-gray-200 dark:border-border-dark">
                    <p class="text-xs font-mono text-text-secondary break-all mb-1">{{ item.errorDetails }}</p>
                    <p class="text-xs text-text-secondary">Failed at {{ item.failedAt }}</p>
                  </div>
                </Transition>
              </div>
              
              <!-- Action Buttons -->
              <div class="flex flex-col gap-2 shrink-0">
                <button @click="retryItem(item.id)" class="flex items-center gap-2 px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                  <span class="material-symbols-outlined text-[16px]">refresh</span>
                  Retry
                </button>
                <div class="relative">
                  <button class="w-full flex items-center justify-center gap-2 px-4 py-2 bg-white dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
                    Try Different Source
                    <span class="material-symbols-outlined text-[14px]">expand_more</span>
                  </button>
                </div>
                <button @click="removeQueueItem(item.id)" class="p-2 text-gray-400 hover:text-error self-center rounded-lg transition-colors" title="Remove">
                  <span class="material-symbols-outlined text-[18px]">close</span>
                </button>

              </div>
            </div>
          </div>
        </div>
      </div>

    </div>

    <!-- Statistics Card (collapsible) -->
    <div v-if="showStats" class="stats-card mx-8 mb-4 shrink-0">
      <div class="flex items-center justify-between p-4 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm">
        <div class="flex items-center gap-8">
          <div class="text-center">
            <p class="text-2xl font-bold text-gray-900 dark:text-white">{{ queueStats?.total || 0 }}</p>
            <p class="text-xs text-text-secondary">Total Tracks</p>
          </div>
          <div class="w-px h-10 bg-gray-200 dark:bg-border-dark"></div>
          <div class="text-center">
            <p class="text-2xl font-bold text-gray-900 dark:text-white">—</p>
            <p class="text-xs text-text-secondary">Total Size</p>
          </div>
          <div class="w-px h-10 bg-gray-200 dark:bg-border-dark"></div>
          <div class="text-center">
            <p class="text-2xl font-bold text-gray-900 dark:text-white">—</p>
            <p class="text-xs text-text-secondary">Avg Speed</p>
          </div>
          <div class="w-px h-10 bg-gray-200 dark:bg-border-dark"></div>
          <div class="text-center">
            <p class="text-2xl font-bold text-success">{{ queueStats?.total ? ((queueStats.completed / queueStats.total) * 100).toFixed(1) : '100.0' }}%</p>
            <p class="text-xs text-text-secondary">Success Rate</p>
          </div>
        </div>

        <button @click="showStats = false" class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors">
          <span class="material-symbols-outlined text-[20px]">close</span>
        </button>
      </div>
    </div>

    <!-- Download Settings Panel (Teleport) -->
    <Teleport to="body">
    <Transition name="fade">
      <div v-if="showSettingsPanel" class="panel-overlay fixed inset-0 bg-black/50 z-50" @click="showSettingsPanel = false"></div>
    </Transition>
    <Transition name="slide-right">
      <div v-if="showSettingsPanel" class="download-settings-panel fixed top-0 right-0 h-full w-[420px] bg-white dark:bg-surface-dark border-l border-gray-200 dark:border-border-dark shadow-2xl z-50 flex flex-col">
        <!-- Panel Header -->
        <div class="flex items-center justify-between px-6 py-5 border-b border-gray-200 dark:border-border-dark shrink-0">
          <h2 class="text-xl font-bold text-gray-900 dark:text-white">Download Settings</h2>
          <button @click="showSettingsPanel = false" class="p-2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors">
            <span class="material-symbols-outlined">close</span>
          </button>
        </div>
        
        <!-- Panel Content (Scrollable) -->
        <div class="flex-1 overflow-y-auto custom-scrollbar p-6">
          <!-- Download Behavior Section -->
          <div class="panel-section mb-8">
            <h3 class="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-4">Download Behavior</h3>
            
            <div class="space-y-4">
              <!-- Concurrent Downloads -->
              <div class="flex items-center justify-between">
                <label class="text-sm text-gray-700 dark:text-gray-300">Concurrent downloads</label>
                <select v-model="settings.concurrentDownloads" class="px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary">
                  <option value="1">1</option>
                  <option value="2">2</option>
                  <option value="3">3</option>
                  <option value="4">4</option>
                  <option value="5">5</option>
                  <option value="0">Unlimited</option>
                </select>
              </div>
              
              <!-- Retry Failed -->
              <div class="flex items-center justify-between">
                <label class="text-sm text-gray-700 dark:text-gray-300">Retry failed downloads</label>
                <select v-model="settings.retryFailed" class="px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary">
                  <option value="never">Never</option>
                  <option value="once">Once</option>
                  <option value="3">Up to 3 times</option>
                  <option value="5">Up to 5 times</option>
                </select>
              </div>
              
              <!-- Auto-download favorites -->
              <div class="flex items-center justify-between">
                <label class="text-sm text-gray-700 dark:text-gray-300">Auto-download new favorites</label>
                <button 
                  @click="settings.autoDownloadFavorites = !settings.autoDownloadFavorites"
                  :class="['relative w-12 h-6 rounded-full transition-colors', settings.autoDownloadFavorites ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600']"
                >
                  <span :class="['absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform', settings.autoDownloadFavorites ? 'translate-x-6' : '']"></span>
                </button>
              </div>
            </div>
          </div>
          
          <!-- Network Section -->
          <div class="panel-section mb-8">
            <h3 class="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-4">Network</h3>
            
            <div class="space-y-4">
              <!-- Max Speed Slider -->
              <div>
                <div class="flex items-center justify-between mb-2">
                  <label class="text-sm text-gray-700 dark:text-gray-300">Max download speed</label>
                  <span class="text-sm font-medium text-primary">{{ settings.maxSpeed === 0 ? 'Unlimited' : settings.maxSpeed + ' MB/s' }}</span>
                </div>
                <input 
                  type="range" 
                  v-model="settings.maxSpeed" 
                  min="0" 
                  max="100" 
                  class="w-full h-2 bg-gray-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary"
                >
              </div>
              
              <!-- Pause on metered -->
              <div class="flex items-center justify-between">
                <label class="text-sm text-gray-700 dark:text-gray-300">Pause on metered connection</label>
                <button 
                  @click="settings.pauseOnMetered = !settings.pauseOnMetered"
                  :class="['relative w-12 h-6 rounded-full transition-colors', settings.pauseOnMetered ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600']"
                >
                  <span :class="['absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform', settings.pauseOnMetered ? 'translate-x-6' : '']"></span>
                </button>
              </div>
            </div>
          </div>
          
          <!-- Storage Section -->
          <div class="panel-section mb-8">
            <h3 class="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-4">Storage</h3>
            
            <div class="space-y-4">
              <!-- Download Location -->
              <div>
                <label class="text-sm text-gray-700 dark:text-gray-300 mb-2 block">Download location</label>
                <div class="flex items-center gap-2">
                  <div class="flex-1 px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-600 dark:text-gray-400 truncate">
                    {{ settings.downloadPath }}
                  </div>
                  <button class="px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
                    Change
                  </button>
                </div>
              </div>
              
              <!-- Organize by artist/album -->
              <div class="flex items-center justify-between">
                <label class="text-sm text-gray-700 dark:text-gray-300">Organize by artist/album</label>
                <button 
                  @click="settings.organizeByArtist = !settings.organizeByArtist"
                  :class="['relative w-12 h-6 rounded-full transition-colors', settings.organizeByArtist ? 'bg-primary' : 'bg-gray-300 dark:bg-gray-600']"
                >
                  <span :class="['absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform', settings.organizeByArtist ? 'translate-x-6' : '']"></span>
                </button>
              </div>
            </div>
          </div>
          
          <!-- Priority Section -->
          <div class="panel-section">
            <h3 class="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-4">Service Priority</h3>
            <p class="text-xs text-text-secondary mb-3">Drag to reorder download source priority</p>
            
            <div class="space-y-2">
              <div 
                v-for="(service, index) in uiSettings.servicePriority" 
                :key="service.id"
                class="reorderable-item flex items-center gap-3 px-4 py-3 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg cursor-grab hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
              >
                <span class="drag-handle material-symbols-outlined text-[18px] text-gray-400">drag_indicator</span>
                <span class="text-sm font-medium text-gray-700 dark:text-gray-200 flex-1">{{ service.name }}</span>
                <span class="text-xs text-text-secondary">{{ index + 1 }}</span>
              </div>
            </div>
          </div>
        </div>
        
        <!-- Panel Footer -->
        <div class="flex items-center justify-end gap-3 px-6 py-4 border-t border-gray-200 dark:border-border-dark shrink-0">
          <button @click="showSettingsPanel = false" :disabled="isProcessing" class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors disabled:opacity-50">
            Cancel
          </button>
          <button @click="saveSettings" :disabled="isProcessing" class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
            <span v-if="isProcessing">Saving...</span>
            <span v-else>Save Settings</span>
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useToast } from '@/composables/useToast'
import { confirm } from '@tauri-apps/plugin-dialog'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { queueApi } from '@/api/queue'
import { invokeCommand } from '@/api/tauri'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { settingsApi } from '@/api/settings'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import type { QueueItem, QueueStats, WorkerStatus, ProgressEvent, PipelineProgressEvent, TidalSingleTrackResponse } from '@/api/types'


// Event bus for real-time updates
const { on } = useEventBus()
const toast = useToast()
const router = useRouter()

// Single Track Direct Pipeline State
const singleTrackQuery = ref('David Bowie - Heroes')
const singleTrackQuality = ref('HI_RES_LOSSLESS')
const isSingleTrackDownloading = ref(false)
const singleTrackProgress = ref<PipelineProgressEvent | null>(null)
const singleTrackResult = ref<TidalSingleTrackResponse | null>(null)
const singleTrackError = ref<string | null>(null)

let unlistenPipelineProgress: UnlistenFn | null = null
import { downloadFavorites } from '@/api/library'

// Toolbar state
const viewFilter = ref<'all' | 'active' | 'queued' | 'completed' | 'failed'>('all')
const showViewDropdown = ref(false)
const showFavoritesDownloadDropdown = ref(false)
const searchQuery = ref('')
const loading = ref(true)
const isProcessing = ref(false)

async function triggerDownloadFavorites(service: string) {
  showFavoritesDownloadDropdown.value = false
  isProcessing.value = true
  try {
    const result = await downloadFavorites(service === 'all' ? undefined : service)
    toast.success(result.message)
    await fetchData()
  } catch (e: any) {
    toast.error(`Download favorites failed: ${e}`)
  } finally {
    isProcessing.value = false
  }
}
// isPaused is synced with workerStatus.value?.paused / is_paused
const isPaused = computed(() => workerStatus.value?.paused ?? workerStatus.value?.is_paused ?? false)


const viewFilterOptions = [
  { value: 'all', label: 'All Downloads', icon: 'list' },
  { value: 'active', label: 'Active', icon: 'downloading' },
  { value: 'queued', label: 'Queued', icon: 'schedule' },
  { value: 'completed', label: 'Completed', icon: 'check_circle' },
  { value: 'failed', label: 'Failed', icon: 'error' },
]

const viewFilterLabel = computed(() => {
  return viewFilterOptions.find(o => o.value === viewFilter.value)?.label || 'All Downloads'
})

// Section visibility
const showCompleted = ref(true)
const showStats = ref(true)

// Settings panel state
const showSettingsPanel = ref(false)

// Backend data
const queueStats = ref<QueueStats>({ total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 })
const workerStatus = ref<WorkerStatus>({ running: true, paused: true, active_downloads: 0, max_concurrent: 3 })
const rawQueueItems = ref<QueueItem[]>([])



// Settings data for panel (UI-only elements)
const uiSettings = ref({
  servicePriority: [
    { id: 1, name: 'Qobuz' },
    { id: 2, name: 'Tidal' },
    { id: 3, name: 'Deezer' },
    { id: 4, name: 'SoundCloud' },
  ]
})

// Computed: Active downloads (currently downloading)
const activeDownloads = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'downloading')
    .map(item => ({
      id: item.id,
      title: item.title || 'Unknown Track',
      artist: item.artist || 'Unknown Artist',
      album: 'Album',
      artGradient: getArtGradient(item.id),
      service: item.service || 'Unknown',
      serviceBadgeClass: getServiceBadgeClass(item.service),
      quality: item.quality || 'FLAC',
      qualityBadgeClass: 'bg-quality-gold/10 text-quality-gold border border-quality-gold/20',
      progress: item.progress_percent || 0,
      status: item.status,
    }))
})

// Computed: Queue items (waiting to download)
const queueItems = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'queued')
    .map(item => ({
      id: item.id,
      title: item.title || 'Unknown Track',
      artist: item.artist || 'Unknown Artist',
      album: 'Album',
      artGradient: getArtGradient(item.id),
      service: item.service || 'Unknown',
      serviceBadgeClass: getServiceBadgeClass(item.service),
      progress: item.progress_percent || 0,
      status: item.status,
    }))
})

const completedItems = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'complete' || item.status === 'completed')
    .map(item => ({
      id: item.id,
      trackId: item.track_id,
      title: item.title || 'Unknown Track',
      artist: item.artist || 'Unknown Artist',
      artGradient: getArtGradient(item.id),
      service: item.service || 'Unknown',
      serviceBadgeClass: getServiceBadgeClass(item.service),
      quality: item.quality || 'FLAC',
      qualityBadgeClass: 'bg-quality-gold/10 text-quality-gold',
      completedAt: formatTime(item.completed_at ?? undefined),
    }))
})


const failedItems = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'failed')
    .map(item => ({
      id: item.id,
      title: item.title || 'Unknown Track',
      artist: item.artist || 'Unknown Artist',
      album: 'Album',
      artGradient: getArtGradient(item.id),
      service: item.service || 'Unknown',
      serviceBadgeClass: getServiceBadgeClass(item.service),
      errorMessage: item.error_message || 'Download failed',
      errorDetails: item.error_message || 'Unknown error',
      failedAt: formatTime(item.completed_at ?? undefined),
      showDetails: false,
    }))
})

const overallProgress = computed(() => {
  if (activeDownloads.value.length === 0) return 0
  const total = activeDownloads.value.reduce((acc, item) => acc + item.progress, 0)
  return total / activeDownloads.value.length
})

// Helper functions
function getArtGradient(id: number): string {
  const gradients = [
    'bg-gradient-to-br from-red-500 to-pink-600',
    'bg-gradient-to-br from-pink-400 to-rose-500',
    'bg-gradient-to-br from-indigo-500 to-violet-500',
    'bg-gradient-to-br from-amber-400 to-orange-500',
    'bg-gradient-to-br from-cyan-400 to-blue-500',
  ]
  return gradients[id % gradients.length]
}

function getServiceBadgeClass(service: string | undefined): string {
  const classes: Record<string, string> = {
    'spotify': 'bg-[#1ed760]/10 text-[#1ed760] border border-[#1ed760]/20',
    'qobuz': 'bg-[#1a8fe3]/10 text-[#1a8fe3] border border-[#1a8fe3]/20',
    'tidal': 'bg-[#00d4aa]/10 text-[#00d4aa] border border-[#00d4aa]/20',
    'deezer': 'bg-[#ff0092]/10 text-[#ff0092] border border-[#ff0092]/20',
  }
  return classes[(service || '').toLowerCase()] || 'bg-gray-500/10 text-gray-400'
}

function formatTime(dateStr: string | undefined): string {
  if (!dateStr) return 'Recently'
  const date = new Date(dateStr)
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  const hours = Math.floor(diff / (1000 * 60 * 60))
  if (hours < 1) return 'Just now'
  if (hours < 24) return `${hours}h ago`
  return 'Yesterday'
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}


// Drag-and-Drop Reorder
const draggedIndex = ref<number | null>(null)

function onDragStart(index: number) {
  draggedIndex.value = index
}

function onDragOver(e: DragEvent) {
  e.preventDefault()
}

async function onDrop(targetIndex: number) {
  if (draggedIndex.value === null || draggedIndex.value === targetIndex) return

  const currentQueued = [...queueItems.value]
  const [dragged] = currentQueued.splice(draggedIndex.value, 1)
  currentQueued.splice(targetIndex, 0, dragged)

  const orderedIds = currentQueued.map(item => item.id)
  draggedIndex.value = null

  try {
    await invokeCommand('reorder_queue', { queueIds: orderedIds })
    await fetchData()
    toast.success('Queue order updated')
  } catch (e) {
    console.error('Failed to reorder queue:', e)
  }
}

// Fetch data
async function fetchData() {
  loading.value = true
  try {
    const [queue, stats, worker] = await Promise.all([
      queueApi.getQueue(),
      queueApi.getQueueStats(),
      queueApi.getWorkerStatus(),
    ])
    
    rawQueueItems.value = queue
    queueStats.value = stats
    workerStatus.value = worker
  } catch (e) {
    console.error('Failed to fetch queue data:', e)
  } finally {
    loading.value = false
  }
}

// Handle progress events
function handleProgressEvent(event: ProgressEvent) {
  if (event.operation !== 'download') return
  
  const item = rawQueueItems.value.find(q => q.id === parseInt(event.id))
  if (item) {
    item.progress_percent = event.percentage
    if (event.status === 'completed') {
      item.status = 'completed'
      item.completed_at = new Date().toISOString()
    } else if (event.status === 'failed') {
      item.status = 'failed'
      item.error_message = event.message || 'Download failed'
    }
  }
}

// Actions
async function pauseAll() {
  isProcessing.value = true
  try {
    await queueApi.pauseDownloads()
    await fetchData()
  } finally {
    isProcessing.value = false
  }
}

async function resumeAll() {
  isProcessing.value = true
  try {
    await queueApi.resumeDownloads()
    await fetchData()
  } finally {
    isProcessing.value = false
  }
}

async function togglePause() {
  if (isPaused.value) {
    await resumeAll()
  } else {
    await pauseAll()
  }
}


async function clearCompleted() {
  isProcessing.value = true
  try {
    await queueApi.clearQueue('complete')
    await fetchData()
  } finally {
    isProcessing.value = false
  }
}

async function clearPendingQueue() {
  const count = queueItems.value.length
  if (count === 0) return
  const confirmed = await confirm(`Are you sure you want to clear all ${count} pending downloads?`, {
    title: 'Clear Pending Downloads',
    kind: 'warning'
  })
  if (confirmed !== true) return
  
  isProcessing.value = true
  try {
    await queueApi.clearQueue('queued')
    await fetchData()
    toast.success(`Cleared ${count} queued items`)
  } finally {
    isProcessing.value = false
  }
}

async function removeQueueItem(id: number) {
  const confirmed = await confirm('Remove this track from the list?', {
    title: 'Remove Track',
    kind: 'warning'
  })
  if (confirmed !== true) return
  
  isProcessing.value = true
  try {
    await queueApi.removeFromQueue(id)
    rawQueueItems.value = rawQueueItems.value.filter(q => q.id !== id)
  } finally {
    isProcessing.value = false
  }
}

async function showInFolder(trackId: number) {
  if (!trackId) return
  try {
    await invokeCommand('show_in_folder', { trackId })
  } catch (e) {
    toast.error('Could not open folder', String(e))
  }
}


async function retryFailed() {
  isProcessing.value = true
  try {
    await queueApi.retryAllFailed()
    await fetchData()
  } finally {
    isProcessing.value = false
  }
}

async function clearFailed() {
  const confirmed = await confirm('Clear all failed downloads?', {
    title: 'Clear Failed Downloads',
    kind: 'warning'
  })
  if (confirmed !== true) return
  
  isProcessing.value = true
  try {
    await queueApi.clearAllFailed()
    await fetchData()
  } finally {
    isProcessing.value = false
  }
}

async function cancelItem(id: number) {
  const confirmed = await confirm('Cancel this download?', {
    title: 'Cancel Download',
    kind: 'warning'
  })
  if (confirmed !== true) return
  
  isProcessing.value = true
  try {
    await queueApi.cancelItem(id)
    await fetchData()
  } finally {
    isProcessing.value = false
  }
}

async function retryItem(id: number) {
  await queueApi.retryItem(id)
  await fetchData()
}


// Settings
const downloadSettings = useDownloadSettings()
const folderSettings = downloadSettings.folderSettings
const settings = downloadSettings.generalSettings
const saveFolderSettings = downloadSettings.saveFolderSettings
const saveGeneralSettings = downloadSettings.saveGeneralSettings

const loadDownloadSettings = async () => {
  await downloadSettings.loadSettings()

  if (!settings.downloadPath || !settings.downloadPath.trim()) {
    settings.downloadPath = await settingsApi.getDefaultDownloadPath()
  }
}

async function saveSettings() {
  isProcessing.value = true
  try {
    folderSettings.base_folder = settings.downloadPath

    await saveFolderSettings()
    await saveGeneralSettings()
    
    showSettingsPanel.value = false;
    toast.success('Settings saved', 'Download preferences updated');
  } catch (e) {
    toast.error('Failed to save settings', String(e));
  } finally {
    isProcessing.value = false;
  }
}

// Single track execution
async function runSingleTrackDownload() {
  if (!singleTrackQuery.value.trim() || isSingleTrackDownloading.value) return

  isSingleTrackDownloading.value = true
  singleTrackProgress.value = {
    target: singleTrackQuery.value.trim(),
    provider: 'Tidal',
    step: 'Authenticating',
    step_number: 1,
    total_steps: 6,
    message: 'Authenticating active Tidal account session...',
  }
  singleTrackResult.value = null
  singleTrackError.value = null

  try {
    const res = await queueApi.downloadTidalSingleTrack({
      trackIdOrQuery: singleTrackQuery.value.trim(),
      quality: singleTrackQuality.value,
      allowFallback: false,
    })
    singleTrackResult.value = res
    toast.success('Single Track Downloaded', `${res.title} - ${res.artist} (${res.codec})`);
  } catch (err: any) {
    const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err)
    singleTrackError.value = msg
    toast.error('Download Failed', msg)
  } finally {
    isSingleTrackDownloading.value = false
    await fetchData()
  }
}

// Initialize
onMounted(async () => {
  await loadDownloadSettings()
  await fetchData()
  on<ProgressEvent>(TauriEvents.DOWNLOAD_PROGRESS, handleProgressEvent)

  unlistenPipelineProgress = await listen<PipelineProgressEvent>('pipeline:progress', (event) => {
    singleTrackProgress.value = event.payload
  })
  unlistenSyncifyProgress = await listen<PipelineProgressEvent>('syncify:progress', (event) => {
    singleTrackProgress.value = event.payload
  })
})

onUnmounted(() => {
  if (unlistenPipelineProgress) unlistenPipelineProgress()
  if (unlistenSyncifyProgress) unlistenSyncifyProgress()
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

/* Slow spin animation for active icon */
@keyframes spin-slow {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.animate-spin-slow {
  animation: spin-slow 2s linear infinite;
}

/* Progress bar shine effect */
@keyframes shine {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(200%);
  }
}

.progress-shine {
  animation: shine 2s ease-in-out infinite;
  background: linear-gradient(
    90deg,
    transparent,
    rgba(255, 255, 255, 0.3),
    transparent
  );
}

/* Progress bar animation */
.progress-bar-animated {
  position: relative;
  overflow: hidden;
}

/* Drag cursor */
.cursor-grab {
  cursor: grab;
}

.cursor-grab:active {
  cursor: grabbing;
}

/* Collapse transition */
.collapse-enter-active,
.collapse-leave-active {
  transition: all 0.3s ease;
  overflow: hidden;
}
.collapse-enter-from,
.collapse-leave-to {
  opacity: 0;
  max-height: 0;
  transform: translateY(-10px);
}
.collapse-enter-to,
.collapse-leave-from {
  opacity: 1;
  max-height: 1000px;
}

/* Expand transition for error details */
.expand-enter-active,
.expand-leave-active {
  transition: all 0.2s ease;
  overflow: hidden;
}
.expand-enter-from,
.expand-leave-to {
  opacity: 0;
  max-height: 0;
  transform: scaleY(0.95);
}
.expand-enter-to,
.expand-leave-from {
  opacity: 1;
  max-height: 200px;
  transform: scaleY(1);
}

/* Slide-right transition for settings panel */
.slide-right-enter-active,
.slide-right-leave-active {
  transition: transform 0.3s ease;
}
.slide-right-enter-from,
.slide-right-leave-to {
  transform: translateX(100%);
}
.slide-right-enter-to,
.slide-right-leave-from {
  transform: translateX(0);
}

/* Fade transition for overlay and modals */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Range input styling */
input[type="range"] {
  -webkit-appearance: none;
  appearance: none;
  background: transparent;
}
input[type="range"]::-webkit-slider-runnable-track {
  width: 100%;
  height: 8px;
  background: linear-gradient(to right, rgb(59, 130, 246) 0%, rgb(59, 130, 246) var(--value, 0%), #374151 var(--value, 0%), #374151 100%);
  border-radius: 4px;
}
input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 18px;
  height: 18px;
  background: white;
  border-radius: 50%;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
  cursor: pointer;
  margin-top: -5px;
}
</style>
