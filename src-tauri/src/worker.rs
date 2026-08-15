//! Background Download Worker
//!
//! Automatically processes the download queue in the background.
//! Supports pause/resume, progress events, and concurrency control.

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Notify;

/// Metadata needed to create a download request
#[derive(Debug, FromRow)]
struct TrackMeta {
    title: Option<String>,
    isrc: Option<String>,
    duration_ms: Option<i64>,
    track_number: Option<i32>,
    disc_number: Option<i32>,
    total_tracks: Option<i32>,
    album_name: Option<String>,
    release_date: Option<String>,
    spotify_id: Option<String>,
    artist_name: Option<String>,
    album_artist: Option<String>,
}

/// Worker state shared across commands
#[derive(Clone)]
pub struct DownloadWorkerState {
    /// Whether the worker is paused
    paused: Arc<AtomicBool>,
    /// Whether the worker should stop completely
    stopped: Arc<AtomicBool>,
    /// Number of active downloads
    active_count: Arc<AtomicUsize>,
    /// Maximum concurrent downloads
    max_concurrent: Arc<AtomicUsize>,
    /// Notify when unpaused
    unpause_notify: Arc<Notify>,
}

impl Default for DownloadWorkerState {
    fn default() -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            active_count: Arc::new(AtomicUsize::new(0)),
            max_concurrent: Arc::new(AtomicUsize::new(2)),
            unpause_notify: Arc::new(Notify::new()),
        }
    }
}

impl DownloadWorkerState {
    pub fn new(max_concurrent: usize) -> Self {
        let state = Self::default();
        state.max_concurrent.store(max_concurrent, Ordering::SeqCst);
        state
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        self.unpause_notify.notify_waiters();
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.unpause_notify.notify_waiters(); // Wake up waiting tasks
    }

    pub fn active_downloads(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent.load(Ordering::SeqCst)
    }

    pub fn set_max_concurrent(&self, max: usize) {
        self.max_concurrent.store(max, Ordering::SeqCst);
    }

    pub async fn wait_if_paused(&self) {
        while self.is_paused() && !self.is_stopped() {
            self.unpause_notify.notified().await;
        }
    }

    fn increment_active(&self) {
        self.active_count.fetch_add(1, Ordering::SeqCst);
    }

