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
            <template v-if="recentMigrations.length > 0">
              <button
                v-for="item in recentMigrations"
                :key="item.id"
                @click="openMigrationDetails(item)"
                class="w-full px-4 py-2.5 text-left text-sm hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors flex items-center gap-3"
              >
                <span class="text-gray-700 dark:text-gray-300">{{ item.source }} → {{ item.dest }}</span>
                <span class="ml-auto text-xs text-text-secondary">{{ item.date }}</span>
              </button>
            </template>
            <div v-else class="px-4 py-2.5 text-sm text-text-secondary">No migrations yet</div>
          </div>
        </div>

        <button @click="scrollToTemplates" class="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors">
          <span class="material-symbols-outlined text-[18px]">bookmark</span>
          Saved templates
        </button>

        <button
          @click="runSchemaAudit"
          :disabled="migration.isRunningAudit.value"
          class="flex items-center gap-2 px-4 py-2.5 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-lg text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-surface-highlight transition-colors disabled:opacity-50"
        >
          <span :class="['material-symbols-outlined text-[18px]', migration.isRunningAudit.value && 'animate-spin']">{{ migration.isRunningAudit.value ? 'progress_activity' : 'fact_check' }}</span>
          Schema audit
        </button>
      </div>

      <!-- Migration schema audit result (IN-5) -->
      <div v-if="migration.migrationAudit.value" class="mt-4 rounded-lg border p-4 text-sm" :class="migration.migrationAudit.value.schema_ok ? 'border-green-500/30 bg-green-500/10 text-green-600 dark:text-green-400' : 'border-red-500/30 bg-red-500/10 text-red-600 dark:text-red-400'">
        <div class="flex items-center gap-2 font-semibold">
          <span class="material-symbols-outlined text-[18px]">{{ migration.migrationAudit.value.schema_ok ? 'check_circle' : 'error' }}</span>
          {{ migration.migrationAudit.value.summary }}
        </div>
        <div v-if="migration.migrationAudit.value.missing_tables.length > 0" class="mt-2 text-xs">
          Missing tables: {{ migration.migrationAudit.value.missing_tables.join(', ') }}
        </div>
        <div v-if="migration.migrationAudit.value.legacy_services_detected.length > 0" class="mt-1 text-xs">
          Legacy services detected: {{ migration.migrationAudit.value.legacy_services_detected.join(', ') }}
        </div>
      </div>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto custom-scrollbar px-8 pb-8">

      <!-- Transfer Wizard Card -->
      <div class="transfer-wizard max-w-[900px] mx-auto">
        <div class="bg-white dark:bg-surface-dark rounded-2xl border border-gray-200 dark:border-border-dark shadow-lg overflow-hidden">

          <!-- Assistant Mode Switch: direct service → service migration or the transfer wizard -->
          <div class="assistant-mode px-8 pt-6 pb-1">
            <div class="inline-flex rounded-lg border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-1">
              <button
                data-testid="mode-service-to-service"
                @click="setAssistantMode('service')"
                :disabled="migration.isStartingMigration.value || transferStarted || svcStarted"
                :class="[
                  'px-4 py-2 rounded-md text-sm font-medium transition-all disabled:opacity-50',
                  assistantMode === 'service' ? 'bg-primary text-white shadow' : 'text-gray-700 dark:text-gray-300 hover:bg-white dark:hover:bg-surface-dark'
                ]"
              >
                <span class="flex items-center gap-2">
                  <span class="material-symbols-outlined text-[16px]">sync_alt</span>
                  Service → Service
                </span>
              </button>
              <button
                data-testid="mode-transfer"
                @click="setAssistantMode('transfer')"
                :disabled="migration.isStartingMigration.value || transferStarted || svcStarted"
                :class="[
                  'px-4 py-2 rounded-md text-sm font-medium transition-all disabled:opacity-50',
                  assistantMode === 'transfer' ? 'bg-primary text-white shadow' : 'text-gray-700 dark:text-gray-300 hover:bg-white dark:hover:bg-surface-dark'
                ]"
              >
                <span class="flex items-center gap-2">
                  <span class="material-symbols-outlined text-[16px]">queue_music</span>
                  Transfer Wizard
                </span>
              </button>
            </div>
            <p class="text-xs text-text-secondary mt-2">{{ assistantModeDescription }}</p>
          </div>

          <!-- Step Progress Indicator -->
          <div class="step-progress px-8 py-6 border-b border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30">
            <div class="flex items-center justify-between relative">
              <!-- Progress Line Background -->
              <div class="absolute top-5 left-0 right-0 h-0.5 bg-gray-200 dark:bg-gray-700"></div>
              <!-- Progress Line Fill -->
              <div class="absolute top-5 left-0 h-0.5 bg-primary transition-all duration-500" :style="{ width: progressWidth }"></div>

              <!-- Step Dots -->
              <div v-for="(step, index) in activeSteps" :key="step.id" class="relative z-10 flex flex-col items-center">
                <div
                  :class="[
                    'w-10 h-10 rounded-full flex items-center justify-center text-sm font-bold transition-all duration-300',
                    activeStepIndex > index ? 'bg-success text-white' :
                    activeStepIndex === index ? 'bg-primary text-white ring-4 ring-primary/20' :
                    'bg-gray-200 dark:bg-gray-700 text-gray-500 dark:text-gray-400'
                  ]"
                >
                  <span v-if="activeStepIndex > index" class="material-symbols-outlined text-[20px]">check</span>
                  <span v-else>{{ index + 1 }}</span>
                </div>
                <p :class="['text-xs font-medium mt-2 text-center', activeStepIndex === index ? 'text-primary' : 'text-text-secondary']">{{ step.label }}</p>
              </div>
            </div>
          </div>

          <!-- Wizard Step Content -->
          <div class="wizard-content p-8">
            <Transition :name="stepDirection" mode="out-in">

              <!-- SERVICE → SERVICE MODE / STEP 1: Source -->
              <div v-if="assistantMode === 'service' && svcStep === 0" key="svc-source" class="wizard-step" data-testid="svc-source-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Select source service</h2>
                <p class="text-text-secondary mb-8">Your saved tracks from this service are migrated to the destination you pick next</p>

                <div class="service-grid grid grid-cols-3 gap-4">
                  <button
                    v-for="service in sourceServices"
                    :key="service.id"
                    :data-service-id="service.id"
                    @click="selectSvcSource(service.id)"
                    :disabled="!isConnected(service.id)"
                    :class="[
                      'service-card relative p-6 rounded-xl border-2 transition-all text-center',
                      !isConnected(service.id) ? 'opacity-50 cursor-not-allowed border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30' :
                      svcSource === service.id ? 'border-primary bg-primary/5 shadow-lg shadow-primary/10' :
                      'border-gray-200 dark:border-border-dark hover:border-primary/50 hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
                    ]"
                  >
                    <div :class="['w-20 h-20 mx-auto rounded-2xl flex items-center justify-center mb-4 text-3xl', service.bgClass]">
                      {{ service.icon }}
                    </div>
                    <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">{{ service.name }}</h3>
                    <div class="flex items-center justify-center gap-1.5">
                      <span :class="['w-2 h-2 rounded-full', isConnected(service.id) ? 'bg-success' : 'bg-gray-400']"></span>
                      <span :class="['text-xs font-medium', isConnected(service.id) ? 'text-success' : 'text-text-secondary']">
                        {{ isConnected(service.id) ? 'Connected' : 'Not Connected' }}
                      </span>
                    </div>
                    <!-- Selected Indicator -->
                    <div v-if="svcSource === service.id" class="absolute top-3 right-3 w-6 h-6 rounded-full bg-primary text-white flex items-center justify-center">
                      <span class="material-symbols-outlined text-[16px]">check</span>
                    </div>
                  </button>
                </div>

                <label v-if="svcSource && accountsForService(svcSource).length > 1" class="block mt-4 text-sm">
                  Cuenta origen
                  <select v-model.number="svcSourceAccount" class="block w-full mt-1 rounded border p-2 bg-white dark:bg-surface-dark" data-testid="svc-source-account">
                    <option v-for="a in accountsForService(svcSource)" :key="a.id" :value="a.id">{{ a.display_name || a.email || `Cuenta ${a.id}` }} {{ a.is_active ? '(activa)' : '' }}</option>
                  </select>
                </label>
                <div v-if="sourceServices.length === 0" class="rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-8 text-center">
                  <span class="material-symbols-outlined text-4xl text-gray-400 mb-2">cloud_off</span>
                  <p class="text-sm text-text-secondary">No services available. Connect a music service in the Accounts tab first.</p>
                </div>
              </div>

              <!-- SERVICE → SERVICE MODE / STEP 2: Destination -->
              <div v-else-if="assistantMode === 'service' && svcStep === 1" key="svc-destination" class="wizard-step" data-testid="svc-destination-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Transfer to...</h2>
                <p class="text-text-secondary mb-8">Pick one destination service — the migration engine adds your saved tracks there</p>

                <div class="service-grid grid grid-cols-3 gap-4">
                  <button
                    v-for="service in destinationChoices"
                    :key="service.id"
                    :data-service-id="service.id"
                    @click="selectSvcDestination(service.id)"
                    :disabled="!isConnected(service.id) || (service.id === svcSource && (!selectedAccountExists(service.id, svcSourceAccount) || accountsForService(service.id).length <= 1))"
                    :class="[
                      'service-card relative p-6 rounded-xl border-2 transition-all text-center',
                      service.id === svcSource && (!selectedAccountExists(service.id, svcSourceAccount) || accountsForService(service.id).length <= 1) ? 'opacity-30 cursor-not-allowed border-gray-200 dark:border-border-dark' :
                      !isConnected(service.id) ? 'opacity-50 cursor-not-allowed border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30' :
                      svcDestination === service.id ? 'border-primary bg-primary/5 shadow-lg shadow-primary/10' :
                      'border-gray-200 dark:border-border-dark hover:border-primary/50 hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
                    ]"
                  >
                    <div :class="['w-20 h-20 mx-auto rounded-2xl flex items-center justify-center mb-4 text-3xl', service.bgClass]">
                      {{ service.icon }}
                    </div>
                    <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">{{ service.name }}</h3>
                    <div class="flex items-center justify-center gap-1.5">
                      <span v-if="service.id === svcSource" class="text-xs text-amber-500 font-medium">Source</span>
                      <template v-else>
                        <span :class="['w-2 h-2 rounded-full', isConnected(service.id) ? 'bg-success' : 'bg-gray-400']"></span>
                        <span :class="['text-xs font-medium', isConnected(service.id) ? 'text-success' : 'text-text-secondary']">
                          {{ isConnected(service.id) ? 'Connected' : 'Not Connected' }}
                        </span>
                      </template>
                    </div>
                    <!-- Selected Indicator -->
                    <div v-if="svcDestination === service.id" class="absolute top-3 right-3 w-6 h-6 rounded-full bg-primary text-white flex items-center justify-center">
                      <span class="material-symbols-outlined text-[16px]">check</span>
                    </div>
                  </button>
                </div>

                <label v-if="svcDestination && accountsForService(svcDestination).length > 1" class="block mt-4 text-sm">
                  Cuenta destino
                  <select v-model.number="svcDestinationAccount" class="block w-full mt-1 rounded border p-2 bg-white dark:bg-surface-dark" data-testid="svc-destination-account">
                    <option v-for="a in accountsForService(svcDestination)" :key="a.id" :value="a.id">{{ a.display_name || a.email || `Cuenta ${a.id}` }} {{ a.is_active ? '(activa)' : '' }}</option>
                  </select>
                </label>
                <div v-if="destinationChoices.length === 0" class="rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-8 text-center">
                  <span class="material-symbols-outlined text-4xl text-gray-400 mb-2">cloud_off</span>
                  <p class="text-sm text-text-secondary">No supported destination services available. The migration engine lists its destinations; connect one of them to continue.</p>
                </div>
              </div>

              <!-- SERVICE → SERVICE MODE / STEP 3: Real preview + reviewable matches -->
              <div v-else-if="assistantMode === 'service' && svcStep === 2" key="svc-review" class="wizard-step" data-testid="svc-review-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Review matches</h2>
                <p class="text-text-secondary mb-6">The preview really matches your {{ getServiceName(svcSource) }} tracks against {{ getServiceName(svcDestination) }} (ISRC and metadata search) without adding anything</p>

                <!-- Preview loading -->
                <div v-if="migration.isPreviewing.value" class="flex items-center justify-center gap-3 py-12 text-text-secondary">
                  <span class="material-symbols-outlined animate-spin text-primary text-2xl">sync</span>
                  Matching your library against {{ getServiceName(svcDestination) }}...
                </div>

                <!-- Preview error -->
                <div v-else-if="svcPreviewError" data-testid="svc-preview-error" class="rounded-xl border border-error/30 bg-error/5 p-6 text-center">
                  <span class="material-symbols-outlined text-error text-3xl mb-2">error</span>
                  <p class="text-sm text-error mb-4">{{ svcPreviewError }}</p>
                  <button @click="loadServicePreview" data-testid="svc-retry-preview" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                    Retry
                  </button>
                </div>

                <template v-else>
                  <!-- Summary Cards (real preview numbers) -->
                  <div v-if="previewSummary.total !== null && previewSummary.total > 0" class="grid grid-cols-3 gap-4 mb-6">
                    <div class="p-4 rounded-xl bg-primary/5 border border-primary/20">
                      <div class="flex items-center gap-3">
                        <div class="w-10 h-10 rounded-lg bg-primary/10 flex items-center justify-center">
                          <span class="material-symbols-outlined text-primary">library_music</span>
                        </div>
                        <div>
                          <p class="text-xl font-bold text-gray-900 dark:text-white" data-testid="svc-preview-total">{{ previewSummary.total.toLocaleString() }}</p>
                          <p class="text-xs text-text-secondary">Total Tracks</p>
                        </div>
                      </div>
                    </div>
                    <div class="p-4 rounded-xl bg-success/5 border border-success/20">
                      <div class="flex items-center gap-3">
                        <div class="w-10 h-10 rounded-lg bg-success/10 flex items-center justify-center">
                          <span class="material-symbols-outlined text-success">verified</span>
                        </div>
                        <div>
                          <p class="text-xl font-bold text-success" data-testid="svc-preview-matched">{{ previewSummary.matched.toLocaleString() }} <span class="text-xs font-normal">({{ matchedPercent }}%)</span></p>
                          <p class="text-xs text-text-secondary">Matched</p>
                        </div>
                      </div>
                    </div>
                    <div class="p-4 rounded-xl bg-amber-500/5 border border-amber-500/20">
                      <div class="flex items-center gap-3">
                        <div class="w-10 h-10 rounded-lg bg-amber-500/10 flex items-center justify-center">
                          <span class="material-symbols-outlined text-amber-500">help</span>
                        </div>
                        <div>
                          <p class="text-xl font-bold text-amber-500" data-testid="svc-preview-unmatched">{{ unmatchedCountDisplay.toLocaleString() }} <span class="text-xs font-normal">({{ unmatchedPercent }}%)</span></p>
                          <p class="text-xs text-text-secondary">Without Match</p>
                        </div>
                      </div>
                    </div>
                  </div>

                  <!-- Honest empty source library -->
                  <div v-else-if="previewSummary.total !== null && previewSummary.total === 0" class="rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-8 text-center mb-6">
                    <span class="material-symbols-outlined text-4xl text-gray-400 mb-2">library_music</span>
                    <p class="text-sm text-text-secondary">No tracks found in your {{ getServiceName(svcSource) }} library. Sync the source service first to import your tracks.</p>
                  </div>

                  <!-- Reviewable matches: real items of the last migration on this route -->
                  <div v-if="reviewJob" class="mb-2">
                    <p class="text-xs text-text-secondary mb-2">Matches from your last {{ reviewJob.source_service }} → {{ reviewJob.destination_service }} migration. Attach a match manually — the next migration applies your manual matches instead of re-searching.</p>
                    <div class="flex items-center gap-2 mb-4">
                      <button
                        v-for="filter in matchFilterOptions"
                        :key="filter.id"
                        @click="matchFilter = filter.id"
                        :class="[
                          'px-4 py-2 rounded-full text-sm font-medium transition-all',
                          matchFilter === filter.id ? 'bg-primary text-white' : 'bg-gray-100 dark:bg-surface-highlight text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
                        ]"
                      >
                        {{ filter.label }}
                        <span v-if="filter.count > 0" class="ml-1.5 opacity-70">({{ filter.count }})</span>
                      </button>
                    </div>
                    <div class="match-preview border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden max-h-[300px] overflow-y-auto custom-scrollbar">
                      <div
                        v-for="item in visibleReviewItems"
                        :key="item.id"
                        data-testid="svc-review-item-row"
                        class="match-row flex items-center gap-4 px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30"
                      >
                        <div class="flex items-center gap-3 flex-1 min-w-0">
                          <div class="w-10 h-10 rounded-lg shrink-0 flex items-center justify-center bg-gradient-to-br from-purple-500 to-pink-500">
                            <span class="material-symbols-outlined text-white/50 text-lg">music_note</span>
                          </div>
                          <div class="min-w-0">
                            <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ item.source_track_title }}</p>
                            <p class="text-xs text-text-secondary truncate">{{ item.source_track_artist }}</p>
                          </div>
                        </div>
                        <div class="flex items-center gap-2 shrink-0">
                          <span v-if="item.destination_track_id" class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-success/10 text-success">
                            Matched<template v-if="item.match_confidence !== null"> {{ Math.round(item.match_confidence * 100) }}%</template>
                          </span>
                          <span v-else-if="item.status === 'skipped'" class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-amber-500/10 text-amber-500">
                            No match found
                          </span>
                          <span v-else-if="item.status === 'failed'" class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-error/10 text-error">
                            Failed
                          </span>
                          <span v-else class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-gray-100 dark:bg-surface-highlight text-text-secondary">
                            {{ item.status }}
                          </span>
                          <button
                            v-if="reviewJob.destination_account_id !== null && !item.destination_track_id && (item.status === 'skipped' || item.status === 'failed')"
                            @click="openManualMatch(item, reviewJob.destination_service)"
                            class="px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors"
                          >
                            Search Manually
                          </button>
                        </div>
                      </div>
                    </div>
                    <button
                      v-if="filteredReviewItems.length > visibleReviewCount"
                      @click="visibleReviewCount += 25"
                      class="mt-2 text-sm text-primary hover:text-primary-hover font-medium"
                    >
                      Show more ({{ filteredReviewItems.length - visibleReviewCount }} remaining)
                    </button>
                  </div>

                  <!-- Honest note when there is no previous migration on this route -->
                  <div v-else class="rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-6 mb-6">
                    <p class="text-sm text-text-secondary">This will be the first migration from {{ getServiceName(svcSource) }} to {{ getServiceName(svcDestination) }}. Track-level matches are recorded as the transfer runs.</p>
                  </div>
                </template>

                <!-- Skip Toggle -->
                <div class="flex items-center gap-3 mt-4 pt-4 border-t border-gray-200 dark:border-border-dark">
                  <button
                    @click="svcSkipNotFound = !svcSkipNotFound"
                    data-testid="svc-skip-not-found-toggle"
                    :class="['w-5 h-5 rounded border-2 flex items-center justify-center transition-colors', svcSkipNotFound ? 'bg-primary border-primary text-white' : 'border-gray-300 dark:border-gray-600']"
                  >
                    <span v-if="svcSkipNotFound" class="material-symbols-outlined text-[14px]">check</span>
                  </button>
                  <span class="text-sm text-gray-700 dark:text-gray-300">Skip all tracks with no match found<template v-if="unmatchedCountDisplay > 0"> ({{ unmatchedCountDisplay }} tracks)</template></span>
                </div>
              </div>

              <!-- SERVICE → SERVICE MODE / STEP 4: Transfer with real progress -->
              <div v-else-if="assistantMode === 'service' && svcStep === 3" key="svc-transfer" class="wizard-step">
                <MigrationTransferPanel
                  :started="svcStarted"
                  :complete="svcComplete"
                  :starting="migration.isStartingMigration.value"
                  :error="svcError"
                  ready-title="Ready to migrate"
                  :ready-description="`Click &quot;Start Transfer&quot; to begin migrating your saved ${getServiceName(svcSource)} tracks to ${getServiceName(svcDestination)}.`"
                  :progress="realProgress"
                  :activity-log="activityLog"
                  start-label="Start Transfer"
                  @start="startServiceMigration"
                  @cancel="cancelServiceMigration"
                  @view-details="openCurrentJobDetails"
                  @reset="resetServiceMode"
                />
              </div>

              <!-- STEP 1: Select Source Service -->
              <div v-else-if="currentStep === 0" key="step1" class="wizard-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Select source service</h2>
                <p class="text-text-secondary mb-8">Choose where to transfer from</p>

                <div class="service-grid grid grid-cols-3 gap-4">
                  <button
                    v-for="service in sourceServices"
                    :key="service.id"
                    :data-service-id="service.id"
                    @click="selectTransferSource(service.id)"
                    :disabled="!isConnected(service.id)"
                    :class="[
                      'service-card relative p-6 rounded-xl border-2 transition-all text-center',
                      !isConnected(service.id) ? 'opacity-50 cursor-not-allowed border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30' :
                      sourceService === service.id ? 'border-primary bg-primary/5 shadow-lg shadow-primary/10' :
                      'border-gray-200 dark:border-border-dark hover:border-primary/50 hover:bg-gray-50 dark:hover:bg-surface-highlight/50'
                    ]"
                  >
                    <div :class="['w-20 h-20 mx-auto rounded-2xl flex items-center justify-center mb-4 text-3xl', service.bgClass]">
                      {{ service.icon }}
                    </div>
                    <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">{{ service.name }}</h3>
                    <div class="flex items-center justify-center gap-1.5">
                      <span :class="['w-2 h-2 rounded-full', isConnected(service.id) ? 'bg-success' : 'bg-gray-400']"></span>
                      <span :class="['text-xs font-medium', isConnected(service.id) ? 'text-success' : 'text-text-secondary']">
                        {{ isConnected(service.id) ? 'Connected' : 'Not Connected' }}
                      </span>
                    </div>
                    <!-- Selected Indicator -->
                    <div v-if="sourceService === service.id" class="absolute top-3 right-3 w-6 h-6 rounded-full bg-primary text-white flex items-center justify-center">
                      <span class="material-symbols-outlined text-[16px]">check</span>
                    </div>
                  </button>
                </div>
                <label v-if="sourceService && accountsForService(sourceService).length > 1" class="block mt-4 text-sm">
                  Cuenta origen
                  <select v-model.number="sourceAccount" class="block w-full mt-1 rounded border p-2 bg-white dark:bg-surface-dark" data-testid="transfer-source-account">
                    <option v-for="a in accountsForService(sourceService)" :key="a.id" :value="a.id">{{ a.display_name || a.email || `Cuenta ${a.id}` }} {{ a.is_active ? '(activa)' : '' }}</option>
                  </select>
                </label>
              </div>

              <!-- STEP 2: Choose Content to Transfer -->
              <div v-else-if="currentStep === 1" key="step2" class="wizard-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">What do you want to transfer?</h2>
                <p class="text-text-secondary mb-8">The migration engine transfers your saved tracks; albums and artists are not supported yet</p>

                <div class="content-cards grid grid-cols-2 gap-4">
                  <button
                    v-for="content in contentTypes"
                    :key="content.id"
                    @click="toggleContentType(content.id)"
                    :disabled="content.disabled"
                    :class="[
                      'content-card relative p-6 rounded-xl border-2 transition-all text-left',
                      content.disabled ? 'opacity-50 cursor-not-allowed border-gray-200 dark:border-border-dark' :
                      selectedContent.includes(content.id) ? 'border-primary bg-primary/5' : 'border-gray-200 dark:border-border-dark hover:border-primary/50'
                    ]"
                  >
                    <div class="flex items-start gap-4">
                      <div :class="['w-14 h-14 rounded-xl flex items-center justify-center', selectedContent.includes(content.id) ? 'bg-primary/10 text-primary' : 'bg-gray-100 dark:bg-surface-highlight text-gray-500']">
                        <span class="material-symbols-outlined text-[28px]">{{ content.icon }}</span>
                      </div>
                      <div class="flex-1">
                        <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{{ content.label }}</h3>
                        <p class="text-sm text-text-secondary">{{ content.description }}</p>
                      </div>
                    </div>
                    <!-- Checkbox -->
                    <div v-if="!content.disabled" :class="[
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
                <p class="text-text-secondary mb-8">Select one or more destination services (a separate migration runs for each)</p>

                <div class="service-grid grid grid-cols-3 gap-4">
                  <button
                    v-for="service in destinationChoices"
                    :key="service.id"
                    :data-service-id="service.id"
                    @click="toggleDestination(service.id)"
                    :disabled="!isConnected(service.id) || (service.id === sourceService && (!selectedAccountExists(service.id, sourceAccount) || accountsForService(service.id).length <= 1))"
                    :class="[
                      'service-card relative p-6 rounded-xl border-2 transition-all text-center',
                      service.id === sourceService && (!selectedAccountExists(service.id, sourceAccount) || accountsForService(service.id).length <= 1) ? 'opacity-30 cursor-not-allowed border-gray-200 dark:border-border-dark' :
                      !isConnected(service.id) ? 'opacity-50 cursor-not-allowed border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30' :
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
                        <span :class="['w-2 h-2 rounded-full', isConnected(service.id) ? 'bg-success' : 'bg-gray-400']"></span>
                        <span :class="['text-xs font-medium', isConnected(service.id) ? 'text-success' : 'text-text-secondary']">
                          {{ isConnected(service.id) ? 'Connected' : 'Not Connected' }}
                        </span>
                      </template>
                    </div>
                    <!-- Checkbox indicator -->
                    <div v-if="destinationServices.includes(service.id)" class="absolute top-3 right-3 w-6 h-6 rounded-full bg-primary text-white flex items-center justify-center">
                      <span class="material-symbols-outlined text-[16px]">check</span>
                    </div>
                  </button>
                </div>

                <label v-for="dest in destinationServices.filter(id => accountsForService(id).length > 1)" :key="dest" class="block mt-4 text-sm">
                  Cuenta destino ({{ dest }})
                  <select v-model.number="destinationAccounts[dest]" class="block w-full mt-1 rounded border p-2 bg-white dark:bg-surface-dark" :data-testid="`transfer-destination-account-${dest}`">
                    <option v-for="a in accountsForService(dest)" :key="a.id" :value="a.id">{{ a.display_name || a.email || `Cuenta ${a.id}` }} {{ a.is_active ? '(activa)' : '' }}</option>
                  </select>
                </label>
                <div v-if="destinationChoices.length === 0" class="rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-8 text-center">
                  <span class="material-symbols-outlined text-4xl text-gray-400 mb-2">cloud_off</span>
                  <p class="text-sm text-text-secondary">The migration engine does not support any of your connected services as a destination yet.</p>
                </div>
              </div>

              <!-- STEP 4: Preview & Match Review -->
              <div v-else-if="currentStep === 3" key="step4" class="wizard-step">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">Review matches</h2>
                <p class="text-text-secondary mb-6" data-testid="preview-description">{{ previewDescription }}</p>

                <!-- Preview loading -->
                <div v-if="migration.isPreviewing.value" data-testid="preview-loading" class="flex items-center justify-center gap-3 py-12 text-text-secondary">
                  <span class="material-symbols-outlined animate-spin text-primary text-2xl">sync</span>
                  Loading match preview...
                </div>

                <!-- Preview error -->
                <div v-else-if="previewError" data-testid="preview-error" class="rounded-xl border border-error/30 bg-error/5 p-6 text-center">
                  <span class="material-symbols-outlined text-error text-3xl mb-2">error</span>
                  <p class="text-sm text-error mb-4">{{ previewError }}</p>
                  <button @click="loadPreview" data-testid="retry-preview" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                    Retry
                  </button>
                </div>

                <template v-else>
                  <!-- Summary Cards (real preview numbers) -->
                  <div v-if="previewSummary.total !== null && previewSummary.total > 0" class="grid grid-cols-3 gap-4 mb-6">
                    <div class="p-4 rounded-xl bg-primary/5 border border-primary/20">
                      <div class="flex items-center gap-3">
                        <div class="w-10 h-10 rounded-lg bg-primary/10 flex items-center justify-center">
                          <span class="material-symbols-outlined text-primary">library_music</span>
                        </div>
                        <div>
                          <p class="text-xl font-bold text-gray-900 dark:text-white" data-testid="preview-total">{{ previewSummary.total.toLocaleString() }}</p>
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
                          <p class="text-xl font-bold text-success" data-testid="preview-matched">{{ previewSummary.matched.toLocaleString() }} <span class="text-xs font-normal">({{ matchedPercent }}%)</span></p>
                          <p class="text-xs text-text-secondary">Matched</p>
                        </div>
                      </div>
                    </div>
                    <div class="p-4 rounded-xl bg-amber-500/5 border border-amber-500/20">
                      <div class="flex items-center gap-3">
                        <div class="w-10 h-10 rounded-lg bg-amber-500/10 flex items-center justify-center">
                          <span class="material-symbols-outlined text-amber-500">help</span>
                        </div>
                        <div>
                          <p class="text-xl font-bold text-amber-500" data-testid="preview-unmatched">{{ unmatchedCountDisplay.toLocaleString() }} <span class="text-xs font-normal">({{ unmatchedPercent }}%)</span></p>
                          <p class="text-xs text-text-secondary">Without Match</p>
                        </div>
                      </div>
                    </div>
                  </div>

                  <!-- Honest empty source library -->
                  <div v-else-if="previewSummary.total !== null && previewSummary.total === 0" class="rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-8 text-center mb-6">
                    <span class="material-symbols-outlined text-4xl text-gray-400 mb-2">library_music</span>
                    <p class="text-sm text-text-secondary">No tracks found in your {{ getServiceName(sourceService) }} library. Sync the source service first to import your tracks.</p>
                  </div>

                  <!-- Playlist previews (real, from the backend preview) -->
                  <div v-if="previewPlaylists.length > 0" class="border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden mb-6">
                    <div class="px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight/30 text-xs font-semibold text-text-secondary uppercase tracking-wide">Playlists</div>
                    <div v-for="playlist in previewPlaylists" :key="playlist.id" class="flex items-center justify-between px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0">
                      <span class="text-sm font-medium text-gray-900 dark:text-white">{{ playlist.name }}</span>
                      <span class="text-xs text-text-secondary">{{ playlist.matched_count }} / {{ playlist.track_count }} tracks matched</span>
                    </div>
                  </div>

                  <!-- Filter Pills (real counts from the last migration on this route) -->
                  <div v-if="reviewJob" class="flex items-center gap-2 mb-4">
                    <button
                      v-for="filter in matchFilterOptions"
                      :key="filter.id"
                      @click="matchFilter = filter.id"
                      :class="[
                        'px-4 py-2 rounded-full text-sm font-medium transition-all',
                        matchFilter === filter.id ? 'bg-primary text-white' : 'bg-gray-100 dark:bg-surface-highlight text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
                      ]"
                    >
                      {{ filter.label }}
                      <span v-if="filter.count > 0" class="ml-1.5 opacity-70">({{ filter.count }})</span>
                    </button>
                  </div>

                  <!-- Match Review List (real items from the last migration on this route) -->
                  <div v-if="reviewJob" class="mb-2">
                    <p class="text-xs text-text-secondary mb-2">Unmatched and matched items from your last {{ reviewJob.source_service }} → {{ reviewJob.destination_service }} migration. You can attach a match manually — the next migration applies your manual matches instead of re-searching.</p>
                    <div class="match-preview border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden max-h-[300px] overflow-y-auto custom-scrollbar">
                      <div
                        v-for="item in visibleReviewItems"
                        :key="item.id"
                        data-testid="review-item-row"
                        class="match-row flex items-center gap-4 px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30"
                      >
                        <!-- Source Track -->
                        <div class="flex items-center gap-3 flex-1 min-w-0">
                          <div class="w-10 h-10 rounded-lg shrink-0 flex items-center justify-center bg-gradient-to-br from-purple-500 to-pink-500">
                            <span class="material-symbols-outlined text-white/50 text-lg">music_note</span>
                          </div>
                          <div class="min-w-0">
                            <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ item.source_track_title }}</p>
                            <p class="text-xs text-text-secondary truncate">{{ item.source_track_artist }}</p>
                          </div>
                        </div>

                        <!-- Outcome -->
                        <div class="flex items-center gap-2 shrink-0">
                          <span v-if="item.destination_track_id" class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-success/10 text-success">
                            Matched<template v-if="item.match_confidence !== null"> {{ Math.round(item.match_confidence * 100) }}%</template>
                          </span>
                          <span v-else-if="item.status === 'skipped'" class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-amber-500/10 text-amber-500">
                            No match found
                          </span>
                          <span v-else-if="item.status === 'failed'" class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-error/10 text-error">
                            Failed
                          </span>
                          <span v-else class="match-confidence px-2 py-1 rounded-full text-xs font-bold bg-gray-100 dark:bg-surface-highlight text-text-secondary">
                            {{ item.status }}
                          </span>
                          <button
                            v-if="reviewJob.destination_account_id !== null && !item.destination_track_id && (item.status === 'skipped' || item.status === 'failed')"
                            @click="openManualMatch(item, reviewJob.destination_service)"
                            class="px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors"
                          >
                            Search Manually
                          </button>
                        </div>
                      </div>
                    </div>
                    <button
                      v-if="filteredReviewItems.length > visibleReviewCount"
                      @click="visibleReviewCount += 25"
                      class="mt-2 text-sm text-primary hover:text-primary-hover font-medium"
                    >
                      Show more ({{ filteredReviewItems.length - visibleReviewCount }} remaining)
                    </button>
                  </div>

                  <!-- Honest note when there is no previous migration on this route -->
                  <div v-else class="rounded-xl border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 p-6 mb-6">
                    <p class="text-sm text-text-secondary">This will be the first migration from {{ getServiceName(sourceService) }} to {{ destinationNames }}. Track-level matches are computed and reviewed once the transfer starts.</p>
                  </div>
                </template>

                <!-- Skip Toggle -->
                <div class="flex items-center gap-3 mt-4 pt-4 border-t border-gray-200 dark:border-border-dark">
                  <button
                    @click="skipNotFound = !skipNotFound"
                    data-testid="skip-not-found-toggle"
                    :class="['w-5 h-5 rounded border-2 flex items-center justify-center transition-colors', skipNotFound ? 'bg-primary border-primary text-white' : 'border-gray-300 dark:border-gray-600']"
                  >
                    <span v-if="skipNotFound" class="material-symbols-outlined text-[14px]">check</span>
                  </button>
                  <span class="text-sm text-gray-700 dark:text-gray-300">Skip all tracks with no match found<template v-if="unmatchedCountDisplay > 0"> ({{ unmatchedCountDisplay }} tracks)</template></span>
                </div>
              </div>

              <!-- STEP 5: Transfer in Progress (shared panel, real progress events) -->
              <div v-else-if="currentStep === 4" key="step5" class="wizard-step">
                <MigrationTransferPanel
                  :started="transferStarted"
                  :complete="transferComplete"
                  :starting="migration.isStartingMigration.value"
                  :error="transferError"
                  :ready-description="`Click &quot;Start Transfer&quot; to begin migrating ${readyTrackCount} to ${destinationNames}.`"
                  :ready-hint="destinationServices.length > 1 ? 'A separate migration job runs for each destination service.' : ''"
                  :progress="realProgress"
                  :activity-log="activityLog"
                  @start="startTransfer"
                  @cancel="cancelTransfer"
                  @view-details="openCurrentJobDetails"
                  @reset="resetWizard"
                />
              </div>

            </Transition>
          </div>

          <!-- Wizard Footer / Navigation -->
          <div class="px-8 py-5 border-t border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/30 flex items-center justify-between">
            <button
              v-if="activeStepIndex > 0"
              @click="prevStep"
              class="flex items-center gap-2 px-5 py-2.5 text-gray-700 dark:text-gray-300 hover:text-gray-900 dark:hover:text-white transition-colors"
            >
              <span class="material-symbols-outlined text-[20px]">arrow_back</span>
              Back
            </button>
            <div v-else></div>

            <button
              v-if="activeStepIndex < activeSteps.length - 1"
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

      <!-- Migration History -->
      <div class="migration-history max-w-[900px] mx-auto mt-8">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-semibold text-gray-900 dark:text-white">Migration History</h2>
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
                v-for="mig in visibleHistory"
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

          <!-- Load More (paginates the real history client-side) -->
          <div v-if="combinedHistory.length > visibleHistoryCount" class="px-4 py-3 border-t border-gray-200 dark:border-border-dark text-center">
            <button @click="visibleHistoryCount += historyPageSize" data-testid="history-load-more" class="text-sm text-primary hover:text-primary-hover font-medium">
              Load More ({{ combinedHistory.length - visibleHistoryCount }} remaining)
            </button>
          </div>
        </div>
      </div>

      <!-- Saved Templates Section -->
      <div id="saved-templates" class="saved-templates max-w-[900px] mx-auto mt-8 mb-8">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-xl font-semibold text-gray-900 dark:text-white">Saved Templates</h2>
          <button @click="openSaveTemplateModal" class="flex items-center gap-2 text-sm text-primary hover:text-primary-hover font-medium">
            <span class="material-symbols-outlined text-[16px]">add</span>
            Create Template
          </button>
        </div>

        <!-- Empty State -->
        <div v-if="backendTemplates.length === 0" class="bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl p-10 text-center">
          <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-2">bookmark_border</span>
          <p class="text-sm text-text-secondary">No templates saved yet. Configure a migration in the wizard and save it here for reuse.</p>
        </div>

        <div v-else class="grid grid-cols-3 gap-4">
          <div
            v-for="template in backendTemplates"
            :key="template.id"
            class="template-item bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark rounded-xl p-4 hover:border-primary/30 transition-colors"
          >
            <div class="flex items-start justify-between mb-3">
              <div>
                <h3 class="font-medium text-gray-900 dark:text-white text-sm">{{ template.name }}</h3>
                <p class="text-xs text-text-secondary mt-0.5">Updated: {{ new Date(template.updated_at).toLocaleDateString() }}</p>
              </div>
              <div class="flex gap-1">
                <button @click="editTemplate(template)" class="p-1 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors">
                  <span class="material-symbols-outlined text-[16px]">edit</span>
                </button>
                <button @click="handleDeleteTemplate(template.id)" class="p-1 text-gray-400 hover:text-error transition-colors">
                  <span class="material-symbols-outlined text-[16px]">delete</span>
                </button>
              </div>
            </div>

            <div class="flex items-center gap-2 mb-3">
              <span class="text-lg">{{ getServiceIcon(template.source_service) }}</span>
              <span class="material-symbols-outlined text-gray-400 text-sm">arrow_forward</span>
              <span class="text-lg">{{ getServiceIcon(template.destination_service) }}</span>
              <span class="text-xs text-text-secondary ml-1">{{ template.description || 'Migration template' }}</span>
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
                <span class="text-2xl">{{ selectedMigration ? getServiceIcon(selectedMigration.source) : '' }}</span>
                <span class="material-symbols-outlined text-gray-400">arrow_forward</span>
                <span class="text-2xl">{{ selectedMigration ? getServiceIcon(selectedMigration.dest) : '' }}</span>
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

              <!-- Failed Tracks (real items of the job) -->
              <div v-if="(selectedMigration?.failedCount ?? 0) > 0 || failedTracks.length > 0" class="mb-4">
                <button @click="showFailedTracks = !showFailedTracks" class="w-full flex items-center justify-between px-4 py-3 bg-error/5 border border-error/20 rounded-lg hover:bg-error/10 transition-colors">
                  <span class="font-medium text-error">Failed Tracks ({{ failedTracks.length }})</span>
                  <span :class="['material-symbols-outlined text-error transition-transform', showFailedTracks ? 'rotate-180' : '']">expand_more</span>
                </button>
                <div v-if="showFailedTracks" class="mt-2 border border-gray-200 dark:border-border-dark rounded-lg overflow-hidden">
                  <div v-for="track in failedTracks" :key="track.id" class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0 flex items-center justify-between">
                    <div class="min-w-0">
                      <p class="text-sm font-medium text-gray-900 dark:text-white">{{ track.source_track_title }}</p>
                      <p class="text-xs text-text-secondary">{{ track.source_track_artist }}</p>
                      <p class="text-xs text-error mt-1">{{ track.error_message || 'Transfer failed' }}</p>
                    </div>
                    <button @click="openManualMatch(track, selectedMigration?.dest || destinationServices[0])" :disabled="!selectedMigration?.destination_account_id" class="shrink-0 ml-3 px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors">
                      Search Manually
                    </button>
                  </div>
                </div>
              </div>

              <!-- Skipped Tracks (real items of the job) -->
              <div v-if="(selectedMigration?.skippedCount ?? 0) > 0 || skippedTracks.length > 0">
                <button @click="showSkippedTracks = !showSkippedTracks" class="w-full flex items-center justify-between px-4 py-3 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-200 dark:border-border-dark rounded-lg hover:bg-gray-100 dark:hover:bg-surface-highlight transition-colors">
                  <span class="font-medium text-gray-700 dark:text-gray-300">Skipped Tracks ({{ skippedTracks.length }})</span>
                  <span :class="['material-symbols-outlined text-gray-400 transition-transform', showSkippedTracks ? 'rotate-180' : '']">expand_more</span>
                </button>
                <div v-if="showSkippedTracks" class="mt-2 border border-gray-200 dark:border-border-dark rounded-lg overflow-hidden">
                  <div v-for="track in skippedTracks" :key="track.id" class="px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0 flex items-center justify-between">
                    <div class="min-w-0">
                      <p class="text-sm font-medium text-gray-900 dark:text-white">{{ track.source_track_title }}</p>
                      <p class="text-xs text-text-secondary">{{ track.source_track_artist }}</p>
                      <p class="text-xs text-amber-500 mt-1">{{ track.error_message || 'No match found on the destination service' }}</p>
                    </div>
                    <button @click="openManualMatch(track, selectedMigration?.dest || destinationServices[0])" :disabled="!selectedMigration?.destination_account_id" class="shrink-0 ml-3 px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors">
                      Search Manually
                    </button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Modal Footer -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-end">
              <button @click="showDetailsModal = false" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors">
                Close
              </button>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>

    <!-- Manual Match Modal (real search + match against the destination service) -->
    <Teleport to="body">
      <Transition name="fade">
        <div v-if="showManualMatchModal && manualMatchTarget" data-testid="manual-match-modal" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-8" @click.self="closeManualMatch">
          <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-xl max-h-[90vh] overflow-hidden shadow-2xl">
            <!-- Modal Header -->
            <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between">
              <div>
                <h3 class="font-semibold text-gray-900 dark:text-white">Find a match for "{{ manualMatchTarget.source_track_title }}"</h3>
                <p class="text-xs text-text-secondary mt-0.5">{{ manualMatchTarget.source_track_artist }} · searching {{ manualMatchService }}</p>
              </div>
              <button @click="closeManualMatch" class="p-2 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
                <span class="material-symbols-outlined text-gray-400">close</span>
              </button>
            </div>

            <!-- Search -->
            <div class="p-6">
              <div class="flex items-center gap-2 mb-4">
                <input
                  v-model="manualSearchQuery"
                  type="text"
                  data-testid="manual-match-search-input"
                  :placeholder="`Search ${manualMatchService} for this track...`"
                  class="flex-1 px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50"
                  @keyup.enter="runManualSearch"
                >
                <button
                  @click="runManualSearch"
                  :disabled="!manualSearchQuery.trim() || migration.isSearching.value"
                  data-testid="manual-match-search-btn"
                  class="px-4 py-2.5 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                >
                  {{ migration.isSearching.value ? 'Searching...' : 'Search' }}
                </button>
              </div>

              <!-- Manual match feedback -->
              <p v-if="manualMatchMessage" class="mb-3 text-sm" :class="manualMatchOk ? 'text-success' : 'text-error'">{{ manualMatchMessage }}</p>

              <!-- Loading -->
              <div v-if="migration.isSearching.value" class="flex items-center justify-center gap-2 py-8 text-text-secondary">
                <span class="material-symbols-outlined animate-spin text-primary">sync</span>
                Searching {{ manualMatchService }}...
              </div>

              <!-- Results -->
              <div v-else-if="migration.searchResults.value.length > 0" class="border border-gray-200 dark:border-border-dark rounded-xl overflow-hidden max-h-[320px] overflow-y-auto custom-scrollbar">
                <div
                  v-for="result in migration.searchResults.value"
                  :key="result.track_id"
                  data-testid="manual-match-result"
                  class="flex items-center gap-3 px-4 py-3 border-b border-gray-100 dark:border-border-dark/50 last:border-0 hover:bg-gray-50 dark:hover:bg-surface-highlight/30"
                >
                  <div class="min-w-0 flex-1">
                    <p class="text-sm font-medium text-gray-900 dark:text-white truncate">{{ result.title }}</p>
                    <p class="text-xs text-text-secondary truncate">{{ result.artist }}<template v-if="result.album"> · {{ result.album }}</template></p>
                  </div>
                  <span v-if="result.quality" class="px-1.5 py-0.5 rounded text-[9px] font-bold bg-quality-gold/10 text-quality-gold shrink-0">{{ result.quality }}</span>
                  <span class="text-xs text-text-secondary shrink-0">{{ Math.round(result.confidence * 100) }}%</span>
                  <button
                    @click="applyManualMatch(result)"
                    :disabled="manualMatchBusy"
                    data-testid="manual-match-apply"
                    class="shrink-0 px-3 py-1.5 bg-primary/10 text-primary hover:bg-primary/20 rounded-lg text-xs font-medium transition-colors disabled:opacity-50"
                  >
                    Use this match
                  </button>
                </div>
              </div>

              <!-- Honest empty state -->
              <div v-else-if="manualSearchPerformed" class="py-8 text-center">
                <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-2">search_off</span>
                <p class="text-sm text-text-secondary">No results for "{{ manualSearchQuery }}" on {{ manualMatchService }}. Try a different query.</p>
              </div>

              <!-- Initial hint -->
              <div v-else class="py-8 text-center">
                <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600 mb-2">manage_search</span>
                <p class="text-sm text-text-secondary">Search the destination service and pick the right track to attach to this item.</p>
              </div>
            </div>

            <!-- Modal Footer -->
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex items-center justify-end">
              <button @click="closeManualMatch" class="px-5 py-2 bg-gray-100 dark:bg-surface-highlight hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors">
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
              <p v-if="!sourceService || destinationServices.length === 0" class="text-xs text-amber-500 mt-1">Select a source and at least one destination service in the wizard first — the template stores the current selection.</p>
            </div>
            <div class="p-6 space-y-4">
              <div>
                <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Template Name</label>
                <input v-model="newTemplateName" type="text" placeholder="My Migration Template" class="w-full px-4 py-2.5 bg-gray-50 dark:bg-surface-highlight border border-gray-200 dark:border-border-dark rounded-lg text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-primary/50">
              </div>
              <p class="text-xs text-text-secondary">Saves the current wizard selection: source {{ sourceService || 'not selected' }}, destination {{ destinationServices[0] || 'not selected' }}, content {{ selectedContent.join(', ') || 'none' }}. Saving with an existing name updates that template.</p>
            </div>
            <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-3">
              <button @click="showSaveTemplateModal = false" class="px-5 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg text-sm font-medium transition-colors">
                Cancel
              </button>
              <button @click="saveTemplate" :disabled="!sourceService || destinationServices.length === 0 || !newTemplateName.trim() || isSavingTemplate" class="px-5 py-2 bg-primary hover:bg-primary-hover text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed">
                {{ isSavingTemplate ? 'Saving...' : 'Save Template' }}
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
// R13: confirm unificado; antes se resolvía el window.confirm global nativo.
import { useToast, confirm } from '../composables/useToast'
import { accountsApi } from '../api/accounts'
import { getMigrationItemsByStatus } from '../api/migration'
import MigrationTransferPanel, { type MigrationActivityEntry, type MigrationProgressDisplay } from '../components/MigrationTransferPanel.vue'
import type { MigrationItem, MigrationOptions, MigrationTemplate, ServiceStatus, Account, DestinationTrackMatch } from '../api/types'

