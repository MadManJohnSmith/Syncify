<template>
  <div class="accounts-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    
    <!-- Page Header -->
    <div class="px-8 pt-8 pb-6 flex items-center justify-between shrink-0">
      <div class="flex items-center gap-4">
        <div class="h-12 w-12 rounded-full bg-gradient-to-br from-violet-500 to-purple-400 text-white flex items-center justify-center">
          <span class="material-symbols-outlined text-[28px]">account_circle</span>
        </div>
        <div>
          <h1 class="text-3xl font-bold tracking-tight text-gray-900 dark:text-white mb-1">Import & Connections</h1>
          <p class="text-text-secondary">Manage your linked services and local library paths</p>
        </div>
      </div>
      
      <button 
        @click="showServiceModal = true"
        class="flex items-center gap-2 px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-semibold transition-colors shadow-lg shadow-primary/20"
      >
        <span class="material-symbols-outlined text-[20px]">add</span>
        Add Connection
      </button>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto custom-scrollbar px-8 pb-8">
      
      <!-- Connected Services Section -->
      <section class="mb-10">
        <h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Connected Services</h2>
        
        <div class="service-cards grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          
          <!-- Service Card Component -->
          <div 
            v-for="service in services" 
            :key="service.id"
            :class="[
              'service-card group relative rounded-xl border bg-white dark:bg-surface-dark p-6 transition-all hover:shadow-lg',
              service.status === 'connected' ? 'border-gray-200 dark:border-border-dark hover:border-primary/50' :
              service.status === 'expiring' ? 'border-amber-500/50' :
              'border-gray-200 dark:border-border-dark opacity-70 hover:opacity-100'
            ]"
          >
            <!-- Settings gear icon -->
            <button v-if="service.status !== 'disconnected'" @click="openServiceSettings(service)" class="absolute top-4 right-4 p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors opacity-0 group-hover:opacity-100">
              <span class="material-symbols-outlined text-[18px]">settings</span>
            </button>
            
            <!-- Service Header -->
            <div class="flex items-center gap-4 mb-5">
              <div :class="['h-14 w-14 flex items-center justify-center rounded-2xl text-2xl', service.bgClass]">
                {{ service.icon }}
              </div>
              <div>
                <h3 class="font-semibold text-gray-900 dark:text-white text-lg">{{ service.name }}</h3>
                <div class="flex items-center gap-1.5 mt-1">
                  <span :class="[
                    'px-2 py-0.5 rounded-full text-xs font-medium flex items-center gap-1',
                    service.status === 'connected' ? 'bg-success/10 text-success' :
                    service.status === 'expiring' ? 'bg-amber-500/10 text-amber-500' :
                    service.status === 'invalid' ? 'bg-error/10 text-error' :
                    'bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400'
                  ]">
                    <span v-if="service.status === 'connected'" class="material-symbols-outlined text-[14px]">check_circle</span>
                    <span v-else-if="service.status === 'expiring'" class="material-symbols-outlined text-[14px]">warning</span>
                    <span v-else-if="service.status === 'invalid'" class="material-symbols-outlined text-[14px]">error</span>
                    {{ service.status === 'connected' ? 'Connected' : service.status === 'expiring' ? 'Session Expiring' : service.status === 'invalid' ? 'Reconnect Required' : 'Not Connected' }}
                  </span>
                </div>
              </div>
            </div>
            
            <!-- Stats (if connected) -->
            <div v-if="service.status === 'connected'" class="mb-5">
              <div class="grid grid-cols-3 gap-3 mb-3">
                <div class="text-center p-2 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                  <div class="flex items-center justify-center gap-1 mb-1">
                    <span class="material-symbols-outlined text-[16px] text-gray-400">music_note</span>
                  </div>
                  <p class="text-sm font-semibold text-gray-900 dark:text-white">{{ service.tracks }}</p>
                  <p class="text-[10px] text-text-secondary uppercase">Tracks</p>
                </div>
                <div class="text-center p-2 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                  <div class="flex items-center justify-center gap-1 mb-1">
                    <span class="material-symbols-outlined text-[16px] text-gray-400">queue_music</span>
                  </div>
                  <p class="text-sm font-semibold text-gray-900 dark:text-white">{{ service.playlists }}</p>
                  <p class="text-[10px] text-text-secondary uppercase">Playlists</p>
                </div>
                <div class="text-center p-2 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                  <div class="flex items-center justify-center gap-1 mb-1">
                    <span class="material-symbols-outlined text-[16px] text-gray-400">favorite</span>
                  </div>
                  <p class="text-sm font-semibold text-gray-900 dark:text-white">{{ service.favorites }}</p>
                  <p class="text-[10px] text-text-secondary uppercase">Favorites</p>
                </div>
              </div>
              <p class="text-xs text-text-secondary text-center">Last synced: {{ service.lastSync }}</p>
              <!-- Apple Music: iCloud Music Library hint -->
              <p v-if="service.id === 'apple_music' && service.tracks === '0' && service.lastSync !== 'Never'" class="text-[11px] text-amber-500 text-center mt-2 leading-tight">
                ⚠️ Sync requires iCloud Music Library enabled in Apple Music → Preferences
              </p>
            </div>
            
            <!-- Session expiring message -->
            <div v-else-if="service.status === 'expiring'" class="mb-5">
              <div class="p-3 bg-amber-500/5 border border-amber-500/20 rounded-lg">
                <p class="text-xs text-amber-500">Your session will expire soon. Please re-authenticate to continue syncing.</p>
              </div>
            </div>
            
            <!-- Session invalid message -->
            <div v-else-if="service.status === 'invalid'" class="mb-5">
              <div class="p-3 bg-error/5 border border-error/20 rounded-lg">
                <p class="text-xs text-error">Session invalid (Machine changed or corrupted). Please reconnect to restore sync.</p>
              </div>
            </div>
            
            <!-- Actions -->
            <div class="flex items-center gap-2">
              <template v-if="service.status === 'connected'">
                <button 
                  @click="importFromService(service.id)"
                  :disabled="syncingServices[service.id]"
                  :class="[
                    'flex-1 px-4 py-2.5 rounded-lg text-sm font-medium transition-colors flex items-center justify-center gap-2',
                    syncingServices[service.id]
                      ? 'bg-primary/50 text-white/70 cursor-wait' 
                      : 'bg-primary/10 text-primary hover:bg-primary/20'
                  ]"
                >
                  <span v-if="syncingServices[service.id]" class="material-symbols-outlined text-[18px] animate-spin">sync</span>
                  <span v-else class="material-symbols-outlined text-[18px]">sync</span>
                  {{ syncingServices[service.id] ? 'Syncing...' : 'Sync Now' }}
                </button>
                <button 
                  @click="disconnectService(service.id)"
                  :disabled="authLoading !== null"
                  class="px-4 py-2.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors"
                >
                  {{ authLoading === service.id ? 'Disconnecting...' : 'Disconnect' }}
                </button>
              </template>
              <template v-else-if="service.status === 'expiring' || service.status === 'invalid'">
                <button 
                  @click="reconnectService(service.id)"
                  :disabled="authLoading !== null"
                  :class="[
                    'flex-1 px-4 py-2.5 text-white rounded-lg text-sm font-medium transition-colors flex items-center justify-center gap-2',
                    service.status === 'invalid' ? 'bg-error hover:bg-error-hover' : 'bg-amber-500 hover:bg-amber-600'
                  ]"
                >
                  <span v-if="authLoading === service.id" class="material-symbols-outlined text-[18px] animate-spin">sync</span>
                  <span v-else class="material-symbols-outlined text-[18px]">refresh</span>
                  {{ authLoading === service.id ? 'Connecting...' : (service.status === 'invalid' ? 'Reconnect' : 'Re-authenticate') }}
                </button>
              </template>
              <template v-else>
                <button 
                  @click="connectServiceFromCard(service.id)"
                  :disabled="authLoading !== null"
                  class="flex-1 px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors flex items-center justify-center gap-2"
                >
                  <span v-if="authLoading === service.id" class="material-symbols-outlined text-[18px] animate-spin">sync</span>
                  <span v-else class="material-symbols-outlined text-[18px]">link</span>
                  {{ authLoading === service.id ? 'Connecting...' : 'Connect' }}
                </button>
              </template>
            </div>
          </div>
          
        </div>
      </section>

      <!-- Import Tools Section -->
      <section class="import-tools mb-10">
        <h2 class="text-xl font-semibold text-gray-900 dark:text-white mb-4">Import Tools</h2>
        
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          
          <!-- Import from URL -->
          <div class="import-card url-import p-6 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark">
            <div class="flex items-center gap-4 mb-4">
              <div class="h-12 w-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
                <span class="material-symbols-outlined text-[28px]">link</span>
              </div>
              <div>
                <h3 class="font-semibold text-gray-900 dark:text-white">Import from URL</h3>
                <p class="text-sm text-text-secondary">Import playlists or albums from streaming service URLs</p>
              </div>
            </div>
            
            <div class="space-y-3">
              <div class="relative">
                <input 
                  v-model="importUrl"
                  type="text" 
                  placeholder="Paste Spotify/Qobuz/Tidal URL here..."
                  class="w-full px-4 py-3 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm"
                >
              </div>
              <p class="text-xs text-text-secondary">e.g., https://open.spotify.com/playlist/...</p>
              
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <span class="text-xs text-text-secondary">Supported:</span>
                  <div class="flex gap-1.5">
                    <span class="text-lg" title="Spotify">🎵</span>
                    <span class="text-lg" title="Qobuz">🎧</span>
                    <span class="text-lg" title="Tidal">🌊</span>
                    <span class="text-lg" title="Deezer">🎶</span>
                  </div>
                </div>
                <button 
                  @click="handleImportUrl"
                  :disabled="!importUrl || importUrlLoading" 
                  :class="[
                    'px-5 py-2 rounded-lg text-sm font-medium transition-colors flex items-center gap-2', 
                    importUrl && !importUrlLoading 
                      ? 'bg-primary hover:bg-primary-hover text-white' 
                      : 'bg-gray-200 dark:bg-gray-700 text-gray-500 cursor-not-allowed'
                  ]"
                >
                  <span v-if="importUrlLoading" class="material-symbols-outlined text-[16px] animate-spin">sync</span>
                  {{ importUrlLoading ? 'Parsing...' : 'Import' }}
                </button>
              </div>
            </div>
          </div>
          
          <!-- Import from File -->
          <div class="import-card file-import p-6 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark">
            <div class="flex items-center gap-4 mb-4">
              <div class="h-12 w-12 rounded-xl bg-purple-500/10 text-purple-500 flex items-center justify-center">
                <span class="material-symbols-outlined text-[28px]">description</span>
              </div>
              <div>
                <h3 class="font-semibold text-gray-900 dark:text-white">Import from File</h3>
                <p class="text-sm text-text-secondary">Import playlists from CSV, M3U, or text files</p>
              </div>
            </div>
            
            <div 
              @dragover.prevent="isDragging = true"
              @dragleave="isDragging = false"
              @drop.prevent="handleFileDrop"
              :class="[
                'drag-drop-area p-6 border-2 border-dashed rounded-xl text-center cursor-pointer transition-all',
                isDragging ? 'border-primary bg-primary/5' : 'border-gray-300 dark:border-gray-600 hover:border-primary/50'
              ]"
              @click="triggerFileInput"
            >
              <input ref="fileInput" type="file" accept=".csv,.m3u,.m3u8,.txt" class="hidden" @change="handleFileSelect">
              <span class="material-symbols-outlined text-[32px] text-gray-400 mb-2">cloud_upload</span>
              <p class="text-sm text-gray-700 dark:text-gray-300 mb-1">Drag file here or click to browse</p>
              <p class="text-xs text-text-secondary">CSV, M3U, M3U8, TXT</p>
            </div>
            
            <div class="mt-3 flex justify-end">
              <button @click="triggerFileInput" class="px-4 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
                Browse Files
              </button>
            </div>
          </div>
          
        </div>
      </section>

      <!-- Local Library Section -->
      <section class="local-library">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-semibold text-gray-900 dark:text-white">Local Library</h2>
          <span class="text-sm text-text-secondary">{{ libraryPaths.length }} folder{{ libraryPaths.length !== 1 ? 's' : '' }}</span>
        </div>
        
        <div class="rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark overflow-hidden">
          <!-- Path Items -->
          <div 
            v-for="path in libraryPaths" 
            :key="path.id"
            class="library-path flex items-center justify-between p-5 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors"
          >
            <div class="flex items-center gap-4">
              <div class="h-11 w-11 flex items-center justify-center rounded-xl bg-gradient-to-br from-blue-500/10 to-cyan-500/10 text-blue-500">
                <span class="material-symbols-outlined text-[24px]">folder</span>
              </div>
              <div class="flex flex-col">
                <span class="text-sm font-medium text-gray-900 dark:text-white font-mono">{{ path.path }}</span>
                <div class="flex items-center gap-3 mt-1">
                  <span class="text-xs text-text-secondary">{{ path.tracks }} Tracks</span>
                  <span class="text-[10px] text-gray-600 dark:text-gray-500">•</span>
                  <span :class="['flex items-center gap-1 text-xs', path.status === 'healthy' ? 'text-success' : 'text-amber-500']">
                    <span class="w-1.5 h-1.5 rounded-full" :class="path.status === 'healthy' ? 'bg-success' : 'bg-amber-500'"></span>
                    {{ path.status === 'healthy' ? 'Healthy' : 'Needs Scan' }}
                  </span>
                  <span class="text-[10px] text-gray-600 dark:text-gray-500">•</span>
                  <span class="text-xs text-text-secondary">Last scan: {{ path.lastScan }}</span>
                </div>
              </div>
            </div>
            
            <div class="flex items-center gap-1">
              <button @click="rescanPath(path)" class="p-2.5 text-gray-400 hover:text-primary hover:bg-primary/10 rounded-lg transition-colors" title="Rescan">
                <span class="material-symbols-outlined text-[20px]">sync</span>
              </button>
              <button @click="removePath(path)" class="p-2.5 text-gray-400 hover:text-error hover:bg-error/10 rounded-lg transition-colors" title="Remove">
                <span class="material-symbols-outlined text-[20px]">delete</span>
              </button>
            </div>
          </div>
          
          <!-- Add Path Button -->
          <div class="p-4 bg-gray-50 dark:bg-[#121b29]/50">
            <button @click="showScanDialog = true" class="w-full py-4 border-2 border-dashed border-gray-300 dark:border-border-dark rounded-xl text-gray-500 dark:text-gray-400 hover:border-primary hover:text-primary hover:bg-primary/5 transition-all flex items-center justify-center gap-2 text-sm font-medium">
              <span class="material-symbols-outlined text-[20px]">add</span>
              Add Library Path
            </button>
          </div>
        </div>
      </section>
      
      <!-- Recent Activity Section (Collapsible) -->
      <section class="activity-log">
        <button @click="showActivityLog = !showActivityLog" class="w-full flex items-center justify-between py-4">
          <div class="flex items-center gap-3">
            <h2 class="text-xl font-semibold text-gray-900 dark:text-white">Recent Activity</h2>
            <span class="px-2 py-0.5 bg-primary/10 text-primary text-xs font-medium rounded-full">{{ activityLog.length }} recent</span>
          </div>
          <span :class="['material-symbols-outlined text-gray-400 transition-transform', showActivityLog ? 'rotate-180' : '']">expand_more</span>
        </button>
        
        <Transition name="expand">
          <div v-if="showActivityLog" class="rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark overflow-hidden">
            <table class="w-full">
              <thead>
                <tr class="border-b border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30">
                  <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide w-24">Time</th>
                  <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide w-32">Service</th>
                  <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide">Action</th>
                  <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide">Result</th>
                  <th class="px-4 py-3 text-center text-xs font-semibold text-text-secondary uppercase tracking-wide w-16">Status</th>
                </tr>
              </thead>
              <tbody>
                <tr 
                  v-for="entry in activityLog" 
                  :key="entry.id"
                  class="activity-entry border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors"
                >
                  <td class="px-4 py-3 text-sm text-text-secondary">{{ entry.time }}</td>
                  <td class="px-4 py-3">
                    <div class="flex items-center gap-2">
                      <span class="text-lg">{{ entry.serviceIcon }}</span>
                      <span class="text-sm text-gray-700 dark:text-gray-300">{{ entry.service }}</span>
                    </div>
                  </td>
                  <td class="px-4 py-3 text-sm text-gray-900 dark:text-white">{{ entry.action }}</td>
                  <td class="px-4 py-3 text-sm" :class="entry.success ? 'text-success' : 'text-error'">{{ entry.result }}</td>
                  <td class="px-4 py-3 text-center">
                    <span :class="['material-symbols-outlined text-[18px]', entry.success ? 'text-success' : 'text-error']">
                      {{ entry.success ? 'check_circle' : 'cancel' }}
                    </span>
                  </td>
                </tr>
              </tbody>
            </table>
            <div class="px-4 py-3 border-t border-gray-200 dark:border-border-dark text-center">
              <button class="text-sm text-primary hover:text-primary-hover font-medium">View All Activity</button>
            </div>
          </div>
        </Transition>
      </section>
      
    </div>
    
    <!-- Service Settings Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showSettingsModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showSettingsModal = false">
          <div class="service-settings-modal bg-white dark:bg-surface-dark rounded-2xl w-full max-w-xl max-h-[90vh] overflow-hidden shadow-2xl">
            <!-- Modal Header -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <div class="flex items-center gap-3">
                <div :class="['h-10 w-10 rounded-xl flex items-center justify-center text-xl', selectedService?.bgClass]">
                  {{ selectedService?.icon }}
                </div>
                <h3 class="font-semibold text-gray-900 dark:text-white text-lg">{{ selectedService?.name }} Settings</h3>
              </div>
              <button @click="showSettingsModal = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <!-- Tab Navigation -->
            <div class="modal-tabs flex border-b border-gray-200 dark:border-border-dark px-6">
              <button 
                v-for="tab in settingsTabs" 
                :key="tab.id"
                @click="activeSettingsTab = tab.id"
                :class="[
                  'px-4 py-3 text-sm font-medium border-b-2 -mb-px transition-colors',
                  activeSettingsTab === tab.id 
                    ? 'text-primary border-primary' 
                    : 'text-text-secondary border-transparent hover:text-gray-700 dark:hover:text-gray-300'
                ]"
              >
                {{ tab.label }}
              </button>
            </div>
            
            <!-- Tab Content -->
            <div class="p-6 overflow-y-auto max-h-[50vh]">
              <!-- General Tab -->
              <div v-if="activeSettingsTab === 'general'" class="space-y-6">
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Account</h4>
                  <div class="p-4 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg space-y-3">
                    <div class="flex items-center justify-between">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Connected as:</span>
                      <span class="text-sm font-medium text-gray-900 dark:text-white">{{ selectedService?.email || 'user@example.com' }}</span>
                    </div>
                    <div class="flex gap-2">
                      <button class="flex-1 px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 transition-colors">
                        Switch Account
                      </button>
                      <button class="flex-1 px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 transition-colors">
                        Re-authenticate
                      </button>
                    </div>
                    <button class="w-full px-3 py-2 border border-error/50 text-error hover:bg-error/10 rounded-lg text-sm font-medium transition-colors">
                      Disconnect Account
                    </button>
                  </div>
                </div>
                
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Display Name</h4>
                  <input type="text" :placeholder="'e.g., Personal ' + selectedService?.name" class="w-full px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm">
                </div>
              </div>
              
              <!-- Import Tab -->
              <div v-else-if="activeSettingsTab === 'import'" class="space-y-6">
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Auto-Import</h4>
                  <div class="space-y-3">
                    <label class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Auto-import new favorites</span>
                      <input type="checkbox" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    </label>
                    <label class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Auto-import new playlists</span>
                      <input type="checkbox" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    </label>
                    <label class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Auto-import saved albums</span>
                      <input type="checkbox" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    </label>
                    <label class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Auto-import followed artists</span>
                      <input type="checkbox" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    </label>
                  </div>
                </div>
                
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Import Schedule</h4>
                  <div class="space-y-3">
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Import frequency</span>
                      <select class="px-3 py-1.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                        <option>Manual only</option>
                        <option>Hourly</option>
                        <option selected>Every 6 hours</option>
                        <option>Daily</option>
                        <option>Weekly</option>
                      </select>
                    </div>
                    <label class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Import on app startup</span>
                      <input type="checkbox" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    </label>
                  </div>
                </div>
                
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Download Preferences</h4>
                  <div class="space-y-3">
                    <label class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Auto-download newly imported favorites</span>
                      <input type="checkbox" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                    </label>
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Preferred quality</span>
                      <select class="px-3 py-1.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50">
                        <option selected>Best available</option>
                        <option>Hi-Res only</option>
                        <option>CD Quality</option>
                        <option>Lossy</option>
                      </select>
                    </div>
                  </div>
                </div>
              </div>
              
              <!-- Advanced Tab -->
              <div v-else-if="activeSettingsTab === 'advanced'" class="space-y-6">
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">API Settings</h4>
                  <div class="space-y-3">
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">API Rate Limit</span>
                      <span class="text-sm font-medium text-gray-900 dark:text-white">180 requests/min</span>
                    </div>
                    <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Request delay (ms)</span>
                      <input type="number" value="100" class="w-20 px-3 py-1.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white text-right focus:outline-none focus:ring-2 focus:ring-primary/50">
                    </div>
                  </div>
                </div>
                
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Cache</h4>
                  <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg">
                    <div>
                      <span class="text-sm text-gray-700 dark:text-gray-300">Cache size:</span>
                      <span class="text-sm font-medium text-gray-900 dark:text-white ml-2">45 MB</span>
                    </div>
                    <button class="px-3 py-1.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 transition-colors">
                      Clear Cache
                    </button>
                  </div>
                </div>
                
                <div>
                  <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Sync Scope</h4>
                  <div class="space-y-2">
                    <label class="flex items-center gap-3 p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <input type="checkbox" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Public playlists</span>
                    </label>
                    <label class="flex items-center gap-3 p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <input type="checkbox" checked class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Private playlists</span>
                    </label>
                    <label class="flex items-center gap-3 p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <input type="checkbox" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Collaborative playlists</span>
                    </label>
                    <label class="flex items-center gap-3 p-3 bg-gray-50 dark:bg-surface-highlight/50 rounded-lg cursor-pointer">
                      <input type="checkbox" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Podcast subscriptions</span>
                    </label>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- Modal Footer -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3">
              <button @click="showSettingsModal = false" class="px-5 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium transition-colors">
                Cancel
              </button>
              <button @click="showSettingsModal = false" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                Save Changes
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Add Connection Wizard Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showServiceModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showServiceModal = false">
          <div class="add-connection-wizard bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[90vh] overflow-hidden shadow-2xl">
            <!-- Modal Header -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <h3 class="font-semibold text-gray-900 dark:text-white text-lg">Add New Service</h3>
              <button @click="showServiceModal = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <!-- Service Grid -->
            <div class="p-6">
              <p class="text-sm text-text-secondary mb-4">Select a streaming service to connect:</p>
              <div class="grid grid-cols-3 gap-4">
                <button 
                  v-for="service in services" 
                  :key="service.id"
                  @click="connectService(service)"
                  :disabled="service.status !== 'disconnected'"
                  :class="[
                    'p-4 rounded-xl border text-center transition-all',
                    service.status === 'disconnected' 
                      ? 'border-gray-200 dark:border-border-dark hover:border-primary/50 hover:bg-primary/5 cursor-pointer' 
                      : 'border-gray-200 dark:border-border-dark opacity-50 cursor-not-allowed'
                  ]"
                >
                  <div :class="['h-12 w-12 mx-auto rounded-xl flex items-center justify-center text-2xl mb-2', service.bgClass]">
                    {{ service.icon }}
                  </div>
                  <p class="font-medium text-gray-900 dark:text-white text-sm">{{ service.name }}</p>
                  <p v-if="service.status !== 'disconnected'" class="text-xs text-success mt-1">Already connected</p>
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Add Library Path Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showScanDialog" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showScanDialog = false">
          <div class="scan-dialog bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Add Local Library Path</h3>
            </div>
            
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Folder Path</label>
                <div class="flex gap-2">
                  <input v-model="newLibraryPath" type="text" placeholder="C:/Music/Library" class="flex-1 px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50 text-sm font-mono">
                  <button @click="browseFolder" class="px-4 py-2.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
                    Browse...
                  </button>
                </div>
              </div>
              
              <div class="space-y-3">
                <label class="flex items-center gap-3 cursor-pointer">
                  <input type="checkbox" v-model="scanOptions.includeSubfolders" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                  <span class="text-sm text-gray-700 dark:text-gray-300">Include subfolders</span>
                </label>
                <label class="flex items-center gap-3 cursor-pointer">
                  <input type="checkbox" v-model="scanOptions.watchForChanges" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                  <span class="text-sm text-gray-700 dark:text-gray-300">Watch for changes (auto-rescan)</span>
                </label>
                <label class="flex items-center gap-3 cursor-pointer">
                  <input type="checkbox" v-model="scanOptions.skipSmallFiles" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                  <span class="text-sm text-gray-700 dark:text-gray-300">Skip files under 1 MB</span>
                </label>
              </div>
            </div>
            
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3">
              <button @click="showScanDialog = false" class="px-5 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium transition-colors">
                Cancel
              </button>
              <button @click="startScan" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                Start Scan
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { accountsApi } from '@/api/accounts'
import { libraryApi } from '@/api/library'
import { useToast } from '@/composables/useToast'
import type { ImportResult } from '@/api/types'
import { useEventBus, TauriEvents } from '@/composables/useEventBus'
import { useGlobalTasks } from '@/composables/useGlobalTasks'
import { useSyncSettings } from '@/composables/useSyncSettings'
import { useAccountsStatus } from '@/composables/useAccountsStatus'

