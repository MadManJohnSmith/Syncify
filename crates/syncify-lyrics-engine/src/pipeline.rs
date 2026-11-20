//! Servicio de integración para los pipelines de descarga (Qobuz / Tidal):
//! resolución + sidecar con caché por identidad. La fachada aporta el
//! constructor que consume la configuración persistida.

use std::collections::HashMap;

use anyhow::Result;
use syncify_lyrics_domain::{LyricsResolution, ResolutionStatus};

use crate::cache::LYRICS_CACHE;
use crate::client::LyricsClient;

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
