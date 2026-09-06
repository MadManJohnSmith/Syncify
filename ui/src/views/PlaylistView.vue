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
            class="px-3 py-1.5 bg-primary hover:bg-primary-hover text-white text-sm font-medium rounded-lg flex items-center gap-1"
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
          <button @click="triggerSyncPlaylists" :disabled="isSyncing" class="flex-1 py-1.5 text-xs text-primary dark:text-primary-light bg-primary/10 hover:bg-primary/20 rounded-lg flex items-center justify-center gap-1 disabled:opacity-50 font-medium">
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
          :class="selectedPlaylist?.id === 'favorites' ? 'bg-primary/10 border border-primary/20' : 'hover:bg-gray-100 dark:hover:bg-surface-highlight'"
          @click="selectPlaylist(favoritesPlaylist)"
        >
          <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-red-500 to-pink-500 flex items-center justify-center">
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
                @contextmenu.prevent="showPlaylistMenu($event, playlist)"
                draggable="true"
                @dragstart="onDragStart($event, playlist)"
              >
                <div class="w-10 h-10 rounded-lg bg-gray-200 dark:bg-gray-700 overflow-hidden shrink-0">
                  <img v-if="playlist.image_url" :src="playlist.image_url" class="w-full h-full object-cover">
                  <div v-else class="w-full h-full flex items-center justify-center">
                    <span class="material-symbols-outlined text-gray-400">queue_music</span>
                  </div>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ playlist.name }}</p>
                  <p class="text-xs text-gray-500">{{ playlist.track_count }} tracks</p>
                </div>
                <div class="opacity-60 group-hover:opacity-100 transition-opacity flex items-center gap-1">
                  <button @click.stop="playPlaylist(playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">play_arrow</span>
                  </button>
                  <button @click.stop="showPlaylistMenu($event, playlist)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
                    <span class="material-symbols-outlined text-sm">more_vert</span>
                  </button>
                </div>
              </div>
            </div>
          </Transition>
        </div>
        
        <!-- Imported Playlists (per service) -->
        <div v-for="service in importedServices" :key="service.id" class="mt-2">
          <button 
            @click="expandedCategories[service.id] = !expandedCategories[service.id]"
            class="w-full px-4 py-2 flex items-center justify-between text-xs font-semibold text-gray-500 uppercase tracking-wide hover:bg-gray-100 dark:hover:bg-surface-highlight"
          >
            <span class="flex items-center gap-2">
              <span :class="['w-3 h-3 rounded-full', service.color]"></span>
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
              >
                <div class="w-10 h-10 rounded-lg bg-gray-200 dark:bg-gray-700 overflow-hidden shrink-0">
                  <img v-if="playlist.coverArt" :src="playlist.coverArt" class="w-full h-full object-cover">
                </div>
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
                v-for="playlist in smartPlaylists" 
                :key="playlist.id"
                class="playlist-item p-2 rounded-lg cursor-pointer transition-colors flex items-center gap-3"
                :class="selectedPlaylist?.id === playlist.id ? 'bg-primary/10 border border-primary/20' : 'hover:bg-gray-100 dark:hover:bg-surface-highlight'"
                @click="selectPlaylist(playlist)"
              >
                <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-amber-400 to-orange-500 flex items-center justify-center">
                  <span class="material-symbols-outlined text-white text-lg">{{ playlist.icon || 'auto_awesome' }}</span>
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ playlist.name }}</p>
                  <p class="text-xs text-gray-500">{{ playlist.trackCount }} tracks · Auto-updates</p>
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
          <button @click="showCreateModal = true" class="px-4 py-2 bg-primary text-white rounded-lg">
            Create Playlist
          </button>
        </div>
      </div>
      
      <!-- Playlist Header -->
      <div v-else class="playlist-header p-6 border-b border-gray-200 dark:border-border-dark">
        <div class="flex gap-6">
          <!-- Cover Art -->
          <div class="relative group shrink-0">
            <div class="w-32 h-32 rounded-xl overflow-hidden bg-gray-200 dark:bg-gray-700 shadow-lg">
              <img v-if="selectedPlaylist.coverArt" :src="selectedPlaylist.coverArt" class="w-full h-full object-cover">
              <div v-else class="w-full h-full flex items-center justify-center">
                <span class="material-symbols-outlined text-5xl text-gray-400">queue_music</span>
              </div>
            </div>
            <button 
              v-if="selectedPlaylist.id !== 'favorites' && !selectedPlaylist.smart"
              @click="changeCoverArt"
              class="absolute inset-0 bg-black/50 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center rounded-xl"
            >
              <span class="material-symbols-outlined text-white">photo_camera</span>
            </button>
          </div>
          
          <!-- Playlist Info -->
          <div class="flex-1">
            <div class="flex items-start justify-between">
              <div>
                <p class="text-xs text-gray-500 uppercase tracking-wide mb-1">
                  {{ selectedPlaylist.smart ? 'Smart Playlist' : selectedPlaylist.service ? `Imported from ${selectedPlaylist.service}` : 'Playlist' }}
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
                  v-if="selectedPlaylist.description || isEditingDescription"
                  @click="startEditDescription"
                  class="text-sm text-gray-500 mb-2"
                >
                  {{ selectedPlaylist.description || 'Add description...' }}
                </p>
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
              <button @click="playAll" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white font-medium rounded-full flex items-center gap-2">
                <span class="material-symbols-outlined">play_arrow</span>
                Play All
              </button>
              <button @click="openDownloadModal" class="px-5 py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-full flex items-center gap-2">
                <span class="material-symbols-outlined">download</span>
                Descargar playlist
              </button>
              <button @click="shufflePlay" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-full">
                <span class="material-symbols-outlined">shuffle</span>
              </button>
              <button @click="showPlaylistActionsMenu" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-full">
                <span class="material-symbols-outlined">more_horiz</span>
              </button>
            </div>

            <!-- S201: banner de resultado de «Descargar playlist» -->
            <div v-if="downloadResult" class="mt-3 p-3 rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight text-sm">
              <!-- Modo A: export M3U -->
              <template v-if="downloadResult.mode === 'm3u'">
                <div class="flex items-center justify-between gap-2">
                  <span class="text-gray-700 dark:text-gray-300">
                    M3U exportado · {{ downloadResult.verified }}/{{ downloadResult.total }} pistas verificadas<template v-if="downloadResult.filePath"> → {{ downloadResult.filePath }}</template>
                  </span>
                  <span class="flex items-center gap-1 shrink-0">
                    <button
                      v-if="downloadResult.missing.length"
                      @click="showMissingDetails = !showMissingDetails"
                      class="px-2 py-1 text-xs rounded-lg bg-primary/10 text-primary hover:bg-primary/20 flex items-center gap-1"
                    >
                      <span class="material-symbols-outlined text-sm">{{ showMissingDetails ? 'expand_less' : 'expand_more' }}</span>
                      Faltantes ({{ downloadResult.missing.length }})
                    </button>
                    <button @click="downloadResult = null" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded" aria-label="Cerrar">
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
              <!-- Modo B: resumen del motor de cola -->
              <template v-else>
                <div class="flex items-center justify-between gap-2">
                  <span class="text-gray-700 dark:text-gray-300">
                    Cola de descargas: {{ downloadResult.enqueued }} encoladas<template v-if="downloadResult.deduplicated !== undefined"> · {{ downloadResult.deduplicated }} ya descargadas o en cola</template><template v-if="downloadResult.skipped > 0"> · {{ downloadResult.skipped }} omitidas</template>.
                  </span>
                  <button @click="downloadResult = null" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded shrink-0" aria-label="Cerrar">
                    <span class="material-symbols-outlined text-base text-gray-500">close</span>
                  </button>
                </div>
              </template>
            </div>
          </div>
        </div>
      </div>
      
      <!-- Track List -->
      <div v-if="selectedPlaylist" class="flex-1 overflow-y-auto custom-scrollbar">
        <!-- Empty Playlist -->
        <div v-if="!playlistTracks.length" class="flex-1 flex items-center justify-center py-16">
          <div class="text-center">
            <div class="w-16 h-16 mx-auto rounded-full bg-gray-100 dark:bg-surface-highlight flex items-center justify-center mb-4">
              <span class="material-symbols-outlined text-3xl text-gray-400">music_note</span>
            </div>
            <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">This playlist is empty</h3>
            <p class="text-gray-500 text-sm mb-4">Drag tracks here or click Add Tracks</p>
            <button class="px-4 py-2 bg-primary text-white rounded-lg text-sm">
              Add Tracks
            </button>
          </div>
        </div>
        
        <!-- Track Table Header -->
        <div v-else class="sticky top-0 bg-white dark:bg-surface-light border-b border-gray-200 dark:border-border-dark px-4 py-2 flex items-center text-xs font-semibold text-gray-500 uppercase">
          <div class="w-10">#</div>
          <div class="w-12"></div>
          <div class="flex-1">Title</div>
          <div class="w-40">Album</div>
          <div class="w-32">Added</div>
          <div class="w-20 text-right">Duration</div>
          <div class="w-10"></div>
        </div>
        
        <!-- Track Rows -->
        <div 
          v-for="(track, index) in playlistTracks" 
          :key="track.id"
          class="track-row px-4 py-2 flex items-center hover:bg-gray-50 dark:hover:bg-surface-highlight group cursor-pointer"
          :class="{ 'bg-primary/5': selectedTracks.includes(track.id) }"
          draggable="true"
          @dragstart="onTrackDragStart($event, track, index)"
          @dragover.prevent
          @drop="onTrackDrop($event, index)"
        >
          <!-- Index / Play -->
          <div class="w-10 text-sm text-gray-500 group-hover:hidden">{{ index + 1 }}</div>
          <div class="w-10 hidden group-hover:block">
            <button @click.stop="playTrack(track)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
              <span class="material-symbols-outlined text-primary">play_arrow</span>
            </button>
          </div>
          
          <!-- Album Art -->
          <div class="w-12 pr-3">
            <div class="w-10 h-10 rounded bg-gray-200 dark:bg-gray-700 overflow-hidden">
              <img v-if="track.albumArt" :src="track.albumArt" class="w-full h-full object-cover">
            </div>
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
            <button @click.stop="showTrackMenu($event, track)" class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
              <span class="material-symbols-outlined text-gray-400 text-lg">more_vert</span>
            </button>
          </div>
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
              <button @click="createPlaylist" class="flex-1 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium">
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
              <button @click="createSmartPlaylist" class="flex-1 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium">
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
              <p class="text-xs text-gray-500">Supports Spotify, Qobuz, Tidal, and Deezer playlist URLs</p>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="autoSyncImport" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-600 dark:text-gray-400">Auto-sync this playlist</span>
              </label>
            </div>
            <div class="px-6 pb-6 flex gap-3">
              <button @click="showImportModal = false" class="flex-1 py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg">
                Cancel
              </button>
              <button @click="importPlaylist" class="flex-1 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg font-medium">
                Import
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- S201: Download Playlist Modal — dos modos -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showDownloadModal" class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8" @click.self="closeDownloadModal">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Descargar playlist</h3>
              <p class="text-xs text-gray-500 truncate">{{ selectedPlaylist?.name }}</p>
            </div>
            <div class="p-6 space-y-3">
              <!-- Modo A -->
              <button
                :disabled="isDownloadBusy"
                @click="downloadExistingAsM3u"
                class="w-full text-left p-4 rounded-xl border border-gray-200 dark:border-border-dark hover:border-primary hover:bg-primary/5 transition-colors disabled:opacity-50 disabled:pointer-events-none flex gap-3"
              >
                <span class="material-symbols-outlined text-primary mt-0.5">save</span>
                <span>
                  <span class="block text-sm font-medium text-gray-900 dark:text-white">Solo las que ya tengo</span>
                  <span class="block text-xs text-gray-500 mt-1">Exporta un archivo .m3u con únicamente las pistas cuyo archivo local existe en tu disco. No descarga nada nuevo.</span>
                </span>
              </button>
              <!-- Modo B -->
              <button
                :disabled="isDownloadBusy"
                @click="downloadMissingTracks"
                class="w-full text-left p-4 rounded-xl border border-gray-200 dark:border-border-dark hover:border-primary hover:bg-primary/5 transition-colors disabled:opacity-50 disabled:pointer-events-none flex gap-3"
              >
                <span class="material-symbols-outlined text-primary mt-0.5">cloud_download</span>
                <span>
                  <span class="block text-sm font-medium text-gray-900 dark:text-white">Descargar las pistas faltantes</span>
                  <span class="block text-xs text-gray-500 mt-1">Encola en la cola de descargas normal solo las pistas que aún no tienes. Requiere conexión.</span>
                </span>
              </button>
              <p v-if="isDownloadBusy" class="text-xs text-gray-500 flex items-center gap-2">
                <span class="material-symbols-outlined text-sm animate-spin">progress_activity</span>
                Procesando…
              </p>
            </div>
            <div class="px-6 pb-6">
              <button @click="closeDownloadModal" class="w-full py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg" :disabled="isDownloadBusy">
                Cerrar
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted } from 'vue'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { libraryApi } from '@/api/library'
import { playlistsApi, exportPlaylistM3u, type MissingPlaylistFile } from '@/api/playlists'
import { addToQueue, addBatchToQueue } from '@/api/queue'
import type { Playlist, LibraryTrack } from '@/api/types'
import { useToast } from '@/composables/useToast'
import { usePlayer } from '@/composables/usePlayer'

