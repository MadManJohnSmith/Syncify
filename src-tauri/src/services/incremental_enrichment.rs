//! Incremental Library Enrichment Service for Syncify
//!
//! Processes existing library tracks to resolve missing or low-precedence metadata
//! without data degradation, without unsafe file renames, and without downloading audio.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{warn};

use syncify_core_domain::{derive_track_version, VersionDerivationInput};
use syncify_metadata_domain::FieldValidator;

use crate::services::musicbrainz::{MusicBrainzClient, MusicBrainzRecording, Release};

/// Mode for selecting tracks to enrich
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EnrichmentMode {
    /// Enrich only tracks that have missing / incomplete metadata fields
    IncompleteOnly,
    /// Revalidate all tracks across the entire library
    RevalidateAll,
    /// Enrich only specific track IDs explicitly selected
    Selection,
}

impl Default for EnrichmentMode {
    fn default() -> Self {
        EnrichmentMode::IncompleteOnly
    }
}

/// State of an individual track during the enrichment job
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrackEnrichmentStatus {
    Queued,
    Resolving,
    Enriching,
    Persisted,
    SkippedComplete,
    SkippedPrecedence,
    Partial,
    Failed,
    Cancelled,
}

/// Overall job lifecycle status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// Preview of an incremental enrichment run
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentPreview {
    pub total_tracks: usize,
    pub total_eligible: usize,
    pub total_complete: usize,
    pub total_skipped_precedence: usize,
    pub available_sources: Vec<String>,
    pub mode: EnrichmentMode,
}

/// Telemetry record for a single track processed in the job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEnrichmentReportItem {
    pub track_id: i64,
    pub track_title: String,
    pub artist_name: String,
    pub requested_fields: Vec<String>,
    pub modified_fields: Vec<String>,
    pub previous_provenance: HashMap<String, String>,
    pub new_provenance: HashMap<String, String>,
    pub provider: String,
    pub confidence: f64,
    pub duration_ms: u64,
    pub error: Option<String>,
    pub status: TrackEnrichmentStatus,
}

/// Full progress and summary report of an enrichment job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentJobSummary {
    pub job_id: String,
    pub mode: EnrichmentMode,
    pub status: JobStatus,
    pub total_tracks: usize,
    pub processed_tracks: usize,
    pub modified_tracks: usize,
    pub skipped_complete_tracks: usize,
    pub skipped_precedence_tracks: usize,
    pub failed_tracks: usize,
    pub current_track: Option<String>,
    pub current_phase: Option<String>,
    pub items: Vec<TrackEnrichmentReportItem>,
    pub available_sources: Vec<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

/// In-memory cache for incremental enrichment sessions
#[derive(Default)]
pub struct EnrichmentMemoryCache {
    pub isrc_cache: HashMap<String, Option<MusicBrainzRecording>>,
    pub mbid_cache: HashMap<String, Option<MusicBrainzRecording>>,
    pub album_cache: HashMap<String, Option<Release>>,
    pub artist_mbid_cache: HashMap<String, Option<String>>,
}

/// Incremental Library Enrichment Service
pub struct IncrementalEnrichmentService {
    mb_client: MusicBrainzClient,
    cache: Arc<RwLock<EnrichmentMemoryCache>>,
    cancellation_token: Arc<AtomicBool>,
    active_job: Arc<RwLock<Option<EnrichmentJobSummary>>>,
}

impl Default for IncrementalEnrichmentService {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalEnrichmentService {
    pub fn new() -> Self {
        Self {
            mb_client: MusicBrainzClient::new(),
            cache: Arc::new(RwLock::new(EnrichmentMemoryCache::default())),
            cancellation_token: Arc::new(AtomicBool::new(false)),
            active_job: Arc::new(RwLock::new(None)),
        }
    }

    /// Clear all in-memory caches
    pub fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.write() {
            *c = EnrichmentMemoryCache::default();
        }
    }

    /// Cancel any currently active enrichment job
    pub fn cancel_job(&self) {
        self.cancellation_token.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.active_job.write() {
            if let Some(ref mut job) = *guard {
                if job.status == JobStatus::Running || job.status == JobStatus::Queued {
                    job.status = JobStatus::Cancelled;
                    job.current_phase = Some("Cancelled".to_string());
                }
            }
        }
    }

    /// Get current active or latest job status
    pub fn get_job_status(&self) -> Option<EnrichmentJobSummary> {
        self.active_job.read().ok().and_then(|g| g.clone())
    }

