<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
       <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Application Behavior</h3>
       <BaseToggle 
         title="Start on system boot" 
         subtitle="Launch Syncify when Windows starts" 
         :checked="generalSettings.settings.start_on_boot" 
         @click="generalSettings.settings.start_on_boot = !generalSettings.settings.start_on_boot" 
       />
       <BaseToggle 
         title="Start minimized to tray" 
         subtitle="Hide main window on startup" 
         :checked="generalSettings.settings.start_minimized" 
         @click="generalSettings.settings.start_minimized = !generalSettings.settings.start_minimized" 
       />
       <BaseToggle 
         title="Close to tray instead of exit" 
         subtitle="Keep running in the background when closing the window" 
         :checked="generalSettings.settings.close_to_tray" 
         @click="generalSettings.settings.close_to_tray = !generalSettings.settings.close_to_tray" 
       />
    </section>

    <section class="space-y-4">
       <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Storage & Database</h3>
       <PathSelector 
         label="Library database location" 
         v-model="generalSettings.settings.db_location" 
         :defaultPath="'C:\\Users\\User\\AppData\\Roaming\\Syncify\\syncify.db'" 
         subtitle="Managed automatically by Syncify engine in OS application data"
         disabled
       />
       <PathSelector 
         label="Download directory" 
         v-model="generalSettings.settings.download_dir" 
         :defaultPath="'C:\\Users\\User\\Music\\Syncify'" 
         subtitle="Primary root folder for downloaded audio and album structures"
         hasReset
         @change="generalSettings.saveSettings()"
       />
       <PathSelector 
         label="Temporary files location" 
         v-model="generalSettings.settings.temp_dir" 
         :defaultPath="'C:\\Users\\User\\AppData\\Local\\Temp\\Syncify'" 
         subtitle="Derived automatically as .staging inside download directory for atomic file operations" 
         disabled
       />
       
       <!-- Reset Database Button -->
       <div class="pt-4 border-t border-gray-200 dark:border-border-dark">
         <div class="flex items-center justify-between">
           <div>
             <h4 class="text-sm font-medium text-gray-900 dark:text-white">Reset Database</h4>
             <p class="text-xs text-text-secondary mt-0.5">Delete all library data, accounts, and settings. This cannot be undone.</p>
           </div>
           <button 
             @click="confirmResetDatabase"
             class="px-4 py-2 bg-red-600 hover:bg-red-700 text-white text-sm font-medium rounded-lg transition-colors"
           >
             Reset Database
           </button>
         </div>
       </div>
    </section>

     <section class="space-y-4">
       <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Updates & Telemetry</h3>
       <BaseToggle 
          title="Check for updates automatically" 
          :checked="generalSettings.settings.auto_updates" 
          @click="generalSettings.settings.auto_updates = !generalSettings.settings.auto_updates" 
       />
       <BaseToggle 
          title="Send anonymous usage statistics" 
          subtitle="Help improve Syncify by sharing non-identifying usage data" 
          :checked="generalSettings.settings.anonymous_stats" 
          @click="generalSettings.settings.anonymous_stats = !generalSettings.settings.anonymous_stats" 
       />
    </section>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { confirm, message } from '@tauri-apps/plugin-dialog'
import { useGeneralSettings } from '@/composables/useGeneralSettings'
import BaseToggle from '@/components/settings/BaseToggle.vue'
import PathSelector from '@/components/settings/PathSelector.vue'

const generalSettings = useGeneralSettings()

onMounted(async () => {
  await generalSettings.loadSettings()
})

// Reset database with confirmation (Audit verified: includes Tauri confirm guard)
async function confirmResetDatabase() {
  const confirmed = await confirm('Are you sure you want to reset your library? This will delete all tracks, albums, artists, and playlists. Your accounts and settings will be PRESERVED.', {
    title: 'Reset Database',
    kind: 'warning'
  })
  
  if (confirmed !== true) return
  
  try {
    const result = await invoke<string>('reset_database')
    await message(result, { title: 'Database Reset', kind: 'info' })
  } catch (err) {
    console.error('Failed to reset database:', err)
    await message('Failed to reset database: ' + err, { title: 'Error', kind: 'error' })
  }
}
</script>
