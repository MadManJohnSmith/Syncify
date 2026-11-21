/**
 * syncRuns.spec.ts
 * Issue #65 (ADR-0006): el wrapper SyncRun es la única puerta IPC de las
 * corridas — ninguna vista llama a `invoke` directamente. Fija el contrato
 * de comandos y parámetros que la ficha de cuenta consume.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { syncRunsApi } from '@/api/syncRuns';
import { mockInvoke, resetMocks } from '../setup';

describe('sync_run_ipc_contract_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('start_sync_run pasa accountId y scope con sus defaults del contrato', async () => {
        mockInvoke((cmd) => (cmd === 'start_sync_run' ? { run_id: 7, status: 'running' } : null));

        const res = await syncRunsApi.startSyncRun(42);

        expect(res).toEqual({ run_id: 7, status: 'running' });
    });

    it('start_sync_run acepta un scope explícito', async () => {
        mockInvoke((cmd) => (cmd === 'start_sync_run' ? { run_id: 8, status: 'running' } : null));

        await syncRunsApi.startSyncRun(42, 'favorites');
    });

    it('cancel_sync_run devuelve la petición registrada y el estado de la fila', async () => {
        mockInvoke((cmd) =>
            cmd === 'cancel_sync_run' ? { cancel_requested: true, status: 'running' } : null
        );

        const res = await syncRunsApi.cancelSyncRun(7);

        expect(res.cancel_requested).toBe(true);
        expect(res.status).toBe('running');
    });

    it('get_sync_run devuelve la corrida con sus marcas de tiempo', async () => {
        mockInvoke((cmd) =>
            cmd === 'get_sync_run'
                ? {
                      id: 7,
                      account_id: 42,
                      service_id: 5,
                      mode: 'import_only',
                      scope: 'favorites',
                      status: 'completed',
                      started_at: '2026-11-16 12:00:00',
                      finished_at: '2026-11-16 12:00:05',
                      reported_total: 2,
                      imported_total: 2,
                      error_count: 0,
                  }
                : null
        );

        const run = await syncRunsApi.getSyncRun(7);

        expect(run.id).toBe(7);
        expect(run.status).toBe('completed');
        expect(run.finished_at).toBe('2026-11-16 12:00:05');
    });

    it('list_sync_runs devuelve el historial de la cuenta', async () => {
        mockInvoke((cmd) =>
            cmd === 'list_sync_runs'
                ? [
                      {
                          id: 7,
                          account_id: 42,
                          service_id: 5,
                          mode: 'import_only',
                          scope: 'favorites',
                          status: 'cancelled',
                          started_at: '2026-11-16 12:00:00',
                          finished_at: '2026-11-16 12:00:02',
                          reported_total: 3,
                          imported_total: 1,
                          error_count: 0,
                      },
                  ]
                : null
        );

        const runs = await syncRunsApi.listSyncRuns(42, 5);

        expect(runs).toHaveLength(1);
        expect(runs[0].status).toBe('cancelled');
        expect(runs[0].imported_total).toBe(1);
    });
});
