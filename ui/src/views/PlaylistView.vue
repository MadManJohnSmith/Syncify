<template>
  <div class="playlist-view flex h-full">
    <!-- Playlist Sidebar (30%) -->
    <div class="playlist-sidebar w-80 border-r border-gray-200 dark:border-border-dark flex flex-col bg-gray-50 dark:bg-surface-dark">
      <!-- Sidebar Header -->
      <div class="p-4 border-b border-gray-200 dark:border-border-dark">
        <div class="flex items-center justify-between mb-3">
          <div>
            <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Playlists</h2>
            <p class="text-xs text-gray-500">{{ totalPlaylists }} playlists</p>
          </div>
          <button 
            @click="showCreateModal = true"
            class="px-3 py-1.5 bg-primary hover:bg-primary-hover text-on-accent text-sm font-medium rounded-lg flex items-center gap-1"
          >
            <span class="material-symbols-outlined text-sm">add</span>
            New
          </button>
        </div>
        
        <!-- Search -->
        <div class="relative">
          <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 text-lg">search</span>
          <input 
            v-model="searchQuery"
            type="text"
            placeholder="Search playlists..."
            class="w-full pl-10 pr-4 py-2 bg-white dark:bg-surface-highlight rounded-lg text-sm text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50"
          >
        </div>
        
        <!-- Quick Actions -->
        <div class="flex gap-2 mt-3">
          <button @click="triggerSyncPlaylists" :disabled="isSyncing" class="flex-1 py-1.5 text-xs text-accent bg-primary/10 hover:bg-primary/20 rounded-lg flex items-center justify-center gap-1 disabled:opacity-50 font-medium">
            <span class="material-symbols-outlined text-sm" :class="{ 'animate-spin': isSyncing }">sync</span>
            {{ isSyncing ? 'Syncing...' : 'Sync All' }}
          </button>
          <button @click="showImportModal = true" class="flex-1 py-1.5 text-xs text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg flex items-center justify-center gap-1">
            <span class="material-symbols-outlined text-sm">link</span>
            Import URL
          </button>
          <button @click="showSmartModal = true" class="flex-1 py-1.5 text-xs text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg flex items-center justify-center gap-1">
            <span class="material-symbols-outlined text-sm">auto_awesome</span>
            Smart
          </button>
        </div>
      </div>
      
      <!-- Playlist Categories -->
      <div class="flex-1 overflow-y-auto custom-scrollbar">
