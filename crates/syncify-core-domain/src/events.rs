//! Typed progress states and events for frontend and CLI consumers.

use serde::{Deserialize, Serialize};

/// High-level step in the download/processing pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStepStatus {
    Authenticating,
    AccountResolved,
    Searching,
    TrackResolved,
    TrackUnresolved,
    CandidateRejected,
    ResolvingStream,
    Downloading,
    DownloadStarted,
    DownloadCompleted,
    Validating,
    Tagging,
    MetadataApplied,
    Enriching,
    Staging,
    StagingCompleted,
    Persisting,
    Persisted,
    Completed,
    RejectedQuality,
    RequiresAuth,
    RecoverableError,
    Cancelled,
}

impl std::fmt::Display for PipelineStepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineStepStatus::Authenticating => write!(f, "authenticating"),
            PipelineStepStatus::AccountResolved => write!(f, "account_resolved"),
            PipelineStepStatus::Searching => write!(f, "searching"),
            PipelineStepStatus::TrackResolved => write!(f, "track_resolved"),
            PipelineStepStatus::TrackUnresolved => write!(f, "track_unresolved"),
            PipelineStepStatus::CandidateRejected => write!(f, "candidate_rejected"),
            PipelineStepStatus::ResolvingStream => write!(f, "resolving_stream"),
            PipelineStepStatus::Downloading => write!(f, "downloading"),
            PipelineStepStatus::DownloadStarted => write!(f, "download_started"),
            PipelineStepStatus::DownloadCompleted => write!(f, "download_completed"),
            PipelineStepStatus::Validating => write!(f, "validating"),
            PipelineStepStatus::Tagging => write!(f, "tagging"),
            PipelineStepStatus::MetadataApplied => write!(f, "metadata_applied"),
            PipelineStepStatus::Enriching => write!(f, "enriching"),
            PipelineStepStatus::Staging => write!(f, "staging"),
            PipelineStepStatus::StagingCompleted => write!(f, "staging_completed"),
            PipelineStepStatus::Persisting => write!(f, "persisting"),
            PipelineStepStatus::Persisted => write!(f, "persisted"),
            PipelineStepStatus::Completed => write!(f, "completed"),
            PipelineStepStatus::RejectedQuality => write!(f, "rejected_quality"),
            PipelineStepStatus::RequiresAuth => write!(f, "requires_auth"),
            PipelineStepStatus::RecoverableError => write!(f, "recoverable_error"),
            PipelineStepStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Detailed context for a resolved track candidate across providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedTrackInfo {
    pub provider: String,
    pub track_id: String,
    pub isrc: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_sec: i32,
    pub requested_quality: String,
    pub obtained_quality: Option<String>,
    pub active_account: Option<String>,
    pub region: Option<String>,
    pub allow_fallback: bool,
    pub stream_codec: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<f64>,
}


/// Structured event payload emitted to UI and event listeners.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PipelineProgressEvent {
    pub item_id: String,
    pub provider: String,
    pub status: PipelineStepStatus,
    pub progress_percent: f64,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_sec: Option<u64>,
    pub resolved_track: Option<ResolvedTrackInfo>,
    pub message: Option<String>,
    pub error: Option<String>,
}

impl PipelineProgressEvent {
    pub fn new(item_id: impl Into<String>, provider: impl Into<String>, status: PipelineStepStatus) -> Self {
        Self {
            item_id: item_id.into(),
            provider: provider.into(),
            status,
            progress_percent: 0.0,
            bytes_downloaded: 0,
            total_bytes: None,
            speed_bytes_per_sec: None,
            resolved_track: None,
            message: None,
            error: None,
        }
    }

    pub fn with_resolved_track(mut self, info: ResolvedTrackInfo) -> Self {
        self.resolved_track = Some(info);
        self
    }


    pub fn with_progress(mut self, percent: f64, bytes: u64, total: Option<u64>) -> Self {
        self.progress_percent = percent.clamp(0.0, 100.0);
        self.bytes_downloaded = bytes;
        self.total_bytes = total;
        self
    }

    pub fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    pub fn with_error(mut self, err: impl Into<String>) -> Self {
        self.error = Some(err.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_progress_event_serialization() {
        let event = PipelineProgressEvent::new("track-123", "tidal", PipelineStepStatus::Downloading)
            .with_progress(45.5, 1048576, Some(2097152))
            .with_message("Downloading segment 5/10");

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("track-123"));
        assert!(json.contains("downloading"));
        assert!(json.contains("45.5"));

        let deserialized: PipelineProgressEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, event);
    }
}
