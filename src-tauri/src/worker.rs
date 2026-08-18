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
#[allow(dead_code)]
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
    musicbrainz_id: Option<String>,
    acoustid_fingerprint: Option<String>,
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
    /// Notify when a download slot becomes available or item is queued
    slot_available_notify: Arc<Notify>,
}

impl Default for DownloadWorkerState {
    fn default() -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            active_count: Arc::new(AtomicUsize::new(0)),
            max_concurrent: Arc::new(AtomicUsize::new(2)),
            unpause_notify: Arc::new(Notify::new()),
            slot_available_notify: Arc::new(Notify::new()),
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
        self.slot_available_notify.notify_waiters();
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.unpause_notify.notify_waiters(); // Wake up waiting tasks
        self.slot_available_notify.notify_waiters();
    }

    pub fn active_downloads(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }

    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent.load(Ordering::SeqCst)
    }

    pub fn set_max_concurrent(&self, max: usize) {
        self.max_concurrent.store(max, Ordering::SeqCst);
        self.slot_available_notify.notify_waiters();
    }

    pub fn notify_available(&self) {
        self.slot_available_notify.notify_waiters();
    }

    pub async fn wait_if_paused(&self) {
        while self.is_paused() && !self.is_stopped() {
            self.unpause_notify.notified().await;
        }
    }

    pub fn increment_active(&self) {
        self.active_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn decrement_active(&self) {
        let _ = self.active_count.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |val| {
            Some(val.saturating_sub(1))
        });
        self.slot_available_notify.notify_waiters();
    }
}

/// RAII guard to ensure active download count is always decremented upon completion or error
pub struct ActiveDownloadGuard(pub DownloadWorkerState);

impl Drop for ActiveDownloadGuard {
    fn drop(&mut self) {
        self.0.decrement_active();
    }
}

/// Progress event for download worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub queue_id: i64,
    pub track_id: i64,
    pub title: String,
    pub artist: String,
    pub status: String, // "started", "downloading", "complete", "failed"
    pub progress_percent: f64,
    pub message: Option<String>,
    #[serde(default)]
    pub bytes_downloaded: u64,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub percent: Option<f64>,
    #[serde(default)]
    pub instant_kbps: f64,
    #[serde(default)]
    pub average_kbps: f64,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub terminal: bool,
}

