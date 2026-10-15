//! Background Metadata Enrichment Worker
//!
//! Automatically enriches tracks in the background with MusicBrainz,
//! respecting centralized RateLimiter, backoff, and progress persistence.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Notify;

use crate::services::enrichment::EnrichmentEngine;
use crate::services::rate_limiter::RateLimiter;

/// Enrichment event payload for UI logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentProgressEvent {
    pub track_id: i64,
    pub title: String,
    pub artist: String,
    pub service: String,
    pub status: String,
    pub message: String,
    pub timestamp: String,
}

/// Overall enrichment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentStatus {
    pub is_paused: bool,
    pub active_jobs: usize,
    pub pending_count: i64,
    pub completed_count: i64,
    pub failed_count: i64,
}

/// Thread-safe enrichment worker state
#[derive(Clone)]
pub struct EnrichmentWorkerState {
    paused: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    active_count: Arc<AtomicUsize>,
    unpause_notify: Arc<Notify>,
}

impl Default for EnrichmentWorkerState {
    fn default() -> Self {
        Self {
            paused: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(false)),
            active_count: Arc::new(AtomicUsize::new(0)),
            unpause_notify: Arc::new(Notify::new()),
        }
    }
}

impl EnrichmentWorkerState {
    pub fn new() -> Self {
        Self::default()
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
        self.unpause_notify.notify_waiters();
    }

    pub fn active_jobs(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }

    pub async fn wait_if_paused(&self) {
        while self.is_paused() && !self.is_stopped() {
            self.unpause_notify.notified().await;
        }
    }
}

fn require_one_row(rows: u64, operation: &str) -> Result<(), String> {
    if rows == 1 {
        Ok(())
    } else {
        Err(format!("{operation}: expected one row, updated {rows}"))
    }
}

async fn finish_enrichment(
    db: &SqlitePool,
    track_id: i64,
    mb_id: Option<&str>,
) -> Result<(), String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let updated_track = sqlx::query(
        "UPDATE tracks SET musicbrainz_id = ?, enrichment_status = 'enriched', enriched_at = datetime('now') WHERE id = ?",
    )
    .bind(mb_id)
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    require_one_row(updated_track.rows_affected(), "tracks enrichment")?;

    let updated_progress = sqlx::query(
        "UPDATE enrichment_progress SET status = 'completed', completed_at = datetime('now'), last_error = NULL WHERE track_id = ? AND service = 'all' AND status = 'in_progress'",
    )
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    require_one_row(
        updated_progress.rows_affected(),
        "enrichment_progress completion",
    )?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

async fn skip_enrichment(db: &SqlitePool, track_id: i64) -> Result<(), String> {
    // The schema has no 'skipped' status; record the terminal no-op as completed.
    let updated = sqlx::query(
        "UPDATE enrichment_progress SET status = 'completed', completed_at = datetime('now'), last_error = 'MusicBrainz enrichment disabled in settings' WHERE track_id = ? AND service = 'all' AND status = 'in_progress'",
    )
    .bind(track_id)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    require_one_row(updated.rows_affected(), "enrichment_progress skip")
}

async fn recover_interrupted_jobs(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE enrichment_progress SET status = 'pending' WHERE service = 'all' AND status = 'in_progress'")
        .execute(db)
        .await?;
    Ok(())
}

/// Background enrichment worker
pub struct EnrichmentWorker {
    db: SqlitePool,
    state: EnrichmentWorkerState,
    rate_limiter: Arc<RateLimiter>,
    engine: EnrichmentEngine,
    app_handle: Option<tauri::AppHandle>,
}

// Keep the counter balanced on every return path, including database errors.
struct ActiveJob<'a>(&'a AtomicUsize);

