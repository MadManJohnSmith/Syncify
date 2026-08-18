<template>
  <div class="downloads-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    
    <!-- Compact Page Header -->
    <div class="px-8 pt-6 pb-3 flex items-center justify-between shrink-0 flex-wrap gap-3">
      <div class="flex items-center gap-3">
        <div>
          <h1 class="text-2xl font-bold tracking-tight text-gray-900 dark:text-white">Downloads</h1>
          <p class="text-xs text-text-secondary">Track progress, control concurrency, and manage queue.</p>
        </div>
      </div>

      <!-- Status Pills, Concurrency & Queue Details Toggle -->
      <div class="flex items-center gap-2 flex-wrap">
        <!-- Active Pill -->
        <button 
          @click="viewFilter = 'active'"
          :class="['flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-all cursor-pointer shadow-xs', viewFilter === 'active' ? 'bg-primary/15 border-primary text-primary' : 'bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight']"
          title="Filter active downloads"
        >
          <span class="material-symbols-outlined text-[15px] text-primary animate-spin-slow">sync</span>
          <span>Active:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.active ?? queueStats?.downloading ?? activeDownloads.length }}</strong>
        </button>

        <!-- Queued Pill -->
        <button 
          @click="viewFilter = 'queued'"
          :class="['flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-all cursor-pointer shadow-xs', viewFilter === 'queued' ? 'bg-amber-500/15 border-amber-500 text-amber-500' : 'bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight']"
          title="Filter queued tracks"
        >
          <span class="material-symbols-outlined text-[15px] text-amber-500">schedule</span>
          <span>Queued:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.queued ?? queueItems.length }}</strong>
        </button>

        <!-- Completed Pill -->
        <button 
          @click="viewFilter = 'completed'"
          :class="['flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-all cursor-pointer shadow-xs', viewFilter === 'completed' ? 'bg-success/15 border-success text-success' : 'bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight']"
          title="Filter completed downloads"
        >
          <span class="material-symbols-outlined text-[15px] text-success">check_circle</span>
          <span>Completed:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.completed ?? completedItems.length }}</strong>
        </button>

        <!-- Failed Pill -->
        <button 
          @click="viewFilter = 'failed'"
          :class="['flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-all cursor-pointer shadow-xs', viewFilter === 'failed' ? 'bg-error/15 border-error text-error' : 'bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight']"
          title="Filter failed downloads"
        >
          <span class="material-symbols-outlined text-[15px] text-error">cancel</span>
          <span>Failed:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.failed ?? failedItems.length }}</strong>
        </button>

        <div class="h-5 w-px bg-gray-200 dark:bg-border-dark mx-1 hidden sm:block"></div>

        <!-- Concurrency Selector -->
        <div class="flex items-center gap-1 bg-white dark:bg-surface-dark px-2.5 py-1 rounded-lg border border-gray-200 dark:border-border-dark shadow-xs" title="Concurrent download threads">
          <span class="material-symbols-outlined text-[16px] text-primary mr-1">bolt</span>
          <span class="text-[11px] font-bold text-text-secondary mr-1">{{ currentConcurrency }} Threads</span>
          <button 
            v-for="t in [1, 2, 3, 4, 5]" 
            :key="t"
            @click="setConcurrency(t)"
            :title="`Set ${t} concurrent download thread${t > 1 ? 's' : ''}`"
            :class="[
              'w-5 h-5 rounded text-[10px] font-bold transition-all flex items-center justify-center',
              currentConcurrency === t 
                ? 'bg-primary text-white shadow-xs' 
                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-surface-highlight'
            ]"
          >
            {{ t }}
          </button>
        </div>

        <!-- Queue Details Toggle -->
        <button 
          @click="showQueueDetails = !showQueueDetails"
          :class="[
            'flex items-center gap-1 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-all shadow-xs',
            showQueueDetails 
              ? 'bg-primary/10 border-primary/40 text-primary' 
              : 'bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-text-secondary hover:text-gray-900 dark:hover:text-white'
          ]"
          title="Toggle extended metrics and reconciliation audit panel"
        >
          <span class="material-symbols-outlined text-[16px]">tune</span>
          <span>Queue details</span>
          <span :class="['material-symbols-outlined text-[16px] transition-transform', showQueueDetails ? 'rotate-180' : '']">expand_more</span>
        </button>
      </div>
    </div>

    <!-- Collapsible Queue Details Panel (Reconciliation, Telemetry, Sidecars/Artifacts) -->
    <div v-show="showQueueDetails" class="mx-8 mb-3 space-y-2 transition-all">
      <!-- Reconciled Queue Audit Strip -->
      <div class="reconciliation-strip px-4 py-2 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-xs flex items-center justify-between gap-3 text-xs flex-wrap">
        <div class="flex items-center gap-1.5 font-semibold text-text-secondary shrink-0">
          <span class="material-symbols-outlined text-[16px] text-primary">analytics</span>
          <span class="uppercase tracking-wider text-[10px]">Queue Reconciliation:</span>
        </div>
        <div class="flex items-center gap-3 flex-wrap text-text-secondary text-[11px]">
          <span title="Total tracks requested by batch / UI action">Submitted: <strong class="text-gray-900 dark:text-white">{{ queueStats?.submitted ?? totalItemCount }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks currently in queued state">Queued: <strong class="text-amber-500">{{ queueStats?.queued ?? queueItems.length }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks currently downloading concurrently">Active: <strong class="text-primary">{{ queueStats?.active ?? queueStats?.downloading ?? activeDownloads.length }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Finished downloads in queue">Completed: <strong class="text-success">{{ queueStats?.completed ?? completedItems.length }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Failed downloads in queue">Failed: <strong class="text-error">{{ queueStats?.failed ?? failedItems.length }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks skipped due to missing/stale/ambiguous sources">Skipped: <strong class="text-gray-500">{{ queueStats?.skipped ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks deduplicated against active queue or existing downloads">Deduplicated: <strong class="text-blue-400">{{ queueStats?.deduplicated ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Physical audio files saved on disk in downloads library">Physical Files: <strong class="text-emerald-400">{{ queueStats?.physical_files ?? queueStats?.downloads_count ?? completedItems.length }}</strong></span>
        </div>
      </div>

      <!-- Live Telemetry & Generated Artifacts Bar -->
      <div class="telemetry-bar px-4 py-2.5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-xs flex items-center justify-between gap-4 flex-wrap">
        <!-- Speed & ETA & Health -->
        <div class="flex items-center gap-5 flex-wrap">
          <!-- Live Speed / Throughput -->
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px] text-blue-500">speed</span>
            <div>
              <div class="flex items-center gap-1">
                <span class="text-[9px] font-bold text-text-secondary uppercase tracking-wider block leading-none">Throughput</span>
                <span v-if="activeDownloads.length > 0 && !isPaused" class="px-1 py-0.2 rounded text-[8px] font-bold bg-blue-500/15 text-blue-400">LIVE</span>
              </div>
              <span class="text-xs font-extrabold text-gray-900 dark:text-white font-mono leading-tight">{{ formattedThroughput }}</span>
            </div>
          </div>

          <div class="w-px h-6 bg-gray-200 dark:bg-border-dark"></div>

          <!-- Estimated Time Remaining (ETA) -->
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px] text-purple-400">timer</span>
            <div>
              <span class="text-[9px] font-bold text-text-secondary uppercase tracking-wider block leading-none">Est. Time Remaining</span>
              <span class="text-xs font-extrabold text-gray-900 dark:text-white font-mono leading-tight">{{ formattedEta }}</span>
            </div>
          </div>

          <div class="w-px h-6 bg-gray-200 dark:bg-border-dark"></div>

          <!-- Success Rate -->
          <div class="flex items-center gap-2">
            <span class="material-symbols-outlined text-[18px] text-emerald-400">verified</span>
            <div>
              <span class="text-[9px] font-bold text-text-secondary uppercase tracking-wider block leading-none">Success Rate</span>
              <span class="text-xs font-extrabold leading-tight" :class="successRate >= 90 ? 'text-emerald-500 dark:text-emerald-400' : (successRate >= 75 ? 'text-amber-500' : 'text-error')">
                {{ successRate }}%
              </span>
            </div>
          </div>
        </div>

        <!-- Right: Generated Artifacts / Sidecars Counters -->
        <div class="flex items-center gap-2 flex-wrap">
          <span class="text-[10px] font-bold text-text-secondary uppercase tracking-wider mr-1">Generated Artifacts:</span>
          
          <div class="flex items-center gap-1 px-2.5 py-1 rounded bg-gray-50 dark:bg-[#1a2333] border border-gray-200 dark:border-border-dark/70 text-xs" title="Audio Tracks Generated (FLAC/MP3)">
            <span class="material-symbols-outlined text-[14px] text-primary">audiotrack</span>
            <span class="text-text-secondary text-[11px]">Audio</span>
            <span class="font-bold font-mono text-primary text-[11px]">{{ artifactCounters.audio }}</span>
          </div>

          <div class="flex items-center gap-1 px-2.5 py-1 rounded bg-gray-50 dark:bg-[#1a2333] border border-gray-200 dark:border-border-dark/70 text-xs" title="Synced Lyrics Sidecars (.lrc)">
            <span class="material-symbols-outlined text-[14px] text-amber-500">lyrics</span>
            <span class="text-text-secondary text-[11px]">LRC</span>
            <span class="font-bold font-mono text-amber-500 text-[11px]">{{ artifactCounters.lrc }}</span>
          </div>

          <div class="flex items-center gap-1 px-2.5 py-1 rounded bg-gray-50 dark:bg-[#1a2333] border border-gray-200 dark:border-border-dark/70 text-xs" title="Album Artwork Portadas">
            <span class="material-symbols-outlined text-[14px] text-pink-500">image</span>
            <span class="text-text-secondary text-[11px]">Covers</span>
            <span class="font-bold font-mono text-pink-500 text-[11px]">{{ artifactCounters.covers }}</span>
          </div>

          <div class="flex items-center gap-1 px-2.5 py-1 rounded bg-gray-50 dark:bg-[#1a2333] border border-gray-200 dark:border-border-dark/70 text-xs" title="Digital Booklets Sidecars (.pdf)">
            <span class="material-symbols-outlined text-[14px] text-cyan-500">menu_book</span>
            <span class="text-text-secondary text-[11px]">Booklets</span>
            <span class="font-bold font-mono text-cyan-500 text-[11px]">{{ artifactCounters.booklets }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Downloads Control Bar: Progress, Fast Action Buttons, Tabs & Search -->
    <div class="downloads-toolbar mx-8 mb-3 p-3 rounded-2xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-xs flex flex-col gap-3 shrink-0">
      <!-- Top Row: Compact Global Progress & Frequent Operational Actions -->
      <div class="flex items-center justify-between gap-3 flex-wrap">
        <!-- Progress Summary & Micro Bar -->
        <div class="flex items-center gap-3 flex-1 min-w-[260px]">
          <div class="flex items-center gap-2 text-xs font-semibold text-gray-700 dark:text-gray-300 shrink-0">
            <span class="material-symbols-outlined text-[18px] text-primary">download</span>
            <span>Downloading <strong class="text-gray-900 dark:text-white">{{ activeDownloads.length }}</strong> of <strong class="text-gray-900 dark:text-white">{{ totalItemCount }}</strong></span>
            <span v-if="searchQuery.trim()" class="text-[11px] text-primary font-medium">
              ({{ matchingCount }} match)
            </span>
          </div>
          
          <div class="flex-1 max-w-xs relative h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden shrink-0">
            <div 
              class="absolute inset-0 bg-gradient-to-r from-primary to-blue-400 rounded-full progress-bar-animated transition-all duration-300"
              :style="{ width: overallProgress + '%' }"
            ></div>
          </div>
          <span class="text-[11px] font-bold text-primary font-mono shrink-0">{{ Math.round(overallProgress) }}%</span>
        </div>

        <!-- Frequent Primary Actions -->
        <div class="flex items-center gap-2 flex-wrap">
          <!-- Pause / Resume -->
          <button 
            @click="togglePause" 
            :disabled="isProcessing" 
            class="flex items-center gap-1.5 px-3 py-1.5 bg-gray-100 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-200 rounded-lg text-xs font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-xs"
            :title="isPaused ? 'Resume all downloads' : 'Pause all downloads'"
          >
            <span class="material-symbols-outlined text-[16px]">{{ isPaused ? 'play_arrow' : 'pause' }}</span>
            <span>{{ isPaused ? 'Resume All' : 'Pause All' }}</span>
          </button>

          <!-- Cancel / Clear Queue -->
          <button 
            @click="clearPendingQueue" 
            :disabled="isProcessing || queueItems.length === 0" 
            class="flex items-center gap-1.5 px-3 py-1.5 bg-gray-100 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-200 rounded-lg text-xs font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-xs"
            title="Cancel queued downloads"
          >
            <span class="material-symbols-outlined text-[16px]">close</span>
            <span>Cancel</span>
          </button>

          <!-- Retry Failed -->
          <button 
            @click="retryFailed" 
            :disabled="isProcessing || failedItems.length === 0" 
            class="flex items-center gap-1.5 px-3 py-1.5 bg-amber-500/10 border border-amber-500/30 hover:bg-amber-500/20 text-amber-600 dark:text-amber-400 rounded-lg text-xs font-semibold transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-xs"
            title="Retry failed downloads"
          >
            <span class="material-symbols-outlined text-[16px]">refresh</span>
            <span>Retry Failed</span>
          </button>

          <!-- Refresh Button -->
          <button 
            @click="fetchData" 
            :disabled="loading" 
            class="p-1.5 bg-gray-100 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg transition-colors disabled:opacity-50 shadow-xs" 
            title="Refresh Queue"
          >
            <span :class="['material-symbols-outlined text-[16px]', loading ? 'animate-spin' : '']">refresh</span>
          </button>

          <!-- Secondary Menu Dropdown -->
          <div class="relative">
            <button 
              @click="showSecondaryMenu = !showSecondaryMenu" 
              class="flex items-center gap-1 px-2.5 py-1.5 bg-gray-100 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-xs font-semibold transition-colors shadow-xs"
              title="More actions"
            >
              <span>More</span>
              <span class="material-symbols-outlined text-[16px]">more_vert</span>
            </button>

            <!-- Dropdown Menu -->
            <div 
              v-if="showSecondaryMenu" 
              class="absolute right-0 top-full mt-1 w-52 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl shadow-xl z-30 py-1 overflow-hidden"
              @click="showSecondaryMenu = false"
            >
              <button 
                @click="showDownloadFavoritesModal = true" 
                :disabled="isProcessing" 
                class="w-full flex items-center gap-2 px-3 py-2 text-xs font-semibold text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors"
              >
                <span class="material-symbols-outlined text-[16px] text-red-500">favorite</span>
                Download Favorites
              </button>
              <button 
                @click="clearCompleted" 
                :disabled="isProcessing || completedItems.length === 0" 
                class="w-full flex items-center gap-2 px-3 py-2 text-xs font-semibold text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors disabled:opacity-50"
              >
                <span class="material-symbols-outlined text-[16px] text-gray-400">delete_sweep</span>
                Clear Completed
              </button>
              <button 
                @click="clearFailed" 
                :disabled="isProcessing || failedItems.length === 0" 
                class="w-full flex items-center gap-2 px-3 py-2 text-xs font-semibold text-error hover:bg-error/10 transition-colors disabled:opacity-50"
              >
                <span class="material-symbols-outlined text-[16px]">delete</span>
                Clear Failed
              </button>
              <div class="h-px bg-gray-200 dark:bg-border-dark my-1"></div>
              <button 
                @click="showSettingsPanel = true" 
                class="w-full flex items-center gap-2 px-3 py-2 text-xs font-semibold text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors"
              >
                <span class="material-symbols-outlined text-[16px] text-primary">settings</span>
                Download Settings
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Bottom Row: Filter Tabs & Search Bar -->
      <div class="flex items-center justify-between gap-3 flex-wrap">
        <!-- Filter Tabs -->
        <div class="flex items-center gap-1 bg-gray-100 dark:bg-surface-highlight p-1 rounded-xl border border-gray-200 dark:border-border-dark/60">
          <button 
            v-for="tab in filterTabs" 
            :key="tab.value"
            @click="viewFilter = tab.value"
            :class="[
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold transition-all',
              viewFilter === tab.value 
                ? 'bg-primary text-white shadow-xs' 
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-white dark:hover:bg-surface-dark'
            ]"
          >
            <span class="material-symbols-outlined text-[15px]">{{ tab.icon }}</span>
            <span>{{ tab.label }}</span>
            <span :class="['px-1.5 py-0.2 rounded-full text-[10px] font-bold', viewFilter === tab.value ? 'bg-white/20 text-white' : 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300']">
              {{ tab.count }}
            </span>
          </button>
        </div>
        
        <!-- Search Input -->
        <div class="relative flex-1 max-w-md min-w-[220px]">
          <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 material-symbols-outlined text-[16px]">search</span>
          <input 
            v-model="searchQuery"
            type="text" 
            placeholder="Filter queue by title, artist, album or service..." 
            class="w-full pl-8 pr-8 py-1.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark/70 rounded-xl text-xs text-gray-900 dark:text-white placeholder:text-gray-400 focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all"
          >
          <button 
            v-if="searchQuery" 
            @click="searchQuery = ''" 
            class="absolute right-2.5 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 p-0.5"
            title="Clear search"
          >
            <span class="material-symbols-outlined text-[14px]">close</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Scrollable Content Area -->
    <div class="flex-1 overflow-y-auto custom-scrollbar px-8 pb-8 space-y-6">

      <!-- Active Downloads Section -->
      <div v-if="(viewFilter === 'all' || viewFilter === 'active') && (filteredActiveDownloads.length > 0 || !searchQuery)" class="active-downloads">
        <div class="flex items-center justify-between mb-3">
          <div class="flex items-center gap-2.5">
            <h2 class="text-base font-bold text-gray-900 dark:text-white">Active Downloads</h2>
            <span class="px-2 py-0.5 rounded-full bg-primary/10 text-primary text-xs font-bold">{{ filteredActiveDownloads.length }}</span>
          </div>
        </div>
        
        <div class="flex flex-col gap-3">
          <!-- Empty state when no active downloads -->
          <div v-if="filteredActiveDownloads.length === 0" class="rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark p-6 text-center">
            <span class="material-symbols-outlined text-3xl text-gray-300 dark:text-gray-600 mb-1">cloud_done</span>
            <p class="text-xs text-text-secondary">No active downloads running</p>
          </div>
          
          <!-- Active Download Items -->
          <div 
            v-for="item in filteredActiveDownloads" 
            :key="item.id"
            class="download-item relative overflow-hidden rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark p-4 shadow-sm"
          >
            <div class="flex items-center gap-4">
              <!-- Album Art -->
              <div :class="['w-14 h-14 rounded-lg shrink-0 flex items-center justify-center text-white/30', item.artGradient]">
                <span class="material-symbols-outlined text-2xl">album</span>
              </div>
              
              <!-- Track Info -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2.5 mb-1">
                  <h3 class="text-sm font-semibold text-gray-900 dark:text-white truncate">{{ item.title }}</h3>
                  <span :class="['px-2 py-0.5 rounded text-[10px] font-bold uppercase', item.serviceBadgeClass]">{{ item.service }}</span>
                  <span :class="['px-2 py-0.5 rounded text-[10px] font-bold', item.qualityBadgeClass]">{{ item.quality }}</span>
                </div>
                <p class="text-xs text-text-secondary truncate mb-2">{{ item.artist }} • {{ item.album }}</p>
                
                <!-- Individual Progress Bar -->
                <div class="item-progress">
                  <div class="flex items-center gap-3 mb-1">
                    <div class="flex-1 h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden relative">
                      <!-- Determinate progress bar if percentage is calculable -->
                      <div 
                        v-if="item.percent !== null && item.percent !== undefined" 
                        class="h-full bg-primary rounded-full transition-all duration-200" 
                        :style="{ width: Math.min(100, Math.max(0, item.percent)) + '%' }"
                      ></div>
                      <!-- Indeterminate shimmer animation if Content-Length missing -->
                      <div 
                        v-else 
                        class="h-full w-full bg-gradient-to-r from-primary/30 via-primary to-primary/30 rounded-full animate-pulse"
                      ></div>
                    </div>
                    <span v-if="item.percent !== null && item.percent !== undefined" class="text-xs font-bold text-primary w-12 text-right font-mono">
                      {{ Math.round(item.percent) }}%
                    </span>
                    <span v-else class="text-xs font-bold text-primary/70 w-12 text-right font-mono">
                      -- %
                    </span>
                  </div>
                  <div class="flex items-center justify-between text-[10px] text-text-secondary">
                    <span v-if="item.totalBytes">
                      {{ formatBytes(item.bytesDownloaded) }} / {{ formatBytes(item.totalBytes) }} • {{ formatSpeed(item.instantKbps || throughputKbps) }}
                    </span>
                    <span v-else-if="item.bytesDownloaded > 0">
                      {{ formatBytes(item.bytesDownloaded) }} downloaded • {{ formatSpeed(item.instantKbps || throughputKbps) }}
                    </span>
                    <span v-else>
                      Connecting stream...
                    </span>
                    <span class="flex items-center gap-1 text-primary font-medium">
                      <span class="material-symbols-outlined text-[13px] animate-spin">sync</span>
                      {{ item.phase === 'finalizing' ? 'Finalizing' : 'In progress' }}
                    </span>
                  </div>
                </div>
              </div>
              
              <!-- Action Buttons -->
              <div class="flex items-center gap-1.5 shrink-0">
                <button @click="cancelItem(item.id)" class="p-2 text-gray-400 hover:text-error hover:bg-error/10 rounded-lg transition-all" title="Cancel">
                  <span class="material-symbols-outlined text-[20px]">close</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Up Next (Queue) Section with High-Performance Virtual Scrolling -->
      <div v-if="(viewFilter === 'all' || viewFilter === 'queued')" class="queue-section">
        <div class="flex items-center justify-between mb-3 flex-wrap gap-2">
          <div class="flex items-center gap-2.5">
            <h2 class="text-base font-bold text-gray-900 dark:text-white">Up Next</h2>
            <span class="px-2 py-0.5 rounded-full bg-gray-100 dark:bg-surface-highlight text-text-secondary text-xs font-bold">
              {{ filteredQueueItems.length.toLocaleString() }}
            </span>
            <span v-if="filteredQueueItems.length > 0" class="text-xs text-text-secondary font-mono">
              (Virtual Window: {{ virtualStartIndex + 1 }} - {{ Math.min(virtualEndIndex, filteredQueueItems.length) }})
            </span>
          </div>

          <div class="flex items-center gap-3">
            <!-- Jump to Position Widget for Mass Queues -->
            <div v-if="filteredQueueItems.length > 50" class="flex items-center gap-1 text-xs text-text-secondary">
              <span>Jump to #</span>
              <input 
                v-model.number="jumpPositionInput" 
                @keyup.enter="jumpToPosition"
                type="number" 
                min="1" 
                :max="filteredQueueItems.length" 
                placeholder="Track #" 
                class="w-16 px-2 py-1 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-md text-xs text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary"
              >
              <button @click="jumpToPosition" class="px-2 py-1 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 rounded-md font-medium text-xs transition-colors">Go</button>
            </div>

            <button @click="clearPendingQueue" :disabled="isProcessing || filteredQueueItems.length === 0" class="text-xs font-semibold text-primary hover:text-primary-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Clear All</button>
            <button @click="pauseAll" :disabled="isProcessing" class="text-xs font-semibold text-text-secondary hover:text-gray-900 dark:text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed">Pause All</button>
          </div>
        </div>
        
        <!-- Virtual Scroller Container for 10,000+ Items -->
        <div class="border border-gray-200 dark:border-border-dark rounded-xl bg-white dark:bg-surface-dark overflow-hidden shadow-sm">
          <div v-if="filteredQueueItems.length === 0" class="p-8 text-center text-text-secondary text-xs">
            <span class="material-symbols-outlined text-3xl text-gray-300 dark:text-gray-600 mb-1">queue_music</span>
            <p>No tracks queued in Up Next</p>
          </div>

          <div 
            v-else
            ref="queueContainerRef" 
            @scroll="onQueueScroll"
            class="max-h-[520px] overflow-y-auto custom-scrollbar relative"
            style="contain: strict;"
          >
            <!-- Virtual scroll spacer -->
            <div :style="{ height: totalVirtualHeight + 'px', position: 'relative', width: '100%' }">
              <!-- Rendered visible items window -->
              <div :style="{ transform: `translateY(${virtualOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
                <div 
                  v-for="item in visibleQueueItems" 
                  :key="item.id"
                  :style="{ height: ROW_HEIGHT + 'px' }"
                  draggable="true"
                  @dragstart="onDragStart(item.absoluteIndex)"
                  @dragover="onDragOver"
                  @drop="onDrop(item.absoluteIndex)"
                  class="queue-item flex items-center gap-3.5 px-4 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors group cursor-grab active:cursor-grabbing select-none"
                >
                  <!-- Drag Handle -->
                  <span class="material-symbols-outlined text-[16px] text-gray-300 dark:text-gray-600 group-hover:text-gray-400 cursor-grab shrink-0">drag_indicator</span>
                  
                  <!-- Queue Position -->
                  <span class="text-xs text-gray-400 w-10 text-right font-mono font-medium shrink-0">{{ item.absoluteIndex + 1 }}</span>
                  
                  <!-- Album Art (compact) -->
                  <div :class="['w-9 h-9 rounded-md shrink-0 flex items-center justify-center', item.artGradient]">
                    <span class="material-symbols-outlined text-lg text-white/40">music_note</span>
                  </div>
                  
                  <!-- Track Info -->
                  <div class="flex-1 min-w-0">
                    <p class="text-xs font-semibold text-gray-900 dark:text-white truncate">{{ item.title }}</p>
                    <p class="text-[11px] text-text-secondary truncate">{{ item.artist }} • {{ item.album }}</p>
                  </div>
                  
                  <!-- Service Badge (small) -->
                  <span :class="['px-2 py-0.5 rounded text-[9px] font-bold uppercase shrink-0', item.serviceBadgeClass]">{{ item.service }}</span>
                  
                  <!-- Status Badge -->
                  <span class="px-2 py-0.5 rounded-full text-[9px] font-bold bg-gray-100 dark:bg-surface-highlight text-text-secondary uppercase tracking-wide shrink-0">Queued</span>
                  
                  <!-- Remove Button -->
                  <button @click.stop="removeQueueItem(item.id)" class="opacity-0 group-hover:opacity-100 p-1.5 text-gray-400 hover:text-error hover:bg-error/10 rounded-md transition-all shrink-0" title="Remove from queue">
                    <span class="material-symbols-outlined text-[16px]">close</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Completed Downloads Section (Collapsible + Windowed) -->
      <div v-if="(viewFilter === 'all' || viewFilter === 'completed')" class="completed-section">
        <div class="flex items-center justify-between mb-3">
          <button @click="showCompleted = !showCompleted" class="flex items-center gap-2.5 group">
            <span :class="['material-symbols-outlined text-[18px] text-gray-400 transition-transform', showCompleted ? 'rotate-0' : '-rotate-90']">expand_more</span>
            <h2 class="text-base font-bold text-gray-900 dark:text-white group-hover:text-primary transition-colors">Completed</h2>
            <span class="px-2 py-0.5 rounded-full bg-success/10 text-success text-xs font-bold">{{ filteredCompletedItems.length }}</span>
          </button>
          <button @click="clearCompleted" :disabled="filteredCompletedItems.length === 0" class="text-xs font-semibold text-text-secondary hover:text-error transition-colors disabled:opacity-50">Clear All</button>
        </div>
        
        <Transition name="collapse">
          <div v-if="showCompleted" class="border border-gray-200 dark:border-border-dark rounded-xl bg-white dark:bg-surface-dark overflow-hidden shadow-sm">
            <div v-if="filteredCompletedItems.length === 0" class="p-6 text-center text-text-secondary text-xs">
              <p>No completed downloads yet</p>
            </div>

            <!-- Completed Items (compact windowed list) -->
            <div v-else class="divide-y divide-gray-100 dark:divide-border-dark/50 max-h-[380px] overflow-y-auto custom-scrollbar">
              <div 
                v-for="item in visibleCompletedSlice" 
                :key="item.id"
                class="completed-item flex items-center gap-3.5 px-4 py-2.5 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors group"
              >
                <!-- Album Art (36x36) -->
                <div :class="['w-9 h-9 rounded-md shrink-0 flex items-center justify-center', item.artGradient]">
                  <span class="material-symbols-outlined text-lg text-white/40">music_note</span>
                </div>
                
                <!-- Track Info -->
                <div class="flex-1 min-w-0">
                  <p class="text-xs font-semibold text-gray-900 dark:text-white truncate">{{ item.title }}</p>
                  <p class="text-[11px] text-text-secondary truncate">{{ item.artist }}</p>
                </div>
                
                <!-- Badges -->
                <span :class="['px-1.5 py-0.5 rounded text-[9px] font-bold uppercase shrink-0', item.serviceBadgeClass]">{{ item.service }}</span>
                <span :class="['px-1.5 py-0.5 rounded text-[9px] font-bold shrink-0', item.qualityBadgeClass]">{{ item.quality }}</span>
                
                <!-- Completion Time -->
                <span class="text-xs text-text-secondary w-20 text-right shrink-0">{{ item.completedAt }}</span>
                
                <!-- Success Icon -->
                <span class="material-symbols-outlined text-[18px] text-success shrink-0">check_circle</span>
                
                <!-- Hover Actions -->
                <div class="opacity-0 group-hover:opacity-100 flex items-center gap-1 transition-opacity shrink-0">
                  <button 
                    @click="showInFolder(item.trackId)" 
                    :disabled="!item.trackId"
                    :class="['p-1 rounded transition-colors', item.trackId ? 'text-gray-400 hover:text-primary' : 'text-gray-600 opacity-50 cursor-not-allowed']" 
                    title="Show in Folder"
                  >
                    <span class="material-symbols-outlined text-[16px]">folder_open</span>
                  </button>
                  <button @click="removeQueueItem(item.id)" class="p-1 text-gray-400 hover:text-error rounded transition-colors" title="Remove from List">
                    <span class="material-symbols-outlined text-[16px]">close</span>
                  </button>
                </div>
              </div>

              <!-- Load more completed if large list -->
              <button 
                v-if="completedLimit < filteredCompletedItems.length" 
                @click="completedLimit += 50"
                class="w-full py-2.5 text-xs font-semibold text-primary hover:text-primary-hover hover:bg-primary/5 transition-colors border-t border-gray-100 dark:border-border-dark flex items-center justify-center gap-1"
              >
                <span>Show More ({{ (filteredCompletedItems.length - completedLimit).toLocaleString() }} remaining)</span>
                <span class="material-symbols-outlined text-[16px]">expand_more</span>
              </button>
            </div>
          </div>
        </Transition>
      </div>

      <!-- Failed Downloads Section -->
      <div v-if="(viewFilter === 'all' || viewFilter === 'failed')" class="failed-section">
        <div class="flex items-center justify-between mb-3">
          <div class="flex items-center gap-2.5">
            <h2 class="text-base font-bold text-gray-900 dark:text-white">Failed</h2>
            <span class="px-2 py-0.5 rounded-full bg-error/10 text-error text-xs font-bold">{{ filteredFailedItems.length }}</span>
          </div>
          <button 
            v-if="filteredFailedItems.length > 0"
            @click="retryFailed" 
            class="flex items-center gap-1.5 px-3 py-1 bg-amber-500/10 border border-amber-500/30 hover:bg-amber-500/20 text-amber-600 dark:text-amber-400 rounded-lg text-xs font-semibold transition-colors"
          >
            <span class="material-symbols-outlined text-[14px]">refresh</span>
            Retry All
          </button>
        </div>
        
        <div class="flex flex-col gap-3">
          <div v-if="filteredFailedItems.length === 0" class="p-6 text-center text-text-secondary text-xs rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark">
            <p>No failed downloads</p>
          </div>

          <!-- Failed Item Cards (windowed to prevent DOM overload) -->
          <div 
            v-for="item in visibleFailedSlice" 
            :key="item.id"
            class="failed-item relative overflow-hidden rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm"
          >
            <!-- Left accent border based on failure reason -->
            <div 
              :class="[
                'absolute left-0 top-0 bottom-0 w-1.5',
                item.failure.reason === 'network' ? 'bg-blue-500' :
                item.failure.reason === 'stale_source' ? 'bg-orange-500' :
                item.failure.reason === 'requires_auth' ? 'bg-rose-500' :
                item.failure.reason === 'rejected_quality' ? 'bg-purple-500' :
                item.failure.reason === 'cancelled' ? 'bg-gray-400' :
                item.failure.reason === 'ambiguous_source' ? 'bg-yellow-500' : 'bg-error'
              ]"
            ></div>
            
            <div class="flex items-start gap-4 p-4 pl-5">
              <!-- Album Art (52x52) -->
              <div :class="['w-13 h-13 rounded-lg shrink-0 flex items-center justify-center text-white/30', item.artGradient]">
                <span class="material-symbols-outlined text-2xl">album</span>
              </div>
              
              <!-- Track Info + Error Classification -->
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2 mb-1 flex-wrap">
                  <h3 class="text-sm font-semibold text-gray-900 dark:text-white truncate">{{ item.title }}</h3>
                  
                  <!-- Failure Classification Badge -->
                  <span :class="['px-2 py-0.5 rounded text-[10px] font-bold uppercase flex items-center gap-1 shrink-0', item.failure.badgeClass]">
                    <span class="material-symbols-outlined text-[13px]">{{ item.failure.icon }}</span>
                    {{ item.failure.label }}
                  </span>

                  <span :class="['px-2 py-0.5 rounded text-[10px] font-bold uppercase shrink-0', item.serviceBadgeClass]">
                    {{ item.service }}
                  </span>
                </div>

                <p class="text-xs text-text-secondary truncate mb-2">{{ item.artist }} • {{ item.album }}</p>
                
                <!-- Failure Metadata Grid: Attempts, Provenance/Origin, Effective, Fallback, Timestamp -->
                <div class="flex items-center gap-2.5 flex-wrap text-[11px] text-text-secondary mb-2 bg-gray-50 dark:bg-surface-highlight/40 px-2.5 py-1.5 rounded-lg border border-gray-100 dark:border-border-dark/40">
                  <span class="font-mono text-gray-700 dark:text-gray-300">
                    <strong>Attempts:</strong> {{ item.retryCount }}
                  </span>
                  <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
                  <span>
                    Origin: <strong class="text-gray-900 dark:text-white capitalize">{{ formatServiceName(item.originalService) }}</strong>
                  </span>
                  <template v-if="item.effectiveService && item.effectiveService.toLowerCase() !== item.originalService.toLowerCase()">
                    <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
                    <span>
                      Effective: <strong class="text-primary capitalize">{{ formatServiceName(item.effectiveService) }}</strong>
                    </span>
                  </template>
                  <template v-if="item.failure.reason === 'stale_source'">
                    <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
                    <span :class="['px-1.5 py-0.2 rounded text-[10px] font-semibold', item.allowFallback ? 'bg-emerald-500/10 text-emerald-500 border border-emerald-500/20' : 'bg-gray-500/10 text-gray-400 border border-gray-500/20']">
                      {{ item.allowFallback ? 'Fallback Allowed' : 'Fallback Disabled' }}
                    </span>
                  </template>
                  <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
                  <span class="text-gray-500 dark:text-gray-400 flex items-center gap-0.5">
                    <span class="material-symbols-outlined text-[13px]">schedule</span>
                    Last attempt: {{ item.failedAt }}
                  </span>
                </div>

                <!-- Error Message Banner -->
                <div class="error-message flex items-center gap-1.5 text-error text-xs mb-1.5">
                  <span class="material-symbols-outlined text-[14px]">error</span>
                  <span class="font-medium truncate">{{ item.errorMessage }}</span>
                </div>
                
                <!-- Expandable Error Details -->
                <button 
                  @click="item.showDetails = !item.showDetails"
                  class="flex items-center gap-1 text-[11px] text-text-secondary hover:text-gray-700 dark:hover:text-gray-300 transition-colors"
                >
                  <span class="material-symbols-outlined text-[14px]">{{ item.showDetails ? 'expand_less' : 'expand_more' }}</span>
                  {{ item.showDetails ? 'Hide details' : 'Show details' }}
                </button>
                
                <Transition name="expand">
                  <div v-if="item.showDetails" class="mt-2 p-2.5 rounded-lg bg-gray-100 dark:bg-gray-800/50 border border-gray-200 dark:border-border-dark space-y-1">
                    <p class="text-[11px] text-gray-700 dark:text-gray-300">{{ item.failure.description }}</p>
                    <p class="text-[11px] font-mono text-text-secondary break-all">{{ item.errorDetails }}</p>
                    <p class="text-[10px] text-text-secondary">Failed at {{ item.failedAt }}</p>
                  </div>
                </Transition>
              </div>
              
              <!-- Action Buttons (Differentiated by Error Cause) -->
              <div class="flex flex-col gap-1.5 shrink-0 items-end">
                <!-- 1. Network retry exhausted -> "Retry original source" -->
                <button 
                  v-if="item.failure.reason === 'network'"
                  @click="retryItem(item.id)" 
                  class="flex items-center justify-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-xs font-semibold transition-colors shadow-xs"
                  title="Retry download from original source"
                >
                  <span class="material-symbols-outlined text-[14px]">refresh</span>
                  <span>Retry original source</span>
                </button>

                <!-- 2. Requires authentication -> "Check Account" (NO fallback button) -->
                <button 
                  v-else-if="item.failure.reason === 'requires_auth'"
                  @click="router.push('/accounts')" 
                  class="flex items-center justify-center gap-1 px-3 py-1.5 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-semibold transition-colors shadow-xs"
                  title="Re-authenticate or update account credentials"
                >
                  <span class="material-symbols-outlined text-[14px]">manage_accounts</span>
                  <span>Check Account</span>
                </button>

                <!-- 3. Stale source -> Retry with Fallback (if allowed) or Retry Original -->
                <template v-else-if="item.failure.reason === 'stale_source'">
                  <button 
                    v-if="item.allowFallback"
                    @click="retryItem(item.id)" 
                    class="flex items-center justify-center gap-1 px-3 py-1.5 bg-orange-600 hover:bg-orange-700 text-white rounded-lg text-xs font-semibold transition-colors shadow-xs"
                    title="Retry search across alternative streaming services"
                  >
                    <span class="material-symbols-outlined text-[14px]">alt_route</span>
                    <span>Retry with Fallback</span>
                  </button>
                  <button 
                    v-else
                    @click="retryItem(item.id)" 
                    class="flex items-center justify-center gap-1 px-3 py-1.5 bg-gray-600 hover:bg-gray-700 text-white rounded-lg text-xs font-semibold transition-colors shadow-xs"
                    title="Retry original provider source"
                  >
                    <span class="material-symbols-outlined text-[14px]">refresh</span>
                    <span>Retry Original</span>
                  </button>
                </template>

                <!-- 4. Rejected quality -> Retry Quality -->
                <button 
                  v-else-if="item.failure.reason === 'rejected_quality'"
                  @click="retryItem(item.id)" 
                  class="flex items-center justify-center gap-1 px-3 py-1.5 bg-purple-600 hover:bg-purple-700 text-white rounded-lg text-xs font-semibold transition-colors shadow-xs"
                  title="Retry with quality tolerance"
                >
                  <span class="material-symbols-outlined text-[14px]">high_quality</span>
                  <span>Retry Quality</span>
                </button>

                <!-- 5. Default Retry -->
                <button 
                  v-else
                  @click="retryItem(item.id)" 
                  class="flex items-center justify-center gap-1 px-3 py-1.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-xs font-semibold transition-colors shadow-xs"
                  title="Retry download"
                >
                  <span class="material-symbols-outlined text-[14px]">refresh</span>
                  <span>Retry</span>
                </button>

                <button @click="removeQueueItem(item.id)" class="p-1 text-gray-400 hover:text-error self-center rounded-lg transition-colors" title="Remove">
                  <span class="material-symbols-outlined text-[16px]">close</span>
                </button>
              </div>
            </div>
          </div>

          <!-- Show more failed if large list -->
          <button 
            v-if="failedLimit < filteredFailedItems.length" 
            @click="failedLimit += 50"
            class="w-full py-2.5 text-xs font-semibold text-primary hover:text-primary-hover hover:bg-primary/5 transition-colors border border-gray-200 dark:border-border-dark rounded-xl flex items-center justify-center gap-1"
          >
            <span>Show More Failed ({{ (filteredFailedItems.length - failedLimit).toLocaleString() }} remaining)</span>
            <span class="material-symbols-outlined text-[16px]">expand_more</span>
          </button>
        </div>
      </div>

    </div>

    <!-- Statistics Card (collapsible footer summary) -->
    <div v-if="showStats" class="stats-card mx-8 mb-4 shrink-0">
      <div class="flex items-center justify-between p-3.5 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-sm">
        <div class="flex items-center gap-6">
          <div class="text-center">
            <p class="text-lg font-bold text-gray-900 dark:text-white">{{ queueStats?.total || rawQueueItems.length }}</p>
            <p class="text-[10px] text-text-secondary">Total Queue</p>
          </div>
          <div class="w-px h-8 bg-gray-200 dark:bg-border-dark"></div>
          <div class="text-center">
            <p class="text-lg font-bold text-primary">{{ auditReport?.source_locked_count || queueStats?.queued || 0 }}</p>
            <p class="text-[10px] text-text-secondary">Source Locked</p>
          </div>
          <div class="w-px h-8 bg-gray-200 dark:bg-border-dark"></div>
          <div class="text-center">
            <p class="text-lg font-bold text-amber-500">{{ auditReport?.stale_source_count || 0 }}</p>
            <p class="text-[10px] text-text-secondary">Stale / 404</p>
          </div>
          <div class="w-px h-8 bg-gray-200 dark:bg-border-dark"></div>
          <div class="text-center">
            <p class="text-lg font-bold text-success">{{ queueStats?.total ? ((queueStats.completed / queueStats.total) * 100).toFixed(1) : '100.0' }}%</p>
            <p class="text-[10px] text-text-secondary">Success Rate</p>
          </div>
        </div>

        <button @click="showStats = false" class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors">
          <span class="material-symbols-outlined text-[18px]">close</span>
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
                <select 
                  :value="currentConcurrency" 
                  @change="setConcurrency(parseInt(($event.target as HTMLSelectElement).value))"
                  class="px-3 py-2 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary"
                >
                  <option value="1">1 Thread</option>
                  <option value="2">2 Threads</option>
                  <option value="3">3 Threads</option>
                  <option value="4">4 Threads</option>
                  <option value="5">5 Threads</option>
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
          
          <!-- Storage Section -->
          <div class="panel-section mb-8">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-sm font-semibold text-text-secondary uppercase tracking-wider">Storage</h3>
              <span :class="['text-xs flex items-center gap-1 font-medium', pathStatusBadgeClass]">
                <span class="material-symbols-outlined text-[14px]">{{ pathStatusIcon }}</span>
                {{ pathStatusLabel }}
              </span>
            </div>
            
            <div class="space-y-4">
              <!-- Download Location -->
              <div>
                <label class="text-sm text-gray-700 dark:text-gray-300 mb-2 block">Download location</label>
                <div class="flex items-center gap-2">
                  <input 
                    type="text"
                    :value="downloadSettings.downloadDto.library_root"
                    @input="handleInputDownloadPath(($event.target as HTMLInputElement).value)"
                    placeholder="Select download directory..."
                    class="flex-1 px-3 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white font-mono placeholder-gray-400 focus:ring-2 focus:ring-primary focus:border-primary outline-none transition-all"
                  />
                  <button 
                    type="button"
                    @click="browseDownloadFolder"
                    class="px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors flex items-center gap-1.5 shrink-0"
                  >
                    <span class="material-symbols-outlined text-[18px]">folder_open</span>
                    <span>Browse...</span>
                  </button>
                </div>
                <div class="mt-2 text-xs text-text-secondary font-mono flex items-center justify-between">
                  <span>Staging: {{ downloadSettings.downloadDto.staging_root || '.staging' }}</span>
                  <span v-if="downloadSettings.downloadDto.free_space_bytes">Free: {{ formattedFreeSpace }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
        
        <!-- Panel Footer -->
        <div class="flex items-center justify-end gap-3 px-6 py-4 border-t border-gray-200 dark:border-border-dark shrink-0">
          <button @click="showSettingsPanel = false" :disabled="isProcessing" class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors disabled:opacity-50">
            Close
          </button>
          <button @click="saveSettings" :disabled="isProcessing" class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
            <span v-if="isProcessing">Saving...</span>
            <span v-else>Save Preferences</span>
          </button>
        </div>
      </div>
    </Transition>
    </Teleport>

    <!-- Download Favorites Modal -->
    <DownloadFavoritesModal 
      v-model="showDownloadFavoritesModal" 
      @enqueued="handleFavoritesEnqueued" 
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useToast } from '@/composables/useToast'
import { confirm } from '@tauri-apps/plugin-dialog'
import { queueApi, classifyFailureReason, type FailureInfo, type FailureReason } from '@/api/queue'
import { invokeCommand } from '@/api/tauri'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { settingsApi } from '@/api/settings'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import type { QueueItem, QueueStats, WorkerStatus, ProgressEvent } from '@/api/types'
import DownloadFavoritesModal from '@/components/DownloadFavoritesModal.vue'
import { auditDownloadQueue, type DownloadFavoritesResult, type QueueAuditReport } from '@/api/library'
import { formatServiceName } from '@/composables/useGlobalTasks'

const router = useRouter()

// Constants for high performance rendering & IPC throttling
const ROW_HEIGHT = 60 // px per virtual queue item
const OVERSCAN = 10   // extra rows above and below visible area
const PROGRESS_THROTTLE_MS = 250 // Max 4 IPC updates/sec per track

function formatBytes(bytes: number | undefined | null): string {
  if (!bytes || bytes <= 0) return '0 B'
  const kb = bytes / 1024
  if (kb < 1024) return `${kb.toFixed(1)} KB`
  const mb = kb / 1024
  if (mb < 1024) return `${mb.toFixed(1)} MB`
  const gb = mb / 1024
  return `${gb.toFixed(2)} GB`
}

function formatSpeed(kbps: number | undefined | null): string {
  if (!kbps || kbps <= 0) return '0 KB/s'
  if (kbps >= 1024) {
    return `${(kbps / 1024).toFixed(1)} MB/s`
  }
  return `${Math.round(kbps)} KB/s`
}

// Event bus for real-time updates
const { on } = useEventBus()
const toast = useToast()

const showDownloadFavoritesModal = ref(false)
const auditReport = ref<QueueAuditReport | null>(null)

async function handleFavoritesEnqueued(res: DownloadFavoritesResult) {
  toast.success('Favorites Enqueued', res.message)
  await fetchData()
}

// Toolbar state
const viewFilter = ref<'all' | 'active' | 'queued' | 'completed' | 'failed'>('all')
const searchQuery = ref('')
const loading = ref(true)
const isProcessing = ref(false)

// Section visibility
const showCompleted = ref(true)
const showStats = ref(true)
const showQueueDetails = ref(false)
const showSecondaryMenu = ref(false)
const completedLimit = ref(50)
const failedLimit = ref(50)

// Settings panel state
const showSettingsPanel = ref(false)

// Backend data
const queueStats = ref<QueueStats>({ total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 })
const workerStatus = ref<WorkerStatus>({ running: true, paused: false, active_downloads: 0, max_concurrent: 3 })
const rawQueueItems = ref<QueueItem[]>([])

// Concurrency state (1 to 5 threads, reactive & persisted in AppState)
const currentConcurrency = computed(() => {
  return workerStatus.value?.max_concurrent ?? 3
})

async function setConcurrency(threads: number) {
  const clamped = Math.max(1, Math.min(5, threads))
  try {
    await queueApi.setMaxConcurrent(clamped)
    if (workerStatus.value) {
      workerStatus.value.max_concurrent = clamped
    }
    settings.concurrentDownloads = String(clamped)
    toast.success('Concurrency Updated', `Worker set to ${clamped} concurrent thread${clamped > 1 ? 's' : ''}`)
  } catch (err) {
    console.error('Failed to set concurrency:', err)
    toast.error('Error', 'Failed to update concurrent worker threads')
  }
}

// Worker paused state
const isPaused = computed(() => workerStatus.value?.paused ?? workerStatus.value?.is_paused ?? false)

// Timestamp map for progress event throttling
const lastProgressTimestamps = new Map<number | string, number>()

// Helper for search matching
function matchesSearch(item: any): boolean {
  if (!searchQuery.value || !searchQuery.value.trim()) return true
  const query = searchQuery.value.toLowerCase().trim()
  const title = (item.title || item.target_title || '').toLowerCase()
  const artist = (item.artist || item.target_artist || '').toLowerCase()
  const album = (item.album || item.target_album || '').toLowerCase()
  const service = (item.service || item.service_name || '').toLowerCase()
  const errorMsg = (item.errorMessage || item.error_message || '').toLowerCase()
  return title.includes(query) || artist.includes(query) || album.includes(query) || service.includes(query) || errorMsg.includes(query)
}

// Computed: Active downloads
const activeDownloads = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'downloading')
    .map(item => {
      const sName = item.service_name || item.service || 'Unknown'
      const rawQuality = item.quality_preference || item.quality || 'FLAC'
      const hasTotal = typeof (item as any).total_bytes === 'number' && (item as any).total_bytes > 0
      const percentVal = (item as any).percent !== undefined ? (item as any).percent : (hasTotal ? item.progress_percent : null)
      return {
        id: item.id,
        trackId: item.track_id,
        title: item.target_title || item.title || 'Unknown Track',
        artist: item.target_artist || item.artist || 'Unknown Artist',
        album: item.target_album || 'Album',
        artGradient: getArtGradient(item.id),
        service: sName,
        serviceBadgeClass: getServiceBadgeClass(sName),
        quality: rawQuality.startsWith('Declared') ? rawQuality : `Declared ${rawQuality}`,
        qualityBadgeClass: 'bg-primary/10 text-primary border border-primary/20',
        progress: item.progress_percent || 0,
        bytesDownloaded: (item as any).bytes_downloaded ?? 0,
        totalBytes: (item as any).total_bytes ?? null,
        percent: percentVal,
        instantKbps: (item as any).instant_kbps || 0,
        averageKbps: (item as any).average_kbps || 0,
        phase: (item as any).phase || 'downloading',
        status: item.status,
      }
    })
})