<!-- Favorites (System) -->
        <div
          class="playlist-item p-3 mx-2 mt-2 rounded-lg cursor-pointer transition-colors flex items-center gap-3"
          :class="selectedPlaylist?.id === FAVORITES_PLAYLIST_ID ? 'bg-primary/10 border border-primary/20' : 'hover:bg-gray-100 dark:hover:bg-surface-highlight'"
          @click="selectPlaylist(favoritesPlaylist)"
        >
          <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-red-500 to-pink-500 flex items-center justify-center shrink-0">
            <span class="material-symbols-outlined text-white text-lg">favorite</span>
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-medium text-gray-900 dark:text-white truncate">Favorites</p>
            <p class="text-xs text-gray-500">{{ favoritesPlaylist.track_count }} tracks</p>
          </div>
        </div>
        
        <!-- My Playlists -->
        <div class="mt-4">
          <button 
            @click="expandedCategories.myPlaylists = !expandedCategories.myPlaylists"
            class="w-full px-4 py-2 flex items-center justify-between text-xs font-semibold text-gray-500 uppercase tracking-wide hover:bg-gray-100 dark:hover:bg-surface-highlight"
          >
            My Playlists ({{ myPlaylists.length }})
            <span class="material-symbols-outlined text-sm">{{ expandedCategories.myPlaylists ? 'expand_less' : 'expand_more' }}</span>
          </button>
          <Transition name="accordion">
            <div v-if="expandedCategories.myPlaylists" class="px-2 space-y-1">
              <div 
                v-for="playlist in filteredMyPlaylists" 
                :key="playlist.id"
                class="playlist-item p-2 rounded-lg cursor-pointer transition-colors flex items-center gap-3 group"
                :class="selectedPlaylist?.id === playlist.id ? 'bg-primary/10 border border-primary/20' : 'hover:bg-gray-100 dark:hover:bg-surface-highlight'"
                @click="selectPlaylist(playlist)"
                @contextmenu.prevent="openPlaylistMenu($event, playlist)"
                draggable="true"
                @dragstart="onDragStart($event, playlist)"
              >
                <PlaylistCover
                  :src="playlist.coverImageUrl"
                  :tiles="playlist.mosaicTiles"
                  :item-id="playlist.id"
                  :alt="playlist.name"
                  size-class="w-10 h-10"
                  rounded="rounded-lg"
                  icon="queue_music"
                />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ playlist.name }}</p>
                  <p class="text-xs text-gray-500">{{ playlist.track_count }} tracks</p>
                </div>
                <div class="opacity-60 group-hover:opacity-100 transition-opacity flex items-center gap-1">
                  <button @click.stop="playPlaylist(playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">play_arrow</span>
                  </button>
                  <button @click.stop="openPlaylistMenu($event, playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">more_vert</span>
                  </button>
                </div>
              </div>
            </div>
          </Transition>
        </div>
        
        <!-- Imported Playlists (per service) -->
        <div v-for="service in filteredImportedServices" :key="service.id" class="mt-2">
          <button
            @click="expandedCategories[service.id] = !expandedCategories[service.id]"
            class="w-full px-4 py-2 flex items-center justify-between text-xs font-semibold text-gray-500 uppercase tracking-wide hover:bg-gray-100 dark:hover:bg-surface-highlight"
          >
            <span class="flex items-center gap-2">
              <span :class="['w-3 h-3 rounded-full', getServiceColor(service.id)]"></span>
              Imported from {{ service.name }} ({{ service.playlists.length }})
            </span>
            <span class="material-symbols-outlined text-sm">{{ expandedCategories[service.id] ? 'expand_less' : 'expand_more' }}</span>
          </button>
          <Transition name="accordion">
            <div v-if="expandedCategories[service.id]" class="px-2 space-y-1">
              <div
                v-for="playlist in service.playlists"
                :key="playlist.id"
                class="playlist-item p-2 rounded-lg cursor-pointer transition-colors flex items-center gap-3 group"
                :class="selectedPlaylist?.id === playlist.id ? 'bg-primary/10 border border-primary/20' : 'hover:bg-gray-100 dark:hover:bg-surface-highlight'"
                @click="selectPlaylist(playlist)"
                @contextmenu.prevent="openPlaylistMenu($event, playlist)"
              >
                <PlaylistCover
                  :src="playlist.coverImageUrl"
                  :tiles="playlist.mosaicTiles"
                  :item-id="playlist.id"
                  :alt="playlist.name"
                  size-class="w-10 h-10"
                  rounded="rounded-lg"
                  icon="queue_music"
                />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ playlist.name }}</p>
                  <div class="flex items-center gap-2 text-xs text-gray-500">
                    <span>{{ playlist.track_count }} tracks</span>
                    <span v-if="playlist.last_synced" class="flex items-center gap-1 text-green-500">
                      <span class="material-symbols-outlined text-xs">sync</span>
                      Synced
                    </span>
                  </div>
                </div>
                <div class="opacity-60 group-hover:opacity-100 transition-opacity flex items-center gap-1">
                  <button @click.stop="playPlaylist(playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">play_arrow</span>
                  </button>
                  <button @click.stop="openPlaylistMenu($event, playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">more_vert</span>
                  </button>
                </div>
              </div>
            </div>
          </Transition>
        </div>

        <!-- Smart Playlists -->
        <div class="mt-2">
          <button
            @click="expandedCategories.smart = !expandedCategories.smart"
            class="w-full px-4 py-2 flex items-center justify-between text-xs font-semibold text-gray-500 uppercase tracking-wide hover:bg-gray-100 dark:hover:bg-surface-highlight"
          >
            <span class="flex items-center gap-2">
              <span class="material-symbols-outlined text-sm text-amber-500">auto_awesome</span>
              Smart Playlists ({{ smartPlaylists.length }})
            </span>
            <span class="material-symbols-outlined text-sm">{{ expandedCategories.smart ? 'expand_less' : 'expand_more' }}</span>
          </button>
          <Transition name="accordion">
            <div v-if="expandedCategories.smart" class="px-2 space-y-1">
              <div
                v-for="playlist in filteredSmartPlaylists"
                :key="playlist.id"
                class="playlist-item p-2 rounded-lg cursor-pointer transition-colors flex items-center gap-3 group"
                :class="selectedPlaylist?.id === playlist.id ? 'bg-primary/10 border border-primary/20' : 'hover:bg-gray-100 dark:hover:bg-surface-highlight'"
                @click="selectPlaylist(playlist)"
                @contextmenu.prevent="openPlaylistMenu($event, playlist)"
              >
                <PlaylistCover
                  :tiles="playlist.mosaicTiles"
                  :item-id="playlist.id"
                  :alt="playlist.name"
                  size-class="w-10 h-10"
                  rounded="rounded-lg"
                  icon="auto_awesome"
                />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ playlist.name }}</p>
                  <p class="text-xs text-gray-500">{{ playlist.track_count }} tracks · Auto-updates</p>
                </div>
                <div class="opacity-60 group-hover:opacity-100 transition-opacity flex items-center gap-1">
                  <button @click.stop="playPlaylist(playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">play_arrow</span>
                  </button>
                  <button @click.stop="openPlaylistMenu($event, playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">more_vert</span>
                  </button>
                </div>
              </div>
            </div>
          </Transition>
        </div>
      </div>
    </div>
    
    <!-- Playlist Contents (70%) -->
    <div class="playlist-contents flex-1 flex flex-col bg-white dark:bg-surface-light overflow-hidden">
      <!-- Empty State -->
      <div v-if="!selectedPlaylist" class="flex-1 flex items-center justify-center">
        <div class="text-center">
          <div class="w-20 h-20 mx-auto rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center mb-4">
            <span class="material-symbols-outlined text-4xl text-gray-400">queue_music</span>
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Select a Playlist</h3>
          <p class="text-gray-500 mb-4">Choose a playlist from the sidebar to view its contents</p>
          <button @click="showCreateModal = true" class="px-4 py-2 bg-primary text-on-accent rounded-lg">
            Create Playlist
          </button>
        </div>
      </div>
      
      <!-- Playlist Header -->
      <div v-else class="playlist-header p-6 border-b border-gray-200 dark:border-border-dark">
        <div class="flex gap-6">
          <!-- Cover Art -->
          <div class="relative group shrink-0">
            <PlaylistCover
              :src="playlistPresentation[selectedPlaylist.id]?.providerImageUrl ?? null"
              :tiles="playlistPresentation[selectedPlaylist.id]?.tiles ?? []"
              :item-id="selectedPlaylist.id"
              :alt="selectedPlaylist.name"
              size-class="w-32 h-32 shadow-lg"
              rounded="rounded-xl"
              icon="queue_music"
            />

          </div>
          
          <!-- Playlist Info -->
          <div class="flex-1">
            <div class="flex items-start justify-between">
              <div>
                <p class="text-xs text-gray-500 uppercase tracking-wide mb-1">
                  {{ selectedPlaylist.smart ? 'Smart Playlist' : selectedPlaylistServiceName ? `Imported from ${selectedPlaylistServiceName}` : 'Playlist' }}
                </p>
                <h1 
                  v-if="!isEditingName"
                  @click="startEditName"
                  class="text-2xl font-bold text-gray-900 dark:text-white mb-1 cursor-pointer hover:text-primary"
                >
                  {{ selectedPlaylist.name }}
                </h1>
                <input 
                  v-else
                  ref="nameInput"
                  v-model="editingName"
                  @blur="saveName"
                  @keyup.enter="saveName"
                  @keyup.escape="cancelEditName"
                  class="text-2xl font-bold bg-transparent text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50 rounded px-1"
                >
                <p
                  v-if="!isEditingDescription"
                  @click="startEditDescription"
                  class="text-sm text-gray-500 mb-2 cursor-pointer hover:text-primary"
                >
                  {{ selectedPlaylist.description || 'Add description...' }}
                </p>
                <textarea
                  v-else
                  ref="descriptionInput"
                  v-model="editingDescription"
                  @blur="saveDescription"
                  @keyup.escape="cancelEditDescription"
                  rows="2"
                  class="w-full text-sm bg-transparent text-gray-700 dark:text-gray-300 mb-2 focus:outline-none focus:ring-2 focus:ring-primary/50 rounded px-1 resize-none"
                ></textarea>
              </div>
              
              <!-- Service Badges -->
              <div v-if="selectedPlaylist.linkedServices?.length" class="flex gap-1">
                <div 
                  v-for="service in selectedPlaylist.linkedServices" 
                  :key="service"
                  :class="['w-6 h-6 rounded-full flex items-center justify-center', getServiceColor(service)]"
                  :title="`Synced with ${service}`"
                >
                  <span class="text-xs text-white font-bold">{{ service[0] }}</span>
                </div>
              </div>
            </div>
            
            <!-- Stats -->
            <div class="flex items-center gap-4 text-sm text-gray-500 mb-4">
              <span>{{ selectedPlaylist.track_count }} tracks</span>
              <span>{{ (selectedPlaylist as any).duration || '0m' }}</span>
              <span>{{ (selectedPlaylist as any).size || '0 MB' }}</span>
              <span v-if="selectedPlaylist.last_synced" class="flex items-center gap-1">
                <span class="material-symbols-outlined text-xs">sync</span>
                {{ selectedPlaylist.last_synced }}
              </span>
            </div>
            
            <!-- Actions -->
            <div class="flex items-center gap-3">
              <button @click="playAll" :disabled="isPlaylistPlaybackBusy" class="px-5 py-2 bg-primary hover:bg-primary-hover text-on-accent font-medium rounded-full flex items-center gap-2 disabled:opacity-50">
                <span class="material-symbols-outlined">{{ isPlaylistPlaybackBusy ? 'progress_activity' : 'play_arrow' }}</span>
                {{ isPlaylistPlaybackBusy ? 'Loading…' : 'Play All' }}
              </button>
              <button @click="openDownloadModal" class="px-5 py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-full flex items-center gap-2">
                <span class="material-symbols-outlined">download</span>
                Download Playlist
              </button>
              <button @click="shufflePlay" :disabled="isPlaylistPlaybackBusy" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-full disabled:opacity-50">
                <span class="material-symbols-outlined">shuffle</span>
              </button>
              <button v-if="canDeleteSelectedPlaylist" @click="deleteSelectedPlaylist" class="p-2 hover:bg-red-50 dark:hover:bg-red-500/10 rounded-full text-red-500" aria-label="Delete playlist" title="Delete playlist">
                <span class="material-symbols-outlined">delete</span>
              </button>
            </div>

            <!-- S201: Download Playlist result banner -->
            <div v-if="downloadResult" class="mt-3 p-3 rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight text-sm">
              <!-- Mode A: M3U export -->
              <template v-if="downloadResult.mode === 'm3u'">
                <div class="flex items-center justify-between gap-2">
                  <span class="text-gray-700 dark:text-gray-300">
                    M3U exported · {{ downloadResult.verified }}/{{ downloadResult.total }} tracks verified<template v-if="downloadResult.filePath"> → {{ downloadResult.filePath }}</template>
                  </span>
                  <span class="flex items-center gap-1 shrink-0">
                    <button
                      v-if="downloadResult.missing.length"
                      @click="showMissingDetails = !showMissingDetails"
                      class="px-2 py-1 text-xs rounded-lg bg-primary/10 text-primary hover:bg-primary/20 flex items-center gap-1"
                    >
                      <span class="material-symbols-outlined text-sm">{{ showMissingDetails ? 'expand_less' : 'expand_more' }}</span>
                      Missing ({{ downloadResult.missing.length }})
                    </button>
                    <button @click="downloadResult = null" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded" aria-label="Close">
                      <span class="material-symbols-outlined text-base text-gray-500">close</span>
                    </button>
                  </span>
                </div>
                <div v-if="showMissingDetails && downloadResult.missing.length" class="mt-2 pt-2 border-t border-gray-200 dark:border-border-dark max-h-40 overflow-y-auto custom-scrollbar">
                  <ul class="space-y-1">
                    <li v-for="m in downloadResult.missing" :key="m.track_id" class="flex items-center justify-between gap-2 text-xs">
                      <span class="truncate text-gray-700 dark:text-gray-300">{{ m.title }}<template v-if="m.artist_name"> — {{ m.artist_name }}</template></span>
                      <span
                        class="shrink-0 px-2 py-0.5 rounded-full text-[10px] font-medium"
                        :class="m.reason === 'sin_archivo_local' ? 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400' : 'bg-amber-100 dark:bg-amber-500/10 text-amber-700 dark:text-amber-400'"
                      >
                        {{ missingReasonLabel(m.reason) }}
                      </span>
                    </li>
                  </ul>
                </div>
              </template>
              <!-- Mode B: queue engine summary -->
              <template v-else>
                <div class="flex items-center justify-between gap-2">
                  <span class="text-gray-700 dark:text-gray-300">
                    Download queue: {{ downloadResult.enqueued }} queued<template v-if="downloadResult.deduplicated !== undefined"> · {{ downloadResult.deduplicated }} already downloaded or queued</template><template v-if="downloadResult.skipped > 0"> · {{ downloadResult.skipped }} skipped</template>.
                  </span>
                  <button @click="downloadResult = null" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded shrink-0" aria-label="Close">
                    <span class="material-symbols-outlined text-base text-gray-500">close</span>
                  </button>
                </div>
              </template>
            </div>
          </div>
        </div>
      </div>
      
      <!-- Track List -->
      <div v-if="selectedPlaylist" class="flex-1 min-h-0 flex flex-col">
        <!-- Track Table Header -->
        <div class="shrink-0 bg-white dark:bg-surface-light border-b border-gray-200 dark:border-border-dark px-4 py-2 flex items-center text-xs font-semibold text-gray-500 uppercase">
          <div class="w-10">#</div>
          <div class="w-12"></div>
          <div class="flex-1">Title</div>
          <div class="w-40">Album</div>
          <div class="w-32">Added</div>
          <div class="w-20 text-right">Duration</div>
          <div class="w-10"></div>
        </div>

        <div
          ref="playlistTracksContainer"
          data-testid="playlist-tracks-scroller"
          class="flex-1 min-h-0 overflow-y-auto custom-scrollbar relative"
          style="contain: strict;"
          @scroll="handlePlaylistTracksScroll"
        >
          <!-- Loading: until tracks arrive we cannot claim the list is empty -->
          <div v-if="isLoading" class="min-h-full flex items-center justify-center py-16" data-testid="playlist-tracks-loading">
            <div class="text-center">
              <span class="material-symbols-outlined text-3xl text-gray-400 animate-spin block mb-4">progress_activity</span>
              <p class="text-gray-500 text-sm">Loading tracks…</p>
            </div>
          </div>

          <!-- Error: a load failure is not an empty playlist -->
          <div v-else-if="tracksError" class="min-h-full flex items-center justify-center py-16 px-6" data-testid="playlist-tracks-error">
            <div class="text-center">
              <div class="w-16 h-16 mx-auto rounded-full bg-red-50 dark:bg-surface-highlight flex items-center justify-center mb-4">
                <span class="material-symbols-outlined text-3xl text-error">error</span>
              </div>
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Could not load tracks</h3>
              <p class="text-gray-500 text-sm mb-4">{{ tracksError }}</p>
              <button @click="loadPlaylistTracks" class="px-4 py-2 bg-primary text-on-accent rounded-lg text-sm">
                Try Again
              </button>
            </div>
          </div>

          <!-- Empty Playlist -->
          <div v-else-if="!playlistTracks.length" class="min-h-full flex items-center justify-center py-16" data-testid="playlist-tracks-empty">
            <div class="text-center">
              <div class="w-16 h-16 mx-auto rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center mb-4">
                <span class="material-symbols-outlined text-3xl text-gray-400">music_note</span>
              </div>
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">This playlist is empty</h3>
              <p class="text-gray-500 text-sm mb-4">Drag tracks here or click Add Tracks</p>
              <button v-if="canMutateSelectedPlaylist" @click="addTracksById" class="px-4 py-2 bg-primary text-on-accent rounded-lg text-sm">
                Add Tracks
              </button>
            </div>
          </div>

          <template v-else>
            <div :style="{ height: totalPlaylistTracksHeight + 'px', position: 'relative', width: '100%' }">
              <div :style="{ transform: `translateY(${virtualOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
                <div
                  v-for="(track, index) in visiblePlaylistTracks"
                  :key="track.rowKey"
                  :data-track-id="track.id"
                  :style="{ height: PLAYLIST_ROW_HEIGHT + 'px' }"
                  class="track-row px-4 py-2 flex items-center hover:bg-gray-50 dark:hover:bg-surface-highlight group cursor-pointer box-border"
                  :class="{ 'bg-primary/5': selectedTracks.includes(track.id) }"
                  draggable="true"
                  @dragstart="onTrackDragStart($event, track, virtualStartIndex + index)"
                  @dragover.prevent
                  @drop="onTrackDrop($event, virtualStartIndex + index)"
                >
                  <!-- Index / Play -->
                  <div class="w-10 text-sm text-gray-500 group-hover:hidden">{{ virtualStartIndex + index + 1 }}</div>
                  <div class="w-10 hidden group-hover:block">
                    <button @click.stop="playTrack(track)" :disabled="isPlaylistPlaybackBusy" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded disabled:opacity-50">
                      <span class="material-symbols-outlined text-primary">play_arrow</span>
                    </button>
                  </div>

                  <!-- Album Art -->
                  <div class="w-12 pr-3">
                    <TrackCover
                      :src="track.albumArt"
                      :item-id="track.id"
                      :alt="track.album || track.title"
                      size-class="w-10 h-10"
                    />
                  </div>

                  <!-- Title & Artist -->
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ track.title }}</p>
                    <p class="text-xs text-gray-500 truncate">{{ track.artist }}</p>
                  </div>

                  <!-- Album -->
                  <div class="w-40 text-sm text-gray-500 truncate">{{ track.album }}</div>

                  <!-- Added Date -->
                  <div class="w-32 text-sm text-gray-500">{{ track.addedDate }}</div>

                  <!-- Duration -->
                  <div class="w-20 text-right text-sm text-gray-500">{{ track.duration }}</div>

                  <!-- Actions (S194: fixed, always visible) -->
                  <div class="w-10 flex justify-end">
                    <button v-if="canMutateSelectedPlaylist" @click.stop="removeTrack(track)" class="p-1 hover:bg-red-50 dark:hover:bg-red-500/10 rounded" :aria-label="`Remove ${track.title} from playlist`" title="Remove from playlist">
                      <span class="material-symbols-outlined text-red-400 text-lg">remove_circle</span>
                    </button>
                  </div>
                </div>
              </div>

              <div
                v-if="playlistFooterHeight > 0"
                class="absolute left-0 right-0 flex items-center justify-center gap-2 px-4 text-sm text-gray-500"
                :style="{ top: playlistTracks.length * PLAYLIST_ROW_HEIGHT + 'px', height: PLAYLIST_FOOTER_HEIGHT + 'px' }"
              >
                <div v-if="isLoadingMore" class="flex items-center gap-2" data-testid="playlist-tracks-loading-more">
                  <span class="material-symbols-outlined text-lg animate-spin">progress_activity</span>
                  Loading more tracks…
                </div>
                <div v-else-if="tracksLoadMoreError" class="flex items-center gap-2 min-w-0" data-testid="playlist-tracks-load-more-error">
                  <span class="truncate">{{ tracksLoadMoreError }}</span>
                  <button @click="loadMorePlaylistTracks" class="shrink-0 px-3 py-1 rounded-lg bg-primary/10 text-primary hover:bg-primary/20">
                    Retry
                  </button>
                </div>
                <div v-else-if="!hasMoreTracks">
                  Showing all {{ playlistTotal.toLocaleString() }} tracks
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>
    </div>
    
    <!-- Create Playlist Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showCreateModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="showCreateModal = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Create Playlist</h3>
            </div>
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Name</label>
                <input 
                  v-model="newPlaylist.name"
                  type="text"
                  placeholder="My Playlist"
                  class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                >
              </div>
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Description (optional)</label>
                <textarea 
                  v-model="newPlaylist.description"
                  rows="3"
                  placeholder="Add a description..."
                  class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50 resize-none"
                ></textarea>
              </div>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="newPlaylist.public" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-600 dark:text-gray-400">Make playlist public</span>
              </label>
            </div>
            <div class="px-6 pb-6 flex gap-3">
              <button @click="showCreateModal = false" class="flex-1 py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg">
                Cancel
              </button>
              <button @click="createPlaylist" class="flex-1 py-2 bg-primary hover:bg-primary-hover text-on-accent rounded-lg font-medium">
                Create
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Smart Playlist Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showSmartModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="showSmartModal = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Create Smart Playlist</h3>
            </div>
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Name</label>
                <input 
                  v-model="smartPlaylist.name"
                  type="text"
                  placeholder="Smart Playlist"
                  class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                >
              </div>
              
              <!-- Rules -->
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Match tracks where:</label>
                <div class="space-y-2">
                  <div v-for="(rule, index) in smartPlaylist.rules" :key="index" class="rule-builder flex items-center gap-2">
                    <select v-model="rule.field" class="px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-sm text-gray-900 dark:text-white">
                      <option value="genre">Genre</option>
                      <option value="quality">Quality</option>
                      <option value="service">Service</option>
                      <option value="addedDate">Added Date</option>
                      <option value="hasLyrics">Has Lyrics</option>
                      <option value="artist">Artist</option>
                      <option value="year">Year</option>
                    </select>
                    <select v-model="rule.operator" class="px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-sm text-gray-900 dark:text-white">
                      <option value="contains">contains</option>
                      <option value="is">is</option>
                      <option value="isNot">is not</option>
                      <option value="greaterThan">greater than</option>
                      <option value="lessThan">less than</option>
                    </select>
                    <input 
                      v-model="rule.value"
                      type="text"
                      class="flex-1 px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-sm text-gray-900 dark:text-white"
                    >
                    <button @click="removeRule(index)" class="p-1 text-red-500 hover:bg-red-50 dark:hover:bg-red-500/10 rounded">
                      <span class="material-symbols-outlined text-lg">close</span>
                    </button>
                  </div>
                </div>
                <button @click="addRule" class="mt-2 text-sm text-primary hover:underline flex items-center gap-1">
                  <span class="material-symbols-outlined text-sm">add</span>
                  Add Rule
                </button>
              </div>
              
              <!-- Preview -->
              <div class="p-3 bg-gray-50 dark:bg-surface-highlight rounded-lg">
                <p class="text-sm text-gray-600 dark:text-gray-400">
                  <span class="font-medium text-gray-900 dark:text-white">Preview:</span> {{ smartPreviewCount }} tracks match
                </p>
              </div>
              
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="smartPlaylist.autoUpdate" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-600 dark:text-gray-400">Auto-update when library changes</span>
              </label>
            </div>
            <div class="px-6 pb-6 flex gap-3">
              <button @click="showSmartModal = false" class="flex-1 py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg">
                Cancel
              </button>
              <button @click="createSmartPlaylist" class="flex-1 py-2 bg-primary hover:bg-primary-hover text-on-accent rounded-lg font-medium">
                Create
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Import Playlist Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showImportModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="showImportModal = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Import Playlist</h3>
            </div>
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">Playlist URL</label>
                <input 
                  v-model="importUrl"
                  type="url"
                  placeholder="https://open.spotify.com/playlist/..."
                  class="w-full px-3 py-2 bg-gray-100 dark:bg-surface-highlight rounded-lg text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                >
              </div>
              <p class="text-xs text-gray-500">Imports playlists from the Spotify, Qobuz, Tidal, or Deezer account identified by the URL.</p>
            </div>
            <div class="px-6 pb-6 flex gap-3">
              <button @click="showImportModal = false" class="flex-1 py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg">
                Cancel
              </button>
              <button @click="importPlaylist" :disabled="isImporting" class="flex-1 py-2 bg-primary hover:bg-primary-hover text-on-accent rounded-lg font-medium disabled:opacity-50">
                {{ isImporting ? 'Importing…' : 'Import' }}
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- S201: Download Playlist Modal — two modes -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showDownloadModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="closeDownloadModal">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Download Playlist</h3>
              <p class="text-xs text-gray-500 truncate">{{ selectedPlaylist?.name }}</p>
            </div>
            <div class="p-6 space-y-3">
              <!-- Mode A -->
              <button
                :disabled="isDownloadBusy"
                @click="downloadExistingAsM3u"
                class="w-full text-left p-4 rounded-xl border border-gray-200 dark:border-border-dark hover:border-primary hover:bg-primary/5 transition-colors disabled:opacity-50 disabled:pointer-events-none flex gap-3"
              >
                <span class="material-symbols-outlined text-primary mt-0.5">save</span>
                <span>
                  <span class="block text-sm font-medium text-gray-900 dark:text-white">Only the ones I already have</span>
                  <span class="block text-xs text-gray-500 mt-1">Exports an .m3u file with only the tracks whose local file exists on your disk. Downloads nothing new.</span>
                </span>
              </button>
              <!-- Mode B -->
              <button
                :disabled="isDownloadBusy"
                @click="downloadMissingTracks"
                class="w-full text-left p-4 rounded-xl border border-gray-200 dark:border-border-dark hover:border-primary hover:bg-primary/5 transition-colors disabled:opacity-50 disabled:pointer-events-none flex gap-3"
              >
                <span class="material-symbols-outlined text-primary mt-0.5">cloud_download</span>
                <span>
                  <span class="block text-sm font-medium text-gray-900 dark:text-white">Download missing tracks</span>
                  <span class="block text-xs text-gray-500 mt-1">Queues only the tracks you don't have yet in the regular download queue. Requires a connection.</span>
                </span>
              </button>
              <p v-if="isDownloadBusy" class="text-xs text-gray-500 flex items-center gap-2">
                <span class="material-symbols-outlined text-sm animate-spin">progress_activity</span>
                Processing…
              </p>
            </div>
            <div class="px-6 pb-6">
              <button @click="closeDownloadModal" class="w-full py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg" :disabled="isDownloadBusy">
                Close
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Playlist options menu (replaces direct deletion) -->
    <Teleport to="body">
      <div
        v-if="playlistMenu"
        class="fixed z-[300] w-50 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-highlight py-1 shadow-xl"
        :style="{ left: `${playlistMenu.x}px`, top: `${playlistMenu.y}px` }"
        data-testid="playlist-options-menu"
        @click.stop
      >
        <button class="w-full flex items-center gap-3 px-4 py-2 text-sm text-left hover:bg-gray-100 dark:hover:bg-surface-dark" @click="menuPlayPlaylist">
          <span class="material-symbols-outlined text-base">play_arrow</span>
          <span>Play</span>
        </button>
        <button class="w-full flex items-center gap-3 px-4 py-2 text-sm text-left hover:bg-gray-100 dark:hover:bg-surface-dark" @click="menuSelectPlaylist">
          <span class="material-symbols-outlined text-base">open_in_new</span>
          <span>Open</span>
        </button>
        <button
          class="w-full flex items-center gap-3 px-4 py-2 text-sm text-left hover:bg-gray-100 dark:hover:bg-surface-dark"
          :disabled="!canMutateMenuPlaylist"
          :class="{ 'opacity-40 pointer-events-none': !canMutateMenuPlaylist }"
          @click="menuStartEditName"
        >
          <span class="material-symbols-outlined text-base">edit</span>
          <span>Rename</span>
        </button>
        <div class="my-1 border-t border-gray-200 dark:border-border-dark"></div>
        <button
          class="w-full flex items-center gap-3 px-4 py-2 text-sm text-left text-error hover:bg-red-50 dark:hover:bg-red-500/10"
          :disabled="!canDeleteMenuPlaylist"
          :class="{ 'opacity-40 pointer-events-none': !canDeleteMenuPlaylist }"
          @click="menuDeletePlaylist"
        >
          <span class="material-symbols-outlined text-base">delete</span>
          <span>Delete</span>
        </button>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute, type LocationQueryRaw } from 'vue-router'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { libraryApi } from '@/api/library'
