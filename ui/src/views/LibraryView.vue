<template>
  <div class="library-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    
    <!-- Page Header with Download & Queue Status Strip (L-12) -->
    <div class="library-header px-8 pt-6 pb-2 shrink-0 flex items-center justify-between flex-wrap gap-4">
      <div>
        <div class="flex items-baseline gap-4">
          <h1 class="text-3xl font-bold tracking-tight text-ink">Library</h1>
          <span class="text-lg text-text-secondary font-medium">{{ totalTracks.toLocaleString() }} Tracks</span>
        </div>
        <p class="text-text-secondary mt-1">Your unified music collection from all services</p>
      </div>

      <!-- Status Pills, Concurrency & Queue Details (Replicated from DownloadsView.vue) -->
      <div class="flex items-center gap-2 flex-wrap">
        <!-- Active Pill -->
        <div
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold shadow-xs bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300"
          title="Active downloads"
        >
          <span class="material-symbols-outlined text-[15px] text-primary animate-spin-slow">sync</span>
          <span>Active:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.active ?? queueStats?.downloading ?? 0 }}</strong>
        </div>

        <!-- Queued Pill -->
        <div
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold shadow-xs bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300"
          title="Queued tracks"
        >
          <span class="material-symbols-outlined text-[15px] text-amber-500">schedule</span>
          <span>Queued:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.queued ?? 0 }}</strong>
        </div>

        <!-- Completed Pill -->
        <button
          @click="toggleFilter('downloaded')"
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold shadow-xs bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight cursor-pointer"
          title="Completed downloads (click to filter)"
        >
          <span class="material-symbols-outlined text-[15px] text-success">check_circle</span>
          <span>Completed:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.completed ?? 0 }}</strong>
        </button>

        <!-- Failed Pill -->
        <div
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold shadow-xs bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300"
          title="Failed downloads"
        >
          <span class="material-symbols-outlined text-[15px] text-error">cancel</span>
          <span>Failed:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ queueStats?.failed ?? 0 }}</strong>
        </div>

        <!-- Selected Pill -->
        <div
          class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs font-semibold shadow-xs bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-700 dark:text-gray-300"
          title="Selected tracks"
        >
          <span class="material-symbols-outlined text-[15px] text-primary">checklist</span>
          <span>Selected:</span>
          <strong class="font-bold text-gray-900 dark:text-white">{{ selectedCount }}</strong>
        </div>

        <div class="h-5 w-px bg-gray-200 dark:bg-border-dark mx-1 hidden sm:block"></div>

        <!-- Concurrency Selector -->
        <div class="flex items-center gap-1 bg-white dark:bg-surface-dark px-2.5 py-1 rounded-lg border border-gray-200 dark:border-border-dark shadow-xs" title="Concurrent download threads">
          <span class="material-symbols-outlined text-[16px] text-primary mr-1">bolt</span>
          <span class="text-[11px] font-bold text-text-secondary mr-1">{{ currentConcurrency }} Threads</span>
          <button
            v-for="t in [1, 2, 3, 4, 5, 6, 8, 10]"
            :key="t"
            @click="setConcurrency(t)"
            :title="`Set ${t} concurrent download thread${t > 1 ? 's' : ''}`"
            :class="[
              'w-5 h-5 rounded text-[10px] font-bold transition-all flex items-center justify-center cursor-pointer',
              currentConcurrency === t
                ? 'bg-primary text-on-accent shadow-xs'
                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-surface-highlight'
            ]"
          >
            {{ t }}
          </button>
        </div>

        <!-- Queue Details Toggle -->
        <button
          @click="showQueueDetails = !showQueueDetails"
          :class="['p-1.5 rounded-lg border transition-all cursor-pointer', showQueueDetails ? 'bg-primary/15 border-primary text-primary' : 'bg-white dark:bg-surface-dark border-gray-200 dark:border-border-dark text-gray-500 hover:text-gray-700 dark:hover:text-gray-300']"
          title="Toggle queue reconciliation and telemetry details"
        >
          <span class="material-symbols-outlined text-[18px]">tune</span>
        </button>
      </div>
    </div>

    <!-- Collapsible Queue Details Panel (Reconciliation Strip) -->
    <div v-show="showQueueDetails" class="px-8 mb-3 space-y-2 transition-all shrink-0">
      <div class="reconciliation-strip px-4 py-2 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-xs flex items-center justify-between gap-3 text-xs flex-wrap">
        <div class="flex items-center gap-1.5 font-semibold text-text-secondary shrink-0">
          <span class="material-symbols-outlined text-[16px] text-primary">analytics</span>
          <span class="uppercase tracking-wider text-[10px]">Queue Reconciliation:</span>
        </div>
        <div class="flex items-center gap-3 flex-wrap text-text-secondary text-[11px]">
          <span title="Total tracks requested by user / selection">Seleccionadas: <strong class="text-gray-900 dark:text-white">{{ queueStats?.submitted ?? selectedCount }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Initial eligible tracks in queue">Elegibles: <strong class="text-indigo-400">{{ (queueStats?.submitted ?? selectedCount) - (queueStats?.skipped ?? 0) - (queueStats?.deduplicated ?? 0) }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Excluded before queuing due to preflight filter">Excluidas Preflight: <strong class="text-amber-500">{{ (queueStats?.skipped ?? 0) + (queueStats?.deduplicated ?? 0) }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks currently in queued state">Pendientes: <strong class="text-amber-500">{{ queueStats?.queued ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks currently downloading concurrently">Activas: <strong class="text-primary">{{ queueStats?.active ?? queueStats?.downloading ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Finished downloads in queue">Completadas: <strong class="text-success">{{ queueStats?.completed ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Failed downloads in queue">Fallidas: <strong class="text-error">{{ queueStats?.failed ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Cancelled downloads in queue">Canceladas: <strong class="text-orange-400">{{ queueStats?.cancelled ?? 0 }}</strong></span>
        </div>
      </div>
    </div>

    <!-- Filter Pills Row -->
    <div class="filter-pills px-8 py-3 flex gap-2 shrink-0 overflow-x-auto custom-scrollbar">
      <button 
        v-for="filter in filterPills" 
        :key="filter.id"
        @click="toggleFilter(filter.id)"
        :class="[
          'filter-pill px-3 py-1.5 rounded-full text-xs font-semibold flex items-center gap-2 whitespace-nowrap transition-all cursor-pointer',
          activeFilters.includes(filter.id) 
            ? 'bg-primary/15 border border-primary/40 text-primary font-bold shadow-xs' 
            : 'bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark text-text-secondary hover:text-ink hover:border-gray-300 dark:hover:border-gray-600'
        ]"
      >
        <span v-if="activeFilters.includes(filter.id)" class="w-1.5 h-1.5 rounded-full bg-primary"></span>
        {{ filter.label }}
        <span 
          v-if="activeFilters.includes(filter.id) && filter.id !== 'all'" 
          @click.stop="removeFilter(filter.id)"
          class="ml-1 hover:text-error cursor-pointer"
        >×</span>
      </button>
    </div>

    <!-- Main Toolbar -->
    <div class="library-toolbar px-8 pb-3 flex items-center gap-4 shrink-0 flex-wrap">
      
      <!-- LEFT: Search -->
      <div class="relative flex-1 max-w-md min-w-[220px]">
        <span class="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 material-symbols-outlined text-[20px]">search</span>
        <input 
          v-model="searchQuery"
          type="text" 
          placeholder="Filter by title, artist, album, genre..." 
          class="w-full pl-10 pr-10 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl text-sm text-ink focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent transition-all hover:border-gray-300 dark:hover:border-gray-600"
        >
        <button 
          v-if="searchQuery" 
          @click="searchQuery = ''" 
          class="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
        >
          <span class="material-symbols-outlined text-[18px]">close</span>
        </button>
      </div>
      
      <!-- MIDDLE: View Toggle + Group By -->
      <div class="flex items-center gap-3">
        <!-- View Toggle -->
        <div class="view-toggle flex items-center bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg p-1">
          <button 
            @click="viewMode = 'list'"
            :class="['p-1.5 rounded-md transition-all', viewMode === 'list' ? 'bg-primary text-on-accent shadow-sm' : 'text-gray-400 hover:text-ink']"
            title="List View"
          >
            <span class="material-symbols-outlined text-[20px]">view_list</span>
          </button>
          <button 
            @click="viewMode = 'grid'"
            :class="['p-1.5 rounded-md transition-all', viewMode === 'grid' ? 'bg-primary text-on-accent shadow-sm' : 'text-gray-400 hover:text-ink']"
            title="Grid View"
          >
            <span class="material-symbols-outlined text-[20px]">grid_view</span>
          </button>
        </div>
        
        <!-- Group By Dropdown -->
        <div class="relative">
          <button 
            @click="showGroupDropdown = !showGroupDropdown"
            class="flex items-center gap-2 px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm font-medium text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
          >
            <span class="material-symbols-outlined text-[18px]">stacks</span>
            <span>{{ groupBy === 'none' ? 'Group' : 'By ' + (groupBy.charAt(0).toUpperCase() + groupBy.slice(1)) }}</span>
            <span class="material-symbols-outlined text-[16px] text-gray-400">expand_more</span>
          </button>
          <div v-if="showGroupDropdown" class="absolute top-full left-0 mt-1 w-40 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg shadow-xl z-20 py-1">
            <button 
              v-for="option in ['None', 'Artist', 'Album', 'Genre', 'Quality']" 
              :key="option"
              @click="groupBy = option.toLowerCase() as typeof groupBy; showGroupDropdown = false"
              :class="['w-full px-3 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors', groupBy === option.toLowerCase() ? 'text-primary font-medium' : 'text-gray-700 dark:text-gray-300']"
            >
              {{ option }}
            </button>
          </div>
        </div>
      </div>
      
      <!-- RIGHT: Actions + Sort + Bulk Actions -->
      <div class="flex items-center gap-2.5 ml-auto">
        <!-- Enrich Metadata Button -->
        <button 
          @click="enrichMetadata"
          :disabled="isEnriching"
          :class="['flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold transition-all', 
            isEnriching 
              ? 'bg-primary/10 text-primary border border-primary/30' 
              : 'bg-primary/10 text-primary hover:bg-primary/20 border border-primary/30']"
          title="Enrich metadata using MusicBrainz"
        >
          <span :class="['material-symbols-outlined text-[16px]', isEnriching && 'animate-spin']">{{ isEnriching ? 'progress_activity' : 'auto_fix_high' }}</span>
          <span v-if="!isEnriching">Enrich Metadata</span>
          <span v-else-if="enrichProgress" class="text-xs">
            {{ enrichProgress.current }}/{{ enrichProgress.total }}
          </span>
          <span v-else>Starting...</span>
        </button>

        <!-- Download Favorites Button -->
        <button 
          @click="showDownloadFavoritesModal = true"
          class="flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold bg-red-500/10 text-red-600 dark:text-red-400 hover:bg-red-500/20 border border-red-500/30 transition-all shadow-xs"
          title="Batch download favorite tracks, albums, and artists"
        >
          <span class="material-symbols-outlined text-[16px]">favorite</span>
          <span>Download Favorites</span>
        </button>

        <!-- Reset Columns Button -->
        <button 
          @click="resetColumnsOrder"
          class="p-2 text-gray-400 hover:text-ink hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-all"
          title="Reset Columns Order"
        >
          <span class="material-symbols-outlined text-[18px]">view_column</span>
        </button>

        <!-- Keyboard Shortcuts Help -->
        <button 
          @click="showShortcutsModal = true"
          class="p-2 text-gray-400 hover:text-ink hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-all"
          title="Keyboard Shortcuts"
        >
          <span class="material-symbols-outlined text-[18px]">keyboard</span>
        </button>

        <!-- Sort Dropdown -->
        <div class="sort-dropdown relative">
          <button 
            @click="showSortDropdown = !showSortDropdown"
            class="flex items-center gap-2 px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-xs font-semibold text-gray-700 dark:text-gray-200 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors"
          >
            <span class="material-symbols-outlined text-[16px]">sort</span>
            <span>{{ sortLabel }}</span>
            <span :class="['material-symbols-outlined text-[16px] text-gray-400 transition-transform', sortDirection === 'desc' ? '' : 'rotate-180']">arrow_downward</span>
          </button>
          <div v-if="showSortDropdown" class="absolute top-full right-0 mt-1 w-48 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg shadow-xl z-20 py-1">
            <button 
              v-for="option in sortOptions" 
              :key="option.value"
              @click="sortBy = option.value; showSortDropdown = false"
              :class="['w-full px-3 py-2 text-left text-xs hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center justify-between', sortBy === option.value ? 'text-primary font-medium' : 'text-gray-700 dark:text-gray-300']"
            >
              {{ option.label }}
              <span v-if="sortBy === option.value" class="material-symbols-outlined text-[14px]">check</span>
            </button>
          </div>
        </div>
        
        <!-- Bulk Actions -->
        <div v-if="selectedCount > 0" class="flex items-center gap-2">
          <button 
            @click="showBulkMenu = !showBulkMenu"
            class="flex items-center gap-2 px-3 py-2 bg-primary text-on-accent rounded-lg text-xs font-semibold shadow-lg shadow-primary/20 hover:bg-primary-hover transition-colors relative"
          >
            <span class="material-symbols-outlined text-[16px]">checklist</span>
            <span>{{ selectedCount }} selected</span>
            <span class="material-symbols-outlined text-[14px]">expand_more</span>
          </button>
          <div v-if="showBulkMenu" class="absolute top-full right-0 mt-1 w-48 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg shadow-xl z-20 py-1">
            <button @click="downloadSelectedTracks" class="w-full px-3 py-2 text-left text-xs hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-2 text-gray-700 dark:text-gray-300">
              <span class="material-symbols-outlined text-[16px]">download</span> Download
            </button>
            <button @click="downloadSelectedTracks" class="w-full px-3 py-2 text-left text-xs hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-2 text-gray-700 dark:text-gray-300">
              <span class="material-symbols-outlined text-[16px]">download</span> Add to Downloads
            </button>
            <hr class="my-1 border-gray-200 dark:border-border-dark">
            <button @click="handleBulkRemove" class="w-full px-3 py-2 text-left text-xs hover:bg-error/10 transition-colors flex items-center gap-2 text-error">
              <span class="material-symbols-outlined text-[16px]">delete</span> Remove
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Active Filters Bar -->
    <div v-if="hasRemovableFilters || searchQuery" class="active-filters px-8 pb-2 flex items-center gap-2 shrink-0 flex-wrap">
      <span class="text-xs text-text-secondary">Active filters:</span>
      <div class="flex items-center gap-2 flex-wrap">
        <span 
          v-for="filterId in activeFilters.filter(f => f !== 'all')" 
          :key="filterId"
          class="px-2 py-0.5 rounded-md bg-primary/10 text-primary text-xs font-medium flex items-center gap-1"
        >
          {{ getFilterLabel(filterId) }}
          <button @click="removeFilter(filterId)" class="hover:text-error cursor-pointer">×</button>
        </span>
        <span 
          v-if="searchQuery"
          class="px-2 py-0.5 rounded-md bg-primary/10 text-primary text-xs font-medium flex items-center gap-1"
        >
          Search: "{{ searchQuery }}"
          <button @click="searchQuery = ''" class="hover:text-error cursor-pointer">×</button>
        </span>
      </div>
      <button @click="clearAllFilters" class="ml-auto text-xs text-text-secondary hover:text-primary transition-colors cursor-pointer">
        Clear all filters
      </button>
    </div>

    <!-- Batch Selection Bar (floating overlay - L-11) -->
    <Transition name="slide-down">
      <div
        v-if="selectedCount > 0"
        class="batch-bar fixed bottom-20 inset-x-8 max-w-xl mx-auto z-40 flex items-center gap-4 px-6 py-3.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl shadow-2xl"
      >
        <span class="text-accent font-bold text-sm whitespace-nowrap">{{ selectedCount }} track{{ selectedCount !== 1 ? 's' : '' }} selected</span>
        <div class="flex-1 flex items-center justify-center gap-3 flex-wrap">
          <button
            @click="downloadSelectedTracks"
            :disabled="isEnqueuingSelection"
            :aria-busy="isEnqueuingSelection"
            class="px-3.5 py-1.5 bg-primary hover:bg-primary-hover text-on-accent rounded-lg text-xs font-semibold shadow-md transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-60 disabled:cursor-wait"
          >
            <span class="material-symbols-outlined text-[16px]">{{ isEnqueuingSelection ? 'progress_activity' : 'download' }}</span>
            {{ isEnqueuingSelection ? 'Enqueuing…' : 'Download' }}
          </button>
          <button @click="handleBulkRemove" class="px-3.5 py-1.5 bg-transparent hover:bg-error/20 text-error border border-error/50 rounded-lg text-xs font-semibold transition-all flex items-center gap-1.5 cursor-pointer">
            <span class="material-symbols-outlined text-[16px]">delete</span>
            Remove
          </button>
        </div>
        <button @click="clearSelection" class="text-ink-2 hover:text-ink text-xs transition-colors cursor-pointer whitespace-nowrap">
          Clear Selection
        </button>
      </div>
    </Transition>

    <!-- Context Menu (Viewport Clamped) -->
    <Teleport to="body">
      <Transition name="fade">
        <div 
          v-if="contextMenu.visible" 
          class="context-menu fixed z-50 w-56 bg-elevated border border-line rounded-xl shadow-2xl py-1 overflow-hidden"
          :style="{ top: contextMenu.y + 'px', left: contextMenu.x + 'px' }"
          @click.stop
        >
          <!-- Play actions -->
          <button @click="handlePlayNow(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">play_arrow</span>
            <span class="flex-1 text-left">Play Now</span>
            <span class="text-[10px] text-gray-500">Space</span>
          </button>
          <button @click="handlePlayNext(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">queue_play_next</span>
            <span class="flex-1 text-left">Play Next</span>
            <span class="text-[10px] text-gray-500">N</span>
          </button>
          <button @click="handleAddToPlayQueue(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">playlist_add</span>
            <span class="flex-1 text-left">Add to Play Queue</span>
          </button>
          <button @click="handleAddToQueue(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">download</span>
            <span class="flex-1 text-left">Add to Downloads</span>
            <span class="text-[10px] text-gray-500">Q</span>
          </button>
          
          <div class="menu-separator h-px bg-line my-1"></div>
          
          <!-- Download submenu -->
          <div class="menu-item-submenu relative group">
            <button class="w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
              <span class="material-symbols-outlined text-[16px]">download</span>
              <span class="flex-1 text-left">Download</span>
              <span class="text-[10px] text-gray-500 mr-1">D</span>
              <span class="material-symbols-outlined text-[14px] text-gray-400">chevron_right</span>
            </button>
            <div class="absolute left-full top-0 ml-1 w-48 bg-elevated border border-line rounded-xl shadow-2xl py-1 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50">
              <button @click="handleDownloadBestQuality(contextMenu.track!)" class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors">Best Quality</button>
              <button v-if="contextMenu.track?.services.includes('Qobuz')" @click="handleDownloadFromService(contextMenu.track!, 'qobuz')" class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors">From Qobuz (24/96)</button>
              <!-- TIDAL-QUAL: pick the tier instead of being locked to one. Tidal
                   serves Hi-Res FLAC (24-bit), CD FLAC (16/44.1) and AAC 320; a
                   single hardcoded label meant either a needless downgrade or a
                   silent lossy fallback. -->
              <div v-if="contextMenu.track?.services.includes('Tidal')" class="menu-item-submenu relative group">
                <button class="w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
                  <span class="flex-1 text-left">From Tidal</span>
                  <span class="material-symbols-outlined text-[14px] text-gray-400">chevron_right</span>
                </button>
                <div class="absolute left-full top-0 ml-1 w-48 bg-elevated border border-line rounded-xl shadow-2xl py-1 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50">
                  <button @click="handleDownloadFromService(contextMenu.track!, 'tidal', 'HI_RES_LOSSLESS')" class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors">Hi-Res FLAC (24-bit)</button>
                  <button @click="handleDownloadFromService(contextMenu.track!, 'tidal', 'LOSSLESS')" class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors">FLAC (16/44.1)</button>
                  <button @click="handleDownloadFromService(contextMenu.track!, 'tidal', 'HIGH')" class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors">AAC 320 (m4a)</button>
                </div>
              </div>
              <button v-if="contextMenu.track?.services.includes('Deezer')" @click="handleDownloadFromService(contextMenu.track!, 'deezer')" class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors">From Deezer (FLAC)</button>
            </div>
          </div>
          
          <div class="menu-separator h-px bg-line my-1"></div>
          
          <!-- Playlist actions -->
          <div class="menu-item-submenu relative group">
            <button class="w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
              <span class="material-symbols-outlined text-[16px]">playlist_add</span>
              <span class="flex-1 text-left">Add to Playlist</span>
              <span class="material-symbols-outlined text-[14px] text-gray-400">chevron_right</span>
            </button>
            <div class="absolute left-full top-0 ml-1 w-48 bg-elevated border border-line rounded-xl shadow-2xl py-1 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50">
              <button class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors flex items-center gap-2" @click="libraryApi.createPlaylist(1, 'New Playlist'); loadPlaylists()">
                <span class="material-symbols-outlined text-[14px]">add</span> New Playlist...
              </button>
              <div class="h-px bg-line my-1"></div>
              <button 
                v-for="playlist in playlists" 
                :key="playlist.id" 
                class="w-full px-4 py-2 text-left text-xs text-gray-200 hover:bg-accent-soft transition-colors"
                @click="addTrackToPlaylist(playlist.id, contextMenu.track?.id ?? 0)"
              >📋 {{ playlist.name }}</button>
              <p v-if="!playlists || playlists.length === 0" class="px-4 py-2 text-xs text-gray-400 italic">No playlists yet</p>
            </div>
          </div>

          <!-- Favorite -->
          <button @click="handleToggleFavorite(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span :class="['material-symbols-outlined text-[16px]', contextMenu.track?.isFavorite ? 'text-red-500 material-symbols-filled' : '']">{{ contextMenu.track?.isFavorite ? 'favorite' : 'favorite_border' }}</span>
            <span class="flex-1 text-left">{{ contextMenu.track?.isFavorite ? 'Remove from Favorites' : 'Add to Favorites' }}</span>
            <span class="text-[10px] text-gray-500">F</span>
          </button>
          
          <div class="menu-separator h-px bg-line my-1"></div>
          
          <!-- External links -->
          <button v-if="contextMenu.track?.services.includes('Spotify')" @click="handleViewOnSpotify(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="w-3.5 h-3.5 rounded-full bg-[#1ed760] flex items-center justify-center text-[8px] font-bold text-black">S</span>
            <span class="flex-1 text-left">View on Spotify</span>
            <span class="material-symbols-outlined text-[14px] text-gray-400">open_in_new</span>
          </button>
          <button v-if="contextMenu.track?.services.includes('Qobuz')" @click="handleViewOnQobuz(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="w-3.5 h-3.5 rounded-full bg-[#1a8fe3] flex items-center justify-center text-[8px] font-bold text-white">Q</span>
            <span class="flex-1 text-left">View on Qobuz</span>
            <span class="material-symbols-outlined text-[14px] text-gray-400">open_in_new</span>
          </button>
          
          <div class="menu-separator h-px bg-line my-1"></div>
          
          <!-- Metadata actions -->
          <button @click="handleShowMetadata(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">info</span>
            <span class="flex-1 text-left">Show Metadata</span>
          </button>
          <button v-if="contextMenu.track?.downloadStatus === 'downloaded'" @click="handleShowInFolder(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">folder</span>
            <span class="flex-1 text-left">Show in Folder</span>
          </button>
          
          <div class="menu-separator h-px bg-line my-1"></div>
          
          <!-- Remove -->
          <button @click="handleRemoveFromLibrary(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-error hover:bg-error/20 transition-colors">
            <span class="material-symbols-outlined text-[16px]">delete</span>
            <span class="flex-1 text-left">Remove from Library</span>
          </button>
        </div>
      </Transition>
    </Teleport>

    <!-- Keyboard Shortcuts Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showShortcutsModal" class="shortcut-modal fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm" @click="showShortcutsModal = false">
          <div class="bg-elevated border border-line rounded-2xl shadow-2xl w-full max-w-md overflow-hidden" @click.stop>
            <div class="flex items-center justify-between px-6 py-4 border-b border-line">
              <h3 class="text-lg font-bold text-ink flex items-center gap-2">
                <span class="material-symbols-outlined">keyboard</span>
                Keyboard Shortcuts
              </h3>
              <button @click="showShortcutsModal = false" class="text-gray-400 hover:text-ink transition-colors">
                <span class="material-symbols-outlined">close</span>
              </button>
            </div>
            <div class="p-6 space-y-4 max-h-[60vh] overflow-y-auto custom-scrollbar">
              <div class="space-y-3">
                <h4 class="text-xs font-bold text-gray-400 uppercase tracking-wider">Playback</h4>
                <div class="flex items-center justify-between text-sm">
                  <span class="text-gray-300">Play / Pause</span>
                  <kbd class="px-2 py-1 bg-base border border-line rounded text-xs text-gray-300 font-mono">Space</kbd>
                </div>
                <div class="flex items-center justify-between text-sm">
                  <span class="text-gray-300">Play Next</span>
                  <kbd class="px-2 py-1 bg-base border border-line rounded text-xs text-gray-300 font-mono">N</kbd>
                </div>
              </div>
              <div class="space-y-3">
                <h4 class="text-xs font-bold text-gray-400 uppercase tracking-wider">Library</h4>
                <div class="flex items-center justify-between text-sm">
                  <span class="text-gray-300">Add to Downloads</span>
                  <kbd class="px-2 py-1 bg-base border border-line rounded text-xs text-gray-300 font-mono">Q</kbd>
                </div>
                <div class="flex items-center justify-between text-sm">
                  <span class="text-gray-300">Download Track</span>
                  <kbd class="px-2 py-1 bg-base border border-line rounded text-xs text-gray-300 font-mono">D</kbd>
                </div>
                <div class="flex items-center justify-between text-sm">
                  <span class="text-gray-300">Toggle Favorite</span>
                  <kbd class="px-2 py-1 bg-base border border-line rounded text-xs text-gray-300 font-mono">F</kbd>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Content Area -->
    <div class="flex-1 overflow-hidden px-8 pb-8 flex flex-col">
      
<!-- ERROR STATE: un fallo de carga no es una biblioteca vacía -->
      <div v-if="loadError && !isLoading" class="library-empty flex-1 flex flex-col items-center justify-center text-center py-16" data-testid="library-load-error">
        <span class="material-symbols-outlined text-[80px] text-error mb-6">error</span>
        <h3 class="text-2xl font-bold text-ink mb-2">Could not load your library</h3>
        <p class="text-text-secondary mb-2 max-w-md">Your tracks are still there — the app could not read them just now.</p>
        <p class="text-xs text-text-secondary mb-8 max-w-md break-words">{{ loadError }}</p>
        <button
          @click="loadLibrary"
          class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-on-accent rounded-xl font-medium shadow-lg shadow-primary/20 transition-all cursor-pointer flex items-center gap-2"
        >
          <span class="material-symbols-outlined text-[18px]">refresh</span>
          Try Again
        </button>
      </div>

      <!-- EMPTY STATE: Syncing in Progress -->
      <div v-else-if="tracks.length === 0 && !isLoading && hasSyncingTask" class="library-empty flex-1 flex flex-col items-center justify-center text-center py-16">
        <span class="material-symbols-outlined text-[80px] text-primary mb-6 animate-spin">sync</span>
        <h3 class="text-2xl font-bold text-ink mb-2">Syncing your music collection...</h3>
        <p class="text-text-secondary mb-8 max-w-md">Importing albums, playlists, and favorites from your connected services. Your tracks and albums will appear here automatically when sync completes.</p>
        <button
          @click="loadLibrary"
          class="px-5 py-2.5 bg-primary/10 hover:bg-primary/20 text-primary border border-primary/30 rounded-xl font-medium transition-all flex items-center gap-2 cursor-pointer"
        >
          <span class="material-symbols-outlined text-[18px]">refresh</span>
          Refresh Library
        </button>
      </div>

      <!-- EMPTY STATE: No Tracks -->
      <div v-else-if="tracks.length === 0 && !isLoading" class="library-empty flex-1 flex flex-col items-center justify-center text-center py-16">
        <span class="material-symbols-outlined text-[80px] text-gray-400 dark:text-gray-600 mb-6">library_music</span>
        <h3 class="text-2xl font-bold text-ink mb-2">Your library is empty</h3>
        <p class="text-text-secondary mb-8 max-w-md">Import music from streaming services or scan local files to get started</p>
        <div class="flex gap-4">
          <button 
            @click="router.push('/accounts')"
            class="px-6 py-3 bg-primary hover:bg-primary-hover text-on-accent rounded-xl font-medium shadow-lg shadow-primary/20 transition-all cursor-pointer"
          >
            Connect Services
          </button>
          <button 
            @click="router.push('/accounts')"
            class="px-6 py-3 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-xl font-medium transition-all cursor-pointer"
          >
            Scan Local Files
          </button>
        </div>
      </div>

      <!-- EMPTY STATE: No Filter Results -->
      <div v-else-if="filteredTracks.length === 0 && !isSearching" class="library-empty flex-1 flex flex-col items-center justify-center text-center py-16">
        <span class="material-symbols-outlined text-[60px] text-gray-400 dark:text-gray-600 mb-6">filter_list_off</span>
        <h3 class="text-xl font-bold text-ink mb-2">No tracks match your filters</h3>
        <p class="text-text-secondary mb-6">Try adjusting your search or filters</p>
        <button @click="clearAllFilters" class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-xl font-medium transition-all cursor-pointer">
          Clear All Filters
        </button>
      </div>

      <!-- SEARCHING STATE -->
      <div v-else-if="isSearching && searchQuery" class="library-empty flex-1 flex flex-col items-center justify-center text-center py-16">
        <span class="material-symbols-outlined text-[60px] text-primary mb-6 animate-spin">progress_activity</span>
        <h3 class="text-xl font-bold text-ink mb-2">Searching...</h3>
        <p class="text-text-secondary">Searching your entire library for "{{ searchQuery }}"</p>
      </div>

      <!-- LIST VIEW (Customizable Columns with Drag & Drop) -->
      <template v-else-if="viewMode === 'list' && groupBy === 'none'">
        <!-- Header Row -->
        <div class="track-list flex items-center gap-2 px-3 py-3 border-b border-gray-200 dark:border-border-dark text-[11px] font-bold text-text-secondary uppercase tracking-wider shrink-0 bg-shell/50 rounded-t-xl backdrop-blur-sm sticky top-0 z-10 select-none">
          <!-- Selection Checkbox -->
          <div class="w-10 text-center shrink-0">
            <input type="checkbox" @change="toggleSelectAll" class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-gray-300 dark:border-gray-600">
          </div>

          <!-- Dynamic Draggable Columns: # | Title | Time | Services | Meta | Lyrics | DL | Quality -->
          <template v-for="(col, colIdx) in columns" :key="col.id">
            <div 
              draggable="true"
              @dragstart="onColDragStart(colIdx, $event)"
              @dragover="onColDragOver(colIdx, $event)"
              @drop="onColDrop(colIdx, $event)"
              @dragend="onColDragEnd"
              :class="[
                'column-header flex items-center gap-1 cursor-grab active:cursor-grabbing hover:text-ink transition-all',
                col.widthClass,
                col.hideBreakpoint || '',
                col.align === 'center' ? 'justify-center text-center' : col.align === 'right' ? 'justify-end text-right' : 'justify-start text-left',
                dragOverCol === colIdx ? 'border-b-2 border-primary text-primary font-extrabold' : '',
                draggedCol === colIdx ? 'opacity-40' : ''
              ]"
              :title="`Drag to reorder column: ${col.label}`"
            >
              <span class="truncate">{{ col.label }}</span>
            </div>
          </template>

          <!-- Actions Header spacer -->
          <div class="w-20 shrink-0"></div>
        </div>

        <!-- Scrollable List (Virtual Scroller for 23,000+ Items - PR-02) -->
        <div
          ref="listContainerRef"
          class="flex-1 min-h-0 overflow-y-auto custom-scrollbar border-x border-b border-gray-200 dark:border-border-dark rounded-b-xl bg-white dark:bg-surface-dark relative"
          style="contain: strict;"
          @scroll="handleScroll"
        >
          <!-- Virtual scroll spacer -->
          <div :style="{ height: totalVirtualHeight + 'px', position: 'relative', width: '100%' }">
            <!-- Rendered visible items window -->
            <div :style="{ transform: `translateY(${virtualOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
              <div
                v-for="(track, index) in visibleTracks"
                :key="track.id"
                :data-track-id="track.id"
                @click="handleTrackClick(track)"
                @dblclick="handleTrackPlay(track)"
                @contextmenu.prevent="openContextMenu($event, track)"
                :style="{ height: ROW_HEIGHT + 'px' }"
                :class="[
                  'track-row flex items-center gap-2 px-3 py-2 border-b border-gray-100 dark:border-border-dark/50 last:border-0 transition-all group cursor-pointer box-border',
                  track.isPlaying ? 'bg-primary/5 border-l-4 border-l-primary' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight/30',
                  track.isSelected ? 'bg-accent-soft' : '',
                  (virtualStartIndex + index) % 2 === 1 ? 'bg-surface/30' : '',
                  highlightedTrackId === track.id ? 'bg-primary/10 ring-1 ring-inset ring-primary' : ''
                ]"
              >
                <!-- Checkbox Cell -->
                <div class="track-cell w-10 flex justify-center shrink-0">
                  <input type="checkbox" v-model="track.isSelected" @click.stop class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-gray-300 dark:border-gray-600">
                </div>

                <!-- Dynamic Cells based on Columns Order -->
                <template v-for="col in columns" :key="col.id">
                  <!-- # (Index) -->
                  <!-- S194: hover play button REMOVED per owner directive. Playback
                       requires the audio-backend sprint (investigation verdict in
                       S194): no native audio crate, asset protocol disabled,
                       no provider stream commands exist yet. -->
                  <div v-if="col.id === 'index'" :class="['track-cell w-10 text-center shrink-0 text-xs text-text-secondary', col.hideBreakpoint || '']">
                    <span v-if="track.isPlaying" class="material-symbols-filled text-[20px] text-primary animate-pulse">equalizer</span>
                    <span v-else>{{ virtualStartIndex + index + 1 }}</span>
                  </div>

              <!-- Title (Artwork + Title + Artist + Album) -->
              <div v-else-if="col.id === 'title'" :class="['track-cell flex-1 min-w-[200px] min-w-0 flex items-center gap-3', col.hideBreakpoint || '']">
                <TrackCover
                  :src="track.coverUrl"
                  :item-id="track.id"
                  :alt="track.album || track.title"
                  size-class="h-10 w-10 rounded-md"
                />
                <div class="flex flex-col gap-0.5 overflow-hidden min-w-0">
                  <span class="font-medium text-ink truncate text-sm">{{ track.title }}</span>
                  <div class="flex items-center gap-1.5 text-xs text-text-secondary truncate">
                    <span class="truncate">{{ track.artist }} · {{ track.album }}</span>
                    <span v-if="track.bpm" class="px-1.5 py-0.2 text-[9px] font-bold rounded bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20 shrink-0" :title="'BPM: ' + Math.round(track.bpm)">{{ Math.round(track.bpm) }} BPM</span>
                  </div>
                </div>
              </div>

              <!-- Time -->
              <div v-else-if="col.id === 'time'" :class="['track-cell w-14 text-right shrink-0 text-xs text-text-secondary font-mono', col.hideBreakpoint || '']">
                {{ track.duration }}
              </div>

              <!-- Services -->
              <div v-else-if="col.id === 'services'" :class="['track-cell service-icons w-28 shrink-0 flex justify-center items-center gap-1', col.hideBreakpoint || '']" :title="track.availabilitySummary ? 'Availability: ' + track.availabilitySummary : (track.availableServices && track.availableServices.length > 0 ? 'Verified available on: ' + track.availableServices.join(', ') : 'Unverified / Unchecked')">
                <template v-if="track.availableServices && track.availableServices.length > 0">
                  <span v-for="service in track.availableServices.slice(0, 3)" :key="service" :class="['w-5 h-5 rounded-full flex items-center justify-center text-[10px] font-bold border border-green-500/50', getServiceStyle(service)]">{{ getServiceIcon(service) }}</span>
                  <span v-if="track.availableServices.length > 3" class="text-[10px] text-text-secondary font-medium">+{{ track.availableServices.length - 3 }}</span>
                </template>
                <template v-else>
                  <span class="text-[10px] text-gray-400 italic px-1.5 py-0.5 rounded bg-gray-100 dark:bg-surface-highlight">Unchecked</span>
                </template>
              </div>

              <!-- Meta -->
              <div v-else-if="col.id === 'meta'" :class="['track-cell w-14 text-center shrink-0 flex items-center justify-center', col.hideBreakpoint || '']">
                <div :class="['w-7 h-7 rounded-full flex items-center justify-center text-[10px] font-bold', getMetadataScoreStyle(track.metadataScore)]">{{ track.metadataScore }}</div>
              </div>

              <!-- Lyrics -->
              <div v-else-if="col.id === 'lyrics'" :class="['track-cell w-14 text-center shrink-0 flex items-center justify-center', col.hideBreakpoint || '']">
                <span :class="['material-symbols-outlined text-[16px]', getLyricsTypeIcon(track.lyricsType).class]" :title="getLyricsTypeIcon(track.lyricsType).title">{{ getLyricsTypeIcon(track.lyricsType).icon }}</span>
              </div>

              <!-- DL (Effective Provider or Status) -->
              <div v-else-if="col.id === 'dl'" :class="['track-cell w-14 text-center shrink-0 flex items-center justify-center', col.hideBreakpoint || '']">
                <template v-if="track.downloadStatus === 'downloaded'">
                  <span 
                    v-if="track.downloadedFrom" 
                    :class="['w-5 h-5 rounded-full flex items-center justify-center text-[10px] font-bold border shadow-xs', getServiceStyle(track.downloadedFrom)]"
                    :title="getEffectiveDownloadTooltip(track.downloadedFrom)"
                  >
                    {{ getServiceIcon(track.downloadedFrom) }}
                  </span>
                  <span 
                    v-else 
                    class="material-symbols-outlined text-[18px] text-success" 
                    title="Downloaded"
                  >
                    check_circle
                  </span>
                </template>
                <template v-else-if="track.downloadStatus === 'downloading' || track.downloadStatus === 'queued'">
                  <span class="material-symbols-outlined text-[18px] text-primary animate-spin" title="Downloading...">progress_activity</span>
                </template>
                <template v-else-if="track.downloadStatus === 'failed'">
                  <span class="material-symbols-outlined text-[18px] text-error" title="Download failed">error</span>
                </template>
                <template v-else-if="track.downloadStatus === 'stale'">
                  <span class="material-symbols-outlined text-[18px] text-warning" title="File stale or missing">warning</span>
                </template>
                <template v-else>
                  <span class="text-xs text-gray-400 font-mono" title="Not downloaded">—</span>
                </template>
              </div>

              <!-- Quality (ONLY IF DOWNLOADED / LOCAL FILE EXISTS) -->
              <div v-else-if="col.id === 'quality'" :class="['track-cell w-20 text-center shrink-0 flex items-center justify-center', col.hideBreakpoint || '']">
                <span 
                  v-if="track.downloadStatus === 'downloaded' && track.quality && track.quality !== '—'" 
                  :class="['text-[11px] font-medium tracking-wide px-2 py-0.5 rounded-full border', getQualityStyle(track.quality)]"
                >
                  {{ track.quality }}
                </span>
                <span v-else class="text-xs text-gray-400 font-mono" title="Not downloaded">—</span>
              </div>
            </template>

            <!-- Actions Cell -->
            <!-- S194: fixed row actions (always visible, dimmed when idle) —
                 the hover-only floating buttons rendered unreliably. -->
            <div class="track-cell track-actions w-20 shrink-0 flex justify-end items-center gap-1">
              <button
                @click.stop="handleToggleFavorite(track)"
                :class="[
                  'p-1 transition-all rounded hover:bg-gray-100 dark:hover:bg-surface-highlight',
                  track.isFavorite
                    ? 'text-red-500 hover:text-red-600'
                    : 'text-gray-400 opacity-60 hover:text-red-400 hover:opacity-100'
                ]"
                :title="track.isFavorite ? 'Remove from favorites' : 'Add to favorites'"
              >
                <span :class="['material-symbols-outlined text-[18px]', track.isFavorite ? 'material-symbols-filled text-red-500' : '']">
                  {{ track.isFavorite ? 'favorite' : 'favorite_border' }}
                </span>
              </button>
              <button v-if="track.downloadStatus !== 'downloaded'" @click.stop="handleDownload(track)" class="p-1 text-gray-400 opacity-60 hover:text-primary hover:opacity-100 transition-all" title="Download"><span class="material-symbols-outlined text-[18px]">download</span></button>
              <button @click.stop="openContextMenu($event, track)" class="p-1 text-gray-400 opacity-60 hover:text-ink hover:opacity-100 transition-all" title="More options"><span class="material-symbols-outlined text-[18px]">more_vert</span></button>
            </div>
          </div>
          
              <!-- Loading More Indicator -->
              <div v-if="isLoadingMore" class="py-4 flex items-center justify-center gap-2 text-text-secondary">
                <span class="material-symbols-outlined text-[20px] animate-spin">progress_activity</span>
                <span class="text-sm">Loading more tracks...</span>
              </div>

              <!-- End of List Indicator -->
              <div v-else-if="!hasMore && tracks.length > 0 && virtualEndIndex >= filteredTracks.length" class="py-4 text-center text-sm text-text-secondary">
                Showing all {{ totalTracks.toLocaleString() }} tracks
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- GRID VIEW -->
      <template v-else-if="viewMode === 'grid' && groupBy === 'none'">
        <div class="library-grid flex-1 overflow-y-auto custom-scrollbar" @scroll="handleScroll">
          <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 2xl:grid-cols-6 gap-5 p-1">
            <div 
              v-for="album in groupedByAlbum" 
              :key="album.id"
              @click="handleAlbumClick(album.albumId || album.id)"
              :class="[
                'grid-tile group bg-white dark:bg-surface-dark rounded-xl overflow-hidden shadow-md hover:shadow-xl transition-all duration-300 cursor-pointer hover:-translate-y-1',
                album.isSelected ? 'ring-3 ring-primary' : ''
              ]"
            >
              <!-- Image Area -->
              <div class="tile-image relative aspect-square overflow-hidden bg-gray-200 dark:bg-gray-800">
                <img v-if="album.coverUrl" :src="album.coverUrl" :alt="album.title" class="absolute inset-0 w-full h-full object-cover transition-transform duration-500 group-hover:scale-105" loading="lazy" />
                <div v-else :class="['absolute inset-0 w-full h-full', album.artGradient]"></div>
                
                <!-- Grid Placeholder Icon -->
                <div v-if="!album.coverUrl && !album.artGradient.includes('bg-')" class="absolute inset-0 flex items-center justify-center opacity-30">
                  <span class="material-symbols-outlined text-[48px]">album</span>
                </div>
                
                <!-- Top-left: Download status -->
                <div class="absolute top-2 left-2 z-10">
                  <span v-if="album.downloadStatus === 'downloaded'" class="w-6 h-6 rounded-full bg-success/90 text-white flex items-center justify-center shadow-lg">
                    <span class="material-symbols-outlined text-[14px]">check</span>
                  </span>
                  <span v-else class="w-6 h-6 rounded-full bg-black/50 text-white flex items-center justify-center">
                    <span class="material-symbols-outlined text-[14px]">cloud</span>
                  </span>
                </div>
                
                <!-- Top-right: Quality badge -->
                <div class="absolute top-2 right-2">
                  <span :class="['px-1.5 py-0.5 rounded text-[9px] font-bold shadow-lg', getQualityBadgeStyle(album.quality)]">
                    {{ album.quality }}
                  </span>
                </div>
                
                <!-- Hover Overlay -->
                <div class="tile-overlay absolute inset-0 bg-black/0 group-hover:bg-black/50 transition-all duration-300 flex flex-col items-center justify-center opacity-0 group-hover:opacity-100">
                  <button @click.stop="handleAlbumPlay(album)" class="w-14 h-14 rounded-full bg-primary text-on-accent flex items-center justify-center shadow-xl transform scale-75 group-hover:scale-100 transition-transform duration-300" title="Play album">
                    <span class="material-symbols-outlined text-[28px]">play_arrow</span>
                  </button>
                  <span class="absolute bottom-3 text-white text-xs font-medium">{{ album.trackCount }} track{{ album.trackCount !== 1 ? 's' : '' }}</span>
                </div>
              </div>
              
              <!-- Info Area -->
              <div class="tile-info p-3 space-y-1">
                <h4 class="text-sm font-semibold text-ink line-clamp-2 leading-tight">{{ album.title }}</h4>
                <p class="text-xs text-text-secondary truncate">{{ album.artist }}</p>
                <div class="flex items-center justify-between pt-1">
                  <div class="flex items-center gap-1">
                    <span v-for="service in album.services.slice(0, 3)" :key="service" :class="['w-4 h-4 rounded-full flex items-center justify-center text-[8px] font-bold', getServiceStyle(service)]">
                      {{ getServiceIcon(service) }}
                    </span>
                  </div>
                  <span :class="['w-2.5 h-2.5 rounded-full', album.metadataScore >= 90 ? 'bg-success' : album.metadataScore >= 70 ? 'bg-warning' : 'bg-error']"></span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- GROUPED VIEW: By Artist -->
      <template v-else-if="groupBy === 'artist'">
        <div class="flex-1 overflow-y-auto custom-scrollbar space-y-6" @scroll="handleScroll">
          <div v-for="artistGroup in groupedByArtist" :key="artistGroup.artist" class="group-section">
            <!-- Group Header -->
            <div 
              @click="toggleGroupExpand(artistGroup.artist)"
              class="group-header sticky top-0 z-10 flex items-center gap-4 px-4 py-3 bg-surface border-b border-gray-700 dark:border-border-dark cursor-pointer hover:bg-elevated transition-colors rounded-t-lg"
            >
              <div class="w-12 h-12 rounded-full bg-gradient-to-br from-primary to-purple-500 flex items-center justify-center text-white font-bold text-lg shrink-0">
                {{ artistGroup.artist.charAt(0) }}
              </div>
              <div class="flex-1 min-w-0">
                <h3 class="text-lg font-bold text-ink truncate">{{ artistGroup.artist }}</h3>
                <p class="text-xs text-text-secondary">{{ artistGroup.tracks.length }} tracks</p>
              </div>
              <span :class="['material-symbols-outlined text-gray-400 transition-transform', expandedGroups.includes(artistGroup.artist) ? 'rotate-180' : '']">expand_more</span>
            </div>
            
            <!-- Group Tracks -->
            <div v-if="expandedGroups.includes(artistGroup.artist)" class="group-tracks bg-white dark:bg-surface-dark border-x border-b border-gray-200 dark:border-border-dark rounded-b-lg overflow-hidden">
              <div
                v-for="(track, idx) in artistGroup.tracks"
                :key="track.id"
                :data-track-id="track.id"
                :class="highlightedTrackId === track.id ? 'bg-primary/10' : ''"
                class="flex items-center gap-3 px-4 py-2.5 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 cursor-pointer group"
                @click="handleTrackClick(track)"
                @contextmenu.prevent="openContextMenu($event, track)"
              >
                <span class="w-6 text-center text-xs text-gray-400 group-hover:text-primary font-medium">{{ idx + 1 }}</span>
                <TrackCover
                  :src="track.coverUrl"
                  :item-id="track.id"
                  :alt="track.album || track.title"
                  size-class="w-10 h-10"
                  rounded="rounded"
                />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-ink truncate">{{ track.title }}</p>
                  <p class="text-xs text-text-secondary truncate">{{ track.album }}</p>
                </div>
                <span v-if="track.downloadStatus === 'downloaded'" class="text-success"><span class="material-symbols-outlined text-[16px]">check_circle</span></span>
                <span class="text-xs text-text-secondary font-mono">{{ track.duration }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- GROUPED VIEW: By Album -->
      <template v-else-if="groupBy === 'album'">
        <div class="flex-1 overflow-y-auto custom-scrollbar space-y-6" @scroll="handleScroll">
          <div v-for="albumGroup in groupedByAlbum" :key="albumGroup.id" class="group-section">
            <!-- Group Header -->
            <div 
              @click="toggleGroupExpand(albumGroup.title)"
              class="group-header sticky top-0 z-10 flex items-center gap-4 px-4 py-3 bg-surface border-b border-gray-700 dark:border-border-dark cursor-pointer hover:bg-elevated transition-colors rounded-t-lg"
            >
              <div :class="['w-12 h-12 rounded-lg shrink-0 overflow-hidden', !albumGroup.coverUrl && albumGroup.artGradient]">
                <img v-if="albumGroup.coverUrl" :src="albumGroup.coverUrl" :alt="albumGroup.title" class="w-full h-full object-cover" loading="lazy" />
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <h3 class="text-base font-bold text-ink truncate">{{ albumGroup.title }}</h3>
                  <button 
                    v-if="albumGroup.albumId"
                    @click.stop="handleAlbumClick(albumGroup.albumId)"
                    class="px-2 py-0.5 rounded bg-primary/20 hover:bg-primary/30 text-primary text-xs font-medium flex items-center gap-1 transition-colors"
                    title="View Album Details"
                  >
                    <span>View Album</span>
                    <span class="material-symbols-outlined text-[14px]">arrow_forward</span>
                  </button>
                </div>
                <p class="text-sm text-text-secondary">{{ albumGroup.artist }} · {{ albumGroup.trackCount }} track{{ albumGroup.trackCount !== 1 ? 's' : '' }}</p>
              </div>
              <span :class="['px-2 py-1 rounded text-xs font-bold', getQualityBadgeStyle(albumGroup.quality)]">{{ albumGroup.quality }}</span>
              <span :class="['material-symbols-outlined text-gray-400 transition-transform', expandedGroups.includes(albumGroup.title) ? 'rotate-180' : '']">expand_more</span>
            </div>
            
            <!-- Group Tracks -->
            <div v-if="expandedGroups.includes(albumGroup.title)" class="group-tracks bg-white dark:bg-surface-dark border-x border-b border-gray-200 dark:border-border-dark rounded-b-lg overflow-hidden">
              <div
                v-for="(track, idx) in albumGroup.tracks"
                :key="track.id"
                :data-track-id="track.id"
                :class="highlightedTrackId === track.id ? 'bg-primary/10' : ''"
                class="flex items-center gap-3 px-4 py-2.5 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 cursor-pointer group"
                @click="handleTrackClick(track)"
                @contextmenu.prevent="openContextMenu($event, track)"
              >
                <span class="w-6 text-center text-xs text-gray-400 group-hover:text-primary font-medium">{{ idx + 1 }}</span>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-ink truncate">{{ track.title }}</p>
                </div>
                <div class="flex items-center gap-1">
                  <span v-if="track.downloadStatus === 'downloaded'" class="text-success"><span class="material-symbols-outlined text-[16px]">check_circle</span></span>
                </div>
                <span class="text-xs text-text-secondary font-mono">{{ track.duration }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- GROUPED VIEW: By Genre (Real Metadata Grouping) -->
      <template v-else-if="groupBy === 'genre'">
        <div class="flex-1 overflow-y-auto custom-scrollbar space-y-6" @scroll="handleScroll">
          <div v-for="genreGroup in groupedByGenre" :key="genreGroup.genre" class="group-section">
            <!-- Group Header -->
            <div 
              @click="toggleGroupExpand(genreGroup.genre)"
              class="group-header sticky top-0 z-10 flex items-center gap-4 px-4 py-3 bg-surface border-b border-gray-700 dark:border-border-dark cursor-pointer hover:bg-elevated transition-colors rounded-t-lg"
            >
              <div class="w-12 h-12 rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-white font-bold shrink-0">
                <span class="material-symbols-outlined text-[22px]">category</span>
              </div>
              <div class="flex-1 min-w-0">
                <h3 class="text-lg font-bold text-ink truncate">{{ genreGroup.genre }}</h3>
                <p class="text-xs text-text-secondary">{{ genreGroup.tracks.length }} track{{ genreGroup.tracks.length !== 1 ? 's' : '' }}</p>
              </div>
              <span :class="['material-symbols-outlined text-gray-400 transition-transform', expandedGroups.includes(genreGroup.genre) ? 'rotate-180' : '']">expand_more</span>
            </div>
            
            <!-- Group Tracks -->
            <div v-if="expandedGroups.includes(genreGroup.genre)" class="group-tracks bg-white dark:bg-surface-dark border-x border-b border-gray-200 dark:border-border-dark rounded-b-lg overflow-hidden">
              <div
                v-for="(track, idx) in genreGroup.tracks"
                :key="track.id"
                :data-track-id="track.id"
                :class="highlightedTrackId === track.id ? 'bg-primary/10' : ''"
                class="flex items-center gap-3 px-4 py-2.5 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 cursor-pointer group"
                @click="handleTrackClick(track)"
                @contextmenu.prevent="openContextMenu($event, track)"
              >
                <span class="w-6 text-center text-xs text-gray-400 group-hover:text-primary font-medium">{{ idx + 1 }}</span>
                <TrackCover
                  :src="track.coverUrl"
                  :item-id="track.id"
                  :alt="track.album || track.title"
                  size-class="w-10 h-10"
                  rounded="rounded"
                />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-ink truncate">{{ track.title }}</p>
                  <p class="text-xs text-text-secondary truncate">{{ track.artist }} · {{ track.album }}</p>
                </div>
                <div class="flex items-center gap-1">
                  <span v-if="track.downloadStatus === 'downloaded'" class="text-success"><span class="material-symbols-outlined text-[16px]">check_circle</span></span>
                </div>
                <span class="text-xs text-text-secondary font-mono">{{ track.duration }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- GROUPED VIEW: By Quality -->
      <template v-else-if="groupBy === 'quality'">
        <div class="flex-1 overflow-y-auto custom-scrollbar space-y-6" @scroll="handleScroll">
          <div v-for="qualityGroup in groupedByQuality" :key="qualityGroup.quality" class="group-section">
            <!-- Group Header -->
            <div 
              @click="toggleGroupExpand(qualityGroup.quality)"
              class="group-header sticky top-0 z-10 flex items-center gap-4 px-4 py-3 bg-surface border-b border-gray-700 dark:border-border-dark cursor-pointer hover:bg-elevated transition-colors rounded-t-lg"
            >
              <div :class="['w-12 h-12 rounded-full flex items-center justify-center text-white font-bold text-[10px] shrink-0', getQualityStyle(qualityGroup.quality)]">
                {{ qualityGroup.quality }}
              </div>
              <div class="flex-1 min-w-0">
                <h3 class="text-lg font-bold text-ink truncate">{{ qualityGroup.quality }}</h3>
                <p class="text-xs text-text-secondary">{{ qualityGroup.tracks.length }} tracks</p>
              </div>
              <span :class="['material-symbols-outlined text-gray-400 transition-transform', expandedGroups.includes(qualityGroup.quality) ? 'rotate-180' : '']">expand_more</span>
            </div>
            
            <!-- Group Tracks -->
            <div v-if="expandedGroups.includes(qualityGroup.quality)" class="group-tracks bg-white dark:bg-surface-dark border-x border-b border-gray-200 dark:border-border-dark rounded-b-lg overflow-hidden">
              <div
                v-for="(track, idx) in qualityGroup.tracks"
                :key="track.id"
                :data-track-id="track.id"
                :class="highlightedTrackId === track.id ? 'bg-primary/10' : ''"
                class="flex items-center gap-3 px-4 py-2.5 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 cursor-pointer group"
                @click="handleTrackClick(track)"
                @contextmenu.prevent="openContextMenu($event, track)"
              >
                <span class="w-6 text-center text-xs text-gray-400 group-hover:text-primary font-medium">{{ idx + 1 }}</span>
                <TrackCover
                  :src="track.coverUrl"
                  :item-id="track.id"
                  :alt="track.album || track.title"
                  size-class="w-10 h-10"
                  rounded="rounded"
                />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-ink truncate">{{ track.title }}</p>
                  <p class="text-xs text-text-secondary truncate">{{ track.artist }} · {{ track.album }}</p>
                </div>
                <span class="text-xs text-text-secondary font-mono">{{ track.duration }}</span>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Download Favorites Modal -->
      <DownloadFavoritesModal 
        v-model="showDownloadFavoritesModal" 
        @enqueued="handleFavoritesEnqueued" 
      />

    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import { useRouter, useRoute, type LocationQueryValue } from 'vue-router'
import { libraryApi, searchTracks, enqueueTracks, reconcileQueue, type DownloadFavoritesResult } from '@/api/library'
import { addToQueue, addBatchToQueue, enqueueEligibleBatch, queueApi } from '@/api/queue'
import { playlistsApi } from '@/api/playlists'
import { settingsApi } from '@/api/settings'
import type { LibraryTrack, Playlist, LyricsType, QueueStats, WorkerStatus, EffectiveDownloadPreferences } from '@/api/types'
import { useToast } from '@/composables/useToast'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { useGlobalTasks } from '@/composables/useGlobalTasks'
import { usePlayer } from '@/composables/usePlayer'
import DownloadFavoritesModal from '@/components/DownloadFavoritesModal.vue'
import TrackCover from '@/components/TrackCover.vue'
import { getArtGradient } from '@/utils/artGradient'
import { formatServiceName } from '@/composables/useGlobalTasks';

const router = useRouter()
const route = useRoute()
const toast = useToast()

// TIDAL-QUAL: the download decision (top tier to ask for, and whether Tidal may
// answer with a worse one) belongs to Settings, not to this view. `null` means
// "not loaded yet", which falls back to the historical behaviour so a failed
// read never blocks downloading.
let effectivePrefsPromise: Promise<EffectiveDownloadPreferences | null> | null = null
const effectivePrefs = ref<EffectiveDownloadPreferences | null>(null)

function loadEffectivePrefs(): Promise<EffectiveDownloadPreferences | null> {
  if (effectivePrefsPromise) return effectivePrefsPromise
  effectivePrefsPromise = settingsApi
    .getEffectiveDownloadPreferences()
    .then((prefs) => {
      effectivePrefs.value = prefs
      return prefs
    })
    .catch(() => null)
  return effectivePrefsPromise
}

/** The quality ceiling the user configured, in the queue's vocabulary. */
function effectiveMaxQualityLabel(): string {
  switch (effectivePrefs.value?.maxQuality) {
    case 'high':
      return 'high'
    case 'lossless':
      return 'lossless'
    default:
      // 'hires', 'any'/'max', or "settings unavailable": ask for the top tier
      // and let the provider serve the best the account entitles.
      return 'hires'
  }
}

/** False when the user chose to skip a track rather than accept a downgrade. */
function isDowngradeAllowed(): boolean {
  return effectivePrefs.value?.strictQuality === false
}
const eventBus = useEventBus()
const { activeTasks } = useGlobalTasks()

const hasSyncingTask = computed(() => {
  return activeTasks.value.some(t => t.type === 'sync' || t.type === 'import')
})

const showDownloadFavoritesModal = ref(false)

function handleFavoritesEnqueued(res: DownloadFavoritesResult) {
  toast.success('Favorites Enqueued', res.message)
}

// Navigation to detail views (Sprint 4)
function handleAlbumClick(albumId: number | null | undefined) {
  if (albumId) router.push({ name: 'AlbumDetail', params: { id: albumId.toString() } })
}

function handleArtistClick(artistId: number | null | undefined) {
  if (artistId) router.push({ name: 'ArtistDetail', params: { id: artistId.toString() } })
}

// FE-9: the grid tile's play affordance was decorative — the click only bubbled
// up to the album navigation. Play the album's first track instead, and stop
// the event so the tile does not also navigate.
function handleAlbumPlay(album: { tracks: Track[] }) {
  const first = album.tracks[0]
  if (first) {
    void handleTrackPlay(first)
  }
}

// Fetch real data from backend
const isLoading = ref(true)
/** Fallo de la última carga; distingue «biblioteca vacía» de «no se pudo leer». */
const loadError = ref<string | null>(null)

// State - will be updated from backend
const trackCount = ref(0)
const searchQuery = ref('')
const searchResults = ref<any[]>([])
const isSearching = ref(false)
const searchOffset = ref(0)
const searchTotal = ref(0)
const hasMoreSearch = ref(false)
const currentFtsQuery = ref('')
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null
const SEARCH_PAGE_SIZE = 100

const sortBy = ref('dateAdded')
const sortDirection = ref<'asc' | 'desc'>('desc')
const selectedCount = ref(0)

// Metadata enrichment
const isEnriching = ref(false)
const enrichProgress = ref<{ current: number; total: number; currentTrack: string } | null>(null)

/** Evento MusicBrainz de `enrich_metadata_musicbrainz`. */
interface MusicBrainzProgress {
  status?: string
  total: number
  current: number
  enriched?: number
  failed?: number
  currentTrack?: string
}

/**
 * `enrichment-progress` es un canal compartido. MusicBrainz emite
 * `{ status, total, current, enriched, failed, currentTrack }` sin campo
 * `type`; Last.fm emite `{ type: "lastfm_genre", status, current, total,
 * message }` donde `current` cuenta aciertos, y el enriquecimiento incremental
 * emite su `EnrichmentJobSummary` en camelCase (`totalTracks`,
 * `processedTracks`). Ninguno de esos dos describe el trabajo que este botón
 * lanzó, así que se ignoran en vez de pintar una barra con `undefined`.
 */
function asMusicBrainzProgress(payload: unknown): MusicBrainzProgress | null {
  if (!payload || typeof payload !== 'object') return null
  const candidate = payload as Record<string, unknown>
  // MusicBrainz nunca etiqueta el evento; el resto de productores siempre lo hacen.
  if ('type' in candidate) return null
  if (typeof candidate.current !== 'number' || typeof candidate.total !== 'number') return null
  if (!Number.isFinite(candidate.current) || !Number.isFinite(candidate.total)) return null
  return {
    status: typeof candidate.status === 'string' ? candidate.status : undefined,
    current: candidate.current,
    total: candidate.total,
    enriched: typeof candidate.enriched === 'number' ? candidate.enriched : undefined,
    failed: typeof candidate.failed === 'number' ? candidate.failed : undefined,
    currentTrack: typeof candidate.currentTrack === 'string' ? candidate.currentTrack : '',
  }
}

async function enrichMetadata() {
  if (isEnriching.value) return

  isEnriching.value = true
  enrichProgress.value = { current: 0, total: 0, currentTrack: '' }

  const { listen } = await import('@tauri-apps/api/event')
  const unlisten = await listen<unknown>(TauriEvents.ENRICHMENT_PROGRESS, (event) => {
    const progress = asMusicBrainzProgress(event.payload)
    if (!progress) return
    enrichProgress.value = {
      current: progress.current,
      total: progress.total,
      currentTrack: progress.currentTrack ?? ''
    }
  })
  
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const result = await invoke<{ total: number; enriched: number; failed: number }>('enrich_metadata_musicbrainz', {})
    
    if (result.enriched > 0) {
      await loadLibrary()
    }
  } catch (error) {
    console.error('Failed to enrich metadata:', error)
  } finally {
    unlisten()
    isEnriching.value = false
    enrichProgress.value = null
  }
}

// View State
const viewMode = ref<'list' | 'grid'>('list')
const groupBy = ref<'none' | 'artist' | 'album' | 'year' | 'genre' | 'quality'>('none')

// Dropdowns
const showGroupDropdown = ref(false)
const showSortDropdown = ref(false)
const showBulkMenu = ref(false)

// Keyboard shortcuts modal
const showShortcutsModal = ref(false)

// Context menu state
const contextMenu = ref<{
  visible: boolean
  x: number
  y: number
  track: Track | null
}>({
  visible: false,
  x: 0,
  y: 0,
  track: null
})

// Filter pills (Consolidated single source of filters)
const filterPills = [
  { id: 'all', label: 'All Items' },
  { id: 'downloaded', label: 'Downloaded' },
  { id: 'notDownloaded', label: 'Not Downloaded' },
  { id: 'favorites', label: 'Favorites' },
  { id: 'flac', label: 'Hi-Res (FLAC)' },
  { id: 'mp3', label: 'Lossy (MP3)' },
  { id: 'multiSource', label: 'Multi-Source' },
  { id: 'duplicates', label: 'Duplicates' },
]

const activeFilters = ref(['all'])

// FE-10: the deep-link 'quality' filter is not a pill, so the pills row cannot
// show it as selected — and any single active filter was hidden from the bar
// too. Keep the Active Filters bar visible whenever something removable is
// applied so every filter (pills and deep-links alike) can be cleared.
const hasRemovableFilters = computed(() => activeFilters.value.some(f => f !== 'all'))

// FE-10: dashboard deep-link /library?filter=quality&quality=<bucket label>.
// The bucket label comes from get_audio_quality_distribution ('Hi-Res
// (24-bit+)', 'CD Quality', 'Lossy') and is matched against the
// quality_bucket the backend computes per track.
const qualityFilter = ref('')

const sortOptions = [
  { value: 'title', label: 'Title' },
  { value: 'artist', label: 'Artist' },
  { value: 'album', label: 'Album' },
  { value: 'dateAdded', label: 'Date Added' },
  { value: 'quality', label: 'Quality' },
  { value: 'metadataScore', label: 'Metadata Score' },
  { value: 'duration', label: 'Duration' },
]

const sortLabel = computed(() => {
  return sortOptions.find(o => o.value === sortBy.value)?.label || 'Date Added'
})

// ==============================================
// TABLE COLUMN DEFINITIONS & DRAG AND DROP
// ==============================================
type ColumnId = 'index' | 'title' | 'time' | 'services' | 'meta' | 'lyrics' | 'dl' | 'quality'

interface ColumnDef {
  id: ColumnId
  label: string
  widthClass: string
  align?: 'left' | 'center' | 'right'
  hideBreakpoint?: string
}

const DEFAULT_COLUMNS: ColumnDef[] = [
  { id: 'index', label: '#', widthClass: 'w-10', align: 'center' },
  { id: 'title', label: 'Title', widthClass: 'flex-1 min-w-[200px]', align: 'left' },
  { id: 'time', label: 'Time', widthClass: 'w-14', align: 'right' },
  { id: 'services', label: 'Services', widthClass: 'w-28', align: 'center', hideBreakpoint: 'hidden xl:flex' },
  { id: 'meta', label: 'Meta', widthClass: 'w-14', align: 'center', hideBreakpoint: 'hidden xl:flex' },
  { id: 'lyrics', label: 'Lyrics', widthClass: 'w-14', align: 'center', hideBreakpoint: 'hidden xl:flex' },
  { id: 'dl', label: 'DL', widthClass: 'w-14', align: 'center', hideBreakpoint: 'hidden lg:flex' },
  { id: 'quality', label: 'Quality', widthClass: 'w-20', align: 'center', hideBreakpoint: 'hidden lg:flex' },
]

const columns = ref<ColumnDef[]>([...DEFAULT_COLUMNS])
const draggedCol = ref<number | null>(null)
const dragOverCol = ref<number | null>(null)

function onColDragStart(index: number, e: DragEvent) {
  draggedCol.value = index
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('text/plain', String(index))
  }
}

function onColDragOver(index: number, e: DragEvent) {
  e.preventDefault()
  if (draggedCol.value !== null && draggedCol.value !== index) {
    dragOverCol.value = index
  }
}

function onColDrop(targetIndex: number, e: DragEvent) {
  if (e && typeof e.preventDefault === 'function') {
    e.preventDefault()
  }
  let sourceIndex = draggedCol.value
  if ((sourceIndex === null || isNaN(sourceIndex)) && e && e.dataTransfer) {
    try {
      const raw = e.dataTransfer.getData('text/plain') || e.dataTransfer.getData('text') || ''
      if (raw !== '') {
        const parsed = parseInt(raw, 10)
        if (!isNaN(parsed)) {
          sourceIndex = parsed
        }
      }
    } catch {
      // ignore
    }
  }

  if (sourceIndex === null || isNaN(sourceIndex) || sourceIndex === targetIndex || sourceIndex < 0 || sourceIndex >= columns.value.length) {
    draggedCol.value = null
    dragOverCol.value = null
    return
  }
  const item = columns.value.splice(sourceIndex, 1)[0]
  columns.value.splice(targetIndex, 0, item)
  draggedCol.value = null
  dragOverCol.value = null
  saveColumnsOrder()
}

function onColDragEnd() {
  draggedCol.value = null
  dragOverCol.value = null
}

function getStorage(): Storage | null {
  try {
    if (typeof window !== 'undefined' && window.localStorage && typeof window.localStorage.getItem === 'function') {
      return window.localStorage
    }
  } catch {
    // ignore
  }
  return null
}

function saveColumnsOrder() {
  try {
    const storage = getStorage()
    if (!storage) return
    const ids = columns.value.map(c => c.id)
    storage.setItem('syncify_library_columns_order', JSON.stringify(ids))
  } catch (err) {
    console.warn('Failed to save column order:', err)
  }
}

function loadColumnsOrder() {
  try {
    const storage = getStorage()
    if (!storage) {
      columns.value = [...DEFAULT_COLUMNS]
      return
    }
    const raw = storage.getItem('syncify_library_columns_order')
    if (raw) {
      const ids: unknown[] = JSON.parse(raw)
      // L-06: purge removed column ids (e.g. 'special') persisted before the
      // column was retired, so stale local state cannot resurrect it.
      const sanitizedIds = (Array.isArray(ids) ? ids : [])
        .filter((id): id is ColumnId => DEFAULT_COLUMNS.some(c => c.id === id))

      const mapped = sanitizedIds
        .map(id => DEFAULT_COLUMNS.find(c => c.id === id))
        .filter((c): c is ColumnDef => !!c)
      const missing = DEFAULT_COLUMNS.filter(c => !sanitizedIds.includes(c.id))
      columns.value = [...mapped, ...missing]
      return
    }
  } catch (err) {
    console.warn('Failed to load column order:', err)
  }
  columns.value = [...DEFAULT_COLUMNS]
}

function resetColumnsOrder() {
  columns.value = [...DEFAULT_COLUMNS]
  saveColumnsOrder()
  toast.info('Columns reset to default')
}

// Track type interface
interface Track {
  id: number
  title: string
  artist: string
  album: string
  albumId?: number | null
  artistId?: number | null
  coverUrl: string | null
  artGradient: string
  services: string[]
  importedFrom: string | null
  downloadedFrom: string | null
  availableServices: string[]
  availabilitySummary: string | null
  quality: string
  qualityBucket?: string | null
  downloadStatus: 'downloaded' | 'queued' | 'not_downloaded'
  metadataScore?: number
  lyricsType: LyricsType
  spotifyTrackId?: string | null
  genre?: string | null
  filePath?: string | null
  displayTitle?: string | null
  sourceTitle?: string | null
  fileDisambiguator?: string | null
  duration: string
  isFavorite: boolean
  isPlaying: boolean
  isSelected: boolean
}

// Mock tracks data
const tracks = ref<Track[]>([])

// Convert LibraryTrack to UI Track
function mapToTrack(item: LibraryTrack, index: number): Track {
  const durationSec = (item.duration_ms ?? 0) / 1000;
  const mins = Math.floor(durationSec / 60);
  const secs = Math.floor(durationSec % 60);
  
  // Parse services from comma-separated string
  const servicesList = item.services 
    ? item.services.split(',').map(s => s.trim()).filter(Boolean)
    : [];

  const availableList = item.available_services
    ? item.available_services.split(',').map(s => s.trim()).filter(Boolean)
    : [];
  
  const artDisplay = item.cover_art_url || getArtGradient(item.id);
  
  return {
    id: item.id,
    title: item.display_title || item.title,
    artist: item.artist_name || 'Unknown Artist',
    album: item.album_name || 'Unknown Album',
    albumId: item.album_id ?? null,
    artistId: item.artist_id ?? null,
    coverUrl: item.cover_art_url || null,
    artGradient: artDisplay,
    services: servicesList,
    importedFrom: item.imported_from || null,
    downloadedFrom: item.downloaded_from || null,
    availableServices: availableList,
    availabilitySummary: item.availability_summary || null,
    quality: item.quality || '—',
    qualityBucket: item.quality_bucket ?? null,
    downloadStatus: (item.download_status ?? 'not_downloaded') as Track['downloadStatus'],
    metadataScore: item.metadata_score ?? 0,
    lyricsType: (item.lyrics_type ?? 'none') as Track['lyricsType'],
    spotifyTrackId: item.spotify_track_id ?? null,
    genre: item.genre || null,
    filePath: item.file_path || null,
    displayTitle: item.display_title || null,
    sourceTitle: item.source_title || item.title,
    fileDisambiguator: item.file_disambiguator || null,
    duration: `${mins}:${secs.toString().padStart(2, '0')}`,
    isFavorite: !!(item as any).is_favorite,
    isPlaying: false,
    isSelected: false,
  };
}

// Pagination state
const totalTracks = ref(0);
const currentOffset = ref(0);
const hasMore = ref(false);
const isLoadingMore = ref(false);
const PAGE_SIZE = 100;

// Load tracks from backend
async function loadLibrary() {
  isLoading.value = true;
  loadError.value = null;
  currentOffset.value = 0;
  try {
    const page = activeFilters.value.includes('duplicates')
      ? await libraryApi.getDuplicateTracks(0, PAGE_SIZE)
      : activeFilters.value.includes('favorites')
      ? await libraryApi.getFavoriteTracks(0, PAGE_SIZE)
      : await libraryApi.getLibrary(0, PAGE_SIZE);

    tracks.value = page.tracks.map(mapToTrack);
    trackCount.value = tracks.value.length;
    totalTracks.value = page.total;
    hasMore.value = page.has_more;
    currentOffset.value = page.offset + page.tracks.length;
  } catch (error) {
    // Sin marcarlo, un fallo de carga se renderiza como biblioteca vacía.
    loadError.value = String(error);
    console.error('Failed to load library:', error);
  } finally {
    isLoading.value = false;
  }
}

// Load more tracks (infinite scroll)
async function loadMore() {
  if (isLoadingMore.value || !hasMore.value) return;
  
  isLoadingMore.value = true;
  try {
    const page = activeFilters.value.includes('duplicates')
      ? await libraryApi.getDuplicateTracks(currentOffset.value, PAGE_SIZE)
      : activeFilters.value.includes('favorites')
      ? await libraryApi.getFavoriteTracks(currentOffset.value, PAGE_SIZE)
      : await libraryApi.getLibrary(currentOffset.value, PAGE_SIZE);
      
    const newTracks = page.tracks.map(mapToTrack);
    tracks.value.push(...newTracks);
    pageAppendCount.value++;
    trackCount.value = tracks.value.length;
    hasMore.value = page.has_more;
    currentOffset.value = page.offset + page.tracks.length;
  } catch (error) {
    console.error('Failed to load more tracks:', error);
  } finally {
    isLoadingMore.value = false;
  }
}

// ==============================================
// VIRTUAL SCROLLING LOGIC FOR LIBRARY (23,000+ ROWS)
// Ported from DownloadsView.vue:549-564, 1768-1807 (PR-02)
// ==============================================
const ROW_HEIGHT = 56 // px per table row (40px cover + py-2 padding + 1px border)
const OVERSCAN = 10   // extra rows above and below visible area

const listContainerRef = ref<HTMLElement | null>(null)
const libraryScrollTop = ref(0)
const libraryViewportHeight = ref(800)

// ==============================================
// DOWNLOAD QUEUE STATS & CONCURRENCY (L-12)
// Replicated from DownloadsView.vue:1420-1454, 1870-1903
// ==============================================
const queueStats = ref<QueueStats>({ total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 })
const workerStatus = ref<WorkerStatus>({ running: true, paused: false, active_downloads: 0, max_concurrent: 3 })
const showQueueDetails = ref(false)

const currentConcurrency = computed(() => {
  return workerStatus.value?.max_concurrent ?? 3
})

async function setConcurrency(threads: number) {
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('set_max_concurrency', { maxConcurrent: threads })
    workerStatus.value.max_concurrent = threads
    toast.success('Concurrency Updated', `Max concurrent downloads set to ${threads}`)
  } catch (err) {
    console.warn('Failed to set worker live concurrency:', err)
  }
}

async function fetchQueueStats() {
  try {
    const [stats, worker] = await Promise.all([
      queueApi.getQueueStats().catch(() => null),
      queueApi.getWorkerStatus().catch(() => null),
    ])
    if (stats) queueStats.value = stats
    if (worker) workerStatus.value = worker
  } catch {
    // Non-blocking in Library
  }
}

// Infinite scroll fires a burst of scroll events per flick (B-4).
//
// PR-01 / PR-02: Reading layout properties (scrollHeight/scrollTop/clientHeight)
// BEFORE the throttle cut forces synchronous reflow on every single event frame.
// We update libraryScrollTop without layout reflow, and move layout reading
// strictly inside the throttled dispatch.
let lastScrollCheck = 0;
const SCROLL_THROTTLE_MS = 120;
let scrollThrottleTimer: ReturnType<typeof setTimeout> | null = null;
function handleScroll(event: Event) {
  const target = event.target as HTMLElement;
  if (!target) return;

  // Immediate reactive update for virtual window slicing (no reflow)
  libraryScrollTop.value = target.scrollTop;
  if (target.clientHeight && libraryViewportHeight.value !== target.clientHeight) {
    libraryViewportHeight.value = target.clientHeight;
  }

  // Throttle heavy layout measurements (scrollHeight/scrollTop/clientHeight)
  // PR-01 / PR-02: layout reading is STRICTLY inside the throttle cut
  const checkAndDispatchLoadMore = (el: HTMLElement) => {
    lastScrollCheck = Date.now();
    const scrollBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    if (scrollBottom < 300) {
      if (searchQuery.value.trim()) {
        if (hasMoreSearch.value && !isSearching.value) {
          loadMoreSearchResults();
        }
      } else {
        if (hasMore.value && !isLoadingMore.value) {
          loadMore();
        }
      }
    }
  };

  const elapsed = Date.now() - lastScrollCheck;
  if (elapsed >= SCROLL_THROTTLE_MS) {
    checkAndDispatchLoadMore(target);
    return;
  }
  if (!scrollThrottleTimer) {
    scrollThrottleTimer = setTimeout(() => {
      scrollThrottleTimer = null;
      const el = listContainerRef.value || target;
      if (el) {
        checkAndDispatchLoadMore(el);
      }
    }, SCROLL_THROTTLE_MS - elapsed);
  }
}

// Initialize data
loadColumnsOrder();
loadLibrary();

// Database search
async function performDatabaseSearch(query: string) {
  if (!query.trim()) {
    searchResults.value = []
    searchOffset.value = 0
    searchTotal.value = 0
    hasMoreSearch.value = false
    currentFtsQuery.value = ''
    isSearching.value = false
    return
  }

  isSearching.value = true
  try {
    const ftsQuery = query
      .trim()
      .split(/\s+/)
      .map(w => w.replace(/[^\w]/g, ''))
      .filter(w => w.length > 0)
      .map(w => w + '*')
      .join(' ')
    
    if (!ftsQuery) {
      searchResults.value = []
      isSearching.value = false
      return
    }
    
    searchOffset.value = 0
    currentFtsQuery.value = ftsQuery
    
    const result = await searchTracks(ftsQuery, 0, SEARCH_PAGE_SIZE)
    searchResults.value = result.tracks.map(mapToTrack)
    searchTotal.value = result.total
    hasMoreSearch.value = result.has_more
    searchOffset.value = result.offset + result.tracks.length
  } catch (error) {
    console.error('Database search failed:', error)
    searchResults.value = []
  } finally {
    isSearching.value = false
  }
}

async function loadMoreSearchResults() {
  if (isSearching.value || !hasMoreSearch.value || !currentFtsQuery.value) return
  
  isSearching.value = true
  try {
    const result = await searchTracks(currentFtsQuery.value, searchOffset.value, SEARCH_PAGE_SIZE)
    const newTracks = result.tracks.map(mapToTrack)
    searchResults.value = [...searchResults.value, ...newTracks]
    hasMoreSearch.value = result.has_more
    searchOffset.value = result.offset + result.tracks.length
  } catch (error) {
    console.error('Failed to load more search results:', error)
  } finally {
    isSearching.value = false
  }
}

watch(searchQuery, (newQuery) => {
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
  
  if (!newQuery.trim()) {
    searchResults.value = []
    searchOffset.value = 0
    searchTotal.value = 0
    hasMoreSearch.value = false
    currentFtsQuery.value = ''
    isSearching.value = false
    return
  }
  
  isSearching.value = true
  searchDebounceTimer = setTimeout(() => {
    performDatabaseSearch(newQuery)
  }, 300)
})

// Playlists
const playlists = ref<Playlist[]>([]);

async function loadPlaylists() {
  try {
    playlists.value = (await libraryApi.getPlaylists()) || [];
  } catch (error) {
    console.error('Failed to load playlists:', error);
    playlists.value = [];
  }
}

loadPlaylists();

async function addTrackToPlaylist(playlistId: number, trackId: number) {
  try {
    const track = tracks.value.find(t => t.id === trackId);
    await libraryApi.addToPlaylist(playlistId, [trackId]);
    closeContextMenu();
    await reconcilePlaylistCounts();
    // L-02: la pista sí se añadía, pero el usuario no recibía ninguna señal.
    toast.success('Added to playlist', `"${track?.title ?? 'Track'}" added`);
  } catch (error) {
    console.error('Failed to add to playlist:', error);
    toast.error('Failed to add to playlist', String(error));
  }
}

// Track selection & downloads
// L-05: el contador era O(n) por clic (filter sobre toda la biblioteca). Con
// 23k pistas eso se notaba al hacer clics seguidos; ahora es incremental.
function handleTrackClick(track: Track) {
  track.isSelected = !track.isSelected;
  selectedCount.value += track.isSelected ? 1 : -1;
}

const { play, playNext, enqueue, toggle: togglePlayback, current: playerCurrent } = usePlayer();

// S194 residual: double-click plays the LOCAL downloaded file through the
// syncify-media protocol. Tracks without a local file fail with the backend's
// honest error: player.error lo muestra el reproductor de la barra de estado mientras hay pista
// en curso, y sin current (barra oculta) estos catches lo revelan como toast;
// provider streaming is out of scope.
async function handleTrackPlay(track: Track) {
  try {
    await play({
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: track.album ?? null,
      coverUrl: track.coverUrl ?? null,
    });
  } catch (error: any) {
    console.error('Failed to play track:', error);
    surfacePlayFailure(track, error);
  }
}

// FE-9: context-menu playback actions wired to the real player.
function handlePlayNow(track: Track) {
  closeContextMenu();
  void handleTrackPlay(track);
}

async function handlePlayNext(track: Track) {
  closeContextMenu();
  try {
    const outcome = await playNext({
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: track.album ?? null,
      coverUrl: track.coverUrl ?? null,
    });
    if (outcome === 'started') {
      toast.success(`Now playing "${track.title}"`);
    } else {
      toast.success('Playing Next', `"${track.title}" will play next`);
    }
  } catch (error: any) {
    console.error('Failed to play next:', error);
    surfacePlayFailure(track, error);
  }
}

// Fase 1: "Add to Play Queue" añade la pista al FINAL de la cola de
// reproducción (playNext inserta al frente); con el player parado la arranca.
async function handleAddToPlayQueue(track: Track) {
  closeContextMenu();
  try {
    const outcome = await enqueue({
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: track.album ?? null,
      coverUrl: track.coverUrl ?? null,
    });
    if (outcome === 'started') {
      toast.success(`Now playing "${track.title}"`);
    } else {
      toast.success('Added to Play Queue', `"${track.title}" sonará al final de la cola`);
    }
  } catch (error: any) {
    console.error('Failed to add to play queue:', error);
    surfacePlayFailure(track, error);
  }
}

/**
 * Un fallo de arranque solo toca player.error: sin pista en curso la barra
 * no se renderiza y el mensaje no llegaría al usuario (antes estos catch
 * eran silenciosos). Con current la barra ya enseña el error; sin ella, toast.
 */
function surfacePlayFailure(track: Track, error: unknown) {
  // L-01: el fallo se silenciaba cuando habia algo sonando, asi que el usuario
  // no sabia por que no cambio de pista. Ahora se muestra siempre con la
  // causa real y el paso a seguir.
  const message = String((error as { message?: string })?.message || error || 'Error desconocido');
  const hasLocalFile = !!track.filePath;
  const hint = hasLocalFile
    ? ''
    : ' Esta pista no esta descargada: la reproduccion local necesita el archivo en disco.';
  toast.error(`No se pudo reproducir "${track.title}"`, message + hint);
}

// "Add to Downloads" enqueues the track into the download queue (QueueView /
// DownloadsView), same as the D shortcut. Etiqueta renombrada en fase 1 para
// desambiguarla de la cola de reproducción.
async function handleAddToQueue(track: Track) {
  closeContextMenu();
  await handleDownload(track);
}

async function handleDownload(track: Track) {
  try {
    await loadEffectivePrefs()
    await addToQueue({
      trackId: track.id,
      targetTitle: track.title,
      targetArtist: track.artist,
      targetAlbum: track.album,
      qualityPreference: effectiveMaxQualityLabel(),
      // TIDAL-QUAL: "best possible quality" means asking for the top tier and
      // NOT letting Tidal silently hand back AAC in its place. The UI used to
      // pin allowFallback:true, which inverted the download setting: even with
      // "skip on downgrade" configured, every Tidal download came back m4a.
      allowFallback: isDowngradeAllowed(),
    });
    track.downloadStatus = 'queued';
    toast.success(`Enqueued "${track.title}" for download`);
  } catch (error: any) {
    console.error('Failed to queue download:', error);
    const errStr = String(error?.message || error || '');
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', `Track "${track.title}" has no available provider source.`);
    } else {
      toast.error(`Failed to enqueue: ${errStr}`);
    }
  }
}

const isEnqueuingSelection = ref(false)

async function downloadSelectedTracks() {
  if (isEnqueuingSelection.value) return
  const selectedTracks = tracks.value.filter(t => t.isSelected)
  if (selectedTracks.length === 0) return

  isEnqueuingSelection.value = true
  try {
    await loadEffectivePrefs()
    const trackIds = selectedTracks.map(t => t.id)
    const allowDowngrade = isDowngradeAllowed()
    const res = await enqueueTracks(
      trackIds,
      50,
      effectiveMaxQualityLabel(),
      undefined,
      !allowDowngrade,
      allowDowngrade,
      true,
      true
    )

    const selected = res?.selected ?? trackIds.length
    const eligible = res?.eligible ?? 0
    const enqueued = res?.enqueued ?? 0
    const excluded = res?.excluded_preflight ?? 0
    const reasons = res?.skip_reasons ?? []

    const enqueuedTracks = Array.isArray(res?.tracks) ? res.tracks : []
    const enqueuedSet = new Set(
      enqueuedTracks
        .filter((t: any) => t?.is_eligible)
        .map((t: any) => t?.track_id)
    )

    selectedTracks.forEach(t => {
      if (enqueuedSet.has(t.id) || (enqueuedSet.size === 0 && enqueued > 0)) {
        t.downloadStatus = 'queued'
      }
    })

    if (enqueued > 0) {
      const breakdownMsg = excluded > 0 && reasons.length > 0
        ? ` (${excluded} excluded: ${reasons[0]})`
        : excluded > 0
        ? ` (${excluded} excluded)`
        : ''
      toast.success(
        'Tracks Enqueued',
        `Enqueued ${enqueued} of ${selected} tracks into download queue${breakdownMsg}.`
      )
    } else {
      const firstReason = reasons.length > 0 ? `: ${reasons[0]}` : ''
      toast.info(
        'No Eligible Tracks',
        `0 of ${selected} tracks enqueued${firstReason}.`
      )
    }
    clearSelection()
    showBulkMenu.value = false
  } catch (error: any) {
    console.error('Failed to queue bulk download:', error)
    const errStr = String(error?.message || error || '')
    toast.error(`Failed to download selection: ${errStr}`)
  } finally {
    isEnqueuingSelection.value = false
  }
}

function clearSelection() {
  tracks.value.forEach(t => t.isSelected = false)
  selectedCount.value = 0
}

// Context menu with viewport bounds protection
function openContextMenu(event: MouseEvent, track: Track) {
  const menuWidth = 240
  const menuHeight = 360
  const padding = 8
  const maxX = Math.max(padding, (window.innerWidth || 1200) - menuWidth - padding)
  const maxY = Math.max(padding, (window.innerHeight || 800) - menuHeight - padding)
  const x = Math.min(Math.max(padding, event.clientX), maxX)
  const y = Math.min(Math.max(padding, event.clientY), maxY)

  contextMenu.value = {
    visible: true,
    x,
    y,
    track
  }
}

function closeContextMenu() {
  contextMenu.value.visible = false;
  contextMenu.value.track = null;
}

// Action Handlers
async function handleDownloadBestQuality(track: Track) {
  try {
    await loadEffectivePrefs()
    await addToQueue({
      trackId: track.id,
      targetTitle: track.title,
      targetArtist: track.artist,
      targetAlbum: track.album,
      qualityPreference: effectiveMaxQualityLabel(),
      allowFallback: isDowngradeAllowed(),
    });
    track.downloadStatus = 'queued';
    toast.success(`Enqueued "${track.title}" for download`);
    closeContextMenu();
  } catch (error: any) {
    console.error('Failed to queue download:', error);
    const errStr = String(error?.message || error || '');
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', `Track "${track.title}" has no available provider source.`);
    } else {
      toast.error(`Failed to enqueue: ${errStr}`);
    }
  }
}