const toast = useToast()
const eventBus = useEventBus()
const globalTasks = useGlobalTasks()
const syncSettings = useSyncSettings()
const { services, rawServices, rawAccounts, fetchData, findAccountForService } = useAccountsStatus()

const showServiceModal = ref(false)
const authLoading = ref<string | null>(null)
const syncingServices = reactive<Record<string, boolean>>({})
const importUrl = ref('')
const importUrlLoading = ref(false)

function showToast(message: string, type: 'success' | 'error' | 'info' = 'success') {
  if (type === 'success') {
    toast.success(message)
  } else if (type === 'error') {
    toast.error(message)
  } else {
    toast.info(message)
  }
}



// Connect service
async function connectService(service: { id: string; name: string }) {
  authLoading.value = service.id
  showServiceModal.value = false
  
  try {
    // S65: Use native WebView2 for Spotify, Python bridge for others
    const result = service.id === 'spotify'
      ? await accountsApi.spotifyAuthWebview()
      : await accountsApi.startAuthAndSave(service.id)
    
    if (result.success) {
      await fetchData()
    } else {
      showToast(`Auth failed: ${result.error || 'Unknown error'}`, 'error')
    }
  } catch (e) {
    showToast(`Failed to connect service: ${e}`, 'error')
  } finally {
    authLoading.value = null
  }
}

