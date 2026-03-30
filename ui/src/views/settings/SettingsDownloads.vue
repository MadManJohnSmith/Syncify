<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
      <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Folder Template</h3>
      
      <div>
         <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Preset templates</label>
         <div class="relative">
           <select v-model="selectedPreset" class="w-full px-3 py-2 bg-white dark:bg-surface-dark border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none appearance-none cursor-pointer">
             <option v-for="(_preset, key) in folderPresets" :key="key" :value="key">{{ key }}</option>
             <option value="Custom">Custom (User Defined)</option>
           </select>
           <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-gray-500">
             <span class="material-symbols-outlined text-[20px]">expand_more</span>
           </div>
         </div>
      </div>
      
      <div class="p-6 rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark mt-4">
         <h4 class="font-medium text-gray-900 dark:text-white mb-4">Template Editor</h4>
         
         <div class="space-y-4">
             <div>
               <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">Folder template</label>
               <input type="text" v-model="folderTemplate" @input="selectedPreset = 'Custom'" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white font-mono text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none">
             </div>
              <div>
               <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">File naming template</label>
               <input type="text" v-model="fileTemplate" @input="selectedPreset = 'Custom'" class="w-full px-3 py-2 bg-gray-50 dark:bg-surface-highlight/50 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-900 dark:text-white font-mono text-sm focus:ring-2 focus:ring-primary focus:border-transparent outline-none">
             </div>
         </div>

         <!-- Template Variables Cheat Sheet -->
         <div class="mt-6">
            <button @click="showVariables = !showVariables" class="flex items-center gap-2 text-sm text-primary hover:text-primary-hover font-medium">
               <span class="material-symbols-outlined text-[18px]">{{ showVariables ? 'expand_less' : 'expand_more' }}</span>
               {{ showVariables ? 'Hide available variables' : 'Show available variables' }}
            </button>
            
            <div v-if="showVariables" class="mt-3 p-4 bg-background-light dark:bg-background-dark rounded-lg border border-gray-200 dark:border-gray-700 text-xs space-y-3 max-h-60 overflow-y-auto custom-scrollbar">
               <div v-for="(vars, category) in templateVariables" :key="category">
                  <h5 class="font-bold text-gray-900 dark:text-white mb-1.5">{{ category }}</h5>
                  <div class="flex flex-wrap gap-2">
                     <span v-for="v in vars" :key="v" 
                           @click="insertVariable(v)"
                           class="px-1.5 py-0.5 bg-gray-100 dark:bg-surface-highlight rounded border border-gray-200 dark:border-gray-600 text-gray-600 dark:text-gray-300 font-mono cursor-pointer hover:bg-primary/10 hover:text-primary hover:border-primary/30 transition-colors"
                           title="Click to copy">
                        {{ v }}
                     </span>
                  </div>
               </div>
            </div>
         </div>

         <div class="mt-6 p-4 bg-background-light dark:bg-background-dark rounded-lg border border-dashed border-gray-300 dark:border-gray-600">
            <div class="flex justify-between items-center mb-2">
               <span class="text-xs text-text-secondary uppercase tracking-wider font-semibold">Preview</span>
            </div>
            <div class="font-mono text-sm text-gray-700 dark:text-gray-300 break-all">
               <span class="text-gray-400">Music\</span>{{ previewPath }}
            </div>
         </div>
      </div>

      <section class="mt-6 space-y-4">
        <h4 class="font-medium text-gray-900 dark:text-white">File Naming Rules</h4>
        <div class="grid grid-cols-2 gap-4">
           <BaseInput label="Replace invalid characters with" defaultValue="_" />
           <BaseInput label="Truncate long names with" defaultValue="..." />
        </div>
        <BaseInput label="Max filename length" defaultValue="255" type="number" />
        
        <div class="pt-4 mt-4 border-t border-gray-100 dark:border-border-dark">
          <BaseSelect 
            label="If primary service fails (Fallback)" 
            v-model="downloadSettings.folderSettings.fallback_action"
            :options="[
              { value: 'try_next', label: 'Try next service' },
              { value: 'skip', label: 'Skip track' },
              { value: 'prompt', label: 'Prompt me' }
            ]"
            @change="downloadSettings.saveFolderSettings()"
          />
        </div>
      </section>
    </section>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import BaseInput from '@/views/settings/BaseInput.vue'
import BaseSelect from '@/components/settings/BaseSelect.vue'

const downloadSettings = useDownloadSettings()

const showVariables = ref(false)
const selectedPreset = ref('Standard')
const folderTemplate = ref('{AlbumArtist}/{Album}')
const fileTemplate = ref('{TrackNumber:pad2} - {Title}.{Format:lower}')

