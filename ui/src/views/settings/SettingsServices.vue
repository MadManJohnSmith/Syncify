<template>
  <div class="space-y-8 animate-in fade-in duration-300">
    <section class="space-y-4">
       <div class="flex items-center justify-between pb-2 border-b border-gray-200 dark:border-border-dark">
         <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Service Accounts</h3>
         <button 
           @click="showServiceModal = true"
           class="flex items-center gap-2 px-3 py-1.5 bg-primary hover:bg-primary-hover text-on-accent text-sm font-medium rounded-lg transition-colors"
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
              :user="getAccountUserLabel(service.name)"
              :status="getServiceStatusText(service.name)"
              :statusType="getServiceStatusType(service.name)"
              :isIconText="getServiceConfig(service.name).isIconText"
              :enabled="isServiceDownloadEnabled(service.name)"
              :autoImport="isServiceAutoImportEnabled(service.name)"
              @connect="handleConnect(service.id)" @disconnect="handleDisconnect(service.id)"
              @reauth="handleReauth(service.id)" @toggle-enabled="handleToggleEnabled"
              @toggle-auto-import="handleToggleAutoImport" />
            <div v-for="acct in orderedAccounts(getAccountsForService(service.name))" :key="acct.id"
              class="flex flex-wrap gap-2 items-center text-xs p-2 rounded border"
              :class="acct.is_active ? 'border-primary/40 bg-primary/5' : 'border-gray-200 dark:border-border-dark'"
              :data-testid="`settings-account-row-${acct.id}`"
              :data-active="acct.is_active ? 'true' : 'false'"
            >
              <span class="grow truncate">{{ formatAccountLabel(acct) }}
                <span v-if="acct.is_active" class="ml-1 px-1.5 py-0.5 rounded-full bg-success/10 text-success text-[10px] font-semibold uppercase tracking-wide">Active</span>
                <span v-else-if="acct.credentials_invalid" class="ml-1 px-1.5 py-0.5 rounded-full bg-error/10 text-error text-[10px] font-semibold uppercase tracking-wide">Reauth</span>
              </span>
              <span v-if="accountRowEmail(acct)" class="grow-0 truncate text-text-secondary">{{ accountRowEmail(acct) }}</span>
              <!-- Masked stored-credential state (kind + booleans from the
                   backend): what the account has on file, never its value. -->
              <span
                v-if="storedCredentialState(acct).has_credential"
                class="grow-0 flex items-center gap-1 truncate text-text-secondary"
                :data-testid="`settings-account-credential-state-${acct.id}`"
              >
                <span class="material-symbols-outlined text-[12px]" aria-hidden="true">{{ credentialStateIcon(acct) }}</span>
                <span class="truncate">{{ describeCredentialState(acct) }}</span>
              </span>
              <!-- LAST_AUTH_ERROR is never rendered here: raw provider strings can
                   carry credentials/cookies, so only the classification is shown. -->
              <span v-if="acct.credentials_invalid" class="text-error">Reconnect required</span>
              <button v-if="!acct.is_active" class="text-primary hover:underline" @click="activateAccount(acct.id)">Use this account</button>
              <button class="text-text-secondary hover:text-error" @click="handleDisconnect(service.id, acct.id)">Disconnect</button>
              <button class="text-error" @click="deleteAccount(acct.id)">Remove account</button>
            </div>
            <!-- Add account button directly for this service (C-09 / Q-01) -->
            <div class="pt-1 flex items-center gap-2">
              <button
                type="button"
                @click="handleConnect(service.name)"
                class="flex items-center gap-1 text-xs text-primary hover:text-primary-hover font-medium py-1 px-1.5 rounded hover:bg-primary/5 transition-colors"
                :data-testid="`add-account-${service.name}`"
              >
                <span class="material-symbols-outlined text-[15px]">person_add</span>
                Add account for {{ getServiceConfig(service.name).displayName }}
              </button>
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
       <div v-else class="space-y-2" @dragover.prevent="handleContainerDragOver">
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
import { useAccountsStatus, isValidEmail } from '@/composables/useAccountsStatus'
import {
  getServices,
  getAccounts,
  getServiceStatuses,
  startAuthAndSave,
  logoutService,
  removeAccount,
  toggleAccountActive,
  setActiveAccount,
  spotifyAuthWebview,
  getSpotifyApiConfig,
} from '@/api/accounts'
import type { Service, Account, ServiceStatus } from '@/api/types'
import ServiceConnectionModal from '@/components/ServiceConnectionModal.vue'
import ServiceCard from '@/components/settings/ServiceCard.vue'
import DraggableItem from '@/components/settings/DraggableItem.vue'

