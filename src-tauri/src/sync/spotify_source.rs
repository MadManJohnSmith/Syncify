//! Adaptador Spotify para el contrato de sincronización (seam 2): favoritos
//! vía `LibrarySource` y páginas de playlists, álbumes y artistas para el
//! recorrido completo del ADR-0006 (decisiones 11-12).
//!
//! Los ítems viajan crudos, igual que en el adaptador Qobuz: el importador
//! del servicio los decodifica con sus tipos. El adaptador solo decide el
//! tamaño de página, lee el total una vez y expone la huella por playlist
//! (`snapshot_id`, A-02).
//!
//! Errores: 401/403 → `AuthExpired` (detiene solo esa cuenta, decisión 20);
//! 429 lo reintenta el `HttpRetryPolicy` del cliente respetando `Retry-After`
//! con el techo de `rate_limiter`; el resto de fallos transitorios se
//! devuelven como `Retryable` para el backoff del job.

use super::source::{CollectionPage, FavoritesPage, LibrarySource, SourceError, SourceErrorKind};

/// Tamaño de página de Spotify para favoritos, álbumes y playlists (el máximo
/// que acepta la API; `/me/following` usa el mismo límite).
pub const SPOTIFY_PAGE_SIZE: i64 = 50;

/// Estado HTTP embebido en los errores de `SpotifyClient` (`Spotify API error
/// (401 Unauthorized): …`). Un mensaje sin estado es transporte o parseo.
fn spotify_error_status(message: &str) -> Option<u16> {
    let rest = message.strip_prefix("Spotify API error (")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Taxonomía del contrato para los errores de Spotify.
pub fn classify_spotify_error(message: &str) -> SourceErrorKind {
    match spotify_error_status(message) {
        Some(401 | 403) => SourceErrorKind::AuthExpired,
        Some(400 | 404 | 422) => SourceErrorKind::Permanent,
        _ => SourceErrorKind::Retryable,
    }
}

fn source_error(message: String) -> SourceError {
    SourceError::new(classify_spotify_error(&message), message)
}

fn serialized_items<T: serde::Serialize>(
    items: Vec<T>,
) -> Result<Vec<serde_json::Value>, SourceError> {
    items
        .into_iter()
        .map(|item| {
            serde_json::to_value(item).map_err(|error| {
                SourceError::permanent(format!("Spotify item serialization failed: {}", error))
            })
        })
        .collect()
}

pub struct SpotifyLibrarySource<'a> {
    client: &'a crate::services::SpotifyClient,
}

impl<'a> SpotifyLibrarySource<'a> {
    pub fn new(client: &'a crate::services::SpotifyClient) -> Self {
        Self { client }
    }

    /// Página de álbumes guardados (`/me/albums`). Sin huella: Spotify no
    /// publica una versión por álbum guardado.
    pub async fn fetch_albums_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let page = self
            .client
            .get_saved_albums(offset as i32, SPOTIFY_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(CollectionPage {
            items: serialized_items(page.items)?,
            reported_total: (page.total >= 0).then_some(page.total as i64),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }

    /// Página de artistas seguidos (`/me/following`, cursor `after`).
    pub async fn fetch_artists_page(
        &self,
        after: Option<&str>,
    ) -> Result<CollectionPage, SourceError> {
        let response = self
            .client
            .get_followed_artists(after, SPOTIFY_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        let artists = response.artists;
        Ok(CollectionPage {
            items: serialized_items(artists.items)?,
            reported_total: artists.total.map(i64::from),
            fingerprints: Vec::new(),
            next_cursor: artists.cursors.and_then(|cursor| cursor.after),
        })
    }

    /// Página de playlists del usuario con su `snapshot_id` (A-02).
    pub async fn fetch_playlists_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let page = self
            .client
            .get_playlists(offset as i32, SPOTIFY_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        let fingerprints = page
            .items
            .iter()
            .map(|playlist| playlist.snapshot_id.clone())
            .collect();
        Ok(CollectionPage {
            items: serialized_items(page.items)?,
            reported_total: (page.total >= 0).then_some(page.total as i64),
            fingerprints,
            next_cursor: None,
        })
    }
}

impl LibrarySource for SpotifyLibrarySource<'_> {
    fn service_name(&self) -> &str {
        "spotify"
    }

    fn favorites_page_size(&self) -> i64 {
        SPOTIFY_PAGE_SIZE
    }

    async fn fetch_favorites_page(&self, offset: i64) -> Result<FavoritesPage, SourceError> {
        let page = self
            .client
            .get_saved_tracks(offset as i32, SPOTIFY_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(FavoritesPage {
            items: serialized_items(page.items)?,
            reported_total: (page.total >= 0).then_some(page.total as i64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_auth_statuses_to_auth_expired() {
        assert_eq!(
            classify_spotify_error("Spotify API error (401 Unauthorized): expired"),
            SourceErrorKind::AuthExpired
        );
        assert_eq!(
            classify_spotify_error("Spotify API error (403 Forbidden): scope"),
            SourceErrorKind::AuthExpired
        );
    }

    #[test]
    fn maps_malformed_requests_to_permanent_and_backoff_to_retryable() {
        assert_eq!(
            classify_spotify_error("Spotify API error (400 Bad Request): ids"),
            SourceErrorKind::Permanent
        );
        assert_eq!(
            classify_spotify_error("Spotify API error (429 Too Many Requests): slow"),
            SourceErrorKind::Retryable
        );
        assert_eq!(
            classify_spotify_error("Spotify API error (502 Bad Gateway)"),
            SourceErrorKind::Retryable
        );
        assert_eq!(
            classify_spotify_error("Request failed: connection reset"),
            SourceErrorKind::Retryable
        );
    }
}