// Initialize migration composable for backend integration
const migration = useMigration()
const toast = useToast()

// Load history, templates, service auth statuses and the engine's supported
// destinations on mount; setup event listener
onMounted(async () => {
  await Promise.all([
    migration.loadHistory(),
    migration.loadTemplates(),
    migration.loadSupportedDestinations(),
    loadServiceStatuses(),
    loadAccounts(),
  ])
  await migration.setupProgressListener()
})

// Cleanup event listener on unmount
onUnmounted(() => {
  migration.cleanup()
})

/** Real auth status of every service, from the backend accounts DB. */
const serviceStatuses = ref<ServiceStatus[]>([])
const migrationAccounts = ref<Account[]>([])
async function loadAccounts(): Promise<void> {
  try { migrationAccounts.value = await accountsApi.getAccounts() }
  catch (e) { console.error('Failed to load accounts for migration:', e) }
}

function accountsForService(service: string): Account[] {
  return migrationAccounts.value.filter(a => a.service_name?.toLowerCase() === service.toLowerCase())
}
function defaultAccount(service: string): number | null {
  const matches = accountsForService(service)
  return (matches.find(a => a.is_active && !a.credentials_invalid)
    ?? matches.find(a => !a.credentials_invalid))?.id ?? null
}
function selectedAccountExists(service: string, id: number | null): boolean {
  return typeof id === 'number' && accountsForService(service).some(account => account.id === id)
}
function accountSelectionValid(service: string, id: number | null): boolean {
  const accounts = accountsForService(service)
  return accounts.length === 0 || accounts.some(account => account.id === id && !account.credentials_invalid)
}
function unsafeSameServicePair(destination: string, source: string, sourceId: number | null, destinationId: number | null): boolean {
  // A null source reads the whole service mirror, including playlists. Never
  // treat it as the active account for a same-service transfer or preview.
  return destination === source && (!selectedAccountExists(source, sourceId) || !selectedAccountExists(destination, destinationId) || sourceId === destinationId)
}


