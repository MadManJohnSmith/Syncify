<template>
  <div class="migrate-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    
    <!-- Page Header -->
    <div class="px-8 pt-8 pb-6 flex items-center justify-between shrink-0">
      <div class="flex items-center gap-4">
        <div class="h-12 w-12 rounded-full bg-gradient-to-br from-primary to-blue-400 text-white flex items-center justify-center">
          <span class="material-symbols-outlined text-[28px]">swap_horiz</span>
        </div>
        <div>
          <h1 class="text-3xl font-bold tracking-tight text-gray-900 dark:text-white mb-1">Migrate</h1>
          <p class="text-text-secondary">Transfer your music between streaming services</p>
        </div>
      </div>
      
      <!-- Quick Actions Bar -->
      <div class="flex items-center gap-3">
        <div class="relative">
          <button @click="showRecentDropdown = !showRecentDropdown" class="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors">
            <span class="material-symbols-outlined text-[18px]">history</span>
            Recent migrations
            <span class="material-symbols-outlined text-[16px] text-gray-400">expand_more</span>
          </button>
          <div v-if="showRecentDropdown" class="absolute top-full right-0 mt-1 w-64 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg shadow-xl z-20 py-2">
            <div class="px-3 py-2 text-xs font-semibold text-text-secondary uppercase tracking-wide">Recent</div>
            <button v-for="item in recentMigrations" :key="item.id" class="w-full px-4 py-2.5 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-3">
              <span class="text-gray-700 dark:text-gray-300">{{ item.source }} → {{ item.destination }}</span>
              <span class="ml-auto text-xs text-text-secondary">{{ item.date }}</span>
            </button>
          </div>
        </div>
        
        <button class="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors">
          <span class="material-symbols-outlined text-[18px]">bookmark</span>
          Saved templates
        </button>
        
        <button class="flex items-center gap-2 px-4 py-2.5 bg-primary/10 border border-primary/30 hover:bg-primary/20 text-primary rounded-lg text-sm font-medium transition-colors">
          <span class="material-symbols-outlined text-[18px]">schedule</span>
          Schedule migration
        </button>
      </div>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto custom-scrollbar px-8 pb-8">
      
      <!-- Transfer Wizard Card -->
      <div class="transfer-wizard max-w-[900px] mx-auto">
        <div class="bg-white dark:bg-surface-dark rounded-2xl border border-gray-200 dark:border-border-dark shadow-lg overflow-hidden">
          
          <!-- Step Progress Indicator -->
          <div class="step-progress px-8 py-6 border-b border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30">
            <div class="flex items-center justify-between relative">
              <!-- Progress Line Background -->
              <div class="absolute top-5 left-0 right-0 h-0.5 bg-gray-200 dark:bg-gray-700"></div>
              <!-- Progress Line Fill -->
              <div class="absolute top-5 left-0 h-0.5 bg-primary transition-all duration-500" :style="{ width: progressWidth }"></div>
              
              <!-- Step Dots -->
              <div v-for="(step, index) in steps" :key="step.id" class="relative z-10 flex flex-col items-center">
                <div 
                  :class="[
                    'w-10 h-10 rounded-full flex items-center justify-center text-sm font-bold transition-all duration-300',
                    currentStep > index ? 'bg-success text-white' : 
                    currentStep === index ? 'bg-primary text-white ring-4 ring-primary/20' : 
                    'bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400'
                  ]"
                >
                  <span v-if="currentStep > index" class="material-symbols-outlined text-[20px]">check</span>
                  <span v-else>{{ index + 1 }}</span>
                </div>
                <p :class="['text-xs font-medium mt-2 text-center', currentStep === index ? 'text-primary' : 'text-text-secondary']">{{ step.label }}</p>
              </div>
            </div>
          </div>
          
          <!-- Wizard Step Content -->
          <div class="wizard-content p-8">
            <Transition :name="stepDirection" mode="out-in">
              
              <!-- STEP 1: Select Source Service -->
              <div v-if="currentStep === 0" key="step1" class="wizard-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Select source service</h2>
                <p class="text-text-secondary mb-8">Choose where to transfer from</p>
                
                <div class="service-grid grid grid-cols-3 gap-4">
                  <button 
                    v-for="service in services" 
                    :key="service.id"
                    @click="service.connected && (sourceService = service.id)"
                    :disabled="!service.connected"
                    :class="[
                      'service-card relative p-6 rounded-xl border-2 transition-all text-center',
                      !service.connected ? 'opacity-50 cursor-not-allowed border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30' :
                      sourceService === service.id ? 'border-primary bg-primary/5 shadow-lg shadow-primary/10' :
                      'border-gray-200 dark:border-border-dark hover:border-primary/50 hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
                    ]"
                  >
                    <div :class="['w-20 h-20 mx-auto rounded-2xl flex items-center justify-center mb-4 text-3xl', service.bgClass]">
                      {{ service.icon }}
                    </div>
                    <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">{{ service.name }}</h3>
                    <div class="flex items-center justify-center gap-1.5">
                      <span :class="['w-2 h-2 rounded-full', service.connected ? 'bg-success' : 'bg-gray-400']"></span>
                      <span :class="['text-xs font-medium', service.connected ? 'text-success' : 'text-text-secondary']">
                        {{ service.connected ? 'Connected' : 'Not Connected' }}
                      </span>
                    </div>
                    <!-- Selected Indicator -->
                    <div v-if="sourceService === service.id" class="absolute top-3 right-3 w-6 h-6 rounded-full bg-primary text-white flex items-center justify-center">
                      <span class="material-symbols-outlined text-[16px]">check</span>
                    </div>
                  </button>
                </div>
              </div>
              
              <!-- STEP 2: Choose Content to Transfer -->
              <div v-else-if="currentStep === 1" key="step2" class="wizard-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">What do you want to transfer?</h2>
                <p class="text-text-secondary mb-8">Select the content types to migrate</p>
                
                <div class="content-cards grid grid-cols-2 gap-4">
                  <button 
                    v-for="content in contentTypes" 
                    :key="content.id"
                    @click="toggleContentType(content.id)"
                    :class="[
                      'content-card relative p-6 rounded-xl border-2 transition-all text-left',
                      selectedContent.includes(content.id) ? 'border-primary bg-primary/5' : 'border-gray-200 dark:border-border-dark hover:border-primary/50'
                    ]"
                  >
                    <div class="flex items-start gap-4">
                      <div :class="['w-14 h-14 rounded-xl flex items-center justify-center', selectedContent.includes(content.id) ? 'bg-primary/10 text-primary' : 'bg-gray-100 dark:bg-surface-highlight text-gray-500']">
                        <span class="material-symbols-outlined text-[28px]">{{ content.icon }}</span>
                      </div>
                      <div class="flex-1">
                        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{{ content.label }}</h3>
                        <p class="text-sm text-text-secondary">{{ content.count }}</p>
                      </div>
                    </div>
                    <!-- Checkbox -->
                    <div :class="[
                      'absolute top-4 right-4 w-6 h-6 rounded-md border-2 flex items-center justify-center transition-all',
                      selectedContent.includes(content.id) ? 'bg-primary border-primary text-white' : 'border-gray-300 dark:border-gray-600'
                    ]">
                      <span v-if="selectedContent.includes(content.id)" class="material-symbols-outlined text-[16px]">check</span>
                    </div>
                  </button>
                </div>
              </div>
              
              <!-- STEP 3: Select Destination Services -->
              <div v-else-if="currentStep === 2" key="step3" class="wizard-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Transfer to...</h2>
                <p class="text-text-secondary mb-8">Select one or more destination services</p>
                
                <div class="service-grid grid grid-cols-3 gap-4">
                  <button 
                    v-for="service in services" 
                    :key="service.id"
                    @click="toggleDestination(service.id)"
                    :disabled="!service.connected || service.id === sourceService"
                    :class="[
                      'service-card relative p-6 rounded-xl border-2 transition-all text-center',
                      service.id === sourceService ? 'opacity-30 cursor-not-allowed border-gray-200 dark:border-border-dark' :
                      !service.connected ? 'opacity-50 cursor-not-allowed border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30' :
                      destinationServices.includes(service.id) ? 'border-primary bg-primary/5 shadow-lg shadow-primary/10' :
                      'border-gray-200 dark:border-border-dark hover:border-primary/50 hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
                    ]"
                  >
                    <div :class="['w-20 h-20 mx-auto rounded-2xl flex items-center justify-center mb-4 text-3xl', service.bgClass]">
                      {{ service.icon }}
                    </div>
                    <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">{{ service.name }}</h3>
                    <div class="flex items-center justify-center gap-1.5">
                      <span v-if="service.id === sourceService" class="text-xs text-amber-500 font-medium">Source</span>
                      <template v-else>
                        <span :class="['w-2 h-2 rounded-full', service.connected ? 'bg-success' : 'bg-gray-400']"></span>
                        <span :class="['text-xs font-medium', service.connected ? 'text-success' : 'text-text-secondary']">
                          {{ service.connected ? 'Connected' : 'Not Connected' }}
                        </span>
                      </template>
                    </div>
                    <!-- Checkbox indicator -->
                    <div v-if="destinationServices.includes(service.id)" class="absolute top-3 right-3 w-6 h-6 rounded-full bg-primary text-white flex items-center justify-center">
                      <span class="material-symbols-outlined text-[16px]">check</span>
                    </div>
                  </button>
                </div>
              </div>
              
              <!-- STEP 4: Preview & Match Review -->
              <div v-else-if="currentStep === 3" key="step4" class="wizard-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Review matches</h2>
                <p class="text-text-secondary mb-6">Verify track matches before transferring</p>
                
                <!-- Summary Cards -->
                <div class="grid grid-cols-3 gap-4 mb-6">
                  <div class="p-4 rounded-xl bg-primary/5 border border-primary/20">
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-primary/10 flex items-center justify-center">
                        <span class="material-symbols-outlined text-primary">library_music</span>
                      </div>
                      <div>
                        <p class="text-xl font-bold text-gray-900 dark:text-white">1,234</p>
                        <p class="text-xs text-text-secondary">Total Items</p>
                      </div>
                    </div>
                  </div>
                  <div class="p-4 rounded-xl bg-success/5 border border-success/20">
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-success/10 flex items-center justify-center">
                        <span class="material-symbols-outlined text-success">verified</span>
                      </div>
                      <div>
                        <p class="text-xl font-bold text-success">1,198 <span class="text-xs font-normal">(97%)</span></p>
                        <p class="text-xs text-text-secondary">High Confidence</p>
                      </div>
                    </div>
                  </div>
                  <div class="p-4 rounded-xl bg-amber-500/5 border border-amber-500/20">
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-amber-500/10 flex items-center justify-center">
                        <span class="material-symbols-outlined text-amber-500">help</span>
                      </div>
                      <div>
                        <p class="text-xl font-bold text-amber-500">36 <span class="text-xs font-normal">(3%)</span></p>
                        <p class="text-xs text-text-secondary">Needs Review</p>
                      </div>
                    </div>
                  </div>
                </div>
                
                <!-- Filter Pills -->
                <div class="flex items-center gap-2 mb-4">
                  <button 
                    v-for="filter in matchFilters" 
                    :key="filter.id"
                    @click="matchFilter = filter.id"
                    :class="[
                      'px-4 py-2 rounded-full text-sm font-medium transition-all',
                      matchFilter === filter.id ? 'bg-primary text-white' : 'bg-gray-100 dark:bg-surface-highlight text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
                    ]"
                  >
                    {{ filter.label }}
                    <span v-if="filter.count" class="ml-1.5 opacity-70">({{ filter.count }})</span>
                  </button>
                </div>
                
                <!-- Match Preview List -->
                <div class="match-preview border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden max-h-[300px] overflow-y-auto custom-scrollbar">
                  <div 
                    v-for="match in filteredMatches" 
                    :key="match.id"
                    class="match-row flex items-center gap-4 px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30"
                  >
                    <!-- Source Track -->
                    <div class="flex items-center gap-3 flex-1 min-w-0">
                      <div :class="['w-10 h-10 rounded-lg shrink-0 flex items-center justify-center', match.sourceGradient]">
                        <span class="material-symbols-outlined text-white/50 text-lg">music_note</span>
                      </div>
                      <div class="min-w-0">
                        <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ match.sourceTitle }}</p>
                        <p class="text-xs text-text-secondary truncate">{{ match.sourceArtist }}</p>
                      </div>
                      <span :class="['px-1.5 py-0.5 rounded text-[9px] font-bold uppercase shrink-0', match.sourceServiceClass]">{{ match.sourceService }}</span>
                    </div>
                    
                    <!-- Match Indicator -->
                    <div class="flex items-center gap-2 shrink-0">
                      <span class="material-symbols-outlined text-gray-400">arrow_forward</span>
                      <span :class="['match-confidence px-2 py-1 rounded-full text-xs font-bold', match.confidenceClass]">
                        {{ match.confidence }}
                      </span>
                    </div>
                    
                    <!-- Destination Track -->
                    <div v-if="match.found" class="flex items-center gap-3 flex-1 min-w-0">
                      <div :class="['w-10 h-10 rounded-lg shrink-0 flex items-center justify-center', match.destGradient]">
                        <span class="material-symbols-outlined text-white/50 text-lg">music_note</span>
                      </div>
                      <div class="min-w-0">
                        <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ match.destTitle }}</p>
                        <p class="text-xs text-text-secondary truncate">{{ match.destArtist }}</p>
                      </div>
                      <span :class="['px-1.5 py-0.5 rounded text-[9px] font-bold uppercase shrink-0', match.destServiceClass]">{{ match.destService }}</span>
                      <span v-if="match.quality" class="px-1.5 py-0.5 rounded text-[9px] font-bold bg-quality-gold/10 text-quality-gold">{{ match.quality }}</span>
                    </div>
                    <div v-else class="flex items-center gap-3 flex-1">
                      <span class="text-sm text-text-secondary italic">No match found</span>
                      <button @click="showManualMatchModal = true; manualMatchTrack = match" class="ml-auto px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors">
                        Search Manually
                      </button>
                    </div>
                  </div>
                </div>
                
                <!-- Skip Toggle -->
                <div class="flex items-center gap-3 mt-4 pt-4 border-t border-gray-200 dark:border-border-dark">
                  <button 
                    @click="skipNotFound = !skipNotFound"
                    :class="['w-5 h-5 rounded border-2 flex items-center justify-center transition-colors', skipNotFound ? 'bg-primary border-primary text-white' : 'border-gray-300 dark:border-gray-600']"
                  >
                    <span v-if="skipNotFound" class="material-symbols-outlined text-[14px]">check</span>
                  </button>
                  <span class="text-sm text-gray-700 dark:text-gray-300">Skip all tracks with no match found (5 tracks)</span>
                </div>
              </div>
              
              <!-- STEP 5: Transfer in Progress -->
              <div v-else-if="currentStep === 4" key="step5" class="wizard-step">
                <!-- Not Started / Ready State -->
                <div v-if="!transferStarted" class="text-center py-12">
                  <div class="w-24 h-24 mx-auto rounded-full bg-primary/10 flex items-center justify-center mb-6">
                    <span class="material-symbols-outlined text-5xl text-primary">cloud_sync</span>
                  </div>
                  <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Ready to transfer</h2>
                  <p class="text-text-secondary mb-8 max-w-md mx-auto">Click "Start Transfer" to begin migrating 1,234 tracks to your destination service(s).</p>
                  
                  <button @click="startTransfer" class="px-8 py-4 bg-primary hover:bg-primary-hover text-white rounded-xl text-lg font-semibold transition-colors shadow-lg shadow-primary/30">
                    <span class="flex items-center gap-3">
                      <span class="material-symbols-outlined text-[24px]">play_arrow</span>
                      Start Transfer
                    </span>
                  </button>
                </div>
                
                <!-- Transfer In Progress -->
                <div v-else-if="!transferComplete" class="transfer-progress">
                  <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Transferring...</h2>
                  <p class="text-text-secondary mb-6">Please keep this window open until complete</p>
                  
                  <!-- Large Progress Bar -->
                  <div class="mb-6">
                    <div class="flex items-center justify-between mb-2">
                      <span class="text-sm font-medium text-gray-700 dark:text-gray-300">{{ transferProgress.current }} / {{ transferProgress.total }} tracks</span>
                      <span class="text-sm font-bold text-primary">{{ transferProgress.percent }}%</span>
                    </div>
                    <div class="h-4 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                      <div class="h-full bg-gradient-to-r from-primary to-blue-400 rounded-full transition-all duration-300 relative" :style="{ width: transferProgress.percent + '%' }">
                        <div class="absolute inset-0 bg-white/20 animate-shine"></div>
                      </div>
                    </div>
                    <div class="flex items-center justify-between mt-2 text-xs text-text-secondary">
                      <span>~{{ transferProgress.eta }} remaining</span>
                      <span>{{ transferProgress.speed }} tracks/sec</span>
                    </div>
                  </div>
                  
                  <!-- Current Action -->
                  <div class="flex items-center gap-3 mb-6 px-4 py-3 bg-primary/5 border border-primary/20 rounded-lg">
                    <span class="material-symbols-outlined text-primary animate-spin text-xl">sync</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300">{{ transferProgress.currentAction }}</span>
                  </div>
                  
                  <!-- Activity Log -->
                  <div class="activity-log mb-6">
                    <h4 class="text-sm font-semibold text-gray-900 dark:text-white mb-2">Activity Log</h4>
                    <div class="bg-gray-900 rounded-lg p-3 h-[160px] overflow-y-auto font-mono text-xs custom-scrollbar">
                      <div 
                        v-for="log in activityLog" 
                        :key="log.id"
                        class="log-entry flex items-start gap-2 py-1"
                      >
                        <span class="text-gray-500 shrink-0">{{ log.time }}</span>
                        <span :class="log.success ? 'text-green-400' : 'text-red-400'">{{ log.success ? '✓' : '✗' }}</span>
                        <span class="text-gray-300">{{ log.message }}</span>
                      </div>
                    </div>
                  </div>
                  
                  <!-- Status Summary -->
                  <div class="grid grid-cols-3 gap-4 mb-6">
                    <div class="text-center">
                      <p class="text-2xl font-bold text-success">{{ transferProgress.transferred }}</p>
                      <p class="text-xs text-text-secondary">Transferred</p>
                    </div>
                    <div class="text-center">
                      <p class="text-2xl font-bold text-error">{{ transferProgress.failed }}</p>
                      <p class="text-xs text-text-secondary">Failed</p>
                    </div>
                    <div class="text-center">
                      <p class="text-2xl font-bold text-gray-400">{{ transferProgress.skipped }}</p>
                      <p class="text-xs text-text-secondary">Skipped</p>
                    </div>
                  </div>
                  
                  <!-- Cancel Button -->
                  <div class="text-center">
                    <button @click="cancelTransfer" class="px-5 py-2.5 bg-error/10 border border-error/30 hover:bg-error/20 text-error rounded-lg text-sm font-medium transition-colors">
                      Cancel Transfer
                    </button>
                  </div>
                </div>
                
                <!-- Transfer Complete -->
                <div v-else class="transfer-complete text-center py-8">
                  <!-- Confetti Container -->
                  <div class="confetti-container absolute inset-0 pointer-events-none overflow-hidden">
                    <div v-for="i in 20" :key="i" :class="['confetti', `confetti-${i}`]"></div>
                  </div>
                  
                  <div class="w-24 h-24 mx-auto rounded-full bg-success/10 flex items-center justify-center mb-6 relative">
                    <span class="material-symbols-outlined text-5xl text-success">celebration</span>
                  </div>
                  <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Transfer complete! 🎉</h2>
                  <p class="text-text-secondary mb-8">Your music has been migrated successfully</p>
                  
                  <!-- Summary -->
                  <div class="bg-gray-50 dark:bg-surface-highlight/50 rounded-xl p-6 mb-8 text-left max-w-md mx-auto">
                    <div class="flex items-center justify-between py-2 border-b border-gray-200 dark:border-border-dark">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Successfully transferred</span>
                      <span class="text-sm font-bold text-success">1,217 tracks</span>
                    </div>
                    <div class="flex items-center justify-between py-2 border-b border-gray-200 dark:border-border-dark">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Failed</span>
                      <button class="text-sm font-bold text-error hover:underline">5 tracks (view details)</button>
                    </div>
                    <div class="flex items-center justify-between py-2">
                      <span class="text-sm text-gray-700 dark:text-gray-300">Skipped</span>
                      <span class="text-sm font-bold text-gray-500">12 tracks</span>
                    </div>
                  </div>
                  
                  <!-- Action Buttons -->
                  <div class="flex items-center justify-center gap-4">
                    <button class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
                      View Failed Tracks
                    </button>
                    <button @click="resetWizard" class="px-5 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
                      New Migration
                    </button>
                    <button class="px-5 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                      Done
                    </button>
                  </div>
                </div>
              </div>
              
            </Transition>
          </div>
          
          <!-- Wizard Footer / Navigation -->
          <div class="px-8 py-5 border-t border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 flex items-center justify-between">
            <button 
              v-if="currentStep > 0"
              @click="prevStep"
              class="flex items-center gap-2 px-5 py-2.5 text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white transition-colors"
            >
              <span class="material-symbols-outlined text-[20px]">arrow_back</span>
              Back
            </button>
            <div v-else></div>
            
            <button 
              v-if="currentStep < steps.length - 1"
              @click="nextStep"
              :disabled="!canProceed"
              :class="[
                'flex items-center gap-2 px-6 py-2.5 rounded-lg text-sm font-semibold transition-all',
                canProceed ? 'bg-primary hover:bg-primary-hover text-white shadow-lg shadow-primary/20' : 'bg-gray-200 dark:bg-gray-700 text-gray-500 cursor-not-allowed'
              ]"
            >
              Next
              <span class="material-symbols-outlined text-[20px]">arrow_forward</span>
            </button>
          </div>
        </div>
      </div>
      
      <!-- Info Panel (Collapsible) -->
      <div class="max-w-[900px] mx-auto mt-8">
        <button @click="showInfoPanel = !showInfoPanel" class="w-full flex items-center justify-between px-6 py-4 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors">
          <div class="flex items-center gap-3">
            <span class="material-symbols-outlined text-primary">help_outline</span>
            <span class="font-medium text-gray-900 dark:text-white">How migration works</span>
          </div>
          <span :class="['material-symbols-outlined text-gray-400 transition-transform', showInfoPanel ? 'rotate-180' : '']">expand_more</span>
        </button>
        
        <Transition name="expand">
          <div v-if="showInfoPanel" class="mt-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden">
            <div v-for="(item, index) in infoItems" :key="item.title" class="border-b border-gray-100 dark:border-border-dark/50 last:border-0">
              <button 
                @click="toggleInfoItem(index)"
                class="w-full flex items-center justify-between px-6 py-4 hover:bg-gray-50 dark:hover:bg-surface-highlight/50 transition-colors"
              >
                <span class="font-medium text-gray-700 dark:text-gray-300">{{ item.title }}</span>
                <span :class="['material-symbols-outlined text-gray-400 transition-transform', expandedInfo.includes(index) ? 'rotate-180' : '']">expand_more</span>
              </button>
              <div v-if="expandedInfo.includes(index)" class="px-6 pb-4 text-sm text-text-secondary">
                {{ item.content }}
              </div>
            </div>
          </div>
        </Transition>
      </div>
      
      <!-- Active Syncs Dashboard -->
      <div v-if="activeSyncs.length > 0" class="max-w-[900px] mx-auto mt-8">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-semibold text-gray-900 dark:text-white">Active Syncs</h2>
          <span class="text-sm text-text-secondary">{{ activeSyncs.length }} active</span>
        </div>
        
        <div class="grid grid-cols-2 gap-4">
          <div 
            v-for="sync in activeSyncs" 
            :key="sync.id"
            class="sync-card bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl p-4 hover:border-primary/30 transition-colors"
          >
            <div class="flex items-center gap-3 mb-3">
              <div class="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center text-lg">{{ sync.sourceIcon }}</div>
              <span class="material-symbols-outlined text-gray-400 text-lg">arrow_forward</span>
              <div class="w-8 h-8 rounded-lg bg-primary/10 flex items-center justify-center text-lg">{{ sync.destIcon }}</div>
              <div class="ml-2">
                <p class="text-sm font-medium text-gray-900 dark:text-white">{{ sync.source }} → {{ sync.dest }}</p>
                <p class="text-xs text-text-secondary">{{ sync.content }}</p>
              </div>
            </div>
            
            <div class="flex items-center gap-4 mb-3 text-xs">
              <div class="flex items-center gap-1.5">
                <span :class="['w-2 h-2 rounded-full', sync.status === 'active' ? 'bg-success animate-pulse' : sync.status === 'paused' ? 'bg-amber-500' : 'bg-error']"></span>
                <span :class="sync.status === 'active' ? 'text-success' : sync.status === 'paused' ? 'text-amber-500' : 'text-error'">{{ sync.status === 'active' ? 'Active' : sync.status === 'paused' ? 'Paused' : 'Error' }}</span>
              </div>
              <span class="text-text-secondary">Last: {{ sync.lastSync }}</span>
              <span class="text-text-secondary">Next: {{ sync.nextSync }}</span>
            </div>
            
            <div class="flex items-center gap-2">
              <button class="flex-1 px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors">
                Sync Now
              </button>
              <button v-if="sync.status === 'active'" class="px-3 py-1.5 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-xs font-medium transition-colors">
                Pause
              </button>
              <button v-else class="px-3 py-1.5 bg-success/10 hover:bg-success/20 text-success rounded-lg text-xs font-medium transition-colors">
                Resume
              </button>
              <button class="p-1.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors">
                <span class="material-symbols-outlined text-[18px]">settings</span>
              </button>
              <button class="p-1.5 text-gray-400 hover:text-error transition-colors">
                <span class="material-symbols-outlined text-[18px]">close</span>
              </button>
            </div>
          </div>
        </div>
      </div>
      
      <!-- Migration History -->
      <div class="migration-history max-w-[900px] mx-auto mt-8">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-semibold text-gray-900 dark:text-white">Migration History</h2>
          <button v-if="combinedHistory.length > 0" class="text-sm text-primary hover:text-primary-hover font-medium">View All</button>
        </div>
        
        <div class="bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden">
          <!-- Empty State -->
          <div v-if="combinedHistory.length === 0" class="empty-state flex flex-col items-center justify-center p-12 text-center" data-testid="migration-history-empty">
            <span class="material-symbols-outlined text-5xl text-gray-300 dark:text-gray-600 mb-3">history</span>
            <h3 class="text-base font-medium text-gray-700 dark:text-gray-300 mb-1">No migration history</h3>
            <p class="text-sm text-text-secondary">Completed and pending migrations will appear here</p>
          </div>

          <table v-else class="history-table w-full">
            <thead>
              <tr class="border-b border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30">
                <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide">Date</th>
                <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide">Migration</th>
                <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide">Content</th>
                <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide">Status</th>
                <th class="px-4 py-3 text-left text-xs font-semibold text-text-secondary uppercase tracking-wide">Success Rate</th>
                <th class="px-4 py-3 text-right text-xs font-semibold text-text-secondary uppercase tracking-wide">Actions</th>
              </tr>
            </thead>
            <tbody>
              <tr 
                v-for="mig in combinedHistory" 
                :key="mig.id"
                class="border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30 transition-colors"
              >
                <td class="px-4 py-3 text-sm text-gray-700 dark:text-gray-300">{{ mig.date }}</td>
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <span class="text-lg">{{ getServiceIcon(mig.source) }}</span>
                    <span class="material-symbols-outlined text-gray-400 text-sm">arrow_forward</span>
                    <span class="text-lg">{{ getServiceIcon(mig.dest) }}</span>
                    <span class="text-sm text-gray-700 dark:text-gray-300">{{ mig.source }} → {{ mig.dest }}</span>
                  </div>
                </td>
                <td class="px-4 py-3 text-sm text-gray-700 dark:text-gray-300">{{ mig.totalCount }} tracks</td>
                <td class="px-4 py-3">
                  <span :class="[
                    'px-2 py-1 rounded-full text-xs font-medium',
                    mig.status === 'completed' ? 'bg-success/10 text-success' :
                    mig.status === 'partial' ? 'bg-amber-500/10 text-amber-500' :
                    'bg-error/10 text-error'
                  ]">
                    {{ mig.status === 'completed' ? 'Completed' : mig.status === 'partial' ? 'Partial' : 'Failed' }}
                  </span>
                </td>
                <td class="px-4 py-3 text-sm text-gray-700 dark:text-gray-300">
                  <span :class="mig.successRate >= 95 ? 'text-success' : mig.successRate >= 80 ? 'text-amber-500' : 'text-error'">{{ mig.successRate }}%</span>
                  <span class="text-text-secondary ml-1">({{ mig.successCount }} / {{ mig.totalCount }})</span>
                </td>
                <td class="px-4 py-3">
                  <div class="flex items-center justify-end gap-1">
                    <button @click="openMigrationDetails(mig)" class="px-2.5 py-1 text-xs text-primary hover:bg-primary/10 rounded transition-colors">Details</button>
                    <button @click="handleRetryMigration(String(mig.id))" class="px-2.5 py-1 text-xs text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded transition-colors">Re-run</button>
                    <button @click="handleDeleteMigration(String(mig.id))" class="p-1 text-gray-400 hover:text-error transition-colors">
                      <span class="material-symbols-outlined text-[16px]">delete</span>
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
          
          <!-- Load More -->
          <div v-if="combinedHistory.length > 0" class="px-4 py-3 border-t border-gray-200 dark:border-border-dark text-center">
            <button class="text-sm text-primary hover:text-primary-hover font-medium">Load More</button>
          </div>
        </div>
      </div>
      
      <!-- Saved Templates Section -->
      <div class="saved-templates max-w-[900px] mx-auto mt-8 mb-8">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-semibold text-gray-900 dark:text-white">Saved Templates</h2>
          <button @click="showSaveTemplateModal = true" class="flex items-center gap-2 text-sm text-primary hover:text-primary-hover font-medium">
            <span class="material-symbols-outlined text-[16px]">add</span>
            Create Template
          </button>
        </div>
        
        <div class="grid grid-cols-3 gap-4">
          <div 
            v-for="template in combinedTemplates" 
            :key="template.id"
            class="template-item bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl p-4 hover:border-primary/30 transition-colors"
          >
            <div class="flex items-start justify-between mb-3">
              <div>
                <h3 class="font-medium text-gray-900 dark:text-white text-sm">{{ template.name }}</h3>
                <p class="text-xs text-text-secondary mt-0.5">Last used: {{ template.lastUsed }}</p>
              </div>
              <div class="flex gap-1">
                <button class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors">
                  <span class="material-symbols-outlined text-[16px]">edit</span>
                </button>
                <button @click="handleDeleteTemplate(template.id)" class="p-1 text-gray-400 hover:text-error transition-colors">
                  <span class="material-symbols-outlined text-[16px]">delete</span>
                </button>
              </div>
            </div>
            
            <div class="flex items-center gap-2 mb-3">
              <span class="text-lg">{{ template.sourceIcon }}</span>
              <span class="material-symbols-outlined text-gray-400 text-sm">arrow_forward</span>
              <span class="text-lg">{{ template.destIcon }}</span>
              <span class="text-xs text-text-secondary ml-1">{{ template.content }}</span>
            </div>
            
            <button @click="useTemplate(template)" class="w-full px-3 py-2 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors">
              Use Template
            </button>
          </div>
        </div>
      </div>
      
    </div>
    
    <!-- Migration Details Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showDetailsModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showDetailsModal = false">
          <div class="details-modal bg-white dark:bg-surface-dark rounded-2xl w-full max-w-2xl max-h-[90vh] overflow-hidden shadow-2xl">
            <!-- Modal Header -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <div class="flex items-center gap-3">
                <span class="text-2xl">{{ selectedMigration?.sourceIcon }}</span>
                <span class="material-symbols-outlined text-gray-400">arrow_forward</span>
                <span class="text-2xl">{{ selectedMigration?.destIcon }}</span>
                <div class="ml-2">
                  <h3 class="font-semibold text-gray-900 dark:text-white">{{ selectedMigration?.source }} → {{ selectedMigration?.dest }}</h3>
                  <p class="text-xs text-text-secondary">{{ selectedMigration?.date }}</p>
                </div>
              </div>
              <button @click="showDetailsModal = false" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>
            
            <!-- Modal Content -->
            <div class="p-6 overflow-y-auto max-h-[60vh]">
              <!-- Summary Stats -->
              <div class="grid grid-cols-4 gap-4 mb-6">
                <div class="text-center p-4 bg-gray-50 dark:bg-surface-highlight/50 rounded-xl">
                  <p class="text-2xl font-bold text-gray-900 dark:text-white">{{ selectedMigration?.totalCount }}</p>
                  <p class="text-xs text-text-secondary">Total Items</p>
                </div>
                <div class="text-center p-4 bg-success/5 rounded-xl">
                  <p class="text-2xl font-bold text-success">{{ selectedMigration?.successCount }}</p>
                  <p class="text-xs text-text-secondary">Successful</p>
                </div>
                <div class="text-center p-4 bg-error/5 rounded-xl">
                  <p class="text-2xl font-bold text-error">{{ selectedMigration?.failedCount }}</p>
                  <p class="text-xs text-text-secondary">Failed</p>
                </div>
                <div class="text-center p-4 bg-gray-50 dark:bg-surface-highlight/50 rounded-xl">
                  <p class="text-2xl font-bold text-gray-500">{{ selectedMigration?.skippedCount }}</p>
                  <p class="text-xs text-text-secondary">Skipped</p>
                </div>
              </div>
              
              <!-- Failed Tracks -->
              <div v-if="selectedMigration?.failedCount > 0" class="mb-4">
                <button @click="showFailedTracks = !showFailedTracks" class="w-full flex items-center justify-between px-4 py-3 bg-error/5 border border-error/20 rounded-lg hover:bg-error/10 transition-colors">
                  <span class="font-medium text-error">Failed Tracks ({{ selectedMigration?.failedCount }})</span>
                  <span :class="['material-symbols-outlined text-error transition-transform', showFailedTracks ? 'rotate-180' : '']">expand_more</span>
                </button>
                <div v-if="showFailedTracks" class="mt-2 border border-gray-200 dark:border-border-dark rounded-lg overflow-hidden">
                  <div v-for="track in failedTracks" :key="track.id" class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0 flex items-center justify-between">
                    <div>
                      <p class="text-sm font-medium text-gray-900 dark:text-white">{{ track.title }}</p>
                      <p class="text-xs text-text-secondary">{{ track.artist }}</p>
                      <p class="text-xs text-error mt-1">{{ track.reason }}</p>
                    </div>
                    <button class="px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors">
                      Retry
                    </button>
                  </div>
                </div>
              </div>
              
              <!-- Skipped Tracks -->
              <div v-if="selectedMigration?.skippedCount > 0">
                <button @click="showSkippedTracks = !showSkippedTracks" class="w-full flex items-center justify-between px-4 py-3 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors">
                  <span class="font-medium text-gray-700 dark:text-gray-300">Skipped Tracks ({{ selectedMigration?.skippedCount }})</span>
                  <span :class="['material-symbols-outlined text-gray-400 transition-transform', showSkippedTracks ? 'rotate-180' : '']">expand_more</span>
                </button>
                <div v-if="showSkippedTracks" class="mt-2 border border-gray-200 dark:border-border-dark rounded-lg overflow-hidden">
                  <div v-for="track in skippedTracks" :key="track.id" class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0">
                    <p class="text-sm font-medium text-gray-900 dark:text-white">{{ track.title }}</p>
                    <p class="text-xs text-text-secondary">{{ track.artist }}</p>
                    <p class="text-xs text-amber-500 mt-1">{{ track.reason }}</p>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- Modal Footer -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-between">
              <button class="flex items-center gap-2 px-4 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
                <span class="material-symbols-outlined text-[18px]">download</span>
                Export Report
              </button>
              <button @click="showDetailsModal = false" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                Close
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
    
    <!-- Save Template Modal -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showSaveTemplateModal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="showSaveTemplateModal = false">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-md shadow-2xl">
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
              <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Save as Template</h3>
            </div>
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Template Name</label>
                <input v-model="newTemplateName" type="text" placeholder="My Migration Template" class="w-full px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50">
              </div>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="templateIncludeDestination" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-700 dark:text-gray-300">Include destination services</span>
              </label>
              <label class="flex items-center gap-3 cursor-pointer">
                <input type="checkbox" v-model="templateIncludeSync" class="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary">
                <span class="text-sm text-gray-700 dark:text-gray-300">Include sync settings</span>
              </label>
            </div>
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3">
              <button @click="showSaveTemplateModal = false" class="px-5 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium transition-colors">
                Cancel
              </button>
              <button @click="saveTemplate" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                Save Template
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useMigration } from '../composables/useMigration'

