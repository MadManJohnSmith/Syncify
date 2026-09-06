// Progress tracking for downloads

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Download status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    /// Waiting in queue
    Queued,
    /// Searching for track on services
    Searching,
    /// Downloading audio file
    Downloading,
    /// Embedding metadata and artwork
    Finalizing,
    /// Download complete
    Complete,
    /// Download failed
    Failed,
    /// Download cancelled by user
    Cancelled,
}

/// Progress information for a download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub item_id: String,
    pub bytes_downloaded: u64,
    pub bytes_total: u64,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub percent: Option<f32>,
    pub status: DownloadStatus,
    pub service: Option<String>,
    pub message: Option<String>,
    #[serde(default)]
    pub instant_kbps: f64,
    #[serde(default)]
    pub average_kbps: f64,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub terminal: bool,
}

#[allow(dead_code)]
impl DownloadProgress {
    pub fn new(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            total_bytes: None,
            percent: Some(0.0),
            status: DownloadStatus::Queued,
            service: None,
            message: None,
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "queued".to_string(),
            terminal: false,
        }
    }

    pub fn searching(item_id: &str, service: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            total_bytes: None,
            percent: Some(0.0),
            status: DownloadStatus::Searching,
            service: Some(service.to_string()),
            message: Some(format!("Searching on {}", service)),
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "searching".to_string(),
            terminal: false,
        }
    }

    pub fn downloading(item_id: &str, service: &str, bytes: u64, total: u64) -> Self {
        let (total_opt, percent_opt) = if total > 0 {
            (Some(total), Some(((bytes as f32 / total as f32) * 100.0).min(100.0)))
        } else {
            (None, None)
        };
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: bytes,
            bytes_total: total,
            total_bytes: total_opt,
            percent: percent_opt,
            status: DownloadStatus::Downloading,
            service: Some(service.to_string()),
            message: None,
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "downloading".to_string(),
            terminal: false,
        }
    }

    pub fn downloading_bytes(
        item_id: &str,
        service: &str,
        bytes: u64,
        total_opt: Option<u64>,
        instant_kbps: f64,
        avg_kbps: f64,
    ) -> Self {
        let percent_opt = total_opt.and_then(|tot| {
            if tot > 0 {
                Some(((bytes as f32 / tot as f32) * 100.0).min(100.0))
            } else {
                None
            }
        });
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: bytes,
            bytes_total: total_opt.unwrap_or(0),
            total_bytes: total_opt,
            percent: percent_opt,
            status: DownloadStatus::Downloading,
            service: Some(service.to_string()),
            message: None,
            instant_kbps,
            average_kbps: avg_kbps,
            phase: "downloading".to_string(),
            terminal: false,
        }
    }

    pub fn finalizing(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            total_bytes: None,
            percent: Some(100.0),
            status: DownloadStatus::Finalizing,
            service: None,
            message: Some("Embedding metadata...".to_string()),
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "finalizing".to_string(),
            terminal: false,
        }
    }

    pub fn complete(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            total_bytes: None,
            percent: Some(100.0),
            status: DownloadStatus::Complete,
            service: None,
            message: None,
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "complete".to_string(),
            terminal: true,
        }
    }

    pub fn failed(item_id: &str, error: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            total_bytes: None,
            percent: None,
            status: DownloadStatus::Failed,
            service: None,
            message: Some(error.to_string()),
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "failed".to_string(),
            terminal: true,
        }
    }

    pub fn cancelled(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            total_bytes: None,
            percent: None,
            status: DownloadStatus::Cancelled,
            service: None,
            message: Some("Download cancelled by user".to_string()),
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: "cancelled".to_string(),
            terminal: true,
        }
    }

    pub fn phase_update(
        item_id: &str,
        service: Option<&str>,
        phase: DownloadPhase,
        message: Option<&str>,
    ) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            total_bytes: None,
            percent: None,
            status: match phase {
                DownloadPhase::Completed => DownloadStatus::Complete,
                DownloadPhase::Failed => DownloadStatus::Failed,
                DownloadPhase::Cancelled => DownloadStatus::Cancelled,
                _ => DownloadStatus::Downloading,
            },
            service: service.map(|s| s.to_string()),
            message: message.map(|m| m.to_string()),
            instant_kbps: 0.0,
            average_kbps: 0.0,
            phase: phase.as_str().to_string(),
            terminal: matches!(
                phase,
                DownloadPhase::Completed | DownloadPhase::Failed | DownloadPhase::Cancelled
            ),
        }
    }
}

