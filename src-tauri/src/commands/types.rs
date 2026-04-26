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

// ==============================================
// RESPONSE TYPES
// ==============================================

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
    pub services: Option<String>,        // Comma-separated service names
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
}

/// Mutex wrapper for library import exclusivity
pub struct ImportLock(pub tokio::sync::Mutex<()>);
