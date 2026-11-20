//! Caché en memoria de resoluciones por identidad (artista, título, álbum);
//! pre-carga y limpieza solo para pruebas.

use std::collections::HashMap;
use std::sync::RwLock;

use syncify_lyrics_domain::LyricsResolution;

/// Identity-level in-memory lyrics cache
// En `src-tauri` el mismo tipo estaba cubierto por `type_complexity = "allow"`.
#[allow(clippy::type_complexity)]
pub(crate) static LYRICS_CACHE: RwLock<
    Option<HashMap<String, (LyricsResolution, Option<String>)>>,
> = RwLock::new(None);

/// Clear in-memory lyrics cache (useful for testing)
#[allow(dead_code)] // Cubierta por `tests/batch_cache_optimization_and_parity_test.rs`.
pub fn clear_lyrics_cache() {
    if let Ok(mut guard) = LYRICS_CACHE.write() {
        *guard = Some(HashMap::new());
    }
}

/// Pre-seed lyrics cache (useful for testing)
#[allow(dead_code)] // Cubierta por `tests/batch_cache_optimization_and_parity_test.rs`.
pub fn set_cached_lyrics(
    artist: &str,
    title: &str,
    album: Option<&str>,
    resolution: LyricsResolution,
    sidecar: Option<String>,
) {
    let cache_key = format!(
        "{}:::{}:::{}",
        artist.to_lowercase().trim(),
        title.to_lowercase().trim(),
        album.unwrap_or("").to_lowercase().trim()
    );
    if let Ok(mut guard) = LYRICS_CACHE.write() {
        let cache = guard.get_or_insert_with(HashMap::new);
        cache.insert(cache_key, (resolution, sidecar));
    }
}
