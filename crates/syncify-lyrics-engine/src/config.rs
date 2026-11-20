//! Configuración del motor de letras y política de fuentes en orden de usuario.

use serde::{Deserialize, Serialize};

/// Configuration for the lyrics download engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsEngineConfig {
    pub min_sync_level: String,
    pub preferred_language: String,
    pub storage_format: String,
    pub auto_fetch_on_import: bool,
    pub retry_failed: bool,
    pub retry_frequency: String,
    /// Stable source variants in user priority order. Empty means no network sources.
    #[serde(default = "default_lyrics_sources")]
    pub sources: Vec<LyricsSourceSetting>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricsSourceSetting {
    pub provider_id: String,
    pub enabled: bool,
    pub priority: i64,
}

pub const LYRICS_SOURCE_IDS: &[&str] = &[
    "apple_music_ttml",
    "spotify_color",
    "musixmatch_richsync",
    "ultrastar",
    "kugou",
    "qqmusic",
    "netease_synced",
    "lyricsplus",
    "lrclib_synced",
    "musixmatch_plain",
    "lrclib_plain",
    "tekstowo_plain",
];

fn default_lyrics_sources() -> Vec<LyricsSourceSetting> {
    LYRICS_SOURCE_IDS
        .iter()
        .enumerate()
        .map(|(i, id)| LyricsSourceSetting {
            provider_id: (*id).to_string(),
            enabled: true,
            priority: i as i64 + 1,
        })
        .collect()
}

impl Default for LyricsEngineConfig {
    fn default() -> Self {
        Self {
            min_sync_level: "line".to_string(),
            preferred_language: "original".to_string(), // Default = ORIGINAL language, not english
            storage_format: "lrc".to_string(),          // LRC only, no TTML for compatibility
            auto_fetch_on_import: true,
            retry_failed: true,
            retry_frequency: "weekly".to_string(),
            sources: default_lyrics_sources(),
        }
    }
}