const filteredActiveDownloads = computed(() => {
  return activeDownloads.value.filter(matchesSearch)
})

// Computed: Queue items (Up Next)
const queueItems = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'queued')
    .map(item => {
      const sName = item.service_name || item.service || 'Unknown'
      const rawQuality = item.quality_preference || item.quality || 'FLAC'
      return {
        id: item.id,
        trackId: item.track_id,
        title: item.target_title || item.title || 'Unknown Track',
        artist: item.target_artist || item.artist || 'Unknown Artist',
        album: item.target_album || 'Album',
        artGradient: getArtGradient(item.id),
        service: sName,
        serviceBadgeClass: getServiceBadgeClass(sName),
        quality: rawQuality.startsWith('Declared') ? rawQuality : `Declared ${rawQuality}`,
        qualityBadgeClass: 'bg-gray-100 dark:bg-surface-highlight text-text-secondary border border-gray-200 dark:border-border-dark',
        progress: item.progress_percent || 0,
        status: item.status,
      }
    })
})

const filteredQueueItems = computed(() => {
  return queueItems.value.filter(matchesSearch)
})

// Computed: Completed items
const completedItems = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'complete' || item.status === 'completed')
    .map(item => {
      const sName = item.service_name || item.service || 'Unknown'
      const rawQuality = item.quality_preference || item.quality || 'FLAC'
      return {
        id: item.id,
        trackId: item.track_id,
        title: item.target_title || item.title || 'Unknown Track',
        artist: item.target_artist || item.artist || 'Unknown Artist',
        album: item.target_album || 'Album',
        artGradient: getArtGradient(item.id),
        service: sName,
        serviceBadgeClass: getServiceBadgeClass(sName),
        quality: rawQuality.startsWith('Downloaded') ? rawQuality : `Downloaded ${rawQuality}`,
        qualityBadgeClass: 'bg-emerald-500/10 text-emerald-500 border border-emerald-500/20',
        completedAt: formatTime(item.completed_at ?? undefined),
      }
    })
})