/// Helper to track byte-level streaming telemetry with throttling and throughput calculation
pub struct ByteStreamTracker {
    pub item_id: String,
    pub service: String,
    pub total_bytes: Option<u64>,
    pub start_time: Instant,
    pub last_sample_time: Instant,
    pub last_sample_bytes: u64,
    pub last_emit_time: Instant,
    pub last_instant_kbps: f64,
    pub throttle_duration: std::time::Duration,
}

impl ByteStreamTracker {
    pub fn new(item_id: &str, service: &str, total_bytes: Option<u64>) -> Self {
        let now = Instant::now();
        Self {
            item_id: item_id.to_string(),
            service: service.to_string(),
            total_bytes,
            start_time: now,
            last_sample_time: now,
            last_sample_bytes: 0,
            last_emit_time: now,
            last_instant_kbps: 0.0,
            throttle_duration: std::time::Duration::from_millis(250), // 4 updates/sec max
        }
    }

    /// Process chunk reception, calculate instant/average throughput, and return progress if throttle window elapsed
    pub fn on_bytes(&mut self, current_downloaded: u64, force: bool) -> Option<DownloadProgress> {
        let now = Instant::now();
        let elapsed_total = now.duration_since(self.start_time).as_secs_f64();
        let average_kbps = if elapsed_total > 0.001 {
            (current_downloaded as f64 / 1024.0) / elapsed_total
        } else {
            0.0
        };

        let sample_elapsed = now.duration_since(self.last_sample_time).as_secs_f64();
        if sample_elapsed >= 0.25 {
            let delta_bytes = current_downloaded.saturating_sub(self.last_sample_bytes);
            self.last_instant_kbps = (delta_bytes as f64 / 1024.0) / sample_elapsed;
            self.last_sample_time = now;
            self.last_sample_bytes = current_downloaded;
        }

        let time_since_last_emit = now.duration_since(self.last_emit_time);
        if force || time_since_last_emit >= self.throttle_duration {
            self.last_emit_time = now;
            Some(DownloadProgress::downloading_bytes(
                &self.item_id,
                &self.service,
                current_downloaded,
                self.total_bytes,
                self.last_instant_kbps,
                average_kbps,
            ))
        } else {
            None
        }
    }
}

/// Result of a successful download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    pub file_path: String,
    pub bit_depth: i32,
    pub sample_rate: i32,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub release_date: Option<String>,
    pub track_number: i32,
    pub disc_number: i32,
    pub isrc: Option<String>,
    pub service: String,
    #[serde(default)]
    pub origin_service: Option<String>,
    #[serde(default)]
    pub origin_service_track_id: Option<String>,
    #[serde(default)]
    pub effective_service: Option<String>,
    #[serde(default)]
    pub effective_service_track_id: Option<String>,
    #[serde(default)]
    pub fallback_reason: Option<String>,
    #[serde(default)]
    pub match_method: Option<String>,
    #[serde(default)]
    pub match_confidence: Option<f64>,
    #[serde(default)]
    pub phase_timings: Option<DownloadPhaseTimings>,
    #[serde(default)]
    pub quality_decision: Option<syncify_core_domain::quality::QualityDecision>,
    #[serde(default)]
    pub channels: Option<i32>,
    #[serde(default)]
    pub bitrate: Option<i32>,
}