async function handleDownloadFromService(track: Track, service: string, tidalQuality?: 'HI_RES_LOSSLESS' | 'LOSSLESS' | 'HIGH') {
  try {
    await addToQueue({
      trackId: track.id,
      serviceName: service.toLowerCase(),
      targetTitle: track.title,
      targetArtist: track.artist,
      targetAlbum: track.album,
      qualityPreference: service.toLowerCase() === 'qobuz' ? '24-96' : service.toLowerCase() === 'tidal' ? (tidalQuality ?? 'HI_RES_LOSSLESS') : '16-44',
      allowFallback: false,
    });
    track.downloadStatus = 'queued';
    toast.success(`Enqueued "${track.title}" from ${service}`);
    closeContextMenu();
  } catch (error: any) {
    console.error(`Failed to queue download from ${service}:`, error);
    const errStr = String(error?.message || error || '');
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', `Track "${track.title}" has no available source for ${service}.`);
    } else {
      toast.error(`Failed to enqueue: ${errStr}`);
    }
  }
}

async function handleToggleFavorite(track: Track) {
  const previousState = track.isFavorite;
  track.isFavorite = !previousState;

  try {
    const confirmedState = await libraryApi.toggleTrackFavorite(track.id);
    track.isFavorite = confirmedState;
    toast.success(
      confirmedState ? 'Added to favorites' : 'Removed from favorites',
      `"${track.title}"`
    );
  } catch (error) {
    track.isFavorite = previousState;
    toast.error('Failed to update favorite', String(error));
  } finally {
    closeContextMenu();
  }
}

