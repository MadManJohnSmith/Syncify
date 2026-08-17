// Download orchestrator - coordinates multiple download services with resilience and cooperative cancellation

use crate::download::amazon::AmazonDownloader;
use crate::download::lyrics::{LyricsClient, LyricsResponse};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use crate::download::qobuz::QobuzDownloader;
use crate::download::songlink::SongLinkClient;
use crate::download::tidal::{TidalDownloader, TidalOrchestratorExt};

use crate::services::enrichment::{
    AudioAnalysisMetrics, AudioAnalyzer, EnrichedMetadata, EnrichmentEngine, OriginTrackMetadata,
};

use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Download orchestrator that manages multiple services
#[allow(dead_code)]
pub struct DownloadOrchestrator {
    qobuz: Arc<QobuzDownloader>,
    tidal: Arc<TidalDownloader>,
    amazon: Arc<AmazonDownloader>,
    songlink: Arc<SongLinkClient>,
    lyrics: Arc<LyricsClient>,
    enrichment: Arc<EnrichmentEngine>,
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
            enrichment: Arc::new(EnrichmentEngine::new()),
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

    /// Analyze an audio file (e.g. in staging) extracting ReplayGain, Acoustic Features, and Fingerprinting.
    #[allow(dead_code)]
    pub async fn analyze_staging_audio(&self, file_path: &std::path::Path) -> Result<AudioAnalysisMetrics, String> {
        AudioAnalyzer::analyze_file(file_path).await
    }

    /// Enrich staging audio file: queries MusicBrainz, computes missing ReplayGain, Acoustic Features, and AcoustID.
    #[allow(dead_code)]
    pub async fn enrich_staging_audio(
        &self,
        file_path: &std::path::Path,
        request: &DownloadRequest,
        origin_meta: Option<&OriginTrackMetadata>,
    ) -> Result<EnrichedMetadata> {
        let enriched = self
            .enrichment
            .resolve_and_enrich_staging_audio(
                file_path,
                &request.artist_name,
                &request.album_name,
                &request.track_name,
                request.isrc.as_deref(),
                origin_meta,
            )
            .await;
        Ok(enriched)
    }

    /// Download a track, trying services in priority order (backwards-compatible)
    pub async fn download_track(&self, request: &DownloadRequest) -> Result<DownloadResult> {
        self.download_track_cancellable(request, None).await
    }

    /// Download a track with cooperative cancellation support
    pub async fn download_track_cancellable(
        &self,
        request: &DownloadRequest,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        PROGRESS_TRACKER.init(item_id);

        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, "Download cancelled"));
                return Err(anyhow!("Download cancelled by user"));
            }
        }

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

        // Determine effective service list based on explicit source identity & fallback policy
        let effective_services: Vec<String> = if let Some(ref target_svc) = request.service_name {
            let normalized = target_svc.to_lowercase().trim().to_string();
            if !request.allow_fallback {
                vec![normalized]
            } else {
                let mut svcs = vec![normalized.clone()];
                for s in &self.service_priority {
                    if *s != normalized && !svcs.contains(s) {
                        svcs.push(s.clone());
                    }
                }
                svcs
            }
        } else {
            self.service_priority.clone()
        };

        // Try each service in effective priority order
        let mut last_error: Option<String> = None;

        for service in &effective_services {
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, "Download cancelled"));
                    return Err(anyhow!("Download cancelled by user"));
                }
            }

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
            service_name: None,
            service_track_id: None,
            service_album_id: None,
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
            smart_studio_origin: false,
            allow_fallback: true,
        };

        let result = orchestrator.download_track(&req).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("RequiresAuth") || err_msg.contains("DownloadOrchestrator requires SqlitePool or user_token"));
    }

    #[tokio::test]
    async fn test_orchestrator_cancellation_token() {
        let orchestrator = DownloadOrchestrator::new()
            .with_priority(vec!["qobuz".to_string()]);
        let cancel_token = CancellationToken::new();
        cancel_token.cancel(); // Pre-cancel

        let req = DownloadRequest {
            item_id: "test_item_cancel".to_string(),
            isrc: None,
            spotify_id: None,
            service_name: Some("qobuz".to_string()),
            service_track_id: Some("123".to_string()),
            service_album_id: None,
            track_name: "Test Track".to_string(),
            artist_name: "Test Artist".to_string(),
            album_name: "Test Album".to_string(),
            album_artist: None,
            duration_ms: 200000,
            track_number: 1,
            disc_number: 1,
            total_tracks: 1,
            release_date: None,
            cover_url: None,
            output_dir: "./downloads".to_string(),
            quality: "LOSSLESS".to_string(),
            embed_lyrics: false,
            embed_artwork: false,
            smart_studio_origin: false,
            allow_fallback: false,
        };

        let result = orchestrator.download_track_cancellable(&req, Some(&cancel_token)).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("cancelled"));
    }

    #[tokio::test]
    async fn test_orchestrator_audio_analysis_and_enrichment_flow() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let flac_path = temp_dir.path().join("orchestrator_audio_test.flac");

        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        data.push(0x80);
        data.push(0x00);
        data.push(0x00);
        data.push(0x22);
        data.extend_from_slice(&[0u8; 34]);
        data.extend_from_slice(&[0x42; 4096]);
        std::fs::write(&flac_path, &data).unwrap();

        let orchestrator = DownloadOrchestrator::new();

        // 1. Test direct staging analysis
        let analysis = orchestrator.analyze_staging_audio(&flac_path).await.unwrap();
        assert!(analysis.replaygain_track_gain.is_some());
        assert!(analysis.bpm.is_some());
        assert!(analysis.acoustid_id.is_some());

        // 2. Test orchestrator enrichment pipeline
        let req = DownloadRequest {
            item_id: "orch_item_1".to_string(),
            isrc: Some("GBAYE7700021".to_string()),
            spotify_id: None,
            service_name: Some("qobuz".to_string()),
            service_track_id: Some("123".to_string()),
            service_album_id: None,
            track_name: "Heroes".to_string(),
            artist_name: "David Bowie".to_string(),
            album_name: "Heroes".to_string(),
            album_artist: None,
            duration_ms: 360000,
            track_number: 3,
            disc_number: 1,
            total_tracks: 10,
            release_date: Some("1977-10-14".to_string()),
            cover_url: None,
            output_dir: temp_dir.path().to_str().unwrap().to_string(),
            quality: "LOSSLESS".to_string(),
            embed_lyrics: false,
            embed_artwork: false,
            smart_studio_origin: false,
            allow_fallback: true,
        };

        let enriched = orchestrator.enrich_staging_audio(&flac_path, &req, None).await.unwrap();
        assert_eq!(enriched.title.value(), Some("Heroes"));
        assert_eq!(enriched.artist.value(), Some("David Bowie"));
        assert!(enriched.replaygain_track_gain.value().is_some());
        assert!(enriched.bpm.value().is_some());
        assert!(enriched.acoustid_id.value().is_some());
    }
}