// Initialize migration composable for backend integration
const migration = useMigration()

// Load history and templates on mount, setup event listener
onMounted(async () => {
  await migration.loadHistory()
  await migration.loadTemplates()
  await migration.setupProgressListener()
})

// Cleanup event listener on unmount
onUnmounted(() => {
  migration.cleanup()
})

// Watch migration progress to update local state
watch(() => migration.progress.status, (newStatus) => {
  if (newStatus === 'completed') {
    transferComplete.value = true
  } else if (newStatus === 'cancelled') {
    transferStarted.value = false
  }
})

// Computed: Real progress from backend 
const realProgress = computed(() => {
  const p = migration.progress
  const percent = p.total_items > 0 ? Math.round((p.current_item / p.total_items) * 100) : 0
  return {
    current: p.current_item,
    total: p.total_items,
    percent,
    transferred: p.completed_count,
    failed: p.failed_count,
    skipped: p.skipped_count,
    currentAction: p.current_track || 'Processing...',
    eta: 'calculating...',
    speed: '---'
  }
})

// Computed: Migration history from backend
const backendHistory = computed(() => migration.history.value.map(migration.formatHistoryItem))

// Computed: Combined history directly from backend (no mock fallback)
const combinedHistory = computed(() => backendHistory.value)

// Computed: Templates from backend  
const backendTemplates = computed(() => migration.templates.value)