function isNonEmptySpotifyId(value: string | null | undefined): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

function handleViewOnSpotify(track: Track) {
  const spotifyId = track.spotifyTrackId;
  if (isNonEmptySpotifyId(spotifyId)) {
    window.open(`https://open.spotify.com/track/${encodeURIComponent(spotifyId.trim())}`, '_blank', 'noopener,noreferrer');
  } else {
    const query = `${track.title || ''} ${track.artist || ''}`.trim();
    window.open(`https://open.spotify.com/search/${encodeURIComponent(query)}`, '_blank', 'noopener,noreferrer');
  }
  closeContextMenu();
}

function handleViewOnQobuz(track: Track) {
  const query = `${track.title || ''} ${track.artist || ''}`.trim();
  window.open(`https://www.qobuz.com/search?q=${encodeURIComponent(query)}`, '_blank', 'noopener,noreferrer');
  closeContextMenu();
}

function handleShowMetadata(track: Track) {
  router.push({
    path: '/metadata',
    query: { trackId: String(track.id) }
  });
  closeContextMenu();
}

async function handleShowInFolder(track: Track) {
  try {
    await libraryApi.showInFolder(track.id);
    closeContextMenu();
  } catch (error) {
    toast.error('Cannot show in folder', String(error));
  }
}

/**
 * Borrar una pista la elimina en cascada de `playlist_tracks`, pero no de
 * `playlists.track_count`: sin esto, la sidebar de Playlists seguiría anunciando
 * canciones que ya no existen. Es una pasada de mantenimiento, así que un fallo
 * aquí no debe tumbar el borrado que ya se completó.
 */
