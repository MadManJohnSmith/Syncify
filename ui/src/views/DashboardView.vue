<template>
  <div class="stats-dashboard h-full min-h-0 flex flex-col overflow-hidden bg-background-dark">
    <!-- Header -->
    <div class="shrink-0 px-4 py-2.5 border-b border-border-dark flex items-center justify-between gap-4">
      <div class="flex items-baseline gap-3 min-w-0">
        <h1 class="text-lg font-bold text-ink">Dashboard</h1>
        <p class="text-xs text-gray-500 truncate">Library statistics and analytics</p>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <span class="text-[11px] text-gray-400">Updated {{ lastUpdated }}</span>
        <button @click="refresh" class="px-3 py-1.5 bg-surface-dark border border-border-dark text-gray-300 rounded-lg text-xs flex items-center gap-1.5 hover:bg-surface-highlight">
          <span class="material-symbols-outlined text-base" :class="{ 'animate-spin': isRefreshing }">refresh</span>
          Refresh
        </button>
        <button @click="exportReport" class="px-3 py-1.5 bg-primary hover:bg-primary-hover text-on-accent rounded-lg text-xs flex items-center gap-1.5">
          <span class="material-symbols-outlined text-base">download</span>
          Export Report
        </button>
      </div>
    </div>

    <!-- Loading state -->
    <div v-if="loading" class="flex-1 min-h-0 flex flex-col items-center justify-center">
      <div class="w-10 h-10 border-4 border-primary border-t-transparent rounded-full animate-spin mb-3"></div>
      <p class="text-sm text-gray-400">Loading dashboard data...</p>
    </div>

    <!-- Error state -->
    <div v-else-if="error" class="flex-1 min-h-0 flex flex-col items-center justify-center px-6 text-center">
      <span class="material-symbols-outlined text-4xl text-red-500 mb-3">error</span>
      <h3 class="text-base font-semibold text-ink mb-1">Failed to load dashboard</h3>
      <p class="text-sm text-gray-400 max-w-md mb-4">{{ error }}</p>
      <button @click="fetchData" class="px-4 py-1.5 bg-surface-highlight hover:bg-surface-highlight/80 text-ink rounded-lg text-sm">
        Try Again
      </button>
    </div>

    <!-- Dashboard grid: una sola pantalla, la página no hace scroll -->
    <div v-else class="flex-1 min-h-0 overflow-y-auto xl:overflow-hidden p-3">
      <div class="xl:h-full grid grid-cols-1 md:grid-cols-2 xl:grid-cols-12 gap-3 xl:grid-rows-3 auto-rows-fr">
        <!-- Library Overview -->
        <div class="stat-card library-overview xl:col-span-4 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Library Overview</h3>
          <div class="flex-1 min-h-0 grid grid-cols-3 gap-2 content-start">
            <div class="metric-box bg-surface-highlight rounded-lg px-2 py-1.5 text-center">
              <p class="text-lg font-bold text-ink leading-tight">{{ stats.totalTracks.toLocaleString() }}</p>
              <p class="text-[11px] text-gray-500">Tracks</p>
            </div>
            <div class="metric-box bg-surface-highlight rounded-lg px-2 py-1.5 text-center">
              <p class="text-lg font-bold text-purple-400 leading-tight">{{ stats.totalAlbums }}</p>
              <p class="text-[11px] text-gray-500">Albums</p>
            </div>
            <div class="metric-box bg-surface-highlight rounded-lg px-2 py-1.5 text-center">
              <p class="text-lg font-bold text-teal-400 leading-tight">{{ stats.totalArtists }}</p>
              <p class="text-[11px] text-gray-500">Artists</p>
            </div>
            <div class="metric-box bg-surface-highlight rounded-lg px-2 py-1.5 text-center">
              <p class="text-lg font-bold text-indigo-400 leading-tight">{{ stats.totalPlaylists }}</p>
              <p class="text-[11px] text-gray-500">Playlists</p>
            </div>
            <div class="metric-box bg-surface-highlight rounded-lg px-2 py-1.5 text-center">
              <p class="text-lg font-bold text-green-400 leading-tight">{{ stats.downloadedTracks.toLocaleString() }}</p>
              <p class="text-[11px] text-gray-500">Downloaded</p>
            </div>
            <div class="metric-box bg-surface-highlight rounded-lg px-2 py-1.5 text-center">
              <p class="text-lg font-bold text-amber-400 leading-tight">{{ stats.activeDownloads }}</p>
              <p class="text-[11px] text-gray-500">Active downloads</p>
            </div>
          </div>
          <div class="shrink-0 mt-2 pt-2 border-t border-border-dark flex items-center justify-between text-[11px] text-gray-500">
            <span>{{ stats.streamingTracks.toLocaleString() }} streaming only</span>
            <span>{{ stats.downloadedPercent }}% of the library on disk</span>
          </div>
        </div>

        <!-- Library Growth -->
        <div class="stat-card growth-chart xl:col-span-5 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <div class="flex items-center justify-between mb-2 shrink-0 gap-2">
            <h3 class="text-xs font-semibold text-ink">Library Growth</h3>
            <select v-model="timeRange" class="px-2 py-1 bg-surface-highlight border border-border-dark rounded text-[11px] text-gray-300 focus:outline-none">
              <option value="7d">Last 7 days</option>
              <option value="30d">Last 30 days</option>
              <option value="1y">Last year</option>
              <option value="all">All time</option>
            </select>
          </div>
          <div v-if="growthData.length > 0" class="flex-1 min-h-0 flex items-end gap-3">
            <div class="flex-1 min-h-0 flex items-end justify-around gap-2">
              <div v-for="(entry, i) in growthData" :key="i" class="flex-1 flex flex-col items-center gap-1 min-w-0">
                <div class="w-full flex items-end justify-center gap-0.5 h-full max-h-[150px]">
                  <div class="w-1/3 bg-primary/40 rounded-t" :style="{ height: entry.total + '%' }"></div>
                  <div class="w-1/3 bg-primary rounded-t" :style="{ height: entry.downloaded + '%' }"></div>
                </div>
                <span class="text-[10px] text-gray-500 truncate w-full text-center">{{ entry.label }}</span>
              </div>
            </div>
            <div class="shrink-0 space-y-1 text-[10px] text-gray-400">
              <div class="flex items-center gap-1">
                <span class="w-2 h-2 rounded bg-primary/40"></span>Total tracks
              </div>
              <div class="flex items-center gap-1">
                <span class="w-2 h-2 rounded bg-primary"></span>Downloaded
              </div>
            </div>
          </div>
          <div v-else class="flex-1 min-h-0 flex flex-col items-center justify-center text-center">
            <span class="material-symbols-outlined text-3xl text-gray-600 mb-1">show_chart</span>
            <p class="text-sm text-gray-300 font-medium">No library growth data</p>
            <p class="text-[11px] text-gray-500 mt-0.5 mb-2">Track trends and history will appear here once tracks are imported.</p>
            <button @click="router.push('/library')" class="px-3 py-1 text-xs bg-primary/20 text-primary hover:bg-primary/30 rounded-lg transition-colors">
              Go to Library
            </button>
          </div>
        </div>

        <!-- Storage Usage -->
        <div class="stat-card storage-usage xl:col-span-3 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Storage Usage</h3>
          <p class="text-xl font-bold text-ink shrink-0">{{ stats.storageUsed }}</p>

          <div class="h-2 bg-gray-700 rounded-full overflow-hidden flex mt-2 mb-2 shrink-0">
            <template v-if="storageData">
              <div
                v-for="(item, i) in storageData.breakdown"
                :key="item.format"
                :class="[i === 0 ? 'bg-blue-500' : i === 1 ? 'bg-green-500' : 'bg-gray-400', 'h-full']"
                :style="{ width: (item.size_bytes / storageData.used_bytes * 100) + '%' }"
              ></div>
            </template>
            <div v-else class="bg-gray-600 h-full w-full"></div>
          </div>

          <div class="flex-1 min-h-0 space-y-1 text-[11px] overflow-hidden">
            <template v-if="storageData">
              <div v-for="(item, i) in storageData.breakdown" :key="item.format" class="flex justify-between">
                <span class="flex items-center gap-1.5 text-gray-300">
                  <span :class="[i === 0 ? 'bg-blue-500' : i === 1 ? 'bg-green-500' : 'bg-gray-400', 'w-2 h-2 rounded']"></span>
                  {{ item.format }}
                </span>
                <span class="text-gray-500">{{ formatBytes(item.size_bytes) }}</span>
              </div>
            </template>
            <p v-else class="text-gray-500 py-1">No storage data</p>
          </div>

          <p class="shrink-0 text-[11px] text-gray-500 mt-1">{{ freeSpaceLabel }} · ~{{ stats.perTrackSize }} per track</p>
        </div>

        <!-- Quality Distribution -->
        <div class="stat-card quality-distribution xl:col-span-3 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Audio Quality</h3>
          <div v-if="qualityData.length > 0" class="flex-1 min-h-0 space-y-2 overflow-hidden">
            <div v-for="(item, i) in qualityData" :key="item.label" @click="handleQualityClick(item.label)" class="cursor-pointer hover:bg-white/5 rounded-lg px-1 -mx-1 py-0.5 transition-colors">
              <div class="flex justify-between text-[11px] mb-0.5">
                <span class="text-gray-300 truncate">{{ item.label }}</span>
                <span class="text-gray-500 shrink-0">{{ item.count }} tracks</span>
              </div>
              <div class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
                <div
                  :class="[i === 0 ? 'bg-indigo-500' : i === 1 ? 'bg-blue-400' : 'bg-gray-500', 'h-full rounded-full']"
                  :style="{ width: (item.count / stats.totalTracks * 100) + '%' }"
                ></div>
              </div>
            </div>
          </div>
          <div v-else class="flex-1 min-h-0 flex flex-col items-center justify-center text-gray-500 italic text-xs">
            <span class="material-symbols-outlined text-3xl mb-1">graphic_eq</span>
            No downloads yet
          </div>
        </div>

        <!-- Service Distribution -->
        <div class="stat-card service-distribution xl:col-span-3 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Sources</h3>
          <div class="flex-1 min-h-0 space-y-2 overflow-hidden">
            <div v-for="service in stats.services" :key="service.name">
              <div class="flex justify-between text-[11px] mb-0.5">
                <span class="text-ink truncate pr-2">{{ service.name }}</span>
                <span class="text-gray-500 shrink-0">{{ service.percent }}%</span>
              </div>
              <div class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
                <div :class="[service.color, 'h-full rounded-full']" :style="{ width: service.percent + '%' }"></div>
              </div>
            </div>
          </div>
        </div>

        <!-- Download Queue -->
        <div class="stat-card queue-stats xl:col-span-2 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Download Queue</h3>
          <div v-if="queueStats" class="flex-1 min-h-0 space-y-1.5 text-[11px] overflow-hidden">
            <div class="flex justify-between">
              <span class="text-gray-400">Queued</span>
              <span class="text-gray-200 font-mono">{{ queueStats.queued }}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-blue-400">Downloading</span>
              <span class="text-blue-300 font-mono">{{ queueStats.downloading }}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-gray-400">Completed</span>
              <span class="text-green-500 font-mono">+{{ queueStats.completed }} tracks</span>
            </div>
            <div class="flex justify-between">
              <span class="text-gray-400">Failed</span>
              <button v-if="queueStats.failed > 0" @click="goToFailed" class="text-red-400 hover:underline font-mono">{{ queueStats.failed }}</button>
              <span v-else class="text-gray-600 font-mono">0</span>
            </div>
            <div class="flex justify-between">
              <span class="text-gray-600">In the queue overall</span>
              <span class="text-gray-400 font-mono">{{ queueStats.total }}</span>
            </div>
          </div>
          <div v-else class="flex-1 min-h-0 flex items-center justify-center text-gray-500 italic text-xs">No queue data</div>
        </div>

        <!-- Recent Activity -->
        <div class="stat-card recent-activity xl:col-span-2 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Recent Activity</h3>
          <div v-if="recentActivity.length > 0" class="flex-1 min-h-0 space-y-1.5 overflow-hidden">
            <div v-for="activity in recentActivity" :key="activity.id" class="flex items-center gap-2">
              <div :class="['w-6 h-6 rounded-full flex items-center justify-center shrink-0', activity.color]">
                <span class="material-symbols-outlined text-xs">{{ activity.icon }}</span>
              </div>
              <div class="min-w-0 flex-1">
                <p class="text-[11px] text-ink truncate">{{ activity.text }}</p>
                <p class="text-[10px] text-gray-500">{{ activity.time }}</p>
              </div>
            </div>
          </div>
          <div v-else class="flex-1 min-h-0 flex flex-col items-center justify-center text-center">
            <span class="material-symbols-outlined text-3xl text-gray-600 mb-1">history</span>
            <p class="text-sm text-gray-300 font-medium">No recent activity</p>
            <p class="text-[11px] text-gray-500 mt-0.5 mb-2">Activity from downloads and queue tasks will appear here.</p>
            <button @click="router.push('/queue')" class="px-3 py-1 text-xs bg-primary/20 text-primary hover:bg-primary/30 rounded-lg transition-colors">
              Go to Queue
            </button>
          </div>
        </div>

        <!-- System Diagnostics -->
        <div class="stat-card system-diagnostics xl:col-span-2 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">System Diagnostics</h3>
          <div class="flex-1 min-h-0 space-y-1.5 text-[11px] overflow-hidden">
            <div class="flex items-center justify-between bg-surface-highlight rounded-lg px-2 py-1.5">
              <span class="flex items-center gap-1.5 text-gray-200">
                <span class="material-symbols-outlined text-sm text-blue-400">transform</span>
                FFmpeg
              </span>
              <span v-if="ffmpegStatus === 'checking'" class="material-symbols-outlined text-sm animate-spin text-gray-500">sync</span>
              <span v-else-if="ffmpegStatus === 'installed'" class="text-success">Installed</span>
              <span v-else class="text-red-400">Missing</span>
            </div>
            <div class="flex items-center justify-between bg-surface-highlight rounded-lg px-2 py-1.5">
              <span class="flex items-center gap-1.5 text-gray-200">
                <span class="material-symbols-outlined text-sm text-purple-400">fingerprint</span>
                Chromaprint
              </span>
              <span v-if="fpcalcStatus === 'checking'" class="material-symbols-outlined text-sm animate-spin text-gray-500">sync</span>
              <span v-else-if="fpcalcStatus === 'installed'" class="text-success">Installed</span>
              <span v-else class="text-red-400">Missing</span>
            </div>
          </div>
        </div>

        <!-- Metadata Quality -->
        <div class="stat-card metadata-quality xl:col-span-3 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Metadata Quality</h3>
          <div v-if="metadataStats" class="flex-1 min-h-0 flex gap-3 overflow-hidden">
            <div class="w-16 h-16 relative shrink-0 self-start">
              <svg viewBox="0 0 100 100" class="transform -rotate-90 w-full h-full">
                <circle cx="50" cy="50" r="40" fill="none" style="stroke: var(--surface-2)" stroke-width="10" />
                <circle cx="50" cy="50" r="40" fill="none" style="stroke: var(--accent)" stroke-width="10" stroke-linecap="round"
                  :stroke-dasharray="`${(metadataStats?.average_completeness || 0) * 2.51} 251`" />
              </svg>
              <div class="absolute inset-0 flex items-center justify-center">
                <p class="text-xs font-bold text-accent">{{ Math.round(metadataStats?.average_completeness || 0) }}%</p>
              </div>
            </div>
            <div class="flex-1 min-w-0 space-y-1 text-[11px]">
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
          <div v-else class="flex-1 min-h-0 flex items-center justify-center text-gray-500 italic text-xs">No metadata data</div>
        </div>

        <!-- Lyrics Coverage -->
        <div class="stat-card lyrics-coverage xl:col-span-3 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Lyrics Coverage</h3>
          <div v-if="lyricsStats" class="flex-1 min-h-0 flex flex-col overflow-hidden">
            <div class="shrink-0 flex items-baseline gap-2 mb-2">
              <span class="text-xl font-bold text-ink">{{ lyricsStats.total_tracks > 0 ? Math.round((lyricsStats.with_lyrics / lyricsStats.total_tracks) * 100) : 0 }}%</span>
              <span class="text-[11px] text-gray-500">{{ lyricsStats.with_lyrics }} / {{ lyricsStats.total_tracks }} tracks</span>
            </div>
            <div class="shrink-0 h-1.5 bg-gray-700 rounded-full overflow-hidden mb-2">
              <div class="h-full bg-primary rounded-full" :style="{ width: (lyricsStats.total_tracks > 0 ? (lyricsStats.with_lyrics / lyricsStats.total_tracks) * 100 : 0) + '%' }"></div>
            </div>
            <div class="flex-1 min-h-0 flex items-end justify-between text-[10px] text-gray-500 shrink-0">
              <span class="flex items-center gap-1"><span class="w-2 h-2 rounded bg-green-500"></span>{{ lyricsStats.synced_lyrics }} synced</span>
              <span class="flex items-center gap-1"><span class="w-2 h-2 rounded bg-yellow-500"></span>Missing {{ lyricsStats.total_tracks > 0 ? lyricsStats.total_tracks - lyricsStats.with_lyrics : 0 }}</span>
            </div>
            <button
              class="shrink-0 mt-2 py-1 text-[11px] text-primary hover:bg-primary/5 rounded-lg border border-primary/20 disabled:opacity-50 disabled:cursor-not-allowed"
              @click="fetchMissingLyrics"
              :disabled="isFetchingLyrics"
            >
              <span v-if="isFetchingLyrics" class="flex items-center justify-center gap-1">
                <span class="material-symbols-outlined text-sm animate-spin">sync</span>
                Fetching...
              </span>
              <span v-else>Fetch Missing Lyrics</span>
            </button>
          </div>
          <div v-else class="flex-1 min-h-0 flex items-center justify-center text-gray-500 italic text-xs">No lyrics data</div>
        </div>

        <!-- Top Artists -->
        <div class="stat-card top-artists xl:col-span-2 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Top Artists</h3>
          <div v-if="stats.topArtists.length > 0" class="flex-1 min-h-0 space-y-1.5 overflow-hidden">
            <div v-for="artist in stats.topArtists" :key="artist.name">
              <div class="flex justify-between text-[11px] mb-0.5">
                <span class="text-ink truncate pr-2">{{ artist.name }}</span>
                <span class="text-gray-500 shrink-0">{{ artist.tracks }} tracks</span>
              </div>
              <div class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
                <div class="bg-primary h-full rounded-full" :style="{ width: artist.percent + '%' }"></div>
              </div>
            </div>
          </div>
          <div v-else class="flex-1 min-h-0 flex flex-col items-center justify-center text-gray-500 italic text-xs text-center">
            <span class="material-symbols-outlined text-3xl mb-1">person_search</span>
            No artists in library
          </div>
        </div>

        <!-- Top Genres -->
        <div class="stat-card top-genres xl:col-span-2 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Top Genres</h3>
          <div v-if="stats.topGenres.length > 0" class="flex-1 min-h-0 space-y-1.5 overflow-hidden">
            <div v-for="genre in stats.topGenres" :key="genre.name">
              <div class="flex justify-between text-[11px] mb-0.5">
                <span class="text-ink truncate pr-2">{{ genre.name }}</span>
                <span class="text-gray-500 shrink-0">{{ genre.tracks }} tracks</span>
              </div>
              <div class="h-1.5 bg-gray-700 rounded-full overflow-hidden">
                <div class="bg-primary h-full rounded-full" :style="{ width: genre.percent + '%' }"></div>
              </div>
            </div>
          </div>
          <div v-else class="flex-1 min-h-0 flex flex-col items-center justify-center text-gray-500 italic text-xs text-center">
            <span class="material-symbols-outlined text-3xl mb-1">category</span>
            No genres in library
          </div>
        </div>

        <!-- Duplicates -->
        <div class="stat-card duplicates xl:col-span-2 bg-surface-dark rounded-xl border border-border-dark p-3 flex flex-col min-h-0 overflow-hidden" :class="{ 'opacity-60': duplicateStats === null }">
          <h3 class="text-xs font-semibold text-ink mb-2 shrink-0">Duplicates</h3>
          <div class="flex-1 min-h-0 flex flex-col items-center justify-center">
            <p class="text-2xl font-bold text-gray-600 mb-0.5" :class="{ 'text-warning': duplicateStats && duplicateStats > 0 }">{{ duplicateStats ?? '—' }}</p>
            <p class="text-[11px] text-gray-500 text-center">{{ duplicateStats === null ? 'Scanning...' : 'Extra tracks detected' }}</p>
          </div>
          <div class="shrink-0 flex gap-1.5 mt-2">
            <button :disabled="!duplicateStats" @click="goToDuplicates" :class="[duplicateStats ? 'bg-primary/20 text-primary hover:bg-primary/30 cursor-pointer' : 'bg-primary/5 text-primary/40 cursor-not-allowed']" class="flex-1 py-1 text-[11px] rounded-lg transition-colors">Review</button>
            <button @click="handleAutoResolveDuplicates" :disabled="isAutoResolving || !duplicateStats" title="Auto-resolve duplicates by keeping highest quality" class="flex-1 py-1 text-[11px] border border-border-dark text-gray-400 hover:text-ink rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
              <span v-if="isAutoResolving" class="material-symbols-outlined text-xs animate-spin mr-1">sync</span>
              Auto-resolve
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useToast } from '@/composables/useToast'
import { getVersion } from '@tauri-apps/api/app'
// R13: confirm unificado (antes plugin-dialog nativo, fuera de la ventana).
import { confirm } from '@/composables/useToast'
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
const topGenresData = ref<TopGenre[]>([])
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