// Helper: Get service icon by name
function getServiceIcon(serviceName: string): string {
  const iconMap: Record<string, string> = {
    'spotify': '🎵',
    'qobuz': '🎧', 
    'tidal': '🌊',
    'deezer': '🎶',
    'soundcloud': '☁️',
    'apple': '🍎'
  }
  return iconMap[serviceName.toLowerCase()] || '🎵'
}

// Handler: Open migration details modal
function openMigrationDetails(mig: any) {
  selectedMigration.value = mig
  showDetailsModal.value = true
  // Load real details from backend if available
  if (mig.id && typeof mig.id === 'string') {
    migration.loadJobDetails(mig.id)
  }
}

// Handler: Delete migration job  
async function handleDeleteMigration(jobId: string) {
  if (confirm('Are you sure you want to delete this migration?')) {
    await migration.deleteJob(jobId)
  }
}

// Handler: Retry failed items
async function handleRetryMigration(jobId: string) {
  const count = await migration.retryFailed(jobId)
  console.log(`Retried ${count} failed items`)
}

// Wizard state
const currentStep = ref(0)
const stepDirection = ref('slide-left')

const steps = [
  { id: 'source', label: 'Source' },
  { id: 'content', label: 'Content' },
  { id: 'destination', label: 'Destination' },
  { id: 'preview', label: 'Preview' },
  { id: 'transfer', label: 'Transfer' },
]