async function reconcilePlaylistCounts() {
  try {
    await playlistsApi.sanitizePlaylists()
  } catch (error) {
    console.warn('Could not refresh playlist track counts:', error)
  }
}

async function handleRemoveFromLibrary(track: Track) {
  closeContextMenu();
  const confirmed = window.confirm(`Remove "${track.title}" from your library? This cannot be undone.`);
  if (!confirmed) return;

  try {
    await libraryApi.removeTrack(track.id);
    const index = tracks.value.findIndex(t => t.id === track.id);
    if (index > -1) {
      tracks.value.splice(index, 1);
      trackCount.value = tracks.value.length;
    }
    await reconcilePlaylistCounts();
    toast.success('Track removed', `"${track.title}" removed from library`);
  } catch (error) {
    toast.error('Failed to remove track', String(error));
  }
}

async function handleBulkRemove() {
  const selectedTracks = tracks.value.filter(t => t.isSelected);
  if (selectedTracks.length === 0) return;

  const ok = window.confirm(`Remove ${selectedTracks.length} track(s) from your library? This cannot be undone.`);
  if (!ok) return;

  try {
    const ids = selectedTracks.map(t => t.id);
    const removed = await libraryApi.bulkRemoveTracks(ids);
    tracks.value = tracks.value.filter(t => !t.isSelected);
    selectedCount.value = 0;
    trackCount.value = tracks.value.length;
    showBulkMenu.value = false;
    await reconcilePlaylistCounts();
    toast.success('Tracks removed', `${removed} tracks removed from library`);
  } catch (error) {
    toast.error('Failed to remove tracks', String(error));
  }
}