const toast = useToast()
const player = usePlayer()

// State
const searchQuery = ref('')
const selectedPlaylist = ref<any>(null)
const selectedTracks = ref<string[]>([])
const showCreateModal = ref(false)
const showSmartModal = ref(false)
const showImportModal = ref(false)

// S201: «Descargar playlist» — dos modos
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
const nameInput = ref<HTMLInputElement | null>(null)

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
const autoSyncImport = ref(true)

const favoritesPlaylist = ref<any>({
  id: -1, // Changed from 'favorites' string to number
  name: 'Favorites',
  track_count: 0,
  duration: '0h 0m',
  size: '0 GB',
})

const myPlaylists = ref<Playlist[]>([])
const importedServices = ref<any[]>([])
const smartPlaylists = ref<any[]>([])
const playlistTracks = ref<any[]>([])
const isLoading = ref(false)

// Map LibraryTrack to UI Track
function mapToTrack(item: LibraryTrack, _index: number) {
  const durationSec = (item.duration_ms ?? 0) / 1000;
  const mins = Math.floor(durationSec / 60);
  const secs = Math.floor(durationSec % 60);
  
  return {
    id: item.id,
    title: item.title,
    artist: item.artist_name || 'Unknown Artist',
    album: item.album_name || 'Unknown Album',
    duration: `${mins}:${secs.toString().padStart(2, '0')}`,
    addedDate: 'Recently',
    albumArt: item.cover_art_url || '',
  };
}

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

