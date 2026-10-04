#[allow(unused_imports)]
use super::*;

// Download Commands - submodule of crate::commands
//
// Download queue management

/// Queue tracks for download
#[tauri::command]
pub async fn queue_downloads(
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
) -> Result<String, String> {
    tracing::info!("queue_downloads called with {} tracks", track_ids.len());

    let mut queued = 0;
    for track_id in &track_ids {
        let res = add_to_queue(
            *track_id,
            Some(50),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(true),
            Some(false),
            None,
            state.clone(),
        )
        .await;

        if res.is_ok() {
            queued += 1;
        }
    }

    Ok(format!("Queued {} tracks for download", queued))
}

/// Get current download queue
#[tauri::command]
pub async fn get_download_queue(state: State<'_, AppState>) -> Result<Vec<DownloadItem>, String> {
    tracing::info!("get_download_queue called");

    let downloads = sqlx::query_as::<_, DownloadItem>(
        r#"
        SELECT
            dq.id,
            t.title,
            COALESCE(a.name, 'Unknown') as artist_name,
            dq.status,
            dq.progress_percent
        FROM download_queue dq
        JOIN tracks t ON t.id = dq.track_id
        LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        LEFT JOIN artists a ON a.id = ta.artist_id
        WHERE dq.status IN ('queued', 'downloading')
        ORDER BY dq.priority DESC, dq.created_at ASC
        LIMIT 50
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(downloads)
}

// ==============================================
// SERVICE COMMANDS
// ==============================================

/// Get failed downloads
#[tauri::command]
pub async fn get_failed_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadItem>, String> {
    tracing::info!("get_failed_downloads called");

    let downloads = sqlx::query_as::<_, DownloadItem>(
        r#"
        SELECT
            dq.id,
            t.title,
            COALESCE(a.name, 'Unknown') as artist_name,
            dq.status,
            dq.progress_percent
        FROM download_queue dq
        JOIN tracks t ON t.id = dq.track_id
        LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        LEFT JOIN artists a ON a.id = ta.artist_id
        WHERE dq.status = 'failed'
        ORDER BY dq.created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(downloads)
}

/// Reintenta las descargas fallidas que se pueden reintentar solas.
///
/// Este comando ya no reencola por su cuenta: delega en
/// [`super::queue::perform_retry_all_failed`], que clasifica cada fallo con la
/// taxonomía de errores, se salta los terminales y los que requieren acción
/// del usuario, y para en el quinto intento.
///
/// Antes hacia un UPDATE sin clasificar (`WHERE status = 'failed'`), que
/// devolvía a la cola descargas con un token caducado o una credencial
/// revocada y las reintentaba sin límite. La UI llamaba a este comando, así
/// que el fallo era alcanzable con un clic.
///
/// Existe para que la UI no tenga que conocer dos comandos que hacen lo mismo:
/// `queue::retry_failed` acepta `queue_id: None` y llama al mismo sitio.
#[tauri::command]
pub async fn retry_failed_downloads(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("retry_failed_downloads called");

    let count = super::queue::perform_retry_all_failed(&state.db)
        .await
        .map_err(|e| format!("Database error: {e}"))?;

    tracing::info!(
        requeued = count,
        "Requeued retryable failed downloads (terminal ones skipped)"
    );

    Ok(format!("Requeued {count} failed downloads"))
}

/// Clear failed downloads
#[tauri::command]
pub async fn clear_failed_downloads(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("clear_failed_downloads called");

    let result = sqlx::query("DELETE FROM download_queue WHERE status = 'failed'")
        .execute(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(format!(
        "Cleared {} failed downloads",
        result.rows_affected()
    ))
}

// ==============================================
// DOWNLOAD SERVICE COMMANDS (Rust-native)
// ==============================================

use crate::services::tidal_pipeline::{
    execute_tidal_single_track_download, TidalSingleTrackRequest, TidalSingleTrackResponse,
};

/// Download a single track directly from Tidal with full pipeline (resolution, validation, Vorbis tagging, staging, SQLite persistence)
#[tauri::command]
pub async fn download_tidal_single_track(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    track_id_or_query: String,
    quality: Option<String>,
    output_dir: Option<String>,
    allow_fallback: Option<bool>,
) -> Result<TidalSingleTrackResponse, String> {
    tracing::info!(
        "download_tidal_single_track called for target '{}'",
        track_id_or_query
    );

    let req = TidalSingleTrackRequest {
        track_id_or_query,
        requested_quality: quality,
        output_dir,
        allow_lossy_fallback: allow_fallback,
        ..Default::default()
    };

    let app_clone = app_handle.clone();
    let on_progress = move |event: syncify_core_domain::events::PipelineProgressEvent| {
        // Single canonical progress channel (IN-4): the UI listens on 'syncify:progress'.
        let _ = app_clone.emit("syncify:progress", &event);
    };

    execute_tidal_single_track_download(&state.db, req, on_progress).await
}

// End of file