let reloadDebounceTimer: ReturnType<typeof setTimeout> | null = null;
function debouncedReloadLibrary() {
  if (reloadDebounceTimer) {
    clearTimeout(reloadDebounceTimer);
  }
  reloadDebounceTimer = setTimeout(() => {
    loadLibrary();
  }, 350);
}

// Helpers
function getServiceStyle(service: string): string {
  const normalized = service.toLowerCase().trim();
  const styles: Record<string, string> = {
    'spotify': 'bg-[#1ed760] text-black',
    'qobuz': 'bg-[#1a8fe3] text-white',
    'tidal': 'bg-black text-white border border-gray-600',
    'deezer': 'bg-[#ff0092] text-white',
    'soundcloud': 'bg-[#ff5500] text-white',
    'apple music': 'bg-gradient-to-br from-[#fc3c44] to-[#8c1c5c] text-white',
    'apple': 'bg-gradient-to-br from-[#fc3c44] to-[#8c1c5c] text-white',
  }
  return styles[normalized] || 'bg-gray-500 text-white'
}

function getServiceIcon(service: string): string {
  // L-07: el nombre canónico lo decide formatServiceName (el mismo formateador
  // que usa el resto de la app) y la inicial sale de ahí, así que dos juegos de
  // alias ('apple' / 'apple music') no pueden divergir entre sí.
  const canonical = formatServiceName(service).trim();
  const short: Record<string, string> = {
    'Spotify': 'S',
    'Qobuz': 'Q',
    'Tidal': 'T',
    'Deezer': 'D',
    'SoundCloud': 'SC',
    'Apple Music': '♪',
    'Local': 'L',
  };
  return short[canonical] || canonical.charAt(0).toUpperCase() || '?';
}

