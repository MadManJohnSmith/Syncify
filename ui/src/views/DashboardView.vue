<template>
  <div class="stats-dashboard h-full overflow-y-auto custom-scrollbar bg-background-dark">
    <!-- Header -->
    <div class="sticky top-0 bg-background-dark z-10 px-6 py-4 border-b border-border-dark flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold text-white">Dashboard</h1>
        <p class="text-sm text-gray-500">Library statistics and analytics</p>
      </div>
      <div class="flex items-center gap-4">
        <span class="text-xs text-gray-400">Updated {{ lastUpdated }}</span>
        <button @click="refresh" class="px-4 py-2 bg-surface-dark border border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg text-sm flex items-center gap-2 hover:bg-gray-50 dark:hover:bg-surface-highlight">
          <span class="material-symbols-outlined text-lg" :class="{ 'animate-spin': isRefreshing }">refresh</span>
          Refresh
        </button>
        <button @click="exportReport" class="px-4 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm flex items-center gap-2">
          <span class="material-symbols-outlined text-lg">download</span>
          Export Report
        </button>
      </div>
    </div>
    
    <!-- Loading state -->
    <div v-if="loading" class="flex-1 flex flex-col items-center justify-center py-20">
      <div class="w-12 h-12 border-4 border-primary border-t-transparent rounded-full animate-spin mb-4"></div>
      <p class="text-gray-400">Loading dashboard data...</p>
    </div>

    <!-- Error state -->
    <div v-else-if="error" class="flex-1 flex flex-col items-center justify-center py-20 px-6 text-center">
      <span class="material-symbols-outlined text-5xl text-red-500 mb-4">error</span>
      <h3 class="text-lg font-semibold text-white mb-2">Failed to load dashboard</h3>
      <p class="text-gray-400 max-w-md mb-6">{{ error }}</p>
      <button @click="fetchData" class="px-6 py-2 bg-surface-highlight hover:bg-surface-highlight/80 text-white rounded-lg transition-colors">
        Try Again
      </button>
    </div>

    <!-- Stats Grid -->
    <div v-else class="p-6">
      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
        
        <!-- Library Overview (Spans 2 columns) -->
        <div class="stat-card library-overview col-span-1 md:col-span-2 bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-6">Library Overview</h3>
          
          <div class="flex flex-col lg:flex-row gap-8">
            <!-- Key Metrics -->
            <div class="flex-1 grid grid-cols-2 lg:grid-cols-4 gap-4">
              <div class="metric-box p-4 bg-surface-highlight rounded-xl text-center">
                <span class="material-symbols-outlined text-3xl text-primary mb-2">music_note</span>
                <p class="text-2xl font-bold text-white">{{ stats.totalTracks.toLocaleString() }}</p>
                <p class="text-sm text-gray-500">Total Tracks</p>
              </div>
              <div class="metric-box p-4 bg-surface-highlight rounded-xl text-center">
                <span class="material-symbols-outlined text-3xl text-purple-500 mb-2">album</span>
                <p class="text-2xl font-bold text-white">{{ stats.totalAlbums }}</p>
                <p class="text-sm text-gray-500">Albums</p>
              </div>
              <div class="metric-box p-4 bg-surface-highlight rounded-xl text-center">
                <span class="material-symbols-outlined text-3xl text-teal-500 mb-2">person</span>
                <p class="text-2xl font-bold text-white">{{ stats.totalArtists }}</p>
                <p class="text-sm text-gray-500">Artists</p>
              </div>
              <div class="metric-box p-4 bg-surface-highlight rounded-xl text-center">
                <span class="material-symbols-outlined text-3xl text-orange-500 mb-2">queue_music</span>
                <p class="text-2xl font-bold text-white">{{ stats.totalPlaylists }}</p>
                <p class="text-sm text-gray-500">Playlists</p>
              </div>
            </div>
            
            <!-- Donut Chart -->
            <div class="w-48 h-48 shrink-0 relative">
              <svg viewBox="0 0 100 100" class="transform -rotate-90">
                <circle cx="50" cy="50" r="40" fill="none" stroke="#e5e7eb" stroke-width="12" class="dark:stroke-gray-700" />
                <circle 
                  cx="50" cy="50" r="40" fill="none" 
                  stroke="#22c55e" stroke-width="12" stroke-linecap="round"
                  :stroke-dasharray="`${stats.downloadedPercent * 2.51} 251`"
                />
              </svg>
              <div class="absolute inset-0 flex flex-col items-center justify-center">
                <p class="text-2xl font-bold text-white">{{ stats.downloadedPercent }}%</p>
                <p class="text-xs text-gray-500">Downloaded</p>
              </div>
            </div>
          </div>
          
          <div class="flex items-center gap-6 mt-4 text-sm">
            <span class="flex items-center gap-2">
              <span class="w-3 h-3 rounded-full bg-green-500"></span>
              Downloaded: {{ stats.downloadedTracks }}
            </span>
            <span class="flex items-center gap-2">
              <span class="w-3 h-3 rounded-full bg-blue-500"></span>
              Streaming: {{ stats.streamingTracks }}
            </span>
          </div>
        </div>
        
        <!-- Storage Usage -->
        <div class="stat-card storage-usage bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Storage Usage</h3>
          <p class="text-3xl font-bold text-white mb-4">{{ stats.storageUsed }}</p>
          
          <!-- Stacked Bar -->
          <div class="h-4 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden flex mb-4">
            <template v-if="storageData">
              <div 
                v-for="(item, i) in storageData.breakdown" 
                :key="item.format"
                :class="[i === 0 ? 'bg-blue-500' : i === 1 ? 'bg-green-500' : 'bg-gray-400', 'h-full']"
                :style="{ width: (item.size_bytes / storageData.used_bytes * 100) + '%' }"
              ></div>
            </template>
            <div v-else class="bg-gray-200 dark:bg-gray-600 h-full w-full"></div>
          </div>
          
          <div class="space-y-2 text-sm">
            <template v-if="storageData">
              <div v-for="(item, i) in storageData.breakdown" :key="item.format" class="flex justify-between">
                <span class="flex items-center gap-2">
                  <span :class="[i === 0 ? 'bg-blue-500' : i === 1 ? 'bg-green-500' : 'bg-gray-400', 'w-2 h-2 rounded']"></span>
                  {{ item.format }}
                </span>
                <span class="text-gray-600 dark:text-gray-400">{{ formatBytes(item.size_bytes) }}</span>
              </div>
            </template>
            <p v-else class="text-gray-500 text-center py-2">No storage data</p>
          </div>
          
          <p class="text-xs text-gray-500 mt-4">{{ stats.storageAvailable }} available · ~{{ stats.perTrackSize }} per track</p>
        </div>
        
        <!-- Quality Distribution -->
        <div class="stat-card quality-distribution bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Audio Quality</h3>
          
          <div v-if="qualityData.length > 0" class="space-y-4">
            <div v-for="(item, i) in qualityData" :key="item.label" @click="handleQualityClick(item.label)" class="cursor-pointer hover:bg-white/5 rounded-lg p-1 -m-1 transition-colors">
              <div class="flex justify-between text-sm mb-1">
                <span class="text-gray-300">{{ item.label }}</span>
                <span class="text-gray-500">{{ item.count }} tracks</span>
              </div>
              <div class="h-2 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden">
                <div 
                  :class="[i === 0 ? 'bg-indigo-500' : i === 1 ? 'bg-blue-400' : 'bg-gray-500', 'h-full rounded-full']" 
                  :style="{ width: (item.count / stats.totalTracks * 100) + '%' }"
                ></div>
              </div>
            </div>
          </div>
          <div v-else class="flex flex-col items-center justify-center py-8 text-gray-500 italic text-sm">
            <span class="material-symbols-outlined text-4xl mb-2">Graphic_Eq</span>
            No downloads yet
          </div>
        </div>
        
        <!-- Service Distribution -->
        <div class="stat-card service-distribution bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Sources</h3>
          
          <div class="space-y-4">
            <div v-for="service in stats.services" :key="service.name">
              <div class="flex justify-between text-sm mb-1">
                <span class="text-gray-700 dark:text-gray-300">{{ service.name }}</span>
                <span class="text-gray-500">{{ service.percent }}%</span>
              </div>
              <div class="h-2 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden">
                <div :class="service.color" class="h-full rounded-full" :style="{ width: service.percent + '%' }"></div>
              </div>
            </div>
          </div>
        </div>
        
        <!-- Download Stats -->
        <div class="stat-card download-stats bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Download Statistics</h3>
          
          <div class="grid grid-cols-2 gap-4 mb-4">
            <div>
              <p class="text-2xl font-bold text-white">{{ queueStats?.total || 0 }}</p>
              <p class="text-sm text-gray-500">Total Downloads</p>
            </div>
            <div>
              <p class="text-2xl font-bold text-green-500">{{ queueStats?.total ? Math.round((queueStats.completed / queueStats.total) * 100) : 100 }}%</p>
              <p class="text-sm text-gray-500">Success Rate</p>
            </div>
          </div>
          
          <div class="space-y-3 text-sm">
            <div class="flex justify-between">
              <span class="text-gray-600 dark:text-gray-400">Failed downloads</span>
              <span class="text-red-500 cursor-pointer hover:underline" @click="goToFailed">{{ queueStats?.failed || 0 }} tracks →</span>
            </div>
            <div class="flex justify-between">
              <span class="text-gray-600 dark:text-gray-400">Average speed</span>
              <span class="text-white">—</span>
            </div>
            <div class="flex justify-between">
              <span class="text-gray-600 dark:text-gray-400">Completed this session</span>
              <span class="text-green-500">+{{ queueStats?.completed || 0 }} tracks</span>
            </div>
          </div>
        </div>
        
        <div class="stat-card metadata-quality bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Metadata Quality</h3>
          
          <div v-if="metadataStats" class="flex items-center gap-6 mb-4">
            <!-- Gauge -->
            <div class="w-24 h-24 relative">
              <svg viewBox="0 0 100 100" class="transform -rotate-90">
                <circle cx="50" cy="50" r="40" fill="none" stroke="#e5e7eb" stroke-width="10" class="dark:stroke-gray-700" />
                <circle cx="50" cy="50" r="40" fill="none" stroke="#6366f1" stroke-width="10" stroke-linecap="round"
                  :stroke-dasharray="`${(metadataStats?.average_completeness || 0) * 2.51} 251`" />
              </svg>
              <div class="absolute inset-0 flex items-center justify-center">
                <p class="text-xl font-bold text-indigo-500">{{ Math.round(metadataStats?.average_completeness || 0) }}%</p>
              </div>
            </div>
            
            <div class="flex-1 space-y-2 text-sm">
              <div class="flex justify-between">
                <span class="text-blue-500">With Art</span>
                <span>{{ metadataStats.total_tracks > 0 ? Math.round((metadataStats.with_art / metadataStats.total_tracks) * 100) : 0 }}%</span>
              </div>
              <div class="flex justify-between">
                <span class="text-teal-500">With Year</span>
                <span>{{ metadataStats.total_tracks > 0 ? Math.round((metadataStats.with_year / metadataStats.total_tracks) * 100) : 0 }}%</span>
              </div>
              <div class="flex justify-between">
                <span class="text-purple-500">With Genre</span>
                <span>{{ metadataStats.total_tracks > 0 ? Math.round((metadataStats.with_genre / metadataStats.total_tracks) * 100) : 0 }}%</span>
              </div>
              <div class="flex justify-between">
                <span class="text-amber-500">With ISRC</span>
                <span>{{ metadataStats.total_tracks > 0 ? Math.round((metadataStats.with_isrc / metadataStats.total_tracks) * 100) : 0 }}%</span>
              </div>
            </div>
          </div>
          
          <button @click="goToMetadata" class="w-full py-2 text-sm text-primary hover:bg-primary/5 rounded-lg border border-primary/20 mt-2">Improve Metadata</button>
        </div>
        
        <div class="stat-card lyrics-coverage bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Lyrics Coverage</h3>
          
          <div v-if="lyricsStats" class="flex items-center gap-4 mb-4">
            <div class="w-20 h-20 relative">
              <svg viewBox="0 0 100 100" class="transform -rotate-90">
                <circle cx="50" cy="50" r="35" fill="none" stroke="#3b82f6" stroke-width="15" 
                  :stroke-dasharray="`${(lyricsStats.total_tracks > 0 ? (lyricsStats.synced_lyrics / lyricsStats.total_tracks) * 100 : 0) * 2.2} 220`" />
                <circle cx="50" cy="50" r="35" fill="none" stroke="#9ca3af" stroke-width="15" 
                  :stroke-dasharray="`${(lyricsStats.total_tracks > 0 ? ((lyricsStats.with_lyrics - lyricsStats.synced_lyrics) / lyricsStats.total_tracks) * 100 : 0) * 2.2} 220`" 
                  :stroke-dashoffset="`-${(lyricsStats.total_tracks > 0 ? (lyricsStats.synced_lyrics / lyricsStats.total_tracks) * 100 : 0) * 2.2}`" />
                <circle cx="50" cy="50" r="35" fill="none" stroke="#ef4444" stroke-width="15" 
                  :stroke-dasharray="`${(lyricsStats.total_tracks > 0 ? ((lyricsStats.total_tracks - lyricsStats.with_lyrics) / lyricsStats.total_tracks) * 100 : 0) * 2.2} 220`" 
                  :stroke-dashoffset="`-${(lyricsStats.total_tracks > 0 ? (lyricsStats.with_lyrics / lyricsStats.total_tracks) * 100 : 0) * 2.2}`" />
              </svg>
            </div>
            
            <div class="flex-1 space-y-2 text-sm">
              <div class="flex justify-between">
                <span class="flex items-center gap-2"><span class="w-2 h-2 rounded bg-blue-500"></span>Synced</span>
                <span>{{ lyricsStats.total_tracks > 0 ? Math.round((lyricsStats.synced_lyrics / lyricsStats.total_tracks) * 100) : 0 }}%</span>
              </div>
              <div class="flex justify-between">
                <span class="flex items-center gap-2"><span class="w-2 h-2 rounded bg-gray-400"></span>Unsynced</span>
                <span>{{ lyricsStats.total_tracks > 0 ? Math.round(((lyricsStats.with_lyrics - lyricsStats.synced_lyrics) / lyricsStats.total_tracks) * 100) : 0 }}%</span>
              </div>
              <div class="flex justify-between">
                <span class="flex items-center gap-2"><span class="w-2 h-2 rounded bg-red-500"></span>Missing</span>
                <span>{{ lyricsStats.total_tracks > 0 ? Math.round(((lyricsStats.total_tracks - lyricsStats.with_lyrics) / lyricsStats.total_tracks) * 100) : 0 }}%</span>
              </div>
            </div>
          </div>
          
          <button 
            class="w-full py-2 text-sm text-primary hover:bg-primary/5 rounded-lg border border-primary/20 mt-2 disabled:opacity-50 disabled:cursor-not-allowed" 
            @click="fetchMissingLyrics"
            :disabled="isFetchingLyrics"
          >
            <span v-if="isFetchingLyrics" class="flex items-center justify-center gap-2">
              <span class="material-symbols-outlined text-[18px] animate-spin">sync</span>
              Fetching...
            </span>
            <span v-else>Fetch Missing Lyrics</span>
          </button>
        </div>
        
        <!-- Top Artists -->
        <div class="stat-card top-artists bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Top Artists</h3>
          <div v-if="stats.topArtists.length > 0" class="space-y-4">
            <div v-for="artist in stats.topArtists" :key="artist.name">
              <div class="flex justify-between text-sm mb-1">
                <span class="text-white truncate pr-2 max-w-[150px]">{{ artist.name }}</span>
                <span class="text-gray-500 shrink-0">{{ artist.tracks }} tracks</span>
              </div>
              <div class="h-2 bg-gray-100 dark:bg-gray-700 rounded-full overflow-hidden">
                <div class="bg-primary h-full rounded-full" :style="{ width: artist.percent + '%' }"></div>
              </div>
            </div>
          </div>
          <div v-else class="flex flex-col items-center justify-center py-8 text-gray-500 italic text-sm text-center">
            <span class="material-symbols-outlined text-4xl mb-2">person_search</span>
            No artists in library
          </div>
        </div>
        
        <!-- Top Genres (Stub - Regression S31) -->
        <div class="stat-card top-genres bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Top Genres</h3>
          <div class="flex flex-col items-center justify-center py-8 text-gray-500 italic text-sm text-center">
            <p class="text-3xl font-bold text-gray-600 mb-1">—</p>
            <p class="text-xs text-gray-500">Metadata analysis pending</p>
          </div>
        </div>
        
        <!-- Library Growth (Spans 2 columns) -->
        <div class="stat-card growth-chart col-span-1 md:col-span-2 bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-lg font-semibold text-white">Library Growth</h3>
            <select v-model="timeRange" class="px-3 py-1.5 bg-gray-100 dark:bg-surface-highlight rounded-lg text-sm text-gray-700 dark:text-gray-300">
              <option value="7d">Last 7 days</option>
              <option value="30d">Last 30 days</option>
              <option value="1y">Last year</option>
              <option value="all">All time</option>
            </select>
          </div>
          
          <!-- Simplified Chart Placeholder -->
          <div class="h-48 flex items-end gap-2 px-4">
            <div v-for="(point, index) in growthData" :key="index" class="flex-1 flex flex-col items-center gap-1">
              <div class="w-full bg-primary/20 rounded-t relative" :style="{ height: point.total + '%' }">
                <div class="absolute bottom-0 w-full bg-green-500 rounded-t" :style="{ height: (point.downloaded / point.total * 100) + '%' }"></div>
              </div>
              <span class="text-xs text-gray-400">{{ point.label }}</span>
            </div>
          </div>
          
          <div class="flex items-center gap-6 mt-4 text-sm">
            <span class="flex items-center gap-2">
              <span class="w-3 h-3 rounded bg-primary/20"></span>
              Total tracks
            </span>
            <span class="flex items-center gap-2">
              <span class="w-3 h-3 rounded bg-green-500"></span>
              Downloaded
            </span>
          </div>
        </div>
        
        <!-- Recent Activity -->
        <div class="stat-card activity-timeline bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">Recent Activity</h3>
          
          <div v-if="recentActivity.length > 0" class="space-y-4">
            <div v-for="activity in recentActivity" :key="activity.id" class="flex items-start gap-3">
              <div :class="['w-8 h-8 rounded-full flex items-center justify-center shrink-0', activity.color]">
                <span class="material-symbols-outlined text-white text-sm">{{ activity.icon }}</span>
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm text-white">{{ activity.text }}</p>
                <p class="text-xs text-gray-500">{{ activity.time }}</p>
              </div>
            </div>
          </div>
          <div v-else class="flex flex-col items-center justify-center py-10 text-gray-500 italic text-sm">
            <span class="material-symbols-outlined text-4xl mb-2">history</span>
            No recent activity
          </div>
        </div>
        
        <!-- Duplicates -->
        <div class="stat-card duplicates bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6" :class="{ 'opacity-60': duplicateStats === null }">
          <h3 class="text-lg font-semibold text-white mb-4">Duplicates</h3>
          
          <div class="text-center py-4">
            <p class="text-3xl font-bold text-gray-600 mb-1" :class="{ 'text-warning': duplicateStats && duplicateStats > 0 }">{{ duplicateStats ?? '—' }}</p>
            <p class="text-sm text-gray-500">{{ duplicateStats === null ? 'Scanning...' : 'Extra tracks detected' }}</p>
          </div>
          
          <div class="flex gap-2 mt-4">
            <button :disabled="!duplicateStats" @click="goToDuplicates" :class="[duplicateStats ? 'bg-primary/20 text-primary hover:bg-primary/30 cursor-pointer' : 'bg-primary/5 text-primary/40 cursor-not-allowed']" class="flex-1 py-2 text-sm rounded-lg transition-colors">Review</button>
            <button @click="handleAutoResolveDuplicates" :disabled="isAutoResolving || !duplicateStats" :title="isAutoResolving ? 'Resolving duplicates...' : 'Auto-resolve duplicates by keeping highest quality'" class="flex-1 py-2 text-sm border border-gray-800 text-gray-400 hover:text-white rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
              <span v-if="isAutoResolving" class="material-symbols-outlined text-sm animate-spin mr-1">sync</span>
              Auto-resolve
            </button>
          </div>
        </div>

        <!-- System Diagnostics -->
        <div class="stat-card system-diagnostics bg-surface-dark rounded-2xl shadow-sm border border-gray-200 dark:border-border-dark p-6">
          <h3 class="text-lg font-semibold text-white mb-4">System Diagnostics</h3>
          
          <div class="space-y-4">
            <!-- FFmpeg -->
            <div class="flex items-center justify-between p-3 bg-surface-highlight rounded-xl">
              <div class="flex items-center gap-3">
                <span class="material-symbols-outlined text-blue-400">transform</span>
                <span class="text-sm text-gray-200">FFmpeg (Conversion)</span>
              </div>
              <div class="flex items-center gap-2">
                <span v-if="ffmpegStatus === 'checking'" class="material-symbols-outlined text-sm animate-spin text-gray-500">sync</span>
                <span v-else-if="ffmpegStatus === 'installed'" class="material-symbols-outlined text-success">check_circle</span>
                <span v-else class="material-symbols-outlined text-error">cancel</span>
                <span class="text-xs uppercase font-bold" :class="ffmpegStatus === 'installed' ? 'text-success' : 'text-gray-500'">
                  {{ ffmpegStatus === 'checking' ? 'Checking' : ffmpegStatus }}
                </span>
              </div>
            </div>

            <!-- Chromaprint (fpcalc) -->
            <div class="flex items-center justify-between p-3 bg-surface-highlight rounded-xl">
              <div class="flex items-center gap-3">
                <span class="material-symbols-outlined text-purple-400">fingerprint</span>
                <span class="text-sm text-gray-200">Chromaprint (fpcalc)</span>
              </div>
              <div class="flex items-center gap-2">
                <span v-if="fpcalcStatus === 'checking'" class="material-symbols-outlined text-sm animate-spin text-gray-500">sync</span>
                <span v-else-if="fpcalcStatus === 'installed'" class="material-symbols-outlined text-success">check_circle</span>
                <span v-else class="material-symbols-outlined text-error">cancel</span>
                <span class="text-xs uppercase font-bold" :class="fpcalcStatus === 'installed' ? 'text-success' : 'text-gray-500'">
                  {{ fpcalcStatus === 'checking' ? 'Checking' : fpcalcStatus }}
                </span>
              </div>
            </div>
          </div>
          
          <p class="text-[10px] text-gray-500 mt-4 leading-tight italic">
            Required for audio conversion and duplicate detection.
          </p>
        </div>
        
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useToast } from '@/composables/useToast'
import { getVersion } from '@tauri-apps/api/app'
import { confirm } from '@tauri-apps/plugin-dialog'
import { libraryApi } from '@/api/library'
import { queueApi } from '@/api/queue'
import { accountsApi } from '@/api/accounts'
import { metadataApi } from '@/api/metadata'
import { lyricsApi } from '@/api/lyrics'
import { dashboardApi } from '@/api/dashboard'
import { toolsApi } from '@/api/tools'
import { getStorageStats, type StorageStats } from '@/api/storage'
import type { 
  LibraryStats, QueueStats, ServiceStatus, MetadataStats, 
  LyricsStats, LibrarySnapshot, 
  TopArtist, TopGenre, QualityBucket 
} from '@/api/types'


