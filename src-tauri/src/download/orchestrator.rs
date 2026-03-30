// Download orchestrator - coordinates multiple download services

use crate::download::amazon::AmazonDownloader;
use crate::download::lyrics::{LyricsClient, LyricsResponse};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use crate::download::qobuz::QobuzDownloader;
use crate::download::songlink::SongLinkClient;
use crate::download::tidal::TidalDownloader;
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
        }
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
                    // Check availability if we have it
                    if let Some(ref avail) = availability {
                        if !avail.qobuz {
                            debug!("[Orchestrator] Qobuz not available per SongLink, skipping");
                            continue;
                        }
                    }
                    self.qobuz.download_track(request).await
                }
                "tidal" => {
                    if let Some(ref avail) = availability {
                        if !avail.tidal {
                            debug!("[Orchestrator] Tidal not available per SongLink, skipping");
                            continue;
                        }
                    }
                    self.tidal.download_track(request).await
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
                    warn!("[Orchestrator] {} failed: {}", service, e);
                    last_error = Some(format!("{}: {}", service, e));
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
}