function getQualityStyle(quality: string): string {
  const q = quality.toUpperCase();
  if (q.includes('24/') || q.includes('HI-RES') || q.includes('HIRES')) {
    return 'bg-quality-gold/10 text-quality-gold border-quality-gold/20 font-bold'
  } else if (q.includes('16/') || q === 'FLAC' || q.includes('LOSSLESS') || q.includes('CD')) {
    return 'bg-quality-silver/10 text-quality-silver border-quality-silver/20 font-medium'
  } else if (q.includes('AAC') || q.includes('M4A') || q.includes('320')) {
    return 'bg-amber-500/10 text-amber-500 border-amber-500/20 font-medium'
  } else {
    return 'bg-gray-500/10 text-gray-400 border-gray-500/20'
  }
}

function getEffectiveDownloadTooltip(service: string | null | undefined): string {
  if (!service) return 'Downloaded'
  const normalized = service.toLowerCase()
  const names: Record<string, string> = {
    'qobuz': 'Qobuz',
    'tidal': 'Tidal',
    'spotify': 'Spotify',
    'deezer': 'Deezer',
    'apple': 'Apple Music',
  }
  const formattedName = names[normalized] || (service.charAt(0).toUpperCase() + service.slice(1))
  return `Downloaded from ${formattedName}`
}