const progressWidth = computed(() => {
  return `${(currentStep.value / (steps.length - 1)) * 100}%`
})

// Services
const services = [
  { id: 'spotify', name: 'Spotify', icon: '🎵', bgClass: 'bg-[#1ed760]/10', connected: true },
  { id: 'qobuz', name: 'Qobuz', icon: '🎧', bgClass: 'bg-[#1a8fe3]/10', connected: true },
  { id: 'tidal', name: 'Tidal', icon: '🌊', bgClass: 'bg-[#00d4aa]/10', connected: true },
  { id: 'deezer', name: 'Deezer', icon: '🎶', bgClass: 'bg-[#ff0092]/10', connected: true },
  { id: 'soundcloud', name: 'SoundCloud', icon: '☁️', bgClass: 'bg-[#ff5500]/10', connected: false },
  { id: 'apple', name: 'Apple Music', icon: '🍎', bgClass: 'bg-[#fa243c]/10', connected: false },
]

const sourceService = ref<string>('')
const destinationServices = ref<string[]>([])

// Content types
const contentTypes = [
  { id: 'favorites', label: 'Favorites', icon: 'favorite', count: '1,234 tracks' },
  { id: 'playlists', label: 'Playlists', icon: 'queue_music', count: '23 playlists' },
  { id: 'albums', label: 'Saved Albums', icon: 'album', count: '156 albums' },
  { id: 'artists', label: 'Followed Artists', icon: 'person', count: '89 artists' },
]