const folderPresets: Record<string, { folder: string, file: string }> = {
  'Standard': { folder: '{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Audiophile': { folder: '{AlbumArtist}/{Year} - {Album} [{Quality}]', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Symfonium-optimized': { folder: '{AlbumArtist}/{Album} ({Year})', file: '{TrackNumber:pad2}. {Title}.{Format:lower}' },
  'MusicBrainz-friendly': { folder: '{AlbumArtist}/{MBReleaseID} - {Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Label-centric': { folder: '{Label}/{AlbumArtist}/{Album} ({Year})', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Alphabetized': { folder: '{FirstLetter:upper}/{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Quality-Sorted': { folder: '{Quality}/{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Source-Based': { folder: '{Source}/{AlbumArtist}/{Album} [{Quality}]', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Genre-Based': { folder: '{Genre}/{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Compilation-Safe': { folder: '{AlbumArtist}/{Album}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Classical Music': { folder: '{Composer}/{Work}', file: '{TrackNumber:pad2} - {Title} ({Conductor}, {Orchestra}).{Format:lower}' },
  'Multi-Disc': { folder: '{AlbumArtist}/{Album}/Disc {DiscNumber}', file: '{TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Plex/Jellyfin Compatible': { folder: '{AlbumArtist}/{Album} ({Year})', file: '{AlbumArtist} - {Album} - {TrackNumber:pad2} - {Title}.{Format:lower}' },
  'Minimalist': { folder: '{AlbumArtist} - {Album}', file: '{TrackNumber:pad2} {Title}.{Format:lower}' },
  'Archival': { folder: '{AlbumArtist}/{Year} - {Album} [{Quality}] [{Source}] [{CatalogNumber}]', file: '{TrackNumber:pad2} - {Title} [{ISRC}].{Format:lower}' },
}

const templateVariables = {
  'Track Metadata': ['{Artist}', '{AlbumArtist}', '{Album}', '{Title}', '{TrackNumber}', '{TrackNumber:pad2}', '{DiscNumber}', '{Year}', '{Date}', '{Genre}'],
  'Audio Quality': ['{Quality}', '{SampleRate}', '{BitDepth}', '{Bitrate}', '{Format}', '{Codec}'],
  'Identifiers': ['{ISRC}', '{MBTrackID}', '{MBReleaseID}', '{Barcode}', '{CatalogNumber}'],
  'Source': ['{Source}', '{Label}', '{ReleaseType}', '{Media}'],
  'Organization': ['{FirstLetter}', '{FirstLetter:upper}', '{ArtistInitial}'],
  'Classical': ['{Composer}', '{Conductor}', '{Orchestra}', '{Work}', '{Opus}']
}

// Watch for preset selection changes
watch(selectedPreset, (newPreset) => {
  if (newPreset !== 'Custom' && folderPresets[newPreset]) {
    folderTemplate.value = folderPresets[newPreset].folder
    fileTemplate.value = folderPresets[newPreset].file
  }
})

// Simple template engine for preview (mock)
const previewPath = computed(() => {
  let path = `${folderTemplate.value}\\${fileTemplate.value}`
  
  // Mock data for "Bohemian Rhapsody"
  const overrides: Record<string, string> = {
    '{Artist}': 'Queen', '{AlbumArtist}': 'Queen', '{Album}': 'A Night at the Opera', '{Title}': 'Bohemian Rhapsody',
    '{TrackNumber}': '1', '{TrackNumber:pad2}': '01', '{Year}': '1975', '{Genre}': 'Rock',
    '{Quality}': '24-96', '{Format}': 'FLAC', '{Format:lower}': 'flac', '{Source}': 'Qobuz',
    '{Label}': 'EMI', '{MBReleaseID}': '1e0eee38-a9f6-49bf-84de-e53f85bc47b7',
    '{FirstLetter:upper}': 'Q', '{Composer}': 'Freddie Mercury', '{Work}': 'Bohemian Rhapsody'
  }
  
  // Replace all known variables
  for (const [key, val] of Object.entries(overrides)) {
    path = path.replaceAll(key, val)
  }
  
  return path
})

const insertVariable = (v: string) => {
  navigator.clipboard.writeText(v)
}

// Save folder settings to backend
async function saveFolderSettings() {
  downloadSettings.folderSettings.folder_template = folderTemplate.value
  downloadSettings.folderSettings.file_template = fileTemplate.value
  try {
    await downloadSettings.saveFolderSettings()
    console.log('Folder settings saved')
  } catch (e) {
    console.error('Failed to save folder settings:', e)
  }
}

// Watch folder template changes and auto-save after debounce
let folderSaveTimeout: number | null = null
watch([folderTemplate, fileTemplate], () => {
  if (folderSaveTimeout) clearTimeout(folderSaveTimeout)
  folderSaveTimeout = window.setTimeout(() => {
    saveFolderSettings()
  }, 1000)
})

onMounted(async () => {
  try {
    await downloadSettings.loadSettings()
    // Sync folder template state with backend after load
    if (downloadSettings.folderSettings.folder_template) {
      folderTemplate.value = downloadSettings.folderSettings.folder_template
      fileTemplate.value = downloadSettings.folderSettings.file_template
    }
  } catch (err) {
    console.error('Failed to load folder settings:', err)
  }
})
</script>