/// Download Phase state machine according to strict contract
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum DownloadPhase {
    QueueWait,
    Auth,
    ResolveStream,
    Transfer,
    ValidateAudio,
    EnrichMetadata,
    ResolveLyrics,
    ResolveCover,
    Tagging,
    Promotion,
    Persisting,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::QueueWait => "QueueWait",
            Self::Auth => "Auth",
            Self::ResolveStream => "ResolveStream",
            Self::Transfer => "Transfer",
            Self::ValidateAudio => "ValidateAudio",
            Self::EnrichMetadata => "EnrichMetadata",
            Self::ResolveLyrics => "ResolveLyrics",
            Self::ResolveCover => "ResolveCover",
            Self::Tagging => "Tagging",
            Self::Promotion => "Promotion",
            Self::Persisting => "Persisting",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        }
    }
}

impl std::fmt::Display for DownloadPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Chronological record of an individual download phase execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DownloadPhaseRecord {
    pub phase: DownloadPhase,
    pub start_ms: u64,
    pub end_ms: u64,
    pub duration_ms: u64,
}

/// Cache hit report across auxiliary enrichment pipelines
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CacheHitReport {
    #[serde(default)]
    pub lyrics_hit: bool,
    #[serde(default)]
    pub cover_hit: bool,
    #[serde(default)]
    pub metadata_hit: bool,
}

fn default_transfer_source() -> String {
    "network".to_string()
}

/// Detailed benchmark phase timings for download execution
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DownloadPhaseTimings {
    #[serde(default)]
    pub queue_wait_ms: u64,
    #[serde(default)]
    pub auth_ms: u64,
    #[serde(default)]
    pub resolve_stream_ms: u64,
    #[serde(default)]
    pub transfer_ms: u64,
    pub stream_duration_ms: u64,
    #[serde(default)]
    pub validate_audio_ms: u64,
    #[serde(default)]
    pub metadata_duration_ms: u64,
    #[serde(default)]
    pub lyrics_duration_ms: u64,
    #[serde(default)]
    pub cover_duration_ms: u64,
    #[serde(default)]
    pub tagging_duration_ms: u64,
    #[serde(default)]
    pub promotion_duration_ms: u64,
    #[serde(default)]
    pub persisting_duration_ms: u64,
    pub total_duration_ms: u64,
    #[serde(default = "default_transfer_source")]
    pub transfer_source: String,
    #[serde(default)]
    pub bytes_transferred: u64,
    #[serde(default)]
    pub throughput_mibps: f64,
    #[serde(default)]
    pub cache_hits: CacheHitReport,
    #[serde(default)]
    pub phases: Vec<DownloadPhaseRecord>,
}

/// Helper to track phases monotonically and generate verified telemetry
pub struct DownloadPhaseTracker {
    pub origin_instant: Instant,
    pub current_phase: Option<(DownloadPhase, Instant)>,
    pub phases: Vec<DownloadPhaseRecord>,
    pub queue_wait_ms: u64,
    pub auth_ms: u64,
    pub resolve_stream_ms: u64,
    pub transfer_ms: u64,
    pub validate_audio_ms: u64,
    pub metadata_duration_ms: u64,
    pub lyrics_duration_ms: u64,
    pub cover_duration_ms: u64,
    pub tagging_duration_ms: u64,
    pub promotion_duration_ms: u64,
    pub persisting_duration_ms: u64,
    pub transfer_source: String,
    pub bytes_transferred: u64,
    pub cache_hits: CacheHitReport,
}

impl DownloadPhaseTracker {
    pub fn new() -> Self {
        Self {
            origin_instant: Instant::now(),
            current_phase: None,
            phases: Vec::new(),
            queue_wait_ms: 0,
            auth_ms: 0,
            resolve_stream_ms: 0,
            transfer_ms: 0,
            validate_audio_ms: 0,
            metadata_duration_ms: 0,
            lyrics_duration_ms: 0,
            cover_duration_ms: 0,
            tagging_duration_ms: 0,
            promotion_duration_ms: 0,
            persisting_duration_ms: 0,
            transfer_source: "network".to_string(),
            bytes_transferred: 0,
            cache_hits: CacheHitReport::default(),
        }
    }

