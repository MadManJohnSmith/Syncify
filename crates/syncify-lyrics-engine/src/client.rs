//! Cliente del motor: construcción inyectada (cliente HTTP + hooks),
//! política de fuentes y orquestación de la cascada de resolución.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use anyhow::{anyhow, Result};
use reqwest::Client;
use tokio::sync::Mutex as TokioMutex;
use tracing::debug;

use crate::config::{LyricsEngineConfig, LyricsSourceSetting, LYRICS_SOURCE_IDS};
use crate::failures::{
    classify_source_error, LyricsCascadeReport, LyricsFailureKind, LyricsResolutionReport,
    LyricsSourceFailure,
};
use crate::hooks::EngineHooks;
use crate::sources::musixmatch::MxmToken;
use crate::types::LyricsResponse;
use syncify_lyrics_domain::{
    detect_language_heuristic, ms_to_lrc_timestamp, LyricsResolution, SelectionOutcome,
    SequentialSelection, SourceOutcome, SyncLevel,
};

/// Lyrics client with caching + Musixmatch token management + Spotify Color Lyrics direct access
pub struct LyricsClient {
    // Campos compartidos con los módulos de fuente: el struct vive aquí pero
    // cada Fuente accede al cliente HTTP, hooks y tokens por `pub(crate)`.
    pub(crate) client: Client,
    pub(crate) hooks: Arc<dyn EngineHooks>,
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    pub(crate) cache: RwLock<HashMap<String, (LyricsResponse, Instant)>>,
    pub(crate) mxm_token: TokioMutex<Option<MxmToken>>,
    pub(crate) spotify_sp_dc: TokioMutex<Option<String>>,
    pub(crate) spotify_access_token: TokioMutex<Option<(String, Instant)>>,
    pub(crate) config: RwLock<LyricsEngineConfig>,
}

impl LyricsClient {
    /// Motor con cliente HTTP y hooks de infraestructura inyectados por la
    /// fachada (`src-tauri`): limitador global y token de Apple Music.
    pub fn with_parts(client: Client, hooks: Arc<dyn EngineHooks>) -> Self {
        Self::with_parts_and_config(client, hooks, LyricsEngineConfig::default())
    }

    /// Como `with_parts`, con la configuración persistida por el núcleo.
    pub fn with_parts_and_config(
        client: Client,
        hooks: Arc<dyn EngineHooks>,
        config: LyricsEngineConfig,
    ) -> Self {
        let env_sp_dc = std::env::var("SPOTIFY_SP_DC")
            .ok()
            .filter(|s| !s.trim().is_empty());
        Self {
            client,
            hooks,
            cache: RwLock::new(HashMap::new()),
            mxm_token: TokioMutex::new(None),
            spotify_sp_dc: TokioMutex::new(env_sp_dc),
            spotify_access_token: TokioMutex::new(None),
            config: RwLock::new(config),
        }
    }