async function loadServiceStatuses(): Promise<void> {
  try {
    serviceStatuses.value = await accountsApi.getServiceStatuses()
  } catch (e) {
    console.error('Failed to load service statuses:', e)
  }
}

/** Legacy service-level status fallback when the account list has no rows. */
const connectedServiceIds = computed(() => {
  return new Set(
    serviceStatuses.value
      .filter(s => s.connected && !s.credentials_invalid)
      .map(s => s.name.toLowerCase())
  )
})

function isConnected(serviceId: string): boolean {
  const accounts = accountsForService(serviceId)
  // When accounts exist, the selected row (not the service-wide active status)
  // determines whether this service can be used in the wizard.
  if (accounts.length > 0) return accounts.some(a => !a.credentials_invalid)
  return connectedServiceIds.value.has(serviceId)
}

// Migration schema/state audit (IN-5)
async function runSchemaAudit() {
  try {
    await migration.runAudit()
  } catch (err) {
    console.error('Migration audit failed:', err)
  }
}

// Cancel must work while the job runs: progress events carry the real job_id.
watch(() => migration.progress.status, (newStatus) => {
  if (newStatus === 'cancelled') {
    transferStarted.value = false
    svcStarted.value = false
  }
})

// Real progress from the backend (migration-progress events via useMigration)
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
    currentAction: p.current_action || p.current_track || 'Processing...',
    eta: p.eta || 'calculating...',
    speed: p.speed > 0 ? p.speed.toFixed(1) : '—'
  }
})