    fn decrement_active(&self) {
        self.active_count.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Progress event for download worker
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgressEvent {
    pub queue_id: i64,
    pub track_id: i64,
    pub title: String,
    pub artist: String,
    pub status: String, // "started", "downloading", "complete", "failed"
    pub progress_percent: f64,
    pub message: Option<String>,
}

/// The background download worker
pub struct DownloadWorker {
    db: SqlitePool,
    state: DownloadWorkerState,
    app_handle: Option<tauri::AppHandle>,
}

impl DownloadWorker {
    pub fn new(db: SqlitePool, state: DownloadWorkerState) -> Self {
        Self {
            db,
            state,
            app_handle: None,
        }
    }

    pub fn with_app_handle(mut self, handle: tauri::AppHandle) -> Self {
        self.app_handle = Some(handle);
        self
    }

    /// Emit a download progress event to the frontend
    fn emit_progress(&self, event: DownloadProgressEvent) {
        if let Some(handle) = &self.app_handle {
            let _ = handle.emit("syncify:download_progress", &event);
        }
    }

    /// Get the next queued item
    async fn get_next_item(&self) -> Option<(i64, i64, String, String)> {
        let item: Option<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT dq.id, dq.track_id, t.title,
                   (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta 
                    JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id) as artist
            FROM download_queue dq
            LEFT JOIN tracks t ON t.id = dq.track_id
            WHERE dq.status = 'queued'
            ORDER BY dq.priority DESC, dq.position ASC, dq.created_at ASC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.db)
        .await
        .ok()?;

        item.map(|(qid, tid, title, artist)| {
            (
                qid,
                tid,
                title.unwrap_or_default(),
                artist.unwrap_or_default(),
            )
        })
    }

    /// Mark item as downloading
    async fn mark_downloading(&self, queue_id: i64) {
        let _ = sqlx::query(
            "UPDATE download_queue SET status = 'downloading', started_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(queue_id)
        .execute(&self.db)
        .await;
    }

    /// Mark item as complete
    async fn mark_complete(&self, queue_id: i64, file_path: Option<&str>) {
        let _ = sqlx::query(
            "UPDATE download_queue SET status = 'complete', completed_at = CURRENT_TIMESTAMP, progress_percent = 100.0 WHERE id = ?"
        )
        .bind(queue_id)
        .execute(&self.db)
        .await;

        // Also insert into downloads table if we have a file path
        if let Some(path) = file_path {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO downloads (track_id, file_path, downloaded_at) 
                 SELECT track_id, ?, CURRENT_TIMESTAMP FROM download_queue WHERE id = ?",
            )
            .bind(path)
            .bind(queue_id)
            .execute(&self.db)
            .await;
        }
    }

    /// Mark item as failed (transient, retryable)
    async fn mark_failed(&self, queue_id: i64, error: &str) {
        let _ = sqlx::query(
            "UPDATE download_queue SET status = 'failed', error_message = ?, last_error = ?, retry_count = retry_count + 1 WHERE id = ?"
        )
        .bind(error)
        .bind(error)
        .bind(queue_id)
        .execute(&self.db)
        .await;
    }

    /// Mark item as permanently failed without automatic retry loop
    async fn mark_permanent_failure(&self, queue_id: i64, status: &str, error: &str) {
        let _ = sqlx::query(
            "UPDATE download_queue SET status = ?, error_message = ?, last_error = ?, retry_count = 99 WHERE id = ?"
        )
        .bind(status)
        .bind(error)
        .bind(error)
        .bind(queue_id)
        .execute(&self.db)
        .await;
    }


    /// Update progress - designed for streaming progress updates from Python subprocess
    #[allow(dead_code)]
    async fn update_progress(&self, queue_id: i64, progress: f64) {
        let _ = sqlx::query("UPDATE download_queue SET progress_percent = ? WHERE id = ?")
            .bind(progress)
            .bind(queue_id)
            .execute(&self.db)
            .await;
    }

    /// Dynamically resolve output directory from folder_settings / settings or fallback
    async fn resolve_download_output_dir(&self) -> String {
        // 1. Check folder_settings.base_folder
        if let Ok(Some((base_folder,))) = sqlx::query_as::<_, (String,)>(
            "SELECT base_folder FROM folder_settings WHERE id = 1 AND base_folder IS NOT NULL AND TRIM(base_folder) != ''"
        )
        .fetch_optional(&self.db)
        .await
        {
            let trimmed = base_folder.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        // 2. Check settings table dl_download_path or download_path
        if let Ok(Some((path_str,))) = sqlx::query_as::<_, (String,)>(
            "SELECT value FROM settings WHERE key IN ('dl_download_path', 'download_path') AND value IS NOT NULL AND TRIM(value) != '' ORDER BY CASE key WHEN 'dl_download_path' THEN 1 ELSE 2 END LIMIT 1"
        )
        .fetch_optional(&self.db)
        .await
        {
            let trimmed = path_str.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        // 3. Fallback to Audio/Syncify
        dirs::audio_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\Music"))
            .join("Syncify")
            .to_string_lossy()
            .to_string()
    }

    /// Process a single download using the download orchestrator (Qobuz preferred -> Tidal fallback)
    async fn process_download(&self, queue_id: i64, track_id: i64, title: &str, artist: &str) {
        self.state.increment_active();

        // Emit started event
        self.emit_progress(DownloadProgressEvent {
            queue_id,
            track_id,
            title: title.to_string(),
            artist: artist.to_string(),
            status: "started".to_string(),
            progress_percent: 0.0,
            message: Some("Starting download...".to_string()),
        });

        self.mark_downloading(queue_id).await;

        // Get full track metadata for download request
        let query_result = sqlx::query_as::<_, TrackMeta>(
            r#"
            SELECT 
                t.title, t.isrc, t.duration_ms,
                t.track_number, t.disc_number, NULL as total_tracks,
                a.title as album_name, a.release_date,
                ts.service_track_id as spotify_id,
                (SELECT GROUP_CONCAT(ar.name, ', ') FROM track_artists ta 
                 JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id) as artist_name,
                (SELECT ar.name FROM track_artists ta 
                 JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id LIMIT 1) as album_artist
            FROM tracks t
            LEFT JOIN albums a ON a.id = t.album_id
            LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (
                SELECT id FROM services WHERE name = 'spotify' LIMIT 1
            )
            WHERE t.id = ?
            "#
        )
        .bind(track_id)
        .fetch_optional(&self.db)
        .await;

        let track_meta = match query_result {
            Ok(meta) => meta,
            Err(e) => {
                tracing::error!("Failed to fetch track metadata for {}: {}", track_id, e);
                None
            }
        };

        // Resolve requested quality preference from queue row
        let quality_pref: Option<String> = sqlx::query_scalar(
            "SELECT quality_preference FROM download_queue WHERE id = ?"
        )
        .bind(queue_id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten();

        let quality = quality_pref.unwrap_or_else(|| "HI_RES_LOSSLESS".to_string());
        let output_dir = self.resolve_download_output_dir().await;

        let result = if let Some(meta) = track_meta {
            // Create download request
            let request = crate::download::DownloadRequest {
                item_id: queue_id.to_string(),
                isrc: meta.isrc.clone(),
                spotify_id: meta.spotify_id.clone(),
                track_name: meta.title.clone().unwrap_or_else(|| title.to_string()),
                artist_name: meta
                    .artist_name
                    .clone()
                    .unwrap_or_else(|| artist.to_string()),
                album_name: meta.album_name.clone().unwrap_or_default(),
                album_artist: meta.album_artist.clone(),
                duration_ms: meta.duration_ms.unwrap_or(0),
                track_number: meta.track_number.unwrap_or(1),
                disc_number: meta.disc_number.unwrap_or(1),
                total_tracks: meta.total_tracks.unwrap_or(1),
                release_date: meta.release_date.clone(),
                cover_url: None,
                output_dir,
                quality,
                embed_lyrics: true,
                embed_artwork: true,
            };

            // Use the Rust download orchestrator with SQLite active account resolution
            let orchestrator = crate::download::DownloadOrchestrator::new().with_db(self.db.clone());
            
            match orchestrator.download_track(&request).await {
                Ok(download_result) => Ok((
                    download_result.file_path,
                    download_result.service,
                    download_result.bit_depth,
                    download_result.sample_rate,
                )),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Track metadata not found in database".to_string())
        };

        // Update status based on result
        match result {
            Ok((file_path, service, bit_depth, sample_rate)) => {
                self.mark_complete(queue_id, Some(&file_path)).await;
                self.emit_progress(DownloadProgressEvent {
                    queue_id,
                    track_id,
                    title: title.to_string(),
                    artist: artist.to_string(),
                    status: "complete".to_string(),
                    progress_percent: 100.0,
                    message: Some(format!("Download complete via {} ({}bit/{}kHz)", service, bit_depth, (sample_rate as f64 / 1000.0))),
                });
                if let Some(handle) = &self.app_handle {
                    let notif = crate::commands::AppNotification::new(
                        crate::commands::NotificationKind::Success,
                        "Download Complete",
                        format!("{} - {} (via {})", artist, title, service),
                        crate::commands::NotificationCategory::Download,
                        Some(serde_json::json!({ 
                            "queue_id": queue_id, 
                            "track_id": track_id, 
                            "file_path": file_path, 
                            "service": service,
                            "bit_depth": bit_depth,
                            "sample_rate": sample_rate
                        })),
                    );
                    let _ = crate::commands::emit_app_notification(handle, &notif);
                }
                tracing::info!("Downloaded via {}: {} - {} -> {}", service, artist, title, file_path);
            }
            Err(error) => {
                let (final_status, is_permanent) = if error.contains("RequiresAuth") || error.contains("PlaybackUnauthorized") || error.contains("401") {
                    ("requires_auth", true)
                } else if error.contains("RejectedQuality") || error.contains("downgrade rejected") {
                    ("rejected_quality", true)
                } else if error.contains("TrackUnresolved") || error.contains("NotFound") || error.contains("not found on") {
                    ("not_found", true)
                } else {
                    ("failed", false)
                };

                if is_permanent {
                    self.mark_permanent_failure(queue_id, final_status, &error).await;
                } else {
                    self.mark_failed(queue_id, &error).await;
                }

                self.emit_progress(DownloadProgressEvent {
                    queue_id,
                    track_id,
                    title: title.to_string(),
                    artist: artist.to_string(),
                    status: final_status.to_string(),
                    progress_percent: 0.0,
                    message: Some(error.clone()),
                });
                if let Some(handle) = &self.app_handle {
                    let notif = crate::commands::AppNotification::new(
                        crate::commands::NotificationKind::Error,
                        "Download Failed",
                        format!("{} - {}: {}", artist, title, error),
                        crate::commands::NotificationCategory::Download,
                        Some(serde_json::json!({ "queue_id": queue_id, "track_id": track_id, "status": final_status, "error": error })),
                    );
                    let _ = crate::commands::emit_app_notification(handle, &notif);
                }
                tracing::warn!("Download error [{}]: {} - {} - {}", final_status, artist, title, error);
            }
        }


        self.state.decrement_active();
    }

    /// Run the background worker loop
    pub async fn run(&self) {
        tracing::info!("Download worker started");

        // Pause interrupted downloads on startup to prevent automatic mass execution during test
        let paused_count = sqlx::query(
            "UPDATE download_queue SET status = 'paused', started_at = NULL WHERE status = 'downloading'"
        )
        .execute(&self.db)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);

        if paused_count > 0 {
            tracing::info!("Paused {} interrupted downloads on startup to isolate testing", paused_count);
        }


        loop {
            // Check if stopped
            if self.state.is_stopped() {
                tracing::info!("Download worker stopped");
                break;
            }

            // Wait if paused
            self.state.wait_if_paused().await;

            // Check again after unpause
            if self.state.is_stopped() {
                break;
            }

            // Check concurrency limit
            if self.state.active_downloads() >= self.state.max_concurrent() {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                continue;
            }

            // Get next item
            if let Some((queue_id, track_id, title, artist)) = self.get_next_item().await {
                // Process download directly (worker is already in background task)
                self.process_download(queue_id, track_id, &title, &artist)
                    .await;
            } else {
                // No items in queue, wait before checking again
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        }
    }
}

/// Worker status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub running: bool,
    pub paused: bool,
    pub active_downloads: usize,
    pub max_concurrent: usize,
    pub is_running: bool,
    pub is_paused: bool,
}

impl DownloadWorkerState {
    pub fn status(&self) -> WorkerStatus {
        let running = !self.is_stopped();
        let paused = self.is_paused();
        WorkerStatus {
            running,
            paused,
            active_downloads: self.active_downloads(),
            max_concurrent: self.max_concurrent(),
            is_running: running,
            is_paused: paused,
        }
    }
}