// Connect service from card (simplified version)
async function connectServiceFromCard(serviceId: string) {
  authLoading.value = serviceId
  
  try {
    // S65: Use native WebView2 for Spotify, Python bridge for others
    const result = serviceId === 'spotify'
      ? await accountsApi.spotifyAuthWebview()
      : await accountsApi.startAuthAndSave(serviceId)
    
    if (result.success) {
      await fetchData()
      showToast(`Connected to ${serviceId} successfully!`, 'success')
    } else {
      showToast(`Failed to connect: ${result.error || 'Unknown error'}`, 'error')
    }
  } catch (e) {
    showToast(`Connection error: ${e}`, 'error')
  } finally {
    authLoading.value = null
  }
}

// Disconnect service
async function disconnectService(serviceId: string) {
  authLoading.value = serviceId
  
  try {
    // First logout via Python bridge (clears session files)
    await accountsApi.logoutService(serviceId)
    
    // Find the account for this service and remove it from database
    const account = findAccountForService(serviceId)
    
    if (account) {
      await accountsApi.removeAccount(account.id)
    }
    
    // Always refresh data
    await fetchData()
    showToast(`Disconnected from ${serviceId}`, 'success')
  } catch (e) {
    showToast(`Disconnect error: ${e}`, 'error')
  } finally {
    authLoading.value = null
  }
}

