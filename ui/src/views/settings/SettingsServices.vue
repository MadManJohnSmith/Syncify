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
          <div v-for="service in services" :key="service.name" class="space-y-2">
            <ServiceCard :id="service.id" :serviceId="service.name"
              :name="getServiceConfig(service.name).displayName"
              :icon="getServiceConfig(service.name).icon"
              :color="getServiceConfig(service.name).color"
              :isConnected="isServiceConnected(service.name)"
              :hasAccount="getAccountsForService(service.name).length > 0"
              :user="getCurrentAccount(service.name)?.display_name ?? undefined"
              :status="getServiceStatusText(service.name)"
              :statusType="getServiceStatusType(service.name)"
              :isIconText="getServiceConfig(service.name).isIconText"
              :enabled="isServiceDownloadEnabled(service.name)"
              :autoImport="isServiceAutoImportEnabled(service.name)"
              @connect="handleConnect(service.id)" @disconnect="handleDisconnect(service.id)"
              @reauth="handleReauth(service.id)" @toggle-enabled="handleToggleEnabled"
              @toggle-auto-import="handleToggleAutoImport" />
            <div v-for="acct in getAccountsForService(service.name)" :key="acct.id" class="flex flex-wrap gap-2 items-center text-xs p-2 rounded border border-gray-200 dark:border-border-dark">
              <span class="grow truncate">{{ acct.display_name || acct.email || `Cuenta ${acct.id}` }} <span v-if="acct.is_active" class="text-success">(activa)</span></span>
              <span v-if="acct.credentials_invalid" class="text-error">Requiere autenticación</span>
              <button v-if="!acct.is_active" class="text-primary" @click="activateAccount(acct.id)">Usar esta cuenta</button>
              <button class="text-text-secondary" @click="handleDisconnect(service.id, acct.id)">Desconectar</button>
              <button class="text-error" @click="deleteAccount(acct.id)">Eliminar cuenta</button>
            </div>
          </div>
       </div>
    </section>

    <section class="space-y-4">
       <h3 class="text-lg font-semibold text-gray-900 dark:text-white pb-2 border-b border-gray-200 dark:border-border-dark">Download Priority</h3>
       <p class="text-sm text-text-secondary">Syncify will try services in this order when downloading tracks. Drag a row, or use its arrow buttons, to reorder.</p>
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
           :total="orderedServicePreferences.length"
           :autoImport="pref.auto_import_enabled"
           @move-up="movePriorityUp(i)"
           @move-down="movePriorityDown(i)"
           @drag-start="handleDragStart"
           @reorder="handleDragReorder"
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
import { 
  getServices, 
  getAccounts, 
  getServiceStatuses,
  startAuthAndSave,
  logoutService,
  removeAccount,
  toggleAccountActive,
  setActiveAccount,
} from '@/api/accounts'
import type { Service, Account, ServiceStatus } from '@/api/types'
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
const serviceStatuses = ref<ServiceStatus[]>([])
const loadingAccounts = ref(true)
const serviceDownloadEnabled = ref<Record<string, boolean>>({})

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
  if (!Array.isArray(services.value)) return [];
  const service = services.value.find((s: Service) => s.name.toLowerCase() === serviceName.toLowerCase());
  if (!service || !Array.isArray(accounts.value)) return [];
  return accounts.value.filter(a => a.service_id === service.id);
}

function getCurrentAccount(serviceName: string): Account | undefined {
  const matches = getAccountsForService(serviceName)
  return matches.find(a => a.is_active) ?? matches[0]
}

function getServiceStatus(serviceName: string): ServiceStatus | undefined {
  if (!Array.isArray(serviceStatuses.value)) return undefined;
  return serviceStatuses.value.find(s => s?.name?.toLowerCase() === serviceName?.toLowerCase())
}

function isServiceConnected(serviceName: string): boolean {
  const status = getServiceStatus(serviceName)
  if (status) return status.connected || status.credentials_invalid
  return getCurrentAccount(serviceName)?.is_active === true
}

function getServiceStatusText(serviceName: string): string {
  const status = getServiceStatus(serviceName)
  if (status?.credentials_invalid) return 'Token Expired / Re-auth Required'
  if (status?.connected) return 'Connected'
  const acct = getCurrentAccount(serviceName)
  if (acct?.credentials_invalid) return 'Token Expired / Re-auth Required'
  if (acct?.is_active) return 'Connected'
  return 'Not Connected'
}

function getServiceStatusType(serviceName: string): 'success' | 'warning' | 'error' {
  const status = getServiceStatus(serviceName)
  if (status?.credentials_invalid) return 'error'
  if (status?.connected) return 'success'
  const acct = getCurrentAccount(serviceName)
  if (acct?.credentials_invalid) return 'error'
  if (acct?.is_active) return 'success'
  return 'warning'
}