impl Drop for ActiveJob<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl EnrichmentWorker {
    pub fn new(
        db: SqlitePool,
        state: EnrichmentWorkerState,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            engine: EnrichmentEngine::with_db(db.clone()),
            db,
            state,
            rate_limiter,
            app_handle: None,
        }
    }

    pub fn with_app_handle(mut self, handle: tauri::AppHandle) -> Self {
        self.app_handle = Some(handle);
        self
    }

    fn emit_event(&self, event: EnrichmentProgressEvent) {
        if let Some(handle) = &self.app_handle {
            let _ = handle.emit("syncify:enrichment_event", &event);
            let bg_status = match event.status.as_str() {
                "completed" => "completed",
                "failed" => "error",
                _ => "running",
            };
            let _ = handle.emit(
                "background-enrichment-status",
                serde_json::json!({
                    "type": "musicbrainz",
                    "status": bg_status,
                    "message": event.message.clone(),
                    "track_id": event.track_id,
                    "title": event.title.clone(),
                    "artist": event.artist.clone(),
                    "enriched": if event.status == "completed" { 1 } else { 0 },
                    "processed": 1,
                }),
            );
        }
    }

    /// Run the enrichment worker loop
    pub async fn run(&self) {
        tracing::info!("EnrichmentWorker started");

        // A prior process may have exited mid-request. Requeue only this worker's claims.
        if let Err(error) = recover_interrupted_jobs(&self.db).await {
            tracing::error!("Unable to recover enrichment jobs: {error}");
            return;
        }

        let mut had_work = false;
        while !self.state.is_stopped() {
            self.state.wait_if_paused().await;

            if self.state.is_stopped() {
                break;
            }

            match self.process_next_track().await {
                Ok(has_work) => {
                    if has_work {
                        had_work = true;
                    } else {
                        // The UI keeps a single task alive for the whole run
                        // (per-track completions do not close it), so it needs
                        // this one signal when the pending queue drains.
                        if had_work {
                            self.emit_queue_drained();
                        }
                        had_work = false;
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Error in EnrichmentWorker loop: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                }
            }
        }

        tracing::info!("EnrichmentWorker stopped");
    }

    fn emit_queue_drained(&self) {
        if let Some(handle) = &self.app_handle {
            let _ = handle.emit(
                "background-enrichment-status",
                serde_json::json!({
                    "type": "musicbrainz",
                    "status": "waiting",
                    "message": "Enrichment queue empty",
                }),
            );
        }
    }

    /// Process a single track
    pub async fn process_next_track(&self) -> Result<bool, String> {
        // 1. Find next pending track
        let pending: Option<(i64, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT t.id, t.title,
                   (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id LIMIT 1) as artist,
                   (SELECT alb.title FROM albums alb WHERE alb.id = t.album_id) as album,
                   t.isrc
            FROM tracks t
            LEFT JOIN enrichment_progress ep ON ep.track_id = t.id AND ep.service = 'all'
            WHERE (ep.status IS NULL OR ep.status = 'pending' OR (ep.status = 'failed' AND ep.retry_count < 3))
            ORDER BY t.id ASC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.db)
        .await
        .map_err(|e| e.to_string())?;

        let (track_id, title, artist_opt, album_opt, isrc_opt) = match pending {
            Some(row) => row,
            None => return Ok(false), // No pending work
        };

        let artist = artist_opt.unwrap_or_else(|| "Unknown Artist".to_string());
        let album = album_opt.unwrap_or_else(|| "Unknown Album".to_string());

        // 2. Mark in_progress in enrichment_progress
        sqlx::query(
            r#"
            INSERT INTO enrichment_progress (track_id, service, status, last_attempt)
            VALUES (?, 'all', 'in_progress', datetime('now'))
            ON CONFLICT(track_id, service) DO UPDATE SET
                status = 'in_progress',
                last_attempt = datetime('now')
            "#,
        )
        .bind(track_id)
        .execute(&self.db)
        .await
        .map_err(|e| e.to_string())?;

        self.state.active_count.fetch_add(1, Ordering::SeqCst);
        let _active_job = ActiveJob(&self.state.active_count);

        self.emit_event(EnrichmentProgressEvent {
            track_id,
            title: title.clone(),
            artist: artist.clone(),
            service: "all".to_string(),
            status: "in_progress".to_string(),
            message: format!("Enriching '{}' by '{}'", title, artist),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });

        // MT-01: Check user preferences for MusicBrainz enrichment
        let enable_mb: bool = sqlx::query_scalar::<_, bool>(
            "SELECT enable_musicbrainz FROM metadata_preferences WHERE id = 1",
        )
        .fetch_optional(&self.db)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or(true);

        if !enable_mb {
            tracing::info!(
                "Skipping MusicBrainz query for track {} (enable_musicbrainz disabled in settings)",
                track_id
            );
            skip_enrichment(&self.db, track_id).await?;

            self.emit_event(EnrichmentProgressEvent {
                track_id,
                title,
                artist,
                service: "all".to_string(),
                status: "skipped".to_string(),
                message: "MusicBrainz enrichment disabled in settings".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });

            return Ok(true);
        }

        // 3. Acquire rate limiter permit
        self.rate_limiter.acquire("musicbrainz").await;

        // 4. Resolve metadata
        let result = self
            .engine
            .resolve_track_metadata(&artist, &album, &title, isrc_opt.as_deref(), None)
            .await;

        // 5. Update database on success
        let mb_id = result.musicbrainz_recording_id.value();
        finish_enrichment(&self.db, track_id, mb_id).await?;

        self.emit_event(EnrichmentProgressEvent {
            track_id,
            title,
            artist,
            service: "all".to_string(),
            status: "completed".to_string(),
            message: "Metadata enrichment completed successfully".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });

        Ok(true)
    }

    /// Get current statistics
    pub async fn get_status(&self) -> Result<EnrichmentStatus, String> {
        let stats: (i64, i64, i64) = sqlx::query_as(
            r#"
            SELECT
                (SELECT COUNT(*) FROM tracks t LEFT JOIN enrichment_progress ep ON ep.track_id = t.id AND ep.service = 'all' WHERE ep.status IS NULL OR ep.status = 'pending' OR (ep.status = 'failed' AND ep.retry_count < 3)) as pending,
                (SELECT COUNT(*) FROM enrichment_progress WHERE service = 'all' AND status = 'completed') as completed,
                (SELECT COUNT(*) FROM enrichment_progress WHERE service = 'all' AND status = 'failed') as failed
            "#
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| e.to_string())?;

        Ok(EnrichmentStatus {
            is_paused: self.state.is_paused(),
            active_jobs: self.state.active_jobs(),
            pending_count: stats.0,
            completed_count: stats.1,
            failed_count: stats.2,
        })
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn db() -> SqlitePool {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT)").execute(&db).await.unwrap();
        sqlx::query("CREATE TABLE enrichment_progress (track_id INTEGER NOT NULL, service TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('pending', 'in_progress', 'completed', 'failed', 'rate_limited')), last_error TEXT, completed_at TEXT)").execute(&db).await.unwrap();
        sqlx::query("INSERT INTO tracks(id, enrichment_status) VALUES (1, 'pending')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO enrichment_progress(track_id, service, status) VALUES (1, 'all', 'in_progress'), (1, 'spotify', 'completed')").execute(&db).await.unwrap();
        db
    }

    #[tokio::test]
    async fn completion_is_atomic_when_progress_write_fails() {
        let db = db().await;
        sqlx::query("CREATE TRIGGER fail_progress BEFORE UPDATE ON enrichment_progress WHEN NEW.status = 'completed' AND NEW.service = 'all' BEGIN SELECT RAISE(ABORT, 'injected write failure'); END").execute(&db).await.unwrap();
        assert!(finish_enrichment(&db, 1, Some("mb-test")).await.is_err());
        let row: (Option<String>, String, String) = sqlx::query_as("SELECT t.musicbrainz_id, t.enrichment_status, ep.status FROM tracks t JOIN enrichment_progress ep ON ep.track_id=t.id AND ep.service='all' WHERE t.id=1").fetch_one(&db).await.unwrap();
        assert_eq!(row, (None, "pending".into(), "in_progress".into()));
    }

    #[tokio::test]
    async fn missing_track_or_progress_cannot_complete() {
        let db = db().await;
        assert!(finish_enrichment(&db, 42, Some("mb-test")).await.is_err());
        sqlx::query("DELETE FROM enrichment_progress WHERE service='all'")
            .execute(&db)
            .await
            .unwrap();
        assert!(finish_enrichment(&db, 1, Some("mb-test")).await.is_err());
        let row: (Option<String>, String) =
            sqlx::query_as("SELECT musicbrainz_id, enrichment_status FROM tracks WHERE id=1")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(row, (None, "pending".into()));
        assert!(skip_enrichment(&db, 1).await.is_err());
    }

    #[tokio::test]
    async fn skipped_event_requires_persisted_terminal_state() {
        let db = db().await;
        sqlx::query("CREATE TRIGGER fail_skip BEFORE UPDATE ON enrichment_progress WHEN NEW.service='all' BEGIN SELECT RAISE(ABORT, 'injected skip failure'); END").execute(&db).await.unwrap();
        assert!(skip_enrichment(&db, 1).await.is_err());
        let status: String =
            sqlx::query_scalar("SELECT status FROM enrichment_progress WHERE service='all'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "in_progress");
        sqlx::query("DROP TRIGGER fail_skip")
            .execute(&db)
            .await
            .unwrap();
        skip_enrichment(&db, 1).await.unwrap();
        let status: String =
            sqlx::query_scalar("SELECT status FROM enrichment_progress WHERE service='all'")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(status, "completed");
    }

    #[tokio::test]
    async fn status_counts_only_all_and_retryable_jobs() {
        let db = db().await;
        sqlx::query("INSERT INTO tracks(id, enrichment_status) VALUES (2, 'pending'), (3, 'pending'), (4, 'pending')")
            .execute(&db).await.unwrap();
        sqlx::query(
            "ALTER TABLE enrichment_progress ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO enrichment_progress(track_id, service, status, retry_count) VALUES (2, 'all', 'failed', 2), (3, 'all', 'failed', 3)")
            .execute(&db).await.unwrap();
        let worker = EnrichmentWorker::new(
            db.clone(),
            EnrichmentWorkerState::new(),
            Arc::new(RateLimiter::new()),
        );
        let status = worker.get_status().await.unwrap();
        assert_eq!(
            (
                status.pending_count,
                status.completed_count,
                status.failed_count,
                status.active_jobs
            ),
            (2, 0, 2, 0)
        );
        finish_enrichment(&db, 1, None).await.unwrap();
        let status = worker.get_status().await.unwrap();
        assert_eq!(
            (
                status.pending_count,
                status.completed_count,
                status.failed_count
            ),
            (2, 1, 2)
        );
    }

    #[tokio::test]
    async fn restart_requeues_only_all_in_progress() {
        let db = db().await;
        recover_interrupted_jobs(&db).await.unwrap();
        let statuses: Vec<(String, String)> =
            sqlx::query_as("SELECT service,status FROM enrichment_progress ORDER BY service")
                .fetch_all(&db)
                .await
                .unwrap();
        assert_eq!(
            statuses,
            vec![
                ("all".into(), "pending".into()),
                ("spotify".into(), "completed".into())
            ]
        );
    }
}