// Activity log built from real migration-progress events.
const activityLog = ref<MigrationActivityEntry[]>([])
const lastProgressCounts = { failed: 0, skipped: 0, completed: 0 }

watch(() => migration.progress.current_item, () => {
  const p = migration.progress
  if (!p.job_id || p.status === 'idle') return

  let status: MigrationActivityEntry['status'] = 'ok'
  let message = p.current_action || p.current_track
  if (p.failed_count > lastProgressCounts.failed) {
    status = 'failed'
    message = p.current_track ? `Failed: ${p.current_track}` : (message || 'Item failed')
  } else if (p.skipped_count > lastProgressCounts.skipped) {
    status = 'skipped'
    message = p.current_track ? `Skipped: ${p.current_track}` : (message || 'Item skipped')
  } else if (!message) {
    message = `Processing item ${p.current_item}`
  }

  lastProgressCounts.failed = p.failed_count
  lastProgressCounts.skipped = p.skipped_count
  lastProgressCounts.completed = p.completed_count

  activityLog.value.unshift({ id: `${p.job_id}-${p.current_item}`, time: new Date().toLocaleTimeString(), status, message })
  if (activityLog.value.length > 100) {
    activityLog.value.splice(100)
  }
})

function resetActivityLog(): void {
  activityLog.value = []
  lastProgressCounts.failed = 0
  lastProgressCounts.skipped = 0
  lastProgressCounts.completed = 0
}

