//! Manifest models for auditability and batch pipeline execution summaries.

use serde::{Deserialize, Serialize};

/// Track-level audit record for reproducible manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrackManifestEntry {
    pub provider: String,
    pub source_track_id: String,
    pub isrc: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub format_requested: String,
    pub format_obtained: Option<String>,
    pub quality_class_requested: String,
    pub quality_class_obtained: Option<String>,
    pub codec: Option<String>,
    pub container: Option<String>,
    pub extension: Option<String>,
    pub source: Option<String>,
    pub quality_fallback: bool,
    pub download_result: String, // "Success", "SkippedExisting", "Failed", "RejectedQuality"
    pub rejection_reason: Option<String>,
    pub audio_validation: String, // "Valid", "Invalid", "None"
    pub error: Option<String>,
    pub format_id_requested: String,
    pub format_id_obtained: Option<String>,
    pub final_path: Option<String>,
    pub size_bytes: Option<u64>,
    pub flac_validation: String, // "Valid", "Invalid", "Skipped", "None"
    pub tagging_result: String, // "Success", "Failed", "Skipped"
    pub enrichment_result: String, // "Success", "Partial", "None"
    pub cover_result: String, // "StaticJPEG", "StaticAndAnimated", "None", "Failed"
    pub lyrics_result: String, // "WordSynced", "LineSynced", "Plain", "None"
}

/// Complete batch execution summary separating all metrics cleanly.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct FavoritesBatchSummary {
    pub requested: usize,
    pub received: usize,
    pub deduplicated: usize,
    pub skipped_existing: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub enriched: usize,
    pub validated: usize,
    pub output_files: usize,
    pub manifest: Vec<TrackManifestEntry>,
}

impl TrackManifestEntry {
    pub fn is_success(&self) -> bool {
        self.download_result == "Success"
    }
}

impl FavoritesBatchSummary {
    pub fn new(requested: usize) -> Self {
        Self {
            requested,
            ..Default::default()
        }
    }

    pub fn all_succeeded(&self) -> bool {
        self.failed == 0 && self.succeeded > 0 && self.succeeded == self.requested
    }

    pub fn record_success(&mut self, entry: TrackManifestEntry) {
        self.succeeded += 1;
        self.manifest.push(entry);
    }

    pub fn record_failure(&mut self, entry: TrackManifestEntry) {
        self.failed += 1;
        self.manifest.push(entry);
    }

    pub fn record_skipped(&mut self, entry: TrackManifestEntry) {
        self.skipped_existing += 1;
        self.manifest.push(entry);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_entry_serialization_roundtrip() {
        let entry = TrackManifestEntry {
            provider: "tidal".to_string(),
            source_track_id: "80654035".to_string(),
            isrc: Some("GBAYE7700021".to_string()),
            title: "Heroes".to_string(),
            artist: "David Bowie".to_string(),
            album: "Heroes".to_string(),
            format_requested: "24-96".to_string(),
            format_obtained: Some("24-96".to_string()),
            quality_class_requested: "Lossless".to_string(),
            quality_class_obtained: Some("Lossless".to_string()),
            codec: Some("FLAC".to_string()),
            container: Some("FLAC".to_string()),
            extension: Some("flac".to_string()),
            source: Some("Tidal Official API".to_string()),
            quality_fallback: false,
            download_result: "Success".to_string(),
            rejection_reason: None,
            audio_validation: "Valid".to_string(),
            error: None,
            format_id_requested: "HI_RES_LOSSLESS".to_string(),
            format_id_obtained: Some("HI_RES_LOSSLESS".to_string()),
            final_path: Some("C:/Music/David Bowie/[1977] Heroes/01 - Heroes.flac".to_string()),
            size_bytes: Some(73400320),
            flac_validation: "Valid".to_string(),
            tagging_result: "Success".to_string(),
            enrichment_result: "Success".to_string(),
            cover_result: "StaticAndAnimated".to_string(),
            lyrics_result: "LineSynced".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("80654035"));
        assert!(json.contains("GBAYE7700021"));

        let deserialized: TrackManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, entry);
    }
}