    #[allow(dead_code)] // telemetría de progreso global: cubierta por queue_global_progress_test; consumo UI pendiente
    pub fn with_queue_wait(queue_wait_ms: u64) -> Self {
        let mut tracker = Self::new();
        tracker.queue_wait_ms = queue_wait_ms;
        if queue_wait_ms > 0 {
            tracker.phases.push(DownloadPhaseRecord {
                phase: DownloadPhase::QueueWait,
                start_ms: 0,
                end_ms: queue_wait_ms,
                duration_ms: queue_wait_ms,
            });
        }
        tracker
    }

    pub fn start_phase(&mut self, phase: DownloadPhase) {
        let now = Instant::now();
        if let Some((prev_phase, prev_start)) = self.current_phase.take() {
            let elapsed_ms = now.duration_since(prev_start).as_millis() as u64;
            let start_offset = prev_start.duration_since(self.origin_instant).as_millis() as u64;
            let end_offset = now.duration_since(self.origin_instant).as_millis() as u64;
            
            self.record_phase_duration(prev_phase, elapsed_ms);
            self.phases.push(DownloadPhaseRecord {
                phase: prev_phase,
                start_ms: start_offset,
                end_ms: end_offset,
                duration_ms: elapsed_ms,
            });
        }
        self.current_phase = Some((phase, now));
    }

    pub fn end_current_phase(&mut self) {
        let now = Instant::now();
        if let Some((prev_phase, prev_start)) = self.current_phase.take() {
            let elapsed_ms = now.duration_since(prev_start).as_millis() as u64;
            let start_offset = prev_start.duration_since(self.origin_instant).as_millis() as u64;
            let end_offset = now.duration_since(self.origin_instant).as_millis() as u64;

            self.record_phase_duration(prev_phase, elapsed_ms);
            self.phases.push(DownloadPhaseRecord {
                phase: prev_phase,
                start_ms: start_offset,
                end_ms: end_offset,
                duration_ms: elapsed_ms,
            });
        }
    }

    fn record_phase_duration(&mut self, phase: DownloadPhase, dur_ms: u64) {
        match phase {
            DownloadPhase::QueueWait => self.queue_wait_ms = self.queue_wait_ms.max(dur_ms),
            DownloadPhase::Auth => self.auth_ms += dur_ms,
            DownloadPhase::ResolveStream => self.resolve_stream_ms += dur_ms,
            DownloadPhase::Transfer => {
                // If bytes transferred > 0, ensure transfer duration is strictly > 0 ms
                let final_dur = if self.bytes_transferred > 0 && dur_ms == 0 { 1 } else { dur_ms };
                self.transfer_ms += final_dur;
            }
            DownloadPhase::ValidateAudio => self.validate_audio_ms += dur_ms,
            DownloadPhase::EnrichMetadata => self.metadata_duration_ms += dur_ms,
            DownloadPhase::ResolveLyrics => self.lyrics_duration_ms += dur_ms,
            DownloadPhase::ResolveCover => self.cover_duration_ms += dur_ms,
            DownloadPhase::Tagging => self.tagging_duration_ms += dur_ms,
            DownloadPhase::Promotion => self.promotion_duration_ms += dur_ms,
            DownloadPhase::Persisting => self.persisting_duration_ms += dur_ms,
            _ => {}
        }
    }

    pub fn set_transfer_metrics(&mut self, bytes: u64, source: &str) {
        self.bytes_transferred = bytes;
        self.transfer_source = source.to_string();
    }

    pub fn set_cache_hits(&mut self, lyrics: bool, cover: bool, metadata: bool) {
        self.cache_hits = CacheHitReport {
            lyrics_hit: lyrics,
            cover_hit: cover,
            metadata_hit: metadata,
        };
    }

