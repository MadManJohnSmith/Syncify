//! Syncify Core Domain Contract
//!
//! Pure, I/O-free domain types, quality policy, error models, manifest schema,
//! typed progress events, metadata models, cover preservation rules, byte validators,
//! and library layout engine.

pub mod byte_validators;
pub mod concurrency;
pub mod cover_rules;
pub mod errors;
pub mod events;
pub mod layout;
pub mod manifest;
pub mod metadata;
pub mod operation_recovery;
pub mod parity;
pub mod quality;
pub mod repair;
pub mod version_derivation;

pub use byte_validators::{AudioByteValidator, FlacStreamInfo, WebpByteValidator, WebpStructureInfo, WebpValidationError};
pub use concurrency::{
    ConcurrencyStatsSummary, LockHierarchyLevel, LockOutcome, LockScope, LockTelemetry,
};
pub use cover_rules::{CoverPreservationPolicy, CoverType, CoverUpdateDecision};
pub use errors::{ErrorTaxonomy, PipelineError, RequiresAuthReason};
pub use events::{PipelineProgressEvent, PipelineStepStatus};
pub use layout::{sanitize_filename, FolderFileTemplateConfig, LibraryLayout, TrackLayoutContext};
pub use manifest::{BatchDownloadManifest, FavoritesBatchSummary, TrackManifestEntry};
pub use metadata::{
    artist_matches, clean_mojibake, clean_title, classify_album, classify_artist, classify_title,
    decode_html_entities, extract_featured_artists, is_placeholder_album, is_placeholder_artist,
    is_placeholder_title, is_valid_isrc, parse_credit_role_and_name, parse_credits_string,
    sanitize_album_title, sanitize_artist_name, sanitize_track_title, score_tidal_candidate, score_tidal_release,
    title_matches, IdentityResolutionStatus, MetadataClassification, ProviderTrackIdentity,
    TidalAlbum, TidalArtist, TidalMediaMetadata, TidalSearchResponse, TidalSearchTracks,
    TidalTrack,
};
pub use operation_recovery::{
    OperationJournalEntry, OperationPhase, OperationRecoveryDetail, OperationStatus, OperationType,
    RecoveryAction, RecoveryAuditSummary,
};
pub use parity::{
    build_parity_report, compare_snapshots, get_expected_intentional_difference_registry,
    NormalizedOutputSnapshot, ParityCaseId, ParityClassification, ParityDifferenceRegistryItem,
    ParityExecutionResult, ParityReport,
};
pub use quality::{
    classify_audio_tier, AudioTier, FormatId, QualityClass, QualityDecision, QualityDecisionKind,
    QualityPolicy, StreamResolution, StreamSourceType,
};
pub use repair::{
    RepairFileBaseline, RepairHistoryRecord, RepairOutputHashes, RepairReport,
    RepairValidationStatus,
};
pub use version_derivation::{
    derive_track_version, DerivedVersionInfo, VersionConfidence, VersionDerivationInput,
};
