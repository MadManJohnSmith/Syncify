//! Shared types for Syncify Tauri commands
//!
//! Contains all response types and progress event structures used across command modules.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt;

// ==============================================
// ERROR TYPES
// ==============================================

/// Standard error type for service operations
#[derive(Debug)]
pub enum ServiceError {
    /// Account not connected or not found
    NotConnected(String),
    /// Invalid or expired credentials
    InvalidCredentials(String),
    /// Database operation failed
    Database(String),
    /// Encryption/decryption failed
    Crypto(String),
    /// Network request failed
    Network(String),
    /// Service-specific API error
    Api(String),
    /// General error with message
    Other(String),
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceError::NotConnected(s) => write!(f, "{} account not connected", s),
            ServiceError::InvalidCredentials(s) => write!(f, "Invalid credentials: {}", s),
            ServiceError::Database(s) => write!(f, "Database error: {}", s),
            ServiceError::Crypto(s) => write!(f, "Encryption error: {}", s),
            ServiceError::Network(s) => write!(f, "Network error: {}", s),
            ServiceError::Api(s) => write!(f, "API error: {}", s),
            ServiceError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for ServiceError {}

impl From<ServiceError> for String {
    fn from(err: ServiceError) -> String {
        err.to_string()
    }
}

impl From<sqlx::Error> for ServiceError {
    fn from(err: sqlx::Error) -> Self {
        ServiceError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for ServiceError {
    fn from(err: serde_json::Error) -> Self {
        ServiceError::InvalidCredentials(err.to_string())
    }
}

// Result type alias for service operations
// pub type ServiceResult<T> = Result<T, ServiceError>;

// ==============================================
// PROGRESS EVENT TYPES
// ==============================================

/// Progress event for downloads, scans, etc.
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub operation: String,               // "download", "scan", "import", "organize"
    pub id: String,                      // unique identifier for this operation
    pub status: String,                  // "started", "progress", "completed", "failed"
    pub current: u64,                    // current progress
    pub total: u64,                      // total items
    pub percentage: f64,                 // 0.0 - 100.0
    pub message: Option<String>,         // human-readable status
    pub data: Option<serde_json::Value>, // additional context
}

impl ProgressEvent {
    pub fn new(operation: &str, id: &str) -> Self {
        Self {
            operation: operation.to_string(),
            id: id.to_string(),
            status: "started".to_string(),
            current: 0,
            total: 0,
            percentage: 0.0,
            message: None,
            data: None,
        }
    }

    pub fn progress(mut self, current: u64, total: u64, message: &str) -> Self {
        self.status = "progress".to_string();
        self.current = current;
        self.total = total;
        self.percentage = if total > 0 {
            (current as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        self.message = Some(message.to_string());
        self
    }

    pub fn completed(mut self, message: &str) -> Self {
        self.status = "completed".to_string();
        self.percentage = 100.0;
        self.message = Some(message.to_string());
        self
    }

    pub fn failed(mut self, error: &str) -> Self {
        self.status = "failed".to_string();
        self.message = Some(error.to_string());
        self
    }
}

/// Unified Progress Event for service synchronizations (S128B)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncProgressEvent {
    pub service: String,
    pub account_id: Option<i64>,
    pub operation: String,
    pub phase: String,
    pub current: u64,
    pub total: Option<u64>,
    pub message: String,
    pub imported_tracks_total: u64,
    pub favorite_tracks_total: u64,
    pub terminal: bool,
    pub status: String, // "running" | "completed" | "failed" | "requires_auth"
}

impl SyncProgressEvent {
    #[allow(dead_code)]
    pub fn new(service: impl Into<String>, account_id: Option<i64>, operation: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            account_id,
            operation: operation.into(),
            phase: "authenticating".to_string(),
            current: 0,
            total: None,
            message: String::new(),
            imported_tracks_total: 0,
            favorite_tracks_total: 0,
            terminal: false,
            status: "running".to_string(),
        }
    }

