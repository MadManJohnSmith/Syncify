use tauri::AppHandle;

#[allow(unused_imports)]
use super::*;

// Download job commands (D02, spec #111) - créación, estimación y consulta.
//
// `create_download_job` devuelve el job inmediatamente: la estimación agregada
// y el encolado por lotes corren en segundo plano, como fija la spec #111.
//
// Ciclo de vida del job (D07, issue #119): cancelar solo pendientes, reanudar
// fallos, limpiar historial terminal. El estado y el throttle del evento viven
// en `download::job_events`; el worker no participa.

use crate::download::job_events;
use crate::download::jobs::{self, DownloadJobEstimate, DownloadJobRequest, DownloadJobRow};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadJobResponse {
    pub job_id: i64,
}

/// Creates a download job and returns its id at once; estimation and batch
/// enqueue continue in the background.
#[tauri::command]
pub async fn create_download_job(
    app: AppHandle,
    state: State<'_, AppState>,
    request: DownloadJobRequest,
) -> Result<CreateDownloadJobResponse, String> {
    let job_id = jobs::create_job(&state.db, &request)
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(
        "[create_download_job] job {} created (kind={})",
        job_id,
        request.kind
    );

    // Contrato D07: el evento de job se emite al crear y el watcher cubre el
    // ciclo hasta la transición terminal.
    job_events::spawn_job_watch(app.clone(), state.db.clone(), job_id);

    let db = state.db.clone();
    let worker_state = state.worker_state.clone();
    tauri::async_runtime::spawn(async move {
        match jobs::run_job(&db, job_id, &request).await {
            Ok(outcome) => {
                tracing::info!(
                    "[create_download_job] job {} enqueued={} already_queued={} duplicate_input={}",
                    job_id,
                    outcome.enqueued,
                    outcome.skipped_existing,
                    outcome.skipped_duplicate_input
                );
                // Encolado terminado: derivar el estado terminal de un job
                // vacío o todo saltado (el watcher lo infiere después vía
                // requested_count > 0, pero así el evento no espera un sondeo).
                if let Err(error) = jobs::refresh_job_status(&db, job_id, Some(true)).await {
                    tracing::warn!(
                        "[create_download_job] job {} refresh after enqueue: {}",
                        job_id,
                        error
                    );
                }
            }
            Err(error) => {
                tracing::warn!("[create_download_job] job {} failed: {}", job_id, error);
                if let Err(mark) = jobs::mark_job_failed(&db, job_id, &error.to_string()).await {
                    tracing::warn!(
                        "[create_download_job] could not mark job {} as failed: {}",
                        job_id,
                        mark
                    );
                }
            }
        }
        if let Err(error) = job_events::emit_job_progress(&app, &db, job_id).await {
            tracing::warn!(
                "[create_download_job] job {} progress emit failed: {}",
                job_id,
                error
            );
        }
        worker_state.notify_available();
    });

    Ok(CreateDownloadJobResponse { job_id })
}

/// Aggregate estimate for a would-be job: counts by source, destination and
/// capacity plus approximate size, without per-track queries.
#[tauri::command]
pub async fn estimate_download_job(
    state: State<'_, AppState>,
    request: DownloadJobRequest,
) -> Result<DownloadJobEstimate, String> {
    jobs::estimate(&state.db, &request)
        .await
        .map_err(|e| e.to_string())
}

/// Lists recent jobs, newest first.
#[tauri::command]
pub async fn get_download_jobs(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<DownloadJobRow>, String> {
    jobs::get_jobs(&state.db, limit)
        .await
        .map_err(|e| e.to_string())
}

/// Reads one job by id.
#[tauri::command]
pub async fn get_download_job(
    state: State<'_, AppState>,
    id: i64,
) -> Result<DownloadJobRow, String> {
    jobs::get_job(&state.db, id)
        .await
        .map_err(|e| e.to_string())
}

/// Cancels a job's pending rows (`queued`); the active download finishes, as
/// fixed by D03. Emits the job event after the transition.
#[tauri::command]
pub async fn cancel_download_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: i64,
) -> Result<DownloadJobRow, String> {
    let outcome = jobs::cancel_job(&state.db, job_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("download job {job_id} not found"))?;
    tracing::info!(
        "[cancel_download_job] job {} cancelled_rows={} status={}",
        job_id,
        outcome.cancelled_rows,
        outcome.job.status
    );
    emit_job_event(&app, &state.db, job_id).await;
    Ok(outcome.job)
}

/// Requeues the failed rows of a job and reopens it if it was terminal.
/// Emits the job event after the transition and wakes the worker.
#[tauri::command]
pub async fn retry_download_job_failed(
    app: AppHandle,
    state: State<'_, AppState>,
    job_id: i64,
) -> Result<DownloadJobRow, String> {
    let job = jobs::retry_job_failed(&state.db, job_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("download job {job_id} not found"))?;
    tracing::info!(
        "[retry_download_job_failed] job {} status={}",
        job_id,
        job.status
    );
    state.worker_state.notify_available();
    emit_job_event(&app, &state.db, job_id).await;
    Ok(job)
}

/// Removes terminal jobs (completed, partial, cancelled, failed) from the
/// history. Running jobs and their queue rows are untouched.
#[tauri::command]
pub async fn clear_download_jobs(state: State<'_, AppState>) -> Result<i64, String> {
    let (deleted, pruned) = jobs::clear_terminal_jobs(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    for job_id in pruned {
        job_events::forget_job_queues(job_id);
    }
    tracing::info!("[clear_download_jobs] removed {} terminal jobs", deleted);
    Ok(deleted as i64)
}

/// Emisión best-effort del evento de job tras una transición de comando: un
/// fallo de emisión no debe fallar la operación que ya se aplicó.
async fn emit_job_event(app: &AppHandle, db: &crate::DbPool, job_id: i64) {
    if let Err(error) = job_events::emit_job_progress(app, db, job_id).await {
        tracing::warn!(
            "[download_jobs] job {} progress emit failed: {}",
            job_id,
            error
        );
    }
}
