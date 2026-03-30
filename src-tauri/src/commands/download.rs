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
        let result = sqlx::query(
            "INSERT OR IGNORE INTO download_queue (track_id, status, priority) VALUES (?, 'queued', 50)"
        )
        .bind(track_id)
        .execute(&state.db)
        .await;

        if result.is_ok() {
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

// End of file