    pub fn running(
        service: &str,
        account_id: Option<i64>,
        phase: &str,
        current: u64,
        total: Option<u64>,
        message: impl Into<String>,
        imported_tracks_total: u64,
        favorite_tracks_total: u64,
    ) -> Self {
        Self {
            service: service.to_string(),
            account_id,
            operation: "sync".to_string(),
            phase: phase.to_string(),
            current,
            total,
            message: message.into(),
            imported_tracks_total,
            favorite_tracks_total,
            terminal: false,
            status: "running".to_string(),
        }
    }

    pub fn completed(
        service: &str,
        account_id: Option<i64>,
        message: impl Into<String>,
        imported_tracks_total: u64,
        favorite_tracks_total: u64,
        total: Option<u64>,
    ) -> Self {
        Self {
            service: service.to_string(),
            account_id,
            operation: "sync".to_string(),
            phase: "completed".to_string(),
            current: total.unwrap_or(imported_tracks_total),
            total,
            message: message.into(),
            imported_tracks_total,
            favorite_tracks_total,
            terminal: true,
            status: "completed".to_string(),
        }
    }

    pub fn failed(
        service: &str,
        account_id: Option<i64>,
        phase: &str,
        message: impl Into<String>,
        imported_tracks_total: u64,
        favorite_tracks_total: u64,
    ) -> Self {
        Self {
            service: service.to_string(),
            account_id,
            operation: "sync".to_string(),
            phase: phase.to_string(),
            current: 0,
            total: None,
            message: message.into(),
            imported_tracks_total,
            favorite_tracks_total,
            terminal: true,
            status: "failed".to_string(),
        }
    }

    pub fn requires_auth(
        service: &str,
        account_id: Option<i64>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            service: service.to_string(),
            account_id,
            operation: "sync".to_string(),
            phase: "requires_auth".to_string(),
            current: 0,
            total: None,
            message: message.into(),
            imported_tracks_total: 0,
            favorite_tracks_total: 0,
            terminal: true,
            status: "requires_auth".to_string(),
        }
    }
}

// ==============================================
// RESPONSE TYPES
// ==============================================

/// Detailed track source availability status
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrackSourceAvailability {
    pub id: i64,
    pub track_id: i64,
    pub service_id: i64,
    pub service_name: String,
    pub service_track_id: String,
    pub format: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<i32>,
    pub quality_score: Option<i32>,
    pub available: i64,
    pub availability_status: String, // "available" | "stale_404" | "region_unavailable" | "requires_auth" | "unknown_unchecked"
    pub availability_reason: Option<String>,
    pub last_checked: Option<String>,
}

/// Track with artist info for UI display
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LibraryTrack {
    pub id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub artist_id: Option<i64>,
    pub album_name: Option<String>,
    pub album_id: Option<i64>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub services: Option<String>,        // Comma-separated service names (historical or all linked)
    pub imported_from: Option<String>,   // Historical import provenance (e.g. "Spotify", "Qobuz")
    pub downloaded_from: Option<String>, // Effective download provider (e.g. "Tidal")
    pub available_services: Option<String>, // Services verified available
    pub availability_summary: Option<String>, // JSON or summary of source statuses
    pub quality: Option<String>,         // e.g. "24/96", "16/44.1", "320kbps"
    pub download_status: Option<String>, // "downloaded", "queued", "not_downloaded"
    pub metadata_score: Option<i32>,     // 0-100 based on field completeness
    pub lyrics_type: Option<String>,     // "synced", "timed", "plain", "none"
    pub cover_art_url: Option<String>,   // Album artwork URL
    pub spotify_track_id: Option<String>, // External Spotify ID
    // Extended metadata fields
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub genre: Option<String>,
    pub bpm: Option<f64>,
    pub musical_key: Option<String>,
    pub release_year: Option<i32>,
    pub explicit: Option<bool>,
    pub is_favorite: Option<bool>,
    pub favorite_at: Option<String>,
    pub file_path: Option<String>,
}

/// Paginated library response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryPage {
    pub tracks: Vec<LibraryTrack>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
}

