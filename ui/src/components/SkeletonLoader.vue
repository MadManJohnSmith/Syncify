<template>
  <div class="skeleton-loaders">
    <!-- Track List Skeleton -->
    <div v-if="type === 'track-list'" class="skeleton-track-list">
      <div v-for="i in count" :key="i" class="skeleton-track flex items-center gap-4 p-3 border-b border-gray-100 dark:border-border-dark">
        <!-- Checkbox -->
        <div class="skeleton-item w-5 h-5 rounded bg-gray-200 dark:bg-gray-700"></div>
        <!-- Album Art -->
        <div class="skeleton-item w-12 h-12 rounded-lg bg-gray-200 dark:bg-gray-700 shrink-0"></div>
        <!-- Track Info -->
        <div class="flex-1 space-y-2">
          <div class="skeleton-item h-4 bg-gray-200 dark:bg-gray-700 rounded w-3/4"></div>
          <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-1/2"></div>
        </div>
        <!-- Service Badges -->
        <div class="flex gap-1.5">
          <div class="skeleton-item w-5 h-5 rounded-full bg-gray-200 dark:bg-gray-700"></div>
          <div class="skeleton-item w-5 h-5 rounded-full bg-gray-200 dark:bg-gray-700"></div>
        </div>
        <!-- Quality Badge -->
        <div class="skeleton-item w-14 h-5 rounded bg-gray-200 dark:bg-gray-700"></div>
        <!-- Actions -->
        <div class="skeleton-item w-8 h-8 rounded-lg bg-gray-200 dark:bg-gray-700"></div>
      </div>
      <!-- Shimmer overlay -->
      <div class="shimmer-overlay"></div>
    </div>
    
    <!-- Grid View Skeleton -->
    <div v-if="type === 'grid'" class="skeleton-grid grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-4 p-4">
      <div v-for="i in count" :key="i" class="skeleton-card">
        <div class="skeleton-item aspect-square rounded-xl bg-gray-200 dark:bg-gray-700 mb-3"></div>
        <div class="skeleton-item h-4 bg-gray-200 dark:bg-gray-700 rounded w-4/5 mb-2"></div>
        <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-3/5"></div>
      </div>
      <div class="shimmer-overlay"></div>
    </div>
    
    <!-- Service Cards Skeleton -->
    <div v-if="type === 'service-cards'" class="skeleton-service-cards grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 p-4">
      <div v-for="i in count" :key="i" class="skeleton-service-card p-5 rounded-2xl bg-gray-50 dark:bg-surface-highlight">
        <div class="flex items-start gap-4">
          <div class="skeleton-item w-16 h-16 rounded-2xl bg-gray-200 dark:bg-gray-700"></div>
          <div class="flex-1 space-y-2">
            <div class="skeleton-item h-5 bg-gray-200 dark:bg-gray-700 rounded w-2/3"></div>
            <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-1/2"></div>
            <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-1/3"></div>
          </div>
        </div>
        <div class="flex gap-2 mt-4">
          <div class="skeleton-item h-9 bg-gray-200 dark:bg-gray-700 rounded-lg flex-1"></div>
          <div class="skeleton-item h-9 bg-gray-200 dark:bg-gray-700 rounded-lg w-20"></div>
        </div>
      </div>
      <div class="shimmer-overlay"></div>
    </div>
    
    <!-- Download Items Skeleton -->
    <div v-if="type === 'downloads'" class="skeleton-downloads space-y-2 p-4">
      <div v-for="i in count" :key="i" class="skeleton-download flex items-center gap-4 p-4 rounded-xl bg-gray-50 dark:bg-surface-highlight">
        <div class="skeleton-item w-14 h-14 rounded-lg bg-gray-200 dark:bg-gray-700 shrink-0"></div>
        <div class="flex-1 space-y-2">
          <div class="skeleton-item h-4 bg-gray-200 dark:bg-gray-700 rounded w-3/4"></div>
          <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-1/2"></div>
          <div class="skeleton-item h-2 bg-gray-200 dark:bg-gray-700 rounded w-full mt-3"></div>
        </div>
        <div class="skeleton-item w-8 h-8 rounded-lg bg-gray-200 dark:bg-gray-700"></div>
      </div>
      <div class="shimmer-overlay"></div>
    </div>
    
    <!-- Metadata Editor Skeleton -->
    <div v-if="type === 'metadata-editor'" class="skeleton-metadata flex h-full">
      <!-- Left Panel -->
      <div class="w-80 border-r border-gray-200 dark:border-border-dark p-4 space-y-2">
        <div v-for="i in 8" :key="i" class="flex items-center gap-3 p-2">
          <div class="skeleton-item w-10 h-10 rounded bg-gray-200 dark:bg-gray-700"></div>
          <div class="flex-1 space-y-1.5">
            <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-4/5"></div>
            <div class="skeleton-item h-2 bg-gray-200 dark:bg-gray-700 rounded w-2/3"></div>
          </div>
        </div>
      </div>
      <!-- Right Panel -->
      <div class="flex-1 p-6">
        <div class="flex gap-6">
          <div class="skeleton-item w-48 h-48 rounded-xl bg-gray-200 dark:bg-gray-700 shrink-0"></div>
          <div class="flex-1 space-y-4">
            <div class="space-y-2">
              <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-20"></div>
              <div class="skeleton-item h-10 bg-gray-200 dark:bg-gray-700 rounded w-full"></div>
            </div>
            <div class="space-y-2">
              <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-16"></div>
              <div class="skeleton-item h-10 bg-gray-200 dark:bg-gray-700 rounded w-full"></div>
            </div>
            <div class="space-y-2">
              <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded w-14"></div>
              <div class="skeleton-item h-10 bg-gray-200 dark:bg-gray-700 rounded w-full"></div>
            </div>
          </div>
        </div>
      </div>
      <div class="shimmer-overlay"></div>
    </div>
    
    <!-- Single Item Skeleton (inline) -->
    <div v-if="type === 'inline'" class="skeleton-inline flex items-center gap-2">
      <div class="skeleton-item w-4 h-4 rounded bg-gray-200 dark:bg-gray-700"></div>
      <div class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded" :style="{ width: width + 'px' }"></div>
    </div>
    
    <!-- Text Block Skeleton -->
    <div v-if="type === 'text'" class="skeleton-text space-y-2">
      <div v-for="i in count" :key="i" class="skeleton-item h-3 bg-gray-200 dark:bg-gray-700 rounded" :style="{ width: getRandomWidth() }"></div>
    </div>
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  type: 'track-list' | 'grid' | 'service-cards' | 'downloads' | 'metadata-editor' | 'inline' | 'text'
  count?: number
  width?: number
}>(), {
  count: 10,
  width: 100
})

function getRandomWidth() {
  return (60 + Math.random() * 40) + '%'
}
</script>

<style scoped>
/* Shimmer animation */
.shimmer-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  pointer-events: none;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 255, 255, 0.05) 50%,
    transparent 100%
  );
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}

.dark .shimmer-overlay {
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 255, 255, 0.03) 50%,
    transparent 100%
  );
  background-size: 200% 100%;
}

@keyframes shimmer {
  0% { background-position: 200% 0; }
  100% { background-position: -200% 0; }
}

/* Container positioning for shimmer */
.skeleton-track-list,
.skeleton-grid,
.skeleton-service-cards,
.skeleton-downloads,
.skeleton-metadata {
  position: relative;
  overflow: hidden;
}

/* Skeleton item base */
.skeleton-item {
  animation: pulse 2s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}
</style>
