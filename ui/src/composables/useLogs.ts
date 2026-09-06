/**
 * System Logs Composable (S170)
 * 
 * Shared global reactive state for structured system and audit logs.
 * Integrates with Rust native ring buffer, captures background events,
 * and maintains history across tab changes without mock logs.
 */

import { ref, computed, readonly } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { getSystemLogs, clearSystemLogs as apiClearSystemLogs, exportSystemLogs as apiExportSystemLogs, type SystemLogEntry } from '@/api/logs'
import { TauriEvents } from '@/api/tauri'
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

        // Avoid adding duplicate identical ID if re-fetched
        const existingIdx = logs.value.findIndex(l => l.id === idStr)
        if (existingIdx >= 0) {
            logs.value[existingIdx] = fullEntry
        } else {
            logs.value.unshift(fullEntry)
        }

        // Bound memory array to maximum 1000 items
        if (logs.value.length > 1000) {
            logs.value.pop()
        }

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
        fetchLogs,
        initLogListeners,
        resetLogs,

        // Formatting Helpers
        getLevelBadgeClass,
        getProviderBadgeClass,
        normalizeProviderName,
    }
}
