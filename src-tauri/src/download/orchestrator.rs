// Download orchestrator - coordinates multiple download services

use crate::download::amazon::AmazonDownloader;
use crate::download::lyrics::{LyricsClient, LyricsResponse};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use crate::download::qobuz::QobuzDownloader;
use crate::download::songlink::SongLinkClient;
use crate::download::tidal::{TidalDownloader, TidalOrchestratorExt};

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Download orchestrator that manages multiple services
#[allow(dead_code)]
pub struct DownloadOrchestrator {
    qobuz: Arc<QobuzDownloader>,
    tidal: Arc<TidalDownloader>,
    amazon: Arc<AmazonDownloader>,
    songlink: Arc<SongLinkClient>,
    lyrics: Arc<LyricsClient>,
    /// Service priority order
    service_priority: Vec<String>,
    /// Database pool for active account token resolution
    db: Option<sqlx::SqlitePool>,
}

impl DownloadOrchestrator {
    pub fn new() -> Self {
        Self {
            qobuz: Arc::new(QobuzDownloader::new()),
            tidal: Arc::new(TidalDownloader::new()),
            amazon: Arc::new(AmazonDownloader::new()),
            songlink: Arc::new(SongLinkClient::new()),
            lyrics: Arc::new(LyricsClient::new()),
            service_priority: vec![
                "qobuz".to_string(),
                "tidal".to_string(),
                "amazon".to_string(),
            ],
            db: None,
        }
    }

    pub fn with_db(mut self, db: sqlx::SqlitePool) -> Self {
        self.db = Some(db);
        self
    }

    /// Set custom service priority
    #[allow(dead_code)]
    pub fn with_priority(mut self, priority: Vec<String>) -> Self {
        self.service_priority = priority;
        self
    }

    /// Download a track, trying services in priority order
    pub async fn download_track(&self, request: &DownloadRequest) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        PROGRESS_TRACKER.init(item_id);

        // Get cross-platform availability if we have a Spotify ID
        let availability = if let Some(spotify_id) = &request.spotify_id {
            match self
                .songlink
                .check_availability(spotify_id, request.isrc.as_deref())
                .await
            {
                Ok(a) => Some(a),
                Err(e) => {
                    debug!("[Orchestrator] SongLink check failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Try each service in priority order
        let mut last_error: Option<String> = None;

        for service in &self.service_priority {
            debug!("[Orchestrator] Trying service: {}", service);

            let result = match service.as_str() {
                "qobuz" => {
                    self.qobuz.download_track(request, self.db.as_ref()).await
                }
                "tidal" => {
                    if self.db.is_none() && self.tidal.user_token().is_none() {
                        Err(anyhow!("RequiresAuth: DownloadOrchestrator requires SqlitePool or user_token to download via Tidal"))
                    } else {
                        self.tidal.download_track(request, self.db.as_ref()).await
                    }
                }
                "amazon" => {
                    // Amazon requires URL from SongLink
                    if let Some(ref avail) = availability {
                        if let Some(ref amazon_url) = avail.amazon_url {
                            self.amazon.download_track(request, amazon_url).await
                        } else {
                            debug!("[Orchestrator] No Amazon URL available, skipping");
                            continue;
                        }
                    } else {
                        debug!("[Orchestrator] No SongLink data for Amazon, skipping");
                        continue;
                    }
                }
                _ => {
                    warn!("[Orchestrator] Unknown service: {}", service);
                    continue;
                }
            };

            match result {
                Ok(download_result) => {
                    info!(
                        "[Orchestrator] Download complete via {}: {}",
                        service, download_result.file_path
                    );
                    PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                    return Ok(download_result);
                }
                Err(e) => {
                    warn!("[Orchestrator] Service '{}' failed: {}. Continuing fallback cascade...", service, e);
                    if let Some(ref prev) = last_error {
                        last_error = Some(format!("{}, {}: {}", prev, service, e));
                    } else {
                        last_error = Some(format!("{}: {}", service, e));
                    }
                }
            }
        }

        // All services failed
        let error_msg = last_error.unwrap_or_else(|| "No services available".to_string());
        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &error_msg));
        Err(anyhow!("All download services failed: {}", error_msg))
    }

    // Note: Race mode removed for simplicity. Using sequential fallback instead.

    /// Fetch lyrics for a track
    #[allow(dead_code)]
    pub async fn fetch_lyrics(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        self.lyrics
            .fetch_all_sources(artist, track, duration_sec)
            .await
    }

    /// Check track availability across platforms
    #[allow(dead_code)]
    pub async fn check_availability(
        &self,
        spotify_id: &str,
    ) -> Result<crate::download::songlink::TrackAvailability> {
        self.songlink.check_availability(spotify_id, None).await
    }
}

impl Default for DownloadOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

// Global orchestrator instance
lazy_static::lazy_static! {
    pub static ref ORCHESTRATOR: DownloadOrchestrator = DownloadOrchestrator::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = DownloadOrchestrator::new();
        assert_eq!(orchestrator.service_priority.len(), 3);
    }

    #[test]
    fn test_custom_priority() {
        let orchestrator = DownloadOrchestrator::new()
            .with_priority(vec!["tidal".to_string(), "qobuz".to_string()]);
        assert_eq!(orchestrator.service_priority[0], "tidal");
    }

    #[tokio::test]
    async fn test_orchestrator_fails_if_tidal_created_without_user_token_or_sqlite_pool() {
        let orchestrator = DownloadOrchestrator::new()
            .with_priority(vec!["tidal".to_string()]);
        // Neither db pool nor user_token provided
        let req = DownloadRequest {
            item_id: "test_item_1".to_string(),
            isrc: None,
            spotify_id: None,
            track_name: "Heroes".to_string(),
            artist_name: "David Bowie".to_string(),
            album_name: "Heroes".to_string(),
            album_artist: None,
            duration_ms: 360000,
            track_number: 1,
            disc_number: 1,
            total_tracks: 10,
            release_date: Some("1977-10-14".to_string()),
            cover_url: None,
            output_dir: "./downloads".to_string(),
            quality: "LOSSLESS".to_string(),
            embed_lyrics: false,
            embed_artwork: false,
        };

        let result = orchestrator.download_track(&req).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("RequiresAuth") || err_msg.contains("DownloadOrchestrator requires SqlitePool or user_token"));
    }
}