// Reconnect service (re-authenticate)
async function reconnectService(serviceId: string) {
  // Same as connect - just re-do the auth flow
  await connectServiceFromCard(serviceId)
}

// Import from URL - parse streaming service URL
async function handleImportUrl() {
  if (!importUrl.value.trim()) {
    showToast('Please enter a URL to import', 'error')
    return
  }
  
  importUrlLoading.value = true
  
  try {
    const result = await accountsApi.importFromUrl(importUrl.value.trim())
    
    // Format service name nicely
    const serviceName = result.service.charAt(0).toUpperCase() + result.service.slice(1)
    const contentType = result.content_type.charAt(0).toUpperCase() + result.content_type.slice(1)
    
    showToast(`Parsed ${serviceName} ${contentType}: ${result.id}`, 'success')
    
    // Clear the input after successful parse
    importUrl.value = ''
  } catch (e) {
    showToast(`Failed to parse URL: ${e}`, 'error')
  } finally {
    importUrlLoading.value = false
  }
}

// Import from service with frontend-owned task lifecycle
async function importFromService(serviceName: string) {
  const serviceKey = serviceName.toLowerCase()
  
  // Prevent duplicate sync calls if already syncing this service
  if (syncingServices[serviceKey]) {
    showToast(`${serviceName} sync already in progress`, 'info')
    return
  }
  
  syncingServices[serviceKey] = true  // Use object property for Vue reactivity
  
  // Start task in global task system (frontend-owned)
  const taskId = globalTasks.startSyncTask(serviceName)
  showToast(`Syncing ${serviceName}...`, 'info')
  
  try {
    let result: ImportResult | undefined
    let notImplemented = false
    let playlistsResult: ImportResult | undefined
    
    switch (serviceKey) {
      case 'spotify':
        // Import favorites and playlists in PARALLEL (lock-free backend)
        const importPromises: Promise<ImportResult>[] = []
        
        if (syncSettings.shouldSyncFavorites('spotify')) {
          importPromises.push(accountsApi.importSpotifyLibrary())
        }
        if (syncSettings.shouldSyncPlaylists('spotify')) {
          importPromises.push(accountsApi.importSpotifyPlaylists())
        }
        
        if (importPromises.length > 0) {
          try {
            const results = await Promise.all(importPromises)
            
            // First result is favorites (if enabled), second is playlists
            if (syncSettings.shouldSyncFavorites('spotify')) {
              result = results[0]
              if (syncSettings.shouldSyncPlaylists('spotify')) {
                playlistsResult = results[1]
              }
            } else if (syncSettings.shouldSyncPlaylists('spotify')) {
              playlistsResult = results[0]
              result = { imported: 0, skipped: 0, errors: [] }
            }
            
            if (playlistsResult) {
              globalTasks.completeTask(`sync-spotify_playlists`, true)
            }
          } catch (e) {
            throw e
          }
        } else {
          result = { imported: 0, skipped: 0, errors: [] }
        }
        break
      case 'qobuz':
        result = await accountsApi.importQobuzLibrary()
        break
      case 'tidal':
        result = await accountsApi.importTidalLibrary()
        break
      case 'deezer':
        result = await accountsApi.importDeezerLibrary()
        break
      case 'soundcloud':
        result = await accountsApi.importSoundCloudLibrary()
        break
      case 'apple_music':
        result = await accountsApi.importAppleMusicLibrary()
        break
      default:
        notImplemented = true
        showToast(`${serviceName} import not available`, 'error')
        globalTasks.completeTask(taskId, false, 'Unknown service')
    }
    
    if (!notImplemented && result) {
      await fetchData()
      const playlistMsg = playlistsResult ? ` + ${playlistsResult.imported} playlists` : ''
      showToast(`Synced ${result.imported} tracks${playlistMsg}, ${result.skipped} skipped`, 'success')
    } else if (!notImplemented) {
      showToast(`Sync completed but no data returned`, 'error')
    }
  } catch (e: any) {
    const errorMsg = e?.message || e?.toString() || String(e) || 'Unknown error'
    
    // Show user-friendly error messages
    if (errorMsg.includes('401') || errorMsg.includes('Unauthorized') || errorMsg.includes('Decryption error') || errorMsg.includes('Credentials expired')) {
      showToast(`⚠️ Service credentials expired. Please reconnect ${serviceName}`, 'error')
    } else if (errorMsg.includes('403') || errorMsg.includes('Forbidden')) {
      showToast(`🔒 Access denied for ${serviceName} - check your subscription`, 'error')
    } else if (errorMsg.includes('network') || errorMsg.includes('fetch')) {
      showToast(`📡 Network error - check your internet connection`, 'error')
    } else if (errorMsg.includes('timeout')) {
      showToast(`⏱️ Request timed out - try again later`, 'error')
    } else if (errorMsg.includes('Token refresh') || errorMsg.includes('refresh_token') || errorMsg.includes('Missing refresh token')) {
      showToast(`🔑 ${serviceName} session expired - please reconnect your account`, 'error')
    } else if (errorMsg.includes('SPOTIFY_CLIENT_ID') || errorMsg.includes('SPOTIFY_CLIENT_SECRET')) {
      showToast(`⚙️ Spotify API keys not configured - check .env file`, 'error')
    } else if (errorMsg.includes('400') || errorMsg.includes('CloudLibrary') || errorMsg.includes('Insufficient')) {
      showToast(`🍎 Apple Music: Enable iCloud Music Library in your Apple Music settings to sync your library. Go to Music → Preferences → General → iCloud Music Library.`, 'error')
    } else {
      showToast(`❌ Sync failed: ${errorMsg.substring(0, 100)}`, 'error')
    }
  } finally {
    // S76: Always complete the task to prevent "stuck" progress bars
    // If the task is already completed (e.g. by a success path), completeTask is idempotent
    globalTasks.completeTask(taskId, true)
    delete syncingServices[serviceKey]
  }
}

