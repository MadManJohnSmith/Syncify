/**
 * System Logs Composable (S170)
 * 
 * Shared global reactive state for structured system and audit logs.
 * Integrates with Rust native ring buffer, captures background events,
 * and maintains history across tab changes without mock logs.
 */

import { ref, computed, readonly, toRaw, triggerRef } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { save as saveDialog } from '@tauri-apps/plugin-dialog'
import { getSystemLogs, clearSystemLogs as apiClearSystemLogs, exportSystemLogs as apiExportSystemLogs, type SystemLogEntry } from '@/api/logs'
import { TauriEvents, invokeCommand } from '@/api/tauri'
import { useToast } from './useToast'

export interface LogEntry {
    id: string
    time: string
    level: 'info' | 'warn' | 'error' | 'success' | 'debug' | 'trace'
    provider: string
    category: string
    message: string
    rawCategory: 'enrichment' | 'downloads' | 'system' | 'library' | 'database' | 'security' | 'worker'
    details?: any
}

// Global singleton in-memory state
const logs = ref<LogEntry[]>([])
const initialized = ref(false)
const isListening = ref(false)
let unlistenFns: UnlistenFn[] = []
let logIdCounter = 1000

export const SESSION_STORAGE_KEY = 'syncify_cached_logs'

/**
 * R15 / límites: el anillo nativo guarda 2000 entradas
 * (`services::logging::DEFAULT_BUFFER_CAPACITY`); la UI quedaba corta en 1000.
 * El histórico anterior a esto se recupera de disco con `read_log_history`.
 */
export const MAX_LOG_ENTRIES = 2000

/** Page size used when paging through the on-disk log history. */
export const HISTORY_PAGE_SIZE = 500

/**
 * Format ISO timestamp or Date into HH:MM:SS
 */
export function formatLogTime(isoOrDate?: string | Date): string {
    if (!isoOrDate) {
        return new Date().toTimeString().split(' ')[0]
    }
    if (isoOrDate instanceof Date) {
        return isoOrDate.toTimeString().split(' ')[0]
    }
    try {
        const d = new Date(isoOrDate)
        if (isNaN(d.getTime())) {
            // Already HH:MM:SS format
            if (typeof isoOrDate === 'string' && isoOrDate.includes(':') && isoOrDate.length <= 8) {
                return isoOrDate
            }
            return new Date().toTimeString().split(' ')[0]
        }
        return d.toTimeString().split(' ')[0]
    } catch {
        return new Date().toTimeString().split(' ')[0]
    }
}

/**
 * Normalize provider / module name for badges and UI categorization
 */
export function normalizeProviderName(raw?: string): string {
    if (!raw) return 'System'
    const lower = raw.toLowerCase()
    if (lower.includes('spotify')) return 'Spotify'
    if (lower.includes('qobuz')) return 'Qobuz'
    if (lower.includes('tidal')) return 'Tidal'
    if (lower.includes('deezer')) return 'Deezer'
    if (lower.includes('apple')) return 'Apple Music'
    if (lower.includes('soundcloud')) return 'SoundCloud'
    if (lower.includes('musicbrainz')) return 'MusicBrainz'
    if (lower.includes('lastfm')) return 'Last.fm'
    if (lower.includes('enrichment')) return 'Enrichment'
    if (lower.includes('downloader') || lower.includes('download')) return 'Downloads'
    if (lower.includes('worker')) return 'Worker'
    if (lower.includes('database') || lower.includes('db') || lower.includes('sqlx')) return 'Database'
    if (lower.includes('filesystem') || lower.includes('scanner') || lower.includes('organize')) return 'Filesystem'
    if (lower.includes('security') || lower.includes('crypto') || lower.includes('auth')) return 'Security'
    if (lower.includes('library')) return 'Library'
    if (lower.includes('lyrics')) return 'Lyrics'
    return raw.charAt(0).toUpperCase() + raw.slice(1)
}

/**
 * Get CSS badge styling for severity levels
 */
