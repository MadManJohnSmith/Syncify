/**
 * PROTOTIPO DESECHABLE — no forma parte del producto.
 *
 * Datos simulados a partir de la captura de `Connected Services` y funciones
 * puras para comparar CÓMO se calculan las cifras hoy vs. una propuesta.
 * No llama a Tauri, no toca SQLite y no persiste nada.
 */

export type ServiceKey = 'spotify' | 'qobuz' | 'tidal'
export type Completeness = 'complete' | 'partial'
export type Freshness = 'fresh' | 'aging' | 'stale' | 'never'
export type AccountState = 'ok' | 'partial' | 'stale' | 'never'

export interface ProtoSnapshot {
  collection: 'favorites'
  reported: number | null
  imported: number
  completeness: Completeness
  scannedAt: string
}

export interface ProtoAccount {
  id: number
  service: ServiceKey
  email: string | null
  displayName: string | null
  isActive: boolean
  credential: 'oauth' | 'password' | 'cookie'
  snapshot?: ProtoSnapshot
}

export interface ProtoService {
  key: ServiceKey
  name: string
  icon: string
  markClass: string
  barClass: string
  status: 'connected' | 'expiring' | 'invalid' | 'disconnected'
  library: {
    tracks: number
    favorites: number
    playlists: number
    lastSyncedAt: string | null
  }
  accounts: ProtoAccount[]
}

const MIN = 60_000
const NOW = Date.now()
const minutesAgo = (minutes: number): string => new Date(NOW - minutes * MIN).toISOString()

export const PROTO_NOW = NOW

export const PROTO_SERVICES: ProtoService[] = [
  {
    key: 'spotify',
    name: 'Spotify',
    icon: 'library_music',
    markClass: 'bg-[#1ed760]/15 text-[#1ed760]',
    barClass: 'bg-[#1ed760]',
    status: 'connected',
    library: {
      tracks: 20_087,
      favorites: 10_180,
      playlists: 131,
      lastSyncedAt: new Date(NOW - 2 * MIN).toISOString(),
    },
    accounts: [
      {
        id: 1,
        service: 'spotify',
        email: 'alandanielsalas@gmail.com',
        displayName: 'MadManJohnSmith',
        isActive: true,
        credential: 'oauth',
        snapshot: {
          collection: 'favorites',
          reported: 12_213,
          imported: 12_211,
          completeness: 'partial',
          scannedAt: minutesAgo(106),
        },
      },
      {
        id: 2,
        service: 'spotify',
        email: 'sony.p.flores@gmail.com',
        displayName: null,
        isActive: false,
        credential: 'password',
      },
    ],
  },
  {
    key: 'qobuz',
    name: 'Qobuz',
    icon: 'album',
    markClass: 'bg-[#79a7ff]/15 text-[#79a7ff]',
    barClass: 'bg-[#79a7ff]',
    status: 'connected',
    library: {
      tracks: 19_410,
      favorites: 9_620,
      playlists: 228,
      lastSyncedAt: new Date(NOW - 3 * MIN).toISOString(),
    },
    accounts: [
      {
        id: 3,
        service: 'qobuz',
        email: 'alansalasctf@gmail.com',
        displayName: null,
        isActive: true,
        credential: 'password',
        snapshot: {
          collection: 'favorites',
          reported: 11_370,
          imported: 11_368,
          completeness: 'partial',
          scannedAt: minutesAgo(100),
        },
      },
      {
        id: 4,
        service: 'qobuz',
        email: 'alandanielsalas@gmail.com',
        displayName: null,
        isActive: false,
        credential: 'password',
      },
      {
        id: 5,
        service: 'qobuz',
        email: 'als6411@thi.de',
        displayName: null,
        isActive: false,
        credential: 'password',
      },
      {
        id: 6,
        service: 'qobuz',
        email: 'qobuz7183@gmail.com',
        displayName: null,
        isActive: false,
        credential: 'password',
      },
    ],
  },
  {
    key: 'tidal',
    name: 'Tidal',
    icon: 'waves',
    markClass: 'bg-[#4bdbe6]/15 text-[#4bdbe6]',
    barClass: 'bg-[#4bdbe6]',
    status: 'connected',
    library: {
      tracks: 6_106,
      favorites: 5_304,
      playlists: 28,
      lastSyncedAt: new Date(NOW - 4 * MIN).toISOString(),
    },
    accounts: [
      {
        id: 7,
        service: 'tidal',
        email: 'alandanielsalas@gmail.com',
        displayName: null,
        isActive: true,
        credential: 'oauth',
        snapshot: {
          collection: 'favorites',
          reported: 7_480,
          imported: 6_675,
          completeness: 'complete',
          scannedAt: minutesAgo(124),
        },
      },
      {
        id: 8,
        service: 'tidal',
        email: 'alansalasctf@gmail.com',
        displayName: null,
        isActive: false,
        credential: 'oauth',
        snapshot: {
          collection: 'favorites',
          reported: 10_012,
          imported: 9_409,
          completeness: 'partial',
          scannedAt: minutesAgo(22 * 60 + 5),
        },
      },
      {
        id: 9,
        service: 'tidal',
        email: 'alansalasp4.18@gmail.com',
        displayName: null,
        isActive: false,
        credential: 'password',
        snapshot: {
          collection: 'favorites',
          reported: 1,
          imported: 1,
          completeness: 'complete',
          scannedAt: minutesAgo(22 * 60 - 7),
        },
      },
      {
        id: 10,
        service: 'tidal',
        email: 'qobuz7183@gmail.com',
        displayName: null,
        isActive: false,
        credential: 'password',
        snapshot: {
          collection: 'favorites',
          reported: 0,
          imported: 0,
          completeness: 'complete',
          scannedAt: minutesAgo(22 * 60 - 3),
        },
      },
    ],
  },
]

