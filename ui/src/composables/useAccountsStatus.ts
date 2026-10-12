/**
 * useAccountsStatus - Composable for service account status management
 *
 * Extracted from AccountsView.vue (S44) to reduce component complexity.
 * Handles: fetching service statuses, reactive error handling and toast notifications (TASK-16),
 * computed service list with UI styling, and formatting helpers.
 *
 * This module also owns the account identity helpers (`resolveAccountIdentity`,
 * `formatAccountLabel`, ...). They are exported here — and imported by
 * SettingsServices.vue — because this composable is the single owner of the
 * account-status data model: it is the only place identity is resolved, so
 * AccountsView and SettingsServices cannot diverge on what an account is
 * labelled as. Identity rules: validated email first, validated human
 * display_name second, otherwise "Identidad no proporcionada" — never an id,
 * a cookie-like string, or a token.
 */

import { ref, computed } from 'vue'
import { accountsApi } from '@/api/accounts'
import { useToast } from '@/composables/useToast'
import type { Service, Account, ServiceStatus, StoredCredentialState } from '@/api/types'

// Service icon/style mappings.
// Icons use the canonical Material Symbols set (the same vocabulary used by
// the settings cards) instead of arbitrary emoji, so Accounts cards look like
// the rest of the app and stay consistent across platforms/fonts.
export const SERVICE_ICON_SYMBOLS: Record<string, string> = {
  'spotify': 'library_music',
  'apple': 'music_note',
  'apple_music': 'music_note',
  'qobuz': 'album',
  'tidal': 'waves',
  'deezer': 'graphic_eq',
  'soundcloud': 'cloud_queue',
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
  /** @deprecated use identityLabel — raw backend string, may be opaque. */
  email: string
  accounts: Account[]
  invalidReason?: string | null
  lastAuthError?: string | null
  // Identity of the account shown on the card (resolved with the shared rules).
  /** Safe identity label for the primary account of this service. */
  identityLabel: string
  /** Email of the primary account when it is a validated identity, else null. */
  identityEmail: string | null
  identitySource: IdentitySource
  /** Number of accounts linked to this service. */
  accountCount: number
  /** Whether the service has more than one account to manage. */
  hasMultipleAccounts: boolean
  /** id of the account the card reports, or null when there is none. */
  primaryAccountId: number | null
}

// ==============================================
// ACCOUNT IDENTITY (shared source of truth)
// ==============================================

export interface AccountIdentityInput {
  display_name?: string | null
  email?: string | null
  is_active?: boolean | null
  id?: number | null
  credentials_invalid?: boolean | null
  /** Masked stored-credential state. Never a credential value. */
  credential_state?: StoredCredentialState | null
}

/** An account row as it is listed in the UI: an identity plus its row id. */
export interface AccountRow extends AccountIdentityInput {
  id: number
}

export type IdentitySource = 'email' | 'display_name' | 'none'

export interface ResolvedIdentity {
  /** Exactly what the UI should render for this account. */
  label: string
  /** Validated email, when displayed. Email is identity, never a secret. */
  email: string | null
  /** Validated human display name, when present. */
  displayName: string | null
  /** Which field the label came from. */
  source: IdentitySource
}

/** Shown whenever the backend provided no usable identity. */
export const NO_IDENTITY_LABEL = 'Identidad no proporcionada'

/** Syntactically valid email (email is identity data, not a credential). */
export function isValidEmail(value: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
}

/**
 * SP-03: detects opaque identifiers and credential-like placeholders that must
 * never be rendered as if they were a person's identity.
 *
 * Matches long numeric ids, long hex/uuid-ish strings, internal `usr_*`
 * handles, and session/cookie/token placeholder names.
 */