// Computed: Migration history from backend
const backendHistory = computed(() => migration.history.value.map(migration.formatHistoryItem))

// Computed: Combined history directly from backend (no mock fallback)
const combinedHistory = computed(() => backendHistory.value)

// Client-side pagination over the real history table
const historyPageSize = 10
const visibleHistoryCount = ref(historyPageSize)
const visibleHistory = computed(() => combinedHistory.value.slice(0, visibleHistoryCount.value))

// Recent migrations dropdown: real, most recent history entries
const showRecentDropdown = ref(false)
const recentMigrations = computed(() => backendHistory.value.slice(0, 5))

// Computed: Templates from backend (no mock fallback)
const backendTemplates = computed(() => migration.templates.value)

// Helper: Get service icon by name
function getServiceIcon(serviceName: string): string {
  const iconMap: Record<string, string> = {
    'spotify': '🎵',
    'qobuz': '🎧',
    'tidal': '🌊',
    'deezer': '🎶',
    'soundcloud': '☁️',
    'apple': '🍎',
    'apple_music': '🍎'
  }
  return iconMap[serviceName.toLowerCase()] || '🎵'
}

// Handler: Open migration details modal (loads the real items of the job)
interface MigrationDetailsCard {
  id: string | number
  source: string
  dest: string
  source_account_id: number | null
  destination_account_id: number | null
  date: string
  totalCount: number
  successCount: number
  failedCount: number
  skippedCount: number
}