// ---------------------------------------------------------------------------
// Formato
// ---------------------------------------------------------------------------

export function formatInt(value: number | null | undefined): string {
  if (value == null) return '—'
  return value.toLocaleString('es-MX')
}

export function formatPct(value: number | null | undefined): string {
  if (value == null) return '—'
  const pct = Math.max(0, Math.min(100, value * 100))
  // Truncar, nunca redondear hacia arriba: un escaneo parcial no puede leerse 100 %.
  if (pct >= 100) return '100%'
  return `${(Math.floor(pct * 10) / 10).toFixed(1)}%`
}

export function formatRelative(iso: string | null, now = Date.now()): string {
  if (!iso) return 'Nunca'
  const diff = new Date(iso).getTime() - now
  const mins = Math.round(Math.abs(diff) / MIN)
  if (mins < 1) return 'Ahora mismo'
  const prefix = diff < 0 ? 'hace' : 'en'
  if (mins < 60) return `${prefix} ${mins} min`
  const hours = Math.floor(mins / 60)
  const rest = mins % 60
  if (hours < 24) {
    const body = hours < 6 && rest > 0 ? `${hours} h ${rest} min` : `${hours} h`
    return `${prefix} ${body}`
  }
  return `${prefix} ${Math.floor(hours / 24)} d`
}

