/**
 * Download Jobs API (D02, spec #111)
 *
 * Un job agrupa filas de la cola persistente. `createDownloadJob` devuelve el
 * id al instante; la estimación agregada y el encolado siguen en segundo
 * plano. `estimateDownloadJob` es una consulta pura (sin escribir).
 */

import { invokeCommand, TauriEvents } from './tauri'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type DownloadJobKind =
    | 'favorites'
    | 'album'
    | 'artist'
    | 'playlist'
    | 'selection'
    | 'url'
    | 'single'

export interface DownloadJobFilter {
    trackIds?: number[]
    albumId?: number
    artistId?: number
    playlistId?: number
    trackId?: number
    itemType?: 'tracks' | 'albums' | 'artists' | 'all'
}

export interface DownloadJobRequest {
    kind: DownloadJobKind
    label?: string
    sourceService?: string
    destinationService?: string
    preferredAccountId?: number
    qualityPreference?: string
    allowFallback?: boolean
    priority?: number
    limit?: number
    filter: DownloadJobFilter
}

export interface ServiceCount {
    service: string
    count: number
}

export interface CapacityCounts {
    hires: number
    lossless: number
    high: number
    lossy: number
    unknown: number
}

export interface DownloadJobEstimate {
    total: number
    bySource: ServiceCount[]
    byDestination: ServiceCount[]
    byCapacity: CapacityCounts
    estimatedSizeMb: number
}

export interface CreateDownloadJobResponse {
    jobId: number
}

export type DownloadJobStatus = 'running' | 'completed' | 'partial' | 'cancelled' | 'failed'

export interface DownloadJob {
    id: number
    kind: DownloadJobKind
    label?: string | null
    status: DownloadJobStatus
    sourceService?: string | null
    destinationService?: string | null
    preferredAccountId?: number | null
    qualityPreference?: string | null
    allowFallback: number
    filterJson?: string | null
    requestedCount: number
    estimatedCount: number
    estimatedSizeMb?: number | null
    completedCount: number
    failedCount: number
    skippedCount: number
    skipBreakdownJson?: string | null
    error?: string | null
    createdAt: string
    updatedAt: string
    startedAt?: string | null
    completedAt?: string | null
}

export async function createDownloadJob(
    request: DownloadJobRequest,
): Promise<CreateDownloadJobResponse> {
    return invokeCommand<CreateDownloadJobResponse>('create_download_job', { request })
}

export async function estimateDownloadJob(
    request: DownloadJobRequest,
): Promise<DownloadJobEstimate> {
    return invokeCommand<DownloadJobEstimate>('estimate_download_job', { request })
}

export async function getDownloadJobs(limit?: number): Promise<DownloadJob[]> {
    return invokeCommand<DownloadJob[]>('get_download_jobs', { limit })
}

export async function getDownloadJob(id: number): Promise<DownloadJob> {
    return invokeCommand<DownloadJob>('get_download_job', { id })
}

// ── D07 (spec #111 / issue #119): ciclo de vida y evento de job ──────────────

/**
 * Payload de `syncify:download_job_progress`. El contrato es snake_case, tal
 * como emite serde en `download/jobs.rs::DownloadJobProgressEvent`.
 */
export interface DownloadJobProgress {
    job_id: number
    label?: string | null
    status: DownloadJobStatus
    total: number
    queued: number
    downloading: number
    completed: number
    failed: number
    cancelled: number
    skipped: number
    percent: number
    terminal: boolean
}

/**
 * Cancela las filas pendientes (`queued`) de un job; la descarga activa
 * termina. Devuelve el job refrescado.
 */
export async function cancelDownloadJob(jobId: number): Promise<DownloadJob> {
    return invokeCommand<DownloadJob>('cancel_download_job', { jobId })
}

/**
 * Reencola las filas `failed` del job y lo reabre si estaba terminal.
 * Devuelve el job refrescado.
 */
export async function retryDownloadJobFailed(jobId: number): Promise<DownloadJob> {
    return invokeCommand<DownloadJob>('retry_download_job_failed', { jobId })
}

/** Borra del historial los jobs terminales. Devuelve cuántos cayeron. */
export async function clearDownloadJobs(): Promise<number> {
    return invokeCommand<number>('clear_download_jobs')
}

/** Escucha `syncify:download_job_progress` (creación, progreso con throttle, terminal). */
export async function onDownloadJobProgress(
    callback: (payload: DownloadJobProgress) => void,
): Promise<UnlistenFn> {
    return listen<DownloadJobProgress>(TauriEvents.DOWNLOAD_JOB_PROGRESS, (e) => {
        callback(e.payload)
    })
}