const router = useRouter()
const toast = useToast()

// State
const appVersion = ref('')
const isRefreshing = ref(false)
const lastUpdated = ref('loading...')
const timeRange = ref('30d')
const loading = ref(true)
const error = ref<string | null>(null)

// Backend data
const libraryStats = ref<LibraryStats | null>(null)
const queueStats = ref<QueueStats | null>(null)
const serviceStatuses = ref<ServiceStatus[]>([])
const metadataStats = ref<MetadataStats | null>(null)
const lyricsStats = ref<LyricsStats | null>(null)
const snapshots = ref<LibrarySnapshot[]>([])
const storageData = ref<StorageStats | null>(null)
const topArtistsData = ref<TopArtist[]>([])
const qualityData = ref<QualityBucket[]>([])
const duplicateStats = ref<number | null>(null)
const isFetchingLyrics = ref(false)
const isAutoResolving = ref(false)
const recentActivity = ref<{id: number, icon: string, color: string, text: string, time: string}[]>([])

// System Diagnostics State
const ffmpegStatus = ref<'checking' | 'installed' | 'missing'>('checking')
const fpcalcStatus = ref<'checking' | 'installed' | 'missing'>('checking')

// Helper to format bytes
function formatBytes(bytes: number, decimals = 2) {
  if (!bytes) return '0 Bytes'
  const k = 1024
  const dm = decimals < 0 ? 0 : decimals
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i]
}