export function getLevelBadgeClass(level: string): string {
    switch (level.toLowerCase()) {
        case 'error': return 'bg-red-500/20 text-red-400 border border-red-500/30'
        case 'warn':
        case 'warning': return 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
        case 'success': return 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
        case 'info': return 'bg-blue-500/20 text-blue-400 border border-blue-500/30'
        case 'debug': return 'bg-purple-500/20 text-purple-400 border border-purple-500/30'
        case 'trace': return 'bg-gray-500/20 text-gray-400 border border-gray-500/30'
        default: return 'bg-gray-500/20 text-gray-400'
    }
}

/**
 * Get CSS badge styling for providers
 */
export function getProviderBadgeClass(provider: string): string {
    const p = (provider || '').toLowerCase()
    if (p.includes('spotify')) return 'bg-[#1ed760]/10 text-[#1ed760] border border-[#1ed760]/20'
    if (p.includes('qobuz')) return 'bg-[#1a8fe3]/10 text-[#1a8fe3] border border-[#1a8fe3]/20'
    if (p.includes('tidal')) return 'bg-[#00d4aa]/10 text-[#00d4aa] border border-[#00d4aa]/20'
    if (p.includes('deezer')) return 'bg-[#ff0092]/10 text-[#ff0092] border border-[#ff0092]/20'
    if (p.includes('apple')) return 'bg-[#fa2d48]/10 text-[#fa2d48] border border-[#fa2d48]/20'
    if (p.includes('soundcloud')) return 'bg-[#ff5500]/10 text-[#ff5500] border border-[#ff5500]/20'
    if (p.includes('musicbrainz')) return 'bg-amber-400/10 text-amber-300 border border-amber-400/20'
    if (p.includes('lastfm')) return 'bg-red-400/10 text-red-300 border border-red-400/20'
    if (p.includes('download')) return 'bg-primary/10 text-primary border border-primary/20'
    if (p.includes('enrichment')) return 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
    if (p.includes('database')) return 'bg-cyan-500/10 text-cyan-400 border border-cyan-500/20'
    if (p.includes('filesystem')) return 'bg-teal-500/10 text-teal-400 border border-teal-500/20'
    if (p.includes('worker')) return 'bg-indigo-500/10 text-indigo-400 border border-indigo-500/20'
    if (p.includes('security')) return 'bg-rose-500/10 text-rose-400 border border-rose-500/20'
    if (p.includes('lyrics')) return 'bg-violet-500/10 text-violet-400 border border-violet-500/20'
    return 'bg-gray-500/10 text-gray-400 border border-gray-500/20'
}

/**
 * Sensitive key patterns to redact in objects/details
 */
const SENSITIVE_KEY_REGEX = /(password|passwd|pwd|secret|api[_-]?key|apikey|cookie|credential|credentials|token|authorization|^auth$|[_-]auth$|[_-]auth[_-])/i

/**
 * Regular expressions for detecting secrets in strings
 */