const filteredCompletedItems = computed(() => {
  return completedItems.value.filter(matchesSearch)
})

const visibleCompletedSlice = computed(() => {
  return filteredCompletedItems.value.slice(0, completedLimit.value)
})

// Computed: Failed items
const failedItems = computed(() => {
  return rawQueueItems.value
    .filter(item => item.status === 'failed')
    .map(item => {
      const originalService = item.service_name || item.service || 'Unknown'
      const effectiveService = item.effective_service || item.service_name || item.service || null
      const rawQuality = item.quality_preference || item.quality || 'FLAC'
      const retryCount = item.retry_count ?? 0
      const failure = classifyFailureReason(item.error_message, item.last_error)

      return {
        id: item.id,
        trackId: item.track_id,
        title: item.target_title || item.title || 'Unknown Track',
        artist: item.target_artist || item.artist || 'Unknown Artist',
        album: item.target_album || 'Album',
        artGradient: getArtGradient(item.id),
        service: originalService,
        originalService,
        effectiveService,
        serviceBadgeClass: getServiceBadgeClass(originalService),
        quality: rawQuality.startsWith('Declared') ? rawQuality : `Declared ${rawQuality}`,
        qualityBadgeClass: 'bg-red-500/10 text-red-500 border border-red-500/20',
        errorMessage: item.error_message || 'Download failed',
        errorDetails: item.last_error || item.error_message || 'Unknown error',
        failure,
        retryCount,
        allowFallback: item.allow_fallback ?? true,
        failedAt: formatTime(item.completed_at ?? item.started_at ?? item.created_at ?? undefined),
        showDetails: false,
      }
    })
})