    /// Reset cancellation state
    pub fn reset_cancellation(&self) {
        self.cancellation_token.store(false, Ordering::SeqCst);
    }

    /// Generate an enrichment preview without mutating database or disk
    pub async fn preview_enrichment(
        &self,
        db: &SqlitePool,
        mode: EnrichmentMode,
        selection_ids: Option<Vec<i64>>,
    ) -> Result<EnrichmentPreview, String> {
        let total_tracks: usize = match (&mode, &selection_ids) {
            (EnrichmentMode::Selection, Some(ids)) => ids.len(),
            _ => {
                let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
                    .fetch_one(db)
                    .await
                    .unwrap_or((0,));
                cnt.0 as usize
            }
        };

        let raw_tracks = self.fetch_candidate_rows(db, &mode, selection_ids.as_deref()).await?;

        let mut total_eligible = 0;
        let mut total_skipped_precedence = 0;

        for track in &raw_tracks {
            if track.enrichment_status.as_deref() == Some("manual") {
                total_skipped_precedence += 1;
                continue;
            }

            let is_incomplete = self.is_track_incomplete(track);
            if is_incomplete || mode == EnrichmentMode::RevalidateAll {
                total_eligible += 1;
            }
        }

        let total_complete = total_tracks.saturating_sub(total_eligible + total_skipped_precedence);

        let available_sources = vec![
            "MusicBrainz".to_string(),
            "Qobuz".to_string(),
            "Spotify".to_string(),
            "LastFM".to_string(),
        ];

        Ok(EnrichmentPreview {
            total_tracks,
            total_eligible,
            total_complete,
            total_skipped_precedence,
            available_sources,
            mode,
        })
    }