// Computed stats (combining backend data with defaults)
const stats = computed(() => {
  const lib = libraryStats.value
  const downloadedPercent = lib && lib.total_tracks > 0 
    ? Math.round((lib.total_downloads / lib.total_tracks) * 100)
    : 0
  
  return {
    totalTracks: lib?.total_tracks ?? 0,
    totalAlbums: lib?.total_albums ?? 0,
    totalArtists: lib?.total_artists ?? 0,
    totalPlaylists: lib?.playlists ?? 0,
    downloadedTracks: lib?.total_downloads ?? 0,
    streamingTracks: (lib?.total_tracks ?? 0) - (lib?.total_downloads ?? 0),
    downloadedPercent,
    storageUsed: storageData.value ? formatBytes(storageData.value.used_bytes) : '-- GB',
    storageAvailable: storageData.value ? formatBytes(storageData.value.available_bytes) : '-- GB',
    perTrackSize: (lib?.total_tracks ?? 0) > 0 && storageData.value 
      ? formatBytes(storageData.value.used_bytes / lib!.total_tracks) 
      : '—',
    services: serviceStatuses.value.map((s, i) => ({
      name: s.name.charAt(0).toUpperCase() + s.name.slice(1),
      percent: s.library_count > 0 ? Math.round((s.library_count / (lib?.total_tracks || 1)) * 100) : 0,
      color: ['bg-blue-500', 'bg-green-500', 'bg-gray-900', 'bg-gray-400'][i] || 'bg-gray-400'
    })),
    topArtists: topArtistsData.value.map((a, i, arr) => ({
      name: a.name,
      tracks: a.track_count,
      percent: arr[0].track_count > 0 ? Math.round((a.track_count / arr[0].track_count) * 100) : 0
    })),
    topGenres: []
  }
})