/// Paginated search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub tracks: Vec<LibraryTrack>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
}

/// Download queue item
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DownloadItem {
    pub id: i64,
    pub title: String,
    pub artist_name: String,
    pub status: String,
    pub progress_percent: f64,
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub download_path: String,
    pub preferred_quality: String,
    pub auto_download_favorites: bool,
}

/// Library statistics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LibraryStats {
    pub total_tracks: i64,
    pub total_artists: i64,
    pub total_albums: i64,
    pub total_downloads: i64,
    pub queued_downloads: i64,
    pub active_downloads: i64,
    pub library_entries: i64,
    pub playlists: i64,
    pub services_with_data: i64,
}

/// Playlist for UI display
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub owner_name: Option<String>,
    pub track_count: i64,
    pub image_url: Option<String>,
    pub service_name: Option<String>,
}

/// Service connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub connected: bool,
    pub account_email: Option<String>,
    pub library_count: i64,
    pub favorites_count: i64,
    pub playlists_count: i64,
    pub last_synced: Option<String>,
    pub credentials_invalid: bool,
    pub invalid_reason: Option<String>,
    pub last_auth_error: Option<String>,
}

/// Real Service Authentication Status DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceAuthStatus {
    pub service: String,
    pub account_id: Option<i64>,
    pub status: String, // "connected_valid" | "requires_auth" | "expired" | "missing" | "error"
    pub is_authenticated: bool,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub error_message: Option<String>,
    pub last_checked: Option<String>,
}

/// Unified Import Preferences per Service DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreferences {
    pub service_name: String,
    pub favorite_tracks: bool,
    pub favorite_albums: bool,
    pub favorite_artists: bool,
    pub playlists: bool,
    pub purchases: bool,
    pub library_history: bool,
    pub include_appearances: bool,
    pub incremental_sync: bool,
}

impl Default for ImportPreferences {
    fn default() -> Self {
        Self {
            service_name: String::new(),
            favorite_tracks: true,
            favorite_albums: false,
            favorite_artists: false,
            playlists: true,
            purchases: false,
            library_history: false,
            include_appearances: false,
            incremental_sync: true,
        }
    }
}

/// Execution time per sync phase (ms)
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncPhaseTimings {
    pub api_fetch_ms: u64,
    pub entity_expansion_ms: u64,
    pub enrichment_ms: u64,
    pub persistence_ms: u64,
    pub availability_check_ms: u64,
    pub total_elapsed_ms: u64,
}

/// Album sync expansion metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSyncExpansionMetrics {
    pub albums_received: u64,
    pub albums_needing_expansion: u64,
    pub album_detail_requests: u64,
    pub album_detail_success: u64,
    pub album_detail_failed: u64,
    pub tracks_received: u64,
    pub tracks_persisted_new: u64,
    pub tracks_existing: u64,
    pub tracks_invalid: u64,
    pub first_error_code: Option<String>,
    pub first_error_album_id: Option<String>,
}

/// Unified Service Sync Result DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSyncResult {
    pub service: String,
    pub account_id: Option<i64>,
    pub success: bool,
    pub message: String,
    pub imported_tracks_total: u64,
    pub favorite_tracks_total: u64,
    pub favorite_albums_total: u64,
    pub favorite_artists_total: u64,
    pub playlists_total: u64,
    pub purchases_total: u64,
    pub skipped_tracks_total: u64,
    #[serde(default)]
    pub albums_total: u64,
    #[serde(default)]
    pub metadata_enriched: u64,
    #[serde(default)]
    pub metadata_partial: u64,
    #[serde(default)]
    pub availability_unknown: u64,
    #[serde(default)]
    pub availability_checked: u64,
    #[serde(default)]
    pub phase_timings: Option<SyncPhaseTimings>,
    #[serde(default)]
    pub album_expansion_metrics: Option<AlbumSyncExpansionMetrics>,
    pub errors: Vec<String>,
}