import { playlistsApi, exportPlaylistM3u, importPlaylists, type MissingPlaylistFile } from '@/api/playlists'
import { addToQueue, addBatchToQueue } from '@/api/queue'
import type { Playlist, LibraryTrack, LibraryPage } from '@/api/types'
// R13: unified `confirm`/`prompt` (in-app dialog) instead of
// `window.confirm`/`window.prompt`, which open outside the window.
import { useToast, confirm, prompt } from '@/composables/useToast'
import { usePlayer } from '@/composables/usePlayer'
import { useVirtualWindow } from '@/composables/useVirtualWindow'
import TrackCover from '@/components/TrackCover.vue'
import PlaylistCover from '@/components/PlaylistCover.vue'
import {
    aggregatePlaylists,
    formatPlaylistServiceName,
    getPlaylistServiceKey,
    reconcilePlaylistMosaic,
    type PlaylistMosaicCache,
    type PlaylistCoverTrack,
    type PlaylistPresentation,
} from '@/utils/playlistCatalog'

/** Synthetic Favorites entry ID: it does not exist in the `playlists` table. */
const FAVORITES_PLAYLIST_ID = -1

const toast = useToast()
const player = usePlayer()
// Chained to the router (optional chaining like SettingsView/LibraryView:
// the view also mounts in tests without a router instance).
const router = useRouter()
const route = useRoute()