function getDaysForTimeRange(range: string): number {
  switch (range) {
    case '7d': return 7
    case '30d': return 30
    case '1y': return 365
    case 'all': return 3650
    default: return 30
  }
}

function formatTimeAgo(dateStr: string): string {
  try {
    const d = new Date(dateStr)
    const now = new Date()
    const diffSec = Math.floor((now.getTime() - d.getTime()) / 1000)
    if (isNaN(diffSec) || diffSec < 0) return 'Just now'
    if (diffSec < 60) return 'Just now'
    if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`
    if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`
    return `${Math.floor(diffSec / 86400)}d ago`
  } catch {
    return 'Recently'
  }
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
    activeDownloads: lib?.active_downloads ?? 0,
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
    topGenres: topGenresData.value.map((g, i, arr) => ({
      name: g.name,
      tracks: g.count,
      percent: arr[0]?.count > 0 ? Math.round((g.count / arr[0].count) * 100) : 0
    }))
  }
})

// Growth chart data (initialized empty to avoid fictitious mock data)
const growthData = ref<{ label: string, total: number, downloaded: number }[]>([])

// `get_storage_stats` returns total_bytes = 0 when it cannot locate the disk
// holding the library (see storage.rs:36-59). Painting that 0 as free space
// claimed a full disk that was never measured.
const freeSpaceLabel = computed(() => {
  const storage = storageData.value
  if (!storage || storage.total_bytes <= 0) return 'Free space unavailable'
  return `${formatBytes(storage.available_bytes)} free on disk`
})