const selectedContent = ref<string[]>([])

// Navigation
const canProceed = computed(() => {
  switch (currentStep.value) {
    case 0: return sourceService.value !== ''
    case 1: return selectedContent.value.length > 0
    case 2: return destinationServices.value.length > 0
    default: return true
  }
})

function nextStep() {
  if (currentStep.value < steps.length - 1 && canProceed.value) {
    stepDirection.value = 'slide-left'
    currentStep.value++
  }
}

function prevStep() {
  if (currentStep.value > 0) {
    stepDirection.value = 'slide-right'
    currentStep.value--
  }
}

function toggleContentType(id: string) {
  const idx = selectedContent.value.indexOf(id)
  if (idx >= 0) {
    selectedContent.value.splice(idx, 1)
  } else {
    selectedContent.value.push(id)
  }
}

function toggleDestination(id: string) {
  if (id === sourceService.value) return
  const idx = destinationServices.value.indexOf(id)
  if (idx >= 0) {
    destinationServices.value.splice(idx, 1)
  } else {
    destinationServices.value.push(id)
  }
}

function getService(id: string) {
  return services.find(s => s.id === id)
}

// Quick actions
const showRecentDropdown = ref(false)
const recentMigrations = [
  { id: 1, source: 'Spotify', destination: 'Qobuz', date: '2 days ago' },
  { id: 2, source: 'Tidal', destination: 'Qobuz', date: '1 week ago' },
]