const showDetailsModal = ref(false)
const selectedMigration = ref<MigrationDetailsCard | null>(null)
const showFailedTracks = ref(false)
const showSkippedTracks = ref(false)

function openMigrationDetails(mig: MigrationDetailsCard) {
  selectedMigration.value = mig
  showDetailsModal.value = true
  showFailedTracks.value = false
  showSkippedTracks.value = false
  showRecentDropdown.value = false
  if (mig.id) {
    migration.loadJobDetails(String(mig.id))
  }
}

/** Open the details modal for the migration that just finished in the wizard. */
function openCurrentJobDetails(expandFailed: boolean): void {
  const jobId = completedJobIds.value[completedJobIds.value.length - 1] || migration.currentJobId.value
  if (!jobId) return
  // The route depends on the mode that ran the migration.
  const [source, dest] = assistantMode.value === 'service'
    ? [svcSource.value, svcDestination.value]
    : [sourceService.value, destinationServices.value[destinationServices.value.length - 1] || '']
  showFailedTracks.value = expandFailed
  showSkippedTracks.value = false
  const card: MigrationDetailsCard = {
    id: jobId,
    source,
    dest,
    source_account_id: assistantMode.value === 'service' ? svcSourceAccount.value : sourceAccount.value,
    destination_account_id: assistantMode.value === 'service' ? svcDestinationAccount.value : (destinationAccounts.value[dest] ?? null),
    date: new Date().toLocaleString(),
    totalCount: realProgress.value.total,
    successCount: realProgress.value.transferred,
    failedCount: realProgress.value.failed,
    skippedCount: realProgress.value.skipped,
  }
  selectedMigration.value = card
  showDetailsModal.value = true
  migration.loadJobDetails(String(jobId))
}

// Real failed/skipped items of the job shown in the details modal
const failedTracks = computed<MigrationItem[]>(() =>
  migration.selectedJobItems.value.filter(i => i.status === 'failed')
)

const skippedTracks = computed<MigrationItem[]>(() =>
  migration.selectedJobItems.value.filter(i => i.status === 'skipped')
)

// Handler: Delete migration job
async function handleDeleteMigration(jobId: string) {
  // R13: confirmación unificada en la app (antes window.confirm nativo).
  const confirmed = await confirm('Are you sure you want to delete this migration?', {
    title: 'Delete Migration',
    variant: 'danger',
    confirmLabel: 'Delete'
  })
  if (!confirmed) return
  const ok = await migration.deleteJob(jobId)
  if (ok) {
    toast.success('Migration deleted')
  } else {
    toast.error('Could not delete the migration')
  }
}

// Handler: Retry failed items
async function handleRetryMigration(jobId: string) {
  const count = await migration.retryFailed(jobId)
  toast.info(`Marked ${count} failed items for retry in job ${jobId}`)
}

// Wizard state
const currentStep = ref(0)
const stepDirection = ref('slide-left')

// ========================
// ASSISTANT MODE (4.2): the service → service mode drives the migration
// engine directly with its own source/destination selection, independent of
// the transfer wizard's selection below.
// ========================

type AssistantMode = 'service' | 'transfer'

const assistantMode = ref<AssistantMode>('transfer')

const assistantModeDescription = computed(() =>
  assistantMode.value === 'service'
    ? 'Migrate your saved tracks directly from one service to another, with a real match preview.'
    : 'Configure a transfer with content selection and one migration per destination service.'
)

const svcStep = ref(0)
const svcSource = ref('')
const svcDestination = ref('')
const svcSourceAccount = ref<number | null>(null)
const svcDestinationAccount = ref<number | null>(null)
const svcSkipNotFound = ref(false)
const svcPreviewError = ref<string | null>(null)
const svcStarted = ref(false)
const svcComplete = ref(false)
const svcError = ref<string | null>(null)

const svcSteps = [
  { id: 'svc-source', label: 'Source' },
  { id: 'svc-destination', label: 'Destination' },
  { id: 'svc-review', label: 'Review' },
  { id: 'svc-migrate', label: 'Migrate' },
]

/** Switch assistant mode; each mode keeps its own selection and progress. */
function setAssistantMode(mode: AssistantMode): void {
  assistantMode.value = mode
}

const steps = [
  { id: 'source', label: 'Source' },
  { id: 'content', label: 'Content' },
  { id: 'destination', label: 'Destination' },
  { id: 'preview', label: 'Preview' },
  { id: 'transfer', label: 'Transfer' },
]

const activeSteps = computed(() => (assistantMode.value === 'service' ? svcSteps : steps))
const activeStepIndex = computed(() => (assistantMode.value === 'service' ? svcStep.value : currentStep.value))

const progressWidth = computed(() => {
  return `${(activeStepIndex.value / (activeSteps.value.length - 1)) * 100}%`
})

// Services: presentation metadata keyed by id; the grids themselves are
// data-driven (sources come from the backend's service statuses, destinations
// from the engine's supported-destination list intersected with connectivity).
const SERVICE_META: Record<string, { name: string; icon: string; bgClass: string }> = {
  spotify: { name: 'Spotify', icon: '🎵', bgClass: 'bg-[#1ed760]/10' },
  qobuz: { name: 'Qobuz', icon: '🎧', bgClass: 'bg-[#1a8fe3]/10' },
  tidal: { name: 'Tidal', icon: '🌊', bgClass: 'bg-[#00d4aa]/10' },
  deezer: { name: 'Deezer', icon: '🎶', bgClass: 'bg-[#ff0092]/10' },
  soundcloud: { name: 'SoundCloud', icon: '☁️', bgClass: 'bg-[#ff5500]/10' },
  apple_music: { name: 'Apple Music', icon: '🍎', bgClass: 'bg-[#fa243c]/10' },
}

interface ServiceCard {
  id: string
  name: string
  icon: string
  bgClass: string
}

function serviceCardMeta(serviceId: string): ServiceCard {
  const meta = SERVICE_META[serviceId.toLowerCase()]
  return meta ? { id: serviceId, ...meta } : { id: serviceId, name: serviceId, icon: '🎵', bgClass: 'bg-gray-500/10' }
}

function getServiceName(serviceId: string): string {
  return serviceCardMeta(serviceId).name
}

/** Source choices: every service the backend reports, with real connectivity. */
const sourceServices = computed<ServiceCard[]>(() =>
  serviceStatuses.value.map(s => serviceCardMeta(s.name.toLowerCase()))
)

const supportedDestinationSet = computed(
  () => new Set(migration.supportedDestinations.value.map(d => d.toLowerCase()))
)

/** Destination choices: only the services the migration engine supports. */
const destinationChoices = computed<ServiceCard[]>(() =>
  serviceStatuses.value
    .map(s => s.name.toLowerCase())
    .filter(id => supportedDestinationSet.value.has(id))
    .map(serviceCardMeta)
)

const sourceService = ref<string>('')
const destinationServices = ref<string[]>([])
const sourceAccount = ref<number | null>(null)
const destinationAccounts = ref<Record<string, number | null>>({})

// Content types: honest descriptions; the engine only transfers saved tracks
const contentTypes = [
  { id: 'favorites', label: 'Favorites & Tracks', icon: 'favorite', description: 'Saved tracks and favorites', disabled: false },
  { id: 'playlists', label: 'Playlists', icon: 'queue_music', description: 'Saved with the migration options', disabled: false },
  { id: 'albums', label: 'Saved Albums', icon: 'album', description: 'Not supported by the migration engine yet', disabled: true },
  { id: 'artists', label: 'Followed Artists', icon: 'person', description: 'Not supported by the migration engine yet', disabled: true },
]

const selectedContent = ref<string[]>(['favorites'])

// Navigation
const canProceed = computed(() => {
  if (assistantMode.value === 'service') {
    switch (svcStep.value) {
      case 0: return svcSource.value !== '' && accountSelectionValid(svcSource.value, svcSourceAccount.value)
      case 1: return svcDestination.value !== '' && accountSelectionValid(svcDestination.value, svcDestinationAccount.value) && !unsafeSameServicePair(svcDestination.value, svcSource.value, svcSourceAccount.value, svcDestinationAccount.value)
      default: return true
    }
  }
  switch (currentStep.value) {
    case 0: return sourceService.value !== '' && accountSelectionValid(sourceService.value, sourceAccount.value)
    case 1: return selectedContent.value.length > 0
    case 2: return destinationServices.value.length > 0 && destinationServices.value.every(id => accountSelectionValid(id, destinationAccounts.value[id] ?? null)) && !destinationServices.value.some(id => unsafeSameServicePair(id, sourceService.value, sourceAccount.value, destinationAccounts.value[id] ?? null))
    default: return true
  }
})