    pub fn finish_completed(&mut self) -> DownloadPhaseTimings {
        self.start_phase(DownloadPhase::Completed);
        self.end_current_phase();

        let total_wall_ms = self.origin_instant.elapsed().as_millis() as u64 + self.queue_wait_ms;
        let effective_transfer_ms = if self.bytes_transferred > 0 && self.transfer_ms == 0 {
            1
        } else {
            self.transfer_ms
        };

        let throughput_mibps = if effective_transfer_ms > 0 && self.bytes_transferred > 0 {
            let mib = self.bytes_transferred as f64 / 1_048_576.0;
            let sec = effective_transfer_ms as f64 / 1000.0;
            mib / sec
        } else {
            0.0
        };

        DownloadPhaseTimings {
            queue_wait_ms: self.queue_wait_ms,
            auth_ms: self.auth_ms,
            resolve_stream_ms: self.resolve_stream_ms,
            transfer_ms: effective_transfer_ms,
            stream_duration_ms: effective_transfer_ms,
            validate_audio_ms: self.validate_audio_ms,
            metadata_duration_ms: self.metadata_duration_ms,
            lyrics_duration_ms: self.lyrics_duration_ms,
            cover_duration_ms: self.cover_duration_ms,
            tagging_duration_ms: self.tagging_duration_ms,
            promotion_duration_ms: self.promotion_duration_ms,
            persisting_duration_ms: self.persisting_duration_ms,
            total_duration_ms: total_wall_ms.max(
                self.queue_wait_ms
                    + self.auth_ms
                    + self.resolve_stream_ms
                    + effective_transfer_ms
                    + self.validate_audio_ms
                    + self.metadata_duration_ms
                    + self.lyrics_duration_ms
                    + self.cover_duration_ms
                    + self.tagging_duration_ms
                    + self.promotion_duration_ms
                    + self.persisting_duration_ms
            ),
            transfer_source: self.transfer_source.clone(),
            bytes_transferred: self.bytes_transferred,
            throughput_mibps,
            cache_hits: self.cache_hits.clone(),
            phases: self.phases.clone(),
        }
    }

    #[allow(dead_code)] // telemetría de fases: cubierta por download_phase_telemetry_test
    pub fn finish_failed(&mut self) -> DownloadPhaseTimings {
        self.start_phase(DownloadPhase::Failed);
        self.end_current_phase();

        let total_wall_ms = self.origin_instant.elapsed().as_millis() as u64 + self.queue_wait_ms;
        DownloadPhaseTimings {
            queue_wait_ms: self.queue_wait_ms,
            auth_ms: self.auth_ms,
            resolve_stream_ms: self.resolve_stream_ms,
            transfer_ms: self.transfer_ms,
            stream_duration_ms: self.transfer_ms,
            validate_audio_ms: self.validate_audio_ms,
            metadata_duration_ms: self.metadata_duration_ms,
            lyrics_duration_ms: self.lyrics_duration_ms,
            cover_duration_ms: self.cover_duration_ms,
            tagging_duration_ms: self.tagging_duration_ms,
            promotion_duration_ms: self.promotion_duration_ms,
            persisting_duration_ms: self.persisting_duration_ms,
            total_duration_ms: total_wall_ms,
            transfer_source: self.transfer_source.clone(),
            bytes_transferred: self.bytes_transferred,
            throughput_mibps: 0.0,
            cache_hits: self.cache_hits.clone(),
            phases: self.phases.clone(),
        }
    }

    #[allow(dead_code)] // telemetría de fases: cubierta por download_phase_telemetry_test
    pub fn finish_cancelled(&mut self) -> DownloadPhaseTimings {
        self.start_phase(DownloadPhase::Cancelled);
        self.end_current_phase();

        let total_wall_ms = self.origin_instant.elapsed().as_millis() as u64 + self.queue_wait_ms;
        DownloadPhaseTimings {
            queue_wait_ms: self.queue_wait_ms,
            auth_ms: self.auth_ms,
            resolve_stream_ms: self.resolve_stream_ms,
            transfer_ms: self.transfer_ms,
            stream_duration_ms: self.transfer_ms,
            validate_audio_ms: self.validate_audio_ms,
            metadata_duration_ms: self.metadata_duration_ms,
            lyrics_duration_ms: self.lyrics_duration_ms,
            cover_duration_ms: self.cover_duration_ms,
            tagging_duration_ms: self.tagging_duration_ms,
            promotion_duration_ms: self.promotion_duration_ms,
            persisting_duration_ms: self.persisting_duration_ms,
            total_duration_ms: total_wall_ms,
            transfer_source: self.transfer_source.clone(),
            bytes_transferred: self.bytes_transferred,
            throughput_mibps: 0.0,
            cache_hits: self.cache_hits.clone(),
            phases: self.phases.clone(),
        }
    }
}

