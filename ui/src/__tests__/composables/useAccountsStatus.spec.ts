/**
 * Unit tests for useAccountsStatus composable (TASK-16)
 * Verifies reactive error handling, loading state lifecycle, retry mechanisms, and toast notifications.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest'
import { useAccountsStatus } from '@/composables/useAccountsStatus'
import { accountsApi } from '@/api/accounts'
import type { Service, Account, ServiceStatus } from '@/api/types'

const mockToast = {
  error: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
  info: vi.fn(),
}

vi.mock('@/composables/useToast', () => ({
  useToast: () => mockToast,
}))

describe('useAccountsStatus Composable (TASK-16)', () => {
  const sampleServices: Service[] = [
    { id: 1, name: 'spotify', supports_download: 1, max_quality: '320kbps' },
    { id: 2, name: 'qobuz', supports_download: 1, max_quality: 'hires' },
  ]

  const sampleAccounts: Account[] = [
    {
      id: 1,
      service_id: 1,
      service_name: 'spotify',
      display_name: 'Spotify User',
      email: 'user@spotify.com',
      is_active: true,
      last_synced: '2026-08-20T10:00:00Z',
      created_at: '2026-08-01T00:00:00Z',
      credentials_invalid: false,
    },
  ]

  const sampleStatuses: ServiceStatus[] = [
    {
      name: 'spotify',
      connected: true,
      credentials_invalid: false,
      library_count: 150,
      favorites_count: 50,
      playlists_count: 5,
      last_synced: '2026-08-20T10:00:00Z',
      account_email: 'user@spotify.com',
      invalid_reason: null,
      last_auth_error: null,
    },
    {
      name: 'qobuz',
      connected: false,
      credentials_invalid: true,
      library_count: 0,
      favorites_count: 0,
      playlists_count: 0,
      last_synced: null,
      account_email: null,
      invalid_reason: 'Token expired',
      last_auth_error: 'Auth expired',
    },
  ]

  beforeEach(() => {
    vi.clearAllMocks()
    vi.restoreAllMocks()
  })

  it('initializes with clean reactive error and idle loading state', () => {
    const { error, hasError, isLoading, loading, rawServices, rawAccounts, serviceStatuses, services } = useAccountsStatus()

    expect(error.value).toBeNull()
    expect(hasError.value).toBe(false)
    expect(isLoading.value).toBe(false)
    expect(loading.value).toBe(false)
    expect(rawServices.value).toEqual([])
    expect(rawAccounts.value).toEqual([])
    expect(serviceStatuses.value).toEqual([])
    expect(services.value).toEqual([])
  })

  it('updates error.value, sets isLoading to false, and triggers toast.error when accountsApi.getAccounts rejects', async () => {
    vi.spyOn(accountsApi, 'getServices').mockResolvedValue(sampleServices)
    vi.spyOn(accountsApi, 'getAccounts').mockRejectedValue(new Error('Tauri IPC backend failure'))
    vi.spyOn(accountsApi, 'getServiceStatuses').mockResolvedValue(sampleStatuses)

    const { error, hasError, isLoading, fetchData } = useAccountsStatus()

    await fetchData()

    expect(error.value).toBe('Tauri IPC backend failure')
    expect(hasError.value).toBe(true)
    expect(isLoading.value).toBe(false)
    expect(mockToast.error).toHaveBeenCalledTimes(1)
    expect(mockToast.error).toHaveBeenCalledWith('Error al cargar cuentas', 'Tauri IPC backend failure')
  })

  it('handles string rejection errors gracefully and exposes the error message', async () => {
    vi.spyOn(accountsApi, 'getServices').mockResolvedValue(sampleServices)
    vi.spyOn(accountsApi, 'getAccounts').mockRejectedValue('Connection timeout from SQLite')
    vi.spyOn(accountsApi, 'getServiceStatuses').mockResolvedValue(sampleStatuses)

    const { error, hasError, isLoading, fetchData } = useAccountsStatus()

    await fetchData()

    expect(error.value).toBe('Connection timeout from SQLite')
    expect(hasError.value).toBe(true)
    expect(isLoading.value).toBe(false)
    expect(mockToast.error).toHaveBeenCalledTimes(1)
    expect(mockToast.error).toHaveBeenCalledWith('Error al cargar cuentas', 'Connection timeout from SQLite')
  })

  it('clears error.value to null and populates accounts when backend returns successfully', async () => {
    vi.spyOn(accountsApi, 'getServices').mockResolvedValue(sampleServices)
    vi.spyOn(accountsApi, 'getAccounts').mockResolvedValue(sampleAccounts)
    vi.spyOn(accountsApi, 'getServiceStatuses').mockResolvedValue(sampleStatuses)

    const { error, hasError, isLoading, rawServices, rawAccounts, serviceStatuses, services, fetchData } = useAccountsStatus()

    await fetchData()

    expect(error.value).toBeNull()
    expect(hasError.value).toBe(false)
    expect(isLoading.value).toBe(false)
    expect(rawServices.value).toEqual(sampleServices)
    expect(rawAccounts.value).toEqual(sampleAccounts)
    expect(serviceStatuses.value).toEqual(sampleStatuses)
    expect(services.value.length).toBe(2)
    expect(services.value[0]).toMatchObject({
      id: 'spotify',
      name: 'Spotify',
      status: 'connected',
      email: 'user@spotify.com',
    })
    expect(services.value[1]).toMatchObject({
      id: 'qobuz',
      name: 'Qobuz',
      status: 'invalid',
      invalidReason: 'Token expired',
    })
    expect(mockToast.error).not.toHaveBeenCalled()
  })

  it('recovers reactively on retry() after a failed request, clearing previous error', async () => {
    const getAccountsSpy = vi.spyOn(accountsApi, 'getAccounts')
    vi.spyOn(accountsApi, 'getServices').mockResolvedValue(sampleServices)
    vi.spyOn(accountsApi, 'getServiceStatuses').mockResolvedValue(sampleStatuses)

    // First attempt fails
    getAccountsSpy.mockRejectedValueOnce(new Error('Network offline'))

    const { error, hasError, isLoading, retry, rawAccounts } = useAccountsStatus()

    await retry()

    expect(error.value).toBe('Network offline')
    expect(hasError.value).toBe(true)
    expect(isLoading.value).toBe(false)
    expect(rawAccounts.value).toEqual([])

    // Second attempt (retry) succeeds
    getAccountsSpy.mockResolvedValueOnce(sampleAccounts)

    await retry()

    expect(error.value).toBeNull()
    expect(hasError.value).toBe(false)
    expect(isLoading.value).toBe(false)
    expect(rawAccounts.value).toEqual(sampleAccounts)
  })

  it('allows refreshing accounts via refreshAccounts() alias', async () => {
    vi.spyOn(accountsApi, 'getServices').mockResolvedValue(sampleServices)
    vi.spyOn(accountsApi, 'getAccounts').mockResolvedValue(sampleAccounts)
    vi.spyOn(accountsApi, 'getServiceStatuses').mockResolvedValue(sampleStatuses)

    const { error, hasError, isLoading, refreshAccounts, rawAccounts } = useAccountsStatus()

    await refreshAccounts()

    expect(error.value).toBeNull()
    expect(hasError.value).toBe(false)
    expect(isLoading.value).toBe(false)
    expect(rawAccounts.value).toEqual(sampleAccounts)
  })

  it('correctly matches account for a given service ID via findAccountForService', async () => {
    vi.spyOn(accountsApi, 'getServices').mockResolvedValue(sampleServices)
    vi.spyOn(accountsApi, 'getAccounts').mockResolvedValue(sampleAccounts)
    vi.spyOn(accountsApi, 'getServiceStatuses').mockResolvedValue(sampleStatuses)

    const { fetchData, findAccountForService } = useAccountsStatus()
    await fetchData()

    const spotifyAccount = findAccountForService('spotify')
    expect(spotifyAccount).toBeDefined()
    expect(spotifyAccount?.email).toBe('user@spotify.com')

    const nonExistentAccount = findAccountForService('deezer')
    expect(nonExistentAccount).toBeUndefined()
  })

  it('formats time ago and service styles correctly', () => {
    const { formatTimeAgo, getServiceIcon, getServiceBgClass } = useAccountsStatus()

    expect(getServiceIcon('spotify')).toBe('🎵')
    expect(getServiceIcon('apple')).toBe('🍎')
    expect(getServiceIcon('unknown_service')).toBe('🎵')

    expect(getServiceBgClass('spotify')).toBe('bg-[#1ed760]/10')
    expect(getServiceBgClass('unknown_service')).toBe('bg-gray-500/10')

    const now = new Date().toISOString()
    expect(formatTimeAgo(now)).toBe('Just now')
  })
})