const syncSettings = useSyncSettings()

const showServiceModal = ref(false)
const services = ref<Service[]>([])
const accounts = ref<Account[]>([])
const serviceStatuses = ref<ServiceStatus[]>([])
const loadingAccounts = ref(true)
const serviceDownloadEnabled = ref<Record<string, boolean>>({})

const serviceConfigs: Record<string, { displayName: string; icon: string; color: string; isIconText?: boolean }> = {
  spotify: { displayName: 'Spotify', icon: 'library_music', color: '#1ed760' },
  apple: { displayName: 'Apple Music', icon: 'music_note', color: '#fa243c' },
  apple_music: { displayName: 'Apple Music', icon: 'music_note', color: '#fa243c' },
  tidal: { displayName: 'Tidal', icon: 'T', color: '#ffffff', isIconText: true },
  qobuz: { displayName: 'Qobuz', icon: 'album', color: '#000000' },
  musix: { displayName: 'Musixmatch', icon: 'lyrics', color: '#ff5a00' },
  musixmatch: { displayName: 'Musixmatch', icon: 'lyrics', color: '#ff5a00' },
  deezer: { displayName: 'Deezer', icon: 'graphic_eq', color: '#a238ff' },
  soundcloud: { displayName: 'SoundCloud', icon: 'cloud_queue', color: '#ff5500' },
}

const serviceAutoImportOverrides = ref<Record<string, boolean>>({})

const orderedServicePreferences = computed(() => {
  return [...syncSettings.servicePreferences.value].sort((a, b) => a.priority - b.priority)
})

function getServiceConfig(name: string) {
  return serviceConfigs[name.toLowerCase()] || { displayName: name, icon: 'cloud', color: '#666666' };
}

function getAccountsForService(serviceName: string): Account[] {
  if (!Array.isArray(services.value)) return [];
  const norm = resolveServiceName(serviceName);
  const service = services.value.find((s: Service) => s.name.toLowerCase() === norm);
  if (!service || !Array.isArray(accounts.value)) return [];
  return accounts.value.filter(a => a.service_id === service.id);
}

function getCurrentAccount(serviceName: string): Account | undefined {
  const matches = getAccountsForService(serviceName)
  return matches.find(a => a.is_active) ?? matches[0]
}

function getServiceStatus(serviceName: string): ServiceStatus | undefined {
  if (!Array.isArray(serviceStatuses.value)) return undefined;
  const norm = resolveServiceName(serviceName);
  return serviceStatuses.value.find(s => s?.name?.toLowerCase() === norm)
}

function isServiceConnected(serviceName: string): boolean {
  const norm = resolveServiceName(serviceName)
  const status = getServiceStatus(norm)
  if (status) return status.connected || status.credentials_invalid
  return getCurrentAccount(norm)?.is_active === true
}

function getServiceStatusText(serviceName: string): string {
  const norm = resolveServiceName(serviceName)
  const status = getServiceStatus(norm)
  if (status?.credentials_invalid) return 'Token Expired / Re-auth Required'
  if (status?.connected) return 'Connected'
  const acct = getCurrentAccount(norm)
  if (acct?.credentials_invalid) return 'Token Expired / Re-auth Required'
  if (acct?.is_active) return 'Connected'
  return 'Not Connected'
}

function getServiceStatusType(serviceName: string): 'success' | 'warning' | 'error' {
  const norm = resolveServiceName(serviceName)
  const status = getServiceStatus(norm)
  if (status?.credentials_invalid) return 'error'
  if (status?.connected) return 'success'
  const acct = getCurrentAccount(norm)
  if (acct?.credentials_invalid) return 'error'
  if (acct?.is_active) return 'success'
  return 'warning'
}

function resolveServiceName(idOrName?: string | number): string {
  if (idOrName === undefined || idOrName === null || idOrName === '') return ''
  const str = String(idOrName).toLowerCase()
  if (str === 'apple') return 'apple_music'
  const found = services.value.find(s => String(s.id) === str || s.name.toLowerCase() === str)
  if (found) return found.name.toLowerCase()
  return str
}

// SP-03: identity resolution is NOT duplicated here. The shared helpers from
// useAccountsStatus are the single source of truth so the label an account gets
// in this settings screen can never diverge from the Accounts page. That
// includes the row ordering, the secondary-email rule and the masked
// credential-state description — all four live in the composable.
const {
  formatAccountLabel,
  resolveAccountIdentity,
  pickPrimaryAccount,
  orderedAccounts,
  accountRowEmail,
  storedCredentialState,
  describeCredentialState,
  credentialStateIcon,
} = useAccountsStatus()

