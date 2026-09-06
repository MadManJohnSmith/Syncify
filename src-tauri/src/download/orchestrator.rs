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

/// Matched candidate for controlled edition-identity fallback
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FallbackMatch {
    pub target_track_id: i64,
    pub target_service: String,
    pub match_method: String,
    pub match_confidence: f64,
    pub candidate_audio_quality: Option<String>,
}

/// Target engine candidate resolved from SongLink cross-platform lookup
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SongLinkEngineTarget {
    Tidal(String),
    Qobuz(String),
    Amazon(String),
}

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

    #[allow(dead_code)]
    pub fn with_songlink(mut self, songlink: Arc<SongLinkClient>) -> Self {
        self.songlink = songlink;
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

    /// Reconciles the downloaded file's physical audio metrics from disk (sample_rate, bit_depth, channels, bitrate)
    /// and performs canonical quality policy evaluation against the physical reality.
    pub fn reconcile_physical_audio_quality(
        res: &mut DownloadResult,
        request: &DownloadRequest,
    ) {
        let path = std::path::Path::new(&res.file_path);
        let phys_info = crate::download::audio_inspector::inspect_physical_audio_file(path);

        let (verified_bd, verified_sr, verified_fmt, verified_channels, verified_bitrate, quality_label) =
            if let Some(ref phys) = phys_info {
                (
                    phys.bit_depth,
                    phys.sample_rate,
                    phys.format.clone(),
                    Some(phys.channels),
                    phys.bitrate,
                    phys.quality_string(),
                )
            } else {
                let is_m4a = res.file_path.to_lowercase().ends_with(".m4a");
                let is_mp3 = res.file_path.to_lowercase().ends_with(".mp3");
                let fmt = if is_m4a { "AAC" } else if is_mp3 { "MP3" } else { "FLAC" };
                let label = if fmt == "FLAC" {
                    format!("FLAC {}-bit / {:.1}kHz", res.bit_depth, res.sample_rate as f64 / 1000.0)
                } else {
                    format!("{} 320kbps", fmt)
                };
                (res.bit_depth, res.sample_rate, fmt.to_string(), res.channels.or(Some(2)), res.bitrate, label)
            };

        res.bit_depth = verified_bd;
        res.sample_rate = verified_sr;
        if res.channels.is_none() {
            res.channels = verified_channels;
        }
        if res.bitrate.is_none() {
            res.bitrate = verified_bitrate;
        }

        let q_eval = syncify_core_domain::quality::QualityPolicy::evaluate_stream_resolution(
            &request.quality,
            &quality_label,
            &verified_fmt,
            verified_bd,
            verified_sr as f64,
            res.origin_service.as_deref().unwrap_or(&res.service),
            &res.service,
            request.strict_quality,
            request.allow_fallback,
        );
        res.quality_decision = Some(q_eval);
    }

    /// Download a track with cooperative cancellation support
    /// Resolve an equivalent edition on Tidal for a stale source following strict equivalence hierarchy
    pub async fn resolve_edition_identity_fallback(
        &self,
        request: &DownloadRequest,
    ) -> Result<FallbackMatch, String> {
        let duration_sec = (request.duration_ms / 1000) as i32;

        // 1. Exact ISRC matching
        if let Some(ref isrc) = request.isrc {
            let isrc_trimmed = isrc.trim();
            if !isrc_trimmed.is_empty() {
                debug!("[Orchestrator] Fallback Step 1: Searching Tidal by exact ISRC '{}'", isrc_trimmed);

                // 1A. Check local database if available
                if let Some(ref db) = self.db {
                    let isrc_candidates: Vec<(String, Option<String>)> = sqlx::query_as(
                        r#"
                        SELECT ts.service_track_id, ts.format
                        FROM track_sources ts
                        JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
                        JOIN tracks t ON t.id = ts.track_id
                        WHERE t.isrc = ? AND ts.available = 1
                        "#
                    )
                    .bind(isrc_trimmed)
                    .fetch_all(db)
                    .await
                    .unwrap_or_default();

                    if isrc_candidates.len() == 1 {
                        let (stid, fmt) = &isrc_candidates[0];
                        if let Ok(tid) = stid.parse::<i64>() {
                            info!("[Orchestrator] ✓ Fallback matched via DB exact ISRC: Tidal ID {}", tid);
                            return Ok(FallbackMatch {
                                target_track_id: tid,
                                target_service: "tidal".to_string(),
                                match_method: "exact_isrc".to_string(),
                                match_confidence: 1.0,
                                candidate_audio_quality: fmt.clone(),
                            });
                        }
                    } else if isrc_candidates.len() > 1 {
                        return Err(format!("AmbiguousSource: Multiple competing Tidal tracks found for ISRC {}", isrc_trimmed));
                    }
                }

                // 1B. Query Tidal API
                match self.tidal.search_by_isrc(isrc_trimmed, duration_sec).await {
                    Ok(track) => {
                        info!("[Orchestrator] ✓ Fallback matched via exact ISRC: Tidal ID {}", track.id);
                        return Ok(FallbackMatch {
                            target_track_id: track.id,
                            target_service: "tidal".to_string(),
                            match_method: "exact_isrc".to_string(),
                            match_confidence: 1.0,
                            candidate_audio_quality: track.audio_quality,
                        });
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        if err_msg.contains("AmbiguousSource") || err_msg.contains("Multiple competing") {
                            return Err(format!("AmbiguousSource: Multiple competing Tidal tracks found for ISRC {}", isrc_trimmed));
                        }
                    }
                }
            }
        }

        // 2. Exact MusicBrainz Recording ID
        if let Some(ref mb_rid) = request.musicbrainz_recording_id {
            let mb_rid_trimmed = mb_rid.trim();
            if !mb_rid_trimmed.is_empty() {
                debug!("[Orchestrator] Fallback Step 2: Searching Tidal by MusicBrainz Recording ID '{}'", mb_rid_trimmed);
                if let Some(ref db) = self.db {
                    let mb_candidates: Vec<(String, Option<String>)> = sqlx::query_as(
                        r#"
                        SELECT ts.service_track_id, ts.format
                        FROM track_sources ts
                        JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
                        JOIN tracks t ON t.id = ts.track_id
                        WHERE t.musicbrainz_id = ? AND ts.available = 1
                        "#
                    )
                    .bind(mb_rid_trimmed)
                    .fetch_all(db)
                    .await
                    .unwrap_or_default();

                    if mb_candidates.len() == 1 {
                        let (stid, fmt) = &mb_candidates[0];
                        if let Ok(tid) = stid.parse::<i64>() {
                            info!("[Orchestrator] ✓ Fallback matched via MusicBrainz Recording ID: Tidal ID {}", tid);
                            return Ok(FallbackMatch {
                                target_track_id: tid,
                                target_service: "tidal".to_string(),
                                match_method: "musicbrainz_recording_id".to_string(),
                                match_confidence: 0.95,
                                candidate_audio_quality: fmt.clone(),
                            });
                        }
                    } else if mb_candidates.len() > 1 {
                        return Err(format!("AmbiguousSource: Multiple competing Tidal tracks for MusicBrainz Recording ID {}", mb_rid_trimmed));
                    }
                }
            }
        }

        // 3. MusicBrainz release/track matching with tolerant duration (±3s)
        if let Some(ref db) = self.db {
            debug!("[Orchestrator] Fallback Step 3: Searching Tidal by MusicBrainz release/track tolerance for '{}'", request.track_name);
            let mb_rel_candidates: Vec<(String, i64, Option<String>)> = sqlx::query_as(
                r#"
                SELECT ts.service_track_id, t.duration_ms, ts.format
                FROM track_sources ts
                JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
                JOIN tracks t ON t.id = ts.track_id
                WHERE t.musicbrainz_id IS NOT NULL AND ts.available = 1
                  AND LOWER(t.title) = LOWER(?)
                "#
            )
            .bind(&request.track_name)
            .fetch_all(db)
            .await
            .unwrap_or_default();

            let duration_matches: Vec<_> = mb_rel_candidates
                .into_iter()
                .filter(|(stid, dur, _)| {
                    !stid.trim().is_empty() && (dur / 1000 - (request.duration_ms / 1000)).abs() <= 3
                })
                .collect();

            if duration_matches.len() == 1 {
                let (stid, _, fmt) = &duration_matches[0];
                if let Ok(tid) = stid.parse::<i64>() {
                    info!("[Orchestrator] ✓ Fallback matched via MusicBrainz release/track: Tidal ID {}", tid);
                    return Ok(FallbackMatch {
                        target_track_id: tid,
                        target_service: "tidal".to_string(),
                        match_method: "musicbrainz_release_track".to_string(),
                        match_confidence: 0.85,
                        candidate_audio_quality: fmt.clone(),
                    });
                }
            } else if duration_matches.len() > 1 {
                return Err("AmbiguousSource: Multiple competing MusicBrainz release/track matches on Tidal".to_string());
            }
        }

        // 4. Exact AcoustID / Fingerprint matching
        if let Some(ref fp) = request.acoustid_fingerprint {
            let fp_trimmed = fp.trim();
            // Audit 2026-08-25: fingerprints with the legacy synthetic "AQAA-" prefix
            // were minted by the removed enrichment fallback (md5 of name+size, fixed
            // 180 s duration) and carry no acoustic identity. Matching on them would
            // compare fabrication against fabrication, so identity is treated as
            // "no comparison possible" and we fall through to the next rule instead.
            let is_legacy_synthetic = fp_trimmed.starts_with("AQAA-");
            if !fp_trimmed.is_empty() && !is_legacy_synthetic {
                debug!("[Orchestrator] Fallback Step 4: Searching Tidal by AcoustID fingerprint");
                if let Some(ref db) = self.db {
                    let fp_candidates: Vec<(String, Option<String>)> = sqlx::query_as(
                        r#"
                        SELECT ts.service_track_id, ts.format
                        FROM track_sources ts
                        JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
                        JOIN tracks t ON t.id = ts.track_id
                        WHERE t.acoustid_fingerprint = ? AND ts.available = 1
                        "#
                    )
                    .bind(fp_trimmed)
                    .fetch_all(db)
                    .await
                    .unwrap_or_default();

                    if fp_candidates.len() == 1 {
                        let (stid, fmt) = &fp_candidates[0];
                        if let Ok(tid) = stid.parse::<i64>() {
                            info!("[Orchestrator] ✓ Fallback matched via AcoustID fingerprint: Tidal ID {}", tid);
                            return Ok(FallbackMatch {
                                target_track_id: tid,
                                target_service: "tidal".to_string(),
                                match_method: "acoustid_fingerprint".to_string(),
                                match_confidence: 0.80,
                                candidate_audio_quality: fmt.clone(),
                            });
                        }
                    } else if fp_candidates.len() > 1 {
                        return Err("AmbiguousSource: Multiple competing AcoustID fingerprint matches on Tidal".to_string());
                    }
                }
            }
        }

        // 5. Title + Artist loose metadata matching check (Rule 5: NEVER automatic download)
        let has_local_metadata_match = if let Some(ref db) = self.db {
            let count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM track_sources ts
                JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
                JOIN tracks t ON t.id = ts.track_id
                WHERE LOWER(t.title) = LOWER(?) AND ts.available = 1
                "#
            )
            .bind(&request.track_name)
            .fetch_one(db)
            .await
            .unwrap_or(0);
            count > 0
        } else {
            false
        };

        let has_metadata_match = if has_local_metadata_match {
            true
        } else {
            match self.tidal.search_by_metadata(&request.track_name, &request.artist_name, duration_sec).await {
                Ok(_) => true,
                Err(_) => false,
            }
        };

        if has_metadata_match {
            return Err("AmbiguousSource: Fallback to Tidal produced only loose metadata (title/artist) match without edition identity (ISRC/MBID/AcoustID). Automatic download forbidden.".to_string());
        }

        // 6. No match found
        Err(format!("SourceIdentityMissing: No equivalent Tidal source found for track '{}'", request.track_name))
    }

    /// Download a track with cooperative cancellation and controlled fallback support
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

        let primary_service = request.service_name.as_deref().unwrap_or("qobuz").to_lowercase();

        if primary_service == "qobuz" {
            // Attempt direct download via locked Qobuz source
            debug!("[Orchestrator] Attempting primary download via Qobuz (service_track_id={:?})", request.service_track_id);
            let qobuz_result = self.qobuz.download_track(request, self.db.as_ref()).await;

            match qobuz_result {
                Ok(mut res) => {
                    res.origin_service = request.service_name.clone().or(Some("qobuz".to_string()));
                    res.origin_service_track_id = request.service_track_id.clone();
                    res.effective_service = Some("qobuz".to_string());
                    res.effective_service_track_id = request.service_track_id.clone();
                    res.fallback_reason = None;
                    res.match_method = Some("exact_locked_source".to_string());
                    res.match_confidence = Some(1.0);
                    Self::reconcile_physical_audio_quality(&mut res, request);
                    info!("[Orchestrator] Download complete via exact Qobuz source: {}", res.file_path);
                    PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                    return Ok(res);
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    warn!("[Orchestrator] Qobuz download failed: {}", err_msg);

                    // 1. Auth failure (401/403) -> abort without fallback
                    if err_msg.contains("401")
                        || err_msg.contains("403")
                        || err_msg.contains("RequiresAuth")
                        || err_msg.contains("authentication failed")
                        || (err_msg.contains("token") && (err_msg.contains("expired") || err_msg.contains("invalid")))
                    {
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                        return Err(anyhow!("RequiresAuth: Qobuz authentication required (HTTP 401/403). Automatic fallback aborted."));
                    }

                    // 2. Rejected quality on Qobuz -> abort without fallback unless permitted
                    if err_msg.contains("RejectedQuality") {
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                        return Err(anyhow!("RejectedQuality: Requested quality not available on Qobuz"));
                    }

                    // 3. Network Exhausted (stream / connection failures that exhausted retries) -> abort without fallback
                    if err_msg.contains("NetworkExhausted") {
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                        return Err(anyhow!("NetworkExhausted: Qobuz network stream exhausted retries: {}", err_msg));
                    }

                    // 3. Stale source (404 / NotFound / Unavailable) -> trigger controlled fallback if allowed
                    let is_stale = err_msg.contains("404")
                        || err_msg.contains("NotFound")
                        || err_msg.contains("StaleSource")
                        || err_msg.contains("not found")
                        || err_msg.contains("track/get failed")
                        || err_msg.contains("unavailable")
                        || err_msg.contains("CountryNotAvailable");

                    if is_stale {
                        if !request.allow_fallback {
                            PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, "StaleSource: Qobuz track not found (HTTP 404) and allow_fallback=false"));
                            return Err(anyhow!("StaleSource: Qobuz track not found (HTTP 404) and allow_fallback=false"));
                        }

                        info!("[Orchestrator] Qobuz source is stale (404/NotFound). Attempting controlled edition-identity fallback to Tidal...");
                        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "tidal"));

                        let fallback_match = self.resolve_edition_identity_fallback(request).await
                            .map_err(|e| {
                                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &e));
                                anyhow!("{}", e)
                            })?;

                        // Quality check against fallback candidate
                        let is_strict = request.strict_quality || !request.allow_fallback;
                        if is_strict {
                            let req_q = request.quality.to_uppercase();
                            if let Some(ref cq) = fallback_match.candidate_audio_quality {
                                let cq_up = cq.to_uppercase();
                                if (req_q.contains("HI_RES") || req_q.contains("HIRES") || req_q.contains("24") || req_q == "LOSSLESS")
                                    && (cq_up.contains("LOW") || cq_up.contains("HIGH") || cq_up.contains("MP3") || cq_up.contains("AAC"))
                                    && !cq_up.contains("HI_RES") && !cq_up.contains("24")
                                {
                                    let rej_err = format!("RejectedQuality: Tidal fallback candidate quality '{}' is inferior to requested '{}' under strict policy", cq, request.quality);
                                    PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &rej_err));
                                    return Err(anyhow!("{}", rej_err));
                                }
                            }
                        }

                        // Prepare and execute Tidal download request
                        let mut tidal_req = request.clone();
                        tidal_req.service_name = Some("tidal".to_string());
                        tidal_req.service_track_id = Some(fallback_match.target_track_id.to_string());

                        let mut tidal_res = self.tidal.download_track(&tidal_req, self.db.as_ref()).await
                            .map_err(|e| {
                                let msg = format!("Tidal fallback download failed: {}", e);
                                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &msg));
                                anyhow!("{}", msg)
                            })?;

                        tidal_res.origin_service = request.service_name.clone().or(Some("qobuz".to_string()));
                        tidal_res.origin_service_track_id = request.service_track_id.clone();
                        tidal_res.effective_service = Some("tidal".to_string());
                        tidal_res.effective_service_track_id = Some(fallback_match.target_track_id.to_string());
                        tidal_res.fallback_reason = Some("StaleSource: Qobuz track not found (HTTP 404)".to_string());
                        tidal_res.match_method = Some(fallback_match.match_method);
                        tidal_res.match_confidence = Some(fallback_match.match_confidence);
                        Self::reconcile_physical_audio_quality(&mut tidal_res, request);

                        info!(
                            "[Orchestrator] Fallback download complete via Tidal: {} (origin: Qobuz ID {:?}, effective: Tidal ID {})",
                            tidal_res.file_path,
                            request.service_track_id,
                            fallback_match.target_track_id
                        );
                        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                        return Ok(tidal_res);
                    } else {
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                        return Err(anyhow!("Qobuz download failed: {}", err_msg));
                    }
                }
            }
        } else if primary_service == "tidal" {
            let mut tidal_res = self.tidal.download_track(request, self.db.as_ref()).await?;
            tidal_res.origin_service = request.service_name.clone().or(Some("tidal".to_string()));
            tidal_res.origin_service_track_id = request.service_track_id.clone();
            tidal_res.effective_service = Some("tidal".to_string());
            tidal_res.effective_service_track_id = request.service_track_id.clone();
            tidal_res.fallback_reason = None;
            tidal_res.match_method = Some("exact_locked_source".to_string());
            tidal_res.match_confidence = Some(1.0);
            Self::reconcile_physical_audio_quality(&mut tidal_res, request);
            PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
            return Ok(tidal_res);
        } else {
            // Other services (e.g. Spotify, Apple Music, Deezer, SoundCloud, Amazon)
            // Query SongLink and route to native Tidal / Qobuz engines or Amazon fallback
            let candidates_res = self.resolve_songlink_candidates(request).await;

            match candidates_res {
                Ok((candidates, avail)) => {
                    info!(
                        "[Orchestrator] SongLink match for '{}': tidal_id={:?}, qobuz_id={:?}, amazon_url={:?}, resolved candidates={}",
                        request.track_name, avail.tidal_id, avail.qobuz_id, avail.amazon_url.is_some(), candidates.len()
                    );

                    let mut last_error = None;
                    for candidate in candidates {
                        if let Some(token) = cancel_token {
                            if token.is_cancelled() {
                                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, "Download cancelled"));
                                return Err(anyhow!("Download cancelled by user"));
                            }
                        }

                        match candidate {
                            SongLinkEngineTarget::Tidal(tidal_id) => {
                                info!("[Orchestrator] Delegating SongLink match to native Tidal engine (Tidal ID: {})", tidal_id);
                                match self.download_tidal_track(request, &tidal_id).await {
                                    Ok(res) => return Ok(res),
                                    Err(e) => {
                                        warn!("[Orchestrator] Native Tidal engine failed for SongLink match (ID {}): {}. Trying next candidate.", tidal_id, e);
                                        last_error = Some(e);
                                    }
                                }
                            }
                            SongLinkEngineTarget::Qobuz(qobuz_id) => {
                                info!("[Orchestrator] Delegating SongLink match to native Qobuz engine (Qobuz ID: {})", qobuz_id);
                                match self.download_qobuz_track(request, &qobuz_id).await {
                                    Ok(res) => return Ok(res),
                                    Err(e) => {
                                        warn!("[Orchestrator] Native Qobuz engine failed for SongLink match (ID {}): {}. Trying next candidate.", qobuz_id, e);
                                        last_error = Some(e);
                                    }
                                }
                            }
                            SongLinkEngineTarget::Amazon(amazon_url) => {
                                info!("[Orchestrator] Delegating SongLink match to Amazon fallback engine");
                                match self.amazon.download_track(request, &amazon_url).await {
                                    Ok(mut res) => {
                                        Self::reconcile_physical_audio_quality(&mut res, request);
                                        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                                        return Ok(res);
                                    }
                                    Err(e) => {
                                        warn!("[Orchestrator] Amazon fallback engine failed for SongLink track: {}. Trying next candidate.", e);
                                        last_error = Some(e);
                                    }
                                }
                            }
                        }
                    }

                    if let Some(err) = last_error {
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err.to_string()));
                        return Err(err);
                    }
                }
                Err(e) => {
                    warn!("[Orchestrator] SongLink query failed for track '{}': {}", request.track_name, e);
                    // If service was directly Amazon with a direct URL in service_track_id, try Amazon
                    if primary_service == "amazon" {
                        if let Some(ref track_url) = request.service_track_id {
                            if track_url.starts_with("http://") || track_url.starts_with("https://") {
                                let mut res = self.amazon.download_track(request, track_url).await?;
                                Self::reconcile_physical_audio_quality(&mut res, request);
                                PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                                return Ok(res);
                            }
                        }
                    }
                }
            }

            PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &format!("Unsupported or unavailable service: {}", primary_service)));
            Err(anyhow!("Unsupported or unavailable service: {}", primary_service))
        }
    }

    /// Check whether a service has active, valid credentials in the database
    pub async fn is_service_available(&self, service: &str) -> bool {
        if let Some(ref db) = self.db {
            let count: Result<(i64,), _> = sqlx::query_as(
                r#"
                SELECT COUNT(*)
                FROM accounts a
                JOIN services s ON s.id = a.service_id
                WHERE LOWER(s.name) = LOWER(?)
                  AND a.is_active = 1
                  AND COALESCE(a.credentials_invalid, 0) = 0
                "#
            )
            .bind(service)
            .fetch_one(db)
            .await;

            match count {
                Ok((c,)) => c > 0,
                Err(_) => true,
            }
        } else {
            // Standalone / test mode without DB attached
            true
        }
    }

    /// Query SongLink cross-platform availability for a download request
    pub async fn query_songlink(
        &self,
        request: &DownloadRequest,
    ) -> Result<crate::download::songlink::SongLinkAvailability> {
        self.songlink.query_songlink(request).await
    }

    /// Resolve candidate engines from SongLink availability ordered by service_priority
    pub async fn resolve_songlink_candidates(
        &self,
        request: &DownloadRequest,
    ) -> Result<(Vec<SongLinkEngineTarget>, crate::download::songlink::SongLinkAvailability)> {
        let avail = self.query_songlink(request).await?;
        let mut candidates = Vec::new();
        let mut handled = std::collections::HashSet::new();

        for service in &self.service_priority {
            let s = service.to_lowercase();
            match s.as_str() {
                "tidal" => {
                    handled.insert("tidal".to_string());
                    if let Some(ref tid) = avail.tidal_id {
                        if self.is_service_available("tidal").await {
                            candidates.push(SongLinkEngineTarget::Tidal(tid.clone()));
                        }
                    }
                }
                "qobuz" => {
                    handled.insert("qobuz".to_string());
                    if let Some(ref qid) = avail.qobuz_id {
                        if self.is_service_available("qobuz").await {
                            candidates.push(SongLinkEngineTarget::Qobuz(qid.clone()));
                        }
                    }
                }
                "amazon" => {
                    handled.insert("amazon".to_string());
                    if let Some(ref aurl) = avail.amazon_url {
                        candidates.push(SongLinkEngineTarget::Amazon(aurl.clone()));
                    }
                }
                _ => {}
            }
        }

        // Add any remaining unhandled native services that matched
        if !handled.contains("tidal") {
            if let Some(ref tid) = avail.tidal_id {
                if self.is_service_available("tidal").await {
                    candidates.push(SongLinkEngineTarget::Tidal(tid.clone()));
                }
            }
        }
        if !handled.contains("qobuz") {
            if let Some(ref qid) = avail.qobuz_id {
                if self.is_service_available("qobuz").await {
                    candidates.push(SongLinkEngineTarget::Qobuz(qid.clone()));
                }
            }
        }
        if !handled.contains("amazon") {
            if let Some(ref aurl) = avail.amazon_url {
                candidates.push(SongLinkEngineTarget::Amazon(aurl.clone()));
            }
        }

        Ok((candidates, avail))
    }

    /// Download a track via native Tidal engine using SongLink matched track ID
    pub async fn download_tidal_track(
        &self,
        request: &DownloadRequest,
        tidal_id: &str,
    ) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "tidal"));

        let mut tidal_req = request.clone();
        tidal_req.service_name = Some("tidal".to_string());
        tidal_req.service_track_id = Some(tidal_id.to_string());

        let mut tidal_res = self.tidal.download_track(&tidal_req, self.db.as_ref()).await?;
        tidal_res.origin_service = request.service_name.clone().or_else(|| Some("spotify".to_string()));
        tidal_res.origin_service_track_id = request.service_track_id.clone().or_else(|| request.spotify_id.clone());
        tidal_res.effective_service = Some("tidal".to_string());
        tidal_res.effective_service_track_id = Some(tidal_id.to_string());
        tidal_res.fallback_reason = Some("SongLink cross-platform match".to_string());
        tidal_res.match_method = Some("songlink_cross_platform".to_string());
        tidal_res.match_confidence = Some(1.0);
        Self::reconcile_physical_audio_quality(&mut tidal_res, request);
        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
        Ok(tidal_res)
    }

    /// Download a track via native Qobuz engine using SongLink matched track ID
    pub async fn download_qobuz_track(
        &self,
        request: &DownloadRequest,
        qobuz_id: &str,
    ) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "qobuz"));

        let mut qobuz_req = request.clone();
        qobuz_req.service_name = Some("qobuz".to_string());
        qobuz_req.service_track_id = Some(qobuz_id.to_string());

        let mut qobuz_res = self.qobuz.download_track(&qobuz_req, self.db.as_ref()).await?;
        qobuz_res.origin_service = request.service_name.clone().or_else(|| Some("spotify".to_string()));
        qobuz_res.origin_service_track_id = request.service_track_id.clone().or_else(|| request.spotify_id.clone());
        qobuz_res.effective_service = Some("qobuz".to_string());
        qobuz_res.effective_service_track_id = Some(qobuz_id.to_string());
        qobuz_res.fallback_reason = Some("SongLink cross-platform match".to_string());
        qobuz_res.match_method = Some("songlink_cross_platform".to_string());
        qobuz_res.match_confidence = Some(1.0);
        Self::reconcile_physical_audio_quality(&mut qobuz_res, request);
        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
        Ok(qobuz_res)
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
            track_name: "Heroes".to_string(),
            artist_name: "David Bowie".to_string(),
            album_name: "Heroes".to_string(),
            duration_ms: 360000,
            track_number: 1,
            disc_number: 1,
            total_tracks: 10,
            release_date: Some("1977-10-14".to_string()),
            output_dir: "./downloads".to_string(),
            quality: "LOSSLESS".to_string(),
            allow_fallback: true,
            ..Default::default()
        };

        let result = orchestrator.download_track(&req).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("RequiresAuth")
                || err_msg.contains("DownloadOrchestrator requires SqlitePool or user_token")
                || err_msg.contains("401")
                || err_msg.contains("Unauthorized")
                || err_msg.contains("failed")
        );
    }

    #[tokio::test]
    async fn test_orchestrator_cancellation_token() {
        let orchestrator = DownloadOrchestrator::new()
            .with_priority(vec!["qobuz".to_string()]);
        let cancel_token = CancellationToken::new();
        cancel_token.cancel(); // Pre-cancel

        let req = DownloadRequest {
            item_id: "test_item_cancel".to_string(),
            service_name: Some("qobuz".to_string()),
            service_track_id: Some("123".to_string()),
            track_name: "Test Track".to_string(),
            artist_name: "Test Artist".to_string(),
            album_name: "Test Album".to_string(),
            duration_ms: 200000,
            track_number: 1,
            disc_number: 1,
            total_tracks: 1,
            output_dir: "./downloads".to_string(),
            quality: "LOSSLESS".to_string(),
            allow_fallback: false,
            ..Default::default()
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
        data.extend_from_slice(&[0x42; 4096]); // filler bytes: no valid FLAC frames, undecodable
        std::fs::write(&flac_path, &data).unwrap();

        let orchestrator = DownloadOrchestrator::new();

        // 1. Test direct staging analysis
        let analysis = orchestrator.analyze_staging_audio(&flac_path).await.unwrap();

        // 2. Test orchestrator enrichment pipeline
        let req = DownloadRequest {
            item_id: "orch_item_1".to_string(),
            isrc: Some("GBAYE7700021".to_string()),
            service_name: Some("qobuz".to_string()),
            service_track_id: Some("123".to_string()),
            track_name: "Heroes".to_string(),
            artist_name: "David Bowie".to_string(),
            album_name: "Heroes".to_string(),
            duration_ms: 360000,
            track_number: 3,
            disc_number: 1,
            total_tracks: 10,
            release_date: Some("1977-10-14".to_string()),
            output_dir: temp_dir.path().to_str().unwrap().to_string(),
            quality: "LOSSLESS".to_string(),
            allow_fallback: true,
            ..Default::default()
        };

        let enriched = orchestrator.enrich_staging_audio(&flac_path, &req, None).await.unwrap();
        assert_eq!(enriched.title.value(), Some("Heroes"));
        assert_eq!(enriched.artist.value(), Some("David Bowie"));

        // Audit 2026-08-25: the fixture is undecodable (no valid FLAC frames), so
        // every analyzer must fail honestly. The old assertions demanded the values
        // fabricated by the removed estimators (pseudo-ReplayGain, synthetic BPM and
        // "AQAA-" fingerprints); honest absence is now the contract end to end.
        assert!(analysis.replaygain_track_gain.is_none());
        assert!(analysis.bpm.is_none());
        assert!(analysis.acoustid_id.is_none());

        assert!(enriched.replaygain_track_gain.value().is_none());
        assert!(enriched.bpm.value().is_none());
        assert!(enriched.acoustid_id.value().is_none());
    }
}
