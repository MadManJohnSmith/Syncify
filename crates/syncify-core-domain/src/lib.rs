//! Syncify Core Domain Contract
//!
//! Pure, I/O-free domain types, quality policy, error models, manifest schema,
//! typed progress events, metadata models, cover preservation rules, and byte validators.

pub mod byte_validators;
pub mod cover_rules;
pub mod errors;
pub mod events;
pub mod manifest;
pub mod metadata;
pub mod quality;

pub use byte_validators::{AudioByteValidator, WebpByteValidator, WebpStructureInfo, WebpValidationError};
pub use cover_rules::{CoverPreservationPolicy, CoverType, CoverUpdateDecision};
pub use errors::{PipelineError, RequiresAuthReason};
pub use events::{PipelineProgressEvent, PipelineStepStatus};
pub use manifest::{FavoritesBatchSummary, TrackManifestEntry};
pub use metadata::{
    artist_matches, clean_title, score_tidal_candidate, score_tidal_release, title_matches,
    TidalAlbum, TidalArtist, TidalMediaMetadata, TidalSearchResponse, TidalSearchTracks, TidalTrack,
};
pub use quality::{QualityClass, QualityPolicy, StreamResolution, StreamSourceType};
