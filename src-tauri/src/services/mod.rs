//! Service connectors module
//!
//! OAuth and API integration for streaming services.

pub mod apple_music;
pub mod deezer;
pub mod enrichment;
pub mod http_retry;
pub mod lastfm;
pub mod musicbrainz;
pub mod qobuz;
pub mod rate_limiter;
pub mod soundcloud;
pub mod spotify;
pub mod tidal;
pub mod tidal_pipeline;
pub mod track_matcher;
pub mod tag_writer;
pub mod mp4_writer;
pub mod animated_cover;
pub mod manifest_writer;
pub mod notification;
pub mod disambiguation_repair;
pub mod incremental_enrichment;
pub mod repair_guardrail;
pub mod repair_history;
pub mod catalog_identity_audit;
pub mod catalog_identity_repair;
pub mod concurrency_manager;
pub mod operation_recovery;
pub mod logging;
pub mod tempo_analyzer;



pub use apple_music::AppleMusicClient;
#[allow(unused_imports)]
pub use concurrency_manager::{
    get_global_concurrency_manager, ConcurrencyError, ConcurrencyGuard, ConcurrencyManager,
    MultiConcurrencyGuard,
};
pub use manifest_writer::ManifestWriter;
pub use deezer::DeezerClient;
pub use musicbrainz::MusicBrainzClient;
pub use qobuz::QobuzClient;
pub use soundcloud::SoundCloudClient;
pub use spotify::{ImportResult, SpotifyClient, SpotifyConfig, SPOTIFY_SCOPES};
pub use tidal::TidalClient;

#[cfg(test)]
mod migration_tests;
