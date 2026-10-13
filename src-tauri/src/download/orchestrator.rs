// Download orchestrator - coordinates multiple download services with resilience and cooperative cancellation

use crate::download::amazon::AmazonDownloader;
use crate::download::lyrics::LyricsClient;
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
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use syncify_core_domain::quality::QualityPolicy;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Antigüedad máxima de un veredicto `credentials_invalid` para seguir creyéndolo.
/// Superada, el latch deja de deny-by-default y la descarga vuelve a intentarlo.
pub const AUTH_LATCH_MAX_AGE_HOURS: i64 = 24;

/// A-4: ¿el latch de credenciales ya es demasiado viejo para seguir bloqueando?
///
/// `marked_at` es `accounts.last_auth_error_at` (RFC3339, escrito por
/// `mark_account_credentials_invalid`).
///
/// Se responde `false` (NO está viejo) ante `None` o una fecha ilegible: no
/// consta que el veredicto sea antiguo, así que se conserva el
/// deny-by-default. Relajar solo ante una antigüedad **demostrada** es lo que
/// garantiza que un rechazo recién ocurrido bloquee igual que antes.
pub fn auth_latch_is_stale(marked_at: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> bool {
    let Some(ts) = marked_at else {
        return false;
    };
    match chrono::DateTime::parse_from_rfc3339(ts) {
        Ok(marked) => {
            let age = now.signed_duration_since(marked.with_timezone(&chrono::Utc));
            // Un sello en el futuro (desfase de reloj, BD editada a mano) NO es
            // evidencia de antigüedad: se trata como reciente.
            age.num_hours() >= AUTH_LATCH_MAX_AGE_HOURS
        }
        Err(_) => false,
    }
}

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
pub struct DownloadOrchestrator {
    qobuz: Arc<QobuzDownloader>,
    tidal: Arc<TidalDownloader>,
    amazon: Arc<AmazonDownloader>,
    songlink: Arc<SongLinkClient>,
    // Cliente de letras: lo consume `enrich_staging_audio`, cubierta por el test
    // unitario de este módulo.
    #[allow(dead_code)]
    lyrics: Arc<LyricsClient>,
    // Motor de enriquecimiento: lo consume `enrich_staging_audio`, cubierta por el
    // test unitario de este módulo.
    #[allow(dead_code)]
    enrichment: Arc<EnrichmentEngine>,
    /// Service priority order
    service_priority: Vec<String>,
    /// Database pool for active account token resolution
    db: Option<sqlx::SqlitePool>,
}

impl DownloadOrchestrator {
    // Cubierto por el test unitario de este módulo (`analyze_staging_audio_and_enrichment_on_undecodable_file`).
    #[allow(dead_code)]
    pub async fn analyze_staging_audio(
        &self,
        file_path: &std::path::Path,
    ) -> Result<AudioAnalysisMetrics, String> {
        AudioAnalyzer::analyze_file(file_path).await
    }

    // Cubierto por el test unitario de este módulo (`analyze_staging_audio_and_enrichment_on_undecodable_file`).
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
        self.enrichment = Arc::new(EnrichmentEngine::with_db(db.clone()));
        self.db = Some(db);
        self
    }

    // Consumido por los tests de este módulo (`#[cfg(test)] mod tests`) y por
    // `tests/songlink_native_engines_test.rs`.
    #[allow(dead_code)]
    pub fn with_songlink(mut self, songlink: Arc<SongLinkClient>) -> Self {
        self.songlink = songlink;
        self
    }

    pub fn songlink(&self) -> Arc<SongLinkClient> {
        self.songlink.clone()
    }

    /// Set custom service priority
    // Cubierto por los tests de este módulo (`test_custom_priority`).
    #[allow(dead_code)]
    pub fn with_priority(mut self, priority: Vec<String>) -> Self {
        self.service_priority = priority;
        self
    }

    pub fn service_priority(&self) -> &[String] {
        &self.service_priority
    }

    /// TASK-76: Post-download dead-silence hygiene — detects lead-in / lead-out dead
    /// silence (> 2 s @ -50 dB) on the downloaded file and trims it losslessly
    /// (FLAC stream-copy remux with tag/picture restoration; lossy containers are
    /// measured but never re-encoded). Mirrors `reconcile_physical_audio_quality` as a
    /// static pipeline stage over a finished `DownloadResult`; must run AFTER tagging
    /// (tags are restored onto the trimmed container) and BEFORE `mark_complete`
    /// (the downloads ledger must record post-trim physical metrics).
    ///
    /// Fails soft by contract: any error is logged and the file is left exactly as
    /// downloaded — a trim problem never fails a download.
    pub async fn trim_downloaded_track_silence(
        res: &mut DownloadResult,
    ) -> Option<crate::services::silence_trimmer::SilenceTrimReport> {
        let file_path = std::path::Path::new(&res.file_path);
        match crate::services::silence_trimmer::SilenceTrimmer::process_file(file_path).await {
            Ok(report) => {
                if report.trimmed {
                    info!(
                        file = %res.file_path,
                        lead_in_ms_before = ?report.lead_in_ms_detected,
                        lead_out_ms_before = ?report.lead_out_ms_detected,
                        lead_in_ms_after = ?report.lead_in_ms_after,
                        lead_out_ms_after = ?report.lead_out_ms_after,
                        "[Orchestrator] TASK-76 dead-silence trim applied for gapless transitions"
                    );
                } else if let Some(ref reason) = report.skipped_reason {
                    debug!(
                        file = %res.file_path,
                        reason = %reason,
                        lead_in_ms = ?report.lead_in_ms_detected,
                        lead_out_ms = ?report.lead_out_ms_detected,
                        "[Orchestrator] TASK-76 silence hygiene: no trim ({}); lead_in_ms={:?}, lead_out_ms={:?}",
                        reason, report.lead_in_ms_detected, report.lead_out_ms_detected
                    );
                }
                Some(report)
            }
            Err(e) => {
                warn!(
                    file = %res.file_path,
                    error = %e,
                    "[Orchestrator] TASK-76 silence trim failed; track left intact (bit-exact)"
                );
                None
            }
        }
    }

    /// Download a track, trying services in priority order (backwards-compatible)
    pub async fn download_track(&self, request: &DownloadRequest) -> Result<DownloadResult> {
        Box::pin(self.download_track_cancellable(request, None)).await
    }

    /// Reconciles the downloaded file's physical audio metrics from disk (sample_rate, bit_depth, channels, bitrate)
    /// and performs canonical quality policy evaluation against the physical reality.
    pub fn reconcile_physical_audio_quality(res: &mut DownloadResult, request: &DownloadRequest) {
        let path = std::path::Path::new(&res.file_path);
        let phys_info = crate::download::audio_inspector::inspect_physical_audio_file(path);

        let (
            verified_bd,
            verified_sr,
            verified_fmt,
            verified_channels,
            verified_bitrate,
            quality_label,
        ) = if let Some(ref phys) = phys_info {
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
            let fmt = if is_m4a {
                "AAC"
            } else if is_mp3 {
                "MP3"
            } else {
                "FLAC"
            };
            let label = if fmt == "FLAC" {
                format!(
                    "FLAC {}-bit / {:.1}kHz",
                    res.bit_depth,
                    res.sample_rate as f64 / 1000.0
                )
            } else {
                format!("{} 320kbps", fmt)
            };
            (
                res.bit_depth,
                res.sample_rate,
                fmt.to_string(),
                res.channels.or(Some(2)),
                res.bitrate,
                label,
            )
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

    /// Post-download quality gate: derives verified physical audio quality tier ("hires", "lossless", "lossy")
    /// directly from physical audio inspection.
    /// Rejects assigning the "hires" label if the stream is 16-bit/44.1kHz or <= 16-bit and <= 48kHz.
    #[allow(dead_code)] // Cubierta por `tests/hires_quality_reconciliation_test.rs`.
    pub fn verify_post_download_quality_gate(res: &DownloadResult) -> &'static str {
        let path = std::path::Path::new(&res.file_path);
        if let Some(phys) = crate::download::audio_inspector::inspect_physical_audio_file(path) {
            phys.canonical_quality()
        } else {
            let fmt = if res.file_path.to_lowercase().ends_with(".flac") {
                "FLAC"
            } else if res.file_path.to_lowercase().ends_with(".mp3") {
                "MP3"
            } else if res.file_path.to_lowercase().ends_with(".m4a")
                || res.file_path.to_lowercase().ends_with(".aac")
            {
                "AAC"
            } else {
                "FLAC"
            };
            crate::download::audio_inspector::classify_physical_audio_quality(
                res.bit_depth,
                res.sample_rate,
                fmt,
            )
        }
    }

    /// Download a track with cooperative cancellation support
    /// Resolve an equivalent edition on Tidal/Qobuz for a stale source following strict equivalence hierarchy
    pub async fn resolve_edition_identity_fallback(
        &self,
        request: &DownloadRequest,
    ) -> Result<FallbackMatch, String> {
        let duration_sec = (request.duration_ms / 1000) as i32;

        let primary_service = request
            .service_name
            .as_deref()
            .unwrap_or("qobuz")
            .to_lowercase();
        let target_service = if primary_service == "tidal" {
            "qobuz"
        } else {
            "tidal"
        };
        let target_service_display = if target_service == "tidal" {
            "Tidal"
        } else {
            "Qobuz"
        };

        // 1. Exact ISRC matching
        if let Some(ref isrc) = request.isrc {
            let isrc_trimmed = isrc.trim();
            if !isrc_trimmed.is_empty() {
                debug!(
                    "[Orchestrator] Fallback Step 1: Searching {} by exact ISRC '{}'",
                    target_service_display, isrc_trimmed
                );

                // 1A. Check local database if available
                if let Some(ref db) = self.db {
                    let isrc_candidates: Vec<(String, Option<String>)> = sqlx::query_as(
                        r#"
                        SELECT ts.service_track_id, ts.format
                        FROM track_sources ts
                        JOIN services s ON s.id = ts.service_id AND s.name = ?
                        JOIN tracks t ON t.id = ts.track_id
                        WHERE t.isrc = ? AND ts.available = 1
                        "#,
                    )
                    .bind(target_service)
                    .bind(isrc_trimmed)
                    .fetch_all(db)
                    .await
                    .map_err(|e| {
                        format!(
                            "DatabaseError: Failed to query {} candidates by ISRC: {}",
                            target_service_display, e
                        )
                    })?;

                    if isrc_candidates.len() == 1 {
                        let (stid, fmt) = &isrc_candidates[0];
                        if let Ok(tid) = stid.parse::<i64>() {
                            info!(
                                "[Orchestrator] ✓ Fallback matched via DB exact ISRC: {} ID {}",
                                target_service_display, tid
                            );
                            return Ok(FallbackMatch {
                                target_track_id: tid,
                                target_service: target_service.to_string(),
                                match_method: "exact_isrc".to_string(),
                                match_confidence: 1.0,
                                candidate_audio_quality: fmt.clone(),
                            });
                        }
                    } else if isrc_candidates.len() > 1 {
                        return Err(format!(
                            "AmbiguousSource: Multiple competing {} tracks found for ISRC {}",
                            target_service_display, isrc_trimmed
                        ));
                    }
                }

                // 1B. Query provider API
                if target_service == "tidal" {
                    match self.tidal.search_by_isrc(isrc_trimmed, duration_sec).await {
                        Ok(track) => {
                            info!(
                                "[Orchestrator] ✓ Fallback matched via exact ISRC: Tidal ID {}",
                                track.id
                            );
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
                            if err_msg.contains("AmbiguousSource")
                                || err_msg.contains("Multiple competing")
                            {
                                return Err(format!("AmbiguousSource: Multiple competing Tidal tracks found for ISRC {}", isrc_trimmed));
                            }
                        }
                    }
                } else if target_service == "qobuz" {
                    match self
                        .qobuz
                        .search_by_isrc(isrc_trimmed, duration_sec, None)
                        .await
                    {
                        Ok(track) => {
                            info!(
                                "[Orchestrator] ✓ Fallback matched via exact ISRC: Qobuz ID {}",
                                track.id
                            );
                            let cand_q = track
                                .max_bit_depth
                                .map(|bd| {
                                    if bd > 16 {
                                        "HI_RES".to_string()
                                    } else {
                                        "LOSSLESS".to_string()
                                    }
                                })
                                .or_else(|| Some("LOSSLESS".to_string()));
                            return Ok(FallbackMatch {
                                target_track_id: track.id,
                                target_service: "qobuz".to_string(),
                                match_method: "exact_isrc".to_string(),
                                match_confidence: 1.0,
                                candidate_audio_quality: cand_q,
                            });
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            if err_msg.contains("AmbiguousSource")
                                || err_msg.contains("Multiple competing")
                            {
                                return Err(format!("AmbiguousSource: Multiple competing Qobuz tracks found for ISRC {}", isrc_trimmed));
                            }
                        }
                    }
                }
            }
        }

        // 2. Exact MusicBrainz Recording ID
        if let Some(ref mb_rid) = request.musicbrainz_recording_id {
            let mb_rid_trimmed = mb_rid.trim();
            if !mb_rid_trimmed.is_empty() {
                debug!(
                    "[Orchestrator] Fallback Step 2: Searching {} by MusicBrainz Recording ID '{}'",
                    target_service_display, mb_rid_trimmed
                );
                if let Some(ref db) = self.db {
                    let mb_candidates: Vec<(String, Option<String>)> = sqlx::query_as(
                        r#"
                        SELECT ts.service_track_id, ts.format
                        FROM track_sources ts
                        JOIN services s ON s.id = ts.service_id AND s.name = ?
                        JOIN tracks t ON t.id = ts.track_id
                        WHERE t.musicbrainz_id = ? AND ts.available = 1
                        "#,
                    )
                    .bind(target_service)
                    .bind(mb_rid_trimmed)
                    .fetch_all(db)
                    .await
                    .map_err(|e| format!("DatabaseError: Failed to query {} candidates by MusicBrainz Recording ID: {}", target_service_display, e))?;

                    if mb_candidates.len() == 1 {
                        let (stid, fmt) = &mb_candidates[0];
                        if let Ok(tid) = stid.parse::<i64>() {
                            info!("[Orchestrator] ✓ Fallback matched via MusicBrainz Recording ID: {} ID {}", target_service_display, tid);
                            return Ok(FallbackMatch {
                                target_track_id: tid,
                                target_service: target_service.to_string(),
                                match_method: "musicbrainz_recording_id".to_string(),
                                match_confidence: 0.95,
                                candidate_audio_quality: fmt.clone(),
                            });
                        }
                    } else if mb_candidates.len() > 1 {
                        return Err(format!("AmbiguousSource: Multiple competing {} tracks for MusicBrainz Recording ID {}", target_service_display, mb_rid_trimmed));
                    }
                }
            }
        }

        // 3. MusicBrainz release/track matching with tolerant duration (±3s)
        if let Some(ref db) = self.db {
            debug!("[Orchestrator] Fallback Step 3: Searching {} by MusicBrainz release/track tolerance for '{}'", target_service_display, request.track_name);
            let mb_rel_candidates: Vec<(String, i64, Option<String>)> = sqlx::query_as(
                r#"
                SELECT ts.service_track_id, t.duration_ms, ts.format
                FROM track_sources ts
                JOIN services s ON s.id = ts.service_id AND s.name = ?
                JOIN tracks t ON t.id = ts.track_id
                WHERE t.musicbrainz_id IS NOT NULL AND ts.available = 1
                  AND LOWER(t.title) = LOWER(?)
                "#,
            )
            .bind(target_service)
            .bind(&request.track_name)
            .fetch_all(db)
            .await
            .map_err(|e| {
                format!(
                    "DatabaseError: Failed to query {} candidates by MusicBrainz release/track: {}",
                    target_service_display, e
                )
            })?;

            let duration_matches: Vec<_> = mb_rel_candidates
                .into_iter()
                .filter(|(stid, dur, _)| {
                    !stid.trim().is_empty()
                        && (dur / 1000 - (request.duration_ms / 1000)).abs() <= 3
                })
                .collect();

            if duration_matches.len() == 1 {
                let (stid, _, fmt) = &duration_matches[0];
                if let Ok(tid) = stid.parse::<i64>() {
                    info!(
                        "[Orchestrator] ✓ Fallback matched via MusicBrainz release/track: {} ID {}",
                        target_service_display, tid
                    );
                    return Ok(FallbackMatch {
                        target_track_id: tid,
                        target_service: target_service.to_string(),
                        match_method: "musicbrainz_release_track".to_string(),
                        match_confidence: 0.85,
                        candidate_audio_quality: fmt.clone(),
                    });
                }
            } else if duration_matches.len() > 1 {
                return Err(format!(
                    "AmbiguousSource: Multiple competing MusicBrainz release/track matches on {}",
                    target_service_display
                ));
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
                debug!(
                    "[Orchestrator] Fallback Step 4: Searching {} by AcoustID fingerprint",
                    target_service_display
                );
                if let Some(ref db) = self.db {
                    let fp_candidates: Vec<(String, Option<String>)> = sqlx::query_as(
                        r#"
                        SELECT ts.service_track_id, ts.format
                        FROM track_sources ts
                        JOIN services s ON s.id = ts.service_id AND s.name = ?
                        JOIN tracks t ON t.id = ts.track_id
                        WHERE t.acoustid_fingerprint = ? AND ts.available = 1
                        "#,
                    )
                    .bind(target_service)
                    .bind(fp_trimmed)
                    .fetch_all(db)
                    .await
                    .map_err(|e| format!("DatabaseError: Failed to query {} candidates by AcoustID fingerprint: {}", target_service_display, e))?;

                    if fp_candidates.len() == 1 {
                        let (stid, fmt) = &fp_candidates[0];
                        if let Ok(tid) = stid.parse::<i64>() {
                            info!("[Orchestrator] ✓ Fallback matched via AcoustID fingerprint: {} ID {}", target_service_display, tid);
                            return Ok(FallbackMatch {
                                target_track_id: tid,
                                target_service: target_service.to_string(),
                                match_method: "acoustid_fingerprint".to_string(),
                                match_confidence: 0.80,
                                candidate_audio_quality: fmt.clone(),
                            });
                        }
                    } else if fp_candidates.len() > 1 {
                        return Err(format!("AmbiguousSource: Multiple competing AcoustID fingerprint matches on {}", target_service_display));
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
                JOIN services s ON s.id = ts.service_id AND s.name = ?
                JOIN tracks t ON t.id = ts.track_id
                WHERE LOWER(t.title) = LOWER(?) AND ts.available = 1
                "#,
            )
            .bind(target_service)
            .bind(&request.track_name)
            .fetch_one(db)
            .await
            .map_err(|e| {
                format!(
                    "DatabaseError: Failed to query {} candidate count: {}",
                    target_service_display, e
                )
            })?;
            count > 0
        } else {
            false
        };

        let has_metadata_match = if has_local_metadata_match {
            true
        } else if target_service == "tidal" {
            self.tidal
                .search_by_metadata(&request.track_name, &request.artist_name, duration_sec)
                .await
                .is_ok()
        } else {
            false
        };

        if has_metadata_match {
            return Err(format!("AmbiguousSource: Fallback to {} produced only loose metadata (title/artist) match without edition identity (ISRC/MBID/AcoustID). Automatic download forbidden.", target_service_display));
        }

        // 6. No match found
        Err(format!(
            "SourceIdentityMissing: No equivalent {} source found for track '{}'",
            target_service_display, request.track_name
        ))
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

        let primary_service = request
            .service_name
            .as_deref()
            .unwrap_or("qobuz")
            .to_lowercase();

        let is_lossless_or_hires = QualityPolicy::is_lossless_or_hires(&request.quality);

        if primary_service == "qobuz" {
            self.download_primary_qobuz(request, is_lossless_or_hires)
                .await
        } else if primary_service == "tidal" {
            self.download_primary_tidal(request, is_lossless_or_hires)
                .await
        } else {
            self.download_songlink_routing(
                request,
                is_lossless_or_hires,
                cancel_token,
                &primary_service,
            )
            .await
        }
    }

    fn download_primary_qobuz<'a>(
        &'a self,
        request: &'a DownloadRequest,
        is_lossless_or_hires: bool,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(async move {
            let item_id = &request.item_id;
            // Attempt direct download via locked Qobuz source
            debug!(
                "[Orchestrator] Attempting primary download via Qobuz (service_track_id={:?})",
                request.service_track_id
            );
            // FS-05: When user asks for lossless/Hi-Res, do NOT degrade to lossy (MP3 320)
            // within Qobuz while other services (Tidal) are available.
            let mut qobuz_req = request.clone();
            if is_lossless_or_hires {
                qobuz_req.allow_fallback = false;
            }
            let qobuz_result = self
                .qobuz
                .download_track(&qobuz_req, self.db.as_ref())
                .await;

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
                    info!(
                        "[Orchestrator] Download complete via exact Qobuz source: {}",
                        res.file_path
                    );
                    PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                    Ok(res)
                }
                Err(err) => {
                    self.handle_qobuz_fallback(request, err, is_lossless_or_hires)
                        .await
                }
            }
        })
    }

    // Keep a preview as the primary failure. A Tidal-side 401/403 is not proof
    // that the Qobuz session expired: the worker still classifies auth from
    // the *whole* returned message. Never append raw fallback errors to it.
    fn preview_fallback_diagnostic(error: &str) -> &'static str {
        if error.starts_with("SourceIdentityMissing:") {
            "no confirmed equivalent Tidal edition found"
        } else if error.starts_with("AmbiguousSource:") {
            "multiple Tidal editions; no safe automatic choice"
        } else if error.contains("RequiresAuth") || error.contains("401") || error.contains("403") {
            "Tidal account could not serve the candidate"
        } else if error.contains("RejectedQuality") {
            "Tidal candidate did not meet the requested quality"
        } else {
            "Tidal candidate lookup or download failed"
        }
    }

    fn handle_qobuz_fallback<'a>(
        &'a self,
        request: &'a DownloadRequest,
        err: anyhow::Error,
        is_lossless_or_hires: bool,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(async move {
            let item_id = &request.item_id;
            let err_msg = err.to_string();
            warn!("[Orchestrator] Qobuz download failed: {}", err_msg);

            // 1. Auth failure (401/403) -> abort without fallback
            if err_msg.contains("401")
                || err_msg.contains("403")
                || err_msg.contains("RequiresAuth")
                || err_msg.contains("authentication failed")
                || (err_msg.contains("token")
                    && (err_msg.contains("expired") || err_msg.contains("invalid")))
            {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                let abort_reason = if err_msg.contains("401") || err_msg.contains("403") {
                    "RequiresAuth: Qobuz authentication required (HTTP 401/403). Automatic fallback aborted.".to_string()
                } else if err_msg.starts_with("RequiresAuth:") {
                    format!(
                        "{}. Automatic fallback aborted.",
                        err_msg.trim_end_matches('.')
                    )
                } else {
                    format!(
                        "RequiresAuth: {}. Automatic fallback aborted.",
                        err_msg.trim_end_matches('.')
                    )
                };
                return Err(anyhow!("{}", abort_reason));
            }

            // 2. Network Exhausted (stream / connection failures that exhausted retries) -> abort without fallback
            if err_msg.contains("NetworkExhausted") {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                return Err(anyhow!(
                    "NetworkExhausted: Qobuz network stream exhausted retries: {}",
                    err_msg
                ));
            }

            // 3. Check for stale source or quality rejection
            let is_stale = err_msg.contains("404")
                || err_msg.contains("NotFound")
                || err_msg.contains("StaleSource")
                || err_msg.contains("not found")
                || err_msg.contains("track/get failed")
                || err_msg.contains("unavailable")
                || err_msg.contains("CountryNotAvailable");
            let is_rejected_q = err_msg.contains("RejectedQuality");
            let is_preview = err_msg.starts_with("QobuzPreviewPayload:");

            // A preview is neither a credential rejection nor a usable 320
            // stream. Do not turn it into StaleSource/RejectedQuality when
            // cross-service fallback is disabled; preserve the diagnosis.
            if is_preview && !request.allow_fallback {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                return Err(err);
            }

            if (is_stale || is_rejected_q) && !request.allow_fallback {
                let fail_msg = if is_rejected_q {
                    "RejectedQuality: Requested quality not available on Qobuz"
                } else {
                    "StaleSource: Qobuz track not found (HTTP 404) and allow_fallback=false"
                };
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, fail_msg));
                return Err(anyhow!("{}", fail_msg));
            }

            // 3.5 WF-RES-01: re-resolución EXPLÍCITA antes de cambiar de servicio.
            //
            // El plan dijo que esta cuenta/edición podía servir la petición; el
            // proveedor no estuvo de acuerdo. Se clasifica el fallo, se excluye
            // ESE intento y se vuelve a resolver con los mismos techos. Tras un
            // preview o un 401, primero corren las cuentas hermanas Qobuz del
            // MISMO service_track_id; Tidal solo entra cuando la política permite
            // salir del servicio, y siempre por la jerarquía de equivalencia.
            if let Some(plan) = request.resolution_plan.as_ref() {
                if let Some(trigger) = crate::download::classify_attempt_failure(&err_msg) {
                    let service_id = plan.primary().map(|c| c.service_id).unwrap_or_default();
                    let attempted_track_id = plan
                        .primary()
                        .map(|c| c.service_track_id.clone())
                        .unwrap_or_default();
                    let attempted_account = plan
                        .primary()
                        .and_then(|c| c.account.as_ref().map(|a| a.account_id));

                    let re_resolution = Box::pin(self.reresolve_after_classified_failure(
                        request,
                        trigger,
                        service_id,
                        &attempted_track_id,
                        attempted_account,
                        is_lossless_or_hires,
                    ))
                    .await;

                    match re_resolution {
                        Ok(res) => {
                            PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                            return Ok(res);
                        }
                        Err(reresolution_err) => {
                            info!(
                                "[Orchestrator] WF-RES-01 re-resolution after {:?} exhausted for {} ({})",
                                trigger, attempted_track_id, reresolution_err
                            );
                        }
                    }
                }
            }

            if (is_stale || is_rejected_q || is_preview) && request.allow_fallback {
                info!("[Orchestrator] Qobuz cannot serve requested quality ({}); attempting cross-service fallback to Tidal...", err_msg);
                PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "tidal"));

                let fallback_res = Box::pin(self.resolve_edition_identity_fallback(request)).await;

                match fallback_res {
                    Ok(fallback_match) => {
                        // First: check if Tidal candidate has the requested quality
                        let is_inferior = QualityPolicy::is_quality_inferior(
                            Some(&request.quality),
                            fallback_match.candidate_audio_quality.as_deref(),
                            fallback_match.candidate_audio_quality.as_deref(),
                            None,
                        );

                        if !is_inferior {
                            let mut tidal_req = request.clone();
                            tidal_req.service_name = Some("tidal".to_string());
                            tidal_req.service_track_id =
                                Some(fallback_match.target_track_id.to_string());
                            if is_lossless_or_hires {
                                tidal_req.allow_fallback = false;
                            }
                            match Box::pin(self.tidal.download_track(&tidal_req, self.db.as_ref()))
                                .await
                            {
                                Ok(mut tidal_res) => {
                                    tidal_res.origin_service =
                                        request.service_name.clone().or(Some("qobuz".to_string()));
                                    tidal_res.origin_service_track_id =
                                        request.service_track_id.clone();
                                    tidal_res.effective_service = Some("tidal".to_string());
                                    tidal_res.effective_service_track_id =
                                        Some(fallback_match.target_track_id.to_string());
                                    tidal_res.fallback_reason = Some(format!(
                                            "Qobuz requested quality unavailable ({}) -> Tidal fallback",
                                            err_msg
                                        ));
                                    tidal_res.match_method = Some(fallback_match.match_method);
                                    tidal_res.match_confidence =
                                        Some(fallback_match.match_confidence);
                                    Self::reconcile_physical_audio_quality(&mut tidal_res, request);
                                    info!(
                                            "[Orchestrator] Download complete via Tidal fallback: {} (was Qobuz ID {:?}, now Tidal ID {})",
                                            tidal_res.file_path,
                                            request.service_track_id,
                                            fallback_match.target_track_id
                                        );
                                    PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                                    return Ok(tidal_res);
                                }
                                Err(tidal_err) if is_preview => {
                                    let combined = format!(
                                        "{}; Tidal equivalent download failed: {}",
                                        err_msg,
                                        Self::preview_fallback_diagnostic(&tidal_err.to_string())
                                    );
                                    PROGRESS_TRACKER
                                        .update(DownloadProgress::failed(item_id, &combined));
                                    return Err(anyhow!("{}", combined));
                                }
                                Err(_) => {}
                            }
                        }

                        // Neither Qobuz nor Tidal provided requested lossless quality
                        let is_strict = request.strict_quality || !request.allow_fallback;
                        if is_strict {
                            if is_preview {
                                let combined = format!(
                                    "{}; Tidal equivalent did not satisfy requested quality",
                                    err_msg
                                );
                                PROGRESS_TRACKER
                                    .update(DownloadProgress::failed(item_id, &combined));
                                return Err(anyhow!("{}", combined));
                            }
                            let cq = fallback_match
                                .candidate_audio_quality
                                .as_deref()
                                .unwrap_or("lossy");
                            let rej_err = format!(
                                        "RejectedQuality: Tidal fallback candidate quality '{}' is inferior to requested '{}' under strict policy",
                                        cq, request.quality
                                    );
                            PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &rej_err));
                            return Err(anyhow!("{}", rej_err));
                        }

                        // FS-05: Solo si no se encuentra en ninguno, bajar a 320 — pero NUNCA por debajo.
                        // 1. Probar Qobuz a 320 kbps (si la pista existía pero no en lossless)
                        if is_rejected_q {
                            let mut qobuz_320_req = request.clone();
                            qobuz_320_req.quality = "HIGH".to_string();
                            qobuz_320_req.allow_fallback = true;
                            if let Ok(mut qobuz_res) = self
                                .qobuz
                                .download_track(&qobuz_320_req, self.db.as_ref())
                                .await
                            {
                                qobuz_res.origin_service = Some("qobuz".to_string());
                                qobuz_res.origin_service_track_id =
                                    request.service_track_id.clone();
                                qobuz_res.effective_service = Some("qobuz".to_string());
                                qobuz_res.effective_service_track_id =
                                    request.service_track_id.clone();
                                qobuz_res.fallback_reason = Some(
                                    "No service had lossless; degraded to 320 kbps on Qobuz"
                                        .to_string(),
                                );
                                qobuz_res.match_method = Some("exact_locked_source".to_string());
                                qobuz_res.match_confidence = Some(1.0);
                                Self::reconcile_physical_audio_quality(&mut qobuz_res, request);
                                PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                                return Ok(qobuz_res);
                            }
                        }

                        // 2. Probar Tidal a 320 kbps (verificando suelo mínimo de 320 kbps)
                        if QualityPolicy::is_below_minimum_floor(
                            fallback_match.candidate_audio_quality.as_deref(),
                            None,
                            None,
                        ) {
                            let floor_err = format!(
                                        "RejectedQuality: Tidal candidate quality '{:?}' is below the 320 kbps minimum floor",
                                        fallback_match.candidate_audio_quality
                                    );
                            PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &floor_err));
                            return Err(anyhow!("{}", floor_err));
                        }

                        let mut tidal_320_req = request.clone();
                        tidal_320_req.service_name = Some("tidal".to_string());
                        tidal_320_req.service_track_id =
                            Some(fallback_match.target_track_id.to_string());
                        tidal_320_req.quality = "HIGH".to_string();
                        tidal_320_req.allow_fallback = true;
                        let mut tidal_res = match Box::pin(
                            self.tidal.download_track(&tidal_320_req, self.db.as_ref()),
                        )
                        .await
                        {
                            Ok(res) => res,
                            Err(tidal_err) if is_preview => {
                                let combined = format!(
                                    "{}; Tidal 320 fallback failed: {}",
                                    err_msg,
                                    Self::preview_fallback_diagnostic(&tidal_err.to_string())
                                );
                                PROGRESS_TRACKER
                                    .update(DownloadProgress::failed(item_id, &combined));
                                return Err(anyhow!("{}", combined));
                            }
                            Err(tidal_err) => return Err(tidal_err),
                        };
                        tidal_res.origin_service =
                            request.service_name.clone().or(Some("qobuz".to_string()));
                        tidal_res.origin_service_track_id = request.service_track_id.clone();
                        tidal_res.effective_service = Some("tidal".to_string());
                        tidal_res.effective_service_track_id =
                            Some(fallback_match.target_track_id.to_string());
                        tidal_res.fallback_reason = Some(format!(
                            "No service had lossless; Tidal 320 kbps fallback ({})",
                            err_msg
                        ));
                        tidal_res.match_method = Some(fallback_match.match_method);
                        tidal_res.match_confidence = Some(fallback_match.match_confidence);
                        Self::reconcile_physical_audio_quality(&mut tidal_res, request);
                        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                        Ok(tidal_res)
                    }
                    Err(fallback_err) => {
                        // No equivalent Tidal source was confirmed. For a
                        // Qobuz preview, do not try the same account at 320:
                        // it may again be a sample, and the full lossless
                        // request remains unsatisfied. Keep both diagnoses.
                        if is_preview {
                            let combined = format!(
                                "{}; Tidal equivalent unavailable: {}",
                                err_msg,
                                Self::preview_fallback_diagnostic(&fallback_err)
                            );
                            PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &combined));
                            return Err(anyhow!("{}", combined));
                        }
                        if is_rejected_q && !request.strict_quality && request.allow_fallback {
                            let mut qobuz_320_req = request.clone();
                            qobuz_320_req.quality = "HIGH".to_string();
                            qobuz_320_req.allow_fallback = true;
                            if let Ok(mut qobuz_res) = self
                                .qobuz
                                .download_track(&qobuz_320_req, self.db.as_ref())
                                .await
                            {
                                qobuz_res.origin_service = Some("qobuz".to_string());
                                qobuz_res.origin_service_track_id =
                                    request.service_track_id.clone();
                                qobuz_res.effective_service = Some("qobuz".to_string());
                                qobuz_res.effective_service_track_id =
                                    request.service_track_id.clone();
                                qobuz_res.fallback_reason = Some(
                                    "No Tidal match; degraded to 320 kbps on Qobuz".to_string(),
                                );
                                qobuz_res.match_method = Some("exact_locked_source".to_string());
                                qobuz_res.match_confidence = Some(1.0);
                                Self::reconcile_physical_audio_quality(&mut qobuz_res, request);
                                PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                                return Ok(qobuz_res);
                            }
                        }
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &fallback_err));
                        Err(anyhow!("{}", fallback_err))
                    }
                }
            } else {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                Err(anyhow!("Qobuz download failed: {}", err_msg))
            }
        })
    }

    /// WF-RES-01: tras un fallo clasificado, resuelve de nuevo y ejecuta la
    /// secuencia completa del plan excluyendo el intento que falló.
    ///
    /// La secuencia es `expand_attempt_sequence`: en cada candidato, su cuenta
    /// primaria seguida INMEDATAMENTE por las cuentas hermanas del MISMO
    /// `service_track_id`. Por tanto un preview o un 401 agota las hermanas Qobuz
    /// antes de que otra edición entre, y nunca se repite una combinación.
    ///
    /// La calidad y la edición viajan con el plan: este método no reevalúa
    /// techos ni reordena cuentas. Tras cruzar de servicio solo se admiten
    /// candidatos de la jerarquía de equivalencia (ISRC validado), nunca un
    /// SongLink: no es prueba de audio descargable.
    #[allow(clippy::too_many_arguments)]
    async fn reresolve_after_classified_failure(
        &self,
        request: &DownloadRequest,
        trigger: crate::download::ReResolutionTrigger,
        service_id: i64,
        attempted_track_id: &str,
        attempted_account: Option<i64>,
        is_lossless_or_hires: bool,
    ) -> Result<DownloadResult> {
        let canonical_track_id = request.canonical_track_id.unwrap_or_default();
        let exclusions = crate::download::exclusions_for_failure(
            trigger,
            canonical_track_id,
            service_id,
            attempted_track_id,
            attempted_account,
        );

        // La re-resolución solo tiene sentido con base de datos: sin ella no hay
        // identidad de edición que reanudar.
        let Some(db) = self.db.as_ref() else {
            return Err(anyhow!(
                "SourceIdentityMissing: cannot re-resolve without a database pool"
            ));
        };

        let intent = crate::download::DownloadIntent {
            canonical_track_id: request.canonical_track_id.unwrap_or_default(),
            requested_service: request.service_name.clone(),
            requested_quality: Some(request.quality.clone()),
            strict_quality: request.strict_quality,
            allow_fallback: request.allow_fallback,
            // La edición ya fijada se conserva: no se permite que el reorden
            // cambie de edición sin pasar por la equivalencia.
            locked_track_source_id: crate::download::resolve_locked_track_source_id(
                db,
                request.canonical_track_id.unwrap_or_default(),
                request.service_name.as_deref(),
                request.service_track_id.as_deref(),
            )
            .await,
            preferred_account_id: None,
        };

        // Misma intención que produjo el plan original, así que el techo de
        // calidad no puede derivar entre el primer intento y este.
        let re_plan = crate::download::resolve_download_plan(db, &intent, &exclusions)
            .await
            .map_err(|e| anyhow!("{e:?}"))?;

        // Las exclusiones se ACUMULAN entre rondas: cada fallo clasificado se
        // suma al paquete anterior, así que una ronda nueva no puede reintentar
        // una combinación que una ronda anterior ya descartó.
        let mut exclusions = exclusions;
        let steps = crate::download::expand_attempt_sequence(&re_plan);
        for step in steps {
            let Some(candidate) = step.identity(&re_plan) else {
                continue;
            };
            let Some(account) = step.account(&re_plan) else {
                continue;
            };
            // Nunca reintentar la misma combinación.
            if exclusions.excludes_identity(candidate.service_id, &candidate.service_track_id)
                || exclusions.excludes_account(
                    re_plan.canonical_track_id,
                    candidate.service_id,
                    account.account_id,
                )
            {
                continue;
            }

            let mut candidate_req = request.clone();
            candidate_req.service_name = Some(candidate.service_name.clone());
            candidate_req.service_track_id = Some(candidate.service_track_id.clone());
            candidate_req.quality = re_plan.quality.effective.clone();
            candidate_req.resolution_plan = Some(crate::download::DownloadPlan {
                canonical_track_id: re_plan.canonical_track_id,
                candidates: vec![candidate.clone()],
                quality: re_plan.quality.clone(),
                strict_quality: re_plan.strict_quality,
                allow_fallback: re_plan.allow_fallback,
            });
            if is_lossless_or_hires {
                candidate_req.allow_fallback = false;
            }

            info!(
                "[Orchestrator] WF-RES-01 re-resolution after {:?}: edition {} on service '{}' with account {}",
                trigger,
                candidate.service_track_id,
                candidate.service_name,
                account.account_id
            );

            let outcome = if candidate.service_name.eq_ignore_ascii_case("qobuz") {
                Box::pin(self.qobuz.download_track(&candidate_req, Some(db))).await
            } else {
                Box::pin(self.tidal.download_track(&candidate_req, Some(db))).await
            };

            match outcome {
                Ok(mut res) => {
                    res.origin_service = request.service_name.clone();
                    res.origin_service_track_id = request.service_track_id.clone();
                    res.effective_service = Some(candidate.service_name.clone());
                    res.effective_service_track_id = Some(candidate.service_track_id.clone());
                    res.fallback_reason = Some(format!(
                        "Qobuz re-resolution after {:?} on edition {}",
                        trigger, attempted_track_id
                    ));
                    res.match_method = Some("plan_re_resolution".to_string());
                    res.match_confidence = Some(0.95);
                    Self::reconcile_physical_audio_quality(&mut res, request);
                    return Ok(res);
                }
                Err(retry_err) => {
                    // Un fallo no clasificable (cancelación incluida) detiene la
                    // secuencia en lugar de seguir intentando.
                    if crate::download::classify_attempt_failure(&retry_err.to_string()).is_none() {
                        warn!(
                            "[Orchestrator] WF-RES-01 re-resolution stopped at edition {}: {}",
                            candidate.service_track_id, retry_err
                        );
                        break;
                    }
                    // Registrar el nuevo fallo en el paquete acumulado.
                    let round_exclusions = crate::download::exclusions_for_failure(
                        crate::download::classify_attempt_failure(&retry_err.to_string())
                            .unwrap_or(trigger),
                        re_plan.canonical_track_id,
                        candidate.service_id,
                        &candidate.service_track_id,
                        Some(account.account_id),
                    );
                    exclusions.merge(&round_exclusions);
                    debug!(
                        "[Orchestrator] WF-RES-01 candidate {} failed again; continuing with the next attempt",
                        candidate.service_track_id
                    );
                }
            }
        }

        Err(anyhow!(
            "Qobuz re-resolution exhausted after {:?} for edition {}",
            trigger,
            attempted_track_id
        ))
    }

    fn download_primary_tidal<'a>(
        &'a self,
        request: &'a DownloadRequest,
        is_lossless_or_hires: bool,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(async move {
            let item_id = &request.item_id;
            let mut tidal_req = request.clone();
            if is_lossless_or_hires {
                tidal_req.allow_fallback = false;
            }
            let tidal_result =
                Box::pin(self.tidal.download_track(&tidal_req, self.db.as_ref())).await;

            match tidal_result {
                Ok(mut res) => {
                    res.origin_service = request.service_name.clone().or(Some("tidal".to_string()));
                    res.origin_service_track_id = request.service_track_id.clone();
                    res.effective_service = Some("tidal".to_string());
                    res.effective_service_track_id = request.service_track_id.clone();
                    res.fallback_reason = None;
                    res.match_method = Some("exact_locked_source".to_string());
                    res.match_confidence = Some(1.0);
                    Self::reconcile_physical_audio_quality(&mut res, request);
                    PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                    Ok(res)
                }
                Err(err) => {
                    self.handle_tidal_fallback(request, err, is_lossless_or_hires)
                        .await
                }
            }
        })
    }

    fn handle_tidal_fallback<'a>(
        &'a self,
        request: &'a DownloadRequest,
        err: anyhow::Error,
        is_lossless_or_hires: bool,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(async move {
            let item_id = &request.item_id;
            let err_msg = err.to_string();
            warn!("[Orchestrator] Tidal download failed: {}", err_msg);

            // 1. Auth failure (401/403) -> abort without fallback
            if err_msg.contains("401")
                || err_msg.contains("403")
                || err_msg.contains("RequiresAuth")
                || err_msg.contains("authentication failed")
            {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                let abort_reason = format!(
                    "RequiresAuth: {}. Automatic fallback aborted.",
                    err_msg.trim_end_matches('.')
                );
                return Err(anyhow!("{}", abort_reason));
            }

            if !request.allow_fallback {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                return Err(err);
            }

            // Attempt fallback to Qobuz
            info!(
                "[Orchestrator] Tidal failed ({}); attempting cross-service fallback to Qobuz...",
                err_msg
            );
            PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "qobuz"));

            let fallback_res = Box::pin(self.resolve_edition_identity_fallback(request)).await;
            match fallback_res {
                Ok(fallback_match) => {
                    let is_inferior = QualityPolicy::is_quality_inferior(
                        Some(&request.quality),
                        fallback_match.candidate_audio_quality.as_deref(),
                        fallback_match.candidate_audio_quality.as_deref(),
                        None,
                    );

                    if !is_inferior {
                        let mut qobuz_req = request.clone();
                        qobuz_req.service_name = Some("qobuz".to_string());
                        qobuz_req.service_track_id =
                            Some(fallback_match.target_track_id.to_string());
                        if is_lossless_or_hires {
                            qobuz_req.allow_fallback = false;
                        }
                        if let Ok(mut qobuz_res) = self
                            .qobuz
                            .download_track(&qobuz_req, self.db.as_ref())
                            .await
                        {
                            qobuz_res.origin_service =
                                request.service_name.clone().or(Some("tidal".to_string()));
                            qobuz_res.origin_service_track_id = request.service_track_id.clone();
                            qobuz_res.effective_service = Some("qobuz".to_string());
                            qobuz_res.effective_service_track_id =
                                Some(fallback_match.target_track_id.to_string());
                            qobuz_res.fallback_reason =
                                Some(format!("Tidal unavailable ({}) -> Qobuz fallback", err_msg));
                            qobuz_res.match_method = Some(fallback_match.match_method);
                            qobuz_res.match_confidence = Some(fallback_match.match_confidence);
                            Self::reconcile_physical_audio_quality(&mut qobuz_res, request);
                            PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                            return Ok(qobuz_res);
                        }
                    }

                    if request.strict_quality || !request.allow_fallback {
                        let rej_err = format!(
                                    "RejectedQuality: Qobuz fallback candidate quality '{:?}' is inferior to requested '{}' under strict policy",
                                    fallback_match.candidate_audio_quality, request.quality
                                );
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &rej_err));
                        return Err(anyhow!("{}", rej_err));
                    }

                    // FS-05: 320 kbps floor fallback
                    let mut tidal_320_req = request.clone();
                    tidal_320_req.quality = "HIGH".to_string();
                    tidal_320_req.allow_fallback = true;
                    if let Ok(mut tidal_res) =
                        Box::pin(self.tidal.download_track(&tidal_320_req, self.db.as_ref())).await
                    {
                        tidal_res.origin_service = Some("tidal".to_string());
                        tidal_res.effective_service = Some("tidal".to_string());
                        tidal_res.fallback_reason = Some(
                            "No service had lossless; degraded to 320 kbps on Tidal".to_string(),
                        );
                        Self::reconcile_physical_audio_quality(&mut tidal_res, request);
                        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                        return Ok(tidal_res);
                    }

                    let mut qobuz_320_req = request.clone();
                    qobuz_320_req.service_name = Some("qobuz".to_string());
                    qobuz_320_req.service_track_id =
                        Some(fallback_match.target_track_id.to_string());
                    qobuz_320_req.quality = "HIGH".to_string();
                    qobuz_320_req.allow_fallback = true;
                    let mut qobuz_res = self
                        .qobuz
                        .download_track(&qobuz_320_req, self.db.as_ref())
                        .await?;
                    qobuz_res.origin_service =
                        request.service_name.clone().or(Some("tidal".to_string()));
                    qobuz_res.origin_service_track_id = request.service_track_id.clone();
                    qobuz_res.effective_service = Some("qobuz".to_string());
                    qobuz_res.effective_service_track_id =
                        Some(fallback_match.target_track_id.to_string());
                    qobuz_res.fallback_reason =
                        Some("No service had lossless; degraded to 320 kbps on Qobuz".to_string());
                    qobuz_res.match_method = Some(fallback_match.match_method);
                    qobuz_res.match_confidence = Some(fallback_match.match_confidence);
                    Self::reconcile_physical_audio_quality(&mut qobuz_res, request);
                    PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                    Ok(qobuz_res)
                }
                Err(fallback_err) => {
                    if !request.strict_quality && request.allow_fallback {
                        let mut tidal_320_req = request.clone();
                        tidal_320_req.quality = "HIGH".to_string();
                        tidal_320_req.allow_fallback = true;
                        if let Ok(mut tidal_res) =
                            Box::pin(self.tidal.download_track(&tidal_320_req, self.db.as_ref()))
                                .await
                        {
                            tidal_res.origin_service = Some("tidal".to_string());
                            tidal_res.effective_service = Some("tidal".to_string());
                            tidal_res.fallback_reason =
                                Some("Degraded to 320 kbps on Tidal".to_string());
                            Self::reconcile_physical_audio_quality(&mut tidal_res, request);
                            PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                            return Ok(tidal_res);
                        }
                    }
                    PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &fallback_err));
                    Err(anyhow!("{}", fallback_err))
                }
            }
        })
    }

    fn download_songlink_routing<'a>(
        &'a self,
        request: &'a DownloadRequest,
        is_lossless_or_hires: bool,
        cancel_token: Option<&'a CancellationToken>,
        primary_service: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<DownloadResult>> + Send + 'a>> {
        Box::pin(async move {
            let item_id = &request.item_id;
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

                    // Pass 1: Try all candidates for requested lossless quality (do not degrade to lossy yet!)
                    for candidate in &candidates {
                        if let Some(token) = cancel_token {
                            if token.is_cancelled() {
                                PROGRESS_TRACKER.update(DownloadProgress::failed(
                                    item_id,
                                    "Download cancelled",
                                ));
                                return Err(anyhow!("Download cancelled by user"));
                            }
                        }

                        let mut cand_req = request.clone();
                        if is_lossless_or_hires {
                            cand_req.allow_fallback = false;
                        }

                        match candidate {
                            SongLinkEngineTarget::Tidal(tidal_id) => {
                                info!("[Orchestrator] Delegating SongLink match to native Tidal engine (Tidal ID: {})", tidal_id);
                                match Box::pin(self.download_tidal_track(&cand_req, tidal_id)).await
                                {
                                    Ok(res) => return Ok(res),
                                    Err(e) => {
                                        warn!("[Orchestrator] Native Tidal engine failed for SongLink match (ID {}): {}. Trying next candidate.", tidal_id, e);
                                        last_error = Some(e);
                                    }
                                }
                            }
                            SongLinkEngineTarget::Qobuz(qobuz_id) => {
                                info!("[Orchestrator] Delegating SongLink match to native Qobuz engine (Qobuz ID: {})", qobuz_id);
                                match Box::pin(self.download_qobuz_track(&cand_req, qobuz_id)).await
                                {
                                    Ok(res) => return Ok(res),
                                    Err(e) => {
                                        warn!("[Orchestrator] Native Qobuz engine failed for SongLink match (ID {}): {}. Trying next candidate.", qobuz_id, e);
                                        last_error = Some(e);
                                    }
                                }
                            }
                            SongLinkEngineTarget::Amazon(amazon_url) => {
                                if !is_lossless_or_hires {
                                    info!("[Orchestrator] Delegating SongLink match to Amazon fallback engine");
                                    match Box::pin(self.amazon.download_track(request, amazon_url))
                                        .await
                                    {
                                        Ok(mut res) => {
                                            Self::reconcile_physical_audio_quality(
                                                &mut res, request,
                                            );
                                            PROGRESS_TRACKER
                                                .update(DownloadProgress::complete(item_id));
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
                    }

                    // Pass 2: If none of the candidates had requested lossless quality,
                    // and allow_fallback is enabled (and not strict), fall back to 320 kbps (never below!)
                    if is_lossless_or_hires && request.allow_fallback && !request.strict_quality {
                        info!("[Orchestrator] No service had lossless for SongLink match; entering 320 kbps fallback pass");
                        for candidate in &candidates {
                            let mut lossy_req = request.clone();
                            lossy_req.quality = "HIGH".to_string(); // 320 kbps floor
                            lossy_req.allow_fallback = true;

                            match candidate {
                                SongLinkEngineTarget::Tidal(tidal_id) => {
                                    if let Ok(res) =
                                        Box::pin(self.download_tidal_track(&lossy_req, tidal_id))
                                            .await
                                    {
                                        return Ok(res);
                                    }
                                }
                                SongLinkEngineTarget::Qobuz(qobuz_id) => {
                                    if let Ok(res) =
                                        Box::pin(self.download_qobuz_track(&lossy_req, qobuz_id))
                                            .await
                                    {
                                        return Ok(res);
                                    }
                                }
                                SongLinkEngineTarget::Amazon(amazon_url) => {
                                    if let Ok(mut res) =
                                        Box::pin(self.amazon.download_track(&lossy_req, amazon_url))
                                            .await
                                    {
                                        Self::reconcile_physical_audio_quality(&mut res, request);
                                        PROGRESS_TRACKER
                                            .update(DownloadProgress::complete(item_id));
                                        return Ok(res);
                                    }
                                }
                            }
                        }
                    } else if is_lossless_or_hires
                        && (request.strict_quality || !request.allow_fallback)
                    {
                        let rej_err = format!("RejectedQuality: Requested quality '{}' not available across services under strict policy", request.quality);
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &rej_err));
                        return Err(anyhow!("{}", rej_err));
                    }

                    if let Some(err) = last_error {
                        PROGRESS_TRACKER
                            .update(DownloadProgress::failed(item_id, &err.to_string()));
                        return Err(err);
                    }
                }
                Err(e) => {
                    warn!(
                        "[Orchestrator] SongLink query failed for track '{}': {}",
                        request.track_name, e
                    );
                    // If service was directly Amazon with a direct URL in service_track_id, try Amazon
                    if primary_service == "amazon" {
                        if let Some(ref track_url) = request.service_track_id {
                            if track_url.starts_with("http://") || track_url.starts_with("https://")
                            {
                                let mut res =
                                    self.amazon.download_track(request, track_url).await?;
                                Self::reconcile_physical_audio_quality(&mut res, request);
                                PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                                return Ok(res);
                            }
                        }
                    }
                }
            }

            PROGRESS_TRACKER.update(DownloadProgress::failed(
                item_id,
                &format!("Unsupported or unavailable service: {}", primary_service),
            ));
            Err(anyhow!(
                "Unsupported or unavailable service: {}",
                primary_service
            ))
        })
    }

    /// Check whether a service has active, valid credentials in the database
    ///
    /// A-4: el filtro de `credentials_invalid` sigue intacto — un rechazo
    /// recién ocurrido bloquea igual que antes. Lo que se añade es la
    /// autocuración por antigüedad: un veredicto VIEJO (más de
    /// `AUTH_LATCH_MAX_AGE`) deja de deny-by-default para que la descarga
    /// vuelva a intentarlo y sea su propio 401 el que decida, vía
    /// `classify_session_auth_failure`. Sin esto, un solo rechazo bloqueaba el
    /// servicio de forma permanente: los únicos que limpian el flag son un
    /// login o un logout manuales.
    pub async fn is_service_available(&self, service: &str) -> bool {
        if let Some(ref db) = self.db {
            // Traemos el flag JUNTO con su edad en una sola fila por servicio.
            let rows: Result<Vec<(i64, Option<String>)>, _> = sqlx::query_as(
                r#"
                SELECT COALESCE(a.credentials_invalid, 0), a.last_auth_error_at
                FROM accounts a
                JOIN services s ON s.id = a.service_id
                WHERE LOWER(s.name) = LOWER(?)
                  AND a.is_active = 1
                "#,
            )
            .bind(service)
            .fetch_all(db)
            .await;

            match rows {
                Ok(rows) => rows.iter().any(|(invalid, marked_at)| {
                    if *invalid == 0 {
                        return true;
                    }
                    !auth_latch_is_stale(marked_at.as_deref(), chrono::Utc::now())
                }),
                // Sin lectura fiable se mantiene el `true` previo: es el
                // comportamiento de antes y no invalida nada por edad.
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
    ) -> Result<(
        Vec<SongLinkEngineTarget>,
        crate::download::songlink::SongLinkAvailability,
    )> {
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

    /// Parse, resolve metadata via SongLink/native services, and enqueue track into download_queue
    #[allow(dead_code)] // Cubierta por `tests/url_import_queue_test.rs` (`test_orchestrator_enqueue_from_url_integration`).
    pub async fn enqueue_from_url(
        &self,
        db: &sqlx::SqlitePool,
        url: &str,
    ) -> Result<crate::commands::ParsedUrl, String> {
        crate::commands::url_import::perform_import_from_url(db, Some(self), url).await
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

        let mut tidal_res =
            Box::pin(self.tidal.download_track(&tidal_req, self.db.as_ref())).await?;
        tidal_res.origin_service = request
            .service_name
            .clone()
            .or_else(|| Some("spotify".to_string()));
        tidal_res.origin_service_track_id = request
            .service_track_id
            .clone()
            .or_else(|| request.spotify_id.clone());
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

        let mut qobuz_res = self
            .qobuz
            .download_track(&qobuz_req, self.db.as_ref())
            .await?;
        qobuz_res.origin_service = request
            .service_name
            .clone()
            .or_else(|| Some("spotify".to_string()));
        qobuz_res.origin_service_track_id = request
            .service_track_id
            .clone()
            .or_else(|| request.spotify_id.clone());
        qobuz_res.effective_service = Some("qobuz".to_string());
        qobuz_res.effective_service_track_id = Some(qobuz_id.to_string());
        qobuz_res.fallback_reason = Some("SongLink cross-platform match".to_string());
        qobuz_res.match_method = Some("songlink_cross_platform".to_string());
        qobuz_res.match_confidence = Some(1.0);
        Self::reconcile_physical_audio_quality(&mut qobuz_res, request);
        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
        Ok(qobuz_res)
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
    fn preview_fallback_diagnostics_do_not_claim_qobuz_auth_failure() {
        assert_eq!(
            DownloadOrchestrator::preview_fallback_diagnostic("RequiresAuth: Tidal HTTP 401"),
            "Tidal account could not serve the candidate"
        );
        assert_eq!(
            DownloadOrchestrator::preview_fallback_diagnostic(
                "SourceIdentityMissing: no Tidal edition"
            ),
            "no confirmed equivalent Tidal edition found"
        );
    }

    #[tokio::test]
    async fn preview_with_ambiguous_tidal_identity_keeps_original_error() {
        use sqlx::sqlite::SqlitePoolOptions;
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY, isrc TEXT)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE track_sources (track_id INTEGER, service_id INTEGER, service_track_id TEXT, format TEXT, available INTEGER)")
            .execute(&db).await.unwrap();
        sqlx::query("INSERT INTO services VALUES (3, 'tidal')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tracks VALUES (1, 'TESTISRC123'), (2, 'TESTISRC123')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO track_sources VALUES (1, 3, '111', 'FLAC', 1), (2, 3, '222', 'FLAC', 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        let orchestrator = DownloadOrchestrator::new().with_db(db);
        let request = DownloadRequest {
            item_id: "preview-ambiguous".to_string(),
            isrc: Some("TESTISRC123".to_string()),
            service_name: Some("qobuz".to_string()),
            service_track_id: Some("999".to_string()),
            quality: "LOSSLESS".to_string(),
            allow_fallback: true,
            ..Default::default()
        };
        let err = orchestrator
            .handle_qobuz_fallback(
                &request,
                anyhow!("QobuzPreviewPayload: Qobuz delivered MP3 instead of FLAC format_id=6"),
                true,
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("QobuzPreviewPayload:"));
        assert!(err.contains("multiple Tidal editions"));
        assert!(!err.contains("RequiresAuth"));
    }

    #[tokio::test]
    async fn preview_without_fallback_keeps_named_error_not_auth() {
        let orchestrator = DownloadOrchestrator::new();
        let request = DownloadRequest {
            item_id: "preview-no-fallback".to_string(),
            quality: "LOSSLESS".to_string(),
            service_name: Some("qobuz".to_string()),
            allow_fallback: false,
            ..Default::default()
        };
        let err = orchestrator
            .handle_qobuz_fallback(
                &request,
                anyhow!("QobuzPreviewPayload: Qobuz delivered MP3 instead of FLAC format_id=6"),
                true,
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("QobuzPreviewPayload:"));
        assert!(!err.contains("RequiresAuth"));
        assert!(!err.contains("StaleSource"));
    }

    /// A-4: un rechazo RECIÉN ocurrido debe seguir bloqueando el servicio.
    /// Este es el comportamiento que no se toca, y falla contra cualquier
    /// versión que relaje el latch sin mirar la antigüedad.
    #[test]
    fn a_fresh_rejection_still_locks_the_service() {
        let now = chrono::Utc::now();
        let fresh = (now - chrono::Duration::minutes(5)).to_rfc3339();
        assert!(
            !auth_latch_is_stale(Some(&fresh), now),
            "un veredicto de hace 5 minutos no puede considerarse viejo"
        );
        // Justo por debajo del umbral tampoco caduca.
        let edge = (now - chrono::Duration::hours(AUTH_LATCH_MAX_AGE_HOURS - 1)).to_rfc3339();
        assert!(!auth_latch_is_stale(Some(&edge), now));
    }

    /// A-4: pasado el umbral, el latch deja de bloquear para siempre. Este
    /// aserto falla contra la versión previa, que no miraba la edad en absoluto.
    #[test]
    fn an_old_rejection_decays_so_the_service_can_retry() {
        let now = chrono::Utc::now();
        let old = (now - chrono::Duration::hours(AUTH_LATCH_MAX_AGE_HOURS + 1)).to_rfc3339();
        assert!(
            auth_latch_is_stale(Some(&old), now),
            "un veredicto de hace más de {} h debe autocurarse",
            AUTH_LATCH_MAX_AGE_HOURS
        );
        let ancient = (now - chrono::Duration::days(30)).to_rfc3339();
        assert!(auth_latch_is_stale(Some(&ancient), now));
    }

    /// Ante duda se conserva el deny-by-default: si no consta la fecha, el
    /// veredicto se trata como reciente (o sea, bloqueante).
    #[test]
    fn an_unknown_marking_time_keeps_the_strict_behavior() {
        let now = chrono::Utc::now();
        assert!(!auth_latch_is_stale(None, now), "sin fecha no se relaja");
        assert!(
            !auth_latch_is_stale(Some(""), now),
            "fecha vacía no se relaja"
        );
        assert!(
            !auth_latch_is_stale(Some("no-es-una-fecha"), now),
            "una fecha ilegible no se relaja"
        );
    }

    /// Un sello en el futuro no es evidencia de antigüedad (desfase de reloj).
    #[test]
    fn a_future_timestamp_is_not_treated_as_stale() {
        let now = chrono::Utc::now();
        let future = (now + chrono::Duration::days(2)).to_rfc3339();
        assert!(!auth_latch_is_stale(Some(&future), now));
    }

    /// El formato es el que escribe `mark_account_credentials_invalid`
    /// (`chrono::Utc::now().to_rfc3339()`), no otro: un desfase de formato
    /// haría que TODO latch pareciera sin fecha y la autocuración no
    /// ocurriría nunca.
    #[test]
    fn the_written_format_is_understood() {
        let now = chrono::Utc::now();
        let as_written = chrono::Utc::now().to_rfc3339();
        assert!(!auth_latch_is_stale(Some(&as_written), now));
        assert!(auth_latch_is_stale(
            Some(&as_written),
            now + chrono::Duration::hours(25)
        ));
    }

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
        let orchestrator = DownloadOrchestrator::new().with_priority(vec!["tidal".to_string()]);
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
        let orchestrator = DownloadOrchestrator::new().with_priority(vec!["qobuz".to_string()]);
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

        let result = orchestrator
            .download_track_cancellable(&req, Some(&cancel_token))
            .await;
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
        let analysis = orchestrator
            .analyze_staging_audio(&flac_path)
            .await
            .unwrap();

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

        let enriched = orchestrator
            .enrich_staging_audio(&flac_path, &req, None)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn test_orchestrator_propagates_database_error_in_fallback() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to connect to SQLite in-memory");
        // No tables created in this pool, so query will fail with SQLite error (no such table: track_sources)
        let orchestrator = DownloadOrchestrator::new().with_db(pool);
        let req = DownloadRequest {
            item_id: "test_db_err".to_string(),
            isrc: Some("USUG12101234".to_string()),
            track_name: "Test Track".to_string(),
            artist_name: "Test Artist".to_string(),
            quality: "LOSSLESS".to_string(),
            allow_fallback: true,
            ..Default::default()
        };
        let res = orchestrator.resolve_edition_identity_fallback(&req).await;
        assert!(res.is_err(), "Must fail due to database query error");
        let err = res.unwrap_err();
        assert!(
            err.starts_with("DatabaseError:"),
            "Database error must be propagated as DatabaseError, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_crash_stack_future_size_is_bounded_by_heap_boxing() {
        let orchestrator = DownloadOrchestrator::new();
        let req = DownloadRequest {
            item_id: "test_size".to_string(),
            track_name: "Size Test".to_string(),
            artist_name: "Artist".to_string(),
            quality: "LOSSLESS".to_string(),
            ..Default::default()
        };
        let fut = orchestrator.download_track(&req);
        let future_size = std::mem::size_of_val(&fut);
        // Sin `Box::pin`, este future excedía 51 KB in-line. Con `Box::pin`,
        // la máquina de estados reside en el heap y el future externo mide < 512 bytes.
        assert!(
            future_size < 1024,
            "Future size must be bounded below 1 KB due to heap boxing; got {} bytes",
            future_size
        );
    }

    #[test]
    fn test_crash_stack_runs_safely_on_constrained_thread_stack() {
        // Regresión CRASH-STACK: ejecución en hilo con pila estrictamente restringida (256 KB).
        // Si existiese desbordamiento por futures no boxeados o recursión infinita en el
        // pipeline de descarga, el hilo abortaría inmediatamente con stack overflow.
        let child = std::thread::Builder::new()
            .name("test-constrained-stack".to_string())
            .stack_size(256 * 1024)
            .spawn(|| {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build tokio runtime");

                rt.block_on(async {
                    let orchestrator = DownloadOrchestrator::new();
                    let req = DownloadRequest {
                        item_id: "test_constrained".to_string(),
                        service_name: Some("qobuz".to_string()),
                        track_name: "Stack Test".to_string(),
                        artist_name: "Artist".to_string(),
                        quality: "LOSSLESS".to_string(),
                        allow_fallback: false,
                        ..Default::default()
                    };
                    let res = orchestrator.download_track(&req).await;
                    assert!(res.is_err(), "Must return error, not overflow stack");
                });
            })
            .expect("Failed to spawn thread with constrained stack");

        child
            .join()
            .expect("Thread with 256 KB stack panicked or overflowed!");
    }
}