// State
const searchQuery = ref('')
const selectedPlaylist = ref<any>(null)
const selectedTracks = ref<number[]>([])
const showCreateModal = ref(false)
const showSmartModal = ref(false)
const showImportModal = ref(false)

// S201: "Download Playlist" — two modes
const showDownloadModal = ref(false)
const isExportingM3u = ref(false)
const isQueueingMissing = ref(false)
const isDownloadBusy = computed(() => isExportingM3u.value || isQueueingMissing.value)

type DownloadPlaylistBanner =
  | { mode: 'm3u'; total: number; verified: number; missing: MissingPlaylistFile[]; filePath?: string | null }
  | { mode: 'queue'; enqueued: number; deduplicated?: number; skipped: number }
const downloadResult = ref<DownloadPlaylistBanner | null>(null)
const showMissingDetails = ref(false)
const isEditingName = ref(false)
const isEditingDescription = ref(false)
const editingName = ref('')
const editingDescription = ref('')
const nameInput = ref<HTMLInputElement | null>(null)
const descriptionInput = ref<HTMLTextAreaElement | null>(null)

// Expanded categories
const expandedCategories = ref<Record<string, boolean>>({
  myPlaylists: true,
  spotify: true,
  qobuz: false,
  smart: true,
})

// New playlist form
const newPlaylist = ref({
  name: '',
  description: '',
  public: false,
})