/// The background download worker
#[derive(Clone)]
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
        self.app_handle = Some(handle.clone());
        let handle_clone = handle.clone();
        crate::download::progress::PROGRESS_TRACKER.set_emitter(move |prog| {
            let q_id = prog.item_id.parse::<i64>().unwrap_or(0);
            let status_str = match prog.status {
                crate::download::progress::DownloadStatus::Queued => "queued",
                crate::download::progress::DownloadStatus::Searching => "searching",
                crate::download::progress::DownloadStatus::Downloading => "downloading",
                crate::download::progress::DownloadStatus::Finalizing => "finalizing",
                crate::download::progress::DownloadStatus::Complete => "complete",
                crate::download::progress::DownloadStatus::Failed => "failed",
                crate::download::progress::DownloadStatus::Cancelled => "failed",
            };

            let evt = DownloadProgressEvent {
                queue_id: q_id,
                track_id: 0,
                title: String::new(),
                artist: String::new(),
                status: status_str.to_string(),
                progress_percent: prog.percent.unwrap_or(0.0) as f64,
                message: prog.message.clone(),
                bytes_downloaded: prog.bytes_downloaded,
                total_bytes: prog.total_bytes,
                percent: prog.percent.map(|p| p as f64),
                instant_kbps: prog.instant_kbps,
                average_kbps: prog.average_kbps,
                phase: prog.phase.clone(),
                terminal: prog.terminal,
            };

            let _ = handle_clone.emit("syncify:download_progress", &evt);
            let is_term = prog.terminal || status_str == "complete" || status_str == "failed";
            let norm_status = if is_term && status_str != "complete" {
                "failed"
            } else {
                status_str
            };

            let _ = handle_clone.emit(
                "syncify:progress",
                serde_json::json!({
                    "item_id": prog.item_id.clone(),
                    "queue_id": q_id,
                    "status": norm_status,
                    "pipeline_status": status_str,
                    "progress_percent": prog.percent.unwrap_or(0.0) as f64,
                    "bytes_downloaded": prog.bytes_downloaded,
                    "total_bytes": prog.total_bytes,
                    "percent": prog.percent.map(|p| p as f64),
                    "instant_kbps": prog.instant_kbps,
                    "average_kbps": prog.average_kbps,
                    "phase": prog.phase.clone(),
                    "message": prog.message.clone(),
                    "terminal": is_term,
                }),
            );
        });
        self
    }

    /// Emit a download progress event to the frontend
    fn emit_progress(&self, event: DownloadProgressEvent) {
        if let Some(handle) = &self.app_handle {
            let _ = handle.emit("syncify:download_progress", &event);

            let is_terminal = event.terminal
                || event.status == "complete"
                || event.status == "failed"
                || event.status == "requires_auth"
                || event.status == "rejected_quality"
                || event.status == "not_found";

            let normalized_status = if is_terminal && event.status != "complete" {
                "failed"
            } else {
                &event.status
            };

            let _ = handle.emit(
                "syncify:progress",
                serde_json::json!({
                    "item_id": event.queue_id.to_string(),
                    "queue_id": event.queue_id,
                    "track_id": event.track_id,
                    "title": event.title,
                    "artist": event.artist,
                    "status": normalized_status,
                    "pipeline_status": event.status,
                    "progress_percent": event.percent.unwrap_or(event.progress_percent),
                    "bytes_downloaded": event.bytes_downloaded,
                    "total_bytes": event.total_bytes,
                    "percent": event.percent,
                    "instant_kbps": event.instant_kbps,
                    "average_kbps": event.average_kbps,
                    "phase": event.phase,
                    "message": event.message,
                    "terminal": is_terminal,
                }),
            );
        }
    }

    /// Get the next queued item
    #[allow(dead_code)]
    pub async fn get_next_item(&self) -> Option<(i64, i64, String, String)> {
        let item: Option<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT dq.id, dq.track_id, 
                   COALESCE(dq.target_title, t.title) as title,
                   COALESCE(dq.target_artist, (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta 
                    JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id)) as artist
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

    /// Atomically claim the next queued item to downloading state
    pub async fn claim_next_item(&self) -> Option<(i64, i64, String, String)> {
        let item: Option<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT dq.id, dq.track_id, 
                   COALESCE(dq.target_title, t.title) as title,
                   COALESCE(dq.target_artist, (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta 
                    JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id)) as artist
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

        if let Some((qid, tid, title, artist)) = item {
            let res = sqlx::query(
                "UPDATE download_queue SET status = 'downloading', started_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'queued'"
            )
            .bind(qid)
            .execute(&self.db)
            .await;

            if let Ok(r) = res {
                if r.rows_affected() > 0 {
                    return Some((
                        qid,
                        tid,
                        title.unwrap_or_default(),
                        artist.unwrap_or_default(),
                    ));
                }
            }
        }
        None
    }

    /// Mark item as downloading
    pub async fn mark_downloading(&self, queue_id: i64) {
        let _ = sqlx::query(
            "UPDATE download_queue SET status = 'downloading', started_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(queue_id)
        .execute(&self.db)
        .await;
    }

    /// Mark item as complete
    async fn mark_complete(
        &self,
        queue_id: i64,
        res: &crate::download::DownloadResult,
    ) {
        let _ = sqlx::query(
            r#"
            UPDATE download_queue 
            SET status = 'complete', 
                completed_at = CURRENT_TIMESTAMP, 
                progress_percent = 100.0,
                origin_service = COALESCE(origin_service, ?),
                origin_service_track_id = COALESCE(origin_service_track_id, ?),
                effective_service = ?,
                effective_service_track_id = ?,
                fallback_reason = ?,
                match_method = ?,
                match_confidence = ?
            WHERE id = ?
            "#
        )
        .bind(&res.origin_service)
        .bind(&res.origin_service_track_id)
        .bind(&res.effective_service)
        .bind(&res.effective_service_track_id)
        .bind(&res.fallback_reason)
        .bind(&res.match_method)
        .bind(res.match_confidence)
        .bind(queue_id)
        .execute(&self.db)
        .await;

        let file_size = tokio::fs::metadata(&res.file_path).await.map(|m| m.len() as i64).ok();
        let effective_srv = res.effective_service.as_deref().unwrap_or(&res.service);
        let _ = sqlx::query(
            r#"
            INSERT INTO downloads (
                track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, downloaded_at,
                origin_service, origin_service_track_id, effective_service, effective_service_track_id, fallback_reason, match_method, match_confidence
            ) 
            SELECT track_id, (SELECT id FROM services WHERE LOWER(name) = LOWER(?)), ?, 'FLAC', ?, ?, ?, CURRENT_TIMESTAMP,
                   ?, ?, ?, ?, ?, ?, ?
            FROM download_queue WHERE id = ?
            ON CONFLICT(track_id) DO UPDATE SET 
                file_path = excluded.file_path, 
                file_format = excluded.file_format,
                bit_depth = excluded.bit_depth,
                sample_rate = excluded.sample_rate,
                file_size_bytes = excluded.file_size_bytes,
                origin_service = excluded.origin_service,
                origin_service_track_id = excluded.origin_service_track_id,
                effective_service = excluded.effective_service,
                effective_service_track_id = excluded.effective_service_track_id,
                fallback_reason = excluded.fallback_reason,
                match_method = excluded.match_method,
                match_confidence = excluded.match_confidence,
                downloaded_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(effective_srv)
        .bind(&res.file_path)
        .bind(res.bit_depth)
        .bind(res.sample_rate)
        .bind(file_size)
        .bind(&res.origin_service)
        .bind(&res.origin_service_track_id)
        .bind(&res.effective_service)
        .bind(&res.effective_service_track_id)
        .bind(&res.fallback_reason)
        .bind(&res.match_method)
        .bind(res.match_confidence)
        .bind(queue_id)
        .execute(&self.db)
        .await;
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

    /// Mark item as permanently failed (non-retryable: requires auth, rejected quality)
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

    /// Process a single download using the download orchestrator with strict source identity
    #[allow(dead_code)]
    pub async fn process_download(&self, queue_id: i64, track_id: i64, title: &str, artist: &str) {
        self.state.increment_active();
        let _guard = ActiveDownloadGuard(self.state.clone());
        self.mark_downloading(queue_id).await;
        self.process_download_internal(queue_id, track_id, title, artist).await;
    }

    /// Process a claimed download task where status is already marked downloading
    pub async fn process_download_claimed(&self, queue_id: i64, track_id: i64, title: &str, artist: &str) {
        self.process_download_internal(queue_id, track_id, title, artist).await;
    }

    async fn process_download_internal(&self, queue_id: i64, track_id: i64, title: &str, artist: &str) {
        // 1. Query full source identity from download_queue row
        let queue_meta: Option<(
            Option<String>, // service_name
            Option<String>, // service_track_id
            Option<String>, // service_album_id
            Option<String>, // target_title
            Option<String>, // target_artist
            Option<String>, // target_album
            Option<String>, // target_isrc
            Option<String>, // quality_preference
            Option<i64>,    // smart_studio_origin
            Option<i64>,    // allow_fallback
        )> = sqlx::query_as(
            r#"
            SELECT service_name, service_track_id, service_album_id, 
                   target_title, target_artist, target_album, target_isrc, 
                   quality_preference, smart_studio_origin, allow_fallback 
            FROM download_queue WHERE id = ?
            "#
        )
        .bind(queue_id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten();

        let (
            s_name,
            s_track_id,
            s_album_id,
            t_title,
            t_artist,
            t_album,
            t_isrc,
            q_pref,
            smart_studio,
            allow_fb,
        ) = queue_meta.unwrap_or((None, None, None, None, None, None, None, None, Some(0), Some(0)));

        let effective_title = t_title.unwrap_or_else(|| title.to_string());
        let effective_artist = t_artist.unwrap_or_else(|| artist.to_string());

        // Emit started event
        self.emit_progress(DownloadProgressEvent {
            queue_id,
            track_id,
            title: effective_title.clone(),
            artist: effective_artist.clone(),
            status: "started".to_string(),
            progress_percent: 0.0,
            message: Some(format!(
                "Starting download{}...",
                s_name.as_deref().map(|s| format!(" via {}", s)).unwrap_or_default()
            )),
            bytes_downloaded: 0,
            total_bytes: None,
            percent: Some(0.0),
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "started".to_string(),
            terminal: false,
        });

        self.mark_downloading(queue_id).await;

        let is_allowed_fallback = allow_fb.unwrap_or(0) != 0;
        let is_smart_studio = smart_studio.unwrap_or(0) != 0;

        if !is_allowed_fallback && !is_smart_studio && (s_track_id.is_none() || s_track_id.as_deref().unwrap_or("").trim().is_empty()) {
            let err_msg = "SourceIdentityMissing: No locked service_track_id and allow_fallback=false".to_string();
            tracing::warn!("[Worker] Rejecting queue item {}: {}", queue_id, err_msg);
            self.mark_permanent_failure(queue_id, "failed", &err_msg).await;
            self.emit_progress(DownloadProgressEvent {
                queue_id,
                track_id,
                title: effective_title.clone(),
                artist: effective_artist.clone(),
                status: "failed".to_string(),
                progress_percent: 0.0,
                message: Some(err_msg.clone()),
                bytes_downloaded: 0,
                total_bytes: None,
                percent: None,
                instant_kbps: 0.0,
                average_kbps: 0.0,
                phase: "failed".to_string(),
                terminal: true,
            });
            return;
        }

        // Get full track metadata for fallback / enrichment
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
                 JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id LIMIT 1) as album_artist,
                t.musicbrainz_id,
                t.acoustid_fingerprint
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

        let quality = q_pref.unwrap_or_else(|| "HI_RES_LOSSLESS".to_string());
        let output_dir = self.resolve_download_output_dir().await;

        let result: Result<crate::download::DownloadResult, String> = if let Some(meta) = track_meta {
            // Create download request with locked source identity
            let request = crate::download::DownloadRequest {
                item_id: queue_id.to_string(),
                isrc: t_isrc.or_else(|| meta.isrc.clone()),
                musicbrainz_recording_id: meta.musicbrainz_id.clone(),
                acoustid_fingerprint: meta.acoustid_fingerprint.clone(),
                spotify_id: meta.spotify_id.clone(),
                service_name: s_name.clone(),
                service_track_id: s_track_id.clone(),
                service_album_id: s_album_id.clone(),
                track_name: effective_title.clone(),
                artist_name: effective_artist.clone(),
                album_name: t_album.or(meta.album_name).unwrap_or_default(),
                album_artist: meta.album_artist.clone(),
                duration_ms: meta.duration_ms.unwrap_or(0),
                track_number: meta.track_number.unwrap_or(1),
                disc_number: meta.disc_number.unwrap_or(1),
                total_tracks: meta.total_tracks.unwrap_or(1),
                release_date: meta.release_date.clone(),
                cover_url: None,
                output_dir: output_dir.clone(),
                quality: quality.clone(),
                embed_lyrics: true,
                embed_artwork: true,
                smart_studio_origin: is_smart_studio,
                allow_fallback: is_allowed_fallback,
                strict_quality: !is_allowed_fallback,
            };

            tracing::info!(
                "[Worker] Processing item {} (track_id={}, service={:?}, service_track_id={:?}, album='{}', allow_fallback={})",
                queue_id, track_id, request.service_name, request.service_track_id, request.album_name, request.allow_fallback
            );

            // Use the Rust download orchestrator with SQLite active account resolution
            let orchestrator = crate::download::DownloadOrchestrator::new().with_db(self.db.clone());
            
            orchestrator.download_track(&request).await.map_err(|e| e.to_string())
        } else if !effective_title.is_empty() {
            let request = crate::download::DownloadRequest {
                item_id: queue_id.to_string(),
                isrc: t_isrc,
                musicbrainz_recording_id: None,
                acoustid_fingerprint: None,
                spotify_id: None,
                service_name: s_name.clone(),
                service_track_id: s_track_id.clone(),
                service_album_id: s_album_id.clone(),
                track_name: effective_title.clone(),
                artist_name: effective_artist.clone(),
                album_name: t_album.unwrap_or_default(),
                album_artist: None,
                duration_ms: 0,
                track_number: 1,
                disc_number: 1,
                total_tracks: 1,
                release_date: None,
                cover_url: None,
                output_dir: output_dir.clone(),
                quality,
                embed_lyrics: true,
                embed_artwork: true,
                smart_studio_origin: smart_studio.unwrap_or(0) != 0,
                allow_fallback: allow_fb.unwrap_or(0) != 0,
                strict_quality: allow_fb.unwrap_or(0) == 0,
            };

            tracing::info!(
                "[Worker] Processing ad-hoc item {} (track_id={}, service={:?}, service_track_id={:?}, album='{}', allow_fallback={})",
                queue_id, track_id, request.service_name, request.service_track_id, request.album_name, request.allow_fallback
            );

            let orchestrator = crate::download::DownloadOrchestrator::new().with_db(self.db.clone());
            
            orchestrator.download_track(&request).await.map_err(|e| e.to_string())
        } else {
            Err("Track metadata not found in database".to_string())
        };

        // Update status based on result
        match result {
            Ok(download_result) => {
                let file_path = download_result.file_path.clone();
                let service = download_result.service.clone();
                let bit_depth = download_result.bit_depth;
                let sample_rate = download_result.sample_rate;

                self.mark_complete(queue_id, &download_result).await;
                let file_size = tokio::fs::metadata(&file_path).await.map(|m| m.len()).ok();
                let _ = crate::services::ManifestWriter::generate_and_save_manifest(&self.db, std::path::Path::new(&output_dir)).await;
                self.emit_progress(DownloadProgressEvent {
                    queue_id,
                    track_id,
                    title: title.to_string(),
                    artist: artist.to_string(),
                    status: "complete".to_string(),
                    progress_percent: 100.0,
                    message: Some(format!("Download complete via {} ({}bit/{}kHz)", service, bit_depth, (sample_rate as f64 / 1000.0))),
                    bytes_downloaded: file_size.unwrap_or(0),
                    total_bytes: file_size,
                    percent: Some(100.0),
                    instant_kbps: 0.0,
                    average_kbps: 0.0,
                    phase: "complete".to_string(),
                    terminal: true,
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
                // Ensure staging artifact is cleaned up upon error
                let staging_file = std::path::PathBuf::from(&output_dir)
                    .join(".staging")
                    .join(format!("{}.part", queue_id));
                if staging_file.exists() {
                    let _ = tokio::fs::remove_file(staging_file).await;
                }

                let is_auth_error = error.contains("RequiresAuth") || error.contains("PlaybackUnauthorized") || error.contains("401");
                let is_permanent = is_auth_error 
                    || error.contains("RejectedQuality") 
                    || error.contains("downgrade rejected") 
                    || error.contains("TrackUnresolved") 
                    || error.contains("NotFound") 
                    || error.contains("not found on") 
                    || error.contains("404") 
                    || error.contains("StaleSource") 
                    || error.contains("track/get failed")
                    || error.contains("NetworkExhausted");

                if is_permanent {
                    self.mark_permanent_failure(queue_id, "failed", &error).await;
                } else {
                    self.mark_failed(queue_id, &error).await;
                }

                if is_auth_error {
                    let target_service = s_name.as_deref().map(|s| s.to_lowercase()).or_else(|| {
                        let err_lower = error.to_lowercase();
                        if err_lower.contains("qobuz") {
                            Some("qobuz".to_string())
                        } else if err_lower.contains("tidal") {
                            Some("tidal".to_string())
                        } else if err_lower.contains("spotify") {
                            Some("spotify".to_string())
                        } else if err_lower.contains("deezer") {
                            Some("deezer".to_string())
                        } else if err_lower.contains("soundcloud") {
                            Some("soundcloud".to_string())
                        } else {
                            None
                        }
                    });

                    if let Some(srv) = target_service {
                        let update_res = sqlx::query(
                            r#"
                            UPDATE accounts 
                            SET credentials_invalid = 1,
                                invalid_reason = 'token_expired',
                                last_auth_error = ?
                            WHERE service_id IN (SELECT id FROM services WHERE LOWER(name) = LOWER(?))
                            "#
                        )
                        .bind(&error)
                        .bind(&srv)
                        .execute(&self.db)
                        .await;

                        if let Ok(affected) = update_res {
                            tracing::info!(
                                "[Worker] Marked {} account(s) as credentials_invalid due to auth failure on {}",
                                affected.rows_affected(), srv
                            );
                        }

                        if let Some(handle) = &self.app_handle {
                            use tauri::Emitter;
                            let _ = handle.emit("auth-session-expired", serde_json::json!({
                                "service": srv,
                                "error": error,
                                "reason": "token_expired",
                            }));
                        }
                    }
                }

                let status_str = if is_auth_error {
                    "requires_auth"
                } else if error.contains("RejectedQuality") || error.contains("downgrade rejected") {
                    "rejected_quality"
                } else if error.contains("TrackUnresolved") || error.contains("NotFound") || error.contains("not found on") || error.contains("404") || error.contains("StaleSource") || error.contains("track/get failed") {
                    "not_found"
                } else {
                    "failed"
                };

                self.emit_progress(DownloadProgressEvent {
                    queue_id,
                    track_id,
                    title: title.to_string(),
                    artist: artist.to_string(),
                    status: status_str.to_string(),
                    progress_percent: 0.0,
                    message: Some(error.clone()),
                    bytes_downloaded: 0,
                    total_bytes: None,
                    percent: None,
                    instant_kbps: 0.0,
                    average_kbps: 0.0,
                    phase: status_str.to_string(),
                    terminal: true,
                });
                if let Some(handle) = &self.app_handle {
                    let notif = crate::commands::AppNotification::new(
                        crate::commands::NotificationKind::Error,
                        "Download Failed",
                        format!("{} - {}: {}", artist, title, error),
                        crate::commands::NotificationCategory::Download,
                        Some(serde_json::json!({ "queue_id": queue_id, "track_id": track_id, "status": status_str, "error": error })),
                    );
                    let _ = crate::commands::emit_app_notification(handle, &notif);
                }
                tracing::warn!("Download error [{}]: {} - {} - {}", status_str, artist, title, error);
            }
        }
    }

    /// Run the background worker loop
    pub async fn run(&self) {
        tracing::info!("Download worker started");
        self.state.active_count.store(0, Ordering::SeqCst);

        // Reset interrupted downloads on startup back to queued so worker resumes them
        let reset_count = sqlx::query(
            "UPDATE download_queue SET status = 'queued', started_at = NULL WHERE status = 'downloading'"
        )
        .execute(&self.db)
        .await
        .map(|r| r.rows_affected())
        .unwrap_or(0);

        if reset_count > 0 {
            tracing::info!("Reset {} interrupted downloads on startup back to queued", reset_count);
        }

        // Repair legacy queue rows with missing service_track_id
        Self::repair_unresolved_queue_sources(&self.db).await;

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
                tokio::select! {
                    _ = self.state.slot_available_notify.notified() => {},
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {},
                }
                continue;
            }

            // Claim next item and spawn concurrent execution
            if let Some((queue_id, track_id, title, artist)) = self.claim_next_item().await {
                self.state.increment_active();
                let worker = self.clone();
                tokio::spawn(async move {
                    let _guard = ActiveDownloadGuard(worker.state.clone());
                    worker.process_download_claimed(queue_id, track_id, &title, &artist).await;
                });
            } else {
                // No items in queue, wait before checking again or until notified of newly queued items
                tokio::select! {
                    _ = self.state.slot_available_notify.notified() => {},
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {},
                }
            }
        }
    }

    /// Quarantine any legacy queue rows that have NULL or empty service_track_id
    pub async fn repair_unresolved_queue_sources(db: &sqlx::SqlitePool) {
        // Find queued items with missing service_track_id where allow_fallback is false
        let unresolved_items: Vec<(i64, i64, Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT id, track_id, service_name, allow_fallback FROM download_queue WHERE status = 'queued' AND (service_track_id IS NULL OR TRIM(service_track_id) = '')"
        )
        .fetch_all(db)
        .await
        .unwrap_or_default();

        let count = unresolved_items.len();
        if count == 0 {
            return;
        }

        tracing::info!("[Worker] Found {} legacy unresolved queue rows. Quarantining as SourceIdentityMissing...", count);
        let mut quarantined = 0;

        for (qid, _tid, _s_name_opt, allow_fb) in unresolved_items {
            if allow_fb.unwrap_or(0) == 0 {
                let reason = "SourceIdentityMissing: Legacy queue row without locked source identity";
                let _ = sqlx::query(
                    "UPDATE download_queue SET status = 'failed', error_message = ?, last_error = ?, retry_count = 99 WHERE id = ?"
                )
                .bind(reason)
                .bind(reason)
                .bind(qid)
                .execute(db)
                .await;
                quarantined += 1;
            }
        }

        tracing::info!("[Worker] Queue quarantine complete: {} legacy rows marked failed (SourceIdentityMissing)", quarantined);
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