export function isOpaqueIdentityString(value?: string | null): boolean {
  if (!value) return false
  const trimmed = value.trim()
  if (!trimmed) return false
  // Identificadores de proveedor, placeholders internos y sesiones/cookies
  // nunca son una identidad pública legible.
  return /^\d{5,}$/.test(trimmed)
    || /^[0-9a-fA-F-]{20,}$/.test(trimmed)
    || /^usr_[0-9a-zA-Z]{12,}$/.test(trimmed)
    || /^(browser[_ -]?session|cookies?|cookie[_ -]?jar|session[_ -]?token|bearer)$/i.test(trimmed)
    || /^(sp_dc|sp_key|arl|auth_token|user_auth_token)\s*[:=]/i.test(trimmed)
}

/** The account email, but only when it is safe to display as identity. */
export function safeEmail(account: AccountIdentityInput | null | undefined): string | null {
  const email = account?.email
  if (!email) return null
  const trimmed = email.trim()
  if (!trimmed || isOpaqueIdentityString(trimmed) || !isValidEmail(trimmed)) return null
  return trimmed
}

/**
 * The account display name, but only when it is a human-readable name.
 *
 * A display_name that simply repeats the real email is dropped (the email slot
 * already shows it); an email stored in the display_name slot when there is no
 * separate email is still valid identity.
 */
export function safeDisplayName(account: AccountIdentityInput | null | undefined): string | null {
  const name = account?.display_name
  if (!name) return null
  const trimmed = name.trim()
  if (!trimmed || isOpaqueIdentityString(trimmed)) return null
  const realEmail = safeEmail(account)
  if (trimmed.includes('@')) {
    return realEmail === null && isValidEmail(trimmed) ? trimmed : null
  }
  return trimmed
}

/**
 * Resolve a safe identity for an account. The returned `label` is exactly what
 * the UI renders — callers never concatenate the parts again, which keeps
 * AccountsView and SettingsServices from diverging.
 */
export function resolveAccountIdentity(
  account: AccountIdentityInput | null | undefined,
): ResolvedIdentity {
  const email = safeEmail(account)
  const displayName = safeDisplayName(account)
  if (email && displayName) {
    return { label: `${displayName} (${email})`, email, displayName, source: 'email' }
  }
  if (email) {
    return { label: email, email, displayName: null, source: 'email' }
  }
  if (displayName) {
    return { label: displayName, email: null, displayName, source: 'display_name' }
  }
  return { label: NO_IDENTITY_LABEL, email: null, displayName: null, source: 'none' }
}

/** Convenience wrapper returning only the display label. */
export function formatAccountLabel(account: AccountIdentityInput | null | undefined): string {
  return resolveAccountIdentity(account).label
}

/**
 * Account a service shows as "the" account on screen: the active one, else the
 * first real row, so a card never renders an empty identity while rows exist.
 */
export function pickPrimaryAccount<T extends AccountIdentityInput>(
  accounts: readonly T[] | null | undefined,
): T | null {
  if (!Array.isArray(accounts) || accounts.length === 0) return null
  const active = accounts.find(a => a && a.is_active === true)
  if (active) return active
  return accounts.find(a => a) ?? null
}

/** True when a service has more than one account to manage. */
export function hasMultipleAccounts(accounts: readonly AccountIdentityInput[] | null | undefined): boolean {
  return Array.isArray(accounts) && accounts.length > 1
}

/**
 * Sort accounts for the "active-account" UI: the active account first, then
 * invalid ones (they need attention), then the rest. Never reorders by id and
 * never drops a row, so the visible order is stable across refreshes.
 */
export function orderedAccounts<T extends AccountRow>(accounts: readonly T[] | null | undefined): T[] {
  if (!Array.isArray(accounts)) return []
  const rank = (a: T) => (a?.is_active ? 0 : (a?.credentials_invalid ? 1 : 2))
  return [...accounts].sort((a, b) => rank(a) - rank(b))
}

/**
 * Secondary email line for an account row, or null.
 *
 * Only rendered when it is real, validated identity AND it is not already part
 * of the label — showing "Ana (ana@x.com)" followed by "ana@x.com" is the exact
 * duplicate-label rendering the accounts UI is supposed to avoid. Shared by
 * AccountsView and SettingsServices so both drop it under the same condition.
 */