// Smart playlist form
const smartPlaylist = ref({
  name: '',
  rules: [{ field: 'genre', operator: 'contains', value: '' }],
  autoUpdate: true,
})

// Import
const importUrl = ref('')
const isImporting = ref(false)

const favoritesPlaylist = ref<any>({
  id: FAVORITES_PLAYLIST_ID,
  name: 'Favorites',
  track_count: 0,
  duration: '0h 0m',
  size: '0 GB',
})

const playlists = ref<Playlist[]>([])
const playlistPresentation = ref<Record<number, PlaylistPresentation>>({})
const playlistMosaicCache: PlaylistMosaicCache = new Map()
const playlistCatalog = computed(() => aggregatePlaylists(playlists.value, playlistPresentation.value))
const myPlaylists = computed(() => playlistCatalog.value.local)
const importedServices = computed(() => playlistCatalog.value.services)
const smartPlaylists = computed(() => playlistCatalog.value.smart)
const playlistTracks = ref<any[]>([])
const playlistTotal = ref(0)
const nextPlaylistOffset = ref(0)
const hasMoreTracks = ref(false)
const isLoading = ref(false)
const isLoadingMore = ref(false)
const isPlaylistPlaybackBusy = ref(false)
/** Last track load failure; distinguishes "empty" from "could not read". */
const tracksError = ref<string | null>(null)
/** A later-page failure keeps the rows already loaded and can be retried. */
const tracksLoadMoreError = ref<string | null>(null)
const PLAYLIST_PAGE_SIZE = 100
const PLAYLIST_ARTWORK_PAGE_SIZE = 4
const PLAYLIST_ROW_HEIGHT = 56
const PLAYLIST_OVERSCAN = 6
const PLAYLIST_LOAD_MORE_THRESHOLD_PX = 280
const PLAYLIST_FOOTER_HEIGHT = 56
let playlistLoadSeq = 0
let playlistCatalogLoadSeq = 0
let playlistPlaybackSeq = 0

function updatePlaylistPresentation(playlistId: number, patch: Partial<PlaylistPresentation>) {
  playlistPresentation.value = {
    ...playlistPresentation.value,
    [playlistId]: { ...playlistPresentation.value[playlistId], ...patch },
  }
}

function updatePlaylistMosaic(playlistId: number, tracks: readonly PlaylistCoverTrack[]) {
  const tiles = reconcilePlaylistMosaic(playlistMosaicCache, playlistId, tracks)
  updatePlaylistPresentation(playlistId, { tiles })
}

function updateLoadedPlaylistMosaic() {
  const playlistId = selectedPlaylist.value?.id
  if (typeof playlistId !== 'number') return
  updatePlaylistMosaic(playlistId, playlistTracks.value.map(track => ({
    id: track.id,
    position: track.position,
    cover_art_url: track.albumArt || null,
  })))
}

function reconcilePlaylistCount(playlistId: number, total: number) {
  const playlist = playlists.value.find(item => item.id === playlistId)
  if (playlist) playlist.track_count = total
  if (selectedPlaylist.value?.id === playlistId) {
    selectedPlaylist.value = { ...selectedPlaylist.value, track_count: total }
  }
}

function cancelPendingPlaylistPlayback() {
  playlistPlaybackSeq += 1
  isPlaylistPlaybackBusy.value = false
}

const playlistFooterHeight = computed(() =>
  playlistTracks.value.length > 0 && (isLoadingMore.value || tracksLoadMoreError.value || !hasMoreTracks.value)
    ? PLAYLIST_FOOTER_HEIGHT
    : 0
)

const selectedPlaylistServiceName = computed(() => {
  const serviceKey = selectedPlaylist.value?.serviceKey
  return serviceKey ? formatPlaylistServiceName(serviceKey) : null
})

const canMutateSelectedPlaylist = computed(() => {
  const playlist = selectedPlaylist.value
  return Boolean(playlist && typeof playlist.id === 'number' && playlist.id > 0 && !playlist.smart)
})

const canDeleteSelectedPlaylist = computed(() => {
  const playlist = selectedPlaylist.value
  return Boolean(playlist && typeof playlist.id === 'number' && playlist.id > 0)
})

// Map LibraryTrack to UI Track
function mapToTrack(item: LibraryTrack, index: number) {
  const durationSec = (item.duration_ms ?? 0) / 1000;
  const mins = Math.floor(durationSec / 60);
  const secs = Math.floor(durationSec % 60);

  // PL02 (#155): la misma pista puede aparecer varias veces en una playlist.
  // `position` distingue las ocurrencias y `rowKey` da a cada fila una clave
  // estable dentro de la carga (un `:key` por track.id duplicaría claves).
  const position = item.position ?? null;
  return {
    id: item.id,
    position,
    rowKey: position === null ? `${item.id}:i${index}` : `${item.id}:p${position}`,
    title: item.title,
    artist: item.artist_name || 'Unknown Artist',
    album: item.album_name || 'Unknown Album',
    duration: `${mins}:${secs.toString().padStart(2, '0')}`,
    addedDate: 'Recently',
    albumArt: item.cover_art_url || '',
  };
}

const {
  containerRef: playlistTracksContainer,
  totalHeight: totalPlaylistTracksHeight,
  startIndex: virtualStartIndex,
  offsetY: virtualOffsetY,
  visibleItems: visiblePlaylistTracks,
  handleScroll: handlePlaylistTracksScroll,
  reset: resetPlaylistVirtualScroll,
} = useVirtualWindow(playlistTracks, {
  rowHeight: PLAYLIST_ROW_HEIGHT,
  overscan: PLAYLIST_OVERSCAN,
  footerHeight: playlistFooterHeight,
  onScrollMeasured: (scrollBottom) => {
    if (scrollBottom <= PLAYLIST_LOAD_MORE_THRESHOLD_PX) {
      void loadMorePlaylistTracks()
    }
  },
})

// Computed
const totalPlaylists = computed(() => {
  return 1 + myPlaylists.value.length +
    importedServices.value.reduce((acc, s) => acc + s.playlists.length, 0) +
    smartPlaylists.value.length
})

const filteredMyPlaylists = computed(() => {
  if (!searchQuery.value) return myPlaylists.value
  return myPlaylists.value.filter(p =>
    p.name.toLowerCase().includes(searchQuery.value.toLowerCase())
  )
})

const filteredSmartPlaylists = computed(() => {
  if (!searchQuery.value) return smartPlaylists.value
  return smartPlaylists.value.filter(p =>
    p.name.toLowerCase().includes(searchQuery.value.toLowerCase())
  )
})

const filteredImportedServices = computed(() => {
  if (!searchQuery.value) return importedServices.value
  const query = searchQuery.value.toLowerCase()
  return importedServices.value
    .map(service => ({
      ...service,
      playlists: service.playlists.filter(p => p.name.toLowerCase().includes(query)),
    }))
    .filter(service => service.playlists.length > 0)
})

const smartPreviewCount = ref(0)
let previewDebounceTimer: ReturnType<typeof setTimeout> | null = null

async function updateSmartPreview() {
  try {
    const rules = smartPlaylist.value.rules || []
    const rulesJson = JSON.stringify(rules)
    const count = await playlistsApi.previewSmartPlaylistCount(rulesJson)
    smartPreviewCount.value = count
  } catch (err) {
    console.error('Failed to preview smart playlist count', err)
    smartPreviewCount.value = 0
  }
}

watch(
  () => smartPlaylist.value.rules,
  () => {
    if (previewDebounceTimer) clearTimeout(previewDebounceTimer)
    previewDebounceTimer = setTimeout(() => {
      updateSmartPreview()
    }, 250)
  },
  { deep: true, immediate: true }
)

onUnmounted(() => {
  playlistLoadSeq += 1
  playlistCatalogLoadSeq += 1
  playlistMosaicCache.clear()
  cancelPendingPlaylistPlayback()
  if (previewDebounceTimer) {
    clearTimeout(previewDebounceTimer)
    previewDebounceTimer = null
  }
  document.removeEventListener('click', closePlaylistMenu)
  document.removeEventListener('keydown', onMenuKeydown)
})

watch(
  () => showSmartModal.value,
  (isOpen) => {
    if (isOpen) {
      updateSmartPreview()
    }
  }
)

// Methods
async function fetchPlaylistPage(playlistId: number, offset: number, limit: number): Promise<LibraryPage> {
  if (playlistId === FAVORITES_PLAYLIST_ID) {
    return libraryApi.getFavoriteTracks(offset, limit)
  }
  return libraryApi.getPlaylistTracks(playlistId, offset, limit)
}