export function formatShortDate(iso: string | null): string {
  if (!iso) return 'Sin registro'
  return new Intl.DateTimeFormat('es-MX', {
    day: 'numeric',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(iso))
}

export function formatAbsolute(iso: string | null): string {
  if (!iso) return 'Sin registro'
  return new Intl.DateTimeFormat('es-MX', {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(iso))
}

/** Exactly the raw UTC stamp the current UI prints (`scanned_at`). */
export function rawUtcStamp(iso: string | null): string {
  if (!iso) return 'no time'
  return iso.slice(0, 19).replace('T', ' ')
}

export function relativeHours(iso: string | null, now = Date.now()): number | null {
  if (!iso) return null
  return (now - new Date(iso).getTime()) / 3_600_000
}

export function freshnessOf(iso: string | null, now = Date.now()): Freshness {
  const hours = relativeHours(iso, now)
  if (hours == null) return 'never'
  if (hours < 6) return 'fresh'
  if (hours < 24) return 'aging'
  return 'stale'
}

export function freshnessLabel(freshness: Freshness): string {
  switch (freshness) {
    case 'fresh':
      return 'Reciente'
    case 'aging':
      return 'Puede estar viejo'
    case 'stale':
      return 'Desactualizado'
    default:
      return 'Sin escanear'
  }
}

export function accountLabel(account: ProtoAccount): string {
  if (account.displayName && account.email && account.displayName !== account.email) {
    return `${account.displayName} (${account.email})`
  }
  return account.email ?? account.displayName ?? 'Identidad no indicada'
}

export function initialOf(account: ProtoAccount): string {
  const source = account.displayName || account.email || '?'
  return source.slice(0, 1).toUpperCase()
}

// ---------------------------------------------------------------------------
// Cálculo propuesto
// ---------------------------------------------------------------------------

export interface AccountFacts {
  account: ProtoAccount
  snapshot: ProtoSnapshot | undefined
  label: string
  initial: string
  isActive: boolean
  reported: number | null
  imported: number | null
  coverage: number | null
  missing: number | null
  surplus: number | null
  completeness: Completeness | null
  freshness: Freshness
  state: AccountState
  stateLabel: string
  scannedAt: string | null
  scannedRelative: string
  scannedAbsolute: string
  credentialLabel: string
}

function stateOf(snapshot: ProtoSnapshot | undefined, freshness: Freshness): AccountState {
  if (!snapshot) return 'never'
  if (freshness === 'stale') return 'stale'
  if (snapshot.completeness !== 'complete') return 'partial'
  return 'ok'
}

const CREDENTIAL_LABEL: Record<ProtoAccount['credential'], string> = {
  oauth: 'OAuth (se renueva solo)',
  password: 'Correo y contraseña',
  cookie: 'Cookie de sesión del navegador',
}

export function accountFacts(account: ProtoAccount, now = Date.now()): AccountFacts {
  const snapshot = account.snapshot
  const reported = snapshot?.reported ?? null
  const imported = snapshot?.imported ?? null
  const coverage = reported && reported > 0 && imported != null ? imported / reported : null
  const freshness = freshnessOf(snapshot?.scannedAt ?? null, now)
  const state = stateOf(snapshot, freshness)
  return {
    account,
    snapshot,
    label: accountLabel(account),
    initial: initialOf(account),
    isActive: account.isActive,
    reported,
    imported,
    coverage,
    missing: reported != null && imported != null ? Math.max(reported - imported, 0) : null,
    surplus: reported != null && imported != null ? Math.max(imported - reported, 0) : null,
    completeness: snapshot?.completeness ?? null,
    freshness,
    state,
    stateLabel:
      state === 'ok'
        ? 'Completo'
        : state === 'partial'
          ? 'Incompleto'
          : state === 'stale'
            ? 'Desactualizado'
            : 'Sin escanear',
    scannedAt: snapshot?.scannedAt ?? null,
    scannedRelative: formatRelative(snapshot?.scannedAt ?? null, now),
    scannedAbsolute: formatAbsolute(snapshot?.scannedAt ?? null),
    credentialLabel: CREDENTIAL_LABEL[account.credential],
  }
}

export interface ServiceFacts {
  service: ProtoService
  accounts: AccountFacts[]
  primary: AccountFacts | null
  totalAccounts: number
  scannedAccounts: number
  partialAccounts: number
  staleAccounts: number
  missingAccounts: number
  /** Sum of provider-declared favorites over the accounts that have data. */
  reportedAcross: number | null
  /** Sum of imported favorites over the accounts that have data. */
  importedAcross: number | null
  coverageAcross: number | null
  /** Coverage of the account currently in use. */
  primaryCoverage: number | null
  favoritesInLibrary: number
  tracksInLibrary: number
  playlists: number
  stateLabel: string
  stateTone: 'ok' | 'partial' | 'never' | 'offline'
  issues: string[]
}

export function serviceFacts(service: ProtoService, now = Date.now()): ServiceFacts {
  const accounts = service.accounts.map(account => accountFacts(account, now))
  const primary = accounts.find(account => account.isActive) ?? accounts[0] ?? null
  const withData = accounts.filter(account => account.reported != null && account.imported != null)
  const reportedAcross = withData.length
    ? withData.reduce((sum, account) => sum + (account.reported ?? 0), 0)
    : null
  const importedAcross = withData.length
    ? withData.reduce((sum, account) => sum + (account.imported ?? 0), 0)
    : null
  const coverageAcross =
    reportedAcross && reportedAcross > 0 && importedAcross != null ? importedAcross / reportedAcross : null

  const scannedAccounts = accounts.filter(account => account.reported != null).length
  const partialAccounts = accounts.filter(account => account.state === 'partial').length
  const staleAccounts = accounts.filter(account => account.state === 'stale').length
  const missingAccounts = accounts.filter(account => account.state === 'never').length

  const offline = service.status === 'invalid' || service.status === 'disconnected'
  const stateTone = offline
    ? 'offline'
    : !primary || primary.state === 'never'
      ? 'never'
      : primary.state === 'partial'
        ? 'partial'
        : primary.state === 'stale'
          ? 'partial'
          : 'ok'
  const stateLabel = offline
    ? 'Desconectado'
    : !primary || primary.state === 'never'
      ? 'Sin escanear'
      : primary.state === 'partial'
        ? 'Escaneo incompleto'
        : primary.state === 'stale'
          ? 'Datos desactualizados'
          : 'Al día'

  const issues: string[] = []
  if (missingAccounts > 0) issues.push(`${missingAccounts} ${missingAccounts === 1 ? 'cuenta sin escanear' : 'cuentas sin escanear'}`)
  if (partialAccounts > 0) issues.push(`${partialAccounts} ${partialAccounts === 1 ? 'escaneo incompleto' : 'escaneos incompletos'}`)
  if (staleAccounts > 0) issues.push(`${staleAccounts} ${staleAccounts === 1 ? 'escaneo viejo' : 'escaneos viejos'}`)

  return {
    service,
    accounts,
    primary,
    totalAccounts: accounts.length,
    scannedAccounts,
    partialAccounts,
    staleAccounts,
    missingAccounts,
    reportedAcross,
    importedAcross,
    coverageAcross,
    primaryCoverage: primary?.coverage ?? null,
    favoritesInLibrary: service.library.favorites,
    tracksInLibrary: service.library.tracks,
    playlists: service.library.playlists,
    stateLabel,
    stateTone,
    issues,
  }
}

export function allServiceFacts(now = Date.now()): ServiceFacts[] {
  return PROTO_SERVICES.map(service => serviceFacts(service, now))
}

export interface GlobalFacts {
  totalAccounts: number
  scannedAccounts: number
  missingAccounts: number
  partialAccounts: number
  reported: number
  imported: number
  coverage: number | null
}

export function globalFacts(facts: ServiceFacts[]): GlobalFacts {
  const accounts = facts.flatMap(service => service.accounts)
  const withData = accounts.filter(account => account.reported != null && account.imported != null)
  const reported = withData.reduce((sum, account) => sum + (account.reported ?? 0), 0)
  const imported = withData.reduce((sum, account) => sum + (account.imported ?? 0), 0)
  return {
    totalAccounts: accounts.length,
    scannedAccounts: withData.length,
    missingAccounts: accounts.filter(account => account.state === 'never').length,
    partialAccounts: accounts.filter(account => account.state === 'partial').length,
    reported,
    imported,
    coverage: reported > 0 ? imported / reported : null,
  }
}