// Info panel
const showInfoPanel = ref(false)
const expandedInfo = ref<number[]>([])

const infoItems = [
  { title: 'Matching process', content: 'Syncify uses ISRC codes (International Standard Recording Code) for precise track matching. When ISRC is unavailable, we use fuzzy matching on title, artist, and album. Each match is assigned a confidence score from 0-100%.' },
  { title: 'What happens to originals', content: 'Your source service library is never modified. Migration only adds content to your destination services. Original favorites, playlists, and follows remain untouched.' },
  { title: 'Handling duplicates', content: 'If a track already exists in your destination library, it will be skipped. Playlists will only add tracks that are not already in the playlist.' },
  { title: 'Sync vs one-time transfer', content: 'One-time transfer migrates your current library state. Scheduled sync continuously monitors your source library and automatically transfers new additions to destinations.' },
]

function toggleInfoItem(index: number) {
  const idx = expandedInfo.value.indexOf(index)
  if (idx >= 0) {
    expandedInfo.value.splice(idx, 1)
  } else {
    expandedInfo.value.push(index)
  }
}

// Match preview state
const matchFilter = ref('all')
const skipNotFound = ref(false)
const showManualMatchModal = ref(false)
const manualMatchTrack = ref<any>(null)

const matchFilters = [
  { id: 'all', label: 'All', count: '1,234' },
  { id: 'high', label: 'High Confidence', count: '1,198' },
  { id: 'review', label: 'Needs Review', count: '31' },
  { id: 'notfound', label: 'Not Found', count: '5' },
]

