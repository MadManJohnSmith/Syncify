/**
 * Status Bar Telemetry Helpers (FE-7)
 *
 * Pure helpers that derive status-bar values from real application state:
 * global task events (sync state / ETA), service statuses (last sync time)
 * and download progress events (download speed). No hardcoded telemetry:
 * every displayed value originates from backend events or real API data.
 */

export type SyncState = 'syncing' | 'idle' | 'error' | 'paused'

/** Minimal structural shape of a task used for state derivation */
export interface SyncTaskLike {
    type: string
    status: string
    error?: string
    startedAt?: number
    progress?: number
}

const TERMINAL_DOWNLOAD_STATUSES = new Set([
    'complete',
    'completed',
    'failed',
    'error',
    'cancelled',
    'canceled',
    'requires_auth',
    'stale_source',
    'rejected_quality',
    'not_found',
])

/**
 * A download progress event is terminal when the download stopped (the speed
 * readout must go back to "no active download" instead of freezing).
 */
export function isTerminalDownloadStatus(status: string): boolean {
    return TERMINAL_DOWNLOAD_STATUSES.has((status || '').toLowerCase())
}

/**
 * Parse a service `last_synced` value. The backend stores SQLite
 * CURRENT_TIMESTAMP ("YYYY-MM-DD HH:MM:SS", UTC); ISO strings are accepted too.
 */
export function parseSqliteUtc(value: string): Date | null {
    const trimmed = (value || '').trim()
    if (!trimmed) return null
    const m = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2}):(\d{2})/.exec(trimmed)
    if (m) {
        return new Date(Date.UTC(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +m[6]))
    }
    const parsed = new Date(trimmed)
    return Number.isNaN(parsed.getTime()) ? null : parsed
}

/** Relative formatting of a past timestamp ("Just now", "5 min ago", ...). */
export function formatRelativeTime(date: Date, now: number = Date.now()): string {
    const diffSec = Math.round((now - date.getTime()) / 1000)
    // Future timestamps (clock skew) are reported as "Just now" rather than negative values.
    if (diffSec < 60) return 'Just now'
    if (diffSec < 3600) {
        const minutes = Math.floor(diffSec / 60)
        return minutes === 1 ? '1 min ago' : `${minutes} min ago`
    }
    if (diffSec < 86400) {
        const hours = Math.floor(diffSec / 3600)
        return hours === 1 ? '1 hour ago' : `${hours} hours ago`
    }
    const days = Math.floor(diffSec / 86400)
    if (days === 1) return 'Yesterday'
    if (days < 30) return `${days} days ago`
    return date.toLocaleDateString()
}

/**
 * Format a download speed given in KB/s. The backend computes
 * instant_kbps/average_kbps as delta_bytes / 1024 / elapsed_seconds, i.e. KB/s.
 * Returns an em dash when there is no active download to measure.
 */
export function formatDownloadSpeed(kbps: number | null): string {
    if (kbps === null || !Number.isFinite(kbps) || kbps <= 0) return '—'
    if (kbps >= 1024) return `${(kbps / 1024).toFixed(1)} MB/s`
    return `${Math.round(kbps)} KB/s`
}

/** Human-friendly duration for an ETA estimate. */
export function formatRemainingDuration(ms: number): string {
    const totalSec = Math.max(1, Math.round(ms / 1000))
    if (totalSec < 60) return `${totalSec}s`
    const minutes = Math.floor(totalSec / 60)
    const seconds = totalSec % 60
    if (minutes < 60) return seconds > 0 ? `${minutes}m ${seconds}s` : `${minutes}m`
    const hours = Math.floor(minutes / 60)
    const mins = minutes % 60
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`
}

/**
 * Estimate remaining milliseconds from the elapsed time and progress of a
 * task (linear interpolation). Returns null when progress/elapsed give no
 * basis for an estimate (just started, no progress, or already done).
 */
export function estimateRemainingMs(startedAt: number, progress: number, now: number): number | null {
    const p = Math.min(100, Math.max(0, progress))
    const elapsed = now - startedAt
    if (p <= 0 || p >= 100 || elapsed <= 0) return null
    return (elapsed / p) * (100 - p)
}

/**
 * Derive the sync state from real tasks. Precedence:
 * 1. any running task → 'syncing'
 * 2. any failed / requires_auth sync task → 'error'
 * 3. any paused task → 'paused'
 * 4. otherwise → 'idle'
 */
export function deriveSyncState(tasks: SyncTaskLike[]): SyncState {
    if (tasks.some(t => t.status === 'running')) return 'syncing'
    if (tasks.some(t => t.type === 'sync' && (t.status === 'failed' || t.status === 'requires_auth'))) return 'error'
    if (tasks.some(t => t.status === 'paused')) return 'paused'
    return 'idle'
}

/**
 * Real error message of the most recent failed sync task, or null when no
 * sync task failed.
 */
export function deriveSyncErrorMessage(tasks: SyncTaskLike[]): string | null {
    const latest = tasks
        .filter(t => t.type === 'sync' && (t.status === 'failed' || t.status === 'requires_auth'))
        .sort((a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0))[0]
    if (!latest) return null
    return latest.error && latest.error.trim() ? latest.error.trim() : 'Sync failed'
}