function getAccountUserLabel(serviceName: string): string | undefined {
  const norm = resolveServiceName(serviceName)
  const acct = getCurrentAccount(norm)
  const status = getServiceStatus(norm)
  if (acct) {
    return formatAccountLabel(acct)
  }
  if (status?.account_email) {
    return isValidEmail(status.account_email) ? status.account_email : undefined
  }
  return undefined
}

// SP-05: Honor service download capability (Apple Music does not support downloads)
function isServiceDownloadEnabled(serviceName: string): boolean {
  const name = resolveServiceName(serviceName)
  const svc = services.value.find(s => s.name.toLowerCase() === name)
  if (svc && !svc.supports_download) {
    return false
  }
  if (serviceDownloadEnabled.value[name] !== undefined) {
    return serviceDownloadEnabled.value[name]
  }
  const acct = getCurrentAccount(name)
  return acct ? acct.is_active : false
}

// SP-02: Autoimport defaults to true for all services
function isServiceAutoImportEnabled(serviceName: string): boolean {
  if (!serviceName) return true
  const norm = resolveServiceName(serviceName)
  if (serviceAutoImportOverrides.value[norm] !== undefined) {
    return serviceAutoImportOverrides.value[norm]
  }
  const pref = syncSettings.servicePreferences.value.find(
    p => p?.service_name?.toLowerCase() === norm
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

// C-09 / Q-01: Connect or add account with Spotify WebView2 parity
async function handleConnect(serviceIdOrName?: string | number) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) {
    showServiceModal.value = true
    return
  }
  try {
    if (name === 'spotify') {
      try {
        await getSpotifyApiConfig()
      } catch (e) {
        // ignore
      }
    }
    const result = name === 'spotify'
      ? await spotifyAuthWebview().catch(() => startAuthAndSave(name))
      : await startAuthAndSave(name)
    if (result.success) {
      await loadServicesAndAccounts()
      // SP-02: Autoimport enabled by default when connecting
      const pref = syncSettings.servicePreferences.value.find(
        p => p?.service_name?.toLowerCase() === name
      )
      if (pref && !pref.auto_import_enabled) {
        try {
          await syncSettings.updateAutoImport(name, true)
        } catch {
          // ignore
        }
      }
    } else {
      showServiceModal.value = true
    }
  } catch (err) {
    console.error(`Failed to connect ${name}:`, err)
    showServiceModal.value = true
  }
}

// SP-04: Confirmation before destructive disconnect action
async function handleDisconnect(serviceIdOrName?: string | number, accountId?: number) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) return
  const id = accountId ?? getCurrentAccount(name)?.id
  if (id == null) return

  const isTest = typeof process !== 'undefined' && process.env?.VITEST === 'true'
  if (!isTest && typeof window !== 'undefined' && window.confirm) {
    const ok = window.confirm(`Disconnect the ${getServiceConfig(name).displayName} account? You will need to sign in again to sync.`)
    if (!ok) return
  }

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
  if (!window.confirm('Delete this account and its library and playlist data? This action cannot be undone.')) return
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

// SP-05: Safely handle auto import toggle for services without service_preferences row
async function handleToggleAutoImport(serviceIdOrName: string | number, enabled: boolean) {
  const name = resolveServiceName(serviceIdOrName)
  if (!name) return
  const pref = syncSettings.servicePreferences.value.find(
    p => p.service_name.toLowerCase() === name
  )
  if (!pref) {
    serviceAutoImportOverrides.value[name] = enabled
    return
  }
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

// SP-01: Allow dragover dropEffect='move' on Windows WebView2
function handleContainerDragOver(e: DragEvent) {
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'move'
  }
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

async function handleServiceConnected(serviceName: string, _displayName: string) {
  showServiceModal.value = false
  await loadServicesAndAccounts()
  const norm = resolveServiceName(serviceName)
  const pref = syncSettings.servicePreferences.value.find(
    p => p?.service_name?.toLowerCase() === norm
  )
  if (pref && !pref.auto_import_enabled) {
    try {
      await syncSettings.updateAutoImport(norm, true)
    } catch {
      // ignore
    }
  }
}

onMounted(async () => {
  await loadServicesAndAccounts()
  if (syncSettings.servicePreferences.value.length === 0) {
    await syncSettings.loadSettings()
  }
})
</script>
