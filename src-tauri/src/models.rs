//! Database models matching the production schema
//!
//! All structs correspond to tables in migrations 0001-0004.
//! Many structs are for future use as the application expands.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ==============================================
// CORE ENTITIES
// ==============================================

/// Streaming service (spotify, qobuz, tidal, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Service {
    pub id: i64,
    pub name: String,
    pub supports_download: bool,
    pub max_quality: Option<String>,
    pub created_at: Option<String>,
}

/// User account for a service
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Account {
    pub id: i64,
    pub service_id: i64,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    pub credentials_json: Option<String>,
    pub last_synced: Option<String>,
    pub created_at: Option<String>,
}

/// Canonical artist (deduplicated)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Artist {
    pub id: i64,
    pub name: String,
    pub musicbrainz_id: Option<String>,
    pub spotify_id: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Canonical album (deduplicated)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Album {
    pub id: i64,
    pub title: String,
    pub release_date: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub upc: Option<String>,
    pub total_tracks: Option<i32>,
    pub cover_art_url: Option<String>,
    pub created_at: Option<String>,
}

/// Canonical track (deduplicated by ISRC)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub album_id: Option<i64>,
    pub duration_ms: Option<i64>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub isrc: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub acoustid_fingerprint: Option<String>,
    pub explicit: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

// ==============================================
// MAPPING TABLES
// ==============================================

/// Track-Artist relationship with role
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrackArtist {
    pub track_id: i64,
    pub artist_id: i64,
    pub role: String,
}

/// Track quality per service
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TrackSource {
    pub id: i64,
    pub track_id: i64,
    pub service_id: i64,
    pub service_track_id: String,
    pub format: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<i32>,
    pub bitrate: Option<i32>,
    pub quality_score: Option<i32>,
    pub available: bool,
    pub last_checked: Option<String>,
}

// ==============================================
// USER DATA
// ==============================================

/// User's library entry (liked track)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LibraryEntry {
    pub id: i64,
    pub account_id: i64,
    pub track_id: i64,
    pub added_at: Option<String>,
    pub is_liked: bool,
    pub play_count: i32,
    pub auto_download: bool,
}

/// Playlist
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Playlist {
    pub id: i64,
    pub account_id: i64,
    pub service_playlist_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub track_count: i32,
    pub last_synced: Option<String>,
    pub created_at: Option<String>,
}

// ==============================================
// DOWNLOADS
// ==============================================

/// Download queue item
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DownloadQueueItem {
    pub id: i64,
    pub track_id: i64,
    pub status: String,
    pub priority: i32,
    pub quality_preference: Option<String>,
    pub progress_percent: f64,
    pub bytes_downloaded: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub created_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Downloaded file
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Download {
    pub id: i64,
    pub track_id: Option<i64>,
    pub source_service_id: Option<i64>,
    pub file_path: String,
    pub file_format: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub file_hash: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<i32>,
    pub metadata_completeness: i32,
    pub downloaded_at: Option<String>,
    pub only_available_on: Option<String>,
    pub not_streaming: bool,
    pub musicbrainz_release_id: Option<String>,
    pub updated_at: Option<String>,
}

// ==============================================
// OTHER
// ==============================================

/// Lyrics
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Lyrics {
    pub id: i64,
    pub track_id: Option<i64>,
    pub format: String,
    pub sync_level: Option<String>,
    pub source: Option<String>,
    pub content: String,
    pub language: Option<String>,
    pub embedded_in_file: bool,
    pub created_at: Option<String>,
}

/// Library stats view
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

// ==============================================
// API RESPONSE TYPES
// ==============================================

/// Track with artist info for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackWithArtist {
    pub id: i64,
    pub title: String,
    pub artist_name: String,
    pub album_title: Option<String>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub is_downloaded: bool,
    pub best_quality: Option<String>,
}

/// Service connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub connected: bool,
    pub account_email: Option<String>,
    pub last_synced: Option<String>,
    pub track_count: i64,
    pub credentials_invalid: bool,
    pub invalid_reason: Option<String>,
    pub last_auth_error: Option<String>,
}

// ==============================================
// SPRINT 1: SERVICE PREFERENCES & SYNC SETTINGS
// ==============================================

/// Service preference for import priority ordering
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServicePreference {
    pub id: i64,
    pub service_name: String,
    pub priority: i64,
    pub auto_import_enabled: bool,
}

/// Global sync settings (singleton)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SyncSettings {
    pub id: i64,
    pub auto_sync_enabled: bool,
    pub sync_interval_value: i64,
    pub sync_interval_unit: String,
    pub sync_on_startup: bool,
    pub background_download: bool,
    pub max_concurrent_downloads: i64,
    pub rate_limit_delay_ms: i64,
    pub pause_on_metered: bool,
    pub pause_on_low_battery: bool,
}