const filteredFailedItems = computed(() => {
  return failedItems.value.filter(matchesSearch)
})

const visibleFailedSlice = computed(() => {
  return filteredFailedItems.value.slice(0, failedLimit.value)
})

// Summary counts
const totalItemCount = computed(() => rawQueueItems.value.length)
const matchingCount = computed(() => {
  return filteredActiveDownloads.value.length + filteredQueueItems.value.length + filteredCompletedItems.value.length + filteredFailedItems.value.length
})

const filterTabs = computed(() => [
  { value: 'all' as const, label: 'All', icon: 'list', count: rawQueueItems.value.length },
  { value: 'active' as const, label: 'Active', icon: 'sync', count: activeDownloads.value.length },
  { value: 'queued' as const, label: 'Queued', icon: 'schedule', count: queueItems.value.length },
  { value: 'completed' as const, label: 'Completed', icon: 'check_circle', count: completedItems.value.length },
  { value: 'failed' as const, label: 'Failed', icon: 'error', count: failedItems.value.length },
])

// ==============================================
// LIVE TELEMETRY & ARTIFACT METRICS
// ==============================================
const throughputKbps = ref<number>(0)
const artifactCounters = ref<{ audio: number; lrc: number; covers: number; booklets: number }>({
  audio: 0,
  lrc: 0,
  covers: 0,
  booklets: 0,
})
const progressSamples: { time: number; bytes: number }[] = []
const prevItemProgress = new Map<number | string, number>()

