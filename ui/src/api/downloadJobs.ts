/**
 * Download Jobs API (D02, spec #111)
 *
 * Un job agrupa filas de la cola persistente. `createDownloadJob` devuelve el
 * id al instante; la estimación agregada y el encolado siguen en segundo
 * plano. `estimateDownloadJob` es una consulta pura (sin escribir).
 */

import { invokeCommand } from './tauri'

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
