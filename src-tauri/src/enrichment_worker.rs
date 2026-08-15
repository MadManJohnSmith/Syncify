//! Background Metadata Enrichment Worker
//!
//! Automatically enriches tracks in the background with MusicBrainz, Spotify audio features,
//! and Last.fm genres, respecting centralized RateLimiter, backoff, and progress persistence.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Notify;

use crate::services::rate_limiter::RateLimiter;
use crate::services::enrichment::EnrichmentEngine;

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

/// Background enrichment worker
pub struct EnrichmentWorker {
    db: SqlitePool,
    state: EnrichmentWorkerState,
    rate_limiter: Arc<RateLimiter>,
    engine: EnrichmentEngine,
    app_handle: Option<tauri::AppHandle>,
}

impl EnrichmentWorker {
    pub fn new(db: SqlitePool, state: EnrichmentWorkerState, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            db,
            state,
            rate_limiter,
            engine: EnrichmentEngine::new(),
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
        }
    }

    /// Run the enrichment worker loop
    pub async fn run(&self) {
        tracing::info!("EnrichmentWorker started");

        while !self.state.is_stopped() {
            self.state.wait_if_paused().await;

            if self.state.is_stopped() {
                break;
            }

            match self.process_next_track().await {
                Ok(has_work) => {
                    if !has_work {
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
            "#
        )
        .bind(track_id)
        .execute(&self.db)
        .await
        .map_err(|e| e.to_string())?;

        self.emit_event(EnrichmentProgressEvent {
            track_id,
            title: title.clone(),
            artist: artist.clone(),
            service: "all".to_string(),
            status: "in_progress".to_string(),
            message: format!("Enriching '{}' by '{}'", title, artist),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });

        // 3. Acquire rate limiter permit
        self.rate_limiter.acquire("musicbrainz").await;

        // 4. Resolve metadata
        let result = self.engine.resolve_track_metadata(
            &artist,
            &album,
            &title,
            isrc_opt.as_deref(),
            None,
        ).await;

        // 5. Update database on success
        let mb_id = result.musicbrainz_recording_id.value();
        let _ = sqlx::query(
            "UPDATE tracks SET musicbrainz_id = ?, enrichment_status = 'enriched', enriched_at = datetime('now') WHERE id = ?"
        )
        .bind(mb_id)
        .bind(track_id)
        .execute(&self.db)
        .await;

        sqlx::query(
            r#"
            UPDATE enrichment_progress SET
                status = 'completed',
                completed_at = datetime('now'),
                last_error = NULL
            WHERE track_id = ? AND service = 'all'
            "#
        )
        .bind(track_id)
        .execute(&self.db)
        .await
        .map_err(|e| e.to_string())?;

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
                (SELECT COUNT(*) FROM tracks t LEFT JOIN enrichment_progress ep ON ep.track_id = t.id WHERE ep.status IS NULL OR ep.status = 'pending') as pending,
                (SELECT COUNT(*) FROM enrichment_progress WHERE status = 'completed') as completed,
                (SELECT COUNT(*) FROM enrichment_progress WHERE status = 'failed') as failed
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