// Mock match data
const mockMatches = ref([
  { id: 1, sourceTitle: 'Bohemian Rhapsody', sourceArtist: 'Queen', sourceGradient: 'bg-gradient-to-br from-purple-500 to-pink-500', sourceService: 'Spotify', sourceServiceClass: 'bg-[#1ed760]/10 text-[#1ed760]', confidence: '100%', confidenceClass: 'bg-success/10 text-success', found: true, destTitle: 'Bohemian Rhapsody', destArtist: 'Queen', destGradient: 'bg-gradient-to-br from-blue-500 to-cyan-500', destService: 'Qobuz', destServiceClass: 'bg-[#1a8fe3]/10 text-[#1a8fe3]', quality: '24/96', type: 'high' },
  { id: 2, sourceTitle: 'Blinding Lights', sourceArtist: 'The Weeknd', sourceGradient: 'bg-gradient-to-br from-red-500 to-orange-500', sourceService: 'Spotify', sourceServiceClass: 'bg-[#1ed760]/10 text-[#1ed760]', confidence: '100%', confidenceClass: 'bg-success/10 text-success', found: true, destTitle: 'Blinding Lights', destArtist: 'The Weeknd', destGradient: 'bg-gradient-to-br from-blue-500 to-cyan-500', destService: 'Qobuz', destServiceClass: 'bg-[#1a8fe3]/10 text-[#1a8fe3]', quality: '16/44', type: 'high' },
  { id: 3, sourceTitle: 'Nightcall', sourceArtist: 'Kavinsky', sourceGradient: 'bg-gradient-to-br from-indigo-500 to-purple-500', sourceService: 'Spotify', sourceServiceClass: 'bg-[#1ed760]/10 text-[#1ed760]', confidence: '95%', confidenceClass: 'bg-success/10 text-success', found: true, destTitle: 'Nightcall', destArtist: 'Kavinsky', destGradient: 'bg-gradient-to-br from-blue-500 to-cyan-500', destService: 'Qobuz', destServiceClass: 'bg-[#1a8fe3]/10 text-[#1a8fe3]', quality: 'FLAC', type: 'high' },
  { id: 4, sourceTitle: 'A Real Hero (Drive OST)', sourceArtist: 'College', sourceGradient: 'bg-gradient-to-br from-pink-500 to-rose-500', sourceService: 'Spotify', sourceServiceClass: 'bg-[#1ed760]/10 text-[#1ed760]', confidence: '78%', confidenceClass: 'bg-amber-500/10 text-amber-500', found: true, destTitle: 'A Real Hero', destArtist: 'College feat. Electric Youth', destGradient: 'bg-gradient-to-br from-blue-500 to-cyan-500', destService: 'Qobuz', destServiceClass: 'bg-[#1a8fe3]/10 text-[#1a8fe3]', quality: '16/44', type: 'review' },
  { id: 5, sourceTitle: 'Turbo Killer', sourceArtist: 'Carpenter Brut', sourceGradient: 'bg-gradient-to-br from-red-600 to-orange-600', sourceService: 'Spotify', sourceServiceClass: 'bg-[#1ed760]/10 text-[#1ed760]', confidence: '65%', confidenceClass: 'bg-amber-500/10 text-amber-500', found: true, destTitle: 'Turbo Killer (Live)', destArtist: 'Carpenter Brut', destGradient: 'bg-gradient-to-br from-blue-500 to-cyan-500', destService: 'Qobuz', destServiceClass: 'bg-[#1a8fe3]/10 text-[#1a8fe3]', quality: 'FLAC', type: 'review' },
  { id: 6, sourceTitle: 'Obscure Indie Track', sourceArtist: 'Unknown Artist', sourceGradient: 'bg-gradient-to-br from-gray-500 to-gray-600', sourceService: 'Spotify', sourceServiceClass: 'bg-[#1ed760]/10 text-[#1ed760]', confidence: 'Not Found', confidenceClass: 'bg-error/10 text-error', found: false, destTitle: '', destArtist: '', destGradient: '', destService: '', destServiceClass: '', quality: '', type: 'notfound' },
])

const filteredMatches = computed(() => {
  if (matchFilter.value === 'all') return mockMatches.value
  return mockMatches.value.filter(m => m.type === matchFilter.value)
})

// Transfer state
const transferStarted = ref(false)
const transferComplete = ref(false)

const transferProgress = ref({
  current: 453,
  total: 1234,
  percent: 37,
  eta: '5 min',
  speed: '4.2',
  transferred: 448,
  failed: 3,
  skipped: 2,
  currentAction: "Adding 'Bohemian Rhapsody' to Qobuz favorites..."
})

// Activity log
const activityLog = ref([
  { id: 1, time: '12:34:52', success: true, message: "Added 'Blinding Lights' to Qobuz favorites" },
  { id: 2, time: '12:34:53', success: true, message: "Added 'Nightcall' to Qobuz favorites" },
  { id: 3, time: '12:34:54', success: false, message: "Failed to add 'Obscure Track' - not available" },
  { id: 4, time: '12:34:55', success: true, message: "Added 'Take On Me' to Qobuz favorites" },
  { id: 5, time: '12:34:56', success: true, message: "Added 'Dreams' to Qobuz favorites" },
  { id: 6, time: '12:34:57', success: true, message: "Added 'The Chain' to Qobuz favorites" },
  { id: 7, time: '12:34:58', success: true, message: "Added 'Bohemian Rhapsody' to Qobuz favorites" },
])

function startTransfer() {
  transferStarted.value = true
  // Start real migration via backend
  const destService = destinationServices.value[0] || 'qobuz'
  migration.start(sourceService.value, destService, undefined, migration.defaultOptions)
    .then((jobId) => {
      if (jobId) {
        console.log('Migration started:', jobId)
      }
    })
    .catch((e) => {
      console.error('Failed to start migration:', e)
      transferStarted.value = false
    })
}

function cancelTransfer() {
  migration.cancel().then(() => {
    transferStarted.value = false
  })
}