// Growth chart data
const growthData = ref([
  { label: 'Jan', total: 65, downloaded: 40 },
  { label: 'Feb', total: 72, downloaded: 50 },
  { label: 'Mar', total: 78, downloaded: 58 },
  { label: 'Apr', total: 82, downloaded: 65 },
  { label: 'May', total: 88, downloaded: 72 },
  { label: 'Jun', total: 95, downloaded: 82 },
  { label: 'Jul', total: 100, downloaded: 90 },
])

// Fetch system secondary diagnostics (FFmpeg, fpcalc)
async function fetchSystemDiagnostics() {
  ffmpegStatus.value = 'checking'
  fpcalcStatus.value = 'checking'
  try {
    const [ffmpeg, fpcalc] = await Promise.all([
      toolsApi.checkFfmpeg(),
      toolsApi.checkFingerprint()
    ])
    ffmpegStatus.value = ffmpeg.success ? 'installed' : 'missing'
    fpcalcStatus.value = fpcalc.success ? 'installed' : 'missing'
  } catch (e) {
    console.error('Failed to check system tools:', e)
    ffmpegStatus.value = 'missing'
    fpcalcStatus.value = 'missing'
  }
}

// Fetch all data
async function handleAutoResolveDuplicates() {
  if (isAutoResolving.value) return
  
  const confirmed = await confirm(
    'Auto-resolve will keep the highest quality version of each duplicate group. Downloaded files are always preserved. This cannot be undone.',
    { title: 'Auto-resolve Duplicates', kind: 'warning' }
  )
  
  if (confirmed !== true) return
  
  isAutoResolving.value = true
  try {
    const result = await dashboardApi.autoResolveDuplicates()
    
    toast.success(`Resolved ${result.groups_resolved} groups, removed ${result.tracks_removed} duplicates`)
    await fetchData()
  } catch (e) {
    toast.error(`Auto-resolve failed: ${e}`)
  } finally {
    isAutoResolving.value = false
  }
}