export function accountRowEmail(account: AccountIdentityInput | null | undefined): string | null {
  if (!account) return null
  const identity = resolveAccountIdentity(account)
  if (!identity.email) return null
  // Already visible inside the label: either as the label itself
  // (`source === 'email'`) or as the "Name (email)" form.
  if (identity.label.includes(identity.email)) return null
  return identity.email
}

// ==============================================
// STORED CREDENTIAL STATE (masked — never a secret)
// ==============================================

/**
 * The masked credential state of an account.
 *
 * Copy of the backend `credential_state` when the backend sent it; otherwise a
 * conservative default that claims nothing, so an older backend cannot make the
 * UI report a stored credential the app never verified.
 */
export function storedCredentialState(
  account: AccountIdentityInput & { credential_state?: StoredCredentialState | null } | null | undefined,
): StoredCredentialState {
  const state = account?.credential_state
  if (!state) {
    return {
      kind: 'none',
      has_password: false,
      has_cookie: false,
      has_token: false,
      has_credential: false,
      auto_refreshable: false,
    }
  }
  return state
}

/** One-line, secret-free description of what an account stores. */
export function describeCredentialState(account: AccountIdentityInput | null | undefined): string {
  const state = storedCredentialState(account)
  if (!state.has_credential) return 'No stored credential'
  switch (state.kind) {
    case 'password':
      // Deliberately does NOT say the password is recoverable: it only says one
      // is on file. The value itself can never be displayed or exported.
      return 'Password on file (never shown)'
    case 'cookie':
      return 'Browser session cookie (expires; reconnect to renew)'
    case 'token':
      return 'Saved API token'
    case 'oauth':
      return state.auto_refreshable ? 'OAuth session (renews automatically)' : 'OAuth session'
    case 'mixed':
      return 'Multiple credentials on file'
    default:
      return 'Credential on file'
  }
}

/**
 * Material Symbols icon name for the credential kind, so the accounts card and
 * the settings list speak the same visual vocabulary.
 */
export function credentialStateIcon(account: AccountIdentityInput | null | undefined): string {
  const { kind } = storedCredentialState(account)
  switch (kind) {
    case 'password':
      return 'password'
    case 'cookie':
      return 'cookie'
    case 'token':
      return 'key'
    case 'oauth':
      return 'sync_lock'
    case 'mixed':
      return 'vpn_key'
    default:
      return 'lock_open'
  }
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

      const serviceAccounts = accountsForService(status.name)
      const identity = resolveAccountIdentity(pickPrimaryAccount(serviceAccounts))

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
        accounts: serviceAccounts,
        invalidReason: status.invalid_reason,
        lastAuthError: status.last_auth_error,
        identityLabel: identity.label,
        identityEmail: identity.email,
        identitySource: identity.source,
        accountCount: serviceAccounts.length,
        hasMultipleAccounts: serviceAccounts.length > 1,
        primaryAccountId: pickPrimaryAccount(serviceAccounts)?.id ?? null,
      }
    })
  })

  function getServiceIcon(name: string): string {
    return SERVICE_ICON_SYMBOLS[name.toLowerCase()] || 'cloud'
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

  /** List every account belonging to a service, including inactive ones. */
  function accountsForService(serviceId: string): Account[] {
    const service = rawServices.value.find(s => s.name.toLowerCase() === serviceId.toLowerCase())
    return service ? rawAccounts.value.filter(a => a.service_id === service.id) : []
  }

  function findAccountForService(serviceId: string): Account | undefined {
    const matches = accountsForService(serviceId)
    return matches.find(a => a.is_active) ?? matches[0]
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
    accountsForService,
    getServiceIcon,
    getServiceBgClass,
    formatTimeAgo,
    NO_IDENTITY_LABEL,
    resolveAccountIdentity,
    formatAccountLabel,
    pickPrimaryAccount,
    hasMultipleAccounts,
    orderedAccounts,
    accountRowEmail,
    isOpaqueIdentityString,
    isValidEmail,
    safeEmail,
    safeDisplayName,
    // Masked stored-credential state.
    storedCredentialState,
    describeCredentialState,
    credentialStateIcon,
  }
}
