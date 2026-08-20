// Download Commands - included via include!() in mod.rs
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

/// Retry failed downloads
#[tauri::command]
pub async fn retry_failed_downloads(state: State<'_, AppState>) -> Result<String, String> {
    tracing::info!("retry_failed_downloads called");

    let result = sqlx::query(
        "UPDATE download_queue SET status = 'queued', error_message = NULL, retry_count = retry_count + 1 WHERE status = 'failed'"
    )
    .execute(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    let count = result.rows_affected();
    tracing::info!("Requeued {} failed downloads", count);

    Ok(format!("Requeued {} failed downloads", count))
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
    tracing::info!("download_tidal_single_track called for target '{}'", track_id_or_query);

    let req = TidalSingleTrackRequest {
        track_id_or_query,
        requested_quality: quality,
        output_dir,
        allow_lossy_fallback: allow_fallback,
        ..Default::default()
    };

    let app_clone = app_handle.clone();
    let on_progress = move |event: syncify_core_domain::events::PipelineProgressEvent| {
        let _ = app_clone.emit("pipeline:progress", &event);
        let _ = app_clone.emit("syncify:progress", &event);
    };

    execute_tidal_single_track_download(&state.db, req, on_progress).await
}

// End of file


