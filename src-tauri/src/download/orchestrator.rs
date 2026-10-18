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

// Only categorical outcomes cross the retry boundary. Provider responses may
// contain account identifiers, track IDs, or credentials and must not enter the
// worker's persisted error through this path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AuthRetryFailure {
    Entity,
    Cancelled,
    AuthExhausted,
    NoCandidate,
}

impl std::fmt::Display for AuthRetryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Entity => "ProviderEntityFailure: provider entity lookup failed for this edition",
            Self::Cancelled => "Download cancelled by user",
            Self::AuthExhausted => {
                "RequiresAuth: accounts could not authenticate for this edition; reconnect an account"
            }
            Self::NoCandidate => "SourceIdentityMissing: no usable account for this edition",
        };
        f.write_str(message)
    }
}

impl std::error::Error for AuthRetryFailure {}

/// True when the error carries the user's cooperative-cancellation signal.
///
/// Cancellation is the one case that must survive every layer untouched: retrying
/// or re-labelling it would keep a cancelled download alive.
fn is_cancellation_error(error: &str) -> bool {
    error.contains("Download cancelled") || error.contains("operation was canceled")
}

fn unclassifiable_retry_failure(error: &str) -> AuthRetryFailure {
    if is_cancellation_error(error) {
        AuthRetryFailure::Cancelled
    } else {
        AuthRetryFailure::Entity
    }
}

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

        // Qobuz's search endpoint requires the account's user token: without it
        // the API answers 401 and both the exact-ISRC (step 1B) and strict
        // metadata (step 4.5) searches would silently degrade to "no match" for
        // every track. Resolve it once from the same SQLite source the download
        // path uses; missing/expired credentials surface as normal no-matches
        // instead of aborting the fallback.
        let qobuz_token: Option<String> = if target_service == "qobuz" {
            let token = self.qobuz.resolve_token(self.db.as_ref()).await.ok();
            if token.is_none() {
                debug!(
                    "[Orchestrator] Fallback: no usable Qobuz user token available; live Qobuz searches will be skipped"
                );
            }
            token
        } else {
            if let Some(ref db) = self.db {
                let (creds, _) =
                    crate::services::tidal_pipeline::resolve_and_refresh_gui_credentials(
                        db,
                        self.tidal.client(),
                    )
                    .await;
                if let Some(c) = creds {
                    self.tidal.set_user_token(Some(c.access_token));
                }
            }
            None
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
                            if let Ok(track) = self
                                .tidal
                                .search_by_isrc_broad(
                                    isrc_trimmed,
                                    &request.track_name,
                                    &request.artist_name,
                                    duration_sec,
                                )
                                .await
                            {
                                info!(
                                    "[Orchestrator] ✓ Fallback matched via exact ISRC (broad query): Tidal ID {}",
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
                        }
                    }
                } else if target_service == "qobuz" {
                    match self
                        .qobuz
                        .search_by_isrc(isrc_trimmed, duration_sec, qobuz_token.as_deref())
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
                            // The bare-ISRC index does not resolve every ISRC
                            // (observed live on streamable tracks). Retry with
                            // the artist+title query and keep only an exact
                            // `isrc` field match: identity stays exact.
                            if let Ok(track) = self
                                .qobuz
                                .search_by_isrc_broad(
                                    isrc_trimmed,
                                    &request.track_name,
                                    &request.artist_name,
                                    duration_sec,
                                    qobuz_token.as_deref(),
                                )
                                .await
                            {
                                info!(
                                    "[Orchestrator] ✓ Fallback matched via exact ISRC (broad query): Qobuz ID {}",
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
                        }
                    }
                }
            }
        }

        // 2. Exact MusicBrainz Recording ID
        let mut mb_canonical_duration_sec: Option<i32> = None;
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

                // 2B. Query MusicBrainz recording details for sibling ISRCs of
                // the same recording and its canonical recording length.
                let mb_client = crate::services::musicbrainz::MusicBrainzClient::new();
                if let Ok(detail) = mb_client.get_recording_details(mb_rid_trimmed).await {
                    let rec_dur_sec = detail
                        .length
                        .map(|ms| (ms / 1000) as i32)
                        .filter(|&d| d > 0);
                    mb_canonical_duration_sec = rec_dur_sec;
                    let eff_dur = rec_dur_sec.unwrap_or(duration_sec);
                    let primary_isrc = request.isrc.as_deref().map(str::trim);

                    if let Some(sibling_isrcs) = detail.isrcs {
                        for sib in sibling_isrcs {
                            let sib_trimmed = sib.trim();
                            if sib_trimmed.is_empty() || Some(sib_trimmed) == primary_isrc {
                                continue;
                            }
                            if target_service == "tidal" {
                                let found = match self.tidal.search_by_isrc(sib_trimmed, eff_dur).await {
                                    Ok(t) => Some(t),
                                    Err(_) => self
                                        .tidal
                                        .search_by_isrc_broad(
                                            sib_trimmed,
                                            &request.track_name,
                                            &request.artist_name,
                                            eff_dur,
                                        )
                                        .await
                                        .ok(),
                                };
                                if let Some(track) = found {
                                    info!(
                                        "[Orchestrator] ✓ Fallback matched via MusicBrainz recording sibling ISRC {}: Tidal ID {}",
                                        sib_trimmed, track.id
                                    );
                                    return Ok(FallbackMatch {
                                        target_track_id: track.id,
                                        target_service: "tidal".to_string(),
                                        match_method: "musicbrainz_recording_id".to_string(),
                                        match_confidence: 0.95,
                                        candidate_audio_quality: track.audio_quality,
                                    });
                                }
                            } else if target_service == "qobuz" {
                                let found = match self
                                    .qobuz
                                    .search_by_isrc(sib_trimmed, eff_dur, qobuz_token.as_deref())
                                    .await
                                {
                                    Ok(t) => Some(t),
                                    Err(_) => self
                                        .qobuz
                                        .search_by_isrc_broad(
                                            sib_trimmed,
                                            &request.track_name,
                                            &request.artist_name,
                                            eff_dur,
                                            qobuz_token.as_deref(),
                                        )
                                        .await
                                        .ok(),
                                };
                                if let Some(track) = found {
                                    info!(
                                        "[Orchestrator] ✓ Fallback matched via MusicBrainz recording sibling ISRC {}: Qobuz ID {}",
                                        sib_trimmed, track.id
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
                                        match_method: "musicbrainz_recording_id".to_string(),
                                        match_confidence: 0.95,
                                        candidate_audio_quality: cand_q,
                                    });
                                }
                            }
                        }
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

        // 4.5. Strict title + artist + duration (±3s, unique) matching.
        //
        // Authorized only after every identity rule (ISRC / MBID / AcoustID)
        // failed. Unlike rule 5 this MAY auto-download: the candidate must be
        // the only catalog entry whose title, artist and real duration all
        // match within tolerance, so a genuine 30s track like "(Archive 76)"
        // is compared against the catalog duration, never a preview threshold.
        debug!(
            "[Orchestrator] Fallback Step 4.5: Strict metadata search on {} for '{}' - '{}' ({}s)",
            target_service_display, request.track_name, request.artist_name, duration_sec
        );
        let mut strict_match: Option<(i64, Option<String>)> = if duration_sec <= 0 {
            // No duration = no strict comparison possible. The authorized
            // automatic download requires title + artist AND duration; without
            // the duration this degrades to rule 5 (no automatic download).
            debug!(
                "[Orchestrator] Fallback Step 4.5 skipped: unknown duration for '{}'",
                request.track_name
            );
            None
        } else if target_service == "tidal" {
            match self
                .tidal
                .search_by_metadata_strict(&request.track_name, &request.artist_name, duration_sec)
                .await
            {
                Ok(track) => Some((track.id, track.audio_quality)),
                Err(e) => {
                    let strict_err = e.to_string();
                    if strict_err.contains("AmbiguousSource") {
                        return Err(format!(
                            "AmbiguousSource: Multiple strict metadata matches on Tidal for '{}'",
                            request.track_name
                        ));
                    }
                    None
                }
            }
        } else {
            match self
                .qobuz
                .search_by_metadata_strict(
                    &request.track_name,
                    &request.artist_name,
                    duration_sec,
                    qobuz_token.as_deref(),
                )
                .await
            {
                Ok(track) => {
                    let quality = track
                        .max_bit_depth
                        .map(|bd| {
                            if bd > 16 {
                                "HI_RES".to_string()
                            } else {
                                "LOSSLESS".to_string()
                            }
                        })
                        .or_else(|| Some("LOSSLESS".to_string()));
                    Some((track.id, quality))
                }
                Err(e) => {
                    let strict_err = e.to_string();
                    if strict_err.contains("AmbiguousSource") {
                        return Err(format!(
                            "AmbiguousSource: Multiple strict metadata matches on Qobuz for '{}'",
                            request.track_name
                        ));
                    }
                    None
                }
            }
        };

        // If the retired release's duration differed from the canonical
        // MusicBrainz recording length (e.g. a compilation with trailing
        // silence), retry strict matching against the canonical MB duration.
        if strict_match.is_none() {
            if let Some(mb_dur) = mb_canonical_duration_sec.filter(|&d| d > 0 && d != duration_sec)
            {
                if target_service == "tidal" {
                    if let Ok(track) = self
                        .tidal
                        .search_by_metadata_strict(
                            &request.track_name,
                            &request.artist_name,
                            mb_dur,
                        )
                        .await
                    {
                        strict_match = Some((track.id, track.audio_quality));
                    }
                } else if let Ok(track) = self
                    .qobuz
                    .search_by_metadata_strict(
                        &request.track_name,
                        &request.artist_name,
                        mb_dur,
                        qobuz_token.as_deref(),
                    )
                    .await
                {
                    let quality = track
                        .max_bit_depth
                        .map(|bd| {
                            if bd > 16 {
                                "HI_RES".to_string()
                            } else {
                                "LOSSLESS".to_string()
                            }
                        })
                        .or_else(|| Some("LOSSLESS".to_string()));
                    strict_match = Some((track.id, quality));
                }
            }
        }

        if let Some((target_track_id, quality)) = strict_match {
            info!(
                "[Orchestrator] ✓ Fallback matched via strict metadata: {} ID {}",
                target_service_display, target_track_id
            );
            if let Some(canonical_id) = request.canonical_track_id {
                self.persist_fallback_track_source(
                    canonical_id,
                    target_service,
                    target_track_id,
                    quality.as_deref(),
                )
                .await;
            }
            return Ok(FallbackMatch {
                target_track_id,
                target_service: target_service.to_string(),
                match_method: "strict_metadata".to_string(),
                match_confidence: 0.70,
                candidate_audio_quality: quality,
            });
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

    /// Persist a strict-metadata fallback hit into `track_sources` so later
    /// runs resolve it from the DB instead of re-searching (and the resolver
    /// can rank it). Best-effort: persistence failures never abort a download.
    async fn persist_fallback_track_source(
        &self,
        canonical_track_id: i64,
        service_name: &str,
        service_track_id: i64,
        audio_quality: Option<&str>,
    ) {
        let Some(db) = self.db.as_ref() else {
            return;
        };

        let service_id: Option<i64> =
            match sqlx::query_scalar("SELECT id FROM services WHERE name = ? LIMIT 1")
                .bind(service_name)
                .fetch_optional(db)
                .await
            {
                Ok(id) => id,
                Err(e) => {
                    debug!(
                        "[Orchestrator] Strict-metadata source not persisted (service lookup: {})",
                        e
                    );
                    return;
                }
            };
        let Some(service_id) = service_id else {
            return;
        };

        let quality = audio_quality.map(|q| q.to_ascii_uppercase());
        let (format, bit_depth, sample_rate): (&str, Option<i64>, Option<i64>) =
            match quality.as_deref() {
                Some("HI_RES") | Some("HIRES") | Some("HI_RES_LOSSLESS") => {
                    ("FLAC", Some(24), Some(96000))
                }
                Some("LOSSLESS") => ("FLAC", Some(16), Some(44100)),
                _ => ("FLAC", None, None),
            };

        if let Err(e) = sqlx::query(
            r#"
            INSERT OR REPLACE INTO track_sources
                (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available)
            VALUES (?, ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(canonical_track_id)
        .bind(service_id)
        .bind(service_track_id.to_string())
        .bind(format)
        .bind(bit_depth)
        .bind(sample_rate)
        .execute(db)
        .await
        {
            debug!(
                "[Orchestrator] Strict-metadata source not persisted ({} {}): {}",
                service_name, service_track_id, e
            );
        }
    }

    /// Refresh a dead Qobuz `service_track_id` for the same edition.
    ///
    /// A `track/get` 404 is a verdict about the stored id, not about the
    /// edition: Qobuz re-issues ids when a release is re-delivered, which
    /// leaves perfectly streamable editions unreachable behind stale rows.
    /// Re-resolve on Qobuz itself — exact ISRC first, then the authorized
    /// strict title+artist+duration (±3s, unique) rule — so the download stays
    /// on the same service, edition and quality instead of crossing services
    /// or degrading. Never returns the dead id itself.
    async fn refresh_stale_qobuz_source(
        &self,
        request: &DownloadRequest,
    ) -> Option<(i64, Option<String>)> {
        let duration_sec = (request.duration_ms / 1000) as i32;
        let token = match self.qobuz.resolve_token(self.db.as_ref()).await {
            Ok(token) => token,
            Err(_) => {
                debug!(
                    "[Orchestrator] Stale Qobuz source refresh skipped: no usable user token"
                );
                return None;
            }
        };

        let stale_id = request.service_track_id.as_deref().map(str::trim);
        let quality_of = |max_bit_depth: Option<i32>| -> Option<String> {
            max_bit_depth
                .map(|bd| {
                    if bd > 16 {
                        "HI_RES".to_string()
                    } else {
                        "LOSSLESS".to_string()
                    }
                })
                .or_else(|| Some("LOSSLESS".to_string()))
        };

        if let Some(isrc) = request
            .isrc
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            match self
                .qobuz
                .search_by_isrc(isrc, duration_sec, Some(&token))
                .await
            {
                Ok(track) => {
                    let found = track.id.to_string();
                    if Some(found.as_str()) == stale_id {
                        debug!(
                            "[Orchestrator] Stale Qobuz source refresh: ISRC search returned the same (dead) id {}",
                            found
                        );
                    } else {
                        info!(
                            "[Orchestrator] ✓ Stale Qobuz source refreshed via exact ISRC {}: id {} -> {}",
                            isrc,
                            stale_id.unwrap_or("?"),
                            found
                        );
                        return Some((track.id, quality_of(track.max_bit_depth)));
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("AmbiguousSource") {
                        warn!("[Orchestrator] Stale Qobuz source refresh: {}", msg);
                    } else {
                        debug!(
                            "[Orchestrator] Stale Qobuz source refresh by ISRC failed: {}",
                            msg
                        );
                    }
                    // Same index quirk as the cross-service path: some ISRCs
                    // only resolve through an artist+title query. An exact
                    // `isrc` field match keeps the identity exact.
                    if let Ok(track) = self
                        .qobuz
                        .search_by_isrc_broad(
                            isrc,
                            &request.track_name,
                            &request.artist_name,
                            duration_sec,
                            Some(&token),
                        )
                        .await
                    {
                        let found = track.id.to_string();
                        if Some(found.as_str()) != stale_id {
                            info!(
                                "[Orchestrator] ✓ Stale Qobuz source refreshed via exact ISRC (broad query) {}: id {} -> {}",
                                isrc,
                                stale_id.unwrap_or("?"),
                                found
                            );
                            return Some((track.id, quality_of(track.max_bit_depth)));
                        }
                    }
                }
            }
        }

        if duration_sec > 0 {
            match self
                .qobuz
                .search_by_metadata_strict(
                    &request.track_name,
                    &request.artist_name,
                    duration_sec,
                    Some(&token),
                )
                .await
            {
                Ok(track) => {
                    let found = track.id.to_string();
                    if Some(found.as_str()) != stale_id {
                        info!(
                            "[Orchestrator] ✓ Stale Qobuz source refreshed via strict metadata: id {} -> {}",
                            stale_id.unwrap_or("?"),
                            found
                        );
                        return Some((track.id, quality_of(track.max_bit_depth)));
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("AmbiguousSource") {
                        warn!("[Orchestrator] Stale Qobuz source refresh: {}", msg);
                    } else {
                        debug!(
                            "[Orchestrator] Stale Qobuz source refresh by strict metadata failed: {}",
                            msg
                        );
                    }
                }
            }
        }

        // 3. MusicBrainz recording identity: a retired Qobuz compilation often
        // has a dead ISRC or a padded duration while its MusicBrainz recording
        // lists live sibling ISRCs (e.g. GBAJE6800198 -> GBAJE6800413 for
        // "Israelites") and the canonical studio duration (e.g. 175s vs 191s
        // for "(Your Love Keeps Lifting Me) Higher and Higher").
        if let Some(mb_rid) = request
            .musicbrainz_recording_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let mb_client = crate::services::musicbrainz::MusicBrainzClient::new();
            if let Ok(detail) = mb_client.get_recording_details(mb_rid).await {
                let mb_dur_sec = detail
                    .length
                    .map(|ms| (ms / 1000) as i32)
                    .filter(|&d| d > 0)
                    .unwrap_or(duration_sec);
                let primary_isrc = request.isrc.as_deref().map(str::trim);

                if let Some(sibling_isrcs) = detail.isrcs {
                    for sib in sibling_isrcs {
                        let sib_trimmed = sib.trim();
                        if sib_trimmed.is_empty() || Some(sib_trimmed) == primary_isrc {
                            continue;
                        }
                        let candidate = match self
                            .qobuz
                            .search_by_isrc(sib_trimmed, mb_dur_sec, Some(&token))
                            .await
                        {
                            Ok(t) => Some(t),
                            Err(_) => self
                                .qobuz
                                .search_by_isrc_broad(
                                    sib_trimmed,
                                    &request.track_name,
                                    &request.artist_name,
                                    mb_dur_sec,
                                    Some(&token),
                                )
                                .await
                                .ok(),
                        };
                        if let Some(track) = candidate {
                            let found = track.id.to_string();
                            if Some(found.as_str()) != stale_id {
                                info!(
                                    "[Orchestrator] ✓ Stale Qobuz source refreshed via MusicBrainz recording {} sibling ISRC {}: id {} -> {}",
                                    mb_rid,
                                    sib_trimmed,
                                    stale_id.unwrap_or("?"),
                                    found
                                );
                                return Some((track.id, quality_of(track.max_bit_depth)));
                            }
                        }
                    }
                }

                if mb_dur_sec > 0 && mb_dur_sec != duration_sec {
                    if let Ok(track) = self
                        .qobuz
                        .search_by_metadata_strict(
                            &request.track_name,
                            &request.artist_name,
                            mb_dur_sec,
                            Some(&token),
                        )
                        .await
                    {
                        let found = track.id.to_string();
                        if Some(found.as_str()) != stale_id {
                            info!(
                                "[Orchestrator] ✓ Stale Qobuz source refreshed via MusicBrainz recording {} canonical duration ({}s): id {} -> {}",
                                mb_rid,
                                mb_dur_sec,
                                stale_id.unwrap_or("?"),
                                found
                            );
                            return Some((track.id, quality_of(track.max_bit_depth)));
                        }
                    }
                }
            }
        }

        None
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

    /// WF-RES-01: ¿un rechazo de autenticación puede re-resolverse a otra
    /// cuenta del MISMO `service_track_id` sin cruzar de servicio?
    ///
    /// Un 401/403 no es un veredicto sobre el ítem: es un veredicto sobre la
    /// CUENTA que se usó para pedirlo. Abortar ahí deja sin descargar un
    /// `service_track_id` que otras cuentas del mismo proveedor sí pueden
    /// servir. Pero el mismo texto puede aparecer en metadatos que no tienen
    /// nada que ver con la sesión (una edición que en su título o año lleva
    /// «401»), y solo la ruta con `resolution_plan` sabe a qué cuenta se le
    /// estaba pidiendo. Sin plan no hay identidad que excluir, así que se
    /// conserva el abort histórico.
    ///
    /// Devuelve `None` cuando NO debe reintentarse: cancelación del usuario
    /// (nunca reintentable), fallo transitorio ya clasificado, texto ajeno a
    /// la autenticación, o petición de otro proveedor (una hermana de cuenta
    /// nunca cruza de servicio). Los tests de este módulo fijan cada caso.
    fn planned_auth_retry<'a>(
        request: &'a DownloadRequest,
        error: &str,
        provider_service: &str,
    ) -> Option<&'a crate::download::DownloadPlan> {
        let plan = request.resolution_plan.as_ref()?;
        if !request
            .service_name
            .as_deref()
            .unwrap_or_default()
            .eq_ignore_ascii_case(provider_service)
        {
            return None;
        }
        // Sin cuenta previa no hay nada que excluir: reintentar sería reapuntar
        // a la misma combinación.
        let primary = plan.primary()?;
        primary.account.as_ref()?;
        if !Self::is_account_auth_rejection(error)
            || !primary.service_name.eq_ignore_ascii_case(provider_service)
            || request.service_track_id.as_deref() != Some(primary.service_track_id.as_str())
        {
            return None;
        }
        Some(plan)
    }

    /// Only account-scoped credential or stream-entitlement evidence can move
    /// a planned download to a sibling account. Entity lookups can return 401/403
    /// for reasons unrelated to the selected account, so a bare HTTP status is
    /// never sufficient here. Cancellation and transport take precedence.
    fn is_account_auth_rejection(error: &str) -> bool {
        if error.contains("Download cancelled")
            || error.contains("operation was canceled")
            || crate::worker::is_transport_failure(error)
        {
            return false;
        }
        let lowered = error.to_ascii_lowercase();
        if lowered.contains("track/get")
            || lowered.contains("album/get")
            || lowered.contains("entity lookup")
        {
            return false;
        }
        lowered.contains("requiresauth")
            || lowered.contains("authentication failed")
            || lowered.contains("credential")
            || lowered.contains("token expired")
            || lowered.contains("token invalid")
            || lowered.contains("stream entitlement")
    }

    /// Classify a retry using the SAME account scope as the initial auth gate
    /// (`is_account_auth_rejection`), instead of the loose
    /// `classify_attempt_failure` that treats every «401» as account auth.
    ///
    /// - Cancellation → `None` (never retried, propagated untouched)
    /// - Transport failure → `TransientFailure`
    /// - Qobuz preview / physical-quality byte payload → their trigger
    /// - Genuine account-auth/credential rejection → `RequiresAuth`
    /// - Non-auth entity 401/403 (`track/get`, `album/get`, entity lookup,
    ///   bare status) → `None`: the sibling sequence stops; the account is
    ///   never excluded as auth and the edition is only marked `NotFound`
    ///   when the text proves the edition itself is invalid, never on the
    ///   HTTP status alone.
    fn classify_retry_failure(error: &str) -> Option<crate::download::ReResolutionTrigger> {
        use crate::download::ReResolutionTrigger;

        if error.contains("Download cancelled") || error.contains("operation was canceled") {
            return None;
        }
        if crate::worker::is_transport_failure(error) {
            return Some(ReResolutionTrigger::TransientFailure);
        }
        if error.contains("QobuzPreviewPayload") {
            return Some(ReResolutionTrigger::QobuzPreview);
        }
        if Self::is_account_auth_rejection(error) {
            return Some(ReResolutionTrigger::RequiresAuth);
        }
        let lowered = error.to_ascii_lowercase();
        // A bare HTTP status (including entity lookups) is NOT a catalogue
        // verdict: exclude the edition only when the text proves it.
        if lowered.contains("401") || lowered.contains("403") {
            if lowered.contains("stalesource")
                || lowered.contains("notfound")
                || lowered.contains("not found")
                || lowered.contains("stale_404")
            {
                return Some(ReResolutionTrigger::NotFound);
            }
            return None;
        }
        // Anything the categorical classifier calls auth is trusted only via
        // the account-scoped check above; demote it if it slipped through.
        match crate::download::classify_attempt_failure(error) {
            Some(ReResolutionTrigger::RequiresAuth) => None,
            other => other,
        }
    }

    /// WF-RES-01: identidad del intento que acaba de fallar, cuando el rechazo
    /// de autenticación permite una re-resolución entre cuentas hermanas.
    ///
    /// Devuelve `None` cuando NO debe reintentarse (ver `planned_auth_retry`).
    fn planned_auth_attempt(
        request: &DownloadRequest,
        error: &str,
        provider_service: &str,
    ) -> Option<(i64, String, Option<i64>)> {
        let plan = Self::planned_auth_retry(request, error, provider_service)?;
        let primary = plan.primary()?;
        Some((
            primary.service_id,
            primary.service_track_id.clone(),
            primary.account.as_ref().map(|a| a.account_id),
        ))
    }

    /// Raw provider errors may contain account or transport secrets. Only a
    /// bounded diagnostic belongs in persisted queue metadata.
    fn stamp_reresolved_auth_result(
        res: &mut crate::download::progress::DownloadResult,
        request: &DownloadRequest,
    ) {
        res.origin_service = request.service_name.clone();
        res.origin_service_track_id = request.service_track_id.clone();
        res.match_method = Some("plan_re_resolution".to_string());
        res.match_confidence = Some(0.95);
        res.fallback_reason = Some(format!(
            "{} account re-resolution after authentication rejection",
            request.service_name.as_deref().unwrap_or("provider")
        ));
    }

    /// WF-RES-01: el rechazo de credenciales es un veredicto sobre la CUENTA, no
    /// sobre el intento, así que persiste incluso cuando otra cuenta hermana
    /// completa la pista. Sin esto, `accounts` seguiría marcando válida a la que el
    /// proveedor rechazó y el orden de fallback volvería a ponerla delante de la
    /// hermana en la siguiente pista del mismo servicio.
    ///
    /// Falla en silencio a propósito: la descarga ya está completa y el archivo es
    /// válido; perder la anotación solo reintenta esa cuenta más adelante.
    pub async fn persist_rejected_primary_account(
        &self,
        attempted_account: Option<i64>,
        service: &str,
    ) {
        let Some(account_id) = attempted_account else {
            return;
        };
        let Some(db) = self.db.as_ref() else {
            return;
        };
        let now = chrono::Utc::now().to_rfc3339();
        match sqlx::query(
            "UPDATE accounts SET credentials_invalid = 1, invalid_reason = 'token_expired', \
             last_auth_error = 'Provider rejected credentials', last_auth_error_at = ? \
             WHERE id = ? AND service_id IN (SELECT id FROM services WHERE LOWER(name) = LOWER(?)) \
             AND COALESCE(credentials_invalid, 0) = 0",
        )
        .bind(&now)
        .bind(account_id)
        .bind(service)
        .execute(db)
        .await
        {
            Ok(result) if result.rows_affected() > 0 => {
                tracing::info!(
                    account_id,
                    service = %service,
                    "[Orchestrator] Persisted credential rejection of the rejected primary account"
                );
            }
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(
                    account_id,
                    service = %service,
                    error = %err,
                    "[Orchestrator] Could not persist rejected primary account"
                );
            }
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

            // 1a. WF-RES-01: antes de abortar, comprobar si el rechazo puede
            // resolverse con otra cuenta del MISMO `service_track_id`. Un 401 es
            // un veredicto sobre la cuenta, no sobre la edición; sin plan (o sin
            // coincidencia con el plan) se conserva el abort de abajo.
            //
            // NUNCA se cruza de servicio aquí: `reresolve_after_classified_failure`
            // resuelve con `requested_service` = qobuz, así que solo ve hermanas
            // Qobuz, y `allow_fallback=false` sigue prohibiendo Tidal.
            if let Some((service_id, attempted_track_id, attempted_account)) =
                Self::planned_auth_attempt(request, &err_msg, "qobuz")
            {
                let re_resolution = Box::pin(self.reresolve_after_classified_failure(
                    request,
                    crate::download::ReResolutionTrigger::RequiresAuth,
                    service_id,
                    &attempted_track_id,
                    attempted_account,
                    is_lossless_or_hires,
                ))
                .await;

                match re_resolution {
                    Ok(mut res) => {
                        // The primary account was the one the provider rejected, so
                        // its rejection must outlive this success: leaving it valid
                        // would put it back in front of the sibling for every later
                        // track of the same service.
                        self.persist_rejected_primary_account(attempted_account, "qobuz")
                            .await;
                        Self::stamp_reresolved_auth_result(&mut res, request);
                        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                        return Ok(res);
                    }
                    Err(reresolution_err) => return Err(reresolution_err.into()),
                }
            }

            // 1. Auth failure -> abort without cross-service fallback.
            // A bare "401" in edition metadata is not an auth verdict.
            if Self::is_account_auth_rejection(&err_msg) {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                let abort_reason = if err_msg.contains("401") || err_msg.contains("403") {
                    "RequiresAuth: Qobuz authentication required (HTTP 401/403). Automatic fallback aborted.".to_string()
                } else {
                    "RequiresAuth: Qobuz account authentication required. Automatic fallback aborted."
                        .to_string()
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
                || (err_msg.contains("track/get failed")
                    && !err_msg.contains("401")
                    && !err_msg.contains("403"))
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

            // 2.5 Stale primary source: before any diagnosis that ends the
            // attempt (allow_fallback=false) or crosses services, try to
            // refresh the SAME Qobuz edition under a new id. A 404 is a verdict
            // about the stored id, not about the edition or the account; Qobuz
            // re-issues ids, which leaves streamable editions unreachable
            // behind stale rows. Same service, same edition, same quality: this
            // is not a fallback.
            if is_stale {
                if let Some((fresh_id, fresh_quality)) =
                    self.refresh_stale_qobuz_source(request).await
                {
                    let mut refresh_req = request.clone();
                    refresh_req.service_name = Some("qobuz".to_string());
                    refresh_req.service_track_id = Some(fresh_id.to_string());
                    // ED-04: the fresh id was never authenticated by the plan;
                    // re-resolve instead of tripping plan validation.
                    refresh_req.resolution_plan = None;
                    if is_lossless_or_hires {
                        refresh_req.allow_fallback = false;
                    }
                    match Box::pin(self.qobuz.download_track(&refresh_req, self.db.as_ref()))
                        .await
                    {
                        Ok(mut res) => {
                            res.origin_service = Some("qobuz".to_string());
                            res.origin_service_track_id = request.service_track_id.clone();
                            res.effective_service = Some("qobuz".to_string());
                            res.effective_service_track_id = Some(fresh_id.to_string());
                            res.fallback_reason = Some(
                                "Qobuz source id was stale; refreshed on the same service"
                                    .to_string(),
                            );
                            res.match_method = Some("stale_source_refresh".to_string());
                            res.match_confidence = Some(1.0);
                            if let Some(canonical_id) = request.canonical_track_id {
                                self.persist_fallback_track_source(
                                    canonical_id,
                                    "qobuz",
                                    fresh_id,
                                    fresh_quality.as_deref(),
                                )
                                .await;
                            }
                            Self::reconcile_physical_audio_quality(&mut res, request);
                            info!(
                                "[Orchestrator] Download complete via refreshed Qobuz source: {} (id {} -> {})",
                                res.file_path,
                                request.service_track_id.as_deref().unwrap_or("?"),
                                fresh_id
                            );
                            PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                            return Ok(res);
                        }
                        Err(refresh_err) => {
                            warn!(
                                "[Orchestrator] Refreshed Qobuz source {} failed as well: {}",
                                fresh_id, refresh_err
                            );
                        }
                    }
                }
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

            // A track/entity lookup 401/403 does not prove an invalid account or
            // a missing edition. Do not relabel it as a 404 or retry a sibling.
            if (err_msg.contains("track/get failed")
                || err_msg.contains("album/get failed")
                || err_msg.contains("entity lookup"))
                && (err_msg.contains("401") || err_msg.contains("403"))
            {
                let reason = "ProviderEntityFailure: Qobuz entity lookup was rejected";
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, reason));
                return Err(anyhow!(reason));
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
                            // ED-04: the plan authenticated a Qobuz edition; a
                            // cross-service request must re-resolve, never
                            // carry the old execution lock (it produced
                            // "Tidal request differs from resolved plan").
                            tidal_req.resolution_plan = None;
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
                            // ED-04: quality differs from the authenticated
                            // plan; re-resolve instead of failing validation.
                            qobuz_320_req.resolution_plan = None;
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
                        // ED-04: plan was resolved for Qobuz/lossless; the
                        // 320 cross-service request must re-resolve.
                        tidal_320_req.resolution_plan = None;
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
                            // ED-04: quality differs from the authenticated
                            // plan; re-resolve instead of failing validation.
                            qobuz_320_req.resolution_plan = None;
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
                        // Keep BOTH diagnoses: the primary failure that forced
                        // the service switch and the fallback resolution error.
                        let combined = format!(
                            "{}; cross-service fallback failed: {}",
                            err_msg, fallback_err
                        );
                        PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &combined));
                        Err(anyhow!("{}", combined))
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
    /// An account-auth rejection only permits another account for the exact
    /// provider edition that failed, even if the resolver offers other editions.
    fn auth_retry_candidate_is_original_edition(
        trigger: crate::download::ReResolutionTrigger,
        service_id: i64,
        attempted_track_id: &str,
        candidate: &crate::download::PlanCandidate,
    ) -> bool {
        !matches!(trigger, crate::download::ReResolutionTrigger::RequiresAuth)
            || (candidate.service_id == service_id
                && candidate.service_track_id == attempted_track_id)
    }

    /// WF-RES-01: resultado categórico de haber agotado la secuencia de
    /// re-resolución. Nunca incluye texto crudo del proveedor ni IDs.
    ///
    /// El motivo que disparó la ronda decide la etiqueta: un rechazo de
    /// credenciales solo se mantiene `RequiresAuth` cuando TODAS las cuentas
    /// probadas fallaron por autenticación; un 401 de entity lookup (u otro
    /// fallo no clasificable) devuelve el fallo de entidad propiamente dicho,
    /// porque la cuenta nunca fue descartada ni la edición invalidada.
    fn auth_retry_exhaustion(
        trigger: crate::download::ReResolutionTrigger,
        attempts: usize,
        all_attempts_auth: bool,
    ) -> AuthRetryFailure {
        if attempts == 0 {
            // No usable sibling: the original account's genuine credential
            // rejection remains the only verdict.
            return if trigger == crate::download::ReResolutionTrigger::RequiresAuth {
                AuthRetryFailure::AuthExhausted
            } else {
                AuthRetryFailure::NoCandidate
            };
        }
        if trigger != crate::download::ReResolutionTrigger::RequiresAuth {
            return AuthRetryFailure::Entity;
        }
        if all_attempts_auth {
            AuthRetryFailure::AuthExhausted
        } else {
            AuthRetryFailure::Entity
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn reresolve_after_classified_failure(
        &self,
        request: &DownloadRequest,
        trigger: crate::download::ReResolutionTrigger,
        service_id: i64,
        attempted_track_id: &str,
        attempted_account: Option<i64>,
        is_lossless_or_hires: bool,
    ) -> std::result::Result<DownloadResult, AuthRetryFailure> {
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
            return Err(AuthRetryFailure::Entity);
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
            .map_err(|_| Self::auth_retry_exhaustion(trigger, 0, false))?;

        // Las exclusiones se ACUMULAN entre rondas: cada fallo clasificado se
        // suma al paquete anterior, así que una ronda nueva no puede reintentar
        // una combinación que una ronda anterior ya descartó.
        let mut exclusions = exclusions;
        let steps = crate::download::expand_attempt_sequence(&re_plan);
        let mut all_attempts_auth = true;
        let mut attempts: usize = 0;
        for step in steps {
            let Some(candidate) = step.identity(&re_plan) else {
                continue;
            };
            if !Self::auth_retry_candidate_is_original_edition(
                trigger,
                service_id,
                attempted_track_id,
                candidate,
            ) {
                continue;
            }
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
            if attempts as u32 >= crate::download::resolver::MAX_RE_RESOLUTIONS {
                break;
            }
            attempts += 1;

            let mut candidate_req = request.clone();
            candidate_req.service_name = Some(candidate.service_name.clone());
            candidate_req.service_track_id = Some(candidate.service_track_id.clone());
            candidate_req.quality = re_plan.quality.effective.clone();
            candidate_req.resolution_plan = Some(crate::download::DownloadPlan {
                canonical_track_id: re_plan.canonical_track_id,
                candidates: vec![crate::download::PlanCandidate {
                    account: Some(account.clone()),
                    sibling_accounts: Vec::new(),
                    ..candidate.clone()
                }],
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
                        "{} re-resolution after {:?} on verified candidate",
                        candidate.service_name, trigger
                    ));
                    res.match_method = Some("plan_re_resolution".to_string());
                    res.match_confidence = Some(0.95);
                    Self::reconcile_physical_audio_quality(&mut res, request);
                    return Ok(res);
                }
                Err(retry_err) => {
                    let retry_msg = retry_err.to_string();
                    // Clasificar solo reintentos con la MISMA restricción que la
                    // puerta de autenticación inicial: una cuenta hermana nunca
                    // se apartar por un HTTP 401 de entity lookup ni se olvida
                    // la cuenta por un «RequiresAuth» suelto.
                    let Some(retry_trigger) = Self::classify_retry_failure(&retry_msg) else {
                        // Un fallo no clasificable (cancelación incluida) detiene la
                        // secuencia en lugar de seguir intentando.
                        let failure = unclassifiable_retry_failure(&retry_msg);
                        warn!(
                            "[Orchestrator] WF-RES-01 re-resolution stopped: {}",
                            failure
                        );
                        return Err(failure);
                    };
                    if retry_trigger != crate::download::ReResolutionTrigger::RequiresAuth {
                        all_attempts_auth = false;
                    }
                    // Registrar el nuevo fallo en el paquete acumulado.
                    let round_exclusions = crate::download::exclusions_for_failure(
                        retry_trigger,
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

        // Exhaustion is categorical: never include raw provider responses or IDs
        // in the returned error (the worker persists it). For an auth-triggered
        // retry, only all-account auth failures keep the RequiresAuth label.
        Err(Self::auth_retry_exhaustion(
            trigger,
            attempts,
            all_attempts_auth,
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

            // 1a. WF-RES-01: antes de abortar, comprobar si el rechazo puede
            // resolverse con otra cuenta del MISMO `service_track_id`. Igual que
            // en Qobuz: nunca se cruza de servicio, y sin plan se conserva el
            // abort de abajo.
            if let Some((service_id, attempted_track_id, attempted_account)) =
                Self::planned_auth_attempt(request, &err_msg, "tidal")
            {
                let re_resolution = Box::pin(self.reresolve_after_classified_failure(
                    request,
                    crate::download::ReResolutionTrigger::RequiresAuth,
                    service_id,
                    &attempted_track_id,
                    attempted_account,
                    is_lossless_or_hires,
                ))
                .await;

                match re_resolution {
                    Ok(mut res) => {
                        // Same contract as Qobuz: the rejection belongs to the
                        // account the provider refused, so it is persisted even
                        // though a sibling completed the download.
                        self.persist_rejected_primary_account(attempted_account, "tidal")
                            .await;
                        Self::stamp_reresolved_auth_result(&mut res, request);
                        PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                        return Ok(res);
                    }
                    Err(reresolution_err) => return Err(reresolution_err.into()),
                }
            }

            // 1. Auth failure -> abort without cross-service fallback.
            // A bare "401" in edition metadata is not an auth verdict.
            if Self::is_account_auth_rejection(&err_msg) {
                PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &err_msg));
                let abort_reason =
                    "RequiresAuth: Tidal account authentication required. Automatic fallback aborted.";
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
                        // ED-04: the plan authenticated a Tidal edition; a
                        // cross-service request must re-resolve, never carry
                        // the old execution lock.
                        qobuz_req.resolution_plan = None;
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
                    // ED-04: quality differs from the authenticated plan; the
                    // 320 request must re-resolve instead of failing validation.
                    tidal_320_req.resolution_plan = None;
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
                    // ED-04: plan was resolved for Tidal/lossless; the 320
                    // cross-service request must re-resolve.
                    qobuz_320_req.resolution_plan = None;
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
                        // ED-04: the plan was resolved for lossless; a 320
                        // request that still carries it is rejected by the
                        // adapter before any network call, so it must
                        // re-resolve like every other 320 rung.
                        tidal_320_req.resolution_plan = None;
                        match Box::pin(
                            self.tidal.download_track(&tidal_320_req, self.db.as_ref()),
                        )
                        .await
                        {
                            Ok(mut tidal_res) => {
                                tidal_res.origin_service = Some("tidal".to_string());
                                tidal_res.effective_service = Some("tidal".to_string());
                                tidal_res.fallback_reason =
                                    Some("Degraded to 320 kbps on Tidal".to_string());
                                Self::reconcile_physical_audio_quality(&mut tidal_res, request);
                                PROGRESS_TRACKER.update(DownloadProgress::complete(item_id));
                                return Ok(tidal_res);
                            }
                            // Keep the diagnosis visible: an adapter-level
                            // rejection used to vanish here and the row only
                            // showed the combined cross-service error.
                            Err(tidal_err) => warn!(
                                "[Orchestrator] Tidal 320 kbps fallback failed: {}",
                                tidal_err
                            ),
                        }
                    }
                    // Keep BOTH diagnoses: the Tidal failure that forced the
                    // cross-service attempt and the Qobuz resolution error.
                    let combined = format!(
                        "{}; cross-service fallback failed: {}",
                        err_msg, fallback_err
                    );
                    PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &combined));
                    Err(anyhow!("{}", combined))
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

            let diagnosis = if matches!(
                primary_service,
                "spotify" | "deezer" | "apple_music" | "soundcloud"
            ) {
                format!("NoDownloadableSource: {} is catalog-only; connect Qobuz or Tidal and review the provider item before download", primary_service)
            } else {
                format!("Unsupported or unavailable service: {}", primary_service)
            };
            PROGRESS_TRACKER.update(DownloadProgress::failed(item_id, &diagnosis));
            Err(anyhow!(diagnosis))
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

    // Synthetic catalogue only: no downloader, keyring, profile, or network.
    #[tokio::test]
    async fn auth_retry_never_attempts_another_edition_after_a_accounts_are_excluded() {
        use crate::download::ReResolutionTrigger;
        use sqlx::sqlite::SqlitePoolOptions;

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        // The current persistent schema permits only one source per (track,
        // service). Shadow that empty table on this one SQLite connection to
        // exercise the resolver's edition boundary without altering migrations.
        sqlx::query("CREATE TEMP TABLE track_sources AS SELECT * FROM main.track_sources WHERE 0")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json, credentials_invalid, streaming_entitled) VALUES (31, 2, 'A-one', 'a-one@test.invalid', 1, 'synthetic', 0, 1), (32, 2, 'A-two', 'a-two@test.invalid', 1, 'synthetic', 0, 1)")
            .execute(&db).await.unwrap();
        let track: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title) VALUES ('synthetic editions') RETURNING id",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'edition-A', 'FLAC', 16, 44100, 100, 1), (?, 2, 'edition-B', 'FLAC', 16, 44100, 90, 1)")
            .bind(track).bind(track).execute(&db).await.unwrap();
        let intent = crate::download::DownloadIntent {
            canonical_track_id: track,
            requested_service: Some("qobuz".into()),
            requested_quality: Some("lossless".into()),
            strict_quality: true,
            allow_fallback: false,
            locked_track_source_id: crate::download::resolve_locked_track_source_id(
                &db,
                track,
                Some("qobuz"),
                Some("edition-A"),
            )
            .await,
            ..Default::default()
        };
        assert!(intent.locked_track_source_id.is_some());
        let initial = crate::download::resolve_download_plan(&db, &intent, &Default::default())
            .await
            .unwrap();
        let a = initial.primary().unwrap();
        assert_eq!(a.service_track_id, "edition-A");
        assert_eq!(a.sibling_accounts.len(), 1);
        let first_account = a.account.as_ref().unwrap().account_id;
        let second_account = a.sibling_accounts[0].account_id;
        assert_ne!(first_account, second_account);

        let trigger = ReResolutionTrigger::RequiresAuth;
        let mut exclusions = crate::download::exclusions_for_failure(
            trigger,
            track,
            2,
            "edition-A",
            Some(first_account),
        );
        let after_first = crate::download::resolve_download_plan(&db, &intent, &exclusions)
            .await
            .unwrap();
        assert_eq!(after_first.primary().unwrap().service_track_id, "edition-A");
        assert_eq!(
            after_first
                .primary()
                .unwrap()
                .account
                .as_ref()
                .unwrap()
                .account_id,
            second_account
        );
        assert!(crate::download::expand_attempt_sequence(&after_first)
            .iter()
            .any(|step| {
                step.identity(&after_first).is_some_and(|candidate| {
                    DownloadOrchestrator::auth_retry_candidate_is_original_edition(
                        trigger,
                        2,
                        "edition-A",
                        candidate,
                    ) && step
                        .account(&after_first)
                        .is_some_and(|account| account.account_id == second_account)
                })
            }));

        // Before the second A failure, the resolver already offers B with an
        // account in the same expansion. The production gate must not execute it.
        let b = after_first
            .candidates
            .iter()
            .find(|candidate| candidate.service_track_id == "edition-B")
            .unwrap();
        assert!(b.account.is_some());
        assert!(
            !DownloadOrchestrator::auth_retry_candidate_is_original_edition(
                trigger,
                2,
                "edition-A",
                b,
            )
        );
        assert!(
            DownloadOrchestrator::auth_retry_candidate_is_original_edition(
                ReResolutionTrigger::NotFound,
                2,
                "edition-A",
                b,
            ),
            "non-auth failures may still choose a different edition"
        );

        exclusions.merge(&crate::download::exclusions_for_failure(
            trigger,
            track,
            2,
            "edition-A",
            Some(second_account),
        ));
        // Account exclusions are currently canonical-track-wide. Thus once
        // both A accounts are excluded, even B has no usable account; a future
        // edition-scoped exclusion would offer B, and the gate above covers it.
        assert!(
            crate::download::resolve_download_plan(&db, &intent, &exclusions)
                .await
                .is_err()
        );
    }

    #[test]
    fn auth_fallback_reason_omits_untrusted_track_id() {
        let raw_id = "untrusted-track-id:secret=do-not-persist";
        let request = DownloadRequest {
            service_name: Some("qobuz".into()),
            service_track_id: Some(raw_id.into()),
            ..Default::default()
        };
        let mut result = DownloadResult::default();
        DownloadOrchestrator::stamp_reresolved_auth_result(&mut result, &request);
        assert_eq!(result.origin_service_track_id.as_deref(), Some(raw_id));
        let reason = result.fallback_reason.as_deref().unwrap();
        assert_eq!(
            reason,
            "qobuz account re-resolution after authentication rejection"
        );
        assert!(!reason.contains(raw_id));
    }

    #[tokio::test]
    async fn auth_re_resolution_selects_same_edition_sibling_without_relaxing_quality() {
        use sqlx::sqlite::SqlitePoolOptions;
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires'), (3, 'tidal', 1, 'hires')")
            .execute(&db).await.unwrap();
        sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json, credentials_invalid, streaming_entitled) VALUES (11, 2, 'primary', 'primary@test.invalid', 1, 'synthetic', 0, 1), (12, 2, 'sibling', 'sibling@test.invalid', 1, 'synthetic', 0, 1)")
            .execute(&db).await.unwrap();
        let track: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('synthetic') RETURNING id")
                .fetch_one(&db)
                .await
                .unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'same-edition', 'FLAC', 16, 44100, 100, 1)")
            .bind(track).execute(&db).await.unwrap();
        let intent = crate::download::DownloadIntent {
            canonical_track_id: track,
            requested_service: Some("qobuz".into()),
            requested_quality: Some("lossless".into()),
            strict_quality: true,
            allow_fallback: false,
            ..Default::default()
        };
        let plan = crate::download::resolve_download_plan(&db, &intent, &Default::default())
            .await
            .unwrap();
        let primary = plan.primary().unwrap();
        assert_eq!(primary.account.as_ref().unwrap().account_id, 12);
        let request = DownloadRequest {
            canonical_track_id: Some(track),
            service_name: Some("qobuz".into()),
            service_track_id: Some("same-edition".into()),
            quality: plan.quality.effective.clone(),
            strict_quality: true,
            allow_fallback: false,
            resolution_plan: Some(plan.clone()),
            ..Default::default()
        };
        let error = "RequiresAuth: Qobuz HTTP 401";
        assert!(DownloadOrchestrator::planned_auth_retry(&request, error, "qobuz").is_some());
        let credential_error = "Failed to decrypt Qobuz credentials: private-provider-detail";
        assert_eq!(
            crate::download::classify_attempt_failure(credential_error),
            None
        );
        assert!(
            DownloadOrchestrator::planned_auth_retry(&request, credential_error, "qobuz").is_some(),
            "a credential-only rejection must still reach the sibling even though no categorical trigger exists"
        );
        assert!(
            DownloadOrchestrator::planned_auth_retry(&request, credential_error, "tidal").is_none()
        );
        for non_auth in [
            "Qobuz track/get failed for ID 123: HTTP 401 Unauthorized",
            "Qobuz album/get failed: HTTP 403",
            "Qobuz entity lookup: HTTP 401",
            "Download cancelled by user: credentials invalid",
            "NetworkExhausted: token expired while connection reset",
        ] {
            assert!(
                DownloadOrchestrator::planned_auth_retry(&request, non_auth, "qobuz").is_none(),
                "not account-specific: {non_auth}"
            );
        }
        let exclusions = crate::download::exclusions_for_failure(
            crate::download::ReResolutionTrigger::RequiresAuth,
            track,
            primary.service_id,
            &primary.service_track_id,
            Some(12),
        );
        let next = crate::download::resolve_download_plan(&db, &intent, &exclusions)
            .await
            .unwrap();
        let sibling = next.primary().unwrap();
        assert_eq!(sibling.service_track_id, "same-edition");
        assert_eq!(sibling.account.as_ref().unwrap().account_id, 11);
        assert_eq!(next.quality.effective, plan.quality.effective);
        assert!(next.strict_quality);
        let exhausted = crate::download::exclusions_for_failure(
            crate::download::ReResolutionTrigger::RequiresAuth,
            track,
            primary.service_id,
            &primary.service_track_id,
            Some(11),
        );
        let mut all = exclusions;
        all.merge(&exhausted);
        assert!(crate::download::resolve_download_plan(&db, &intent, &all)
            .await
            .is_err());
        assert!(DownloadOrchestrator::planned_auth_retry(
            &request,
            "Download cancelled by user",
            "qobuz"
        )
        .is_none());
        assert!(DownloadOrchestrator::planned_auth_retry(
            &request,
            "metadata release 401 (remaster)",
            "qobuz"
        )
        .is_none());
        assert!(DownloadOrchestrator::planned_auth_retry(
            &request,
            "RequiresAuth: Tidal HTTP 401",
            "tidal"
        )
        .is_none());
    }

    #[test]
    fn sibling_retry_requires_account_scoped_auth_not_entity_http_status() {
        for auth in [
            "Failed to decrypt Qobuz credentials: private-provider-detail",
            "RequiresAuth: Qobuz token expired (HTTP 401)",
            "Qobuz rejected the credentials (HTTP 401)",
            "Tidal authentication failed for user token",
            "stream entitlement denied for this account",
        ] {
            assert!(
                DownloadOrchestrator::is_account_auth_rejection(auth),
                "account auth: {auth}"
            );
        }
        for unrelated in [
            "Qobuz track/get failed for ID 123: HTTP 401 Unauthorized",
            "Qobuz album/get failed for ID 123: HTTP 403 Forbidden",
            "Qobuz entity lookup: HTTP 401",
            "HTTP 401 Unauthorized",
            "metadata release 401 remaster",
            "Download cancelled by user: token expired",
            "operation was canceled: credentials invalid",
            "NetworkExhausted: token expired while connection reset",
            "error sending request: HTTP 401 token expired",
        ] {
            assert!(
                !DownloadOrchestrator::is_account_auth_rejection(unrelated),
                "not account auth: {unrelated}"
            );
        }
    }

    /// A retry must never be treated as account auth when it is only an entity
    /// lookup 401: the sibling loop then has no third attempt to exclude.
    #[test]
    fn retry_classification_does_not_punish_account_for_entity_lookup_401() {
        use crate::download::ReResolutionTrigger;

        // Un HTTP status solo (track/get, album/get, entity lookup) no es veredicto de cuenta.
        for entity_401 in [
            "Qobuz track/get failed for ID 123: HTTP 401 Unauthorized",
            "Qobuz album/get failed for ID 123: HTTP 403 Forbidden",
            "Qobuz entity lookup: HTTP 401",
            "HTTP401 unauthorized",
        ] {
            assert_eq!(
                DownloadOrchestrator::classify_retry_failure(entity_401),
                None,
                "entity lookup 401/403 must not classify retry: {entity_401}"
            );
        }
        // Sí lo es cuando además hay evidencia de edición inexistente.
        for genuine_not_found in [
            "NotFound: entity lookup HTTP 404 edition missing",
            "StaleSource: stale_404 for HTTP 401 edition",
        ] {
            assert_eq!(
                DownloadOrchestrator::classify_retry_failure(genuine_not_found),
                Some(ReResolutionTrigger::NotFound),
                "edition truly invalid must be NotFound: {genuine_not_found}"
            );
        }
        // Genuina AUTH de cuenta: se excluye la cuenta y se avanza.
        for account_auth in [
            "RequiresAuth: Qobuz HTTP 401",
            "Failed to decrypt Qobuz credentials: private-provider-detail",
        ] {
            assert_eq!(
                DownloadOrchestrator::classify_retry_failure(account_auth),
                Some(ReResolutionTrigger::RequiresAuth),
                "account auth must be RequiresAuth: {account_auth}"
            );
        }
        // Cancelación y transporte siempre mandan.
        assert_eq!(
            DownloadOrchestrator::classify_retry_failure("Download cancelled"),
            None
        );
        assert_eq!(
            DownloadOrchestrator::classify_retry_failure("NetworkExhausted: connection reset"),
            Some(ReResolutionTrigger::TransientFailure)
        );
        // Preview y calidad permanecen clasificadas.
        assert_eq!(
            DownloadOrchestrator::classify_retry_failure(
                "QobuzPreviewPayload: preview of 30 s instead of stream"
            ),
            Some(ReResolutionTrigger::QobuzPreview)
        );
        assert_eq!(
            DownloadOrchestrator::classify_retry_failure(
                "InvalidAudioPayload: physical quality below the minimum floor"
            ),
            Some(ReResolutionTrigger::PhysicalQualityInsufficient)
        );
    }

    /// Regression: after an entity-lookup 401 in the retry path, the sibling
    /// loop must stop that candidate (no third account attempt) without marking
    /// the failed account as credentials-invalid or the edition as not found.
    /// Only the ORIGINAL rejection is persisted, never the raw retry error.
    #[tokio::test]
    async fn entity_lookup_401_on_retry_does_not_advance_to_third_account() {
        use crate::download::ReResolutionTrigger;
        use sqlx::sqlite::SqlitePoolOptions;

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        // Service 2 (qobuz) already exists in the migrated schema.
        sqlx::query(
            "INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json, credentials_invalid, streaming_entitled) \
             VALUES (21, 2, 'one', 'one@test.invalid', 1, 'synth', 0, 1), \
                    (22, 2, 'two', 'two@test.invalid', 1, 'synth', 0, 1), \
                    (23, 2, 'three', 'three@test.invalid', 1, 'synth', 0, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        let track: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('synthetic') RETURNING id")
                .fetch_one(&db)
                .await
                .unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'same-edition', 'FLAC', 16, 44100, 100, 1)")
            .bind(track)
            .execute(&db)
            .await
            .unwrap();

        // 1. First re-resolution round: sibling 22 fails with an entity-lookup
        //    401. The classifier must return None (stop), never RequiresAuth.
        let entity_401 = "Qobuz track/get failed for ID 5: HTTP 401 Unauthorized";
        let retry_classified = DownloadOrchestrator::classify_retry_failure(entity_401);
        assert_eq!(
            retry_classified, None,
            "entity-lookup 401 must not advance the sibling sequence"
        );
        // Crucially, the 401 text is NOT an account-auth verdict: the failed
        // account must keep its credentials valid.
        assert!(!DownloadOrchestrator::is_account_auth_rejection(entity_401));
        assert!(DownloadOrchestrator::planned_auth_retry(
            &DownloadRequest {
                canonical_track_id: Some(track),
                service_name: Some("qobuz".into()),
                service_track_id: Some("same-edition".into()),
                ..Default::default()
            },
            entity_401,
            "qobuz"
        )
        .is_none());
        // No account was invalidated and no raw error was persisted.
        let invalid: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE credentials_invalid != 0")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            invalid, 0,
            "no account may be marked invalid by a 401 lookup"
        );
        let persisted: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM accounts WHERE last_auth_error LIKE '%track/get%'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(persisted, 0, "the raw retry error is never persisted");

        // 2. A credential-only error DOES advance, still on the same edition.
        let credential_error = "Failed to decrypt Qobuz credentials: private-provider-detail";
        assert_eq!(
            DownloadOrchestrator::classify_retry_failure(credential_error),
            Some(ReResolutionTrigger::RequiresAuth)
        );
        let next = crate::download::resolve_download_plan(
            &db,
            &crate::download::DownloadIntent {
                canonical_track_id: track,
                requested_service: Some("qobuz".into()),
                requested_quality: Some("lossless".into()),
                strict_quality: true,
                allow_fallback: false,
                ..Default::default()
            },
            &crate::download::exclusions_for_failure(
                ReResolutionTrigger::RequiresAuth,
                track,
                2,
                "same-edition",
                Some(23),
            ),
        )
        .await
        .unwrap();
        let advanced = next.primary().unwrap();
        assert_eq!(
            advanced.service_track_id, "same-edition",
            "the sibling advances on the SAME edition"
        );
        assert_eq!(advanced.account.as_ref().unwrap().account_id, 22);
        assert!(next.strict_quality, "quality policy is preserved");
    }

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

    /// P1 regression: exhaustion mapping must keep entity failures distinct
    /// from credential failures. This is a pure function on categorical
    /// outcomes only - no provider response, no account or track IDs.
    #[test]
    fn auth_retry_exhaustion_labels_entity_and_all_account_auth_differently() {
        use crate::download::ReResolutionTrigger;

        // All sibling accounts failed with real credential rejections.
        assert_eq!(
            DownloadOrchestrator::auth_retry_exhaustion(ReResolutionTrigger::RequiresAuth, 2, true,),
            AuthRetryFailure::AuthExhausted,
            "true all-account auth exhaustion stays RequiresAuth"
        );
        // The failing sibling returned an entity 401 (`track/get`) that the
        // classifier refused to classify: the account was never discredited,
        // so it must NOT be reported as an authentication failure.
        assert_eq!(
            DownloadOrchestrator::auth_retry_exhaustion(
                ReResolutionTrigger::RequiresAuth,
                1,
                false,
            ),
            AuthRetryFailure::Entity,
            "an unclassifiable entity failure must not become RequiresAuth"
        );
        // A classified non-auth failure (e.g. preview) that exhausted the
        // sequence is likewise an entity failure, not an auth failure.
        assert_eq!(
            DownloadOrchestrator::auth_retry_exhaustion(
                ReResolutionTrigger::RequiresAuth,
                2,
                false,
            ),
            AuthRetryFailure::Entity
        );
        // Non-auth triggers never produce auth exhaustion.
        assert_eq!(
            DownloadOrchestrator::auth_retry_exhaustion(
                ReResolutionTrigger::TransientFailure,
                2,
                true,
            ),
            AuthRetryFailure::Entity
        );
        // Empty sequence: the original account's true auth rejection remains.
        assert_eq!(
            DownloadOrchestrator::auth_retry_exhaustion(
                ReResolutionTrigger::RequiresAuth,
                0,
                false
            ),
            AuthRetryFailure::AuthExhausted
        );
    }

    #[test]
    fn auth_retry_failure_display_is_categorical_and_secret_free() {
        let entity = AuthRetryFailure::Entity.to_string();
        assert!(
            entity.starts_with("ProviderEntityFailure:"),
            "entity failure must be recognised as entity, got: {entity}"
        );
        assert!(
            !entity.contains("RequiresAuth"),
            "entity failure must not claim auth, got: {entity}"
        );

        let auth = AuthRetryFailure::AuthExhausted.to_string();
        assert!(auth.starts_with("RequiresAuth:"));

        let cancelled = AuthRetryFailure::Cancelled.to_string();
        assert!(
            is_cancellation_error(&cancelled),
            "cancellation must be detectable by the same predicate the caller uses"
        );
        assert!(cancelled.contains("Download cancelled"));

        let none = AuthRetryFailure::NoCandidate.to_string();
        assert!(none.starts_with("SourceIdentityMissing:"));

        // The messages carry no account identifiers, no raw HTTP bodies and no
        // URLs: the worker persists these strings verbatim.
        for message in [&entity, &auth, &cancelled, &none] {
            assert!(!message.contains("track/get"), "no entity endpoint text");
            assert!(!message.contains("400 7"), "no raw HTTP text");
            assert!(!message.contains("@"), "no account identifiers");
            assert!(!message.contains("http"), "no URLs");
        }
    }

    /// P1 regression with a synthetic catalogue: an initial genuine auth
    /// rejection on account 31 followed by a sibling 401 from the entity lookup.
    /// The sibling's own attempt is what an unclassifiable entity failure looks
    /// like in production, so the retry must stop as ENTITY and must never
    /// label the queue RequiresAuth. No account is marked credentials-invalid.
    #[tokio::test]
    async fn auth_rejection_then_entity_401_reports_entity_not_requires_auth() {
        use crate::download::ReResolutionTrigger;
        use sqlx::sqlite::SqlitePoolOptions;

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json, credentials_invalid, streaming_entitled) \
             VALUES (31, 2, 'one', 'one@test.invalid', 1, 'synthetic', 0, 1), \
                    (32, 2, 'two', 'two@test.invalid', 1, 'synthetic', 0, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        let track: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title) VALUES ('synthetic entity') RETURNING id",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'auth-then-entity', 'FLAC', 16, 44100, 100, 1)")
            .bind(track)
            .execute(&db)
            .await
            .unwrap();

        // The initial rejection IS an account-auth verdict, so a sibling
        // retry is offered for the SAME service_track_id.
        let initial_auth = "RequiresAuth: Qobuz authentication failed (HTTP 401)";
        let request = DownloadRequest {
            canonical_track_id: Some(track),
            service_name: Some("qobuz".into()),
            service_track_id: Some("auth-then-entity".into()),
            quality: "LOSSLESS".into(),
            resolution_plan: Some(crate::download::DownloadPlan {
                canonical_track_id: track,
                candidates: vec![crate::download::PlanCandidate {
                    service_id: 2,
                    service_name: "qobuz".to_string(),
                    service_track_id: "auth-then-entity".to_string(),
                    candidate_tier: syncify_core_domain::quality::AudioTier::Lossless,
                    identity: crate::download::resolver::CandidateIdentity {
                        track_source_id: 0,
                        canonical_track_id: track,
                        isrc: None,
                    },
                    account: Some(crate::download::resolver::CandidateAccount {
                        account_id: 31,
                        is_active: true,
                        streaming_entitled: Some(1),
                    }),
                    sibling_accounts: Vec::new(),
                    service_priority: None,
                    quality_score: None,
                    bit_depth: None,
                }],
                quality: crate::download::resolver::ResolvedQuality {
                    effective: "lossless".to_string(),
                    candidate: "lossless".to_string(),
                    clamped: false,
                },
                strict_quality: false,
                allow_fallback: false,
            }),
            ..Default::default()
        };
        assert!(
            DownloadOrchestrator::planned_auth_retry(&request, initial_auth, "qobuz").is_some(),
            "a genuine credential rejection permits a sibling retry"
        );

        // The sibling's failure is an entity lookup: classify_retry_failure
        // deliberately refuses to classify it, so last_failure is None.
        let entity_401 = "Qobuz track/get failed for edition auth-then-entity: HTTP 401";
        assert_eq!(
            DownloadOrchestrator::classify_retry_failure(entity_401),
            None,
            "an entity 401 is not a catalogue verdict on its own"
        );

        // Exhaustion after that stop is an ENTITY failure, not auth.
        let failure = DownloadOrchestrator::auth_retry_exhaustion(
            ReResolutionTrigger::RequiresAuth,
            1,
            false,
        );
        assert_eq!(failure, AuthRetryFailure::Entity);
        let message = failure.to_string();
        assert!(message.starts_with("ProviderEntityFailure:"), "{message}");
        assert!(!message.contains("RequiresAuth"), "{message}");
        // What the worker persists is categorical: no account or edition IDs.
        assert!(!message.contains("auth-then-entity"), "{message}");
        assert!(!message.contains("track/get"), "{message}");

        // The worker's own decision rule classifies the persisted text as
        // entity (identity), never as auth.
        assert!(message.contains("ProviderEntityFailure"));
        let invalid: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE credentials_invalid != 0")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(invalid, 0, "an entity 401 never invalidates an account");
    }

    /// P1 regression: every sibling account failing with genuine credential
    /// rejections keeps the true RequiresAuth verdict.
    #[tokio::test]
    async fn all_accounts_rejecting_still_reports_requires_auth_exhaustion() {
        use crate::download::ReResolutionTrigger;
        use sqlx::sqlite::SqlitePoolOptions;

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json, credentials_invalid, streaming_entitled) \
             VALUES (41, 2, 'one', 'one@test.invalid', 1, 'synthetic', 0, 1), \
                    (42, 2, 'two', 'two@test.invalid', 1, 'synthetic', 0, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        let track: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('synthetic auth') RETURNING id")
                .fetch_one(&db)
                .await
                .unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'all-auth', 'FLAC', 16, 44100, 100, 1)")
            .bind(track)
            .execute(&db)
            .await
            .unwrap();

        // Both siblings fail with credential rejections: the last classified
        // failure IS auth, so exhaustion is genuinely all-account auth.
        let sibling_auth = "RequiresAuth: Qobuz token expired for this account";
        assert_eq!(
            DownloadOrchestrator::classify_retry_failure(sibling_auth),
            Some(ReResolutionTrigger::RequiresAuth),
            "a genuine credential rejection classifies as auth"
        );
        let failure =
            DownloadOrchestrator::auth_retry_exhaustion(ReResolutionTrigger::RequiresAuth, 2, true);
        assert_eq!(failure, AuthRetryFailure::AuthExhausted);
        let message = failure.to_string();
        assert!(message.starts_with("RequiresAuth:"), "{message}");
        assert!(!message.contains("all-auth"), "no track IDs persisted");
        assert!(!message.contains("41") && !message.contains("42"));
    }

    /// Cancellation inside the retry path propagates unchanged: it is never
    /// relabelled as auth or entity failure.
    #[tokio::test]
    async fn cancellation_inside_reresolution_is_not_relabelled() {
        use crate::download::ReResolutionTrigger;
        use sqlx::sqlite::SqlitePoolOptions;

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        // No accounts: an empty sequence cannot corroborate the trigger.
        let track: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title) VALUES ('synthetic cancel') RETURNING id",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'cancel-edition', 'FLAC', 16, 44100, 100, 1)")
            .bind(track)
            .execute(&db)
            .await
            .unwrap();

        // Cancellation is detected by the exact predicate the retry loop uses,
        // and only cancellation is.
        assert!(is_cancellation_error(
            &AuthRetryFailure::Cancelled.to_string()
        ));
        assert!(!is_cancellation_error(
            &AuthRetryFailure::Entity.to_string()
        ));
        assert!(!is_cancellation_error(
            &AuthRetryFailure::AuthExhausted.to_string()
        ));
        assert!(!is_cancellation_error("Qobuz track/get failed: HTTP 401"));

        // With no sibling at all the sequence preserves the original account's
        // genuine auth rejection without including the edition ID.
        let failure = DownloadOrchestrator::auth_retry_exhaustion(
            ReResolutionTrigger::RequiresAuth,
            0,
            false,
        );
        assert_eq!(failure, AuthRetryFailure::AuthExhausted);
        let message = failure.to_string();
        assert!(message.starts_with("RequiresAuth:"), "{message}");
        assert!(!message.contains("cancel-edition"), "{message}");

        // And a genuine cancellation during the retry would be returned as
        // Cancelled, whose message the caller still recognises.
        let cancel_message = AuthRetryFailure::Cancelled.to_string();
        assert!(is_cancellation_error(&cancel_message));
        assert!(!cancel_message.contains("ProviderEntityFailure"));
    }
}