/// Per-service sync settings
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceSyncSettings {
    pub id: i64,
    pub service_name: String,
    pub sync_favorites: bool,
    pub sync_playlists: bool,
    pub sync_albums: bool,
    pub incremental_sync: bool,
    pub last_synced: Option<String>,
}

// ==============================================
// SPRINT 2: DOWNLOADS + FILE SETTINGS
// ==============================================

/// Quality preference per streaming service
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QualityPreference {
    pub id: i64,
    pub service_name: String,
    pub max_quality: String,
    pub preferred_format: String,
    pub fallback_quality: String,
    pub fallback_format: String,
}

/// Folder structure and file naming settings (singleton)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FolderSettings {
    pub id: i64,
    pub base_folder: String,
    pub folder_template: String,
    pub file_template: String,
    pub artist_separator: String,
    pub replace_spaces_with: Option<String>,
    pub max_path_length: i64,
    pub fallback_action: String,
}

/// Duplicate detection and handling settings (singleton)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DuplicateSettings {
    pub id: i64,
    pub enable_detection: bool,
    pub prefer_higher_quality: bool,
    pub prefer_lossless: bool,
    pub replace_same_quality_different_source: bool,
    pub quality_threshold_kbps: i64,
    pub delete_duplicates_immediately: bool,
    pub move_to_trash: bool,
}

/// Audio processing settings (singleton)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AudioProcessingSettings {
    pub id: i64,
    pub replay_gain_mode: String,
    pub target_loudness_lufs: f64,
    pub transcode_enabled: bool,
    pub transcode_format: String,
    pub transcode_bitrate: i64,
    pub keep_original_after_transcode: bool,
    pub embed_lyrics: bool,
    pub embed_artwork: bool,
    pub artwork_max_size: i64,
}

// ==============================================
// SPRINT 3: LYRICS TAB + SETTINGS
// ==============================================

/// Lyrics provider settings (priority ordering)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LyricsProviderSetting {
    pub id: i64,
    pub provider_id: String,
    pub provider_name: String,
    pub enabled: bool,
    pub priority: i64,
    pub sync_level: String,
}

/// Global lyrics configuration (singleton)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LyricsConfig {
    pub id: i64,
    pub min_sync_level: String,
    pub preferred_language: String,
    pub storage_format: String,
    pub auto_fetch_on_import: bool,
    pub retry_failed: bool,
    pub retry_frequency: String,
}

// ==============================================
// SPRINT 4: DASHBOARD + LIBRARY DETAIL VIEWS
// ==============================================

/// Library snapshot for historical tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LibrarySnapshot {
    pub id: i64,
    pub snapshot_date: String,
    pub total_tracks: i64,
    pub total_albums: i64,
    pub total_artists: i64,
    pub total_size_bytes: i64,
    pub tracks_with_lyrics: i64,
    pub tracks_lossless: i64,
    pub tracks_hires: i64,
    pub metadata_excellent: i64,
    pub metadata_good: i64,
    pub metadata_needs_work: i64,
    pub downloaded_tracks: i64,
}

/// Service health information
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceHealthInfo {
    pub id: i64,
    pub service_name: String,
    pub is_connected: bool,
    pub token_valid: bool,
    pub token_expires_at: Option<String>,
    pub last_checked: String,
    pub error_message: Option<String>,
    pub rate_limit_remaining: Option<i64>,
    pub rate_limit_reset_at: Option<String>,
}

/// Extended album info for detail view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumDetail {
    pub id: i64,
    pub title: String,
    pub artist_name: String,
    pub release_year: Option<i32>,
    pub genre: Option<String>,
    pub label: Option<String>,
    pub track_count: i64,
    pub total_duration_ms: i64,
    pub artwork_url: Option<String>,
    pub quality: Option<String>,
    pub source_service: Option<String>,
}

/// Extended artist info for detail view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistDetail {
    pub id: i64,
    pub name: String,
    pub album_count: i64,
    pub track_count: i64,
    pub genres: Vec<String>,
    pub artwork_url: Option<String>,
}

// ==============================================
// SPRINT 5: ADVANCED SETTINGS & POLISH
// ==============================================