watch(
  () => showSmartModal.value,
  (isOpen) => {
    if (isOpen) {
      updateSmartPreview()
    }
  }
)

// Methods
async function selectPlaylist(playlist: any) {
  selectedPlaylist.value = playlist
  selectedTracks.value = []
  
  if (playlist.id === 'favorites') {
    // Handle Favorites specially if needed, but for core wiring:
    playlistTracks.value = []
    return
  }
  
  try {
    isLoading.value = true
    const page = await libraryApi.getPlaylistTracks(playlist.id)
    playlistTracks.value = page.tracks.map((t, i) => mapToTrack(t, i))
  } catch (error) {
    toast.error('Failed to load playlist tracks', String(error))
  } finally {
    isLoading.value = false
  }
}

async function loadPlaylists() {
  try {
    const playlists = await libraryApi.getPlaylists()
    
    // Separate smart playlists vs regular playlists
    smartPlaylists.value = playlists
      .filter(p => Boolean(p.is_smart))
      .map(p => {
        let parsedRules = []
        if (p.rules_json) {
          try {
            parsedRules = JSON.parse(p.rules_json)
          } catch {
            parsedRules = []
          }
        }
        return {
          id: p.id,
          name: p.name,
          trackCount: p.track_count,
          icon: 'auto_awesome',
          smart: true,
          rules: parsedRules,
          rules_json: p.rules_json,
        }
      })

    myPlaylists.value = playlists.filter(p => !p.is_smart)
    
    // Reset categories
    importedServices.value = []
  } catch (error) {
    console.error('Failed to load playlists:', error)
  }
}

