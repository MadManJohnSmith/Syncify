//! Servicio de integración para los pipelines de descarga (Qobuz / Tidal):
//! resolución + sidecar con caché por identidad. La fachada aporta el
//! constructor que consume la configuración persistida.

use std::collections::HashMap;

use anyhow::Result;
use syncify_lyrics_domain::{native_candidate_wins, LyricsResolution, ResolutionStatus, SyncLevel};

use crate::cache::LYRICS_CACHE;
use crate::client::{response_sync_level, LyricsClient};
use crate::config::LyricsEngineConfig;
use crate::types::LyricsResponse;

/// Generate sidecar `.lrc` file content (only for valid synced lyrics)
#[allow(dead_code)] // Cubierta por `tests/lyrics_pipeline_test.rs`.
pub fn generate_sidecar_lrc(resolution: &LyricsResolution) -> Option<String> {
    resolution.generate_sidecar_lrc()
}

/// Explicit integration interface for audio download pipelines (Qobuz / Tidal)
pub struct LyricsPipelineService {
    client: LyricsClient,
}

impl LyricsPipelineService {
    /// Motor con cliente y hooks ya inyectados por la fachada.
    pub fn with_client(client: LyricsClient) -> Self {
        Self { client }
    }

    /// Resolve lyrics and extract optional sidecar LRC content without touching audio files.
    pub async fn resolve_lyrics_and_sidecar(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
        duration_sec: f64,
    ) -> Result<(LyricsResolution, Option<String>), String> {
        self.resolve_generic(artist, title, album, duration_sec)
            .await
    }

    /// Resolve with a provider-native candidate competing against the generic
    /// engine (ADR-0017 decision 4, LY14/#192).
    ///
    /// The caller fetches the candidate through its own gateway and hands over
    /// the parsed payload: the Tidal pipeline has the track id and the account
    /// token, and the engine never reaches for them on its own. (That gateway
    /// stays unwired until the LY13/#182 spike confirms the endpoint.) The
    /// candidate is judged like any other Fuente — the same validity and
    /// language policy — and then wins only if its **Nivel de sincronía** is
    /// strictly finer than the accepted generic result; on a tie the generic
    /// source keeps its priority. `None`, a policy rejection or a coarser
    /// payload yields exactly what `resolve_lyrics_and_sidecar` returns, so a
    /// provider without a confirmed native endpoint cannot change resolution.
    pub async fn resolve_lyrics_and_sidecar_competing(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
        duration_sec: f64,
        native: Option<LyricsResponse>,
    ) -> Result<(LyricsResolution, Option<String>), String> {
        let (generic, _) = self
            .resolve_generic(artist, title, album, duration_sec)
            .await?;
        let winner = compete_with_native(&self.client.config(), title, generic, native);
        let sidecar = if winner.status == ResolutionStatus::Resolved {
            winner.generate_sidecar_lrc()
        } else {
            None
        };
        Ok((winner, sidecar))
    }

    /// Generic cascade with the identity cache. The cache stores this outcome
    /// only, so an offered native candidate is re-judged on every call instead
    /// of being frozen into the cached identity.
    async fn resolve_generic(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
        duration_sec: f64,
    ) -> Result<(LyricsResolution, Option<String>), String> {
        let cache_key = format!(
            "{}:::{}:::{}",
            artist.to_lowercase().trim(),
            title.to_lowercase().trim(),
            album.unwrap_or("").to_lowercase().trim()
        );

        // Check identity cache
        if let Ok(guard) = LYRICS_CACHE.read() {
            if let Some(ref cache) = *guard {
                if let Some(cached) = cache.get(&cache_key) {
                    tracing::debug!(
                        "[LyricsPipeline] Reusing cached lyrics for '{} - {}'",
                        artist,
                        title
                    );
                    return Ok(cached.clone());
                }
            }
        }

        let (resolution, _latency) = self
            .client
            .orchestrate_resolution(artist, title, album, duration_sec)
            .await;

        let sidecar_content = if resolution.status == ResolutionStatus::Resolved {
            resolution.generate_sidecar_lrc()
        } else {
            None
        };

        let result = (resolution, sidecar_content);

        // Cache resolution for this identity
        if let Ok(mut guard) = LYRICS_CACHE.write() {
            let cache = guard.get_or_insert_with(HashMap::new);
            cache.insert(cache_key, result.clone());
        }

        Ok(result)
    }
}

/// Chooses between the accepted generic resolution and an optional
/// provider-native candidate. Pure: no I/O, no cache, no network.
///
/// Policy first (a native payload that is not genuine lyrics or does not match
/// the preferred language is dropped, exactly as the cascade drops a Fuente),
/// then the sync level, then source priority (the tie keeps the generic
/// result). It never lowers the accepted level, so a native probe cannot
/// degrade what the generic engine already resolved.
pub fn compete_with_native(
    config: &LyricsEngineConfig,
    title: &str,
    generic: LyricsResolution,
    native: Option<LyricsResponse>,
) -> LyricsResolution {
    let Some(native) = native else {
        return generic;
    };
    if !LyricsClient::content_matches_policy(&native, title, config) {
        tracing::debug!(
            "[LyricsPipeline] Provider-native candidate rejected by the lyrics policy; keeping the generic result"
        );
        return generic;
    }

    let generic_level = (generic.status == ResolutionStatus::Resolved)
        .then(|| SyncLevel::from_sync_type(&generic.sync_type));
    let native_level = response_sync_level(&native);
    if !native_candidate_wins(native_level, generic_level) {
        tracing::debug!(
            native = ?native_level,
            generic = ?generic_level,
            "[LyricsPipeline] Provider-native candidate did not beat the generic result"
        );
        return generic;
    }

    tracing::info!(
        native = ?native_level,
        generic = ?generic_level,
        "[LyricsPipeline] Provider-native lyrics won the quality comparison"
    );
    native.to_domain_resolution()
}