async function selectPlaylist(playlist: any) {
  const seq = ++playlistLoadSeq
  cancelPendingPlaylistPlayback()
  selectedPlaylist.value = playlist
  selectedTracks.value = []
  playlistTracks.value = []
  playlistTotal.value = 0
  nextPlaylistOffset.value = 0
  hasMoreTracks.value = false
  tracksError.value = null
  tracksLoadMoreError.value = null
  isLoading.value = true
  isLoadingMore.value = false
  resetPlaylistVirtualScroll()

  try {
    const page = await fetchPlaylistPage(playlist.id, 0, PLAYLIST_PAGE_SIZE)
    if (seq !== playlistLoadSeq) return

    playlistTracks.value = page.tracks.map((track, index) => mapToTrack(track, page.offset + index))
    playlistTotal.value = page.total
    nextPlaylistOffset.value = page.offset + page.tracks.length
    hasMoreTracks.value = page.has_more && page.tracks.length > 0
    updatePlaylistMosaic(playlist.id, page.tracks)

    // PL02 (#155): `total` cuenta ocurrencias, no pistas distintas.
    if (playlist.id === FAVORITES_PLAYLIST_ID) {
      favoritesPlaylist.value.track_count = page.total
    } else {
      reconcilePlaylistCount(playlist.id, page.total)
    }
  } catch (error) {
    if (seq !== playlistLoadSeq) return
    playlistTracks.value = []
    tracksError.value = String(error)
    toast.error('Failed to load playlist tracks', String(error))
  } finally {
    if (seq === playlistLoadSeq) isLoading.value = false
  }
}

/** Reloads the current selection's tracks after an error or a retry. */
async function loadPlaylistTracks() {
  if (!selectedPlaylist.value) return
  await selectPlaylist(selectedPlaylist.value)
}

async function loadMorePlaylistTracks() {
  const playlist = selectedPlaylist.value
  if (!playlist || isLoading.value || isLoadingMore.value || tracksError.value || !hasMoreTracks.value) return

  const seq = playlistLoadSeq
  const offset = nextPlaylistOffset.value
  isLoadingMore.value = true
  tracksLoadMoreError.value = null
  try {
    const page = await fetchPlaylistPage(playlist.id, offset, PLAYLIST_PAGE_SIZE)
    if (seq !== playlistLoadSeq) return
    if (page.tracks.length === 0) {
      throw new Error('Playlist returned an empty page before the end')
    }

    playlistTracks.value.push(...page.tracks.map((track, index) => mapToTrack(track, offset + index)))
    playlistTotal.value = page.total
    nextPlaylistOffset.value = offset + page.tracks.length
    hasMoreTracks.value = page.has_more
    updateLoadedPlaylistMosaic()
  } catch (error) {
    if (seq !== playlistLoadSeq) return
    tracksLoadMoreError.value = String(error)
    toast.error('Failed to load more playlist tracks', String(error))
  } finally {
    if (seq === playlistLoadSeq) isLoadingMore.value = false
  }
}

async function loadPlaylists() {
  const seq = ++playlistCatalogLoadSeq
  try {
    const loadedPlaylists = await libraryApi.getPlaylists()
    if (seq !== playlistCatalogLoadSeq) return

    playlists.value = loadedPlaylists
    playlistPresentation.value = {}
    playlistMosaicCache.clear()
    if (typeof selectedPlaylist.value?.id === 'number' && selectedPlaylist.value.id > 0) {
      updateLoadedPlaylistMosaic()
    }

    const artwork = await Promise.all(loadedPlaylists.map(async playlist => {
      const serviceKey = playlist.is_smart ? null : getPlaylistServiceKey(playlist)
      const [trackPage, detail] = await Promise.all([
        libraryApi.getPlaylistTracks(playlist.id, 0, PLAYLIST_ARTWORK_PAGE_SIZE).catch(error => {
          console.error(`Failed to load artwork for playlist ${playlist.id}:`, error)
          return null
        }),
        serviceKey
          ? playlistsApi.getPlaylist(playlist.id).catch(error => {
              console.error(`Failed to load provider artwork for playlist ${playlist.id}:`, error)
              return null
            })
          : Promise.resolve(null),
      ])
      return { playlist, trackPage, providerImageUrl: detail?.image_url ?? null, serviceKey }
    }))
    if (seq !== playlistCatalogLoadSeq) return

    for (const { playlist, trackPage, providerImageUrl, serviceKey } of artwork) {
      if (trackPage) {
        reconcilePlaylistCount(playlist.id, trackPage.total)
        if (selectedPlaylist.value?.id !== playlist.id) {
          updatePlaylistMosaic(playlist.id, trackPage.tracks)
        }
      }
      if (serviceKey) updatePlaylistPresentation(playlist.id, { providerImageUrl })
    }
  } catch (error) {
    console.error('Failed to load playlists:', error)
  }
}

const isSyncing = ref(false)

async function triggerSyncPlaylists() {
  isSyncing.value = true
  try {
    const res = await playlistsApi.syncPlaylists()
    if (res.playlists_synced > 0) {
      toast.success(res.message)
    } else {
      toast.info(res.message || 'No linked service playlists found.')
    }
    await loadPlaylists()
  } catch (e: any) {
    toast.error(`Sync failed: ${e}`)
  } finally {
    isSyncing.value = false
  }
}

onMounted(() => {
  loadPlaylists()
  void consumeCreateQuery()
  document.addEventListener('click', closePlaylistMenu)
  document.addEventListener('keydown', onMenuKeydown)
})

// Item 46: the "New Playlist" quick action (App.handleNewPlaylist) navigates to
// /playlists?create=1. Mounting with that flag opens the creation form and
// the parameter is ALWAYS consumed so reloading or going back does not
// reopen it (same pattern as LibraryView with ?track=).
async function consumeCreateQuery() {
  if (route?.query?.create !== '1') return

  const rest: LocationQueryRaw = { ...route.query }
  delete rest.create
  await router.replace({ query: rest })

  showCreateModal.value = true
}

function onMenuKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && playlistMenu.value) closePlaylistMenu()
}

function startEditName() {
  if (selectedPlaylist.value?.id === FAVORITES_PLAYLIST_ID || selectedPlaylist.value?.smart) return
  editingName.value = selectedPlaylist.value.name
  isEditingName.value = true
  nextTick(() => nameInput.value?.focus())
}

async function saveName() {
  if (!canMutateSelectedPlaylist.value) {
    isEditingName.value = false
    return
  }
  const name = editingName.value.trim()
  if (!name || name === selectedPlaylist.value.name) {
    isEditingName.value = false
    return
  }
  try {
    const updated = await playlistsApi.updatePlaylist({ id: selectedPlaylist.value.id, name })
    applyUpdatedPlaylist(updated)
    toast.success('Playlist name saved')
  } catch (error) {
    toast.error('Failed to update playlist name', String(error))
  } finally {
    isEditingName.value = false
  }
}

function cancelEditName() {
  isEditingName.value = false
}

function startEditDescription() {
  if (!canMutateSelectedPlaylist.value) return
  editingDescription.value = selectedPlaylist.value.description || ''
  isEditingDescription.value = true
  nextTick(() => descriptionInput.value?.focus())
}

async function saveDescription() {
  if (!canMutateSelectedPlaylist.value) {
    isEditingDescription.value = false
    return
  }
  const description = editingDescription.value.trim()
  if (description === (selectedPlaylist.value.description || '')) {
    isEditingDescription.value = false
    return
  }
  try {
    const updated = await playlistsApi.updatePlaylist({ id: selectedPlaylist.value.id, description })
    applyUpdatedPlaylist(updated)
    toast.success('Playlist description saved')
  } catch (error) {
    toast.error('Failed to update playlist description', String(error))
  } finally {
    isEditingDescription.value = false
  }
}

function cancelEditDescription() {
  isEditingDescription.value = false
}

function applyUpdatedPlaylist(updated: Playlist) {
  selectedPlaylist.value = { ...selectedPlaylist.value, ...updated }
  const index = playlists.value.findIndex(p => p.id === updated.id)
  if (index >= 0) playlists.value[index] = { ...playlists.value[index], ...updated }
}

