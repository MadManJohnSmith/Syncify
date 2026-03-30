<template>
  <div class="relative rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark p-4 transition-all hover:border-primary/50 hover:shadow-glow group">
     <div class="flex justify-between items-start mb-4">
       <div class="flex items-center gap-3">
           <div class="h-10 w-10 flex items-center justify-center rounded-lg" :style="{ backgroundColor: color + '1A', color: color }">
               <span :class="isIconText ? 'font-extrabold text-lg italic' : 'material-symbols-outlined text-[24px]'">{{ icon }}</span>
           </div>
           <div>
              <h4 class="font-medium text-gray-900 dark:text-white">{{ name }}</h4>
              <div v-if="isConnected" class="flex items-center gap-1.5 mt-0.5">
                 <span class="relative flex h-2 w-2">
                    <span class="relative inline-flex rounded-full h-2 w-2" :class="statusType === 'warning' ? 'bg-warning' : 'bg-success'"></span>
                 </span>
                 <span class="text-[10px] font-medium text-text-secondary">{{ status || 'Connected' }}</span>
              </div>
              <span v-else class="text-[10px] font-medium text-text-secondary">Not Connected</span>
           </div>
       </div>
     </div>
     
     <div v-if="isConnected" class="mb-3 space-y-2">
        <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300">
           <input type="checkbox" checked class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent">
           Enable for downloads
        </label>
         <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300">
           <input type="checkbox" checked class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent">
           Auto-import favorites
        </label>
     </div>
     
     <div class="flex gap-2 text-xs">
        <button v-if="isConnected" class="flex-1 py-1.5 bg-gray-100 dark:bg-surface-highlight rounded hover:bg-gray-200 dark:hover:bg-[#384866] transition-colors text-gray-700 dark:text-gray-300">Re-authenticate</button>
        <button v-else class="flex-1 py-1.5 bg-primary/10 text-primary rounded hover:bg-primary hover:text-white transition-colors">Connect</button>
        <button v-if="isConnected" class="py-1.5 px-3 text-error hover:bg-error/10 rounded transition-colors">Disconnect</button>
     </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  name: string
  icon: string
  color: string
  isConnected: boolean
  user?: string
  status?: string
  statusType?: 'success' | 'warning' | 'error'
  isIconText?: boolean
}>()
</script>
