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
                <div class="opacity-0 group-hover:opacity-100 flex items-center gap-1">
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
              <button @click="downloadAll" class="px-5 py-2 border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-full flex items-center gap-2">
                <span class="material-symbols-outlined">download</span>
                Download
              </button>
              <button @click="shufflePlay" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-full">
                <span class="material-symbols-outlined">shuffle</span>
              </button>
              <button @click="showPlaylistActionsMenu" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-full">
                <span class="material-symbols-outlined">more_horiz</span>
              </button>
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
            <button class="p-1 hover:bg-gray-200 dark:hover:bg-gray-600 rounded">
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
          
          <!-- Actions -->
          <div class="w-10 opacity-0 group-hover:opacity-100 flex justify-end">
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
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted } from 'vue'
import { libraryApi } from '@/api/library'
import { playlistsApi } from '@/api/playlists'
import { addToQueue, addBatchToQueue } from '@/api/queue'
import type { Playlist, LibraryTrack } from '@/api/types'
import { useToast } from '@/composables/useToast'

const toast = useToast()

// State
const searchQuery = ref('')
const selectedPlaylist = ref<any>(null)
const selectedTracks = ref<string[]>([])
const showCreateModal = ref(false)
const showSmartModal = ref(false)
const showImportModal = ref(false)
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

const smartPreviewCount = computed(() => {
  // Mock: would calculate based on rules
  return 42
})

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
    // Group playlists: Local vs Service
    myPlaylists.value = playlists.filter(p => !p.service_playlist_id || p.account_id === -1) // Assuming -1 or similar for local
    
    // For now, put all in myPlaylists until we have more metadata for service grouping
    myPlaylists.value = playlists
    
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

function createSmartPlaylist() {
  const playlist = {
    id: 'smart' + Date.now(),
    name: smartPlaylist.value.name || 'Smart Playlist',
    trackCount: smartPreviewCount.value,
    icon: 'auto_awesome',
    smart: true,
    rules: [...smartPlaylist.value.rules],
  }
  smartPlaylists.value.push(playlist)
  showSmartModal.value = false
  smartPlaylist.value = { name: '', rules: [{ field: 'genre', operator: 'contains', value: '' }], autoUpdate: true }
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

function playAll() {
  console.log('Playing all tracks')
}

async function downloadAll() {
  if (!playlistTracks.value || playlistTracks.value.length === 0) {
    toast.warning('No tracks to download')
    return
  }
  try {
    const trackIds = playlistTracks.value.map(t => t.id).filter(Boolean)
    if (trackIds.length === 0) return
    const res = await addBatchToQueue({ trackIds, allowFallback: false })
    toast.success(`Queued ${res.added} tracks for download`)
  } catch (error: any) {
    const errStr = String(error?.message || error || '')
    if (errStr.includes('SourceIdentityMissing')) {
      toast.error('Source identity missing', 'One or more tracks in playlist have no available provider source.')
    } else {
      toast.error(`Failed to queue playlist: ${errStr}`)
    }
  }
}

async function downloadTrack(track: any) {
  try {
    await addToQueue({
      trackId: track.id,
      targetTitle: track.title,
      targetArtist: track.artist,
      targetAlbum: track.album,
      allowFallback: false,
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

function shufflePlay() {
  console.log('Shuffle play')
}

function playPlaylist(playlist: any) {
  console.log('Playing playlist:', playlist.name)
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