async function deleteSelectedPlaylist() {
  if (!canDeleteSelectedPlaylist.value) return
  const playlist = selectedPlaylist.value
  // R13: unified in-app confirmation (previously native window.confirm).
  const confirmed = await confirm(`Delete playlist “${playlist.name}”?`, {
    title: 'Delete Playlist',
    variant: 'danger',
    confirmLabel: 'Delete'
  })
  if (!confirmed) return
  try {
    await playlistsApi.deletePlaylist(playlist.id)
    playlistLoadSeq += 1
    isLoading.value = false
    isLoadingMore.value = false
    cancelPendingPlaylistPlayback()
    playlists.value = playlists.value.filter(p => p.id !== playlist.id)
    selectedPlaylist.value = null
    playlistTracks.value = []
    playlistMosaicCache.delete(playlist.id)
    const remainingPresentation = { ...playlistPresentation.value }
    delete remainingPresentation[playlist.id]
    playlistPresentation.value = remainingPresentation
    toast.success('Playlist deleted')
  } catch (error) {
    toast.error('Failed to delete playlist', String(error))
  }
}

async function addTracksById() {
  if (!canMutateSelectedPlaylist.value) return
  // R13: unified in-app prompt (previously native window.prompt).
  const raw = await prompt('Track IDs to add (comma-separated)', {
    title: 'Add Tracks by ID',
    placeholder: 'e.g. 301, 302, 303',
    confirmLabel: 'Add'
  })
  if (raw === null) return
  const trackIds = [...new Set(raw.split(',').map(value => Number(value.trim())).filter(id => Number.isSafeInteger(id) && id > 0))]
  if (trackIds.length === 0) {
    toast.error('Enter at least one valid track ID')
    return
  }
  try {
    await playlistsApi.addTracksToPlaylist(selectedPlaylist.value.id, trackIds)
    await selectPlaylist(selectedPlaylist.value)
    await loadPlaylists()
    toast.success(`Added ${trackIds.length} track${trackIds.length === 1 ? '' : 's'}`)
  } catch (error) {
    toast.error('Failed to add tracks', String(error))
  }
}

async function removeTrack(track: any) {
  if (!canMutateSelectedPlaylist.value) return
  try {
    // PL02 (#155): el backend quita UNA ocurrencia de esa pista, no todas.
    // Recargar en vez de filtrar por id mantiene las demás repeticiones y el
    // contador tal como quedaron en la base.
    const removed = await playlistsApi.removeTracksFromPlaylist(selectedPlaylist.value.id, [track.id])
    await selectPlaylist(selectedPlaylist.value)
    if (!removed) {
      toast.error('Track was no longer in the playlist')
      return
    }
    toast.success('Track removed from playlist')
  } catch (error) {
    toast.error('Failed to remove track', String(error))
  }
}

async function createPlaylist() {
  if (!newPlaylist.value.name.trim()) {
    toast.error('Playlist name is required')
    return
  }

  try {
    const playlist = await playlistsApi.createPlaylist({
      name: newPlaylist.value.name.trim(),
      description: newPlaylist.value.description.trim() || undefined,
      is_public: newPlaylist.value.public
    })
    
    playlists.value.unshift(playlist)
    showCreateModal.value = false
    newPlaylist.value = { name: '', description: '', public: false }
    selectPlaylist(playlist)
  } catch (error) {
    toast.error('Failed to create playlist', String(error))
  }
}

async function createSmartPlaylist() {
  const name = smartPlaylist.value.name.trim() || 'Smart Playlist'
  const rules = smartPlaylist.value.rules || []
  const rulesJson = JSON.stringify(rules)

  try {
    const created = await playlistsApi.createSmartPlaylist({
      name,
      rulesJson,
    })
    toast.success(`Smart playlist "${created.name}" created`)
    showSmartModal.value = false
    smartPlaylist.value = { name: '', rules: [{ field: 'genre', operator: 'contains', value: '' }], autoUpdate: true }
    await loadPlaylists()
    const found = smartPlaylists.value.find(p => p.id === created.id)
    if (found) {
      selectPlaylist(found)
    }
  } catch (err) {
    toast.error('Failed to create smart playlist', String(err))
  }
}

function addRule() {
  smartPlaylist.value.rules.push({ field: 'genre', operator: 'contains', value: '' })
}

function removeRule(index: number) {
  smartPlaylist.value.rules.splice(index, 1)
}

function serviceFromPlaylistUrl(rawUrl: string): string | null {
  try {
    const trimmed = rawUrl.trim()
    const withProto = /^https?:\/\//i.test(trimmed) ? trimmed : `https://${trimmed}`
    const host = new URL(withProto).hostname.toLowerCase().replace(/^www\./, '')
    if (host === 'open.spotify.com' || host.endsWith('.spotify.com')) return 'spotify'
    if (host === 'play.qobuz.com' || host.endsWith('.qobuz.com')) return 'qobuz'
    if (host === 'listen.tidal.com' || host.endsWith('.tidal.com')) return 'tidal'
    if (host === 'deezer.com' || host.endsWith('.deezer.com')) return 'deezer'
  } catch {
    return null
  }
  return null
}

async function importPlaylist() {
  const service = serviceFromPlaylistUrl(importUrl.value)
  if (!service) {
    toast.error('Unsupported playlist URL', 'Use a Spotify, Qobuz, Tidal, or Deezer URL.')
    return
  }

  isImporting.value = true
  try {
    const result = await importPlaylists(service)
    if (result.errors.length > 0 && result.imported === 0) {
      throw new Error(result.errors.join('; '))
    }
    toast.success(`Imported ${result.imported} playlist${result.imported === 1 ? '' : 's'} from ${service}`)
    showImportModal.value = false
    importUrl.value = ''
    await loadPlaylists()
  } catch (error) {
    toast.error('Failed to import playlists', String(error))
  } finally {
    isImporting.value = false
  }
}

async function playTrackNow(track: any) {
  try {
    await player.play({
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: track.album,
      coverUrl: track.albumArt || null,
    })
  } catch (err) {
    toast.error('Could not play track', String(err))
  }
}

async function playTrack(track: any) {
  if (isPlaylistPlaybackBusy.value) return
  await playTrackNow(track)
}

function toPlayerTrack(track: any) {
  return {
    id: track.id,
    title: track.title,
    artist: track.artist,
    album: track.album,
    coverUrl: track.albumArt || null,
  }
}

async function selectedPlaylistRows(): Promise<any[]> {
  if (!hasMoreTracks.value) return playlistTracks.value
  const playlistId = selectedPlaylist.value?.id
  if (typeof playlistId !== 'number') return playlistTracks.value
  const tracks = await fetchAllPlaylistTracks(playlistId)
  return tracks.map((track, index) => mapToTrack(track, index))
}

async function runWithAllPlaylistTracks(
  action: (tracks: any[]) => Promise<void>,
  errorMessage: string,
) {
  if (isPlaylistPlaybackBusy.value || !playlistTracks.value.length) return

  const seq = ++playlistPlaybackSeq
  isPlaylistPlaybackBusy.value = true
  try {
    const tracks = await selectedPlaylistRows()
    if (seq !== playlistPlaybackSeq || !tracks.length) return
    await action(tracks)
  } catch (err) {
    if (seq === playlistPlaybackSeq) toast.error(errorMessage, String(err))
  } finally {
    if (seq === playlistPlaybackSeq) isPlaylistPlaybackBusy.value = false
  }
}

async function playAll() {
  // Phase 1 fix: the previous comment was wrong — playNext inserts at the
  // front, so chaining slice(1)+playNext reversed the playlist order.
  // replaceQueueAndPlay plays the first track and queues the rest in the
  // list's real order.
  await runWithAllPlaylistTracks(
    tracks => player.replaceQueueAndPlay(tracks.map(toPlayerTrack)),
    'Could not play playlist',
  )
}

// ==============================================
// S201: Download Playlist — Mode A (M3U) and Mode B (queue)
// ==============================================

function selectedNumericPlaylistId(): number | null {
  const id = selectedPlaylist.value?.id
  return typeof id === 'number' && id > 0 ? id : null
}

function openDownloadModal() {
  if (!selectedPlaylist.value || selectedNumericPlaylistId() === null) {
    toast.warning('Select a playlist', 'Choose a real playlist to download (Favorites does not apply).')
    return
  }
  downloadResult.value = null
  showMissingDetails.value = false
  showDownloadModal.value = true
}

function closeDownloadModal() {
  if (isDownloadBusy.value) return
  showDownloadModal.value = false
}