async function fetchRecentActivity() {
  try {
    const items = await queueApi.getQueue(undefined, 8)
    if (items && items.length > 0) {
      recentActivity.value = items.slice(0, 5).map(item => {
        let icon = 'download'
        let color = 'bg-blue-500/20 text-blue-400'
        let statusText = 'Downloaded'
        const status = (item.status || '').toLowerCase()
        if (status === 'downloading') {
          icon = 'sync'
          color = 'bg-blue-500/20 text-blue-400'
          statusText = 'Downloading'
        } else if (status === 'failed') {
          icon = 'error'
          color = 'bg-red-500/20 text-red-400'
          statusText = 'Failed'
        } else if (status === 'queued') {
          icon = 'schedule'
          color = 'bg-yellow-500/20 text-yellow-400'
          statusText = 'Queued'
        } else if (status === 'complete' || status === 'completed') {
          icon = 'check_circle'
          color = 'bg-green-500/20 text-green-400'
          statusText = 'Downloaded'
        }
        const title = item.title || item.target_title || 'Unknown track'
        const artist = item.artist || item.target_artist ? ` • ${item.artist || item.target_artist}` : ''
        const timeStr = item.completed_at || item.started_at || item.created_at
        const time = timeStr ? formatTimeAgo(timeStr) : 'Recently'
        return {
          id: item.id,
          icon,
          color,
          text: `${statusText}: ${title}${artist}`,
          time
        }
      })
    } else {
      recentActivity.value = []
    }
  } catch (e) {
    console.error('Failed to fetch recent activity:', e)
    recentActivity.value = []
  }
}

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
    { title: 'Auto-resolve Duplicates', variant: 'warning' }
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
      topArtists, topGenres, qualityList, dupeStats
    ] = await Promise.all([
      libraryApi.getLibraryStats(),
      queueApi.getQueueStats(),
      accountsApi.getServiceStatuses(),
      metadataApi.getMetadataStats(),
      lyricsApi.getLyricsStats(),
      dashboardApi.getLibrarySnapshots(getDaysForTimeRange(timeRange.value)).catch(() => []),
      getStorageStats(),
      libraryApi.getTopArtists(5),
      libraryApi.getTopGenres(5).catch(() => []),
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
    topGenresData.value = topGenres
    qualityData.value = qualityList
    duplicateStats.value = dupeStats
    
    // Map snapshots to growthData or derive from real library statistics
    if (snaps && snaps.length > 0 && snaps.some((s: LibrarySnapshot) => s.total_tracks > 0)) {
      const maxTracks = Math.max(...snaps.map((s: LibrarySnapshot) => s.total_tracks), 1)
      growthData.value = snaps.map((s: LibrarySnapshot) => ({
        label: s.snapshot_date.split('-').slice(1).join('/'),
        total: Math.round((s.total_tracks / maxTracks) * 100),
        downloaded: Math.round((s.downloaded_tracks / maxTracks) * 100)
      }))
    } else if (libStats && libStats.total_tracks > 0) {
      const dlPct = Math.round((libStats.total_downloads / libStats.total_tracks) * 100)
      growthData.value = [
        { label: 'Current', total: 100, downloaded: dlPct }
      ]
    } else {
      growthData.value = []
    }
    
    await fetchRecentActivity()
    
    lastUpdated.value = 'just now'
  } catch (e) {
    console.error('Failed to fetch dashboard data:', e)
    error.value = e instanceof Error ? e.message : 'Failed to load data'
    lastUpdated.value = 'error'
  } finally {
    loading.value = false
  }
}

watch(timeRange, async (newRange) => {
  try {
    const snaps = await dashboardApi.getLibrarySnapshots(getDaysForTimeRange(newRange)).catch(() => [])
    if (snaps && snaps.length > 0 && snaps.some((s: LibrarySnapshot) => s.total_tracks > 0)) {
      const maxTracks = Math.max(...snaps.map((s: LibrarySnapshot) => s.total_tracks), 1)
      growthData.value = snaps.map((s: LibrarySnapshot) => ({
        label: s.snapshot_date.split('-').slice(1).join('/'),
        total: Math.round((s.total_tracks / maxTracks) * 100),
        downloaded: Math.round((s.downloaded_tracks / maxTracks) * 100)
      }))
    } else if (libraryStats.value && libraryStats.value.total_tracks > 0) {
      const dlPct = Math.round((libraryStats.value.total_downloads / libraryStats.value.total_tracks) * 100)
      growthData.value = [
        { label: 'Current', total: 100, downloaded: dlPct }
      ]
    } else {
      growthData.value = []
    }
  } catch (e) {
    console.error('Failed to update snapshots for timeRange:', e)
  }
})

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
  await dashboardApi.createLibrarySnapshot().catch(() => null)
  await fetchData()
  
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