const successRate = computed<number>(() => {
  if (queueStats.value && typeof (queueStats.value as any).success_rate === 'number') {
    return Math.round((queueStats.value as any).success_rate * 10) / 10
  }
  const finished = completedItems.value.length + failedItems.value.length
  if (finished === 0) return 100.0
  return Math.round((completedItems.value.length / finished) * 1000) / 10
})

const formattedThroughput = computed<string>(() => {
  const kbps = throughputKbps.value
  if (kbps <= 0 || activeDownloads.value.length === 0) return '0 KB/s'
  if (kbps >= 1024) {
    return `${(kbps / 1024).toFixed(1)} MB/s`
  }
  return `${Math.round(kbps)} KB/s`
})

const etaSeconds = computed<number | null>(() => {
  const activeCount = activeDownloads.value.length
  const queuedCount = queueItems.value.length
  if (activeCount === 0 && queuedCount === 0) return 0
  if (isPaused.value) return null

  const avgTrackBytes = 25 * 1024 * 1024 // ~25MB FLAC
  const remainingActivePercent = activeDownloads.value.reduce((acc, item) => acc + (100 - (item.progress || 0)), 0)
  const totalRemainingBytes = (queuedCount * avgTrackBytes) + ((remainingActivePercent / 100) * avgTrackBytes)

  const currentSpeedBytesPerSec = throughputKbps.value > 0 
    ? throughputKbps.value * 1024 
    : (activeCount > 0 ? 1.5 * 1024 * 1024 : 0)

  if (currentSpeedBytesPerSec <= 0) return null

  const est = Math.ceil(totalRemainingBytes / currentSpeedBytesPerSec)
  return Math.max(1, est)
})