    /// Execute incremental library enrichment
    pub async fn run_enrichment<F>(
        &self,
        db: &SqlitePool,
        mode: EnrichmentMode,
        selection_ids: Option<Vec<i64>>,
        mut progress_cb: F,
    ) -> Result<EnrichmentJobSummary, String>
    where
        F: FnMut(&EnrichmentJobSummary) + Send + 'static,
    {
        let total_tracks: usize = match (&mode, &selection_ids) {
            (EnrichmentMode::Selection, Some(ids)) => ids.len(),
            _ => {
                let cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks")
                    .fetch_one(db)
                    .await
                    .unwrap_or((0,));
                cnt.0 as usize
            }
        };

        let candidates = self.fetch_candidate_rows(db, &mode, selection_ids.as_deref()).await?;
        let job_id = uuid::Uuid::new_v4().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();

        let initial_skipped_complete = if mode == EnrichmentMode::IncompleteOnly {
            total_tracks.saturating_sub(candidates.len())
        } else {
            0
        };

        let mut summary = EnrichmentJobSummary {
            job_id: job_id.clone(),
            mode: mode.clone(),
            status: JobStatus::Running,
            total_tracks,
            processed_tracks: initial_skipped_complete,
            modified_tracks: 0,
            skipped_complete_tracks: initial_skipped_complete,
            skipped_precedence_tracks: 0,
            failed_tracks: 0,
            current_track: None,
            current_phase: Some("Resolving".to_string()),
            items: Vec::new(),
            available_sources: vec![
                "MusicBrainz".to_string(),
                "Qobuz".to_string(),
                "Spotify".to_string(),
                "LastFM".to_string(),
            ],
            started_at,
            completed_at: None,
        };

        if self.cancellation_token.load(Ordering::SeqCst) {
            summary.status = JobStatus::Cancelled;
            summary.current_phase = Some("Cancelled".to_string());
            if let Ok(mut guard) = self.active_job.write() {
                *guard = Some(summary.clone());
            }
            progress_cb(&summary);
            return Ok(summary);
        }

        {
            let mut guard = self.active_job.write().unwrap();
            *guard = Some(summary.clone());
        }
        progress_cb(&summary);

        for candidate in candidates {
            if self.cancellation_token.load(Ordering::SeqCst) {
                summary.status = JobStatus::Cancelled;
                summary.current_phase = Some("Cancelled".to_string());
                break;
            }

            summary.current_track = Some(format!("{} - {}", candidate.artist_name, candidate.title));
            summary.current_phase = Some("Enriching".to_string());
            progress_cb(&summary);

            let start_time = Instant::now();

            // 1. Check if track is manual
            if candidate.enrichment_status.as_deref() == Some("manual") {
                summary.processed_tracks += 1;
                summary.skipped_precedence_tracks += 1;
                summary.items.push(TrackEnrichmentReportItem {
                    track_id: candidate.id,
                    track_title: candidate.title.clone(),
                    artist_name: candidate.artist_name.clone(),
                    requested_fields: Vec::new(),
                    modified_fields: Vec::new(),
                    previous_provenance: HashMap::from([("all".to_string(), "manual".to_string())]),
                    new_provenance: HashMap::from([("all".to_string(), "manual".to_string())]),
                    provider: "manual".to_string(),
                    confidence: 1.0,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    error: None,
                    status: TrackEnrichmentStatus::SkippedPrecedence,
                });
                progress_cb(&summary);
                continue;
            }

            // 2. Identify requested fields
            let requested_fields = self.determine_requested_fields(&candidate, &mode);
            if requested_fields.is_empty() && mode != EnrichmentMode::RevalidateAll {
                summary.processed_tracks += 1;
                summary.skipped_complete_tracks += 1;
                summary.items.push(TrackEnrichmentReportItem {
                    track_id: candidate.id,
                    track_title: candidate.title.clone(),
                    artist_name: candidate.artist_name.clone(),
                    requested_fields: Vec::new(),
                    modified_fields: Vec::new(),
                    previous_provenance: HashMap::new(),
                    new_provenance: HashMap::new(),
                    provider: candidate.primary_service.clone().unwrap_or_else(|| "existing".to_string()),
                    confidence: 1.0,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    error: None,
                    status: TrackEnrichmentStatus::SkippedComplete,
                });
                progress_cb(&summary);
                continue;
            }

            // 3. Resolve metadata via MusicBrainz / providers with cache
            let enrichment_res = self.enrich_single_track(db, &candidate, &requested_fields).await;
            summary.processed_tracks += 1;

            match enrichment_res {
                Ok(item) => {
                    if !item.modified_fields.is_empty() {
                        summary.modified_tracks += 1;
                    } else if item.status == TrackEnrichmentStatus::SkippedComplete {
                        summary.skipped_complete_tracks += 1;
                    } else if item.status == TrackEnrichmentStatus::SkippedPrecedence {
                        summary.skipped_precedence_tracks += 1;
                    }
                    summary.items.push(item);
                }
                Err(err) => {
                    warn!(track_id = candidate.id, error = %err, "Track enrichment encountered error; continuing job");
                    summary.failed_tracks += 1;
                    summary.items.push(TrackEnrichmentReportItem {
                        track_id: candidate.id,
                        track_title: candidate.title.clone(),
                        artist_name: candidate.artist_name.clone(),
                        requested_fields,
                        modified_fields: Vec::new(),
                        previous_provenance: HashMap::new(),
                        new_provenance: HashMap::new(),
                        provider: "unknown".to_string(),
                        confidence: 0.0,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        error: Some(err),
                        status: TrackEnrichmentStatus::Failed,
                    });
                }
            }

            progress_cb(&summary);
        }

        if summary.status != JobStatus::Cancelled {
            summary.status = if summary.failed_tracks > 0 && summary.modified_tracks == 0 && summary.skipped_complete_tracks == 0 {
                JobStatus::Failed
            } else {
                JobStatus::Completed
            };
        }

        summary.current_track = None;
        summary.current_phase = Some("Finished".to_string());
        summary.completed_at = Some(chrono::Utc::now().to_rfc3339());

        {
            let mut guard = self.active_job.write().unwrap();
            *guard = Some(summary.clone());
        }
        progress_cb(&summary);

        Ok(summary)
    }