    pub fn config(&self) -> LyricsEngineConfig {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn set_config(&self, config: LyricsEngineConfig) {
        if let Ok(mut c) = self.config.write() {
            *c = config;
        }
    }

    /// Source policy shared by interactive search and automatic downloads.
    pub(crate) fn ordered_sources(&self) -> Vec<LyricsSourceSetting> {
        let mut sources = self.config().sources;
        sources.retain(|s| s.enabled && LYRICS_SOURCE_IDS.contains(&s.provider_id.as_str()));
        sources.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.provider_id.cmp(&b.provider_id))
        });
        sources
    }

    /// Content policy shared by interactive search, automatic download and the
    /// sequential cascade: genuine payload plus the language preference. The
    /// sync level is decided by the caller (`policy_accepts` filters by the
    /// user's target; the cascade uses it to keep probing for improvement).
    pub(crate) fn content_matches_policy(
        lyrics: &LyricsResponse,
        title: &str,
        config: &LyricsEngineConfig,
    ) -> bool {
        if !is_valid_lyrics(lyrics, title) {
            return false;
        }
        let lang = config.preferred_language.trim();
        // Unknown languages cannot prove a requested language; original accepts any language.
        if lang.is_empty()
            || lang.eq_ignore_ascii_case("original")
            || lang.eq_ignore_ascii_case("any")
        {
            return true;
        }
        let content = lyrics
            .plain_lyrics
            .as_deref()
            .or(lyrics.elrc_content.as_deref())
            .or_else(|| lyrics.lines.first().map(|line| line.words.as_str()));
        content
            .and_then(detect_language_heuristic)
            .is_some_and(|detected| detected.eq_ignore_ascii_case(lang))
    }

    pub(crate) fn policy_accepts(&self, lyrics: &LyricsResponse, title: &str) -> bool {
        let config = self.config();
        Self::content_matches_policy(lyrics, title, &config)
            && response_sync_level(lyrics)
                >= SyncLevel::target_from_min_sync_level(&config.min_sync_level)
    }

    pub(crate) async fn fetch_source(
        &self,
        id: &str,
        artist: &str,
        track: &str,
        duration: f64,
    ) -> Result<LyricsResponse> {
        match id {
            "apple_music_ttml" => self.fetch_apple_music_ttml(artist, track, duration).await,
            "spotify_color" => self.fetch_spotify_lyrics(artist, track, duration).await,
            "musixmatch_richsync" => {
                self.fetch_musixmatch_richsync(artist, track, duration)
                    .await
            }
            "ultrastar" => self.fetch_ultrastar_karaoke(artist, track).await,
            "kugou" => self.fetch_kugou_karaoke(artist, track, duration).await,
            "qqmusic" => self.fetch_qqmusic_lyrics(artist, track, duration).await,
            "netease_synced" => self.fetch_netease_lyrics(artist, track, duration).await,
            "lyricsplus" => self.fetch_lyricsplus(artist, track, duration).await,
            "lrclib_synced" => {
                self.search_lyrics(&format!("{} {}", artist, track), duration)
                    .await
            }
            "musixmatch_plain" => self.fetch_musixmatch_plain(artist, track, duration).await,
            "lrclib_plain" => self.fetch_lrclib_plain(artist, track).await,
            "tekstowo_plain" => self.fetch_tekstowo_plain(artist, track).await,
            _ => Err(anyhow!("Unknown lyrics source")),
        }
    }

    /// All enabled candidates, ordered by user priority; disabled sources are never contacted.
    pub async fn search_all_providers(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Vec<LyricsResponse> {
        let mut results = Vec::new();
        for source in self.ordered_sources() {
            if let Ok(mut lyrics) = self
                .fetch_source(&source.provider_id, artist, track, duration_sec)
                .await
            {
                if self.policy_accepts(&lyrics, track) {
                    lyrics.lines.sort_by_key(|line| line.start_time_ms);
                    results.push(lyrics);
                }
            }
        }
        results
    }

    /// Automatic fetch uses the same enabled/priority/language policy as search.
    ///
    /// Sequential selection with improvement: it stops at the first candidate
    /// that reaches `min_sync_level` (as before), but while the best result is
    /// below that target it keeps probing so a finer result lower in the list
    /// can win. At exhaustion the best candidate seen is returned.
    pub async fn fetch_all_sources(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let report = self
            .fetch_all_sources_skipping(artist, track, duration_sec, &HashSet::new())
            .await;
        report
            .response
            .ok_or_else(|| anyhow!(report.error.unwrap_or_default()))
    }

    /// Same cascade as `fetch_all_sources`, but `skip` names the Fuentes whose
    /// known failure is still fresh in the persistent negative cache: they are
    /// not contacted and the report says which ones were skipped. Every probed
    /// Fuente that does not yield an acceptable candidate leaves a classified
    /// failure in the report for the caller to persist.
    pub async fn fetch_all_sources_skipping(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
        skip: &HashSet<String>,
    ) -> LyricsCascadeReport {
        let config = self.config();
        let mut selection = SequentialSelection::new(&config.min_sync_level);
        let mut report = LyricsCascadeReport::default();

        for source in self.ordered_sources() {
            if skip.contains(&source.provider_id) {
                debug!(
                    "[LyricsCascade] Skipping '{}' for '{} - {}': fresh failure in the negative cache",
                    source.provider_id, artist, track
                );
                report.skipped.push(source.provider_id);
                continue;
            }

            let probe = match self
                .fetch_source(&source.provider_id, artist, track, duration_sec)
                .await
            {
                Ok(mut lyrics) if Self::content_matches_policy(&lyrics, track, &config) => {
                    lyrics.lines.sort_by_key(|line| line.start_time_ms);
                    let level = response_sync_level(&lyrics);
                    SourceOutcome::Candidate {
                        level,
                        value: lyrics,
                    }
                }
                // A genuine payload rejected by policy is negative evidence
                // too: re-probing the same Fuente would only reach the same
                // verdict until the preference or the payload changes.
                Ok(_) => {
                    report.failures.push(LyricsSourceFailure {
                        provider_id: source.provider_id.clone(),
                        kind: LyricsFailureKind::PolicyRejected,
                        detail: "lyrics rejected by the configured content policy".to_string(),
                    });
                    SourceOutcome::NoCandidate
                }
                // Per-source failures (rate limits, HTTP/network/parse errors,
                // auth) only skip that source: the cascade never aborts.
                // Classification of the final failure stays in
                // `orchestrate_resolution`.
                Err(error) => {
                    report.failures.push(LyricsSourceFailure {
                        provider_id: source.provider_id.clone(),
                        kind: classify_source_error(&error),
                        detail: error.to_string(),
                    });
                    SourceOutcome::NoCandidate
                }
            };
            if selection.offer(probe) == SelectionOutcome::Stop {
                break;
            }
        }

        match selection.finish() {
            Some(response) => {
                report.response = Some(response);
            }
            None => {
                report.error =
                    Some("No lyrics matched enabled sources and lyrics policy".to_string());
            }
        }
        report
    }

    /// Convert to LRC format string
    pub fn to_lrc_string(lyrics: &LyricsResponse) -> String {
        let mut lrc = String::new();
        for line in &lyrics.lines {
            let timestamp = ms_to_lrc_timestamp(line.start_time_ms);
            lrc.push_str(&format!("{}{}\n", timestamp, line.words));
        }
        lrc
    }

    /// Orchestrate resolution across the full multi-tier priority cascade (16 tiers)
    /// Returns (LyricsResolution, elapsed_ms).
    pub async fn orchestrate_resolution(
        &self,
        artist: &str,
        track: &str,
        album: Option<&str>,
        duration_sec: f64,
    ) -> (LyricsResolution, u64) {
        let report = self
            .orchestrate_resolution_reporting(artist, track, album, duration_sec, &HashSet::new())
            .await;
        (report.resolution, report.elapsed_ms)
    }

    /// Same orchestration, returning the per-Fuente evidence the persistent
    /// negative cache needs. `skip` carries the Fuentes with a fresh failure:
    /// they are not contacted and appear in `skipped`.
    pub async fn orchestrate_resolution_reporting(
        &self,
        artist: &str,
        track: &str,
        _album: Option<&str>,
        duration_sec: f64,
        skip: &HashSet<String>,
    ) -> LyricsResolutionReport {
        let start = Instant::now();

        // 1. Try full multi-tier priority resolution cascade
        let cascade = self
            .fetch_all_sources_skipping(artist, track, duration_sec, skip)
            .await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        let resolution = match cascade.response {
            Some(response) => response.to_domain_resolution(),
            None => {
                let err_str = cascade.error.unwrap_or_default();
                if err_str.contains("401") || err_str.contains("auth") || err_str.contains("sp_dc")
                {
                    LyricsResolution::new_requires_auth("Spotify", "color_lyrics", err_str)
                } else if err_str.contains("failed")
                    || err_str.contains("timed out")
                    || err_str.contains("network")
                {
                    LyricsResolution::new_source_unavailable(
                        "Orchestrator",
                        "multi_provider_cascade",
                        err_str,
                    )
                } else {
                    LyricsResolution::new_not_found("Orchestrator", "multi_provider_cascade")
                }
            }
        };

        LyricsResolutionReport {
            resolution,
            elapsed_ms,
            failures: cascade.failures,
            skipped: cascade.skipped,
        }
    }
}