function nextStep() {
  if (assistantMode.value === 'service') {
    if (svcStep.value < svcSteps.length - 1 && canProceed.value) {
      stepDirection.value = 'slide-left'
      svcStep.value++
      if (svcStep.value === 2) {
        loadServicePreview()
      }
    }
    return
  }
  if (currentStep.value < steps.length - 1 && canProceed.value) {
    stepDirection.value = 'slide-left'
    currentStep.value++
    if (currentStep.value === 3) {
      loadPreview()
    }
  }
}

function prevStep() {
  if (assistantMode.value === 'service') {
    if (svcStep.value > 0) {
      stepDirection.value = 'slide-right'
      svcStep.value--
      // Coming back from the migration: the matches to review are the ones of
      // the job that just ran, which the refreshed history now reports. The
      // preview counts are left as loaded (re-matching costs real API calls).
      if (svcStep.value === 2) {
        loadReviewItems(svcSource.value, svcDestination.value)
      }
    }
    return
  }
  if (currentStep.value > 0) {
    stepDirection.value = 'slide-right'
    currentStep.value--
    if (currentStep.value === 3) {
      loadReviewItems(sourceService.value, destinationServices.value[0] || '')
    }
  }
}

function toggleContentType(id: string) {
  const contentType = contentTypes.find(c => c.id === id)
  if (!contentType || contentType.disabled) return
  const idx = selectedContent.value.indexOf(id)
  if (idx >= 0) {
    selectedContent.value.splice(idx, 1)
  } else {
    selectedContent.value.push(id)
  }
}

function selectSvcSource(id: string) {
  if (!isConnected(id)) return
  svcSource.value = id
  svcSourceAccount.value = defaultAccount(id)
  svcDestination.value = ''
  svcDestinationAccount.value = null
}
function selectSvcDestination(id: string) {
  if (!isConnected(id) || (id === svcSource.value && (!selectedAccountExists(id, svcSourceAccount.value) || accountsForService(id).length <= 1))) return
  svcDestination.value = id
  svcDestinationAccount.value = defaultAccount(id)
  // Same-service transfers require two explicitly selected, distinct accounts.
  if (id === svcSource.value && svcDestinationAccount.value === svcSourceAccount.value) {
    svcDestinationAccount.value = accountsForService(id).find(a => a.id !== svcSourceAccount.value)?.id ?? null
  }
}
function selectTransferSource(id: string) {
  if (!isConnected(id)) return
  sourceService.value = id
  sourceAccount.value = defaultAccount(id)
  destinationServices.value = []
  destinationAccounts.value = {}
}

function toggleDestination(id: string) {
  if (id === sourceService.value && (!selectedAccountExists(id, sourceAccount.value) || accountsForService(id).length <= 1)) return
  if (!isConnected(id)) return
  const idx = destinationServices.value.indexOf(id)
  if (idx >= 0) {
    destinationServices.value.splice(idx, 1)
    delete destinationAccounts.value[id]
  } else {
    destinationServices.value.push(id)
    destinationAccounts.value[id] = defaultAccount(id)
    if (id === sourceService.value && destinationAccounts.value[id] === sourceAccount.value) {
      destinationAccounts.value[id] = accountsForService(id).find(a => a.id !== sourceAccount.value)?.id ?? null
    }
  }
}

const destinationNames = computed(() =>
  destinationServices.value.map(id => getServiceName(id)).join(', ') || 'your destination service(s)'
)

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

// ========================
// STEP 4: REAL MATCH PREVIEW
// ========================

const matchFilter = ref<'all' | 'matched' | 'review' | 'notfound'>('all')
const skipNotFound = ref(false)
const previewError = ref<string | null>(null)
const visibleReviewCount = ref(25)

const previewSummary = computed(() => {
  const r = migration.previewResult.value
  return {
    total: r ? r.total_tracks : null,
    matched: r ? r.matched_tracks : 0,
    unmatched: r ? r.unmatched_tracks : null,
  }
})

const matchedPercent = computed(() => {
  const { total, matched } = previewSummary.value
  return total && total > 0 ? Math.round((matched / total) * 100) : 0
})

const unmatchedPercent = computed(() => {
  const { total, unmatched } = previewSummary.value
  return total && total > 0 ? Math.round(((unmatched ?? 0) / total) * 100) : 0
})

const unmatchedCountDisplay = computed(() => previewSummary.value.unmatched ?? 0)

const previewPlaylists = computed(() => migration.previewResult.value?.playlists ?? [])

/**
 * What the review step really previews: preview_migration takes a single
 * destination, so with several services selected only the first one is
 * previewed here and each extra destination is matched when its own migration
 * runs. The copy names that destination instead of the whole selection.
 */
const previewedDestinationName = computed(() =>
  getServiceName(destinationServices.value[0] || '')
)

const previewDescription = computed(() => {
  const base = `The preview really matches your ${getServiceName(sourceService.value)} tracks against ${previewedDestinationName.value} (ISRC and metadata search) without adding anything`
  if (destinationServices.value.length > 1) {
    return `${base}. ${destinationNames.value} are all selected, so each additional destination is matched when its own migration runs.`
  }
  return `${base}.`
})

/** Load the real preview from the backend when the wizard reaches step 4. */
async function loadPreview(): Promise<void> {
  previewError.value = null
  const destination = destinationServices.value[0] || ''
  if (!accountSelectionValid(sourceService.value, sourceAccount.value) ||
      !accountSelectionValid(destination, destinationAccounts.value[destination] ?? null) ||
      unsafeSameServicePair(destination, sourceService.value, sourceAccount.value, destinationAccounts.value[destination] ?? null)) {
    previewError.value = 'Choose valid accounts; same-service migrations require two explicit, distinct accounts.'
    return
  }
  const result = await migration.preview(sourceService.value, destination, undefined, migration.defaultOptions, sourceAccount.value, destinationAccounts.value[destination] ?? null)
  if (!result) {
    previewError.value = 'Could not load the match preview. Check that the destination service is connected and try again.'
    return
  }
  await loadReviewItems(sourceService.value, destination)
}

/** Load the real preview for the service → service mode's review step. */
async function loadServicePreview(): Promise<void> {
  svcPreviewError.value = null
  if (!accountSelectionValid(svcSource.value, svcSourceAccount.value) ||
      !accountSelectionValid(svcDestination.value, svcDestinationAccount.value) ||
      unsafeSameServicePair(svcDestination.value, svcSource.value, svcSourceAccount.value, svcDestinationAccount.value)) {
    svcPreviewError.value = 'Choose valid accounts; same-service migrations require two explicit, distinct accounts.'
    return
  }
  const result = await migration.preview(svcSource.value, svcDestination.value, undefined, migration.defaultOptions, svcSourceAccount.value, svcDestinationAccount.value)
  if (!result) {
    svcPreviewError.value = 'Could not load the match preview. Check that the destination service is connected and try again.'
    return
  }
  await loadReviewItems(svcSource.value, svcDestination.value)
}

/**
 * Real per-item review data: the migration items of the most recent job for
 * the given source → destination route. The backend creates migration items
 * only when a job runs, so this surfaces the last run's real matches instead
 * of inventing pre-start rows. Shared by both assistant modes.
 */
const reviewJob = ref<{ id: string; source_service: string; destination_service: string; source_account_id: number | null; destination_account_id: number | null } | null>(null)
const reviewItems = ref<MigrationItem[]>([])

async function loadReviewItems(source: string, destination: string): Promise<void> {
  reviewJob.value = null
  reviewItems.value = []
  visibleReviewCount.value = 25
  const sourceId = assistantMode.value === 'service' ? svcSourceAccount.value : sourceAccount.value
  const destinationId = assistantMode.value === 'service' ? svcDestinationAccount.value : (destinationAccounts.value[destination] ?? null)
  const job = migration.history.value.find(
    j => j.source_service === source && j.destination_service === destination
      && j.source_account_id === sourceId && j.destination_account_id === destinationId
  )
  if (!job) return
  try {
    const items = await getMigrationItemsByStatus(job.id)
    reviewJob.value = { id: job.id, source_service: job.source_service, destination_service: job.destination_service,
      source_account_id: job.source_account_id, destination_account_id: job.destination_account_id }
    reviewItems.value = items
  } catch (e) {
    console.error('Failed to load review items:', e)
  }
}

const filteredReviewItems = computed<MigrationItem[]>(() => {
  switch (matchFilter.value) {
    case 'matched':
      return reviewItems.value.filter(i => !!i.destination_track_id || i.status === 'transferred' || i.status === 'matched')
    case 'review':
      return reviewItems.value.filter(i => i.status === 'failed')
    case 'notfound':
      return reviewItems.value.filter(i => i.status === 'skipped')
    default:
      return reviewItems.value
  }
})

const visibleReviewItems = computed(() => filteredReviewItems.value.slice(0, visibleReviewCount.value))

const matchFilterOptions = computed(() => {
  const items = reviewItems.value
  const count = (fn: (i: MigrationItem) => boolean) => items.filter(fn).length
  return [
    { id: 'all' as const, label: 'All', count: items.length },
    { id: 'matched' as const, label: 'Matched', count: count(i => !!i.destination_track_id || i.status === 'transferred' || i.status === 'matched') },
    { id: 'review' as const, label: 'Needs Review', count: count(i => i.status === 'failed') },
    { id: 'notfound' as const, label: 'Not Found', count: count(i => i.status === 'skipped') },
  ]
})

// ========================
// STEP 5: REAL TRANSFER
// ========================

const transferStarted = ref(false)
const transferComplete = ref(false)
const transferError = ref<string | null>(null)
const completedJobIds = ref<string[]>([])

const readyTrackCount = computed(() => {
  const total = previewSummary.value.total
  return total !== null ? `${total.toLocaleString()} tracks` : 'your library'
})