const formattedEta = computed<string>(() => {
  const s = etaSeconds.value
  if (s === 0) return 'Completed'
  if (s === null) return activeDownloads.value.length > 0 ? 'Calculating...' : '--'
  if (s < 60) return `${s}s`
  const mins = Math.floor(s / 60)
  const secs = s % 60
  if (mins < 60) {
    return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`
  }
  const hours = Math.floor(mins / 60)
  const remMins = mins % 60
  return `${hours}h ${remMins}m`
})

const overallProgress = computed(() => {
  if (activeDownloads.value.length === 0) return 0
  const total = activeDownloads.value.reduce((acc, item) => acc + item.progress, 0)
  return total / activeDownloads.value.length
})

// ==============================================
// VIRTUAL SCROLLING LOGIC FOR UP NEXT (10,000+ ROWS)
// ==============================================
const queueContainerRef = ref<HTMLElement | null>(null)
const queueScrollTop = ref(0)
const queueViewportHeight = ref(520)
const jumpPositionInput = ref<number | undefined>(undefined)

const totalVirtualHeight = computed(() => {
  return filteredQueueItems.value.length * ROW_HEIGHT
})

const virtualStartIndex = computed(() => {
  return Math.max(0, Math.floor(queueScrollTop.value / ROW_HEIGHT) - OVERSCAN)
})

const virtualEndIndex = computed(() => {
  return Math.min(
    filteredQueueItems.value.length,
    Math.ceil((queueScrollTop.value + queueViewportHeight.value) / ROW_HEIGHT) + OVERSCAN
  )
})

const virtualOffsetY = computed(() => {
  return virtualStartIndex.value * ROW_HEIGHT
})

const visibleQueueItems = computed(() => {
  return filteredQueueItems.value
    .slice(virtualStartIndex.value, virtualEndIndex.value)
    .map((item, idx) => ({
      ...item,
      absoluteIndex: virtualStartIndex.value + idx,
    }))
})

function onQueueScroll(e: Event) {
  const target = e.target as HTMLElement
  if (target) {
    queueScrollTop.value = target.scrollTop
    queueViewportHeight.value = target.clientHeight || 520
  }
}

function jumpToPosition() {
  if (!jumpPositionInput.value || !queueContainerRef.value) return
  const targetIdx = Math.max(1, Math.min(filteredQueueItems.value.length, jumpPositionInput.value))
  const targetScrollTop = (targetIdx - 1) * ROW_HEIGHT
  queueContainerRef.value.scrollTop = targetScrollTop
  queueScrollTop.value = targetScrollTop
}

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

// Drag-and-Drop Reorder
const draggedIndex = ref<number | null>(null)

function onDragStart(absoluteIndex: number) {
  draggedIndex.value = absoluteIndex
}

function onDragOver(e: DragEvent) {
  e.preventDefault()
}

async function onDrop(targetAbsoluteIndex: number) {
  if (draggedIndex.value === null || draggedIndex.value === targetAbsoluteIndex) return

  const currentQueued = [...queueItems.value]
  const [dragged] = currentQueued.splice(draggedIndex.value, 1)
  currentQueued.splice(targetAbsoluteIndex, 0, dragged)

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
    const [queue, stats, worker, audit] = await Promise.all([
      queueApi.getQueue(undefined, 50000),
      queueApi.getQueueStats(),
      queueApi.getWorkerStatus(),
      auditDownloadQueue().catch(() => null),
    ])
    
    rawQueueItems.value = queue
    queueStats.value = stats
    workerStatus.value = worker
    if (stats) {
      const s = stats as any
      if (typeof s.audio_count === 'number') artifactCounters.value.audio = s.audio_count
      else if (typeof s.completed === 'number') artifactCounters.value.audio = s.completed
      if (typeof s.lrc_count === 'number') artifactCounters.value.lrc = s.lrc_count
      else if (typeof s.completed === 'number') artifactCounters.value.lrc = s.completed
      if (typeof s.cover_count === 'number') artifactCounters.value.covers = s.cover_count
      else if (typeof s.completed === 'number') artifactCounters.value.covers = s.completed
      if (typeof s.booklet_count === 'number') artifactCounters.value.booklets = s.booklet_count
    }
    if (audit) {
      auditReport.value = audit
    }
  } catch (e) {
    console.error('Failed to fetch queue data:', e)
  } finally {
    loading.value = false
  }
}

/**
 * Handle progress event with max 4 updates/sec per track throttling and live telemetry calculation
 */
function handleProgressEvent(event: any) {
  if (!event) return
  
  const queueId = event.queue_id ? parseInt(String(event.queue_id), 10) : (event.id ? parseInt(String(event.id), 10) : (event.item_id ? parseInt(String(event.item_id), 10) : undefined))
  if (queueId === undefined || isNaN(queueId)) return

  const hasTotalBytes = typeof event.total_bytes === 'number' && event.total_bytes > 0
  const totalBytes = hasTotalBytes ? event.total_bytes : (event.total_bytes === null ? null : undefined)
  const rawPercent = typeof event.percent === 'number' ? event.percent : (typeof event.progress_percent === 'number' ? event.progress_percent : (typeof event.percentage === 'number' ? event.percentage : undefined))
  const percent = event.total_bytes === null && (event.percent === null || event.percent === undefined) ? null : (typeof rawPercent === 'number' ? rawPercent : null)

  const status = event.status || (event.phase && ['complete', 'failed', 'cancelled', 'downloading', 'started'].includes(event.phase) ? event.phase : 'downloading')
  const isTerminal = event.terminal === true || status === 'completed' || status === 'complete' || status === 'failed' || status === 'cancelled' || status === 'stale_source' || status === 'error' || status === 'rejected_quality' || (percent !== null && percent >= 100)
  const isInitial = status === 'started' || (percent !== null && percent === 0)

  const now = Date.now()
  const lastTime = lastProgressTimestamps.get(queueId) || 0

  // Calculate delta progress for throughput calculation
  const prevPerc = prevItemProgress.get(queueId) ?? 0
  const currentPerc = percent ?? 0
  const deltaPerc = Math.max(0, currentPerc - prevPerc)
  prevItemProgress.set(queueId, currentPerc)

  const estTrackBytes = 25 * 1024 * 1024
  const deltaBytes = typeof event.bytes_downloaded === 'number'
    ? Math.max(0, event.bytes_downloaded - (event.prev_bytes || 0))
    : (deltaPerc / 100) * estTrackBytes

  if (deltaBytes > 0) {
    progressSamples.push({ time: now, bytes: deltaBytes })
  }

  // Prune samples older than 3.5 seconds
  const cutoff = now - 3500
  while (progressSamples.length > 0 && progressSamples[0].time < cutoff) {
    progressSamples.shift()
  }

  // Calculate instant throughput in KB/s
  if (typeof event.instant_kbps === 'number' && event.instant_kbps > 0) {
    throughputKbps.value = Math.round(event.instant_kbps)
  } else if (progressSamples.length > 1) {
    const durationSec = Math.max(0.5, (now - progressSamples[0].time) / 1000)
    const totalBytesInWindow = progressSamples.reduce((sum, s) => sum + s.bytes, 0)
    const instantKbps = (totalBytesInWindow / durationSec) / 1024
    throughputKbps.value = Math.round(
      throughputKbps.value === 0 ? instantKbps : (throughputKbps.value * 0.65 + instantKbps * 0.35)
    )
  } else if (activeDownloads.value.length === 0) {
    throughputKbps.value = 0
  }

  // Apply throttle for intermediate progress events (max 4 per sec = 250ms)
  if (!isTerminal && !isInitial && now - lastTime < PROGRESS_THROTTLE_MS) {
    return
  }

  lastProgressTimestamps.set(queueId, now)
  if (isTerminal) {
    lastProgressTimestamps.delete(queueId)
    prevItemProgress.delete(queueId)
  }

  const item = rawQueueItems.value.find(q => q.id === queueId)
  if (item) {
    if (percent !== null) {
      item.progress_percent = percent
      ;(item as any).percent = percent
    } else {
      ;(item as any).percent = null
    }

    if (typeof event.bytes_downloaded === 'number') {
      ;(item as any).bytes_downloaded = event.bytes_downloaded
    }
    if (totalBytes !== undefined) {
      ;(item as any).total_bytes = totalBytes
    }
    if (typeof event.instant_kbps === 'number') {
      ;(item as any).instant_kbps = event.instant_kbps
    }
    if (typeof event.average_kbps === 'number') {
      ;(item as any).average_kbps = event.average_kbps
    }
    if (event.phase) {
      ;(item as any).phase = event.phase
    }

    if (status === 'completed' || status === 'complete') {
      item.status = 'complete'
      item.progress_percent = 100
      ;(item as any).percent = 100
      item.completed_at = new Date().toISOString()
      artifactCounters.value.audio += 1
      artifactCounters.value.lrc += 1
      artifactCounters.value.covers += 1
      if (item.target_album && item.target_album.includes('Edition')) {
        artifactCounters.value.booklets += 1
      }
    } else if (status === 'failed' || status === 'cancelled' || status === 'stale_source' || status === 'error' || status === 'rejected_quality') {
      item.status = 'failed'
      item.error_message = event.message || event.error || (status === 'cancelled' ? 'Download cancelled by user' : 'Download failed')
    } else if (status === 'started' || status === 'downloading') {
      item.status = 'downloading'
    }
  }

  if (activeDownloads.value.length === 0) {
    throughputKbps.value = 0
  }
}

// Actions
async function pauseAll() {
  isProcessing.value = true
  try {
    await queueApi.pauseWorker()
    if (workerStatus.value) {
      workerStatus.value.paused = true
    }
    toast.info('Paused', 'Download queue paused')
  } catch (e) {
    toast.error('Failed to pause', String(e))
  } finally {
    isProcessing.value = false
  }
}

async function resumeAll() {
  isProcessing.value = true
  try {
    await queueApi.resumeWorker()
    if (workerStatus.value) {
      workerStatus.value.paused = false
    }
    toast.success('Resumed', 'Download queue resumed')
  } catch (e) {
    toast.error('Failed to resume', String(e))
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
    toast.success('Completed downloads cleared')
  } finally {
    isProcessing.value = false
  }
}

async function clearPendingQueue() {
  const count = queueItems.value.length
  if (count === 0) return
  const confirmed = await confirm(`Are you sure you want to clear all ${count.toLocaleString()} pending downloads?`, {
    title: 'Clear Pending Downloads',
    kind: 'warning'
  })
  if (confirmed !== true) return
  
  isProcessing.value = true
  try {
    await queueApi.clearQueue('queued')
    await fetchData()
    toast.success(`Cleared ${count.toLocaleString()} queued items`)
  } finally {
    isProcessing.value = false
  }
}

async function removeQueueItem(id: number) {
  const confirmed = await confirm('Remove this track from the queue?', {
    title: 'Remove Track',
    kind: 'warning'
  })
  if (confirmed !== true) return
  
  isProcessing.value = true
  try {
    await queueApi.removeFromQueue(id)
    rawQueueItems.value = rawQueueItems.value.filter(q => q.id !== id)
    lastProgressTimestamps.delete(id)
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
    lastProgressTimestamps.clear()
    await fetchData()
    toast.success('Retrying failed downloads')
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
    lastProgressTimestamps.clear()
    await fetchData()
    toast.success('Cleared failed downloads')
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
    lastProgressTimestamps.delete(id)
    await fetchData()
  } finally {
    isProcessing.value = false
  }
}

async function retryItem(id: number) {
  await queueApi.retryItem(id)
  lastProgressTimestamps.delete(id)
  await fetchData()
}

// Settings
const downloadSettings = useDownloadSettings()
const folderSettings = downloadSettings.folderSettings
const settings = downloadSettings.generalSettings
const saveFolderSettings = downloadSettings.saveFolderSettings
const saveGeneralSettings = downloadSettings.saveGeneralSettings

const pathStatusIcon = computed(() => {
  switch (downloadSettings.downloadDto.path_status) {
    case 'valid': return 'check_circle'
    case 'missing': return 'folder_off'
    case 'not_writable': return 'lock'
    case 'unavailable': return 'disc_full'
    default: return 'help'
  }
})

const pathStatusLabel = computed(() => {
  switch (downloadSettings.downloadDto.path_status) {
    case 'valid': return 'Valid'
    case 'missing': return 'Missing'
    case 'not_writable': return 'Read-Only'
    case 'unavailable': return 'Unavailable'
    default: return 'Unknown'
  }
})

const pathStatusBadgeClass = computed(() => {
  switch (downloadSettings.downloadDto.path_status) {
    case 'valid': return 'text-emerald-500 dark:text-emerald-400'
    case 'missing': return 'text-amber-500 dark:text-amber-400'
    case 'not_writable': return 'text-red-500 dark:text-red-400'
    case 'unavailable': return 'text-red-500 dark:text-red-400'
    default: return 'text-text-secondary'
  }
})

const formattedFreeSpace = computed(() => {
  const bytes = downloadSettings.downloadDto.free_space_bytes
  if (!bytes || bytes <= 0) return null
  const gb = bytes / (1024 * 1024 * 1024)
  if (gb >= 1000) {
    return `${(gb / 1024).toFixed(1)} TB`
  }
  return `${gb.toFixed(1)} GB`
})

function handleInputDownloadPath(val: string) {
  downloadSettings.downloadDto.library_root = val
}

const loadDownloadSettings = async () => {
  try {
    await downloadSettings.loadSettings()
  } catch (e) {
    console.warn('Failed to load download settings:', e)
  }
}

async function browseDownloadFolder() {
  const chosen = await downloadSettings.browseDownloadDirectory()
  if (chosen) {
    downloadSettings.downloadDto.library_root = chosen
  }
}

async function saveSettings() {
  isProcessing.value = true
  try {
    folderSettings.base_folder = downloadSettings.downloadDto.library_root
    await saveFolderSettings()
    await saveGeneralSettings()
    showSettingsPanel.value = false
    toast.success('Settings saved', 'Download preferences updated')
  } catch (e) {
    toast.error('Failed to save settings', String(e))
  } finally {
    isProcessing.value = false
  }
}

let unlistenProgress: (() => void) | null = null

// Initialize
onMounted(async () => {
  await loadDownloadSettings()
  await fetchData()
  unlistenProgress = await on<ProgressEvent>(TauriEvents.DOWNLOAD_PROGRESS, handleProgressEvent)
})

onUnmounted(() => {
  if (unlistenProgress) {
    unlistenProgress()
    unlistenProgress = null
  }
})
</script>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
  height: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(128, 128, 128, 0.25);
  border-radius: 4px;
}

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(128, 128, 128, 0.45);
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
</style>