function getMetadataScoreStyle(score: number): string {
  if (score >= 80) return 'bg-success/20 text-success';
  if (score >= 60) return 'bg-warning/20 text-warning';
  return 'bg-error/20 text-error';
}

function getLyricsTypeIcon(type: string): { icon: string; class: string; title: string } {
  switch (type) {
    case 'synced':
      return { icon: 'lyrics', class: 'text-primary', title: 'Synced lyrics' };
    case 'timed':
      return { icon: 'timer', class: 'text-blue-400', title: 'Timed lyrics' };
    case 'plain':
      return { icon: 'notes', class: 'text-gray-400', title: 'Plain lyrics' };
    default:
      return { icon: 'music_off', class: 'text-gray-300 dark:text-gray-600', title: 'No lyrics' };
  }
}

function toggleSelectAll() {
  const allSelected = tracks.value.every(t => t.isSelected)
  tracks.value.forEach(t => t.isSelected = !allSelected)
  selectedCount.value = allSelected ? 0 : tracks.value.length
}

// Only page appends may reuse a sorted prefix; all other invalidations sort anew.
const pageAppendCount = ref(0)
let sortedPrefix: Track[] = []
let previousSortKey = ''
let previousFilterKey = ''
let previousSource: Track[] | null = null
let previousFilteredLength = 0
let previousAppendCount = -1

function compareTracks(a: Track, b: Track): number {
  let comparison = 0
  switch (sortBy.value) {
    case 'title': comparison = a.title.localeCompare(b.title); break
    case 'artist': comparison = a.artist.localeCompare(b.artist); break
    case 'album': comparison = a.album.localeCompare(b.album); break
    case 'duration': comparison = a.duration.localeCompare(b.duration); break
    case 'quality': comparison = a.quality.localeCompare(b.quality); break
    case 'metadataScore': comparison = (a.metadataScore ?? 0) - (b.metadataScore ?? 0); break
  }
  return sortDirection.value === 'desc' ? -comparison : comparison
}

function mergeSorted(prefix: Track[], appended: Track[]): Track[] {
  const merged: Track[] = []
  let i = 0
  let j = 0
  while (i < prefix.length && j < appended.length) {
    // Stable sort: earlier pages precede later ones on equal keys.
    if (compareTracks(prefix[i], appended[j]) <= 0) merged.push(prefix[i++])
    else merged.push(appended[j++])
  }
  while (i < prefix.length) merged.push(prefix[i++])
  while (j < appended.length) merged.push(appended[j++])
  return merged
}

const filteredTracks = computed(() => {
  let trackList = searchQuery.value.trim() ? searchResults.value : tracks.value
  
  if (activeFilters.value.includes('flac')) {
    trackList = trackList.filter(t => t.quality.includes('/') && !t.quality.includes('kbps'))
  }
  if (activeFilters.value.includes('mp3')) {
    trackList = trackList.filter(t => t.quality.includes('kbps'))
  }
  if (activeFilters.value.includes('downloaded')) {
    trackList = trackList.filter(t => t.downloadStatus === 'downloaded')
  }
  if (activeFilters.value.includes('notDownloaded')) {
    trackList = trackList.filter(t => t.downloadStatus !== 'downloaded')
  }
  if (activeFilters.value.includes('multiSource')) {
    trackList = trackList.filter(t => t.services.length >= 2)
  }
  if (activeFilters.value.includes('favorites')) {
    trackList = trackList.filter(t => t.isFavorite)
  }
  if (activeFilters.value.includes('quality')) {
    // FE-10: deep-link quality filter. Tracks carry the backend-computed
    // bucket; tracks without one fall back to an exact quality-string match.
    trackList = trackList.filter(t => {
      if (!qualityFilter.value) return true
      if (t.qualityBucket) return t.qualityBucket === qualityFilter.value
      return !!t.quality && t.quality !== '—' && t.quality.toUpperCase() === qualityFilter.value.toUpperCase()
    })
  }
  
  const sortKey = `${sortBy.value}:${sortDirection.value}`
  const filterKey = JSON.stringify(activeFilters.value) + '|' + qualityFilter.value

  // The sorted prefix can only be reused when the sort key and filters are
  // unchanged, the previous pass ran over the very same collection (same
  // identities, same length), and exactly one page was appended since.
  // Anything else re-sorts the whole list, exactly as before.
  const isAppendOnly =
    sortKey === previousSortKey &&
    filterKey === previousFilterKey &&
    previousSource === tracks.value &&
    trackList === tracks.value &&
    pageAppendCount.value > previousAppendCount &&
    previousFilteredLength < trackList.length

  previousSortKey = sortKey
  previousFilterKey = filterKey
  previousSource = trackList
  previousFilteredLength = trackList.length
  previousAppendCount = pageAppendCount.value

  if (isAppendOnly) {
    const appendedSorted = trackList.slice(sortedPrefix.length).sort(compareTracks)
    sortedPrefix = mergeSorted(sortedPrefix, appendedSorted)
    return sortedPrefix
  }

  sortedPrefix = [...trackList].sort(compareTracks)
  return sortedPrefix
})

// Virtual scrolling computed window (PR-02)
const totalVirtualHeight = computed(() => {
  return filteredTracks.value.length * ROW_HEIGHT + (!hasMore.value && tracks.value.length > 0 ? 48 : 0)
})