/// Global progress calculation state with stable denominator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)] // telemetría de progreso global: cubierta por queue_global_progress_test; consumo UI pendiente
pub struct QueueGlobalProgress {
    pub total_selected: usize,
    pub preflight_excluded: usize,
    pub initial_eligible_total: usize,
    pub pending: usize,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub skipped: usize,
    pub active_fraction: f64,
    pub progress_percent: f64,
    pub transfer_throughput_bps: f64,
    pub eta_seconds: Option<u64>,
}

impl QueueGlobalProgress {
    #[allow(dead_code)] // telemetría de progreso global: cubierta por queue_global_progress_test; consumo UI pendiente
    pub fn compute(
        total_selected: usize,
        preflight_excluded: usize,
        initial_eligible_total: usize,
        pending: usize,
        active: usize,
        completed: usize,
        failed: usize,
        cancelled: usize,
        skipped: usize,
        active_fraction: f64,
        real_transfer_throughput_bps: f64,
        remaining_bytes: u64,
    ) -> Self {
        let progress_percent = if initial_eligible_total == 0 {
            if total_selected > 0 { 100.0 } else { 0.0 }
        } else {
            let numerator = (completed as f64) + (failed as f64) + (skipped as f64) + active_fraction.clamp(0.0, active as f64);
            ((numerator / (initial_eligible_total as f64)) * 100.0).clamp(0.0, 100.0)
        };

        let eta_seconds = if active == 0 && pending == 0 {
            Some(0)
        } else if real_transfer_throughput_bps > 0.0 && remaining_bytes > 0 {
            Some((remaining_bytes as f64 / real_transfer_throughput_bps).ceil() as u64)
        } else {
            None
        };

        Self {
            total_selected,
            preflight_excluded,
            initial_eligible_total,
            pending,
            active,
            completed,
            failed,
            cancelled,
            skipped,
            active_fraction,
            progress_percent,
            transfer_throughput_bps: real_transfer_throughput_bps,
            eta_seconds,
        }
    }
}

/// Request to download a track with explicit source identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub item_id: String,
    #[serde(default)]
    pub canonical_track_id: Option<i64>,
    #[serde(default)]
    pub queue_id: Option<i64>,
    #[serde(default)]
    pub operation_id: Option<String>,
    pub isrc: Option<String>,
    #[serde(default)]
    pub musicbrainz_recording_id: Option<String>,
    #[serde(default)]
    pub acoustid_fingerprint: Option<String>,
    pub spotify_id: Option<String>,
    pub service_name: Option<String>,
    pub service_track_id: Option<String>,
    pub service_album_id: Option<String>,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub album_artist: Option<String>,
    pub duration_ms: i64,
    pub track_number: i32,
    pub disc_number: i32,
    pub total_tracks: i32,
    pub release_date: Option<String>,
    pub cover_url: Option<String>,
    pub output_dir: String,
    pub quality: String,
    pub embed_lyrics: bool,
    pub embed_artwork: bool,
    pub smart_studio_origin: bool,
    pub allow_fallback: bool,
    #[serde(default)]
    pub strict_quality: bool,
}

impl Default for DownloadRequest {
    fn default() -> Self {
        Self {
            item_id: String::new(),
            canonical_track_id: None,
            queue_id: None,
            operation_id: None,
            isrc: None,
            musicbrainz_recording_id: None,
            acoustid_fingerprint: None,
            spotify_id: None,
            service_name: None,
            service_track_id: None,
            service_album_id: None,
            track_name: String::new(),
            artist_name: String::new(),
            album_name: String::new(),
            album_artist: None,
            duration_ms: 0,
            track_number: 1,
            disc_number: 1,
            total_tracks: 1,
            release_date: None,
            cover_url: None,
            output_dir: String::new(),
            quality: "HI_RES_LOSSLESS".to_string(),
            embed_lyrics: false,
            embed_artwork: false,
            smart_studio_origin: false,
            allow_fallback: false,
            strict_quality: false,
        }
    }
}