/// Parsed URL result from streaming service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedUrl {
    pub service: String,      // "spotify", "qobuz", "tidal", "deezer"
    pub content_type: String, // "track", "album", "playlist", "artist"
    pub id: String,           // service-specific ID
    pub url: String,          // original URL
}

// ==============================================
// DOWNLOAD PREFLIGHT & SAFE BATCH TYPES (S138A)
// ==============================================

/// Preflight downloadability classification status for tracks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DownloadPreflightStatus {
    /// Exact source locked and available on requested/primary download provider with active account
    ReadyExactSource,
    /// Exact fallback identity matched via ISRC, MusicBrainz Recording ID, MusicBrainz Release+duration, or AcoustID
    ReadyFallbackExactIdentity,
    /// Candidate source returned 404/not found/stale with no valid alternative fallback
    StaleSource,
    /// Candidate provider requires credentials or active authenticated account is missing
    RequiresAuth,
    /// Source quality is below requested quality under strict quality policy
    RejectedQuality,
    /// Ambiguous source due to competing active accounts or only loose title+artist match
    AmbiguousSource,
    /// Spotify/unsupported source with no downloadable provider source (Qobuz/Tidal) or identity mapping
    NoDownloadProvider,
    /// Transient network error, rate limit (HTTP 429), or timeout retryable
    NetworkRetryable,
    /// Track is already downloaded and present in local library
    AlreadyDownloaded,
    /// Track is already in queue in 'queued' or 'downloading' status
    AlreadyQueued,
}

impl DownloadPreflightStatus {
    #[allow(dead_code)]
    pub fn is_eligible(&self) -> bool {
        matches!(
            self,
            DownloadPreflightStatus::ReadyExactSource
                | DownloadPreflightStatus::ReadyFallbackExactIdentity
        )
    }

    #[allow(dead_code)]
    pub fn code(&self) -> &'static str {
        match self {
            DownloadPreflightStatus::ReadyExactSource => "ReadyExactSource",
            DownloadPreflightStatus::ReadyFallbackExactIdentity => "ReadyFallbackExactIdentity",
            DownloadPreflightStatus::StaleSource => "StaleSource",
            DownloadPreflightStatus::RequiresAuth => "RequiresAuth",
            DownloadPreflightStatus::RejectedQuality => "RejectedQuality",
            DownloadPreflightStatus::AmbiguousSource => "AmbiguousSource",
            DownloadPreflightStatus::NoDownloadProvider => "NoDownloadProvider",
            DownloadPreflightStatus::NetworkRetryable => "NetworkRetryable",
            DownloadPreflightStatus::AlreadyDownloaded => "AlreadyDownloaded",
            DownloadPreflightStatus::AlreadyQueued => "AlreadyQueued",
        }
    }
}

/// Detailed preflight evaluation result for a single track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackPreflightResult {
    pub track_id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub status: DownloadPreflightStatus,
    pub is_eligible: bool,
    pub resolved_service_id: Option<i64>,
    pub resolved_service_name: Option<String>,
    pub resolved_service_track_id: Option<String>,
    pub resolved_quality: Option<String>,
    pub reason: String,
    pub match_method: Option<String>,
}

/// Consolidated counters for preflight batch evaluation
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PreflightSummaryCounts {
    pub requested_total: i64,
    pub eligible_total: i64,
    pub ready_exact: i64,
    pub ready_fallback: i64,
    pub already_downloaded: i64,
    pub already_queued: i64,
    pub no_download_provider: i64,
    pub ambiguous_source: i64,
    pub rejected_quality: i64,
    pub stale_source: i64,
    pub requires_auth: i64,
    pub network_retryable: i64,
}

/// Response returned by preflight_download_batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightBatchResponse {
    pub summary: PreflightSummaryCounts,
    pub tracks: Vec<TrackPreflightResult>,
    pub estimated_size_mb: f64,
}

/// Response returned by safe batch enqueue operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEnqueueResult {
    pub submitted: i64,
    pub added: i64,
    pub enqueued: i64,
    pub deduplicated: i64,
    pub skipped: i64,
    pub summary: PreflightSummaryCounts,
    pub tracks: Vec<TrackPreflightResult>,
}

