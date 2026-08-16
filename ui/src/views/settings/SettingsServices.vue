<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
       <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
         <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Service Accounts</h3>
         <button 
           @click="showServiceModal = true"
           class="flex items-center gap-2 px-3 py-1.5 bg-primary hover:bg-primary-hover text-white text-sm font-medium rounded-lg transition-colors"
         >
           <span class="material-symbols-outlined text-[18px]">add</span>
           Add Service
         </button>
       </div>
       <div v-if="loadingAccounts" class="flex items-center gap-2 text-text-secondary py-4">
         <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
         <span class="text-sm">Loading services...</span>
       </div>
       <div v-else class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <ServiceCard 
            v-for="service in services" 
            :key="service.name"
            :name="getServiceConfig(service.name).displayName" 
            :icon="getServiceConfig(service.name).icon" 
            :color="getServiceConfig(service.name).color" 
            :isConnected="getAccountsForService(service.name).length > 0"
            :user="getAccountsForService(service.name)[0]?.display_name"
            :status="getAccountsForService(service.name)[0]?.credentials_invalid ? 'Token Expirado' : 'Connected'"
            :statusType="getAccountsForService(service.name)[0]?.credentials_invalid ? 'error' : 'success'"
            :isIconText="getServiceConfig(service.name).isIconText"
          />
       </div>
    </section>

    <section class="space-y-4">
       <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Download Priority</h3>
       <p class="text-sm text-text-secondary">Syncify will try services in this order when downloading tracks. Drag to reorder.</p>
       <div v-if="syncSettings.isLoading.value" class="flex items-center gap-2 text-text-secondary">
         <span class="material-symbols-outlined animate-spin text-[18px]">progress_activity</span>
         <span class="text-sm">Loading service preferences...</span>
       </div>
       <div v-else class="space-y-2">
         <DraggableItem 
           v-for="(pref, i) in orderedServicePreferences" 
           :key="pref.service_name" 
           :text="formatServiceName(pref.service_name)"
           :index="i+1"
           :autoImport="pref.auto_import_enabled"
           @move-up="movePriorityUp(i)"
           @move-down="movePriorityDown(i)"
           @toggle-auto-import="toggleAutoImport(pref.service_name)"
         />
       </div>
    </section>

    <section class="space-y-4">
       <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Fallback Behavior</h3>
       <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
           <BaseSelect 
             label="Retry failed downloads" 
             v-model="advancedSettings.settings.max_retries"
             :options="[
               { value: 0, label: 'Never' },
               { value: 1, label: 'Once' },
               { value: 3, label: 'Up to 3 times' },
               { value: 5, label: 'Up to 5 times' },
               { value: -1, label: 'Indefinitely' }
             ]"
             @change="advancedSettings.saveSettings()"
           />
           <BaseInput 
             label="Delay between retries (seconds)" 
             type="number"
             v-model="advancedSettings.settings.retry_delay_seconds"
             @change="advancedSettings.saveSettings()"
           />
       </div>
    </section>

    <!-- Modal for service connection -->
    <ServiceConnectionModal 
      v-model="showServiceModal" 
      @connected="handleServiceConnected"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useSyncSettings } from '@/composables/useSyncSettings'
import { useDownloadSettings } from '@/composables/useDownloadSettings'
import { useAdvancedSettings } from '@/composables/useAdvancedSettings'
import { getServices, getAccounts } from '@/api/accounts'
import type { Service, Account } from '@/api/types'
import ServiceConnectionModal from '@/components/ServiceConnectionModal.vue'
import ServiceCard from '@/components/settings/ServiceCard.vue'
import DraggableItem from '@/components/settings/DraggableItem.vue'
import BaseSelect from '@/components/settings/BaseSelect.vue'
import BaseInput from '@/components/settings/BaseInput.vue'

const syncSettings = useSyncSettings()
const downloadSettings = useDownloadSettings()
const advancedSettings = useAdvancedSettings()

const showServiceModal = ref(false)
const services = ref<Service[]>([])
const accounts = ref<Account[]>([])
const loadingAccounts = ref(true)

const serviceConfigs: Record<string, { displayName: string; icon: string; color: string; isIconText?: boolean }> = {
  spotify: { displayName: 'Spotify', icon: 'library_music', color: '#1ed760' },
  apple_music: { displayName: 'Apple Music', icon: 'music_note', color: '#fa243c' },
  tidal: { displayName: 'Tidal', icon: 'T', color: '#ffffff', isIconText: true },
  qobuz: { displayName: 'Qobuz', icon: 'album', color: '#000000' },
  deezer: { displayName: 'Deezer', icon: 'graphic_eq', color: '#a238ff' },
  soundcloud: { displayName: 'SoundCloud', icon: 'cloud_queue', color: '#ff5500' },
}

const orderedServicePreferences = computed(() => {
  return [...syncSettings.servicePreferences.value].sort((a, b) => a.priority - b.priority)
})

function getServiceConfig(name: string) {
  return serviceConfigs[name.toLowerCase()] || { displayName: name, icon: 'cloud', color: '#666666' };
}

function getAccountsForService(serviceName: string): Account[] {
  const service = services.value.find((s: Service) => s.name === serviceName);
  if (!service) return [];
  return accounts.value.filter(a => a.service_id === service.id);
}

async function loadServicesAndAccounts() {
  loadingAccounts.value = true;
  try {
    const [servicesData, accountsData] = await Promise.all([
      getServices(),
      getAccounts()
    ]);
    services.value = servicesData;
    accounts.value = accountsData;
  } catch (err) {
    console.error('Failed to load accounts:', err)
  } finally {
    loadingAccounts.value = false;
  }
}

function formatServiceName(name: string): string {
  return getServiceConfig(name).displayName;
}

async function movePriorityUp(index: number) {
  if (index <= 0) return
  const newPreferences = [...orderedServicePreferences.value]
  const temp = newPreferences[index]
  newPreferences[index] = newPreferences[index - 1]
  newPreferences[index - 1] = temp
  const newOrder = newPreferences.map(p => p.service_name)
  await syncSettings.reorderPriorities(newOrder)
}

async function movePriorityDown(index: number) {
  if (index >= orderedServicePreferences.value.length - 1) return
  const newPreferences = [...orderedServicePreferences.value]
  const temp = newPreferences[index]
  newPreferences[index] = newPreferences[index + 1]
  newPreferences[index + 1] = temp
  const newOrder = newPreferences.map(p => p.service_name)
  await syncSettings.reorderPriorities(newOrder)
}

async function toggleAutoImport(serviceName: string) {
  const pref = orderedServicePreferences.value.find(p => p.service_name === serviceName)
  if (pref) {
    await syncSettings.updateAutoImport(serviceName, !pref.auto_import_enabled)
  }
}

function handleServiceConnected(_serviceName: string, _displayName: string) {
  showServiceModal.value = false
  loadServicesAndAccounts()
}

onMounted(async () => {
  await loadServicesAndAccounts()
  if (syncSettings.servicePreferences.value.length === 0) {
    await syncSettings.loadSettings()
  }
})
</script>