async function fetchData() {
  loading.value = true
  error.value = null
  
  // Start diagnostics in parallel
  fetchSystemDiagnostics()
  
  try {
    const [
      libStats, qStats, services, meta, lyrics, snaps, storage, 
      topArtists, qualityList, dupeStats
    ] = await Promise.all([
      libraryApi.getLibraryStats(),
      queueApi.getQueueStats(),
      accountsApi.getServiceStatuses(),
      metadataApi.getMetadataStats(),
      lyricsApi.getLyricsStats(),
      dashboardApi.getLibrarySnapshots(30),
      getStorageStats(),
      libraryApi.getTopArtists(5),
      libraryApi.getAudioQualityDistribution(),
      dashboardApi.getDuplicateStats().catch(() => 0) // Fallback inline for partial failures
    ])
    
    libraryStats.value = libStats
    queueStats.value = qStats
    serviceStatuses.value = services
    metadataStats.value = meta
    lyricsStats.value = lyrics
    snapshots.value = snaps
    storageData.value = storage
    topArtistsData.value = topArtists
    qualityData.value = qualityList
    duplicateStats.value = dupeStats
    
    // Map snapshots to growthData
    if (snaps && snaps.length > 0) {
      const maxTracks = Math.max(...snaps.map((s: LibrarySnapshot) => s.total_tracks), 1)
      growthData.value = snaps.map((s: LibrarySnapshot) => ({
        label: s.snapshot_date.split('-').slice(1).join('/'),
        total: Math.round((s.total_tracks / maxTracks) * 100),
        downloaded: Math.round((s.downloaded_tracks / maxTracks) * 100)
      }))
    } else {
      growthData.value = [{ label: 'Today', total: 0, downloaded: 0 }]
    }
    
    lastUpdated.value = 'just now'
  } catch (e) {
    console.error('Failed to fetch dashboard data:', e)
    error.value = e instanceof Error ? e.message : 'Failed to load data'
    lastUpdated.value = 'error'
  } finally {
    loading.value = false
  }
}