function sanitizeFileBaseName(name: string): string {
  return name.replace(/[\\/:*?"<>|]/g, '_').trim() || 'playlist'
}

/**
 * Mode A "Only the ones I already have": save-dialog + export_playlist_m3u.
 * The backend checks each file with a real stat() and writes the .m3u only
 * with verified tracks; the {total, verified, missing} counts are real.
 */
async function downloadExistingAsM3u() {
  const playlistId = selectedNumericPlaylistId()
  if (playlistId === null) return
  isExportingM3u.value = true
  try {
    const path = await saveDialog({
      title: 'Export Playlist (.m3u)',
      defaultPath: `${sanitizeFileBaseName(selectedPlaylist.value?.name ?? 'playlist')}.m3u`,
      filters: [{ name: 'Playlist M3U', extensions: ['m3u'] }],
    })
    if (!path) return // cancelled by the user
    const res = await exportPlaylistM3u(playlistId, path)
    downloadResult.value = {
      mode: 'm3u',
      total: res.total_tracks,
      verified: res.verified_count,
      missing: res.missing_tracks,
      filePath: res.file_path,
    }
    toast.success('M3U exported', `${res.verified_count}/${res.total_tracks} tracks verified`)
    showDownloadModal.value = false
  } catch (error: any) {
    toast.error('Could not export M3U', String(error?.message || error || ''))
  } finally {
    isExportingM3u.value = false
  }
}

/** Fetches every page in playlist order, preserving repeated occurrences. */
async function fetchAllPlaylistTracks(playlistId: number): Promise<LibraryTrack[]> {
  const all: LibraryTrack[] = []
  let offset = 0
  while (true) {
    const page = await fetchPlaylistPage(playlistId, offset, PLAYLIST_PAGE_SIZE)
    if (page.tracks.length === 0) {
      if (page.has_more) throw new Error('Playlist returned an empty page before the end')
      break
    }
    all.push(...page.tracks)
    if (!page.has_more) break
    offset += page.tracks.length
  }
  return all
}

/**
 * Mode B "Download missing tracks": locally filters tracks without a current
 * download and queues them via add_batch_to_queue (enqueue_eligible_batch),
 * with the same priority/quality as manual queueing from Library.
 * The displayed counters come straight from the engine.
 */
async function downloadMissingTracks() {
  const playlistId = selectedNumericPlaylistId()
  if (playlistId === null) return
  isQueueingMissing.value = true
  try {
    const tracks = await fetchAllPlaylistTracks(playlistId)
    const missingIds = tracks
      .filter(t => t.download_status !== 'downloaded')
      .map(t => t.id)
      .filter(id => Number.isFinite(id))

    if (missingIds.length === 0) {
      toast.info('Nothing to download', 'You already have all tracks in this playlist.')
      showDownloadModal.value = false
      return
    }

    // Same policy as manual queueing in Library: priority 50, hires, fallback allowed.
    const res = await addBatchToQueue({
      trackIds: missingIds,
      priority: 50,
      qualityPreference: 'hires',
      allowFallback: true,
      jobKind: 'playlist',
    })

    const enqueued = res.enqueued ?? res.added
    downloadResult.value = {
      mode: 'queue',
      enqueued,
      deduplicated: res.deduplicated,
      skipped: res.skipped ?? 0,
    }
    toast.success('Tracks queued', `${enqueued} of ${missingIds.length} in the download queue.`)
    showDownloadModal.value = false
  } catch (error: any) {
    const errStr = String(error?.message || error || '')
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', 'Some tracks have no download provider available.')
    } else {
      toast.error(`Could not queue playlist: ${errStr}`)
    }
  } finally {
    isQueueingMissing.value = false
  }
}

function missingReasonLabel(reason: string): string {
  if (reason === 'sin_archivo_local') return 'No local file'
  if (reason === 'archivo_no_encontrado') return 'File not found'
  return reason
}

async function downloadTrack(track: any) {
  try {
    // Item 63: a track whose available source is Tidal (and not Qobuz) locks
    // its source identity to Tidal. The backend only sets the exact Qobuz
    // source by default (tracks.qobuz_id in perform_add_to_queue), so without
    // this lock a track with a Tidal source might never download via Tidal.
    // With the lock the worker processes it through the Tidal pipeline
    // (orchestrator → TidalDownloader → execute_tidal_single_track_download)
    // with the same queue, states and progress as a Qobuz download.
    const availableServices = String(track.available_services || track.services || '')
      .split(',')
      .map(s => s.trim().toLowerCase())
      .filter(Boolean)
    const tidalOnly = availableServices.includes('tidal') && !availableServices.includes('qobuz')

    await addToQueue({
      trackId: track.id,
      targetTitle: track.title,
      targetArtist: track.artist,
      targetAlbum: track.album,
      ...(tidalOnly ? { serviceName: 'tidal' } : {}),
      allowFallback: true,
    })
    toast.success('Queued for download', track.title)
  } catch (error: any) {
    const errStr = String(error?.message || error || '')
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', `Track "${track.title}" has no available provider source.`)
    } else {
      toast.error(`Failed to enqueue: ${errStr}`)
    }
  }
}

async function shufflePlay() {
  await runWithAllPlaylistTracks(async (tracks) => {
    const randomIndex = Math.floor(Math.random() * tracks.length)
    await playTrackNow(tracks[randomIndex])
  }, 'Could not shuffle playlist')
}

async function playPlaylist(playlist: any) {
  if (selectedPlaylist.value?.id !== playlist.id) {
    await selectPlaylist(playlist)
  }
  await playAll()
}

// Sidebar options menu: opening it never deletes anything on its own.
const playlistMenu = ref<{ playlist: any; x: number; y: number } | null>(null)

function openPlaylistMenu(event: MouseEvent, playlist: any) {
  event.preventDefault()
  event.stopPropagation()

  // Anchored to the element that opened the menu, not the event: the same menu
  // serves the three-dot button (left) and right-click.
  const target = event.currentTarget as HTMLElement | null
  const anchor = target?.getBoundingClientRect()
  const width = 200
  const height = 208
  const x = anchor ? anchor.right - width : event.clientX
  const y = anchor ? anchor.bottom + 4 : event.clientY

  playlistMenu.value = {
    playlist,
    x: Math.max(8, Math.min(x, window.innerWidth - width - 8)),
    y: Math.max(8, Math.min(y, window.innerHeight - height - 8)),
  }
}

function closePlaylistMenu() {
  playlistMenu.value = null
}

/** The menu acts on its own entry, which may not be the selected one. */
const canMutateMenuPlaylist = computed(() => {
  const id = playlistMenu.value?.playlist?.id
  return typeof id === 'number' && id > 0 && !playlistMenu.value?.playlist?.smart
})

const canDeleteMenuPlaylist = computed(() => {
  const id = playlistMenu.value?.playlist?.id
  return typeof id === 'number' && id > 0
})

async function menuSelectPlaylist() {
  const playlist = playlistMenu.value?.playlist
  closePlaylistMenu()
  if (playlist) await selectPlaylist(playlist)
}

async function menuPlayPlaylist() {
  const playlist = playlistMenu.value?.playlist
  closePlaylistMenu()
  if (playlist) await playPlaylist(playlist)
}

async function menuStartEditName() {
  const playlist = playlistMenu.value?.playlist
  closePlaylistMenu()
  if (!playlist) return
  await selectPlaylist(playlist)
  startEditName()
}

async function menuDeletePlaylist() {
  const playlist = playlistMenu.value?.playlist
  closePlaylistMenu()
  if (!playlist) return
  await selectPlaylist(playlist)
  await deleteSelectedPlaylist()
}

function getServiceColor(service: string) {
  const colors: Record<string, string> = {
    spotify: 'bg-green-500',
    qobuz: 'bg-blue-600',
    tidal: 'bg-black',
    deezer: 'bg-purple-500',
    apple_music: 'bg-red-500',
    soundcloud: 'bg-orange-500',
  }
  return colors[service.toLowerCase()] || 'bg-gray-500'
}

// Drag and drop
function onDragStart(event: DragEvent, playlist: any) {
  event.dataTransfer?.setData('playlist', JSON.stringify(playlist))
}

function onTrackDragStart(event: DragEvent, track: any, index: number) {
  event.dataTransfer?.setData('track', JSON.stringify({ track, index }))
}

async function onTrackDrop(event: DragEvent, newIndex: number) {
  const data = event.dataTransfer?.getData('track')
  if (!data || !canMutateSelectedPlaylist.value) return
  const { track, index: oldIndex } = JSON.parse(data)
  const previous = [...playlistTracks.value]
  const tracks = [...previous]
  tracks.splice(oldIndex, 1)
  tracks.splice(newIndex, 0, track)
  playlistTracks.value = tracks
  try {
    // PL02 (#155): el payload es el orden completo y cada entrada mueve UNA
    // ocurrencia; con repeticiones, el trackId se repite y el backend consume
    // las ocurrencias en el orden del payload.
    await playlistsApi.reorderPlaylistTracks(
      selectedPlaylist.value.id,
      tracks.map((item, index) => ({ trackId: item.id, newPosition: index + 1 })),
    )
    updateLoadedPlaylistMosaic()
  } catch (error) {
    playlistTracks.value = previous
    toast.error('Failed to reorder playlist tracks', String(error))
  }
}
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.accordion-enter-active,
.accordion-leave-active {
  transition: all 0.2s ease;
  overflow: hidden;
}

.accordion-enter-from,
.accordion-leave-to {
  opacity: 0;
  max-height: 0;
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

.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background-color: rgba(155, 155, 155, 0.5);
}
</style>