function resolveServiceName(idOrName?: string | number): string {
  if (idOrName === undefined || idOrName === null || idOrName === '') return ''
  const str = String(idOrName).toLowerCase()
  const found = services.value.find(s => String(s.id) === str || s.name.toLowerCase() === str)
  if (found) return found.name.toLowerCase()
  return str
}

function isServiceDownloadEnabled(serviceName: string): boolean {
  const name = serviceName.toLowerCase()
  if (serviceDownloadEnabled.value[name] !== undefined) {
    return serviceDownloadEnabled.value[name]
  }
  const acct = getCurrentAccount(name)
  return acct ? acct.is_active : true
}

function isServiceAutoImportEnabled(serviceName: string): boolean {
  if (!serviceName) return true
  const pref = syncSettings.servicePreferences.value.find(
    p => p?.service_name?.toLowerCase() === serviceName.toLowerCase()
  )
  return pref ? pref.auto_import_enabled : true
}

async function loadServicesAndAccounts() {
  loadingAccounts.value = true;
  try {
    const [servicesData, accountsData, statusesData] = await Promise.all([
      getServices().catch(() => []),
      getAccounts().catch(() => []),
      getServiceStatuses().catch(() => []),
    ]);
    services.value = Array.isArray(servicesData) ? servicesData : [];
    accounts.value = Array.isArray(accountsData) ? accountsData : [];
    serviceStatuses.value = Array.isArray(statusesData) ? statusesData : [];
    serviceDownloadEnabled.value = {};
  } catch (err) {
    console.error('Failed to load accounts:', err)
  } finally {
    loadingAccounts.value = false;
  }
}

async function handleConnect(serviceIdOrName?: string | number) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) {
    showServiceModal.value = true
    return
  }
  try {
    const result = await startAuthAndSave(name)
    if (result.success) {
      await loadServicesAndAccounts()
    } else {
      showServiceModal.value = true
    }
  } catch (err) {
    console.error(`Failed to connect ${name}:`, err)
    showServiceModal.value = true
  }
}

async function handleDisconnect(serviceIdOrName?: string | number, accountId?: number) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) return
  const id = accountId ?? getCurrentAccount(name)?.id
  if (id == null) return
  try {
    // Desconectar retains the account and its library; deletion is separate.
    await logoutService(name, id)
    await loadServicesAndAccounts()
  } catch (err) {
    console.error(`Failed to disconnect ${name}:`, err)
  }
}

async function activateAccount(accountId: number) {
  try {
    await setActiveAccount(accountId)
    await loadServicesAndAccounts()
  } catch (err) {
    console.error('Failed to activate account:', err)
  }
}

async function deleteAccount(accountId: number) {
  if (!window.confirm('¿Eliminar esta cuenta y sus datos de biblioteca y playlists? Esta acción no se puede deshacer.')) return
  try {
    await removeAccount(accountId)
    await loadServicesAndAccounts()
  } catch (err) {
    console.error('Failed to delete account:', err)
  }
}

async function handleReauth(serviceIdOrName?: string | number) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) return
  try {
    const result = await startAuthAndSave(name)
    if (result.success) {
      await loadServicesAndAccounts()
    } else {
      showServiceModal.value = true
    }
  } catch (err) {
    console.error(`Failed to re-authenticate ${name}:`, err)
    showServiceModal.value = true
  }
}

async function handleToggleEnabled(serviceIdOrName: string | number, enabled: boolean) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) return

  const acct = getCurrentAccount(name)
  if (!acct) return
  try {
    if (enabled) await setActiveAccount(acct.id)
    else await toggleAccountActive(acct.id, false)
    await loadServicesAndAccounts()
  } catch (err) {
    console.error(`Failed to toggle account active for ${name}:`, err)
  }
}

async function handleToggleAutoImport(serviceIdOrName: string | number, enabled: boolean) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) return
  try {
    await syncSettings.updateAutoImport(name, enabled)
  } catch (err) {
    console.error(`Failed to toggle auto import for ${name}:`, err)
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

const draggingPriorityIndex = ref<number | null>(null)

function handleDragStart(fromIndex: number) {
  draggingPriorityIndex.value = fromIndex
}

async function handleDragReorder(toIndex: number) {
  const fromIndex = draggingPriorityIndex.value
  draggingPriorityIndex.value = null
  if (fromIndex === null || fromIndex === toIndex) return
  if (toIndex < 0 || toIndex >= orderedServicePreferences.value.length) return
  const newPreferences = [...orderedServicePreferences.value]
  const [moved] = newPreferences.splice(fromIndex, 1)
  newPreferences.splice(toIndex, 0, moved)
  await syncSettings.reorderPriorities(newPreferences.map(p => p.service_name))
}

async function toggleAutoImport(serviceName: string) {
  const pref = orderedServicePreferences.value.find(p => p.service_name === serviceName)
  if (pref) {
    await handleToggleAutoImport(serviceName, !pref.auto_import_enabled)
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
