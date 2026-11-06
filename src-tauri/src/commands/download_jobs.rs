#[allow(unused_imports)]
use super::*;

// Download job commands (D02, spec #111) - créación, estimación y consulta.
//
// `create_download_job` devuelve el job inmediatamente: la estimación agregada
// y el encolado por lotes corren en segundo plano, como fija la spec #111.

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

    let db = state.db.clone();
    let worker_state = state.worker_state.clone();
    tauri::async_runtime::spawn(async move {
        match jobs::run_job(&db, job_id, &request).await {
            Ok(outcome) => tracing::info!(
                "[create_download_job] job {} enqueued={} already_queued={} duplicate_input={}",
                job_id,
                outcome.enqueued,
                outcome.skipped_existing,
                outcome.skipped_duplicate_input
            ),
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