// Import state
const isDragging = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)

function triggerFileInput() {
  fileInput.value?.click()
}

function handleFileDrop(e: DragEvent) {
  isDragging.value = false
  const files = e.dataTransfer?.files
  if (files?.length) {
    // TODO: Implement file import processing
  }
}

function handleFileSelect(e: Event) {
  const target = e.target as HTMLInputElement
  const files = target.files
  if (files?.length) {
    // TODO: Implement file import processing
  }
}

// Library paths
const libraryPaths = ref([
  { id: 1, path: 'D:/Music/Flac_Library', tracks: '2,405', status: 'healthy', lastScan: '1 day ago' },
  { id: 2, path: 'E:/Downloads/Music', tracks: '847', status: 'pending', lastScan: '1 week ago' },
])

// Activity Log
const showActivityLog = ref(false)
const activityLog = ref([
  { id: 1, time: '2h ago', service: 'Spotify', serviceIcon: '🎵', action: 'Synced favorites', result: 'Added 15 tracks', success: true },
  { id: 2, time: '2h ago', service: 'Qobuz', serviceIcon: '🎧', action: 'Imported playlist', result: '24 tracks added', success: true },
])

// Service Settings Modal
const showSettingsModal = ref(false)
const selectedService = ref<any>(null)
const activeSettingsTab = ref('general')