/// Advanced application settings (singleton)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AdvancedSettings {
    pub id: i64,
    // Logging settings
    pub log_level: String,
    pub log_to_file: bool,
    pub log_file_max_size_mb: i64,
    pub log_file_retention_days: i64,
    // Worker settings
    pub max_concurrent_downloads: i64,
    pub max_concurrent_imports: i64,
    pub worker_timeout_seconds: i64,
    // Cache settings
    pub cache_enabled: bool,
    pub cache_max_size_mb: i64,
    pub cache_ttl_hours: i64,
    // Matching settings
    pub fuzzy_match_threshold: f64,
    pub use_acoustic_fingerprinting: bool,
    pub prefer_exact_matches: bool,
    // Network settings
    pub request_timeout_seconds: i64,
    pub max_retries: i64,
    pub retry_delay_seconds: i64,
    pub use_proxy: bool,
    pub proxy_url: Option<String>,
    // Debug settings
    pub debug_mode: bool,
    pub verbose_api_logging: bool,
}

// ==============================================
// SPRINT 14: METADATA PREFERENCES
// ==============================================

/// Metadata preferences for tagging (singleton)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MetadataPreferences {
    pub id: i64,
    pub enable_musicbrainz: bool,
    pub enable_lastfm: bool,
    pub enable_acoustid: bool,
    pub overwrite_on_reimport: bool,
    pub preserve_custom_tags: bool,
    pub multi_value_separator: String,
    pub write_releasetype: bool,
    pub write_label: bool,
    pub write_work_composer: bool,
    pub write_musicbrainz_ids: bool,
    pub write_download_source: bool,
    pub write_download_date: bool,
    pub write_only_available_on: bool,
    pub write_not_available_streaming: bool,
    pub write_quality_score: bool,
    pub write_lyrics_tags: bool,
    pub weight_album: i64,
    pub weight_isrc: i64,
    pub weight_mb_id: i64,
    pub weight_cover: i64,
    pub weight_year: i64,
    pub weight_genre: i64,
}

/// Cache statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CacheStats {
    pub id: i64,
    pub cache_type: String,
    pub size_bytes: i64,
    pub item_count: i64,
    pub hit_count: i64,
    pub miss_count: i64,
    pub last_updated: String,
}

/// Diagnostic result for system health check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticResult {
    pub check_name: String,
    pub status: String,
    pub message: String,
    pub duration_ms: i64,
}

// ==============================================
// SPRINT 6: MIGRATION TAB
// ==============================================

/// Migration job tracking
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MigrationJob {
    pub id: String,
    pub source_service: String,
    pub destination_service: String,
    pub source_playlist_ids: Option<String>, // JSON array
    pub options: String,                     // JSON MigrationOptions
    pub status: String,
    pub total_items: i64,
    pub completed_items: i64,
    pub failed_items: i64,
    pub skipped_items: i64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
}

/// Individual migration item (track)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MigrationItem {
    pub id: i64,
    pub job_id: String,
    pub source_track_id: String,
    pub source_track_title: String,
    pub source_track_artist: String,
    pub source_track_album: Option<String>,
    pub source_playlist_id: Option<String>,
    pub source_playlist_name: Option<String>,
    pub destination_track_id: Option<String>,
    pub match_confidence: Option<f64>,
    pub match_method: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub processed_at: Option<String>,
    pub created_at: String,
}

/// Migration template for saved configurations
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MigrationTemplate {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub source_service: String,
    pub destination_service: String,
    pub options: String, // JSON
    pub created_at: String,
    pub updated_at: String,
}

/// Migration options (passed to start_migration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationOptions {
    pub match_threshold: f64,
    pub skip_unmatched: bool,
    pub create_playlists: bool,
    pub merge_existing: bool,
    pub download_matched: bool,
}

impl Default for MigrationOptions {
    fn default() -> Self {
        Self {
            match_threshold: 0.80,
            skip_unmatched: true,
            create_playlists: true,
            merge_existing: false,
            download_matched: true,
        }
    }
}

/// Preview result for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPreviewResult {
    pub total_tracks: i64,
    pub matched_tracks: i64,
    pub unmatched_tracks: i64,
    pub playlists: Vec<PlaylistPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistPreview {
    pub id: String,
    pub name: String,
    pub track_count: i64,
    pub matched_count: i64,
}

/// Migration progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProgress {
    pub job_id: String,
    pub current_item: i64,
    pub total_items: i64,
    pub current_track: String,
    pub status: String,
    pub completed_count: i64,
    pub failed_count: i64,
    pub skipped_count: i64,
}

/// Migration schema audit report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub schema_version: i64,
    pub schema_ok: bool,
    pub missing_tables: Vec<String>,
    pub legacy_services_detected: Vec<String>,
    pub summary: String,
}

/// Search result for manual matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationTrackMatch {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: i64,
    pub quality: Option<String>,
    pub confidence: f64,
}