function resetWizard() {
  currentStep.value = 0
  sourceService.value = ''
  destinationServices.value = []
  selectedContent.value = []
  transferStarted.value = false
  transferComplete.value = false
}

// Active Syncs
const activeSyncs = ref([
  { id: 1, source: 'Spotify', sourceIcon: '🎵', dest: 'Qobuz', destIcon: '🎧', content: 'Favorites', status: 'active', lastSync: '2h ago', nextSync: 'in 4h' },
  { id: 2, source: 'Spotify', sourceIcon: '🎵', dest: 'Tidal', destIcon: '🌊', content: 'Playlists', status: 'paused', lastSync: '1d ago', nextSync: 'paused' },
])

// Details Modal
const showDetailsModal = ref(false)
const selectedMigration = ref<any>(null)
const showFailedTracks = ref(false)
const showSkippedTracks = ref(false)

const failedTracks = ref([
  { id: 1, title: 'Obscure Indie Track', artist: 'Unknown Band', reason: 'Not available on destination service' },
  { id: 2, title: 'Live Recording 2019', artist: 'Artist Name', reason: 'Match confidence too low (42%)' },
  { id: 3, title: 'Bootleg Remix', artist: 'DJ Name', reason: 'Not available on destination service' },
  { id: 4, title: 'Regional Release', artist: 'Local Artist', reason: 'Region-locked content' },
  { id: 5, title: 'Deleted Track', artist: 'Former Artist', reason: 'Track no longer exists' },
])

const skippedTracks = ref([
  { id: 1, title: 'Already In Library', artist: 'Popular Artist', reason: 'Already exists in destination library' },
  { id: 2, title: 'Duplicate Entry', artist: 'Some Artist', reason: 'Duplicate detection triggered' },
  { id: 3, title: 'Low Quality Source', artist: 'Artist Name', reason: 'Source quality lower than destination threshold' },
])

// Saved Templates
const savedTemplates = ref([
  { id: 1, name: 'Spotify → Qobuz (Favorites)', sourceIcon: '🎵', destIcon: '🎧', content: 'Favorites only', lastUsed: '2 days ago' },
  { id: 2, name: 'Full Library Sync', sourceIcon: '🎵', destIcon: '🎧', content: 'All content types', lastUsed: '1 week ago' },
  { id: 3, name: 'Playlist Migration', sourceIcon: '🎵', destIcon: '🌊', content: 'Playlists only', lastUsed: '2 weeks ago' },
])

const showSaveTemplateModal = ref(false)
const newTemplateName = ref('')
const templateIncludeDestination = ref(true)
const templateIncludeSync = ref(false)

function useTemplate(template: any) {
  // Pre-fill wizard with template settings
  sourceService.value = template.sourceIcon === '🎵' ? 'spotify' : template.sourceIcon === '🎧' ? 'qobuz' : 'tidal'
  if (template.content.includes('Favorites')) {
    selectedContent.value = ['favorites']
  }
  currentStep.value = 0
}

// Computed: Combined templates (backend data with fallback to mock)
const combinedTemplates = computed(() => {
  if (backendTemplates.value.length > 0) {
    // Transform backend templates to match UI structure
    return backendTemplates.value.map(t => ({
      id: t.id,
      name: t.name,
      sourceIcon: getServiceIcon(t.source_service),
      destIcon: getServiceIcon(t.destination_service),
      content: t.description || 'Migration template',
      lastUsed: new Date(t.updated_at).toLocaleDateString()
    }))
  }
  return savedTemplates.value
})

async function saveTemplate() {
  // Save current wizard state as template to backend
  const destService = destinationServices.value[0] || 'qobuz'
  await migration.saveTemplate(
    newTemplateName.value || 'New Template',
    `Source: ${sourceService.value}, Content: ${selectedContent.value.join(', ')}`,
    sourceService.value,
    destService,
    migration.defaultOptions
  )
  showSaveTemplateModal.value = false
  newTemplateName.value = ''
}

async function handleDeleteTemplate(templateId: number) {
  if (confirm('Are you sure you want to delete this template?')) {
    await migration.deleteTemplate(templateId)
  }
}
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

/* Slide transitions */
.slide-left-enter-active,
.slide-left-leave-active,
.slide-right-enter-active,
.slide-right-leave-active {
  transition: all 0.3s ease;
}

.slide-left-enter-from {
  opacity: 0;
  transform: translateX(30px);
}
.slide-left-leave-to {
  opacity: 0;
  transform: translateX(-30px);
}

.slide-right-enter-from {
  opacity: 0;
  transform: translateX(-30px);
}
.slide-right-leave-to {
  opacity: 0;
  transform: translateX(30px);
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
  max-height: 500px;
}

/* Service card hover effect */
.service-card:not(:disabled):hover {
  transform: translateY(-2px);
}

/* Content card hover effect */
.content-card:hover {
  transform: translateY(-2px);
}

/* Shine animation for progress bar */
@keyframes shine {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(200%); }
}

.animate-shine {
  animation: shine 2s ease-in-out infinite;
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.3), transparent);
}

/* Confetti animation */
.confetti-container {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.confetti {
  position: absolute;
  width: 10px;
  height: 10px;
  top: -10px;
  animation: confetti-fall 3s ease-out forwards;
}

@keyframes confetti-fall {
  0% {
    transform: translateY(0) rotate(0deg);
    opacity: 1;
  }
  100% {
    transform: translateY(400px) rotate(720deg);
    opacity: 0;
  }
}

.confetti-1 { left: 5%; background: #3b82f6; animation-delay: 0s; }
.confetti-2 { left: 10%; background: #10b981; animation-delay: 0.1s; }
.confetti-3 { left: 15%; background: #f59e0b; animation-delay: 0.2s; }
.confetti-4 { left: 20%; background: #ef4444; animation-delay: 0.3s; }
.confetti-5 { left: 25%; background: #8b5cf6; animation-delay: 0.4s; }
.confetti-6 { left: 30%; background: #ec4899; animation-delay: 0.5s; }
.confetti-7 { left: 35%; background: #3b82f6; animation-delay: 0.1s; }
.confetti-8 { left: 40%; background: #10b981; animation-delay: 0.2s; }
.confetti-9 { left: 45%; background: #f59e0b; animation-delay: 0.3s; }
.confetti-10 { left: 50%; background: #ef4444; animation-delay: 0.4s; }
.confetti-11 { left: 55%; background: #8b5cf6; animation-delay: 0s; }
.confetti-12 { left: 60%; background: #ec4899; animation-delay: 0.1s; }
.confetti-13 { left: 65%; background: #3b82f6; animation-delay: 0.2s; }
.confetti-14 { left: 70%; background: #10b981; animation-delay: 0.3s; }
.confetti-15 { left: 75%; background: #f59e0b; animation-delay: 0.4s; }
.confetti-16 { left: 80%; background: #ef4444; animation-delay: 0.5s; }
.confetti-17 { left: 85%; background: #8b5cf6; animation-delay: 0s; }
.confetti-18 { left: 90%; background: #ec4899; animation-delay: 0.1s; }
.confetti-19 { left: 92%; background: #3b82f6; animation-delay: 0.2s; }
.confetti-20 { left: 95%; background: #10b981; animation-delay: 0.3s; }

/* Activity log styling */
.activity-log .custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(128, 128, 128, 0.5);
}
</style>