const settingsTabs = [
  { id: 'general', label: 'General' },
  { id: 'import', label: 'Import' },
  { id: 'advanced', label: 'Advanced' },
]

function openServiceSettings(service: any) {
  selectedService.value = service
  activeSettingsTab.value = 'general'
  showSettingsModal.value = true
}

// Scan Dialog
const showScanDialog = ref(false)
const newLibraryPath = ref('')
const isScanning = ref(false)
const scanOptions = reactive({
  includeSubfolders: true,
  watchForChanges: false,
  skipSmallFiles: false,
})

// Browse for folder using native dialog
async function browseFolder() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Music Library Folder',
    })
    
    if (selected && typeof selected === 'string') {
      newLibraryPath.value = selected
    }
  } catch (e) {
    showToast('Failed to open folder dialog', 'error')
  }
}

// Start local library scan
async function startScan() {
  if (!newLibraryPath.value.trim()) {
    showToast('Please select a folder to scan', 'error')
    return
  }
  
  isScanning.value = true
  const path = newLibraryPath.value.trim()
  
  // Start task in global task system
  const taskId = globalTasks.startScanTask(path)
  showScanDialog.value = false
  
  try {
    // Call backend scan with progress events
    const result = await libraryApi.scanLocalLibraryWithProgress(path, {
      recursive: scanOptions.includeSubfolders,
    })
    
    if (result.success && result.data) {
      const count = result.data.total_files
      const errorCount = result.data.errors?.length ?? 0
      
      // Add to library paths
      const newPath = {
        id: Date.now(),
        path,
        tracks: count.toLocaleString(),
        status: 'healthy' as const,
        lastScan: 'Just now',
      }
      libraryPaths.value.push(newPath)
      
      // Complete task and show toast
      globalTasks.completeTask(taskId, true)
      
      if (errorCount > 0) {
        showToast(`Scanned ${count} files (${errorCount} errors)`, 'info')
      } else {
        showToast(`Found ${count} audio files`, 'success')
      }
      
      // Add activity log entry
      activityLog.value.unshift({
        id: Date.now(),
        time: 'Just now',
        service: 'Local',
        serviceIcon: '📁',
        action: 'Scanned folder',
        result: `${count} files found`,
        success: true,
      })
    } else {
      globalTasks.completeTask(taskId, false, result.error || 'Scan failed')
      showToast(`Scan failed: ${result.error || 'Unknown error'}`, 'error')
    }
  } catch (e: any) {
    globalTasks.completeTask(taskId, false, e?.toString() || 'Scan failed')
    showToast(`Scan error: ${e}`, 'error')
  } finally {
    isScanning.value = false
    newLibraryPath.value = ''
  }
}

