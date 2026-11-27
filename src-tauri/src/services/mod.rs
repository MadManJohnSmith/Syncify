//! Service connectors module
//!
//! OAuth and API integration for streaming services.

pub mod animated_cover;
pub mod apple_music;
pub mod catalog_identity_audit;
pub mod catalog_identity_repair;
pub mod concurrency_manager;
pub mod deezer;
pub mod disambiguation_repair;
pub mod enrichment;
pub mod flac_cover_sanitizer;
pub mod flac_picture;
pub mod http_retry;
pub mod import_pagination;
pub mod incremental_enrichment;
pub mod lastfm;
pub mod library_membership;
pub mod logging;
pub mod lyrics_negative_cache;
pub mod manifest_writer;
pub mod mb_dump_import;
pub mod mp4_writer;
pub mod mpris;
pub mod musicbrainz;
pub mod notification;
pub mod operation_recovery;
pub mod playback_advance;
pub mod playback_promotion;
pub mod playback_session;
pub mod playlist_canonical;
// El unico consumidor es MPRIS, que solo existe en Linux: fuera de Linux sus
// funciones quedarian sin uso y `-D warnings` rompe el binario.
#[cfg(target_os = "linux")]
pub mod player_control;
pub mod qobuz;
pub mod rate_limiter;
pub mod repair_guardrail;
pub mod repair_history;
pub mod silence_trimmer;
pub mod soundcloud;
pub mod spotify;
pub mod stream_cache;
pub mod streaming;
pub mod tag_writer;
pub mod tempo_analyzer;
pub mod tidal;
pub mod tidal_pipeline;
pub mod track_identity;
pub mod track_identity_review;
pub mod track_matcher;
pub mod user_reports;

pub use apple_music::AppleMusicClient;
#[allow(unused_imports)]
pub use concurrency_manager::{
    get_global_concurrency_manager, ConcurrencyError, ConcurrencyGuard, ConcurrencyManager,
    MultiConcurrencyGuard,
};
pub use deezer::DeezerClient;
pub use manifest_writer::ManifestWriter;
pub use musicbrainz::MusicBrainzClient;
pub use qobuz::QobuzClient;
pub use soundcloud::SoundCloudClient;
pub use spotify::{ImportResult, SpotifyClient, SpotifyConfig, SPOTIFY_SCOPES};
pub use tidal::TidalClient;

#[cfg(test)]
mod migration_tests;

pub mod catalog_search;