/// Sync level of a fetched response, shared by search filtering and the
/// sequential cascade so both rank payloads the same way.
pub(crate) fn response_sync_level(lyrics: &LyricsResponse) -> SyncLevel {
    if lyrics.elrc_content.as_ref().is_some_and(|s| !s.is_empty())
        || matches!(
            lyrics.sync_type.as_str(),
            "KARAOKE_WORD_SYNCED" | "WORD_SYNCED"
        )
    {
        SyncLevel::Word
    } else if lyrics.sync_type == "LINE_SYNCED" || !lyrics.lines.is_empty() {
        SyncLevel::Line
    } else {
        SyncLevel::Plain
    }
}

/// Validate lyrics response quality (ensures content is genuine and not just title/artist placeholder)
pub fn is_valid_lyrics(lyrics: &LyricsResponse, expected_title: &str) -> bool {
    if lyrics.lines.is_empty()
        && lyrics
            .plain_lyrics
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return false;
    }

    let title_clean = expected_title.to_lowercase().trim().to_string();
    let non_title_lines = lyrics
        .lines
        .iter()
        .filter(|l| {
            let line_clean = l.words.to_lowercase().trim().to_string();
            !line_clean.is_empty()
                && line_clean != title_clean
                && !line_clean.starts_with("作词")
                && !line_clean.starts_with("作曲")
        })
        .count();

    if non_title_lines < 1
        && lyrics
            .plain_lyrics
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        debug!(
            "[LyricsValidation] Rejected lyrics for '{}': placeholder title-only content",
            expected_title
        );
        return false;
    }

    true
}