// Rescan an existing library path
async function rescanPath(pathEntry: typeof libraryPaths.value[0]) {
  const taskId = globalTasks.startScanTask(pathEntry.path)
  
  try {
    const result = await libraryApi.scanLocalLibraryWithProgress(pathEntry.path, {
      recursive: true,
    })
    
    if (result.success && result.data) {
      pathEntry.tracks = result.data.total_files.toLocaleString()
      pathEntry.status = 'healthy'
      pathEntry.lastScan = 'Just now'
      globalTasks.completeTask(taskId, true)
      showToast(`Rescanned: ${result.data.total_files} files`, 'success')
    } else {
      globalTasks.completeTask(taskId, false, result.error)
      showToast(`Rescan failed: ${result.error}`, 'error')
    }
  } catch (e: any) {
    globalTasks.completeTask(taskId, false, e?.toString())
    showToast(`Rescan error: ${e}`, 'error')
  }
}

// Remove a library path
function removePath(pathEntry: typeof libraryPaths.value[0]) {
  const index = libraryPaths.value.findIndex(p => p.id === pathEntry.id)
  if (index !== -1) {
    libraryPaths.value.splice(index, 1)
    showToast('Library path removed', 'info')
  }
}

// Initialize
onMounted(async () => {
  await fetchData()
  
  // Listen for import completion to refresh stats
  await eventBus.on(TauriEvents.IMPORT_COMPLETE, async (payload: any) => {
    if (payload?.service) {
      syncingServices[payload.service.toLowerCase()] = false
    }
    // Show toast with backend-composed message including real counts
    if (payload?.message) {
      showToast(payload.message, 'success')
    }
    await fetchData()
  })

  // Listen for import progress
  await eventBus.on(TauriEvents.IMPORT_PROGRESS, (payload: any) => {
    if (payload?.service) {
      // Ensure service is marked as syncing
      syncingServices[payload.service.toLowerCase()] = true
    }
  })
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

/* Service card hover effect */
.service-card:hover {
  transform: translateY(-2px);
}

/* Drag and drop styling */
.drag-drop-area {
  min-height: 120px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

/* Library path hover */
.library-path:hover .material-symbols-outlined {
  opacity: 1;
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

/* Expand transition */
.expand-enter-active,
.expand-leave-active {
  transition: all 0.3s ease;
  overflow: hidden;
}
.expand-enter-from,
.expand-leave-to {
  opacity: 0;
  max-height: 0;
}
.expand-enter-to,
.expand-leave-from {
  opacity: 1;
  max-height: 600px;
}
</style>
