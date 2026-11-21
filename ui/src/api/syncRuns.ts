/**
 * SyncRun — corridas de sincronización con estado (issue #65, ADR-0006).
 *
 * Única puerta IPC para el agregado SyncRun: ninguna vista llama a `invoke`
 * directamente. Este corte cubre Qobuz con scope `favorites`; el resto de
 * proveedores y colecciones responden `Unsupported` desde el backend hasta
 * sus tickets.
 */

import { invokeCommand } from './tauri';

/** Una corrida de sincronización tal como la devuelve el backend. */
export interface SyncRunRecord {
    id: number;
    account_id: number;
    service_id: number;
    mode: string;
    scope: string;
    /** running | completed | partial | failed | cancelled */
    status: string;
    started_at: string;
    finished_at: string | null;
    /** Total que el proveedor reporta; null si no lo publica (desconocida). */
    reported_total: number | null;
    imported_total: number;
    error_count: number;
}

export interface StartSyncRunResponse {
    run_id: number;
    status: string;
}

export interface CancelSyncRunResponse {
    cancel_requested: boolean;
    status: string;
}

/** Lanza la corrida de una cuenta y devuelve su id ya persistido. */
export async function startSyncRun(accountId: number, scope: string = 'favorites'): Promise<StartSyncRunResponse> {
    return invokeCommand<StartSyncRunResponse>('start_sync_run', { accountId, scope });
}

/** Pide la cancelación: se atiende en el límite de página conservando lo importado. */
export async function cancelSyncRun(runId: number): Promise<CancelSyncRunResponse> {
    return invokeCommand<CancelSyncRunResponse>('cancel_sync_run', { runId });
}

/** Lee una corrida por id. */
export async function getSyncRun(runId: number): Promise<SyncRunRecord> {
    return invokeCommand<SyncRunRecord>('get_sync_run', { runId });
}

/** Historial de corridas de la cuenta, la más reciente primero. */
export async function listSyncRuns(accountId: number, limit?: number): Promise<SyncRunRecord[]> {
    return invokeCommand<SyncRunRecord[]>('list_sync_runs', {
        accountId,
        limit: limit ?? null,
    });
}

export const syncRunsApi = {
    startSyncRun,
    cancelSyncRun,
    getSyncRun,
    listSyncRuns,
};
