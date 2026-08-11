//! Domain contract for Metadata Enrichment engine in `src-tauri` (Phase 2).
//!
//! Provides canonical representations for enrichment states, field provenance,
//! conflict resolution, ISO 639-3 language codes, and ISO 3166-1 alpha-2 country codes.

use serde::{Deserialize, Serialize};
use std::future::Future;

/// Explicit resolution states for enrichment fields
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FieldResolution {
    Resolved {
        value: String,
        source: String,
        confidence: f64,
        resolved_at: String,
        conflict: Option<ConflictInfo>,
    },
    NotFound {
        source: String,
        checked_at: String,
    },
    NotSupported {
        reason: String,
    },
    SourceUnavailable {
        source: String,
        error: String,
    },
    Failed {
        source: String,
        error: String,
        failed_at: String,
    },
    NotRequested,
}

impl Default for FieldResolution {
    fn default() -> Self {
        FieldResolution::NotRequested
    }
}

/// Information registered when two valid enrichment sources conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictInfo {
    pub alternate_source: String,
    pub alternate_value: String,
    pub alternate_confidence: f64,
    pub conflict_reason: String,
}

/// Enriched Metadata domain DTO with canonical ISO language/country codes
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EnrichedMetadata {
    // External IDs & Provenance
    pub isrc: FieldResolution,
    pub musicbrainz_recording_id: FieldResolution,
    pub musicbrainz_release_id: FieldResolution,
    pub musicbrainz_release_group_id: FieldResolution,
    pub musicbrainz_artist_id: FieldResolution,
    pub discogs_release_id: FieldResolution,
    pub barcode: FieldResolution,

    // Title & Structure
    pub title: FieldResolution,
    pub artist: FieldResolution,
    pub album_artist: FieldResolution,
    pub album: FieldResolution,
    pub track_number: FieldResolution,
    pub track_total: FieldResolution,
    pub disc_number: FieldResolution,
    pub disc_total: FieldResolution,

    // Release Details (ISO-normalized)
    pub original_date: FieldResolution,
    pub label: FieldResolution,
    pub catalog_number: FieldResolution,
    pub release_type: FieldResolution,
    pub release_status: FieldResolution,
    /// Canonical ISO 3166-1 alpha-2 country code (e.g. "US", "GB", "XW")
    pub release_country: FieldResolution,
    /// Canonical ISO 639-3 language code (e.g. "eng", "jpn", "spa", "zxx")
    pub language: FieldResolution,

    // Musical Attributes
    pub genre: FieldResolution,
    pub style: FieldResolution,
    pub mood: FieldResolution,
    pub bpm: FieldResolution,
    pub key: FieldResolution,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub loudness: Option<f64>,

    pub enriched_at: String,
}

/// Asynchronous domain trait for metadata enrichment providers
pub trait EnrichmentProvider: Send + Sync {
    fn enrich(
        &self,
        artist: &str,
        album: &str,
        title: &str,
    ) -> impl Future<Output = Result<EnrichedMetadata, String>> + Send;
}
