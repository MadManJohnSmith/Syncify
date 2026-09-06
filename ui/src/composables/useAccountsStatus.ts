/**
 * useAccountsStatus - Composable for service account status management
 * 
 * Extracted from AccountsView.vue (S44) to reduce component complexity.
 * Handles: fetching service statuses, reactive error handling and toast notifications (TASK-16),
 * computed service list with UI styling, and formatting helpers.
 */

import { ref, computed } from 'vue'
import { accountsApi } from '@/api/accounts'
import { useToast } from '@/composables/useToast'
import type { Service, Account, ServiceStatus } from '@/api/types'

// Service icon/style mappings
const SERVICE_ICONS: Record<string, string> = {
  'spotify': '🎵',
  'apple': '🍎',
  'apple_music': '🍎',
  'qobuz': '🎧',
  'tidal': '🌊',
  'deezer': '🎶',
  'soundcloud': '☁️',
}

const SERVICE_BG_CLASSES: Record<string, string> = {
  'spotify': 'bg-[#1ed760]/10',
  'apple': 'bg-[#fa243c]/10',
  'apple_music': 'bg-[#fa243c]/10',
  'qobuz': 'bg-[#1a8fe3]/10',
  'tidal': 'bg-[#00d4aa]/10',
  'deezer': 'bg-[#ff0092]/10',
  'soundcloud': 'bg-[#ff5500]/10',
}

export type ServiceAccountStatus = 'connected' | 'disconnected' | 'expiring' | 'invalid' | 'syncing' | 'error'

export interface ServiceCardData {
  id: string
  name: string
  icon: string
  bgClass: string
  status: 'connected' | 'disconnected' | 'expiring' | 'invalid'
  importedTracks: string
  favoriteTracks: string
  playlists: string
  lastSync: string
  email: string
  invalidReason?: string | null
  lastAuthError?: string | null
}

export function useAccountsStatus(customToast?: ReturnType<typeof useToast>) {
  const toast = customToast ?? useToast()
  const loading = ref(false)
  const isLoading = loading
  const error = ref<string | null>(null)
  const hasError = computed<boolean>(() => error.value !== null)

  const rawServices = ref<Service[]>([])
  const rawAccounts = ref<Account[]>([])
  const serviceStatuses = ref<ServiceStatus[]>([])

  // Computed services with UI styling
  const services = computed<ServiceCardData[]>(() => {
    return serviceStatuses.value.map(status => {
      // Determine valid session status based on backend validation
      let cardStatus: 'connected' | 'disconnected' | 'expiring' | 'invalid' = 'disconnected'
      if (status.credentials_invalid) {
        cardStatus = 'invalid'
      } else if (status.connected) {
        cardStatus = 'connected'
      } else {
        cardStatus = 'disconnected'
      }

      return {
        id: status.name.toLowerCase(),
        name: status.name.charAt(0).toUpperCase() + status.name.slice(1),
        icon: getServiceIcon(status.name),
        bgClass: getServiceBgClass(status.name),
        status: cardStatus,
        importedTracks: (status.library_count ?? 0).toLocaleString(),
        favoriteTracks: (status.favorites_count ?? 0).toLocaleString(),
        playlists: (status.playlists_count ?? 0).toLocaleString(),
        lastSync: status.last_synced ? formatTimeAgo(status.last_synced) : 'Never',
        email: status.account_email || '',
        invalidReason: status.invalid_reason,
        lastAuthError: status.last_auth_error,
      }
    })
  })

  function getServiceIcon(name: string): string {
    return SERVICE_ICONS[name.toLowerCase()] || '🎵'
  }

  function getServiceBgClass(name: string): string {
    return SERVICE_BG_CLASSES[name.toLowerCase()] || 'bg-gray-500/10'
  }

  function formatTimeAgo(dateStr: string): string {
    const date = new Date(dateStr)
    const now = new Date()
    const diff = now.getTime() - date.getTime()
    const hours = Math.floor(diff / (1000 * 60 * 60))
    if (hours < 1) return 'Just now'
    if (hours < 24) return `${hours}h ago`
    const days = Math.floor(hours / 24)
    if (days === 1) return '1 day ago'
    return `${days} days ago`
  }

  /** Fetch all service data from backend with reactive error handling */
  async function fetchData() {
    isLoading.value = true
    error.value = null
    try {
      const [servicesData, accountsData, statusesData] = await Promise.all([
        accountsApi.getServices(),
        accountsApi.getAccounts(),
        accountsApi.getServiceStatuses(),
      ])
      rawServices.value = servicesData
      rawAccounts.value = accountsData
      serviceStatuses.value = statusesData
      error.value = null
    } catch (e: unknown) {
      const errorMessage = e instanceof Error 
        ? e.message 
        : (typeof e === 'string' ? e : 'Error al cargar el estado de las cuentas')
      error.value = errorMessage
      console.error('Failed to fetch accounts data:', e)
      toast.error('Error al cargar cuentas', errorMessage)
    } finally {
      isLoading.value = false
    }
  }

  /** Retry or refresh accounts data */
  const retry = fetchData
  const refreshAccounts = fetchData

  /** Find account for a given service ID */
  function findAccountForService(serviceId: string): Account | undefined {
    return rawAccounts.value.find(a => {
      const service = rawServices.value.find(s => s.id === a.service_id)
      return service && service.name.toLowerCase() === serviceId.toLowerCase()
    })
  }

  return {
    loading,
    isLoading,
    error,
    hasError,
    rawServices,
    rawAccounts,
    serviceStatuses,
    services,
    fetchData,
    refreshAccounts,
    retry,
    findAccountForService,
    getServiceIcon,
    getServiceBgClass,
    formatTimeAgo,
  }
}