    /// Enriches a single track respecting provenance, precedence, and version derivation
    async fn enrich_single_track(
        &self,
        db: &SqlitePool,
        track: &CandidateTrackRow,
        requested_fields: &[String],
    ) -> Result<TrackEnrichmentReportItem, String> {
        let start_time = Instant::now();
        let mut modified_fields = Vec::new();
        let mut previous_provenance = HashMap::new();
        let mut new_provenance = HashMap::new();
        let mut provider_used = "none".to_string();
        let mut overall_confidence = 0.85;
        let mut track_error = None;

        // Populate previous provenance
        if let Some(ref p) = track.primary_service {
            previous_provenance.insert("source".to_string(), p.clone());
        }
        if let Some(ref mb) = track.musicbrainz_id {
            previous_provenance.insert("musicbrainz_id".to_string(), mb.clone());
        }

        // Query MusicBrainz with caching only if relevant fields are requested
        let needs_mb = requested_fields.iter().any(|f| {
            f == "musicbrainz_id" || f == "release_year" || f == "record_label" || f == "genre"
        });

        let mb_rec = if needs_mb {
            self.resolve_musicbrainz_recording(track).await
        } else {
            Ok(None)
        };

        let mut new_mbid: Option<String> = None;
        let mut new_year: Option<i32> = None;
        let mut new_label: Option<String> = None;
        let mut _new_country: Option<String> = None;
        let mut new_display_title: Option<String> = None;

        if let Ok(Some(rec)) = mb_rec {
            provider_used = "musicbrainz".to_string();
            overall_confidence = 0.90;

            // 1. MusicBrainz ID
            if track.musicbrainz_id.is_none() || track.musicbrainz_id.as_deref() == Some("NOT_FOUND") {
                new_mbid = Some(rec.id.clone());
            }

            // 2. Release metadata (Year, Label, Country)
            if let Some(ref releases) = rec.releases {
                let norm_album = syncify_metadata_domain::normalize_title(&track.album_name);
                let matched_rel = releases
                    .iter()
                    .find(|r| {
                        let t = syncify_metadata_domain::normalize_title(&r.title);
                        t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                    })
                    .or_else(|| releases.first());

                if let Some(rel) = matched_rel {
                    // Release year (only fill if empty or lower priority)
                    if (track.release_year.is_none() || track.release_year == Some(0))
                        && requested_fields.contains(&"release_year".to_string())
                    {
                        if let Some(ref dt) = rel.date {
                            if let Ok(yr) = dt.chars().take(4).collect::<String>().parse::<i32>() {
                                if FieldValidator::is_valid_year(&yr.to_string()) {
                                    new_year = Some(yr);
                                }
                            }
                        }
                    }

                    // Record Label
                    if track.record_label.is_none() && requested_fields.contains(&"record_label".to_string()) {
                        if let Some(ref l_info) = rel.label_info {
                            if let Some(first_l) = l_info.first().and_then(|li| li.label.as_ref()) {
                                if FieldValidator::is_valid_label(&first_l.name) {
                                    new_label = Some(first_l.name.clone());
                                }
                            }
                        }
                    }

                    // Country
                    if let Some(ref c) = rel.country {
                        if FieldValidator::is_valid_country(c) {
                            _new_country = Some(c.clone());
                        }
                    }
                }
            }
        } else if let Err(e) = mb_rec {
            track_error = Some(format!("MusicBrainz lookup warning: {}", e));
        }

        // 3. Version derivation check for display_title
        if requested_fields.contains(&"display_title".to_string()) || track.display_title.is_none() {
            let (is_dup, _): (i64, Option<String>) = sqlx::query_as(
                "SELECT COUNT(*), title FROM tracks WHERE album_id = ? AND title = ? AND id != ?"
            )
            .bind(track.album_id)
            .bind(&track.title)
            .bind(track.id)
            .fetch_one(db)
            .await
            .unwrap_or((0, None));

            let input = VersionDerivationInput {
                title: track.title.clone(),
                provider_version: None,
                musicbrainz_disambiguation: track.musicbrainz_id.clone(),
                performer_or_remixer_credit: None,
                comment_text: None,
                track_number: track.track_number,
                is_duplicate_title_in_album: is_dup > 0,
            };

            let derived = derive_track_version(&input);
            if derived.can_apply_to_catalog_and_disk() {
                if let Some(dt) = derived.display_title {
                    if track.display_title.as_deref() != Some(&dt) {
                        new_display_title = Some(dt);
                    }
                }
            }
        }

        // 4. Atomic database update inside transaction
        let mut tx = db.begin().await.map_err(|e| format!("DB tx begin failed: {}", e))?;

        if let Some(ref mbid) = new_mbid {
            sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                .bind(mbid)
                .bind(track.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update musicbrainz_id: {}", e))?;
            modified_fields.push("musicbrainz_id".to_string());
            new_provenance.insert("musicbrainz_id".to_string(), "musicbrainz".to_string());
        }

        if let Some(yr) = new_year {
            sqlx::query("UPDATE tracks SET release_year = ? WHERE id = ?")
                .bind(yr)
                .bind(track.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update release_year: {}", e))?;
            modified_fields.push("release_year".to_string());
            new_provenance.insert("release_year".to_string(), "musicbrainz".to_string());
        }

        if let Some(ref lbl) = new_label {
            sqlx::query("UPDATE tracks SET record_label = ? WHERE id = ?")
                .bind(lbl)
                .bind(track.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update record_label: {}", e))?;
            modified_fields.push("record_label".to_string());
            new_provenance.insert("record_label".to_string(), "musicbrainz".to_string());
        }

        if let Some(ref dt) = new_display_title {
            sqlx::query("UPDATE tracks SET display_title = ? WHERE id = ?")
                .bind(dt)
                .bind(track.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update display_title: {}", e))?;
            modified_fields.push("display_title".to_string());
            new_provenance.insert("display_title".to_string(), "derived_version".to_string());
        }

        if !modified_fields.is_empty() {
            sqlx::query("UPDATE tracks SET enrichment_status = 'enriched', enriched_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(track.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update enrichment status: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("DB tx commit failed: {}", e))?;

        let status = if !modified_fields.is_empty() {
            if modified_fields.len() >= requested_fields.len() {
                TrackEnrichmentStatus::Persisted
            } else {
                TrackEnrichmentStatus::Partial
            }
        } else if track_error.is_some() {
            TrackEnrichmentStatus::Failed
        } else {
            TrackEnrichmentStatus::SkippedComplete
        };

        Ok(TrackEnrichmentReportItem {
            track_id: track.id,
            track_title: track.title.clone(),
            artist_name: track.artist_name.clone(),
            requested_fields: requested_fields.to_vec(),
            modified_fields,
            previous_provenance,
            new_provenance,
            provider: provider_used,
            confidence: overall_confidence,
            duration_ms: start_time.elapsed().as_millis() as u64,
            error: track_error,
            status,
        })
    }

    /// Resolve MusicBrainz recording using in-memory cache
    async fn resolve_musicbrainz_recording(
        &self,
        track: &CandidateTrackRow,
    ) -> Result<Option<MusicBrainzRecording>, String> {
        // 1. Check ISRC cache
        if let Some(ref isrc) = track.isrc {
            if let Ok(guard) = self.cache.read() {
                if let Some(cached) = guard.isrc_cache.get(isrc) {
                    return Ok(cached.clone());
                }
            }

            match self.mb_client.lookup_by_isrc(isrc).await {
                Ok(res) => {
                    if let Ok(mut guard) = self.cache.write() {
                        guard.isrc_cache.insert(isrc.clone(), res.clone());
                    }
                    return Ok(res);
                }
                Err(e) => {
                    warn!("ISRC lookup failed for {}: {}", isrc, e);
                }
            }
        }

        // 2. Search recordings by title & artist
        let album_opt = if !track.album_name.is_empty() { Some(track.album_name.as_str()) } else { None };
        let results = self
            .mb_client
            .search_recordings(&track.title, &track.artist_name, album_opt, 1)
            .await
            .map_err(|e| e.to_string())?;

        let res = results.into_iter().next();
        if let Some(ref r) = res {
            if let Some(ref isrc) = track.isrc {
                if let Ok(mut guard) = self.cache.write() {
                    guard.isrc_cache.insert(isrc.clone(), Some(r.clone()));
                }
            }
        }

        Ok(res)
    }

    /// Fetch candidate rows from SQLite
    async fn fetch_candidate_rows(
        &self,
        db: &SqlitePool,
        mode: &EnrichmentMode,
        selection_ids: Option<&[i64]>,
    ) -> Result<Vec<CandidateTrackRow>, String> {
        let base_query = r#"
            SELECT 
                t.id, 
                t.title, 
                t.source_title, 
                t.display_title,
                COALESCE(ar.name, 'Unknown Artist') as artist_name, 
                COALESCE(ar.id, 0) as artist_id,
                COALESCE(al.title, '') as album_name, 
                COALESCE(t.album_id, 0) as album_id,
                t.track_number, 
                t.disc_number, 
                t.duration_ms,
                t.isrc, 
                t.musicbrainz_id, 
                t.release_year,
                t.genre, 
                t.subgenre, 
                t.record_label, 
                t.bpm, 
                t.musical_key,
                t.explicit, 
                t.enrichment_status,
                s.name as primary_service, 
                ts.service_track_id as primary_service_track_id
            FROM tracks t
            LEFT JOIN albums al ON t.album_id = al.id
            LEFT JOIN track_artists ta ON t.id = ta.track_id AND ta.role IN ('main', 'primary')
            LEFT JOIN artists ar ON ta.artist_id = ar.id
            LEFT JOIN track_sources ts ON t.id = ts.track_id
            LEFT JOIN services s ON ts.service_id = s.id
        "#;

        let incomplete_filter = r#"
            (t.isrc IS NULL 
             OR t.musicbrainz_id IS NULL 
             OR t.musicbrainz_id = 'NOT_FOUND' 
             OR t.release_year IS NULL 
             OR t.release_year = 0 
             OR t.genre IS NULL 
             OR t.record_label IS NULL 
             OR t.bpm IS NULL 
             OR t.musical_key IS NULL)
        "#;

        let rows = match (mode, selection_ids) {
            (EnrichmentMode::Selection, Some(ids)) if !ids.is_empty() => {
                let id_placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                let query = format!("{} WHERE t.id IN ({}) ORDER BY t.id ASC", base_query, id_placeholders);
                let mut q = sqlx::query_as::<_, CandidateTrackRow>(&query);
                for id in ids {
                    q = q.bind(id);
                }
                q.fetch_all(db).await.map_err(|e| format!("DB query failed: {}", e))?
            }
            (EnrichmentMode::IncompleteOnly, _) => {
                let query = format!("{} WHERE {} ORDER BY t.id ASC", base_query, incomplete_filter);
                sqlx::query_as::<_, CandidateTrackRow>(&query)
                    .fetch_all(db)
                    .await
                    .map_err(|e| format!("DB query failed: {}", e))?
            }
            _ => {
                let query = format!("{} ORDER BY t.id ASC", base_query);
                sqlx::query_as::<_, CandidateTrackRow>(&query)
                    .fetch_all(db)
                    .await
                    .map_err(|e| format!("DB query failed: {}", e))?
            }
        };

        Ok(rows)
    }

    /// Determines whether a track is missing essential enrichable fields
    fn is_track_incomplete(&self, track: &CandidateTrackRow) -> bool {
        track.isrc.is_none()
            || track.musicbrainz_id.is_none()
            || track.musicbrainz_id.as_deref() == Some("NOT_FOUND")
            || track.release_year.is_none()
            || track.release_year == Some(0)
            || track.genre.is_none()
            || track.record_label.is_none()
            || track.bpm.is_none()
            || track.musical_key.is_none()
    }

    /// Determines which fields should be queried for a given track
    fn determine_requested_fields(&self, track: &CandidateTrackRow, mode: &EnrichmentMode) -> Vec<String> {
        let mut fields = Vec::new();

        if *mode == EnrichmentMode::RevalidateAll {
            fields.push("isrc".to_string());
            fields.push("musicbrainz_id".to_string());
            fields.push("release_year".to_string());
            fields.push("genre".to_string());
            fields.push("record_label".to_string());
            fields.push("bpm".to_string());
            fields.push("musical_key".to_string());
            fields.push("display_title".to_string());
            return fields;
        }

        if track.isrc.is_none() {
            fields.push("isrc".to_string());
        }
        if track.musicbrainz_id.is_none() || track.musicbrainz_id.as_deref() == Some("NOT_FOUND") {
            fields.push("musicbrainz_id".to_string());
        }
        if track.release_year.is_none() || track.release_year == Some(0) {
            fields.push("release_year".to_string());
        }
        if track.genre.is_none() {
            fields.push("genre".to_string());
        }
        if track.record_label.is_none() {
            fields.push("record_label".to_string());
        }
        if track.bpm.is_none() {
            fields.push("bpm".to_string());
        }
        if track.musical_key.is_none() {
            fields.push("musical_key".to_string());
        }

        fields
    }
}

/// Internal database row structure for candidate tracks
#[derive(Debug, sqlx::FromRow)]
pub struct CandidateTrackRow {
    pub id: i64,
    pub title: String,
    pub source_title: Option<String>,
    pub display_title: Option<String>,
    pub artist_name: String,
    pub artist_id: i64,
    pub album_name: String,
    pub album_id: i64,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub release_year: Option<i32>,
    pub genre: Option<String>,
    pub subgenre: Option<String>,
    pub record_label: Option<String>,
    pub bpm: Option<f64>,
    pub musical_key: Option<String>,
    pub explicit: Option<i64>,
    pub enrichment_status: Option<String>,
    pub primary_service: Option<String>,
    pub primary_service_track_id: Option<String>,
}