const BEARER_REGEX = /\b(bearer\s+)([a-zA-Z0-9_\-\.=]+)/gi
const BASIC_AUTH_REGEX = /\b(basic\s+)([a-zA-Z0-9+/=]{8,})/gi
const COOKIE_HEADER_REGEX = /\b(cookie\s*:\s*)([^\r\n]+)/gi
const KEY_VALUE_SECRET_REGEX = /\b((?:token|access_token|refresh_token|api[_-]?key|apikey|secret|client[_-]?secret|password|passwd|pwd|auth_token|cookie)\s*[:=]\s*)(['"]?)([^'"\s,;&]+)(\2)/gi
const BASIC_AUTH_URL_REGEX = /(https?:\/\/)([^:\s]+):([^@\s]+)@/gi
const JWT_REGEX = /\beyJ[a-zA-Z0-9_-]{10,}\.eyJ[a-zA-Z0-9_-]{10,}\.[a-zA-Z0-9_-]+/g

/**
 * Redact sensitive strings (tokens, bearer, passwords, api keys, cookies, URLs with credentials, JWTs)
 */
export function redactSecretString(str: string): string {
    if (!str || typeof str !== 'string') return str
    return str
        .replace(BEARER_REGEX, '$1[REDACTED]')
        .replace(BASIC_AUTH_REGEX, '$1[REDACTED]')
        .replace(COOKIE_HEADER_REGEX, '$1[REDACTED]')
        .replace(KEY_VALUE_SECRET_REGEX, '$1$2[REDACTED]$4')
        .replace(BASIC_AUTH_URL_REGEX, '$1$2:[REDACTED]@')
        .replace(JWT_REGEX, '[REDACTED_JWT]')
}

/**
 * Recursively redact secrets in objects, arrays, or primitive values
 */
export function redactSensitiveData<T = any>(val: T, visited = new WeakSet()): T {
    if (val === null || val === undefined) return val
    if (typeof val === 'string') {
        return redactSecretString(val) as unknown as T
    }
    if (typeof val !== 'object') {
        return val
    }
    if (visited.has(val as object)) {
        return '[CIRCULAR]' as unknown as T
    }
    visited.add(val as object)

    if (Array.isArray(val)) {
        return val.map(item => redactSensitiveData(item, visited)) as unknown as T
    }

    const result: Record<string, any> = {}
    for (const [key, value] of Object.entries(val)) {
        if (SENSITIVE_KEY_REGEX.test(key)) {
            result[key] = '[REDACTED]'
        } else {
            result[key] = redactSensitiveData(value, visited)
        }
    }
    return result as T
}

/**
 * Redact a single LogEntry before serialization/persistence
 */
export function redactLogEntry(entry: LogEntry): LogEntry {
    return {
        ...entry,
        message: redactSecretString(entry.message),
        details: entry.details !== undefined ? redactSensitiveData(entry.details) : undefined,
    }
}

let persistTimeout: any = null

/**
 * Safely cache recent logs to sessionStorage to prevent massive sync writes on flood.
 * Sanitizes and redacts all secrets before storing.
 */
export function persistToSessionStorage(entries: LogEntry[], immediate: boolean = false) {
    const doPersist = () => {
        try {
            if (typeof window !== 'undefined' && window.sessionStorage) {
                const slice = entries.slice(0, 200).map(redactLogEntry)
                window.sessionStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(slice))
            }
        } catch {
            // Ignore session storage quota or restriction errors
        }
    }

    if (immediate) {
        if (persistTimeout) {
            clearTimeout(persistTimeout)
            persistTimeout = null
        }
        doPersist()
        return
    }

    if (persistTimeout) return
    persistTimeout = setTimeout(() => {
        persistTimeout = null
        doPersist()
    }, 50)
}

/**
 * Convert backend SystemLogEntry into frontend LogEntry
 */
export function mapSystemLogToLogEntry(sys: SystemLogEntry): LogEntry {
    const rawCategory = sys.module.toLowerCase().includes('enrichment') ? 'enrichment'
        : sys.module.toLowerCase().includes('download') ? 'downloads'
        : sys.module.toLowerCase().includes('database') || sys.module.toLowerCase().includes('db') ? 'database'
        : sys.module.toLowerCase().includes('worker') ? 'worker'
        : sys.module.toLowerCase().includes('library') ? 'library'
        : 'system'

    return {
        id: sys.id || `sys-${++logIdCounter}`,
        time: formatLogTime(sys.timestamp),
        level: sys.level as any,
        provider: normalizeProviderName(sys.module),
        category: sys.module || normalizeProviderName(sys.target),
        message: sys.message,
        rawCategory,
        details: sys.fields,
    }
}

/**
 * R15: el histórico leído de disco tiene fechas pasadas, así que la columna de
 * tiempo lleva la fecha cuando la entrada no es de hoy ("MM-DD HH:MM:SS").
 */
export function formatHistoryLogTime(isoOrDate?: string | Date): string {
    try {
        const d = isoOrDate instanceof Date ? isoOrDate : new Date(isoOrDate ?? '')
        if (isNaN(d.getTime())) return formatLogTime(isoOrDate)
        if (d.toDateString() === new Date().toDateString()) {
            return d.toTimeString().split(' ')[0]
        }
        const mm = String(d.getMonth() + 1).padStart(2, '0')
        const dd = String(d.getDate()).padStart(2, '0')
        return `${mm}-${dd} ${d.toTimeString().split(' ')[0]}`
    } catch {
        return formatLogTime(isoOrDate)
    }
}

/**
 * Convert a history SystemLogEntry (read from the rotating files on disk)
 * into a frontend LogEntry, keeping the date in the time column.
 */
export function mapHistorySystemLogToLogEntry(sys: SystemLogEntry): LogEntry {
    return {
        ...mapSystemLogToLogEntry(sys),
        time: formatHistoryLogTime(sys.timestamp),
    }
}

/** Filters accepted by loadLogHistory / exportLogHistory. */
export interface LogHistoryFilters {
    from?: string
    to?: string
    level?: string
    query?: string
}

/** One page of the on-disk log history returned by `read_log_history`. */
export interface LogHistoryPage {
    entries: LogEntry[]
    total: number
    offset: number
    limit: number
    hasMore: boolean
}

/**
 * Reset all logs and listeners (primarily for tests)
 */
export function resetLogs(initialEntries: LogEntry[] = []) {
    if (persistTimeout) {
        clearTimeout(persistTimeout)
        persistTimeout = null
    }
    logs.value = [...initialEntries]
    unlistenFns.forEach(fn => fn())
    unlistenFns = []
    initialized.value = false
    isListening.value = false
    try {
        if (typeof window !== 'undefined' && window.sessionStorage) {
            window.sessionStorage.removeItem(SESSION_STORAGE_KEY)
        }
    } catch {
        // Ignore
    }
}

export function useLogs() {
    const toast = useToast()

    /**
     * Add a structured log entry into the reactive array
     */
    function addLog(entry: Omit<LogEntry, 'id' | 'time'> & { id?: string; time?: string }) {
        const timeStr = entry.time || formatLogTime()
        const idStr = entry.id || String(++logIdCounter)

        const fullEntry: LogEntry = {
            id: idStr,
            time: timeStr,
            level: entry.level,
            provider: normalizeProviderName(entry.provider),
            category: entry.category || normalizeProviderName(entry.provider),
            message: entry.message,
            rawCategory: entry.rawCategory,
            details: entry.details,
        }

        // R15 (rendimiento): la lista puede recibir ráfagas de miles de
        // entradas y mutar el proxy reactivo es O(n) por llamada (unshift
        // dispara un set reactivo por cada elemento desplazado). Se muta el
        // array crudo y se notifica UNA vez con triggerRef: cualquier
        // consumidor reacciona igual porque siempre lee `logs.value`
        // (dependencia del propio ref) antes de recorrer la lista.
        const rawLogs = toRaw(logs.value)

        // Avoid adding duplicate identical ID if re-fetched
        const existingIdx = rawLogs.findIndex(l => l.id === idStr)
        if (existingIdx >= 0) {
            rawLogs[existingIdx] = fullEntry
        } else {
            rawLogs.unshift(fullEntry)
        }

        // Bound memory array: aligned with the native 2000-entry ring buffer
        // (MAX_LOG_ENTRIES); older entries live on disk and are paged in.
        if (rawLogs.length > MAX_LOG_ENTRIES) {
            rawLogs.pop()
        }

        triggerRef(logs)
        persistToSessionStorage(logs.value)
    }

    /**
     * Fetch logs directly from backend IPC
     */
    async function fetchLogs(params?: { limit?: number; level_filter?: string; module_filter?: string; search?: string }) {
        try {
            const rawLogs = await getSystemLogs(params)
            if (rawLogs && Array.isArray(rawLogs) && rawLogs.length > 0) {
                const mapped = rawLogs.map(mapSystemLogToLogEntry)
                // Merge without duplicating IDs
                const existingIds = new Set(logs.value.map(l => l.id))
                for (const item of mapped) {
                    if (!existingIds.has(item.id)) {
                        logs.value.push(item)
                    }
                }
                // Sort newest first if timestamps available
                persistToSessionStorage(logs.value)
            }
        } catch (err) {
            console.warn('[useLogs] Could not fetch native backend logs:', err)
        }
    }

    /**
     * Clear displayed logs and backend ring buffer
     */
    async function clearLogs() {
        try {
            await apiClearSystemLogs()
        } catch {
            // Ignore if backend not reachable
        }
        logs.value = []
        try {
            if (typeof window !== 'undefined' && window.sessionStorage) {
                window.sessionStorage.removeItem(SESSION_STORAGE_KEY)
            }
        } catch {}
        toast.success('Logs Cleared', 'System and console logs buffer cleared')
    }

    /**
     * Copy displayed/filtered logs to clipboard
     */
    async function copyLogs(filteredList?: LogEntry[]): Promise<boolean> {
        const listToCopy = filteredList || logs.value
        if (listToCopy.length === 0) return false
        const text = listToCopy.map(l => `[${l.time}] [${l.level.toUpperCase()}] [${l.provider}] [${l.category}] ${l.message}`).join('\n')
        try {
            await navigator.clipboard.writeText(text)
            toast.success('Copied', `${listToCopy.length} log lines copied to clipboard`)
            return true
        } catch {
            toast.error('Copy Failed', 'Could not copy logs to clipboard')
            return false
        }
    }

    /**
     * Export all system logs to a text file download
     */
    async function exportLogsFile(filteredList?: LogEntry[]): Promise<void> {
        try {
            let content = ''
            try {
                content = await apiExportSystemLogs()
            } catch {
                // Fallback to in-memory formatting
                const list = filteredList || logs.value
                content = `# Syncify Log Export - ${new Date().toISOString()}\n` +
                          list.map(l => `[${l.time}] [${l.level.toUpperCase()}] [${l.provider}] [${l.category}] ${l.message}`).join('\n')
            }

            const blob = new Blob([content], { type: 'text/plain;charset=utf-8' })
            const url = URL.createObjectURL(blob)
            const a = document.createElement('a')
            a.href = url
            a.download = `syncify-logs-${new Date().toISOString().replace(/[:.]/g, '-')}.txt`
            document.body.appendChild(a)
            a.click()
            document.body.removeChild(a)
            URL.revokeObjectURL(url)

            toast.success('Export Successful', 'Logs exported to text file')
        } catch (e) {
            console.error('Failed to export logs file:', e)
            toast.error('Export Failed', 'Could not export logs file')
        }
    }

    /**
     * R15: load a page of the on-disk log history (rotating files read back by
     * the `read_log_history` command) merged into the live log list.
     * History entries carry stable ids (`hist-...`) so re-loading or paging
     * never duplicates them.
     */
    async function loadLogHistory(
        filters: LogHistoryFilters & { offset?: number; limit?: number },
    ): Promise<LogHistoryPage> {
        const offset = filters.offset ?? 0
        const limit = filters.limit ?? HISTORY_PAGE_SIZE
        const raw = await invokeCommand<unknown>('read_log_history', {
            from: filters.from ?? null,
            to: filters.to ?? null,
            level: filters.level ?? null,
            query: filters.query ?? null,
            offset,
            limit,
        })

        const rec = (raw ?? {}) as Record<string, any>
        const rawEntries = Array.isArray(rec.entries) ? rec.entries : []
        const mapped: LogEntry[] = rawEntries.map((entry: any) => mapHistorySystemLogToLogEntry(entry as SystemLogEntry))

        // Newest-first pages: page 2+ (older entries) appends below what is
        // already on screen; ids are stable so reloads do not duplicate rows.
        const existingIds = new Set(logs.value.map(l => l.id))
        for (const item of mapped) {
            if (!existingIds.has(item.id)) {
                logs.value.push(item)
                existingIds.add(item.id)
            }
        }
        if (logs.value.length > MAX_LOG_ENTRIES) {
            logs.value.splice(MAX_LOG_ENTRIES)
        }
        persistToSessionStorage(logs.value, true)

        const total = typeof rec.total === 'number' ? rec.total : mapped.length
        const pageLimit = typeof rec.limit === 'number' ? rec.limit : limit
        return {
            entries: mapped,
            total,
            offset: typeof rec.offset === 'number' ? rec.offset : offset,
            limit: pageLimit,
            hasMore: offset + mapped.length < total,
        }
    }

    /**
     * R15: export the filtered on-disk history. The native dialog is used ONLY
     * to pick the destination path; all feedback goes through useToast.
     */
    async function exportLogHistory(filters: LogHistoryFilters): Promise<boolean> {
        let destPath: string | null = null
        try {
            destPath = await saveDialog({
                title: 'Export log history',
                defaultPath: `syncify-logs-history-${new Date().toISOString().slice(0, 10)}.txt`,
                filters: [{ name: 'Text', extensions: ['txt'] }],
            })
        } catch (e) {
            console.error('[useLogs] Native save dialog failed:', e)
            toast.error('Export Failed', 'Could not open the file dialog')
            return false
        }
        if (!destPath) return false // Usuario canceló el diálogo

        try {
            const raw = await invokeCommand<unknown>('export_log_range', {
                from: filters.from ?? null,
                to: filters.to ?? null,
                destPath,
                level: filters.level ?? null,
                query: filters.query ?? null,
            })
            const rec = (raw ?? {}) as Record<string, any>
            const exportedCount = typeof rec.exportedCount === 'number' ? rec.exportedCount : 0
            toast.success('Export Successful', `Log history exported to ${rec.path || destPath} (${exportedCount} entries)`)
            return true
        } catch (e) {
            console.error('[useLogs] Failed to export log history:', e)
            toast.error('Export Failed', `Could not export the log history: ${e instanceof Error ? e.message : String(e)}`)
            return false
        }
    }

    /**
     * Initialize background event listeners across whole app lifecycle (idempotent)
     */
    async function initLogListeners() {
        if (initialized.value) return
        initialized.value = true

        // Clean any previous listeners
        unlistenFns.forEach(fn => fn())
        unlistenFns = []

        // 1. Fetch existing log buffer from Rust backend
        await fetchLogs({ limit: 200 })

        // 2. Listen for live native Rust logs
        try {
            const unlistenLive = await listen<SystemLogEntry>(TauriEvents.LOG_EVENT, (event) => {
                if (!event.payload) return
                const mapped = mapSystemLogToLogEntry(event.payload)
                addLog(mapped)
            })
            unlistenFns.push(unlistenLive)
        } catch (e) {
            console.warn('[useLogs] Native syncify:log_event listener not available:', e)
        }

        // 3. Listen for enrichment events
        try {
            const unlistenEnrichment = await listen<any>(TauriEvents.ENRICHMENT_EVENT, (event) => {
                const payload = event.payload
                if (!payload) return
                const level: LogEntry['level'] = 
                    payload.status === 'completed' ? 'success' :
                    payload.status === 'failed' ? 'error' :
                    payload.status === 'rate_limited' ? 'warn' : 'info'

                addLog({
                    level,
                    provider: normalizeProviderName(payload.service || 'Enrichment'),
                    category: 'Enrichment',
                    message: payload.message || `Track ${payload.track_id}: status ${payload.status}`,
                    rawCategory: 'enrichment',
                    details: payload,
                })
            })
            unlistenFns.push(unlistenEnrichment)
        } catch (e) {
            console.warn('[useLogs] syncify:enrichment_event listener not available:', e)
        }

        // 4. Listen for download progress & completion events
        try {
            const unlistenDownloads = await listen<any>(TauriEvents.DOWNLOAD_PROGRESS, (event) => {
                const payload = event.payload
                if (!payload) return
                const status = (payload.status || '').toLowerCase()
                const title = payload.title || payload.target_title || `Track #${payload.track_id || payload.queue_id}`
                const level: LogEntry['level'] = 
                    status === 'complete' || status === 'completed' ? 'success' :
                    status === 'failed' || status === 'stale_source' || status === 'rejected_quality' ? 'error' :
                    status === 'paused' ? 'warn' : 'info'

                const msg = payload.message 
                    ? `"${title}" - ${payload.message}` 
                    : (status === 'complete' || status === 'completed' 
                        ? `Downloaded "${title}" (100%) - Tags and sidecars verified.`
                        : (status === 'failed' 
                            ? `Download failed for "${title}": ${payload.error || payload.error_message || 'Unknown error'}`
                            : `Download in progress for "${title}": ${Math.round(payload.progress_percent || 0)}%`))

                addLog({
                    level,
                    provider: normalizeProviderName(payload.service_name || payload.service || 'Downloads'),
                    category: 'Downloads',
                    message: msg,
                    rawCategory: 'downloads',
                    details: payload,
                })
            })
            unlistenFns.push(unlistenDownloads)
        } catch (e) {
            console.warn('[useLogs] syncify:download_progress listener not available:', e)
        }

        // 5. Listen for general pipeline progress events
        try {
            const unlistenProgress = await listen<any>(TauriEvents.PROGRESS, (event) => {
                const payload = event.payload
                if (!payload) return
                // R15 (dedup): la telemetría de progreso NO es una entrada de
                // log. Cada tick del worker llega identificado por
                // queue_id/item_id + progress_percent y contaminaba el
                // histórico de auditoría. La barra de progreso se alimenta de
                // su propio canal (syncify:download_progress → store), los
                // resultados de descarga siguen entrando por ese listener y
                // los eventos de pipeline sin porcentaje (scan, organize…)
                // se siguen registrando aquí.
                const isDownloadProgressTick =
                    (payload.queue_id !== undefined && payload.queue_id !== null) ||
                    (payload.item_id !== undefined && payload.item_id !== null)
                if (isDownloadProgressTick && payload.progress_percent !== undefined && payload.progress_percent !== null) {
                    return
                }
                addLog({
                    level: payload.status === 'completed' ? 'success' : (payload.status === 'failed' ? 'error' : 'info'),
                    provider: normalizeProviderName(payload.provider || payload.operation || 'System'),
                    category: payload.operation || 'Pipeline',
                    message: payload.message || `Operation ${payload.operation || 'task'}: ${payload.status || 'in progress'}`,
                    rawCategory: 'system',
                    details: payload,
                })
            })
            unlistenFns.push(unlistenProgress)
        } catch (e) {
            console.warn('[useLogs] syncify:progress listener not available:', e)
        }

        // 6. Listen for system notifications
        try {
            const unlistenNotification = await listen<any>(TauriEvents.NOTIFICATION, (event) => {
                const payload = event.payload
                if (!payload) return
                addLog({
                    level: payload.level || payload.kind || 'info',
                    provider: 'System',
                    category: 'Notification',
                    message: payload.message || payload.title || 'System Notification',
                    rawCategory: 'system',
                    details: payload,
                })
            })
            unlistenFns.push(unlistenNotification)
        } catch (e) {
            console.warn('[useLogs] syncify:notification listener not available:', e)
        }

        isListening.value = true
    }

    return {
        // State
        logs: readonly(logs),
        rawLogs: logs,
        initialized: readonly(initialized),
        isListening: readonly(isListening),

        // Actions
        addLog,
        clearLogs,
        copyLogs,
        exportLogsFile,
        exportLogHistory,
        fetchLogs,
        loadLogHistory,
        initLogListeners,
        resetLogs,

        // Formatting Helpers
        getLevelBadgeClass,
        getProviderBadgeClass,
        normalizeProviderName,
    }
}
