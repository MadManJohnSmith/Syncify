// Progress tracking for downloads

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
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
    pub percent: f32,
    pub status: DownloadStatus,
    pub service: Option<String>,
    pub message: Option<String>,
}

#[allow(dead_code)]
impl DownloadProgress {
    pub fn new(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            percent: 0.0,
            status: DownloadStatus::Queued,
            service: None,
            message: None,
        }
    }

    pub fn searching(item_id: &str, service: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            percent: 0.0,
            status: DownloadStatus::Searching,
            service: Some(service.to_string()),
            message: Some(format!("Searching on {}", service)),
        }
    }

    pub fn downloading(item_id: &str, service: &str, bytes: u64, total: u64) -> Self {
        let percent = if total > 0 {
            (bytes as f32 / total as f32) * 100.0
        } else {
            0.0
        };
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: bytes,
            bytes_total: total,
            percent,
            status: DownloadStatus::Downloading,
            service: Some(service.to_string()),
            message: None,
        }
    }

    pub fn finalizing(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            percent: 100.0,
            status: DownloadStatus::Finalizing,
            service: None,
            message: Some("Embedding metadata...".to_string()),
        }
    }

    pub fn complete(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            percent: 100.0,
            status: DownloadStatus::Complete,
            service: None,
            message: None,
        }
    }

    pub fn failed(item_id: &str, error: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            bytes_downloaded: 0,
            bytes_total: 0,
            percent: 0.0,
            status: DownloadStatus::Failed,
            service: None,
            message: Some(error.to_string()),
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
}

/// Request to download a track with explicit source identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub item_id: String,
    pub isrc: Option<String>,
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
}

/// Global progress tracker
pub struct ProgressTracker {
    items: RwLock<HashMap<String, (DownloadProgress, Instant)>>,
}

#[allow(dead_code)]
impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
        }
    }

    pub fn init(&self, item_id: &str) {
        let mut items = match self.items.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("ProgressTracker items lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        items.insert(
            item_id.to_string(),
            (DownloadProgress::new(item_id), Instant::now()),
        );
    }

    pub fn update(&self, progress: DownloadProgress) {
        let mut items = match self.items.write() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("ProgressTracker items lock poisoned, recovering");
                poisoned.into_inner()
            }
        };
        items.insert(progress.item_id.clone(), (progress, Instant::now()));
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
        assert_eq!(progress.percent, 10.0);

        tracker.remove("test-1");
        assert!(tracker.get("test-1").is_none());
    }
}