async function refresh() {
  isRefreshing.value = true
  await fetchData()
  isRefreshing.value = false
}

function goToDuplicates() {
  router.push('/library?filter=duplicates')
}

function exportReport() {
  const report = {
    exportedAt: new Date().toISOString(),
    syncifyVersion: appVersion.value || 'unknown',
    libraryStats: {
      totalTracks: stats.value.totalTracks,
      totalAlbums: stats.value.totalAlbums,
      totalArtists: stats.value.totalArtists,
      totalPlaylists: stats.value.totalPlaylists,
      downloadedTracks: stats.value.downloadedTracks,
      streamingTracks: stats.value.streamingTracks,
      downloadedPercent: stats.value.downloadedPercent,
      storageUsed: stats.value.storageUsed,
    },
    services: stats.value.services.map(s => ({
      name: s.name,
      libraryPercent: s.percent
    })),
    queueStatus: {
      queued: queueStats.value?.queued ?? 0,
      downloading: queueStats.value?.downloading ?? 0,
      completed: queueStats.value?.completed ?? 0,
      failed: queueStats.value?.failed ?? 0,
    },
    connectedServices: serviceStatuses.value.map(s => ({
      name: s.name,
      connected: s.connected,
      email: s.account_email,
      libraryCount: s.library_count
    })),
    rawData: {
      libraryStats: libraryStats.value,
      queueStats: queueStats.value,
      serviceStatuses: serviceStatuses.value
    }
  }
  
  // Create downloadable JSON file
  const blob = new Blob([JSON.stringify(report, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `syncify-dashboard-report-${new Date().toISOString().slice(0, 10)}.json`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
  
  // Also copy to clipboard for easy sharing
  const textReport = `
=== Syncify Dashboard Report ===
Generated: ${new Date().toLocaleString()}

📊 LIBRARY STATS
Total Tracks: ${stats.value.totalTracks.toLocaleString()}
Albums: ${stats.value.totalAlbums}
Artists: ${stats.value.totalArtists}
Playlists: ${stats.value.totalPlaylists}
Downloaded: ${stats.value.downloadedTracks.toLocaleString()} (${stats.value.downloadedPercent}%)
Streaming Only: ${stats.value.streamingTracks.toLocaleString()}

📥 DOWNLOAD QUEUE
Queued: ${queueStats.value?.queued ?? 0}
Downloading: ${queueStats.value?.downloading ?? 0}
Completed: ${queueStats.value?.completed ?? 0}
Failed: ${queueStats.value?.failed ?? 0}

🔗 CONNECTED SERVICES
${serviceStatuses.value.map(s => `- ${s.name}: ${s.connected ? 'Connected' : 'Disconnected'} (${s.library_count} tracks)`).join('\n')}
`.trim()
  
  navigator.clipboard.writeText(textReport).then(() => {
    console.log('Report copied to clipboard!')
  }).catch(() => {
    console.log('Clipboard copy failed, but file was downloaded')
  })
  
  console.log('Dashboard report exported:', report)
}
function handleQualityClick(label: string) {
  router.push({ path: '/library', query: { filter: 'quality', quality: label } })
}
function goToFailed() {
  router.push({ path: '/downloads', query: { filter: 'failed' } })
}
function goToMetadata() {
  router.push({ path: '/metadata', query: { filter: 'needs_work' } })
}
async function fetchMissingLyrics() {
  if (isFetchingLyrics.value) return
  isFetchingLyrics.value = true
  try {
    const result = await lyricsApi.fetchMissingLyrics()
    toast.success(`Fetched ${result.fetched} lyrics`, `${result.skipped} skipped, ${result.failed} failed`)
    await fetchData()
  } catch (e) {
    console.error('Failed to fetch missing lyrics:', e)
    toast.error('Failed to fetch lyrics', String(e))
  } finally {
    isFetchingLyrics.value = false
  }
}


onMounted(async () => {
  await dashboardApi.createLibrarySnapshot()
  fetchData()
  
  // Get app version
  try {
    appVersion.value = await getVersion()
  } catch (e) {
    console.error('Failed to get app version:', e)
    appVersion.value = 'unknown'
  }
})
</script>

<style scoped>
@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 1s linear infinite;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.custom-scrollbar::-webkit-scrollbar-thumb {
  background-color: rgba(155, 155, 155, 0.3);
  border-radius: 3px;
}
</style>