impl Default for DownloadResult {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            bit_depth: 16,
            sample_rate: 44100,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            release_date: None,
            track_number: 1,
            disc_number: 1,
            isrc: None,
            service: String::new(),
            origin_service: None,
            origin_service_track_id: None,
            effective_service: None,
            effective_service_track_id: None,
            fallback_reason: None,
            match_method: None,
            match_confidence: None,
            phase_timings: None,
            quality_decision: None,
            channels: None,
            bitrate: None,
        }
    }
}

type ProgressEmitterFn = Arc<dyn Fn(&DownloadProgress) + Send + Sync>;

/// Global progress tracker
pub struct ProgressTracker {
    items: RwLock<HashMap<String, (DownloadProgress, Instant)>>,
    emitter: RwLock<Option<ProgressEmitterFn>>,
}

#[allow(dead_code)]
impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
            emitter: RwLock::new(None),
        }
    }

    pub fn set_emitter<F>(&self, emitter: F)
    where
        F: Fn(&DownloadProgress) + Send + Sync + 'static,
    {
        let mut guard = match self.emitter.write() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(Arc::new(emitter));
    }

    pub fn clear_emitter(&self) {
        let mut guard = match self.emitter.write() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
    }

    pub fn init(&self, item_id: &str) {
        let mut items = match self.items.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("ProgressTracker items lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let p = DownloadProgress::new(item_id);
        items.insert(item_id.to_string(), (p.clone(), Instant::now()));

        let emitter_opt = match self.emitter.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        if let Some(emitter) = emitter_opt {
            emitter(&p);
        }
    }

    pub fn update(&self, progress: DownloadProgress) {
        {
            let mut items = match self.items.write() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("ProgressTracker items lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            items.insert(progress.item_id.clone(), (progress.clone(), Instant::now()));
        }

        let emitter_opt = match self.emitter.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        if let Some(emitter) = emitter_opt {
            emitter(&progress);
        }
    }

    pub fn get(&self, item_id: &str) -> Option<DownloadProgress> {
        let items = match self.items.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("ProgressTracker items lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        items.get(item_id).map(|(p, _)| p.clone())
    }

    pub fn get_all(&self) -> Vec<DownloadProgress> {
        let items = match self.items.read() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("ProgressTracker items lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        items.values().map(|(p, _)| p.clone()).collect()
    }

    pub fn remove(&self, item_id: &str) {
        let mut items = match self.items.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("ProgressTracker items lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        items.remove(item_id);
    }

    pub fn clear_completed(&self) {
        let mut items = match self.items.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("ProgressTracker items lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        items.retain(|_, (p, _)| {
            p.status != DownloadStatus::Complete && p.status != DownloadStatus::Failed
        });
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    pub static ref PROGRESS_TRACKER: ProgressTracker = ProgressTracker::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_tracker() {
        let tracker = ProgressTracker::new();
        tracker.init("test-1");

        let progress = tracker.get("test-1").unwrap();
        assert_eq!(progress.status, DownloadStatus::Queued);

        tracker.update(DownloadProgress::downloading(
            "test-1", "qobuz", 1000, 10000,
        ));
        let progress = tracker.get("test-1").unwrap();
        assert_eq!(progress.percent, Some(10.0));

        tracker.remove("test-1");
        assert!(tracker.get("test-1").is_none());
    }

    #[test]
    fn test_stream_without_content_length() {
        let mut stream_tracker = ByteStreamTracker::new("test-no-len", "qobuz", None);
        let progress_opt = stream_tracker.on_bytes(50_000, true);
        assert!(progress_opt.is_some());
        let progress = progress_opt.unwrap();
        assert_eq!(progress.total_bytes, None);
        assert_eq!(progress.percent, None);
        assert_eq!(progress.bytes_downloaded, 50_000);
    }
}