// ==============================================
// TESTS
// ==============================================

#[cfg(test)]
mod types_tests {
    use super::*;

    #[test]
    fn test_service_error_display() {
        let err = ServiceError::NotConnected("Spotify".to_string());
        assert_eq!(err.to_string(), "Spotify account not connected");

        let err = ServiceError::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");

        let err = ServiceError::InvalidCredentials("expired".to_string());
        assert_eq!(err.to_string(), "Invalid credentials: expired");
    }

    #[test]
    fn test_service_error_to_string() {
        let err = ServiceError::Api("rate limited".to_string());
        let s: String = err.into();
        assert_eq!(s, "API error: rate limited");
    }

    #[test]
    fn test_progress_event_new() {
        let event = ProgressEvent::new("download", "track-123");
        assert_eq!(event.operation, "download");
        assert_eq!(event.id, "track-123");
        assert_eq!(event.status, "started");
        assert_eq!(event.current, 0);
        assert_eq!(event.total, 0);
    }

    #[test]
    fn test_progress_event_progress() {
        let event = ProgressEvent::new("import", "job-1").progress(50, 100, "Halfway done");
        assert_eq!(event.status, "progress");
        assert_eq!(event.current, 50);
        assert_eq!(event.total, 100);
        assert_eq!(event.percentage, 50.0);
        assert_eq!(event.message, Some("Halfway done".to_string()));
    }

    #[test]
    fn test_progress_event_completed() {
        let event = ProgressEvent::new("scan", "scan-1").completed("All done");
        assert_eq!(event.status, "completed");
        assert_eq!(event.percentage, 100.0);
        assert_eq!(event.message, Some("All done".to_string()));
    }

    #[test]
    fn test_progress_event_failed() {
        let event = ProgressEvent::new("download", "d-1").failed("Network timeout");
        assert_eq!(event.status, "failed");
        assert_eq!(event.message, Some("Network timeout".to_string()));
    }

    #[test]
    fn test_sync_progress_event_constructors() {
        let started = SyncProgressEvent::new("qobuz", Some(1), "sync");
        assert_eq!(started.service, "qobuz");
        assert_eq!(started.account_id, Some(1));
        assert_eq!(started.operation, "sync");
        assert_eq!(started.phase, "authenticating");
        assert_eq!(started.status, "running");
        assert_eq!(started.terminal, false);
        assert_eq!(started.current, 0);

        let running = SyncProgressEvent::running(
            "qobuz",
            Some(1),
            "fetching_favorite_tracks",
            10,
            Some(50),
            "Fetching favorite tracks (10/50)",
            10,
            10,
        );
        assert_eq!(running.phase, "fetching_favorite_tracks");
        assert_eq!(running.current, 10);
        assert_eq!(running.total, Some(50));
        assert_eq!(running.imported_tracks_total, 10);
        assert_eq!(running.favorite_tracks_total, 10);
        assert_eq!(running.terminal, false);

        let completed = SyncProgressEvent::completed("qobuz", Some(1), "Done", 50, 50, Some(50));
        assert_eq!(completed.phase, "completed");
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.terminal, true);
        assert_eq!(completed.current, 50);

        let failed = SyncProgressEvent::failed("qobuz", Some(1), "fetching_favorite_tracks", "Network error", 10, 10);
        assert_eq!(failed.phase, "fetching_favorite_tracks");
        assert_eq!(failed.status, "failed");
        assert_eq!(failed.terminal, true);

        let req_auth = SyncProgressEvent::requires_auth("qobuz", Some(1), "Token expired");
        assert_eq!(req_auth.phase, "requires_auth");
        assert_eq!(req_auth.status, "requires_auth");
        assert_eq!(req_auth.terminal, true);
    }
}

/// Mutex wrapper for library import exclusivity
pub struct ImportLock(pub tokio::sync::Mutex<()>);
