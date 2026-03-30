<template>
  <div class="h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark flex items-center justify-between bg-white dark:bg-[#101723]">
      <h1 class="text-lg font-bold tracking-tight text-gray-900 dark:text-white">System Logs</h1>
      
      <div class="flex items-center gap-3">
        <!-- Search -->
        <div class="relative group">
          <span class="absolute left-3 top-2.5 text-gray-400 material-symbols-outlined text-[18px]">search</span>
          <input 
            type="text" 
            placeholder="Search logs..." 
            class="pl-9 pr-4 py-2 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary w-64 transition-all"
          >
        </div>

        <!-- Level Filter -->
        <select class="px-3 py-2 bg-gray-100 dark:bg-surface-dark border border-transparent dark:border-border-dark rounded-lg text-sm text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary">
          <option>All Levels</option>
          <option>Info</option>
          <option>Warning</option>
          <option>Error</option>
          <option>Debug</option>
        </select>

        <button class="p-2 text-gray-400 hover:text-gray-900 dark:hover:text-white hover:bg-gray-100 dark:hover:bg-surface-highlight rounded-lg transition-colors">
          <span class="material-symbols-outlined">delete</span>
        </button>
      </div>
    </div>

    <!-- Logs Console -->
    <div class="flex-1 overflow-y-auto custom-scrollbar p-6 bg-[#0d121c] font-mono text-sm">
      <div class="flex flex-col gap-1">
        
        <!-- Log Entry: Info -->
        <div class="flex items-start gap-4 hover:bg-white/5 p-1 rounded transition-colors group">
          <span class="text-gray-500 shrink-0 w-20">10:42:05</span>
          <span class="text-info font-bold shrink-0 w-16">INFO</span>
          <span class="text-purple shrink-0 w-24">Core</span>
          <span class="text-gray-300 group-hover:text-white">Application started successfully. v2.1.0</span>
        </div>

        <!-- Log Entry: Warning -->
        <div class="flex items-start gap-4 hover:bg-white/5 p-1 rounded transition-colors group">
          <span class="text-gray-500 shrink-0 w-20">10:42:08</span>
          <span class="text-warning font-bold shrink-0 w-16">WARN</span>
          <span class="text-purple shrink-0 w-24">Spotify</span>
          <span class="text-gray-300 group-hover:text-white">Rate limit approaching. Backing off for 2s.</span>
        </div>

        <!-- Log Entry: Success/Info -->
        <div class="flex items-start gap-4 hover:bg-white/5 p-1 rounded transition-colors group">
          <span class="text-gray-500 shrink-0 w-20">10:42:15</span>
          <span class="text-success font-bold shrink-0 w-16">SUCCESS</span>
          <span class="text-purple shrink-0 w-24">Library</span>
          <span class="text-gray-300 group-hover:text-white">Scanned 1,240 tracks in 3.5s.</span>
        </div>

        <!-- Log Entry: Error -->
        <div class="flex items-start gap-4 hover:bg-white/5 p-1 rounded transition-colors group">
          <span class="text-gray-500 shrink-0 w-20">10:45:00</span>
          <span class="text-error font-bold shrink-0 w-16">ERROR</span>
          <span class="text-purple shrink-0 w-24">Download</span>
          <span class="text-gray-300 group-hover:text-white">Failed to download "Track 4". Retrying... (1/3)</span>
          <p class="w-full text-gray-500 pl-64 text-xs mt-1 font-mono">
            Error: Connection timed out at 192.168.1.5<br>
            at DownloadWorker.ts:45:12
          </p>
        </div>

         <!-- Log Entry: Info -->
        <div class="flex items-start gap-4 hover:bg-white/5 p-1 rounded transition-colors group">
          <span class="text-gray-500 shrink-0 w-20">10:46:22</span>
          <span class="text-info font-bold shrink-0 w-16">INFO</span>
          <span class="text-purple shrink-0 w-24">System</span>
          <span class="text-gray-300 group-hover:text-white">Memory usage stable at 145MB.</span>
        </div>

      </div>
    </div>
  </div>
</template>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 10px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: #0d121c;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: #1e293b;
  border: 2px solid #0d121c;
}
</style>
