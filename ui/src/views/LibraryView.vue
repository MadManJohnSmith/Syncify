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

    <!-- L03 (#100): armazón de pestañas de la biblioteca (spec #96). El
         cambio conserva los filtros compatibles (`libraryTabQuery`) y
         descarta solo los que no aplican; el contenido de álbumes,
         artistas y duplicados llega con L07/L08/L10. -->
    <LibraryTabs current="tracks" />

    <!-- L03 (#100): chips de estado de la biblioteca. El chip Descargadas
         aplica y retira su filtro; su contador viene de los contadores que
         L01 pide con la primera página. L07 (#104) activa el chip Álbumes:
         muestra los álbumes distintos con al menos una pista viva y navega a
         su pestaña. Artistas, cuarentena y duplicados siguen deshabilitados
         con su motivo (L08, #72, #73): no se simula una dependencia. -->
    <div class="library-chips px-8 py-2 flex items-center gap-2 flex-wrap shrink-0" data-testid="library-chips">
      <button
        v-for="chip in headerChips"
        :key="chip.id"
        :disabled="chip.disabled"
        :title="chip.reason"
        :aria-pressed="chip.disabled ? undefined : chip.active"
        :data-testid="`library-chip-${chip.id}`"
        @click="toggleHeaderChip(chip)"
        :class="[
          'flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-semibold transition-all',
          chip.disabled
            ? 'opacity-40 cursor-not-allowed border border-gray-200 dark:border-border-dark text-text-secondary bg-white dark:bg-surface-dark'
            : chip.active
            ? 'bg-primary/15 border border-primary/40 text-primary shadow-xs cursor-pointer'
            : 'bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark text-text-secondary hover:text-ink hover:border-gray-300 dark:hover:border-gray-600 cursor-pointer'
        ]"
      >
        <span class="material-symbols-outlined text-[15px]" aria-hidden="true">{{ chip.icon }}</span>
        <span>{{ chip.label }}</span>
        <strong class="font-bold" data-testid="library-chip-count">{{ chip.count === null ? '—' : chip.count.toLocaleString() }}</strong>
      </button>
    </div>

    <!-- Collapsible Queue Details Panel (Reconciliation Strip) -->
    <div v-show="showQueueDetails" class="px-8 mb-3 space-y-2 transition-all shrink-0">
      <div class="reconciliation-strip px-4 py-2 rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-xs flex items-center justify-between gap-3 text-xs flex-wrap">
        <div class="flex items-center gap-1.5 font-semibold text-text-secondary shrink-0">
          <span class="material-symbols-outlined text-[16px] text-primary">analytics</span>
          <span class="uppercase tracking-wider text-[10px]">Queue Reconciliation:</span>
        </div>
        <div class="flex items-center gap-3 flex-wrap text-text-secondary text-[11px]">
          <span title="Total tracks requested by user / selection">Selected: <strong class="text-gray-900 dark:text-white">{{ queueStats?.submitted ?? selectedCount }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Initial eligible tracks in queue">Eligible: <strong class="text-indigo-400">{{ (queueStats?.submitted ?? selectedCount) - (queueStats?.skipped ?? 0) - (queueStats?.deduplicated ?? 0) }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Excluded before queuing due to preflight filter">Excluded Preflight: <strong class="text-amber-500">{{ (queueStats?.skipped ?? 0) + (queueStats?.deduplicated ?? 0) }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks currently in queued state">Pending: <strong class="text-amber-500">{{ queueStats?.queued ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Tracks currently downloading concurrently">Active: <strong class="text-primary">{{ queueStats?.active ?? queueStats?.downloading ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Finished downloads in queue">Completed: <strong class="text-success">{{ queueStats?.completed ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Failed downloads in queue">Failed: <strong class="text-error">{{ queueStats?.failed ?? 0 }}</strong></span>
          <span class="w-px h-3 bg-gray-200 dark:bg-border-dark"></span>
          <span title="Cancelled downloads in queue">Cancelled: <strong class="text-orange-400">{{ queueStats?.cancelled ?? 0 }}</strong></span>
        </div>
      </div>
    </div>

    <!-- Filter Pills Row -->
    <div class="filter-pills px-8 py-3 flex gap-2 shrink-0 overflow-x-auto custom-scrollbar">
      <button 
        v-for="filter in filterPills" 
        :key="filter.id"
        :disabled="filter.disabled"
        :title="filter.reason"
        @click="toggleFilter(filter.id)"
        :class="[
          'filter-pill px-3 py-1.5 rounded-full text-xs font-semibold flex items-center gap-2 whitespace-nowrap transition-all',
          filter.disabled
            ? 'opacity-40 cursor-not-allowed border border-gray-200 dark:border-border-dark text-text-secondary bg-white dark:bg-surface-dark'
            : activeFilters.includes(filter.id)
            ? 'bg-primary/15 border border-primary/40 text-primary font-bold shadow-xs cursor-pointer'
            : 'bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark text-text-secondary hover:text-ink hover:border-gray-300 dark:hover:border-gray-600 cursor-pointer'
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

        <!-- MB Dump Catalog Button (#264 punto 1): importación local del mbdump -->
        <button
          @click="toggleMbDumpPanel"
          :class="['flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold transition-all border', mbDumpPanelOpen ? 'bg-primary/20 text-primary border-primary/40' : 'bg-primary/10 text-primary hover:bg-primary/20 border-primary/30']"
          title="Import a local MusicBrainz mbdump catalog"
        >
          <span class="material-symbols-outlined text-[16px]">storage</span>
          <span>MB Catalog</span>
        </button>

        <!-- MF-BULK-15: no complete account/playlist inventory or safe preview API is exposed yet. -->
        <button
          type="button"
          disabled
          aria-label="Download complete library including playlists unavailable"
          title="Unavailable: requires a complete paginated per-account playlist inventory, dry-run preview, confirmation and resumable idempotent batches"
          class="px-3 py-2 rounded-lg text-xs font-semibold border border-gray-300 opacity-60 cursor-not-allowed"
        >Download complete library (unavailable)</button>

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
        
        <!-- Bulk Actions (L05, #102): selección de ids o de todo el filtro -->
        <div v-if="selectedCount > 0" class="flex items-center gap-2 relative">
          <button 
            @click="showBulkMenu = !showBulkMenu"
            class="flex items-center gap-2 px-3 py-2 bg-primary text-on-accent rounded-lg text-xs font-semibold shadow-lg shadow-primary/20 hover:bg-primary-hover transition-colors relative"
          >
            <span class="material-symbols-outlined text-[16px]">checklist</span>
            <span>{{ selectAllMatching ? `All ${selectedCount} matching` : `${selectedCount} selected` }}</span>
            <span class="material-symbols-outlined text-[14px]">expand_more</span>
          </button>
          <div v-if="showBulkMenu" class="absolute top-full right-0 mt-1 w-56 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg shadow-xl z-20 py-1">
            <button @click="openBulkDownload" class="w-full px-3 py-2 text-left text-xs hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-2 text-gray-700 dark:text-gray-300">
              <span class="material-symbols-outlined text-[16px]">download</span> Add to Downloads
            </button>
            <button @click="openBulkFavorite(true)" class="w-full px-3 py-2 text-left text-xs hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-2 text-gray-700 dark:text-gray-300">
              <span class="material-symbols-outlined text-[16px]">favorite</span> Mark as Favorite
            </button>
            <button @click="openBulkFavorite(false)" class="w-full px-3 py-2 text-left text-xs hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-2 text-gray-700 dark:text-gray-300">
              <span class="material-symbols-outlined text-[16px]">heart_minus</span> Remove from Favorites
            </button>
            <hr class="my-1 border-gray-200 dark:border-border-dark">
            <button v-if="!selectAllMatching" @click="handleBulkRemove" class="w-full px-3 py-2 text-left text-xs hover:bg-error/10 transition-colors flex items-center gap-2 text-error">
              <span class="material-symbols-outlined text-[16px]">delete</span> Remove from library
            </button>
            <p v-else class="px-3 py-2 text-[10px] text-text-secondary leading-snug">
              Destructive actions are per-track only when selecting all matching tracks.
            </p>
          </div>
        </div>
      </div>
    </div>

    <!-- MB dump catalog import (#264 punto 1): panel mínimo colapsable -->
    <div v-if="mbDumpPanelOpen" class="px-8 pb-3">
      <div class="p-4 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl space-y-3">
        <div class="flex items-center gap-2">
          <input
            v-model="mbDumpPath"
            type="text"
            placeholder="Path to the MusicBrainz mbdump (tar with recording/isrc/artist/artist_credit_name)"
            class="flex-1 px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm text-ink focus:outline-none focus:ring-2 focus:ring-primary transition-all"
            :disabled="mbDumpImporting"
          >
          <button
            @click="startMbDumpImport"
            :disabled="mbDumpImporting || !mbDumpPath.trim()"
            class="flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold bg-primary/10 text-primary hover:bg-primary/20 border border-primary/30 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <span :class="['material-symbols-outlined text-[16px]', mbDumpImporting && 'animate-spin']">{{ mbDumpImporting ? 'progress_activity' : 'upload_file' }}</span>
            <span>{{ mbDumpImporting ? 'Importing…' : 'Import catalog' }}</span>
          </button>
        </div>
        <p v-if="mbDumpProgressLine" class="text-xs text-text-secondary">{{ mbDumpProgressLine }}</p>
        <p v-if="mbDumpMessage" :class="['text-xs', mbDumpError ? 'text-error' : 'text-primary']">{{ mbDumpMessage }}</p>
        <p v-if="!mbDumpMessage && mbDumpState" class="text-xs text-text-secondary">
          Last import: {{ mbDumpState.status }} · {{ mbDumpState.recording_rows }} recordings · {{ mbDumpState.artist_credit_name_rows }} artist credits · {{ mbDumpState.isrc_rows }} isrcs<template v-if="mbDumpState.error"> · {{ mbDumpState.error }}</template>
        </p>
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

    <!-- Batch Selection Bar (floating overlay - L-11 / L05 #102) -->
    <Transition name="slide-down">
      <div
        v-if="selectedCount > 0"
        class="batch-bar fixed inset-x-8 max-w-3xl mx-auto z-40 flex items-center gap-3 px-6 py-3.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl shadow-2xl"
        style="bottom: calc(var(--status-bar-h, 48px) + 16px)"
      >
        <div class="flex flex-col gap-0.5 whitespace-nowrap">
          <span class="text-accent font-bold text-sm">
            {{ selectAllMatching ? `All ${selectedCount} matching tracks` : `${selectedCount} track${selectedCount !== 1 ? 's' : ''} selected` }}
          </span>
          <button
            v-if="selectAllMatching"
            @click="selectLoadedPageOnly"
            class="text-[10px] text-text-secondary hover:text-primary text-left transition-colors cursor-pointer"
          >
            Select the loaded page instead
          </button>
        </div>
        <div class="flex-1 flex items-center justify-center gap-2 flex-wrap">
          <button
            @click="openBulkDownload"
            :disabled="isBulkRunning"
            :aria-busy="isBulkRunning"
            class="px-3.5 py-1.5 bg-primary hover:bg-primary-hover text-on-accent rounded-lg text-xs font-semibold shadow-md transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-60 disabled:cursor-wait"
          >
            <span class="material-symbols-outlined text-[16px]">{{ isBulkRunning ? 'progress_activity' : 'download' }}</span>
            {{ isBulkRunning ? 'Working…' : 'Download' }}
          </button>
          <button @click="openBulkFavorite(true)" :disabled="isBulkRunning" class="px-3.5 py-1.5 bg-transparent hover:bg-primary/10 text-primary border border-primary/40 rounded-lg text-xs font-semibold transition-all flex items-center gap-1.5 cursor-pointer disabled:opacity-60">
            <span class="material-symbols-outlined text-[16px]">favorite</span>
            Favorite
          </button>
          <select
            v-model="bulkPlaylistId"
            :disabled="isBulkRunning"
            class="max-w-[9rem] px-2 py-1.5 bg-gray-100 dark:bg-surface-highlight border border-gray-300 dark:border-gray-600 rounded-lg text-xs text-ink"
            aria-label="Playlist for the batch action"
          >
            <option :value="null" disabled>Playlist…</option>
            <option v-for="playlist in playlists" :key="playlist.id" :value="playlist.id">{{ playlist.name }}</option>
          </select>
          <button @click="openBulkAddToPlaylist" :disabled="isBulkRunning || bulkPlaylistId === null" class="px-3 py-1.5 bg-transparent hover:bg-primary/10 text-primary border border-primary/40 rounded-lg text-xs font-semibold transition-all cursor-pointer disabled:opacity-50">
            Add
          </button>
          <button v-if="!selectAllMatching" @click="handleBulkRemove" class="px-3.5 py-1.5 bg-transparent hover:bg-error/20 text-error border border-error/50 rounded-lg text-xs font-semibold transition-all flex items-center gap-1.5 cursor-pointer">
            <span class="material-symbols-outlined text-[16px]">delete</span>
            Remove
          </button>
        </div>
        <div class="flex flex-col items-end gap-0.5 whitespace-nowrap">
          <span v-if="bulkProgress" class="text-[10px] text-text-secondary">
            {{ bulkProgress.processed }}/{{ bulkProgress.total }} · block {{ bulkProgress.blocksDone }}/{{ bulkProgress.blocksTotal || '?' }}
          </span>
          <button @click="clearSelection" class="text-ink-2 hover:text-ink text-xs transition-colors cursor-pointer whitespace-nowrap">
            Clear Selection
          </button>
        </div>
      </div>
    </Transition>

    <!-- Confirmación de acción por lotes: recuento exacto ANTES de ejecutar -->
    <div
      v-if="bulkDialog"
      class="fixed inset-0 z-[60] flex items-center justify-center bg-black/50 p-4"
      data-testid="library-bulk-confirm"
      @click.self="bulkDialog = null"
    >
      <div class="w-full max-w-lg rounded-xl bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark shadow-2xl p-5 space-y-3">
        <h3 class="text-sm font-bold text-ink">{{ bulkDialog.preview.label }}</h3>
        <p class="text-sm text-ink">
          <strong>{{ bulkDialog.preview.eligible }}</strong> of
          <strong>{{ bulkDialog.preview.total }}</strong> selected tracks are eligible.
        </p>
        <ul v-if="bulkDialog.preview.omissions.length" class="text-xs text-text-secondary space-y-1 max-h-40 overflow-y-auto">
          <li v-for="omission in bulkDialog.preview.omissions" :key="omission.reason">
            {{ omission.count }} · {{ omission.message }}
          </li>
        </ul>
        <p class="text-[11px] text-text-secondary">
          The server re-checks the selection when it runs. If sync changed the library, the summary is recalculated.
        </p>
        <div class="flex justify-end gap-2 pt-1">
          <button @click="bulkDialog = null" class="px-3.5 py-1.5 rounded-lg text-xs font-semibold border border-gray-300 dark:border-gray-600 text-ink">Cancel</button>
          <button
            @click="confirmBulkAction"
            :disabled="isBulkRunning"
            class="px-3.5 py-1.5 rounded-lg text-xs font-semibold bg-primary hover:bg-primary-hover text-on-accent disabled:opacity-60"
          >
            {{ isBulkRunning ? 'Running…' : `Run on ${bulkDialog.preview.eligible}` }}
          </button>
        </div>
      </div>
    </div>

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
            <span class="flex-1 text-left">{{ contextMenu.track?.isFavorite ? 'Remove from Local Favorites' : 'Add to Local Favorites' }}</span>
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
          <!-- L04 (#101): panel lateral de detalle (solo lectura); la edición vive en /metadata. -->
          <button @click="openTrackDetail(contextMenu.track!.id)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors" data-testid="context-track-detail">
            <span class="material-symbols-outlined text-[16px]">side_pane</span>
            <span class="flex-1 text-left">Track Detail</span>
          </button>
          <button @click="handleShowMetadata(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">info</span>
            <span class="flex-1 text-left">Show Metadata</span>
          </button>
          <button v-if="contextMenu.track?.downloadStatus === 'downloaded'" @click="handleShowInFolder(contextMenu.track!)" class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors">
            <span class="material-symbols-outlined text-[16px]">folder</span>
            <span class="flex-1 text-left">Show in Folder</span>
          </button>
          <!-- L14 (#109): re-descarga por la cola existente. -->
          <button
            v-if="contextMenu.track?.downloadStatus === 'downloaded'"
            @click="contextMenu.track && handleReDownload(contextMenu.track)"
            class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-gray-200 hover:bg-accent-soft transition-colors"
            data-testid="context-re-download"
          >
            <span class="material-symbols-outlined text-[16px]">refresh</span>
            <span class="flex-1 text-left">Re-download</span>
          </button>
          <!-- L14 (#109): borrar audio (no la pista) con confirmación. -->
          <button
            v-if="contextMenu.track?.downloadStatus === 'downloaded' || contextMenu.track?.downloadStatus === 'failed'"
            @click="deleteAudioFromContextMenu"
            class="menu-item w-full px-4 py-2 flex items-center gap-3 text-xs text-warning hover:bg-warning/10 transition-colors"
            data-testid="context-delete-audio"
          >
            <span class="material-symbols-outlined text-[16px]">audiotrack</span>
            <span class="flex-1 text-left">Delete audio</span>
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
      
<!-- ERROR STATE: a load failure is not an empty library -->
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
      <div v-else-if="tracks.length === 0 && !isLoading" class="library-empty flex-1 flex flex-col items-center justify-center text-center py-16">
        <span class="material-symbols-outlined text-[60px] text-gray-400 dark:text-gray-600 mb-6">filter_list_off</span>
        <h3 class="text-xl font-bold text-ink mb-2">No tracks match your filters</h3>
        <p class="text-text-secondary mb-6">Try adjusting your search or filters</p>
        <button @click="clearAllFilters" class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-xl font-medium transition-all cursor-pointer">
          Clear All Filters
        </button>
      </div>

      <!-- LIST VIEW (Customizable Columns with Drag & Drop) -->
      <template v-else-if="viewMode === 'list' && groupBy === 'none'">
        <!-- Header Row -->
        <div class="track-list flex items-center gap-2 px-3 py-3 border-b border-gray-200 dark:border-border-dark text-[11px] font-bold text-text-secondary uppercase tracking-wider shrink-0 bg-shell/50 rounded-t-xl backdrop-blur-sm sticky top-0 z-10 select-none">
          <!-- Selection Checkbox: «todo el filtro», no solo la página cargada -->
          <div class="w-10 text-center shrink-0">
            <input
              type="checkbox"
              :checked="allMatchingSelected"
              :title="selectAllMatching ? 'Clear the selection' : 'Select all tracks matching the current filter'"
              @change="toggleSelectAll"
              class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-gray-300 dark:border-gray-600"
            >
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
          <div class="w-24 shrink-0"></div>
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
                :key="trackRowKey(track)"
                :data-track-id="track.id"
                @click="handleTrackClick(track)"
                @dblclick="handleTrackPlay(track)"
                @contextmenu.prevent="openContextMenu($event, track)"
                :style="{ height: ROW_HEIGHT + 'px' }"
                :class="[
                  'track-row flex items-center gap-2 px-3 py-2 border-b border-gray-100 dark:border-border-dark/50 last:border-0 transition-all group cursor-pointer box-border',
                  track.isPlaying ? 'bg-primary/5 border-l-4 border-l-primary' : 'hover:bg-gray-50 dark:hover:bg-surface-highlight/30',
                  isTrackSelected(track.id) ? 'bg-accent-soft' : '',
                  (virtualStartIndex + index) % 2 === 1 ? 'bg-surface/30' : '',
                  highlightedTrackId === track.id ? 'bg-primary/10 ring-1 ring-inset ring-primary' : ''
                ]"
              >
                <!-- Checkbox Cell -->
                <div class="track-cell w-10 flex justify-center shrink-0">
                  <input type="checkbox" :checked="isTrackSelected(track.id)" @click.stop="toggleTrackSelection(track.id)" class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-gray-300 dark:border-gray-600">
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
                  <button type="button" class="text-xs text-primary text-left" :aria-label="`Show known origins for ${track.title}`" @click.stop="provenanceTrack = track">Known origins</button>
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

              <!-- DL (Effective Provider or one of the five download states, L14 #109) -->
              <div
                v-else-if="col.id === 'dl'"
                :class="['track-cell w-14 text-center shrink-0 flex items-center justify-center', col.hideBreakpoint || '']"
                data-testid="track-download-badge"
                :data-download-status="track.downloadStatus"
                :title="downloadStatusLabel(track.downloadStatus)"
              >
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
                <template v-else-if="track.downloadStatus === 'downloading'">
                  <span class="material-symbols-outlined text-[18px] text-primary animate-spin" :title="downloadStatusLabel('downloading')">progress_activity</span>
                </template>
                <template v-else-if="track.downloadStatus === 'queued'">
                  <span class="material-symbols-outlined text-[18px] text-amber-500" :title="downloadStatusLabel('queued')">schedule</span>
                </template>
                <template v-else-if="track.downloadStatus === 'failed'">
                  <span class="material-symbols-outlined text-[18px] text-error" :title="downloadStatusLabel('failed')">error</span>
                </template>
                <template v-else-if="track.downloadStatus === 'stale'">
                  <span class="material-symbols-outlined text-[18px] text-warning" title="File stale or missing">warning</span>
                </template>
                <template v-else>
                  <span class="text-xs text-gray-400 font-mono" :title="downloadStatusLabel('not_downloaded')">—</span>
                </template>
              </div>

              <!-- Quality: effective file quality when downloaded (L14 #109) -->
              <div v-else-if="col.id === 'quality'" :class="['track-cell w-20 text-center shrink-0 flex items-center justify-center', col.hideBreakpoint || '']">
                <span 
                  v-if="track.downloadStatus === 'downloaded' && effectiveQuality(track) !== '—'"
                  :class="['text-[11px] font-medium tracking-wide px-2 py-0.5 rounded-full border', getQualityStyle(effectiveQuality(track))]"
                  data-testid="track-quality-badge"
                  :data-quality="effectiveQuality(track)"
                  :title="`Effective quality: ${effectiveQuality(track)}`"
                >
                  {{ effectiveQuality(track) }}
                </span>
                <span v-else class="text-xs text-gray-400 font-mono" title="Not downloaded">—</span>
              </div>
            </template>

            <!-- Actions Cell -->
            <!-- S194: fixed row actions (always visible, dimmed when idle) —
                 the hover-only floating buttons rendered unreliably. -->
            <div class="track-cell track-actions w-24 shrink-0 flex justify-end items-center gap-1">
              <button @click.stop="openTrackDetail(track.id)" class="p-1 text-gray-400 opacity-60 hover:text-primary hover:opacity-100 transition-all" title="Track detail" :data-testid="`track-detail-open-${track.id}`"><span class="material-symbols-outlined text-[18px]">side_pane</span></button>
              <button
                @click.stop="handleToggleFavorite(track)"
                :class="[
                  'p-1 transition-all rounded hover:bg-gray-100 dark:hover:bg-surface-highlight',
                  track.isFavorite
                    ? 'text-red-500 hover:text-red-600'
                    : 'text-gray-400 opacity-60 hover:text-red-400 hover:opacity-100'
                ]"
                :title="track.isFavorite ? 'Remove from local favorites' : 'Add to local favorites'"
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
              <div v-else-if="!hasMore && tracks.length > 0 && virtualEndIndex >= tracks.length" class="py-4 text-center text-sm text-text-secondary">
                Showing all {{ totalTracks.toLocaleString() }} tracks
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- GRID VIEW (L13, #110): virtualizado por filas de baldosas. Solo se
           renderiza la ventana visible con overscan; el alto de fila se deriva
           del ancho de baldosa para no depender del layout del navegador. -->
      <template v-else-if="viewMode === 'grid' && groupBy === 'none'">
        <div
          ref="gridContainerRef"
          class="library-grid flex-1 min-h-0 overflow-y-auto custom-scrollbar border-x border-b border-gray-200 dark:border-border-dark rounded-b-xl bg-white dark:bg-surface-dark relative"
          style="contain: strict;"
          data-testid="library-grid"
          @scroll="handleGridScroll"
        >
          <div :style="{ height: totalGridHeight + 'px', position: 'relative', width: '100%' }">
            <div :style="{ transform: `translateY(${gridOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
              <div
                v-for="(gridRow, gridRowIndex) in visibleGridRows"
                :key="gridRowIndex"
                class="grid-row flex gap-5 px-1"
                :style="{ height: (gridTileSize + GRID_INFO_PX) + 'px', marginBottom: GRID_GAP_PX + 'px' }"
              >
                <div
                  v-for="album in gridRow"
                  :key="album.key"
                  @click="handleAlbumClick(album.albumId || album.id)"
                  :class="[
                    'grid-tile group bg-white dark:bg-surface-dark rounded-xl overflow-hidden shadow-md hover:shadow-xl transition-all duration-300 cursor-pointer hover:-translate-y-1 shrink-0',
                    album.isSelected ? 'ring-3 ring-primary' : ''
                  ]"
                  :style="{ width: gridTileSize + 'px', height: (gridTileSize + GRID_INFO_PX) + 'px' }"
                >
                  <!-- Image Area -->
                  <div class="tile-image relative overflow-hidden bg-gray-200 dark:bg-gray-800" :style="{ height: gridTileSize + 'px' }">
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

                  <!-- Info Area: altura fija para que la ventana virtual sea exacta -->
                  <div class="tile-info p-3 space-y-1 overflow-hidden" :style="{ height: GRID_INFO_PX + 'px' }">
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
          </div>
        </div>
      </template>

      <!-- GROUPED VIEWS (L13, #110): una sola rama para artista/álbum/género/
           calidad. Las filas se aplanan en cabeceras y pistas con altura fija
           por tipo y se virtualizan con el kernel variable: solo se pinta la
           ventana visible. El orden de los grupos es el del servidor (sin
           comparadores en JavaScript). -->
      <template v-else-if="groupBy !== 'none'">
        <div
          ref="groupContainerRef"
          class="flex-1 min-h-0 overflow-y-auto custom-scrollbar border-x border-b border-gray-200 dark:border-border-dark rounded-b-xl bg-white dark:bg-surface-dark relative"
          style="contain: strict;"
          data-testid="library-groups"
          @scroll="handleGroupScroll"
        >
          <div :style="{ height: totalGroupHeight + 'px', position: 'relative', width: '100%' }">
            <div :style="{ transform: `translateY(${groupOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
              <div v-for="row in visibleGroupRows" :key="row.id" :style="{ height: row.height + 'px' }" class="box-border">
                <!-- Group header -->
                <div
                  v-if="row.rowType === 'header' && row.header"
                  @click="toggleGroupExpand(row.groupKey)"
                  class="group-header h-full flex items-center gap-4 px-4 border-b border-gray-700 dark:border-border-dark cursor-pointer hover:bg-elevated transition-colors"
                >
                  <template v-if="row.groupKind === 'artist'">
                    <div class="w-12 h-12 rounded-full bg-gradient-to-br from-primary to-purple-500 flex items-center justify-center text-white font-bold text-lg shrink-0">
                      {{ row.header.label.charAt(0) }}
                    </div>
                  </template>
                  <template v-else-if="row.groupKind === 'album' && row.header.album">
                    <div :class="['w-12 h-12 rounded-lg shrink-0 overflow-hidden', !row.header.album.coverUrl && row.header.album.artGradient]">
                      <img v-if="row.header.album.coverUrl" :src="row.header.album.coverUrl" :alt="row.header.label" class="w-full h-full object-cover" loading="lazy" />
                    </div>
                  </template>
                  <template v-else-if="row.groupKind === 'genre'">
                    <div class="w-12 h-12 rounded-full bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-white font-bold shrink-0">
                      <span class="material-symbols-outlined text-[22px]">category</span>
                    </div>
                  </template>
                  <template v-else>
                    <div :class="['w-12 h-12 rounded-full flex items-center justify-center text-white font-bold text-[10px] shrink-0', getQualityStyle(row.header.label)]">
                      {{ row.header.label }}
                    </div>
                  </template>

                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2">
                      <h3 class="text-base font-bold text-ink truncate">{{ row.header.label }}</h3>
                      <button
                        v-if="row.groupKind === 'album' && row.header.album?.albumId"
                        @click.stop="handleAlbumClick(row.header.album?.albumId)"
                        class="px-2 py-0.5 rounded bg-primary/20 hover:bg-primary/30 text-primary text-xs font-medium flex items-center gap-1 transition-colors"
                        title="View Album Details"
                      >
                        <span>View Album</span>
                        <span class="material-symbols-outlined text-[14px]">arrow_forward</span>
                      </button>
                    </div>
                    <p class="text-xs text-text-secondary truncate">
                      <template v-if="row.groupKind === 'album' && row.header.album">
                        {{ row.header.album.artist }} · {{ row.header.album.trackCount }} track{{ row.header.album.trackCount !== 1 ? 's' : '' }}
                      </template>
                      <template v-else>{{ row.header.tracks.length }} track{{ row.header.tracks.length !== 1 ? 's' : '' }}</template>
                    </p>
                  </div>
                  <span v-if="row.groupKind === 'album' && row.header.album" :class="['px-2 py-1 rounded text-xs font-bold', getQualityBadgeStyle(row.header.album.quality)]">{{ row.header.album.quality }}</span>
                  <span :class="['material-symbols-outlined text-gray-400 transition-transform', expandedGroups.includes(row.groupKey) ? 'rotate-180' : '']">expand_more</span>
                </div>

                <!-- Track row inside an expanded group -->
                <div
                  v-else-if="row.track"
                  :data-track-id="row.track.id"
                  :class="['group-track h-full flex items-center gap-3 px-4 border-b border-gray-100 dark:border-border-dark/50 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 cursor-pointer group', highlightedTrackId === row.track.id ? 'bg-primary/10' : '']"
                  @click="handleTrackClick(row.track)"
                  @contextmenu.prevent="openContextMenu($event, row.track)"
                >
                  <span class="w-6 text-center text-xs text-gray-400 group-hover:text-primary font-medium">{{ (row.trackIndex ?? 0) + 1 }}</span>
                  <TrackCover
                    v-if="row.groupKind !== 'album'"
                    :src="row.track.coverUrl"
                    :item-id="row.track.id"
                    :alt="row.track.album || row.track.title"
                    size-class="w-10 h-10"
                    rounded="rounded"
                  />
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium text-ink truncate">{{ row.track.title }}</p>
                    <p class="text-xs text-text-secondary truncate">{{ row.track.artist }} · {{ row.track.album }}</p>
                  </div>
                  <span
                    class="shrink-0"
                    data-testid="track-download-badge"
                    :data-download-status="row.track.downloadStatus"
                    :title="downloadStatusLabel(row.track.downloadStatus)"
                  >
                    <span v-if="row.track.downloadStatus === 'downloaded'" class="material-symbols-outlined text-[16px] text-success">check_circle</span>
                    <span v-else-if="row.track.downloadStatus === 'downloading'" class="material-symbols-outlined text-[16px] text-primary animate-spin">progress_activity</span>
                    <span v-else-if="row.track.downloadStatus === 'queued'" class="material-symbols-outlined text-[16px] text-amber-500">schedule</span>
                    <span v-else-if="row.track.downloadStatus === 'failed'" class="material-symbols-outlined text-[16px] text-error">error</span>
                    <span v-else class="text-xs text-gray-400 font-mono">—</span>
                  </span>
                  <span class="text-xs text-text-secondary font-mono">{{ row.track.duration }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Existing summary is service-level only; account/collection provenance needs a new backend contract. -->
      <div v-if="provenanceTrack" role="dialog" aria-label="Known track origins" class="fixed inset-0 z-50 flex items-center justify-center bg-black/60" @click.self="provenanceTrack = null">
        <div class="rounded-xl bg-white dark:bg-surface-dark p-6 max-w-md w-full space-y-3 text-ink">
          <div class="flex justify-between gap-4"><h2 class="font-semibold">Known origins · {{ provenanceTrack.title }}</h2><button type="button" aria-label="Close known origins" @click="provenanceTrack = null">Close</button></div>
          <p class="text-xs text-text-secondary">Service-level summary only. Accounts, collections and multiple editions are not available from this library response; no single origin is implied.</p>
          <p v-if="provenanceTrack.importedFrom">Imported from: {{ provenanceTrack.importedFrom }}</p>
          <p v-if="provenanceTrack.downloadedFrom">Downloaded from: {{ provenanceTrack.downloadedFrom }}</p>
          <p v-if="provenanceTrack.services.length">Services: {{ provenanceTrack.services.join(', ') }}</p>
          <p v-else>No service origins recorded.</p>
        </div>
      </div>

      <!-- Download Favorites Modal -->
      <DownloadFavoritesModal
        v-model="showDownloadFavoritesModal"
        @enqueued="handleFavoritesEnqueued"
      />

      <!-- L04 (#101): panel lateral de detalle de pista. Solo lectura de
           metadatos; una sola consulta al abrirse (get_library_track_detail).
           Escape, el fondo o un cambio de ruta lo cierran. Las secciones de
           cuarentena y duplicados van deshabilitadas con su motivo mientras
           #72/#73 no existan: no se simulan. La edición de metadatos vive en
           /metadata («Abrir ficha»). -->
      <div v-if="detailTrackId !== null" role="dialog" aria-label="Track detail" class="fixed inset-0 z-50 bg-black/40" data-testid="track-detail-backdrop" @click.self="closeTrackDetail">
        <aside class="absolute right-0 top-0 h-full w-[400px] max-w-full bg-white dark:bg-surface-dark border-l border-gray-200 dark:border-border-dark shadow-xl overflow-y-auto custom-scrollbar text-ink" data-testid="track-detail-panel">
          <div class="sticky top-0 z-10 bg-white dark:bg-surface-dark border-b border-gray-200 dark:border-border-dark px-4 py-3 flex items-start justify-between gap-2">
            <div class="min-w-0">
              <h2 class="font-semibold text-sm truncate" data-testid="track-detail-title">{{ detailData?.track.display_title || detailData?.track.title || 'Track detail' }}</h2>
              <p class="text-xs text-text-secondary truncate">{{ detailData?.track.artist_name || 'Unknown Artist' }} · {{ detailData?.track.album_name || 'Unknown Album' }}</p>
            </div>
            <button type="button" class="p-1 text-text-secondary hover:text-ink transition-colors" data-testid="track-detail-close" aria-label="Close track detail" @click="closeTrackDetail">
              <span class="material-symbols-outlined text-[18px]">close</span>
            </button>
          </div>

          <div v-if="detailLoading" class="px-4 py-8 flex items-center justify-center gap-2 text-text-secondary" data-testid="track-detail-loading">
            <span class="material-symbols-outlined text-[20px] animate-spin">progress_activity</span>
            <span class="text-sm">Loading detail…</span>
          </div>

          <!-- Pista inexistente: error tipado del backend, no una fila vacía. -->
          <div v-else-if="detailError" class="px-4 py-6 space-y-1" data-testid="track-detail-error">
            <p class="text-sm font-medium text-error">{{ detailError.kind === 'track_not_found' ? 'Track not found' : 'Could not load track detail' }}</p>
            <p class="text-xs text-text-secondary">{{ detailError.kind === 'track_not_found' ? `No track with id ${detailError.trackId}.` : detailError.message }}</p>
          </div>

          <template v-else-if="detailData">
            <!-- Fuentes por servicio: formato, profundidad, frecuencia, bitrate y disponibilidad -->
            <section class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50" data-testid="track-detail-sources">
              <h3 class="text-[11px] font-bold uppercase tracking-wider text-text-secondary mb-2">Sources</h3>
              <p v-if="detailData.sources.length === 0" class="text-xs text-gray-400 italic">No service sources recorded.</p>
              <ul v-else class="space-y-1.5">
                <li v-for="source in detailData.sources" :key="source.serviceId" class="flex items-center justify-between gap-2 text-xs">
                  <span class="flex items-center gap-1.5 min-w-0">
                    <span :class="['w-4 h-4 rounded-full flex items-center justify-center text-[8px] font-bold border shrink-0', getServiceStyle(source.serviceName)]">{{ getServiceIcon(source.serviceName) }}</span>
                    <span class="font-medium truncate">{{ source.serviceName }}</span>
                  </span>
                  <span class="text-text-secondary text-right" :title="source.availabilityReason ?? undefined">
                    {{ [source.format, source.bitDepth && source.sampleRate ? `${source.bitDepth}/${source.sampleRate}` : null, source.bitrate ? `${source.bitrate}kbps` : null].filter(Boolean).join(' · ') || '—' }}
                    <span :class="source.available ? 'text-success' : 'text-warning'">{{ source.available ? 'available' : source.availabilityStatus }}</span>
                  </span>
                </li>
              </ul>
            </section>

            <!-- Cuentas y pertenencias por colección -->
            <section class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50" data-testid="track-detail-memberships">
              <h3 class="text-[11px] font-bold uppercase tracking-wider text-text-secondary mb-2">Accounts &amp; collections</h3>
              <p v-if="detailData.memberships.length === 0" class="text-xs text-gray-400 italic">No library memberships recorded.</p>
              <ul v-else class="space-y-1.5">
                <li v-for="(membership, idx) in detailData.memberships" :key="`${membership.accountId}-${membership.collection}-${membership.collectionId ?? 'fav'}-${idx}`" class="flex items-center justify-between gap-2 text-xs">
                  <span class="truncate">{{ membership.displayName || `Account ${membership.accountId}` }} <span class="text-text-secondary">({{ membership.serviceName }})</span></span>
                  <span class="text-text-secondary text-right shrink-0">{{ membership.collection === 'playlist' ? `playlist: ${membership.collectionName ?? ''}` : 'favorite' }}</span>
                </li>
              </ul>
            </section>

            <!-- Descarga: estado, servicio efectivo, calidad y ruta -->
            <section class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50" data-testid="track-detail-download">
              <h3 class="text-[11px] font-bold uppercase tracking-wider text-text-secondary mb-2">Download</h3>
              <p v-if="!detailData.download" class="text-xs text-gray-400 italic">Not downloaded.</p>
              <dl v-else class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs">
                <dt class="text-text-secondary">Status</dt>
                <dd data-testid="track-detail-download-status">{{ detailData.download.status }}</dd>
                <template v-if="detailData.download.effectiveService">
                  <dt class="text-text-secondary">Service</dt>
                  <dd>{{ detailData.download.effectiveService }}</dd>
                </template>
                <template v-if="detailDownloadQuality(detailData.download)">
                  <dt class="text-text-secondary">Quality</dt>
                  <dd data-testid="track-detail-download-quality">{{ detailDownloadQuality(detailData.download) }}</dd>
                </template>
                <template v-if="detailData.download.filePath">
                  <dt class="text-text-secondary">Path</dt>
                  <dd class="break-all font-mono text-[10px]" data-testid="track-detail-download-path">{{ detailData.download.filePath }}</dd>
                </template>
              </dl>
            </section>

            <!-- Facetas deshabilitadas con su motivo (#72/#73): no se simulan -->
            <section class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 opacity-60" data-testid="track-detail-quarantine-disabled">
              <h3 class="text-[11px] font-bold uppercase tracking-wider text-text-secondary mb-1">Quarantine</h3>
              <p class="text-xs text-gray-400 italic">Unavailable: sync quarantine (reason and last seen) arrives with #72/L09.</p>
            </section>
            <section class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 opacity-60" data-testid="track-detail-duplicates-disabled">
              <h3 class="text-[11px] font-bold uppercase tracking-wider text-text-secondary mb-1">Duplicates</h3>
              <p class="text-xs text-gray-400 italic">Unavailable: ISRC + fingerprint identity arrives with #73/L10.</p>
            </section>

            <!-- Acciones que no editan la ficha -->
            <section class="px-4 py-3 space-y-2" data-testid="track-detail-actions">
              <div class="grid grid-cols-2 gap-2">
                <button type="button" class="px-3 py-1.5 rounded-lg text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors flex items-center justify-center gap-1" data-testid="track-detail-play" @click="panelTrack && handleTrackPlay(panelTrack)">
                  <span class="material-symbols-outlined text-[16px]">play_arrow</span> Play
                </button>
                <button type="button" class="px-3 py-1.5 rounded-lg text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors flex items-center justify-center gap-1" data-testid="track-detail-download" @click="panelTrack && handlePanelDownload(panelTrack, panelHasDownload)">
                  <span class="material-symbols-outlined text-[16px]">download</span> {{ panelHasDownload ? 'Re-download' : 'Download' }}
                </button>
                <button type="button" class="px-3 py-1.5 rounded-lg text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors flex items-center justify-center gap-1" data-testid="track-detail-favorite" @click="panelFavorite">
                  <span :class="['material-symbols-outlined text-[16px]', panelTrack?.isFavorite ? 'material-symbols-filled text-red-500' : '']">{{ panelTrack?.isFavorite ? 'favorite' : 'favorite_border' }}</span> {{ panelTrack?.isFavorite ? 'Unfavorite' : 'Favorite' }}
                </button>
                <button type="button" class="px-3 py-1.5 rounded-lg text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors flex items-center justify-center gap-1" data-testid="track-detail-open-metadata" @click="panelTrack && handleShowMetadata(panelTrack)">
                  <span class="material-symbols-outlined text-[16px]">open_in_new</span> Abrir ficha
                </button>
              </div>
              <div class="flex items-center gap-2">
                <select v-model="detailPlaylistChoice" class="flex-1 min-w-0 text-xs rounded-lg border border-gray-300 dark:border-gray-600 bg-gray-100 dark:bg-surface-highlight px-2 py-1.5 text-ink" data-testid="track-detail-playlist-select">
                  <option :value="null" disabled>Add to playlist…</option>
                  <option v-for="playlist in playlists" :key="playlist.id" :value="playlist.id">{{ playlist.name }}</option>
                </select>
                <button type="button" class="px-3 py-1.5 rounded-lg text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors" data-testid="track-detail-playlist-add" :disabled="detailPlaylistChoice === null" @click="panelAddToPlaylist">Add</button>
              </div>
              <button type="button" class="w-full px-3 py-1.5 rounded-lg text-xs font-medium bg-error/10 text-error hover:bg-error/20 transition-colors flex items-center justify-center gap-1" data-testid="track-detail-delete-audio" @click="panelDeleteAudio">
                <span class="material-symbols-outlined text-[16px]">delete</span> Delete audio
              </button>
            </section>
          </template>
        </aside>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
// FE-PERF-01: nombre explícito — el `include` de <KeepAlive> (App.vue /
// keepAliveViews.ts) filtra por `name`, y debe coincidir exactamente.
defineOptions({ name: 'LibraryView' })
import { ref, computed, onMounted, onUnmounted, onActivated, onDeactivated, watch, nextTick } from 'vue'
import { useRouter, useRoute, type LocationQueryValue } from 'vue-router'
import { libraryApi, queryLibrary, reconcileQueue, importMbDump, getMbDumpImportState, type TrackDetail, type TrackDetailError, type MbDumpImportState, type MbDumpProgress }  from '@/api/library'
import type {
  LibraryQuery,
  LibraryQueryCounts,
  LibraryQueryResult,
  LibrarySortKey,
  DownloadStatusFilter,
  QualityBucketFilter,
  LibrarySelection,
  LibraryBatchAction,
  LibraryActionPreview,
  LibraryActionProgress,
  LibraryActionSummary,
} from '@/api/types'
import { addToQueue, addBatchToQueue, enqueueEligibleBatch, queueApi } from '@/api/queue'
import { playlistsApi } from '@/api/playlists'
import { metadataApi } from '@/api/metadata'
import { settingsApi } from '@/api/settings'
import type { LibraryTrack, Playlist, LyricsType, QueueStats, WorkerStatus, EffectiveDownloadPreferences } from '@/api/types'
import { useToast } from '@/composables/useToast'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { useGlobalTasks } from '@/composables/useGlobalTasks'
import { usePlayer } from '@/composables/usePlayer'
import { useVirtualWindow } from '@/composables/useVirtualWindow'
import { useTrackBrowser } from '@/composables/useTrackBrowser'
import { pageQueryKey, usePagedQueryCache } from '@/composables/usePagedQueryCache'
import { invalidateLibraryPages, onLibraryPagesInvalidated } from '@/composables/libraryCacheInvalidation'
import { useLibraryCacheInvalidation } from '@/composables/useLibraryCacheInvalidation'
import { useEnrichmentProgress } from '@/composables/useEnrichmentProgress'
import {
  buildLibraryUrlQuery,
  parseLibraryUrlQuery,
  libraryTabQuery,
  LIBRARY_URL_KEYS,
  QUALITY_BUCKET_LABELS,
  type LibraryUrlState,
} from '@/composables/libraryUrlState'
import LibraryTabs from '@/components/LibraryTabs.vue'
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

function handleFavoritesEnqueued(res: { message: string }) {
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

// State - will be updated from backend
const searchQuery = ref('')
let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null

const sortBy = ref('dateAdded')
const sortDirection = ref<'asc' | 'desc'>('desc')

// Metadata enrichment
const isEnriching = ref(false)
const {
  progress: enrichProgress,
  start: startEnrichmentProgress,
  stop: stopEnrichmentProgress,
} = useEnrichmentProgress()

async function enrichMetadata() {
  if (isEnriching.value) return

  isEnriching.value = true
  await startEnrichmentProgress()

  try {
    const result = await metadataApi.enrichMetadataMusicBrainz()

    if (result.enriched > 0) {
      await loadLibrary()
    }
  } catch (error) {
    console.error('Failed to enrich metadata:', error)
  } finally {
    stopEnrichmentProgress()
    isEnriching.value = false
  }
}

// View State
const viewMode = ref<'list' | 'grid'>('list')
// El tipo 'year' murió sin uso (sin dropdown, computed ni template): fuera.
const groupBy = ref<'none' | 'artist' | 'album' | 'genre' | 'quality'>('none')

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
// L01 (#98): los pills se traducen a dimensiones de LibraryQuery. Las piezas
// sin semántica en el contrato quedan expuestas DESHABILITADAS con su motivo,
// no eliminadas ni inventadas: «Multi-Source» no es dimensión del contrato
// (la cabecera con badges de servicio llega con L03) y «Duplicates» espera la
// identidad ISRC/huella de L10 (#73).
const filterPills: Array<{ id: string; label: string; disabled?: boolean; reason?: string }> = [
  { id: 'all', label: 'All Items' },
  // L14 (#109): los cinco estados del badge, cada uno filtrable. «Not
  // Downloaded» es la ausencia de descarga y de cola, no un cajón que mezcle
  // los otros cuatro estados.
  { id: 'downloaded', label: 'Downloaded' },
  { id: 'queued', label: 'Queued' },
  { id: 'downloading', label: 'Downloading' },
  { id: 'failed', label: 'Failed' },
  { id: 'notDownloaded', label: 'Not Downloaded' },
  { id: 'favorites', label: 'Favorites' },
  { id: 'flac', label: 'Hi-Res (FLAC)' },
  { id: 'mp3', label: 'Lossy (MP3)' },
  { id: 'multiSource', label: 'Multi-Source', disabled: true, reason: 'Not available: the server query (L01) has no multi-source dimension; service badges arrive with L03' },
  { id: 'duplicates', label: 'Duplicates', disabled: true, reason: 'Not available: duplicates arrive with L10 (#73) over ISRC/fingerprint identity' },
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
  /** L14 (#109): calidad efectiva del fichero descargado (p. ej. "FLAC 24/96"). */
  downloadQuality: string | null
  downloadStatus: 'downloaded' | 'queued' | 'downloading' | 'failed' | 'stale' | 'not_downloaded'
  metadataScore: number
  bpm: number | null
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
  // Route 2 (2026-10-18): identifies the favorited release when the same track
  // appears once per release in the expanded favorites list.
  editionItemId?: string | null
}

// Las filas viven en el composable (`tracks` es alias de `browser.rows`).
const provenanceTrack = ref<Track | null>(null)

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
    downloadQuality: item.download_quality ?? null,
    downloadStatus: (item.download_status ?? 'not_downloaded') as Track['downloadStatus'],
    metadataScore: item.metadata_score ?? 0,
    bpm: item.bpm,
    lyricsType: (item.lyrics_type ?? 'none') as Track['lyricsType'],
    spotifyTrackId: item.spotify_track_id ?? null,
    genre: item.genre || null,
    filePath: item.file_path || null,
    displayTitle: item.display_title || null,
    sourceTitle: item.source_title || item.title,
    fileDisambiguator: item.file_disambiguator || null,
    duration: `${mins}:${secs.toString().padStart(2, '0')}`,
    isFavorite: !!item.is_favorite,
    isPlaying: false,
    editionItemId: item.edition_item_id ?? null,
  };
}

// L01: una fila por tracks.id — la expansión por release de favoritos
// desapareció de la vista con get_favorite_tracks; el id basta de clave.
function trackRowKey(track: Track): string {
  return String(track.id)
}

// ==============================================
// Navegador de pistas (ADR-0025, decisiones 2 y 3): la vista declara la fuente
// (triple fuente según filtros + búsqueda FTS), el mapeo y el tamaño de
// página; el composable posee la paginación, los resultados de búsqueda y la
// selección por ids. Debajo, los nombres históricos quedan como alias para que
// el resto del script y el template no cambien.
// ==============================================
const PAGE_SIZE = 100;
const SEARCH_PAGE_SIZE = 100;

// L01 (#98): fuente ÚNICA. query_library resuelve en servidor filtros, orden,
// búsqueda y contadores; el renderer deja de filtrar, ordenar o sustituir la
// lista por una búsqueda.

// L02 (#99): el token de secuencia de la consulta vive en `useTrackBrowser`
// (load/loadMore) desde L13 (#110). La caché de páginas deduplica peticiones
// simultáneas de la misma clave: dos cargas seguidas de la misma consulta
// comparten una sola consulta al servidor.

/** Caché de páginas por clave de consulta (L13, #110). Sin TTL: los eventos
 * invalidan (sync, importación, favorito, playlist, descarga, metadatos,
 * cuarentena). Una página ya leída no vuelve a pedirse. */
const libraryPages = usePagedQueryCache<LibraryQuery, LibraryQueryResult>({
  keyOf: pageQueryKey,
  fetchPage: (query) => libraryApi.queryLibrary(query),
})

const libraryCounts = ref<LibraryQueryCounts | null>(null);

async function fetchLibraryPage(offset: number, limit: number): Promise<LibraryQueryResult> {
  const query = buildLibraryQuery(offset, limit)
  const requestKey = pageQueryKey(query)
  const result = await libraryPages.load(query)
  // Contadores: solo de la primera página, solo si la clave pedida sigue
  // siendo la del estado actual y solo si la generación no fue invalidada.
  // Al paginar (offset > 0) no se recalculan: la cabecera conserva los de la
  // consulta.
  if (
    offset === 0 &&
    result.page.counts &&
    result.generation === libraryPages.generation.value &&
    requestKey === pageQueryKey(buildLibraryQuery(0, limit))
  ) {
    libraryCounts.value = result.page.counts;
  }
  return result.page
}

// ==============================================
// L03 (#100): estado de la vista → LibraryQuery y → URL
// ==============================================
// UNA traducción compartida: la consulta (buildLibraryQuery) y la URL
// (syncUrlFromView) leen el mismo estado, para que no puedan divergir. Las
// dimensiones del contrato que la vista todavía no dirige (servicios,
// cuentas, colecciones además de favoritos, «en playlist», cuarentena,
// género) no tienen ni control ni contador: no se inventan.

/** Estado gestionado por la vista, en la forma del contrato LibraryQuery. */
function currentLibraryState(): LibraryUrlState {
  const filters = activeFilters.value;
  // OR dentro de la dimensión de estado de descarga.
  const downloadStatuses: DownloadStatusFilter[] = [];
  if (filters.includes('downloaded')) downloadStatuses.push('downloaded');
  if (filters.includes('queued')) downloadStatuses.push('queued');
  if (filters.includes('downloading')) downloadStatuses.push('downloading');
  if (filters.includes('failed')) downloadStatuses.push('failed');
  if (filters.includes('notDownloaded')) downloadStatuses.push('not_downloaded');
  // OR dentro de la dimensión de calidad (buckets del servidor, mismas
  // etiquetas que el deep-link FE-10 del dashboard). Orden canónico del
  // contrato para que el round-trip URL → vista → URL sea estable.
  let qualityBuckets: QualityBucketFilter[] = [];
  if (filters.includes('flac')) qualityBuckets.push('hi_res', 'cd_quality');
  if (filters.includes('mp3')) qualityBuckets.push('lossy');
  if (filters.includes('quality') && qualityFilter.value) {
    for (const label of qualityFilter.value.split(', ')) {
      const bucket = (Object.entries(QUALITY_BUCKET_LABELS) as Array<[QualityBucketFilter, string]>)
        .find(([, l]) => l === label)?.[0];
      if (bucket && !qualityBuckets.includes(bucket)) qualityBuckets.push(bucket);
    }
  }
  qualityBuckets = (['hi_res', 'cd_quality', 'lossy'] as QualityBucketFilter[]).filter((b) =>
    qualityBuckets.includes(b)
  );
  return {
    text: searchQuery.value.trim(),
    sort: sortBy.value as LibrarySortKey,
    direction: sortDirection.value,
    downloadStatuses,
    qualityBuckets,
    favoritesOnly: filters.includes('favorites'),
  };
}

/** Contrato LibraryQuery: el estado de la vista se traduce a dimensiones. */
function buildLibraryQuery(offset: number, limit: number | null): LibraryQuery {
  const state = currentLibraryState();
  return {
    text: state.text || null,
    serviceIds: [],
    accountIds: [],
    collections: [],
    favoritesOnly: state.favoritesOnly,
    downloadStatuses: state.downloadStatuses,
    qualityBuckets: state.qualityBuckets,
    inPlaylists: null,
    duplicates: null,
    quarantine: 'exclude',
    genre: [],
    sort: state.sort,
    direction: state.direction,
    offset,
    limit,
  };
}

// ---------------------------------------------
// Estado de la vista EN LA URL (deep-linking).
// La URL es la fuente al montar y al volver atrás (atrás restaura lista y
// filtro); la vista escribe en ella en cada cambio — `push` en cambios
// discretos (filtros, orden) para que atrás recorra estados, `replace` en
// el texto para no inundar el historial por tecla. `lastWrittenUrl`
// recuerda la última serialización aplicada o escrita: los cambios propios
// no se re-aplican (guard del watcher de ruta).
// ---------------------------------------------
let lastWrittenUrl: string | null = null;
let suppressSearchWatcher = false;
let suppressSortWatcher = false;

/** Query canónico para el estado actual: claves gestionadas reemplazadas,
 * legacy FE-10 (`filter=`) canonicalizado, params ajenos (p. ej. `track`)
 * conservados. */
function canonicalQuery(state: LibraryUrlState): Record<string, unknown> {
  const query: Record<string, unknown> = { ...(route?.query ?? {}) };
  for (const key of Object.values(LIBRARY_URL_KEYS)) delete query[key];
  delete query.filter;
  Object.assign(query, buildLibraryUrlQuery(state));
  return query;
}

/** Escribe el estado actual en la URL. No-op si ya coincide. */
function syncUrlFromView(push: boolean) {
  if (!router) return;
  const query = canonicalQuery(currentLibraryState());
  const serialized = JSON.stringify(query);
  if (serialized === lastWrittenUrl) return;
  lastWrittenUrl = serialized;
  const navigate = push ? router.push : router.replace;
  void navigate.call(router, { query: query as Record<string, string> }).catch(() => {
    // Navegación redundante o a la misma URL: inofensiva.
  });
}

/**
 * URL → estado de la vista. Devuelve si algo cambió (y por tanto toca
 * recargar). Asignaciones solo con diff real: si nada cambia, ningún
 * watcher se dispara y no hay recarga (un back a la misma vista no relanza
 * la consulta). Las dimensiones pendientes que llegan por deep-link se
 * reportan y NO se aplican.
 */
function applyUrlState(): boolean {
  const parsed = parseLibraryUrlQuery(route?.query as Record<string, unknown> | undefined);
  const state = parsed.state;
  lastWrittenUrl = JSON.stringify(canonicalQuery(state));

  let changed = false;
  if (searchQuery.value !== state.text) {
    suppressSearchWatcher = true;
    searchQuery.value = state.text;
    // Un debounce en vuelo del texto anterior pediría dos veces: se cancela.
    if (searchDebounceTimer) {
      clearTimeout(searchDebounceTimer);
      searchDebounceTimer = null;
    }
    changed = true;
  }
  if (sortBy.value !== state.sort || sortDirection.value !== state.direction) {
    suppressSortWatcher = true;
    sortBy.value = state.sort;
    sortDirection.value = state.direction;
    changed = true;
  }
  const previousQualityLabel = qualityFilter.value;
  const nextFilters = filtersFromState(state, parsed.legacyQualityLabel);
  if (nextFilters.join('|') !== activeFilters.value.join('|')) {
    activeFilters.value = nextFilters;
    changed = true;
  }
  // Un cambio de bucket del deep-link FE-10 cambia la representación
  // ('quality' + etiqueta) sin cambiar los ids de filtro: también recarga.
  if (qualityFilter.value !== previousQualityLabel) {
    changed = true;
  }
  if (parsed.unsupported.length) {
    console.warn('[library] deep-link params sin semántica todavía, ignorados:', parsed.unsupported.join(', '));
  }
  return changed;
}

/** Filtros (ids de píldora / 'quality') que representan el estado.
 * Con etiqueta legacy FE-10 se conserva SU representación exacta
 * ('quality' + etiqueta): colapsarla a la píldora 'flac' ampliaría el
 * filtro (flac = hi_res + cd_quality) y rompería el deep-link del
 * dashboard. Los buckets sin píldora exacta usan la misma vía. */
function filtersFromState(state: LibraryUrlState, legacyLabel: string | null): string[] {
  const filters: string[] = [];
  if (state.downloadStatuses.includes('downloaded')) filters.push('downloaded');
  if (state.downloadStatuses.includes('queued')) filters.push('queued');
  if (state.downloadStatuses.includes('downloading')) filters.push('downloading');
  if (state.downloadStatuses.includes('failed')) filters.push('failed');
  if (state.downloadStatuses.includes('not_downloaded')) filters.push('notDownloaded');
  if (state.favoritesOnly) filters.push('favorites');
  let buckets = state.qualityBuckets;
  if (legacyLabel !== null) {
    // FE-10 legacy con etiqueta: representación exacta, sin colapsar los
    // buckets a píldoras (los demás filtros de la URL se conservan).
    qualityFilter.value = legacyLabel;
    filters.push('quality');
    return filters;
  }
  if (buckets.includes('hi_res') && buckets.includes('cd_quality')) {
    filters.push('flac');
    buckets = buckets.filter((b) => b !== 'hi_res' && b !== 'cd_quality');
  }
  if (buckets.includes('lossy')) {
    filters.push('mp3');
    buckets = buckets.filter((b) => b !== 'lossy');
  }
  if (buckets.length) {
    filters.push('quality');
    qualityFilter.value = buckets.map((b) => QUALITY_BUCKET_LABELS[b]).join(', ');
  } else {
    qualityFilter.value = '';
  }
  return filters;
}

// ==============================================
// L03 (#100): cabecera con chips de estado (spec #96, historias 9-11)
// ==============================================
// El contador de cada chip se calcula en servidor sobre el filtro actual
// SIN la condición del propio chip (los contadores de L01 vienen con la
// primera página y la vista los conserva al paginar): el número del chip
// coincide con el total que se ve al aplicarlo. El chip Álbumes (L07, #104)
// cuenta álbumes distintos con pistas vivas y navega a su lista. Artistas,
// cuarentena y duplicados siguen deshabilitados con su motivo (#105, #72,
// #73) — el contador que no existe se muestra como '—', nunca inventado.
type HeaderChipId = 'albums' | 'artists' | 'downloaded' | 'quarantine' | 'duplicates';

interface HeaderChip {
  id: HeaderChipId;
  label: string;
  icon: string;
  disabled: boolean;
  reason: string;
  /** `null` = sin contador (pendiente o primera página aún en vuelo). */
  count: number | null;
  active: boolean;
}

const headerChips = computed<HeaderChip[]>(() => [
  {
    id: 'albums',
    label: 'Albums',
    icon: 'album',
    // L07 (#104): la pestaña de álbumes ya existe. El chip cuenta álbumes
    // distintos con al menos una pista viva en el filtro (contador del
    // servidor) y navega a la lista; no aplica una dimensión de pista.
    disabled: false,
    reason: 'Browse albums with at least one live track in the current filter',
    count: libraryCounts.value?.albums ?? null,
    active: false,
  },
  {
    id: 'artists',
    label: 'Artists',
    icon: 'artist',
    // L08 (#105): la pestaña de artistas ya existe. El chip cuenta artistas
    // distintos con al menos una pista viva en el filtro (contador del
    // servidor) y navega a la lista; no aplica una dimensión de pista.
    disabled: false,
    reason: 'Browse artists with at least one live track in the current filter',
    count: libraryCounts.value?.artists ?? null,
    active: false,
  },
  {
    id: 'downloaded',
    label: 'Downloaded',
    icon: 'download_done',
    disabled: false,
    reason: 'Filter downloaded tracks',
    count: libraryCounts.value?.downloaded ?? null,
    active: activeFilters.value.includes('downloaded'),
  },
  {
    id: 'quarantine',
    label: 'Quarantine',
    icon: 'rule',
    disabled: true,
    reason: 'Pending: #72 defines the quarantine table and L09 wires the library to it',
    count: null,
    active: false,
  },
  {
    id: 'duplicates',
    label: 'Duplicates',
    icon: 'content_copy',
    disabled: true,
    reason: 'Pending: #73 defines ISRC/fingerprint identity and L10 wires the duplicates view',
    count: null,
    active: false,
  },
]);

function toggleHeaderChip(chip: HeaderChip) {
  if (chip.disabled) return;
  if (chip.id === 'albums' || chip.id === 'artists') {
    // L07/L08: las pestañas comparten texto y orden con la vista de pistas;
    // el resto de dimensiones de pista no aplican a esas listas.
    router.push({
      path: chip.id === 'albums' ? '/library/albums' : '/library/artists',
      query: libraryTabQuery(chip.id, route.query),
    });
    return;
  }
  if (chip.id === 'downloaded') {
    toggleFilter('downloaded');
  }
}

const browser = useTrackBrowser<LibraryTrack, Track>({
  source: { kind: 'paged', load: fetchLibraryPage },
  mapRow: mapToTrack,
  pageSize: PAGE_SIZE,
});

const selection = browser.selection;
const tracks = browser.rows;
const totalTracks = browser.total;
const currentOffset = browser.currentOffset;
const hasMore = browser.hasMore;
const isLoadingMore = browser.isLoadingMore;

// L05 (#102): la selección tiene dos modos. En «todo el filtro» el contador
// sale del total del servidor (no de la página cargada) menos las exclusiones;
// en ids explícitos sale de los ids marcados. Nunca se mezclan.
const selectAllMatching = ref(false);
const excludedIds = ref<number[]>([]);
const excludedIdSet = computed(() => new Set(excludedIds.value));
const selectedCount = computed(() =>
  selectAllMatching.value
    ? Math.max(0, totalTracks.value - excludedIdSet.value.size)
    : selection.selectedCount.value
);
const allMatchingSelected = computed(
  () => selectAllMatching.value && excludedIds.value.length === 0
);

function isTrackSelected(id: number): boolean {
  return selectAllMatching.value ? !excludedIdSet.value.has(id) : selection.isSelected(id);
}

function toggleTrackSelection(id: number): void {
  if (!selectAllMatching.value) {
    selection.toggle(id);
    return;
  }
  const index = excludedIds.value.indexOf(id);
  if (index === -1) {
    excludedIds.value.push(id);
  } else {
    excludedIds.value.splice(index, 1);
  }
}


/** Failure of the last load; distinguishes "empty library" from "could not read". */
const isLoading = browser.isLoading;
const loadError = browser.loadError;

// L13 (#110): la vista sigue activa o cacheada por <KeepAlive>. La caché se
// invalida siempre que una mutación de la app cambia la consulta; si la vista
// está oculta, la recarga espera a onActivated (una sola consulta al volver).
let viewActive = true
let cacheDirty = false
useLibraryCacheInvalidation()
const offLibraryInvalidated = onLibraryPagesInvalidated(() => {
  libraryPages.invalidate()
  cacheDirty = true
  if (viewActive) {
    debouncedReloadLibrary()
  }
})

// Load tracks from backend
async function loadLibrary() {
  await browser.load();
  // Los flags de selección morían con las filas al recargar: por ids, la
  // recarga parte de selección vacía (solo con carga exitosa; el contador
  // derivado no puede quedar huérfano como antes quedaba el incremental).
  if (browser.loadError.value === null) {
    clearSelection();
    cacheDirty = false
  }
}

// Load more tracks (infinite scroll)
const loadMore = browser.loadMore;

// ==============================================
// VIRTUAL SCROLLING LOGIC FOR LIBRARY (23,000+ ROWS)
// La mecánica de ventana vive en el kernel compartido useVirtualWindow
// (ADR-0025); aquí queda la config congelada de esta vista (filas de 56 px,
// overscan 10, throttle de 120 ms, footer de fin de lista de 48 px).
// ==============================================
const ROW_HEIGHT = 56 // px per table row (40px cover + py-2 padding + 1px border)
const OVERSCAN = 10   // extra rows above and below visible area
const SCROLL_THROTTLE_MS = 120
// Reserva vertical del indicador "Showing all N tracks" al acabar la lista.
const END_OF_LIST_FOOTER_PX = 48

// ==============================================
// DOWNLOAD QUEUE STATS & CONCURRENCY (L-12)
// Réplica del panel de la cola de DownloadsView; la ventana virtual de esa
// vista ya no se duplica: vive en el kernel useVirtualWindow (ADR-0025).
// ==============================================
const queueStats = ref<QueueStats>({ total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 })
const workerStatus = ref<WorkerStatus>({ running: true, paused: false, active_downloads: 0, max_concurrent: 3 })
const showQueueDetails = ref(false)

const currentConcurrency = computed(() => {
  return workerStatus.value?.max_concurrent ?? 3
})

async function setConcurrency(threads: number) {
  try {
    // El backend expone `set_max_concurrent_downloads(max: usize)`; el wrapper
    // de la API es la unica via (el nombre anterior, `set_max_concurrency`, no
    // existia en Rust y el fallo se tragaba en el catch).
    await settingsApi.setMaxConcurrentDownloads(threads)
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

// Infinite scroll fires a burst of scroll events per flick (B-4). El kernel
// mide la distancia al fondo DENTRO del corte de throttle (PR-01/PR-02);
// aquí solo se decide si toca pedir la página siguiente.
function maybeLoadMore(scrollBottom: number) {
  if (scrollBottom < 300 && hasMore.value && !isLoadingMore.value) {
    loadMore();
  }
}

// Initialize data
loadColumnsOrder();
// L03 (#100): la URL es la fuente del estado inicial — se aplica ANTES de la
// primera carga para que el deep-link llegue a la primera consulta y no se
// pida dos veces. Los watchers aún no están registrados (van más abajo),
// así que los flags de supresión que applyUrlState deja puestos no los
// consume nadie y se limpian aquí.
applyUrlState();
suppressSearchWatcher = false;
suppressSortWatcher = false;
loadLibrary();

// L02 (#99): la búsqueda alimenta `query_library.text` (FTS con relevancia
// en servidor) y se combina con los filtros EN SERVIDOR. Espera 250 ms desde
// la ÚLTIMA tecla (una sola consulta por pausa, ninguna por pulsación) y el
// token de secuencia de fetchLibraryPage descarta la respuesta vieja.
// L03 (#100): el texto también vive en la URL — la carga del debounce es la
// que lo escribe (replace, sin entradas de historial por tecla). Si el texto
// llegó de la URL (applyUrlState), el flag suprime el debounce: la carga la
// hizo quien aplicó el estado.
const SEARCH_DEBOUNCE_MS = 250
watch(searchQuery, () => {
  if (suppressSearchWatcher) {
    suppressSearchWatcher = false
    return
  }
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
  searchDebounceTimer = setTimeout(() => {
    loadLibrary()
    syncUrlFromView(false)
  }, SEARCH_DEBOUNCE_MS)
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
    // L-02: the track was indeed added, but the user got no feedback.
    toast.success('Added to playlist', `"${track?.title ?? 'Track'}" added`);
  } catch (error) {
    console.error('Failed to add to playlist:', error);
    toast.error('Failed to add to playlist', String(error));
  }
}

// Track selection & downloads. La selección vive en el composable por ids
// (ADR-0025, decisión 3): el contador se deriva de los ids, no se lleva a mano.
function handleTrackClick(track: Track) {
  toggleTrackSelection(track.id);
}

const { play, playNext, enqueue, toggle: togglePlayback, current: playerCurrent } = usePlayer();

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

// Phase 1: "Add to Play Queue" appends the track to the END of the play queue
// (playNext inserts at the front); with the player stopped it starts playback.
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
      toast.success('Added to Play Queue', `"${track.title}" will play at the end of the queue`);
    }
  } catch (error: any) {
    console.error('Failed to add to play queue:', error);
    surfacePlayFailure(track, error);
  }
}

/**
 * A playback-start failure only touches player.error: with no track in progress
 * the bar does not render and the message would never reach the user (these
 * catches used to be silent). With current the bar already shows the error;
 * without it, a toast.
 */
function surfacePlayFailure(track: Track, error: unknown) {
  // L-01: the failure was silenced whenever something was playing, so the user
  // did not know why the track did not change. Now it always shows the real
  // cause and the next step.
  const message = String((error as { message?: string })?.message || error || 'Unknown error');
  const hasLocalFile = !!track.filePath;
  // La cascada de fuentes (ISRC + metadatos) es la misma para reproducir y
  // para descargar: si no encontró fuente, descargar falla igual. Decir sólo
  // "no está descargada" mandaba al usuario a un botón que repite el fallo.
  const noReproducibleSource = message.includes('No se pudo resolver ninguna fuente reproducible');
  const hint = hasLocalFile
    ? ''
    : noReproducibleSource
      ? ' No provider carries this recording, so downloading it would fail the same way.'
      : ' This track is not downloaded: local playback requires the file on disk.';
  toast.error(`Could not play "${track.title}"`, message + hint);
}

// "Add to Downloads" enqueues the track into the download queue (QueueView /
// DownloadsView), same as the D shortcut. Label renamed in phase 1 to
// disambiguate it from the play queue.
async function handleAddToQueue(track: Track) {
  closeContextMenu();
  await handleDownload(track);
}

/** L14 (#109): re-descargar una pista ya descargada. Reutiliza la única
 * ruta de re-descarga existente (`force_redownload_tracks`: limpia el
 * Download y las filas de cola de esa pista y la vuelve a encolar por
 * `perform_add_to_queue`). No es una segunda cola. */
async function handleReDownload(track: Track) {
  closeContextMenu()
  try {
    await loadEffectivePrefs()
    const queued = await queueApi.forceRedownloadTracks([track.id], 50, effectiveMaxQualityLabel())
    if (queued < 1) {
      toast.error('Could not re-download', `No source could be re-queued for "${track.title}".`)
      return
    }
    track.downloadStatus = 'queued'
    track.downloadQuality = null
    toast.success(`Re-download queued for "${track.title}"`)
  } catch (error: any) {
    console.error('Failed to re-download:', error)
    toast.error(`Failed to re-download: ${String(error?.message || error || '')}`)
  }
}

/** Acción del panel: «Download» encola una pista sin fichero y «Re-download»
 * usa la ruta de re-descarga para una pista ya descargada. */
async function handlePanelDownload(track: Track, alreadyDownloaded: boolean) {
  if (alreadyDownloaded || track.downloadStatus === 'downloaded') {
    await handleReDownload(track)
    return
  }
  await handleDownload(track)
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

// L05 (#102): TODA acción por lotes pasa por preview + run_library_action.
// La selección se envía como ids explícitos o como «todo el filtro» con
// exclusiones; el servidor resuelve, bloquea en 500 y agrega el resumen.
const isBulkRunning = ref(false)
const bulkDialog = ref<{ action: LibraryBatchAction; preview: LibraryActionPreview } | null>(null)
const bulkPlaylistId = ref<number | null>(null)
const bulkProgress = ref<LibraryActionProgress | null>(null)

function currentSelection(): LibrarySelection {
  if (selectAllMatching.value) {
    return {
      mode: 'filter',
      // offset/limit no intervienen: el servidor selecciona todo el filtro.
      query: buildLibraryQuery(0, null),
      excludedIds: [...excludedIds.value],
    }
  }
  return { mode: 'ids', ids: [...selection.selectedIds.value] }
}

function bulkDownloadAction(): LibraryBatchAction {
  return {
    action: 'queue_download',
    priority: 50,
    qualityPreference: effectiveMaxQualityLabel(),
    allowFallback: isDowngradeAllowed(),
  }
}

function omissionSummary(omissions: Array<{ count: number; message: string }>): string {
  return omissions
    .filter((omission) => omission.count > 0)
    .map((omission) => `${omission.count} ${omission.message.toLowerCase()}`)
    .join(', ')
}

async function openBulkAction(action: LibraryBatchAction) {
  if (isBulkRunning.value) return
  const selection = currentSelection()
  try {
    const preview = await libraryApi.previewLibraryAction(selection, action)
    if (preview.total === 0) {
      toast.info('Nothing selected', 'The current selection has no tracks.')
      return
    }
    if (preview.eligible === 0) {
      const reasons = omissionSummary(preview.omissions)
      toast.info(
        'No eligible tracks',
        `0 of ${preview.total} selected tracks can run.${reasons ? ` ${reasons}.` : ''}`
      )
      return
    }
    bulkDialog.value = { action, preview }
  } catch (error) {
    console.error('Failed to preview library action:', error)
    toast.error('Could not preview the action', String(error))
  }
}

async function openBulkDownload() {
  showBulkMenu.value = false
  await loadEffectivePrefs()
  await openBulkAction(bulkDownloadAction())
}

async function openBulkFavorite(favorite: boolean) {
  showBulkMenu.value = false
  await openBulkAction({ action: 'set_favorite', favorite })
}

async function openBulkAddToPlaylist() {
  showBulkMenu.value = false
  if (bulkPlaylistId.value === null) return
  await openBulkAction({ action: 'add_to_playlist', playlistId: bulkPlaylistId.value })
}

async function confirmBulkAction() {
  const dialog = bulkDialog.value
  if (!dialog || isBulkRunning.value) return
  const selection = currentSelection()
  if (selection.mode === 'ids' && selection.ids.length === 0) {
    bulkDialog.value = null
    return
  }

  isBulkRunning.value = true
  try {
    const summary: LibraryActionSummary = await libraryApi.runLibraryAction(
      selection,
      dialog.action,
      dialog.preview.total,
      dialog.preview.eligible
    )
    const details = [
      `${summary.succeeded} succeeded`,
      `${summary.failed} failed`,
      `${summary.skipped} omitted`,
    ].join(' · ')
    const reasons = omissionSummary(summary.omissions)
    const recalculated = summary.recalculated
      ? ' The library changed while previewing; the summary was recalculated.'
      : ''
    const message = `${details}.${reasons ? ` ${reasons}.` : ''}${recalculated}`
    if (summary.failed > 0) {
      toast.error('Batch action finished with failures', message)
    } else {
      toast.success('Batch action finished', message)
    }
    bulkDialog.value = null
    clearSelection()
    showBulkMenu.value = false
    await loadLibrary()
  } catch (error) {
    console.error('Failed to run library action:', error)
    toast.error('Batch action failed', String(error))
  } finally {
    isBulkRunning.value = false
    bulkProgress.value = null
  }
}

function clearSelection() {
  selectAllMatching.value = false
  excludedIds.value = []
  selection.clear()
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
      confirmedState ? 'Added to local favorites' : 'Removed from local favorites',
      `"${track.title}"`
    );
  } catch (error) {
    track.isFavorite = previousState;
    toast.error('Failed to update local favorite', String(error));
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
 * Deleting a track cascades to `playlist_tracks`, but not to
 * `playlists.track_count`: without this, the Playlists sidebar would keep
 * advertising songs that no longer exist. It is a maintenance pass, so a
 * failure here must not take down the deletion that already completed.
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
    }
    await reconcilePlaylistCounts();
    toast.success('Track removed', `"${track.title}" removed from library`);
  } catch (error) {
    toast.error('Failed to remove track', String(error));
  }
}

async function handleBulkRemove() {
  if (selectAllMatching.value) {
    // Acción destructiva: no se ofrece en «seleccionar todo».
    toast.info(
      'Per-track action',
      'Removing tracks is not offered for all matching tracks. Select the tracks explicitly.'
    );
    return;
  }
  const selectedTracks = tracks.value.filter(t => selection.isSelected(t.id));
  if (selectedTracks.length === 0) return;

  const ok = window.confirm(`Remove ${selectedTracks.length} track(s) from your library? This cannot be undone.`);
  if (!ok) return;

  try {
    const ids = selectedTracks.map(t => t.id);
    const removed = await libraryApi.bulkRemoveTracks(ids);
    tracks.value = tracks.value.filter(t => !selection.isSelected(t.id));
    clearSelection();
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
  // L-07: formatServiceName decides the canonical name (the same formatter the
  // rest of the app uses) and the initial comes from it, so two sets of aliases
  // ('apple' / 'apple music') cannot diverge from each other.
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

/** L14 (#109): etiqueta accesible de los cinco estados de descarga. */
function downloadStatusLabel(status: Track['downloadStatus']): string {
  switch (status) {
    case 'downloaded': return 'Downloaded'
    case 'queued': return 'Queued for download'
    case 'downloading': return 'Downloading'
    case 'failed': return 'Download failed'
    case 'stale': return 'File stale or missing'
    default: return 'Not downloaded'
  }
}

/** Calidad efectiva del fichero; si el backend aún no la proyecta (filas
 * antiguas o fixtures), cae al formato que ya mostraba la columna. */
function effectiveQuality(track: Track): string {
  return track.downloadQuality || track.quality || '—'
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
  // L05 (#102): la casilla de cabecera selecciona TODO el filtro, no la página
  // cargada (el total lo da el servidor). Si ya está todo seleccionado, vacía.
  if (selectAllMatching.value || (selectedCount.value > 0 && tracks.value.every(t => isTrackSelected(t.id)))) {
    clearSelection()
    return
  }
  selectAllMatching.value = true
  excludedIds.value = []
}

/** Cambia de «todo el filtro» a los ids de las filas ya cargadas. */
function selectLoadedPageOnly() {
  selectAllMatching.value = false
  excludedIds.value = []
  selection.selectAll(tracks.value)
}

// L01 (#98): filtros, búsqueda y orden viven EN SERVIDOR (query_library:
// orden con desempates NOCASE estables y nulos al final). L13 (#110): el
// renderer no filtra, no ordena y no conserva un alias «filtered»: las filas
// que pinta son exactamente las páginas que devuelve el servidor.
//
// La dirección y la clave de orden son parte de la consulta: al cambiarlas
// se pide la página de nuevo (la dirección afecta SOLO a la clave primaria
// y los desempates los fija el servidor). L03 (#100): también viven en la
// URL; si llegaron de ella (applyUrlState) la carga ya se hizo.
watch([sortBy, sortDirection], () => {
  if (suppressSortWatcher) {
    suppressSortWatcher = false
    return
  }
  syncUrlFromView(true)
  scheduleLibraryReload()
})

// Ventana virtual de la lista: kernel compartido (useVirtualWindow,
// ADR-0025). Filas de 56 px, overscan 10, throttle de 120 ms y footer de fin
// de lista de 48 px.
const {
  containerRef: listContainerRef,
  scrollTop: libraryScrollTop,
  viewportHeight: libraryViewportHeight,
  totalHeight: totalVirtualHeight,
  startIndex: virtualStartIndex,
  endIndex: virtualEndIndex,
  offsetY: virtualOffsetY,
  visibleItems: visibleTracks,
  handleScroll,
  reset: resetVirtualScroll,
} = useVirtualWindow(tracks, {
  rowHeight: ROW_HEIGHT,
  overscan: OVERSCAN,
  footerHeight: () => (!hasMore.value && tracks.value.length > 0 ? END_OF_LIST_FOOTER_PX : 0),
  scrollThrottleMs: SCROLL_THROTTLE_MS,
  onScrollMeasured: maybeLoadMore,
})

watch([() => tracks.value.length, searchQuery, () => activeFilters.value], () => {
  if (listContainerRef.value && libraryScrollTop.value > totalVirtualHeight.value) {
    resetVirtualScroll()
  }
})

// Album interface for grid view
interface Album {
  /** Identidad estable del grupo (albumId real o título+artista). */
  key: string
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

// L13 (#110): agrupar NO es filtrar ni ordenar. Los grupos se forman en el
// orden en que el servidor devuelve las filas (Map conserva la primera
// aparición) y las pistas dentro de cada grupo mantienen ese mismo orden. No
// hay `localeCompare` ni comparador de «fecha añadida» en el renderer.
const expandedGroups = ref<string[]>([])

type GroupKind = 'artist' | 'album' | 'genre' | 'quality'

interface GroupSection {
  key: string
  kind: GroupKind
  label: string
  tracks: Track[]
  album?: Album
}

const groupedByAlbum = computed<Album[]>(() => {
  const albumMap = new Map<string, Track[]>()

  for (const track of tracks.value) {
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

  albumMap.forEach((albumTracks, groupKey) => {
    const firstTrack = albumTracks[0]
    const albumRealId = firstTrack.albumId ?? idCounter++
    const uniqueServices = new Set<string>()
    albumTracks.forEach(t => t.services.forEach(s => uniqueServices.add(s)))

    albums.push({
      key: groupKey,
      id: albumRealId,
      albumId: firstTrack.albumId ?? null,
      title: firstTrack.album,
      artist: firstTrack.artist,
      coverUrl: firstTrack.coverUrl,
      artGradient: firstTrack.artGradient,
      services: Array.from(uniqueServices),
      quality: firstTrack.quality,
      downloadStatus: albumTracks.every(t => t.downloadStatus === 'downloaded') ? 'downloaded' :
                      albumTracks.some(t => t.downloadStatus === 'queued') ? 'queued' : 'not_downloaded',
      metadataScore: Math.round(albumTracks.reduce((sum, t) => sum + (t.metadataScore ?? 0), 0) / albumTracks.length),
      trackCount: albumTracks.length,
      isSelected: false,
      tracks: albumTracks
    })
  })

  return albums
})

const groupedByArtist = computed<GroupSection[]>(() => {
  const artistMap = new Map<string, Track[]>()
  for (const track of tracks.value) {
    const existing = artistMap.get(track.artist)
    if (existing) {
      existing.push(track)
    } else {
      artistMap.set(track.artist, [track])
    }
  }
  return Array.from(artistMap, ([artist, groupTracks]) => ({
    key: `artist:${artist}`,
    kind: 'artist' as const,
    label: artist,
    tracks: groupTracks,
  }))
})

const albumSections = computed<GroupSection[]>(() => groupedByAlbum.value.map(album => ({
  key: `album:${album.key}`,
  kind: 'album' as const,
  label: album.title,
  tracks: album.tracks,
  album,
})))

const groupedByGenre = computed<GroupSection[]>(() => {
  const genreMap = new Map<string, Track[]>()
  for (const track of tracks.value) {
    const genre = track.genre ? track.genre.trim() : 'Unknown Genre'
    const existing = genreMap.get(genre)
    if (existing) {
      existing.push(track)
    } else {
      genreMap.set(genre, [track])
    }
  }
  return Array.from(genreMap, ([genre, groupTracks]) => ({
    key: `genre:${genre}`,
    kind: 'genre' as const,
    label: genre,
    tracks: groupTracks,
  }))
})

const groupedByQuality = computed<GroupSection[]>(() => {
  const qualityMap = new Map<string, Track[]>()
  for (const track of tracks.value) {
    const quality = (!track.quality || track.quality === '—') ? 'Unknown' : track.quality
    const existing = qualityMap.get(quality)
    if (existing) {
      existing.push(track)
    } else {
      qualityMap.set(quality, [track])
    }
  }
  return Array.from(qualityMap, ([quality, groupTracks]) => ({
    key: `quality:${quality}`,
    kind: 'quality' as const,
    label: quality,
    tracks: groupTracks,
  }))
})

const groupSections = computed<GroupSection[]>(() => {
  switch (groupBy.value) {
    case 'artist': return groupedByArtist.value
    case 'album': return albumSections.value
    case 'genre': return groupedByGenre.value
    case 'quality': return groupedByQuality.value
    default: return []
  }
})

// Filas heterogéneas de la vista agrupada: una cabecera por grupo y, si está
// expandido, una fila por pista. Alturas fijas por tipo: el kernel variable
// usa los prefijos acumulados para saber qué ventana pintar.
const GROUP_HEADER_HEIGHT = 64
const GROUP_TRACK_HEIGHT = 52
const GROUP_OVERSCAN = 8

interface GroupDisplayRow {
  id: string
  rowType: 'header' | 'track'
  groupKind: GroupKind
  groupKey: string
  height: number
  header?: GroupSection
  track?: Track
  trackIndex?: number
}

const groupDisplayRows = computed<GroupDisplayRow[]>(() => {
  const rows: GroupDisplayRow[] = []
  for (const section of groupSections.value) {
    rows.push({
      id: `header:${section.key}`,
      rowType: 'header',
      groupKind: section.kind,
      groupKey: section.key,
      height: GROUP_HEADER_HEIGHT,
      header: section,
    })
    if (expandedGroups.value.includes(section.key)) {
      section.tracks.forEach((track, trackIndex) => {
        rows.push({
          id: `track:${section.key}:${track.id}`,
          rowType: 'track',
          groupKind: section.kind,
          groupKey: section.key,
          height: GROUP_TRACK_HEIGHT,
          track,
          trackIndex,
        })
      })
    }
  }
  return rows
})

const {
  containerRef: groupContainerRef,
  totalHeight: totalGroupHeight,
  offsetY: groupOffsetY,
  visibleItems: visibleGroupRows,
  handleScroll: handleGroupScroll,
  reset: resetGroupScroll,
} = useVirtualWindow(groupDisplayRows, {
  rowHeight: GROUP_TRACK_HEIGHT,
  rowHeightFor: (index) => groupDisplayRows.value[index]?.height ?? GROUP_TRACK_HEIGHT,
  overscan: GROUP_OVERSCAN,
  scrollThrottleMs: SCROLL_THROTTLE_MS,
  onScrollMeasured: maybeLoadMore,
})

// Grid virtualizado: las columnas responden a los breakpoints del diseño
// (mismos que el grid de Tailwind) y el alto de fila se deriva del ancho de
// baldosa, para que la ventana visible no dependa de un `aspect-square`
// calculado por el navegador.
const GRID_GAP_PX = 20
const GRID_INFO_PX = 92
const GRID_HORIZONTAL_PADDING_PX = 72
const GRID_OVERSCAN = 3

const gridColumns = ref(2)
const gridAvailableWidth = ref(1024)

function updateGridMetrics() {
  const width = typeof window !== 'undefined' && window.innerWidth ? window.innerWidth : 1024
  gridAvailableWidth.value = width
  gridColumns.value = width >= 1536 ? 6 : width >= 1024 ? 5 : width >= 768 ? 4 : width >= 640 ? 3 : 2
}
updateGridMetrics()

const gridTileSize = computed(() => Math.max(
  120,
  Math.floor((gridAvailableWidth.value - GRID_HORIZONTAL_PADDING_PX - GRID_GAP_PX * (gridColumns.value - 1)) / gridColumns.value)
))
const gridRowHeight = computed(() => gridTileSize.value + GRID_INFO_PX + GRID_GAP_PX)

const gridRows = computed<Album[][]>(() => {
  const rows: Album[][] = []
  const albums = groupedByAlbum.value
  for (let index = 0; index < albums.length; index += gridColumns.value) {
    rows.push(albums.slice(index, index + gridColumns.value))
  }
  return rows
})

const {
  containerRef: gridContainerRef,
  totalHeight: totalGridHeight,
  offsetY: gridOffsetY,
  visibleItems: visibleGridRows,
  handleScroll: handleGridScroll,
} = useVirtualWindow(gridRows, {
  rowHeight: gridRowHeight,
  overscan: GRID_OVERSCAN,
  footerHeight: () => (!hasMore.value && gridRows.value.length > 0 ? END_OF_LIST_FOOTER_PX : 0),
  scrollThrottleMs: SCROLL_THROTTLE_MS,
  onScrollMeasured: maybeLoadMore,
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
/**
 * Recarga coalescida de la vista (L13, #110): una ráfaga síncrona de cambios
 * de filtro/orden produce UNA consulta cuando el estado ya está asentado, no
 * una por clic. La caché se encarga de que volver a un filtro ya visitado no
 * consulte de nuevo.
 */
let reloadQueued = false
function scheduleLibraryReload() {
  if (reloadQueued) return
  reloadQueued = true
  void Promise.resolve().then(() => {
    reloadQueued = false
    void loadLibrary()
  })
}

function toggleFilter(filterId: string) {
  // Piezas deshabilitadas con su motivo: no activan un filtro sin semántica.
  if (filterPills.find(f => f.id === filterId)?.disabled) return
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

  // Cualquier cambio de filtro cambia la consulta: se pide de nuevo al
  // servidor (los contadores vienen con la primera página) y se refleja en
  // la URL (push: atrás recorre estados de filtro). La recarga se coalesce:
  // una ráfaga de clics deja una sola consulta.
  scheduleLibraryReload()
  syncUrlFromView(true)
}

function removeFilter(filterId: string) {
  activeFilters.value = activeFilters.value.filter(f => f !== filterId)
  if (activeFilters.value.length === 0) {
    activeFilters.value = ['all']
  }
  scheduleLibraryReload()
  syncUrlFromView(true)
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
  if (searchQuery.value !== '') {
    // La carga la hace el loadLibrary de abajo: sin doble consulta ni
    // replace posterior del debounce.
    suppressSearchWatcher = true
    searchQuery.value = ''
  }
  scheduleLibraryReload()
  syncUrlFromView(true)
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
      void openBulkDownload()
    } else if (contextMenu.value.track) {
      handleDownload(contextMenu.value.track)
    }
  } else if (event.key === 'q' || event.key === 'Q') {
    // FE-9: "Add to Downloads" — same enqueue path as D (download queue).
    if (selectedCount.value > 0) {
      void openBulkDownload()
    } else if (contextMenu.value.track) {
      void handleAddToQueue(contextMenu.value.track)
    }
  } else if (event.key === 'n' || event.key === 'N') {
    // FE-9: "Play Next" on the context track or the first selected track.
    const nextTarget = contextMenu.value.track ?? tracks.value.find(t => isTrackSelected(t.id))
    if (nextTarget) void handlePlayNext(nextTarget)
  } else if (event.key === 'f' || event.key === 'F') {
    if (contextMenu.value.track) {
      handleToggleFavorite(contextMenu.value.track)
    } else {
      const selected = tracks.value.find(t => isTrackSelected(t.id))
      if (selected) handleToggleFavorite(selected)
    }
  } else if (event.key === ' ' || event.code === 'Space') {
    // FE-9: with the context menu open on an idle player, Space starts that
    // track ("Play Now"). General play/pause is a window shortcut since P04
    // (#135): if the registry already dispatched it (preventDefault), it is
    // not repeated here — two toggles would leave the transport as it was.
    if (contextMenu.value.visible && contextMenu.value.track && !playerCurrent.value) {
      event.preventDefault()
      void handleTrackPlay(contextMenu.value.track)
    } else if (!event.defaultPrevented) {
      event.preventDefault()
      togglePlayback()
    }
  } else if (event.key === 'Escape') {
    // L04 (#101): el panel de detalle es la capa superior; Escape lo cierra.
    if (detailTrackId.value !== null) {
      closeTrackDetail()
      return
    }
    clearSelection()
    closeContextMenu()
    showShortcutsModal.value = false
    showDownloadFavoritesModal.value = false
  } else if (event.key === 'v' || event.key === 'V') {
    viewMode.value = viewMode.value === 'list' ? 'grid' : 'list'
  }
}

// FE-10 / L03 (#100): consume los deep-links de la biblioteca. El legacy del
// dashboard (`?filter=quality&quality=<bucket>`; `?filter=duplicates` sin
// semántica) y las claves nuevas (q, sort, dir, dl, quality, fav) los
// parsea `applyUrlState` (composables/libraryUrlState.ts). Leerlo solo al
// montar dejaba un filtro viejo cuando el query cambia en el sitio
// (back/forward u otro deep-link a la misma ruta), así que el watcher de
// ruta de abajo re-aplica el estado serializado.

// ==============================================
// Item 45: deep-link /library?track=<id>
// ==============================================
// On mount, with the library already loaded, if the query carries `track` it
// scrolls to that row, highlights it for a few seconds and clears the
// parameter so reloading does not repeat the jump.
const highlightedTrackId = ref<number | null>(null)
let highlightClearTimer: ReturnType<typeof setTimeout> | null = null

/** Cap on pages loaded while looking for the linked track (50 × PAGE_SIZE). */
const TRACK_DEEP_LINK_MAX_PAGES = 50

async function focusTrackFromQuery() {
  const raw = route?.query?.track
  if (raw === undefined || raw === null || raw === '') return

  // The parameter is ALWAYS consumed, whether or not the track exists: that way
  // a reload or going back does not repeat the scroll.
  const rest: Record<string, LocationQueryValue | LocationQueryValue[]> = { ...route.query }
  delete rest.track
  await router.replace({ query: rest })

  const trackId = typeof raw === 'string' ? Number.parseInt(raw, 10) : NaN
  if (!Number.isFinite(trackId)) return

  // If the track falls on pages not loaded yet, page until it is found.
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

  const trackIdx = tracks.value.findIndex(t => t.id === trackId)
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

  // L04 (#101): el deep link también abre el panel de esa pista.
  openTrackDetail(trackId)
}

// ==============================================
// L04 (#101): panel lateral de detalle de pista
// ==============================================
// UNA consulta al abrirse (get_library_track_detail: Track, fuentes,
// pertenencias y descarga en una llamada — no por campo, fuente o
// pertenencia). Tras una acción o un evento de sync se refresca el panel
// entero con la misma llamada. Una pista inexistente muestra el error
// tipado (`track_not_found`), no una fila vacía.
const detailTrackId = ref<number | null>(null)
const detailData = ref<TrackDetail | null>(null)

/** L14 (#109): calidad efectiva del panel = formato + profundidad/frecuencia
 * reales del fichero. No decide calidad ni fallback: solo muestra lo que el
 * Download ya tiene. */
function detailDownloadQuality(download: NonNullable<TrackDetail['download']>): string {
  const parts: string[] = []
  if (download.fileFormat) parts.push(download.fileFormat)
  if (download.bitDepth !== null && download.sampleRate !== null) {
    parts.push(`${download.bitDepth}/${download.sampleRate / 1000}`)
  }
  return parts.join(' · ')
}
const detailError = ref<TrackDetailError | null>(null)
const detailLoading = ref(false)
const detailPlaylistChoice = ref<number | null>(null)
// Token de secuencia: si el panel cambia de pista o se cierra mientras una
// respuesta está en vuelo, la respuesta vieja se descarta.
let detailQuerySeq = 0

/** La fila canónica del panel, mapeada al Track de la vista para reusar los
 * manejadores existentes (play, descarga, favorito, abrir ficha). */
const panelTrack = computed<Track | null>(() => (detailData.value ? mapToTrack(detailData.value.track, 0) : null))
/** L14 (#109): hay Download aunque el fichero se haya verificado como
 * ausente (fila `downloads` presente): la acción correcta es re-descargar. */
const panelHasDownload = computed<boolean>(() => {
  const download = detailData.value?.download
  return !!download && (download.status === 'downloaded' || !!download.filePath)
})

async function openTrackDetail(trackId: number) {
  detailTrackId.value = trackId
  detailPlaylistChoice.value = null
  await refreshTrackDetail()
}

async function refreshTrackDetail() {
  const trackId = detailTrackId.value
  if (trackId === null) return
  const seq = ++detailQuerySeq
  detailLoading.value = true
  detailError.value = null
  try {
    const detail = await libraryApi.getTrackDetail(trackId)
    if (seq !== detailQuerySeq || detailTrackId.value !== trackId) return
    detailData.value = detail
  } catch (error) {
    if (seq !== detailQuerySeq || detailTrackId.value !== trackId) return
    // El wrapper normaliza el rechazo al error tipado del contrato.
    detailError.value = error as TrackDetailError
    detailData.value = null
  } finally {
    if (seq === detailQuerySeq) detailLoading.value = false
  }
}

function closeTrackDetail() {
  // Un token nuevo invalida cualquier respuesta en vuelo del panel cerrado.
  detailQuerySeq++
  detailTrackId.value = null
  detailData.value = null
  detailError.value = null
  detailLoading.value = false
  detailPlaylistChoice.value = null
}

/** Tras una acción del panel: refresca el panel y la lista sin esperar al
 * debounce (el debounce es para tormentas de eventos, no para una acción
 * explícita del usuario). */
async function refreshPanelAndList() {
  await Promise.allSettled([refreshTrackDetail(), loadLibrary()])
}

async function panelFavorite() {
  const track = panelTrack.value
  if (!track) return
  await handleToggleFavorite(track)
  // El estado confirmado llega del backend; la fila canónica manda.
  await refreshTrackDetail()
}

async function panelAddToPlaylist() {
  const trackId = detailTrackId.value
  const playlistId = detailPlaylistChoice.value
  if (trackId === null || playlistId === null) return
  await addTrackToPlaylist(playlistId, trackId)
  await refreshTrackDetail()
}

/** Borrar audio con confirmación: elimina solo el Download y el fichero,
 * nunca el Track, sus fuentes ni su pertenencia (story 22 del spec #96). */
async function panelDeleteAudio() {
  const trackId = detailTrackId.value
  const track = panelTrack.value
  if (trackId === null || !track) return
  const deleted = await confirmDeleteAudio(track)
  if (deleted) await refreshPanelAndList()
}

/** L14 (#109): borrado de audio por pista y con confirmación. Elimina solo
 * el Download, sus entradas de cola y el fichero; el Track, sus TrackSource,
 * su pertenencia y la evidencia de edición se conservan (lo garantiza el
 * backend de L04, reutilizado aquí). */
async function deleteAudioFromContextMenu() {
  const track = contextMenu.value.track
  closeContextMenu()
  if (!track) return
  const deleted = await confirmDeleteAudio(track)
  if (!deleted) return
  if (detailTrackId.value === track.id) await refreshTrackDetail()
  await loadLibrary()
}

async function confirmDeleteAudio(track: Track): Promise<boolean> {
  const confirmed = window.confirm(`Delete downloaded audio for "${track.title}"? The track, its sources and memberships are kept.`)
  if (!confirmed) return false
  try {
    const summary = await libraryApi.deleteTrackAudio(track.id)
    track.downloadStatus = 'not_downloaded'
    track.downloadQuality = null
    track.downloadedFrom = null
    track.filePath = null
    toast.success('Audio deleted', `Download removed${summary.audioFileDeleted ? ' and file deleted' : ''}`)
    return true
  } catch (error) {
    toast.error('Failed to delete audio', String(error))
    return false
  }
}

// Lifecycle
let queueStatsInterval: ReturnType<typeof setInterval> | null = null

// MB dump catalog import (#264 punto 1)
const mbDumpPanelOpen = ref(false)
const mbDumpPath = ref('')
const mbDumpImporting = ref(false)
const mbDumpProgressLine = ref('')
const mbDumpMessage = ref('')
const mbDumpError = ref(false)
const mbDumpState = ref<MbDumpImportState | null>(null)

function toggleMbDumpPanel() {
  mbDumpPanelOpen.value = !mbDumpPanelOpen.value
  if (mbDumpPanelOpen.value) {
    getMbDumpImportState()
      .then((s) => { mbDumpState.value = s })
      .catch(() => { mbDumpState.value = null })
  }
}

async function startMbDumpImport() {
  if (mbDumpImporting.value || !mbDumpPath.value.trim()) return
  mbDumpImporting.value = true
  mbDumpError.value = false
  mbDumpMessage.value = ''
  mbDumpProgressLine.value = ''
  try {
    const counts = await importMbDump(mbDumpPath.value.trim())
    mbDumpMessage.value = `Catalog imported: ${counts.recording} recordings · ${counts.artist_credit_name} artist credits · ${counts.isrc} isrcs · ${counts.artist} artists`
  } catch (e) {
    mbDumpError.value = true
    mbDumpMessage.value = `Import failed: ${e instanceof Error ? e.message : String(e)}`
  } finally {
    mbDumpImporting.value = false
    getMbDumpImportState()
      .then((s) => { mbDumpState.value = s })
      .catch(() => { mbDumpState.value = null })
  }
}

onMounted(async () => {
  document.addEventListener('click', closeContextMenu)
  window.addEventListener('keydown', handleKeydown)
  // Si el viewport cambia de ancho, las columnas del grid cambian: el kernel
  // recalcula la ventana con el nuevo alto de fila.
  window.addEventListener('resize', updateGridMetrics)

  // L03 (#100): la URL es la fuente del estado inicial (deep-linking).
  // applyUrlState asigna sin disparar watchers suprimidos; la carga única
  // es la de abajo.
  applyUrlState()

  await loadLibrary()
  await fetchQueueStats()

  // Item 45: after the initial load, handle the ?track=<id> deep-link.
  await focusTrackFromQuery()

  // FE-PERF-01: startQueueStatsInterval hace de guard (véase su comentario).
  startQueueStatsInterval()

  // L04 (#101): con el panel abierto, un evento de sync refresca también el
  // panel entero (misma llamada única), sin recargar la página.
  const refreshDetailIfOpen = () => {
    if (detailTrackId.value !== null) void refreshTrackDetail()
  }
  // La invalidación de caché y la recarga con debounce las dispara
  // `useLibraryCacheInvalidation` (matriz de eventos, L13); aquí solo se
  // refresca el panel abierto con la misma llamada única.
  eventBus.on(TauriEvents.LIBRARY_UPDATED, async () => {
    refreshDetailIfOpen()
  })
  eventBus.on(TauriEvents.IMPORT_COMPLETE, async () => {
    refreshDetailIfOpen()
  })
  eventBus.on(TauriEvents.SYNC_COMPLETE, async () => {
    refreshDetailIfOpen()
  })
  eventBus.on(TauriEvents.MB_DUMP_PROGRESS, (progress: MbDumpProgress) => {
    mbDumpProgressLine.value = `${progress.table}: ${progress.rows}/${progress.expected} rows`
  })
  eventBus.on(TauriEvents.DOWNLOAD_PROGRESS, async () => {
    fetchQueueStats()
  })
  // L05 (#102): los eventos del runner son agregados (uno por bloque / final);
  // la barra de lote muestra el avance sin una fila por pista.
  eventBus.on(TauriEvents.LIBRARY_ACTION_PROGRESS, (progress: LibraryActionProgress) => {
    bulkProgress.value = progress
  })
  eventBus.on(TauriEvents.LIBRARY_ACTION_FINISHED, () => {
    bulkProgress.value = null
  })
})

// FE-10 / L03 (#100): react to query changes on the same route (browser
// back/forward between filter states, dashboard deep-links, tab round-trip)
// instead of keeping the mount-time state. La clave es el query SERIALIZADO
// (no la identidad del objeto route) y solo cuando la vista es la activa:
// una instancia cacheada por KeepAlive no debe pedir en la sombra. La vista
// no se re-aplica a sí misma: `syncUrlFromView` dejó `lastWrittenUrl` antes
// de navegar y el guard evita el eco.
watch(
  () => (route && route.path === '/library' ? JSON.stringify(route.query) : ''),
  () => {
    if (!route || route.path !== '/library') return
    const serialized = JSON.stringify(route.query)
    if (serialized === lastWrittenUrl) return
    if (applyUrlState()) {
      void loadLibrary()
    }
  }
)

// L04 (#101): un cambio de ruta cierra el panel. Con KeepAlive la instancia
// sigue montada al navegar a otra vista; el panel no debe quedar flotando
// al volver.
watch(
  () => route?.path,
  (path, previous) => {
    if (previous !== undefined && path !== '/library' && detailTrackId.value !== null) {
      closeTrackDetail()
    }
  }
)

// FE-PERF-01: con LibraryView cacheada por <KeepAlive>, el sondeo de 4s de la
// cola no debe seguir latiendo mientras la vista está oculta. onActivated/
// onDeactivated SOLO disparan en instancias dentro de un KeepAlive, así que
// una vista fuera del include conserva el comportamiento de siempre (start en
// onMounted, stop en onUnmounted). El guard evita el doble intervalo: con
// KeepAlive, onActivated llega justo tras el mount y antes de que acaben los
// `await` de onMounted, que ya arrancan el sondeo.
function startQueueStatsInterval() {
  if (queueStatsInterval) return
  queueStatsInterval = setInterval(fetchQueueStats, 4000)
}

function stopQueueStatsInterval() {
  if (queueStatsInterval) {
    clearInterval(queueStatsInterval)
    queueStatsInterval = null
  }
}

onActivated(() => {
  viewActive = true
  startQueueStatsInterval()
  // Una mutación ocurrida mientras la vista estaba cacheada invalidó la
  // caché; al volver se relee una vez en vez de mostrar filas viejas.
  if (cacheDirty) {
    void loadLibrary()
  }
})

onDeactivated(() => {
  viewActive = false
  stopQueueStatsInterval()
  // Sin recargas en la sombra: si un evento invalidó la caché mientras la
  // vista estaba oculta, `cacheDirty` hace la única recarga al volver.
  if (reloadDebounceTimer) {
    clearTimeout(reloadDebounceTimer)
    reloadDebounceTimer = null
  }
})

onUnmounted(() => {
  document.removeEventListener('click', closeContextMenu)
  window.removeEventListener('keydown', handleKeydown)
  window.removeEventListener('resize', updateGridMetrics)
  offLibraryInvalidated()
  stopQueueStatsInterval()
  if (searchDebounceTimer) {
    clearTimeout(searchDebounceTimer)
  }
  if (reloadDebounceTimer) {
    clearTimeout(reloadDebounceTimer)
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