const isSyncing = ref(false)

async function triggerSyncPlaylists() {
  isSyncing.value = true
  try {
    const res = await playlistsApi.syncPlaylists()
    toast.success(res.message)
    await loadPlaylists()
  } catch (e: any) {
    toast.error(`Sync failed: ${e}`)
  } finally {
    isSyncing.value = false
  }
}

onMounted(() => {
  loadPlaylists()
})

function startEditName() {
  if (selectedPlaylist.value?.id === 'favorites' || selectedPlaylist.value?.smart) return
  editingName.value = selectedPlaylist.value.name
  isEditingName.value = true
  nextTick(() => nameInput.value?.focus())
}

function saveName() {
  if (editingName.value.trim()) {
    selectedPlaylist.value.name = editingName.value.trim()
  }
  isEditingName.value = false
}

function cancelEditName() {
  isEditingName.value = false
}

function startEditDescription() {
  if (selectedPlaylist.value?.id === -1 || (selectedPlaylist.value as any)?.smart) return
  isEditingDescription.value = true
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
    
    myPlaylists.value.unshift(playlist)
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

function importPlaylist() {
  console.log('Importing:', importUrl.value)
  showImportModal.value = false
  importUrl.value = ''
}

async function playTrack(track: any) {
  try {
    await player.play({
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: track.album,
      coverUrl: track.albumArt || null,
    })
  } catch (err) {
    toast.error('No se pudo reproducir la pista', String(err))
  }
}

async function playAll() {
  if (playlistTracks.value.length > 0) {
    await playTrack(playlistTracks.value[0])
  }
}

// ==============================================
// S201: Descargar playlist — Modo A (M3U) y Modo B (cola)
// ==============================================

function selectedNumericPlaylistId(): number | null {
  const id = selectedPlaylist.value?.id
  return typeof id === 'number' && id > 0 ? id : null
}

function openDownloadModal() {
  if (!selectedPlaylist.value || selectedNumericPlaylistId() === null) {
    toast.warning('Selecciona una playlist', 'Elige una playlist real para descargar (Favoritos no aplica).')
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
 * Modo A «Solo las que ya tengo»: save-dialog + export_playlist_m3u.
 * El backend verifica con stat() real cada archivo y escribe el .m3u solo
 * con las pistas verificadas; los conteos {total, verified, missing} son reales.
 */
async function downloadExistingAsM3u() {
  const playlistId = selectedNumericPlaylistId()
  if (playlistId === null) return
  isExportingM3u.value = true
  try {
    const path = await saveDialog({
      title: 'Exportar playlist (.m3u)',
      defaultPath: `${sanitizeFileBaseName(selectedPlaylist.value?.name ?? 'playlist')}.m3u`,
      filters: [{ name: 'Playlist M3U', extensions: ['m3u'] }],
    })
    if (!path) return // cancelado por el usuario
    const res = await exportPlaylistM3u(playlistId, path)
    downloadResult.value = {
      mode: 'm3u',
      total: res.total_tracks,
      verified: res.verified_count,
      missing: res.missing_tracks,
      filePath: res.file_path,
    }
    toast.success('M3U exportado', `${res.verified_count}/${res.total_tracks} pistas verificadas`)
    showDownloadModal.value = false
  } catch (error: any) {
    toast.error('No se pudo exportar el M3U', String(error?.message || error || ''))
  } finally {
    isExportingM3u.value = false
  }
}

/** Trae TODAS las páginas de la playlist (offset/limit/has_more). */
async function fetchAllPlaylistTracks(playlistId: number): Promise<LibraryTrack[]> {
  const PAGE_SIZE = 500
  const all: LibraryTrack[] = []
  let offset = 0
  for (let guard = 0; guard < 1000; guard++) {
    const page = await libraryApi.getPlaylistTracks(playlistId, offset, PAGE_SIZE)
    all.push(...page.tracks)
    if (!page.has_more || page.tracks.length === 0) break
    offset += page.tracks.length
  }
  return all
}

/**
 * Modo B «Descargar las pistas faltantes»: filtra localmente las pistas sin
 * descarga vigente y las encola vía add_batch_to_queue (enqueue_eligible_batch),
 * con prioridad/calidad idénticas al encolado manual desde Library.
 * Los contadores mostrados salen tal cual del motor.
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
      toast.info('Nada que descargar', 'Ya tienes todas las pistas de esta playlist.')
      showDownloadModal.value = false
      return
    }

    // Misma política que el encolado manual en Library: prioridad 50, hires, fallback permitido.
    const res = await addBatchToQueue({
      trackIds: missingIds,
      priority: 50,
      qualityPreference: 'hires',
      allowFallback: true,
    })

    const enqueued = res.enqueued ?? res.added
    downloadResult.value = {
      mode: 'queue',
      enqueued,
      deduplicated: res.deduplicated,
      skipped: res.skipped ?? 0,
    }
    toast.success('Pistas encoladas', `${enqueued} de ${missingIds.length} en la cola de descargas.`)
    showDownloadModal.value = false
  } catch (error: any) {
    const errStr = String(error?.message || error || '')
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', 'Algunas pistas no tienen proveedor de descarga disponible.')
    } else {
      toast.error(`No se pudo encolar la playlist: ${errStr}`)
    }
  } finally {
    isQueueingMissing.value = false
  }
}

function missingReasonLabel(reason: string): string {
  if (reason === 'sin_archivo_local') return 'Sin archivo local'
  if (reason === 'archivo_no_encontrado') return 'Archivo no encontrado'
  return reason
}

async function downloadTrack(track: any) {
  try {
    await addToQueue({
      trackId: track.id,
      targetTitle: track.title,
      targetArtist: track.artist,
      targetAlbum: track.album,
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
  if (playlistTracks.value.length > 0) {
    const randomIndex = Math.floor(Math.random() * playlistTracks.value.length)
    await playTrack(playlistTracks.value[randomIndex])
  }
}

async function playPlaylist(playlist: any) {
  if (selectedPlaylist.value?.id !== playlist.id) {
    await selectPlaylist(playlist)
  }
  if (playlistTracks.value.length > 0) {
    await playTrack(playlistTracks.value[0])
  }
}

function changeCoverArt() {
  console.log('Change cover art')
}

function showPlaylistMenu(_event: MouseEvent, playlist: any) {
  console.log('Show menu for:', playlist.name)
}

function showPlaylistActionsMenu() {
  console.log('Show actions menu')
}

function showTrackMenu(_event: MouseEvent, track: any) {
  console.log('Show track menu:', track.title)
}

function getServiceColor(service: string) {
  const colors: Record<string, string> = {
    Spotify: 'bg-green-500',
    Qobuz: 'bg-blue-600',
    Tidal: 'bg-black',
    Deezer: 'bg-purple-500',
  }
  return colors[service] || 'bg-gray-500'
}

// Drag and drop
function onDragStart(event: DragEvent, playlist: any) {
  event.dataTransfer?.setData('playlist', JSON.stringify(playlist))
}

function onTrackDragStart(event: DragEvent, track: any, index: number) {
  event.dataTransfer?.setData('track', JSON.stringify({ track, index }))
}

function onTrackDrop(event: DragEvent, newIndex: number) {
  const data = event.dataTransfer?.getData('track')
  if (data) {
    const { track, index: oldIndex } = JSON.parse(data)
    const tracks = [...playlistTracks.value]
    tracks.splice(oldIndex, 1)
    tracks.splice(newIndex, 0, track)
    playlistTracks.value = tracks
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
