//! Motor de letras v2 (ADR-0017): un módulo por Fuente de letras, con la
//! política y la selección dentro del crate; `src-tauri` lo consume vía su
//! fachada fina en `download/lyrics.rs`. La frontera es el contrato de dominio
//! (`syncify-lyrics-domain`): sin Tauri ni BD — el cliente HTTP, el limitador
//! de peticiones y el token de Apple Music llegan inyectados vía `EngineHooks`.

pub mod cache;
pub mod client;
pub mod config;
pub mod failures;
pub mod flac;
pub mod hooks;
pub mod pipeline;
pub mod sources;
pub mod types;

pub use cache::{clear_lyrics_cache, set_cached_lyrics};
pub use client::{is_valid_lyrics, LyricsClient};
pub use config::{LyricsEngineConfig, LyricsSourceSetting, LYRICS_SOURCE_IDS};
pub use failures::{
    classify_failure_message, classify_source_error, redact_failure_detail, LyricsCascadeReport,
    LyricsFailureKind, LyricsResolutionReport, LyricsSourceFailure,
};
pub use flac::validate_and_embed_flac_lyrics;
pub use hooks::{EngineHooks, NoopHooks};
pub use pipeline::{generate_sidecar_lrc, LyricsPipelineService};
pub use sources::lrclib::LRCLibResponse;
pub use types::{LyricsLine, LyricsResponse};

#[cfg(test)]
mod tests;