const virtualStartIndex = computed(() => {
  return Math.max(0, Math.floor(libraryScrollTop.value / ROW_HEIGHT) - OVERSCAN)
})

const virtualEndIndex = computed(() => {
  return Math.min(
    filteredTracks.value.length,
    Math.ceil((libraryScrollTop.value + libraryViewportHeight.value) / ROW_HEIGHT) + OVERSCAN
  )
})

const virtualOffsetY = computed(() => {
  return virtualStartIndex.value * ROW_HEIGHT
})

const visibleTracks = computed(() => {
  return filteredTracks.value.slice(virtualStartIndex.value, virtualEndIndex.value)
})

watch([() => filteredTracks.value.length, searchQuery, () => activeFilters.value], () => {
  if (listContainerRef.value && libraryScrollTop.value > totalVirtualHeight.value) {
    listContainerRef.value.scrollTop = 0
    libraryScrollTop.value = 0
  }
})

// Album interface for grid view
interface Album {
  id: number
  albumId?: number | null
  title: string
  artist: string
  coverUrl: string | null
  artGradient: string
  services: string[]
  quality: string
  downloadStatus: 'downloaded' | 'queued' | 'not_downloaded'
  metadataScore: number
  trackCount: number
  isSelected: boolean
  tracks: Track[]
}

const expandedGroups = ref<string[]>(['Kavinsky', 'M83', 'OutRun', 'Hurry Up, We\'re Dreaming', 'Electronic', 'Synthwave', 'Rock', 'Pop'])

const groupedByArtist = computed(() => {
  const groups: { artist: string; tracks: Track[] }[] = []
  const artistMap = new Map<string, Track[]>()
  
  for (const track of filteredTracks.value) {
    const existing = artistMap.get(track.artist)
    if (existing) {
      existing.push(track)
    } else {
      artistMap.set(track.artist, [track])
    }
  }
  
  artistMap.forEach((tracks, artist) => {
    groups.push({ artist, tracks })
  })
  
  return groups.sort((a, b) => a.artist.localeCompare(b.artist))
})

const groupedByAlbum = computed<Album[]>(() => {
  const albumMap = new Map<string, Track[]>()
  
  for (const track of filteredTracks.value) {
    const key = track.albumId ? `id_${track.albumId}` : `${track.album}||${track.artist}`
    const existing = albumMap.get(key)
    if (existing) {
      existing.push(track)
    } else {
      albumMap.set(key, [track])
    }
  }
  
  const albums: Album[] = []
  let idCounter = 1
  
  albumMap.forEach((tracks) => {
    const firstTrack = tracks[0]
    const albumRealId = firstTrack.albumId ?? idCounter++
    const uniqueServices = new Set<string>()
    tracks.forEach(t => t.services.forEach(s => uniqueServices.add(s)))
    
    albums.push({
      id: albumRealId,
      albumId: firstTrack.albumId ?? null,
      title: firstTrack.album,
      artist: firstTrack.artist,
      coverUrl: firstTrack.coverUrl,
      artGradient: firstTrack.artGradient,
      services: Array.from(uniqueServices),
      quality: firstTrack.quality,
      downloadStatus: tracks.every(t => t.downloadStatus === 'downloaded') ? 'downloaded' : 
                      tracks.some(t => t.downloadStatus === 'queued') ? 'queued' : 'not_downloaded',
      metadataScore: Math.round(tracks.reduce((sum, t) => sum + (t.metadataScore ?? 0), 0) / tracks.length),
      trackCount: tracks.length,
      isSelected: false,
      tracks: tracks
    })
  })
  
  return albums.sort((a, b) => a.title.localeCompare(b.title))
})

// Real Genre Grouping
const groupedByGenre = computed(() => {
  const groups: { genre: string; tracks: Track[] }[] = []
  const genreMap = new Map<string, Track[]>()
  
  for (const track of filteredTracks.value) {
    const g = track.genre ? track.genre.trim() : 'Unknown Genre'
    const existing = genreMap.get(g)
    if (existing) {
      existing.push(track)
    } else {
      genreMap.set(g, [track])
    }
  }
  
  genreMap.forEach((tracks, genre) => {
    groups.push({ genre, tracks })
  })
  
  return groups.sort((a, b) => {
    if (a.genre === 'Unknown Genre') return 1
    if (b.genre === 'Unknown Genre') return -1
    return a.genre.localeCompare(b.genre)
  })
})

const groupedByQuality = computed(() => {
  const groups: { quality: string; tracks: Track[] }[] = []
  const qualityMap = new Map<string, Track[]>()
  
  for (const track of filteredTracks.value) {
    const quality = (!track.quality || track.quality === '—') ? 'Unknown' : track.quality
    const existing = qualityMap.get(quality)
    if (existing) {
      existing.push(track)
    } else {
      qualityMap.set(quality, [track])
    }
  }
  
  qualityMap.forEach((tracks, quality) => {
    groups.push({ quality, tracks })
  })
  
  return groups.sort((a, b) => {
    if (a.quality === 'Unknown') return 1
    if (b.quality === 'Unknown') return -1
    return b.quality.localeCompare(a.quality)
  })
})

function toggleGroupExpand(groupName: string) {
  const idx = expandedGroups.value.indexOf(groupName)
  if (idx >= 0) {
    expandedGroups.value.splice(idx, 1)
  } else {
    expandedGroups.value.push(groupName)
  }
}

function getQualityBadgeStyle(quality: string): string {
  if (quality.includes('24/')) {
    return 'bg-quality-gold text-black'
  } else if (quality.includes('16/') || quality === 'FLAC') {
    return 'bg-quality-silver text-black'
  } else {
    return 'bg-gray-600 text-white'
  }
}

// Methods
function toggleFilter(filterId: string) {
  if (filterId === 'all') {
    activeFilters.value = ['all']
  } else {
    activeFilters.value = activeFilters.value.filter(f => f !== 'all')
    
    if (activeFilters.value.includes(filterId)) {
      activeFilters.value = activeFilters.value.filter(f => f !== filterId)
      if (activeFilters.value.length === 0) {
        activeFilters.value = ['all']
      }
    } else {
      activeFilters.value.push(filterId)
    }
  }

  if (filterId === 'duplicates' || filterId === 'favorites' || activeFilters.value.includes('all')) {
    loadLibrary()
  }
}

function removeFilter(filterId: string) {
  activeFilters.value = activeFilters.value.filter(f => f !== filterId)
  if (activeFilters.value.length === 0) {
    activeFilters.value = ['all']
  }
  if (filterId === 'duplicates' || filterId === 'favorites') {
    loadLibrary()
  }
}

function getFilterLabel(filterId: string) {
  if (filterId === 'quality') {
    return qualityFilter.value ? `Quality: ${qualityFilter.value}` : 'Quality'
  }
  return filterPills.find(f => f.id === filterId)?.label || filterId
}

function clearAllFilters() {
  activeFilters.value = ['all']
  qualityFilter.value = ''
  searchQuery.value = ''
  loadLibrary()
}

// Keep the deep-link value in sync with the chip: whenever the 'quality'
// filter leaves activeFilters (chip removed, clear all, switch to All), the
// bucket value is dropped too.
watch(() => activeFilters.value.includes('quality'), (hasQuality) => {
  if (!hasQuality) {
    qualityFilter.value = ''
  }
})

function handleKeydown(event: KeyboardEvent) {
  const target = event.target as HTMLElement
  if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) {
    return
  }

  if (event.key === 'd' || event.key === 'D') {
    if (selectedCount.value > 0) {
      downloadSelectedTracks()
    } else if (contextMenu.value.track) {
      handleDownload(contextMenu.value.track)
    }
  } else if (event.key === 'q' || event.key === 'Q') {
    // FE-9: "Add to Downloads" — same enqueue path as D (download queue).
    if (selectedCount.value > 0) {
      downloadSelectedTracks()
    } else if (contextMenu.value.track) {
      void handleAddToQueue(contextMenu.value.track)
    }
  } else if (event.key === 'n' || event.key === 'N') {
    // FE-9: "Play Next" on the context track or the first selected track.
    const nextTarget = contextMenu.value.track ?? tracks.value.find(t => t.isSelected)
    if (nextTarget) void handlePlayNext(nextTarget)
  } else if (event.key === 'f' || event.key === 'F') {
    if (contextMenu.value.track) {
      handleToggleFavorite(contextMenu.value.track)
    } else {
      const selected = tracks.value.find(t => t.isSelected)
      if (selected) handleToggleFavorite(selected)
    }
  } else if (event.key === ' ' || event.code === 'Space') {
    // FE-9: Space = Play/Pause as announced by the context menu and the
    // shortcuts modal. With the menu open on an idle player it starts that
    // track ("Play Now"); otherwise it toggles the current playback.
    event.preventDefault()
    if (contextMenu.value.visible && contextMenu.value.track && !playerCurrent.value) {
      void handleTrackPlay(contextMenu.value.track)
    } else {
      togglePlayback()
    }
  } else if (event.key === 'Escape') {
    clearSelection()
    closeContextMenu()
    showShortcutsModal.value = false
    showDownloadFavoritesModal.value = false
  } else if (event.key === 'v' || event.key === 'V') {
    viewMode.value = viewMode.value === 'list' ? 'grid' : 'list'
  }
}

// FE-10: consume the DashboardView deep-links. `?filter=quality&quality=<bucket>`
// comes from the quality cards and `?filter=duplicates` from the duplicates
// shortcut. Reading it on mount alone left a stale filter when the query
// changes in place (back/forward, or another deep-link to the same route),
// so the query is watched like MetadataView does for `?filter=needs_work`.
function applyRouteFilters() {
  const filterParam = route?.query?.filter
  if (typeof filterParam !== 'string' || filterParam === '') return

  if (filterParam === 'quality') {
    const qualityParam = route?.query?.quality
    qualityFilter.value = typeof qualityParam === 'string' && qualityParam ? qualityParam : ''
    if (qualityFilter.value) {
      activeFilters.value = ['quality']
    }
    return
  }

  activeFilters.value = [filterParam]
}

// ==============================================
// Ítem 45: deep-link /library?track=<id>
// ==============================================
// Al montar, con la biblioteca ya cargada, si el query trae `track` se hace
// scroll hasta esa fila, se resalta unos segundos y se limpia el parámetro
// para que recargar no repita el salto.
const highlightedTrackId = ref<number | null>(null)
let highlightClearTimer: ReturnType<typeof setTimeout> | null = null

/** Tope de páginas cargadas al buscar la pista enlazada (50 × PAGE_SIZE). */
const TRACK_DEEP_LINK_MAX_PAGES = 50

async function focusTrackFromQuery() {
  const raw = route?.query?.track
  if (raw === undefined || raw === null || raw === '') return

  // El parámetro se consume SIEMPRE, exista o no la pista: así un recargazo o
  // volver atrás no repite el scroll.
  const rest: Record<string, LocationQueryValue | LocationQueryValue[]> = { ...route.query }
  delete rest.track
  await router.replace({ query: rest })

  const trackId = typeof raw === 'string' ? Number.parseInt(raw, 10) : NaN
  if (!Number.isFinite(trackId)) return

  // Si la pista cae en páginas aún no cargadas, pagina hasta encontrarla.
  let pagesLoaded = 0
  while (
    !tracks.value.some(t => t.id === trackId) &&
    hasMore.value &&
    !isLoadingMore.value &&
    pagesLoaded < TRACK_DEEP_LINK_MAX_PAGES
  ) {
    await loadMore()
    pagesLoaded++
  }

  const trackIdx = filteredTracks.value.findIndex(t => t.id === trackId)
  if (trackIdx >= 0 && listContainerRef.value) {
    listContainerRef.value.scrollTop = Math.max(0, trackIdx * ROW_HEIGHT - (libraryViewportHeight.value / 2))
    libraryScrollTop.value = listContainerRef.value.scrollTop
  }

  await nextTick()
  const row = document.querySelector(`[data-track-id="${trackId}"]`)
  if (!row) return

  row.scrollIntoView({ behavior: 'smooth', block: 'center' })
  highlightedTrackId.value = trackId
  if (highlightClearTimer) {
    clearTimeout(highlightClearTimer)
  }
  highlightClearTimer = setTimeout(() => {
    highlightedTrackId.value = null
    highlightClearTimer = null
  }, 5000)
}

// Lifecycle
let queueStatsInterval: ReturnType<typeof setInterval> | null = null

onMounted(async () => {
  document.addEventListener('click', closeContextMenu)
  window.addEventListener('keydown', handleKeydown)

  applyRouteFilters()

  await loadLibrary()
  await fetchQueueStats()

  // Ítem 45: tras la carga inicial, atiende el deep-link ?track=<id>.
  await focusTrackFromQuery()

  queueStatsInterval = setInterval(fetchQueueStats, 4000)

  eventBus.on(TauriEvents.LIBRARY_UPDATED, async () => {
    debouncedReloadLibrary()
  })
  eventBus.on(TauriEvents.IMPORT_COMPLETE, async () => {
    debouncedReloadLibrary()
  })
  eventBus.on(TauriEvents.SYNC_COMPLETE, async () => {
    debouncedReloadLibrary()
  })
  eventBus.on(TauriEvents.DOWNLOAD_PROGRESS, async () => {
    fetchQueueStats()
  })
})

watch(() => [...activeFilters.value], async (newFilters, oldFilters) => {
  const hasNow = newFilters.includes('duplicates');
  const hadBefore = oldFilters ? oldFilters.includes('duplicates') : false;
  if (hasNow !== hadBefore) {
    await loadLibrary();
  }
})

// FE-10: react to deep-link query changes on the same route (e.g. browser
// back between two quality buckets) instead of keeping the mount-time filter.
watch(() => [route?.query?.filter, route?.query?.quality], () => {
  applyRouteFilters()
})

onUnmounted(() => {
  document.removeEventListener('click', closeContextMenu)
  window.removeEventListener('keydown', handleKeydown)
  if (queueStatsInterval) {
    clearInterval(queueStatsInterval)
    queueStatsInterval = null
  }
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
  if (reloadDebounceTimer) {
    clearTimeout(reloadDebounceTimer)
  }
  if (scrollThrottleTimer) {
    clearTimeout(scrollThrottleTimer)
    scrollThrottleTimer = null
  }
  if (highlightClearTimer) {
    clearTimeout(highlightClearTimer)
    highlightClearTimer = null
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
  background: var(--surface-2);
  border-radius: 3px;
}

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: var(--accent);
}

.filter-pills::-webkit-scrollbar {
  height: 4px;
}

/* Slow spin animation for active icon (L-12) */
@keyframes spin-slow {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.animate-spin-slow {
  animation: spin-slow 3s linear infinite;
}

/* Fade transition for context menu and modals */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Slide down transition for batch selection bar */
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.2s ease;
}
.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>