async function startTransfer(): Promise<void> {
  if (!sourceService.value || !accountSelectionValid(sourceService.value, sourceAccount.value) || destinationServices.value.length === 0 ||
      destinationServices.value.some(id => !accountSelectionValid(id, destinationAccounts.value[id] ?? null) || unsafeSameServicePair(id, sourceService.value, sourceAccount.value, destinationAccounts.value[id] ?? null))) return

  transferError.value = null
  transferComplete.value = false
  transferStarted.value = true
  completedJobIds.value = []
  resetActivityLog()

  const options: MigrationOptions = {
    ...migration.defaultOptions,
    skip_unmatched: skipNotFound.value,
    create_playlists: selectedContent.value.includes('playlists'),
  }

  // The backend takes one destination per job: run one real migration per selected service.
  const failedDestinations: string[] = []
  for (const dest of destinationServices.value) {
    if (migration.progress.status === 'cancelled') break
    const jobId = await migration.start(sourceService.value, dest, undefined, options, sourceAccount.value, destinationAccounts.value[dest] ?? null)
    if (jobId) {
      completedJobIds.value.push(jobId)
    } else {
      failedDestinations.push(dest)
    }
  }

  if (migration.progress.status === 'cancelled') {
    transferStarted.value = false
    return
  }

  if (failedDestinations.length > 0) {
    // Surface the failure and return to the ready screen instead of hanging.
    transferStarted.value = false
    transferError.value = failedDestinations.length === destinationServices.value.length
      ? 'Could not start the migration. Make sure the destination service is connected and try again.'
      : `Could not start the migration to ${failedDestinations.join(', ')}. The other destinations were migrated.`
    return
  }

  transferComplete.value = true
  await migration.loadHistory()
}

async function cancelTransfer(): Promise<void> {
  const ok = await migration.cancel()
  if (ok) {
    transferStarted.value = false
    toast.info('Migration cancelled')
  } else {
    toast.error('Could not cancel the migration', 'No running migration was found')
  }
}

// ========================
// SERVICE → SERVICE MODE: start with real progress, cancel, reset
// ========================

async function startServiceMigration(): Promise<void> {
  if (!svcSource.value || !svcDestination.value || !accountSelectionValid(svcSource.value, svcSourceAccount.value) ||
      !accountSelectionValid(svcDestination.value, svcDestinationAccount.value) ||
      unsafeSameServicePair(svcDestination.value, svcSource.value, svcSourceAccount.value, svcDestinationAccount.value)) return

  svcError.value = null
  svcComplete.value = false
  svcStarted.value = true
  completedJobIds.value = []
  resetActivityLog()

  const options: MigrationOptions = {
    ...migration.defaultOptions,
    skip_unmatched: svcSkipNotFound.value,
    // The service → service mode migrates the saved library; playlist
    // creation belongs to the transfer wizard's content selection.
    create_playlists: false,
  }

  const jobId = await migration.start(svcSource.value, svcDestination.value, undefined, options, svcSourceAccount.value, svcDestinationAccount.value)

  if (migration.progress.status === 'cancelled') {
    // The user cancelled while the job ran; do not show a completed screen.
    svcStarted.value = false
    return
  }

  if (!jobId) {
    // Surface the failure and return to the ready screen instead of hanging.
    svcStarted.value = false
    svcError.value = 'Could not start the migration. Make sure the destination service is connected and try again.'
    return
  }

  completedJobIds.value = [jobId]
  svcComplete.value = true
  await migration.loadHistory()
}

async function cancelServiceMigration(): Promise<void> {
  const ok = await migration.cancel()
  if (ok) {
    svcStarted.value = false
    toast.info('Migration cancelled')
  } else {
    toast.error('Could not cancel the migration', 'No running migration was found')
  }
}

function resetServiceMode(): void {
  svcStep.value = 0
  svcSource.value = ''
  svcDestination.value = ''
  svcSourceAccount.value = null
  svcDestinationAccount.value = null
  svcSkipNotFound.value = false
  svcPreviewError.value = null
  svcStarted.value = false
  svcComplete.value = false
  svcError.value = null
  completedJobIds.value = []
  matchFilter.value = 'all'
  reviewJob.value = null
  reviewItems.value = []
  visibleReviewCount.value = 25
  resetActivityLog()
}

function resetWizard() {
  currentStep.value = 0
  stepDirection.value = 'slide-left'
  sourceService.value = ''
  destinationServices.value = []
  sourceAccount.value = null
  destinationAccounts.value = {}
  selectedContent.value = ['favorites']
  transferStarted.value = false
  transferComplete.value = false
  transferError.value = null
  completedJobIds.value = []
  matchFilter.value = 'all'
  reviewJob.value = null
  reviewItems.value = []
  visibleReviewCount.value = 25
  previewError.value = null
  resetActivityLog()
}

// ========================
// MANUAL MATCH MODAL (real search + match)
// ========================

const showManualMatchModal = ref(false)
const manualMatchTarget = ref<MigrationItem | null>(null)
const manualMatchAccountId = ref<number | null>(null)
const manualMatchService = ref('')
const manualSearchQuery = ref('')
const manualSearchPerformed = ref(false)
const manualMatchBusy = ref(false)
const manualMatchMessage = ref<string | null>(null)
const manualMatchOk = ref(false)

function openManualMatch(item: MigrationItem, service: string): void {
  const job = migration.history.value.find(j => j.id === item.job_id)
    ?? (migration.selectedJob.value?.id === item.job_id ? migration.selectedJob.value : null)
  // Never search or attach a match without the owning job's destination scope.
  if (!job || job.destination_service !== service || job.destination_account_id === null) return
  manualMatchAccountId.value = job.destination_account_id
  manualMatchTarget.value = item
  manualMatchService.value = service || destinationServices.value[0] || ''
  manualSearchQuery.value = [item.source_track_title, item.source_track_artist].filter(Boolean).join(' ')
  migration.searchResults.value = []
  manualSearchPerformed.value = false
  manualMatchMessage.value = null
  showManualMatchModal.value = true
}

function closeManualMatch(): void {
  showManualMatchModal.value = false
  manualMatchTarget.value = null
  manualMatchAccountId.value = null
  migration.searchResults.value = []
}

async function runManualSearch(): Promise<void> {
  if (!manualSearchQuery.value.trim() || !manualMatchService.value) return
  manualSearchPerformed.value = true
  // A historical item must never be searched with today's selected account.
  await migration.searchTracks(manualMatchService.value, manualSearchQuery.value, manualMatchAccountId.value)
}

async function applyManualMatch(result: DestinationTrackMatch): Promise<void> {
  if (!manualMatchTarget.value || !migration.history.value.some(j =>
    j.id === manualMatchTarget.value?.job_id && j.destination_service === manualMatchService.value
      && j.destination_account_id !== null && j.destination_account_id === manualMatchAccountId.value
  ) && !(migration.selectedJob.value?.id === manualMatchTarget.value.job_id
    && migration.selectedJob.value.destination_service === manualMatchService.value
    && migration.selectedJob.value.destination_account_id !== null
    && migration.selectedJob.value.destination_account_id === manualMatchAccountId.value)) return
  manualMatchBusy.value = true
  manualMatchMessage.value = null
  const ok = await migration.matchItem(manualMatchTarget.value.id, result.track_id)
  manualMatchBusy.value = false

  if (ok) {
    manualMatchOk.value = true
    manualMatchMessage.value = `"${manualMatchTarget.value.source_track_title}" matched to "${result.title}".`
    toast.success('Track matched', `"${result.title}" attached to "${manualMatchTarget.value.source_track_title}"`)
    // Refresh the real item lists so the new match shows up
    if (reviewJob.value) {
      reviewItems.value = await getMigrationItemsByStatus(reviewJob.value.id)
    }
    if (showDetailsModal.value && selectedMigration.value?.id) {
      await migration.loadJobDetails(String(selectedMigration.value.id))
    }
    showManualMatchModal.value = false
    manualMatchTarget.value = null
  } else {
    manualMatchOk.value = false
    manualMatchMessage.value = 'Could not save the match. Try again.'
  }
}

// ========================
// SAVED TEMPLATES
// ========================

const showSaveTemplateModal = ref(false)
const newTemplateName = ref('')
const isSavingTemplate = ref(false)

function scrollToTemplates(): void {
  document.getElementById('saved-templates')?.scrollIntoView({ behavior: 'smooth' })
}

function openSaveTemplateModal(): void {
  newTemplateName.value = ''
  showSaveTemplateModal.value = true
}

/** Load a real backend template into the wizard. */
function useTemplate(template: MigrationTemplate): void {
  sourceService.value = template.source_service.toLowerCase()
  destinationServices.value = [template.destination_service.toLowerCase()]
  sourceAccount.value = defaultAccount(sourceService.value)
  destinationAccounts.value = { [destinationServices.value[0]]: defaultAccount(destinationServices.value[0]) }
  if (sourceService.value === destinationServices.value[0] && destinationAccounts.value[sourceService.value] === sourceAccount.value) {
    destinationAccounts.value[sourceService.value] = accountsForService(sourceService.value).find(a => a.id !== sourceAccount.value)?.id ?? null
  }
  let options: Partial<MigrationOptions> = {}
  try {
    options = JSON.parse(template.options) || {}
  } catch {
    options = {}
  }
  selectedContent.value = options.create_playlists ? ['favorites', 'playlists'] : ['favorites']
  // Templates store a transfer-wizard selection: show that mode.
  assistantMode.value = 'transfer'
  currentStep.value = 0
  toast.info('Template loaded', `"${template.name}" applied to the wizard`)
}

/** Edit: load the template into the wizard and pre-fill the save dialog (same name updates it). */
function editTemplate(template: MigrationTemplate): void {
  useTemplate(template)
  newTemplateName.value = template.name
  showSaveTemplateModal.value = true
}

async function saveTemplate(): Promise<void> {
  if (!sourceService.value || destinationServices.value.length === 0) {
    toast.error('Cannot save the template', 'Select a source and a destination service in the wizard first.')
    return
  }
  isSavingTemplate.value = true
  const ok = await migration.saveTemplate(
    newTemplateName.value.trim() || 'New Template',
    `Source: ${sourceService.value}, Content: ${selectedContent.value.join(', ')}`,
    sourceService.value,
    destinationServices.value[0],
    {
      ...migration.defaultOptions,
      create_playlists: selectedContent.value.includes('playlists'),
    }
  )
  isSavingTemplate.value = false
  if (ok) {
    showSaveTemplateModal.value = false
    newTemplateName.value = ''
    toast.success('Template saved')
  } else {
    toast.error('Could not save the template')
  }
}

async function handleDeleteTemplate(templateId: number) {
  // R13: confirmación unificada en la app (antes window.confirm nativo).
  const confirmed = await confirm('Are you sure you want to delete this template?', {
    title: 'Delete Template',
    variant: 'danger',
    confirmLabel: 'Delete'
  })
  if (!confirmed) return
  const ok = await migration.deleteTemplate(templateId)
  if (ok) {
    toast.success('Template deleted')
  } else {
    toast.error('Could not delete the template')
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

/* Fade transition (modals) */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Service card hover effect */
.service-card:not(:disabled):hover {
  transform: translateY(-2px);
}

/* Content card hover effect */
.content-card:not(:disabled):hover {
  transform: translateY(-2px);
}

</style>
