//! Adaptador Tidal para el contrato de sincronización (seam 2): favoritos vía
//! `LibrarySource` y páginas de playlists, álbumes y artistas para el
//! recorrido completo (ADR-0006, decisiones 11-12).
//!
//! Tidal no publica una versión por playlist: la huella disponible es el
//! conteo de pistas (`numberOfTracks`). Los ítems viajan crudos, como en los
//! demás adaptadores.
//!
//! Errores: el cliente ya marca 401/403 con `RequiresAuth:`; el adaptador lo
//! mapea a `AuthExpired`. Los 429 los reintenta el cliente respetando
//! `Retry-After` (con el techo de `rate_limiter`); si persiste, se devuelve
//! `Retryable` para el backoff del job.

use super::source::{CollectionPage, FavoritesPage, LibrarySource, SourceError, SourceErrorKind};

/// Tamaño de página del adaptador Tidal. La API acepta hasta 100; 50 mantiene
/// el mismo ritmo conservador que los demás brazos.
pub const TIDAL_PAGE_SIZE: i64 = 50;

/// Estado HTTP embebido en los errores de `TidalClient` (`Tidal API error 429:
/// …`). Los rechazos de sesión llegan con el prefijo `RequiresAuth:`.
fn tidal_error_status(message: &str) -> Option<u16> {
    let rest = message
        .strip_prefix("Tidal API error ")
        .or_else(|| message.split_once("HTTP ").map(|(_, rest)| rest))?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Taxonomía del contrato para los errores de Tidal.
pub fn classify_tidal_error(message: &str) -> SourceErrorKind {
    if message.contains("RequiresAuth") {
        return SourceErrorKind::AuthExpired;
    }
    match tidal_error_status(message) {
        Some(401 | 403) => SourceErrorKind::AuthExpired,
        Some(400 | 404 | 422) => SourceErrorKind::Permanent,
        _ => SourceErrorKind::Retryable,
    }
}

fn source_error(message: String) -> SourceError {
    SourceError::new(classify_tidal_error(&message), message)
}

fn serialized_items<T: serde::Serialize>(
    items: Vec<T>,
) -> Result<Vec<serde_json::Value>, SourceError> {
    items
        .into_iter()
        .map(|item| {
            serde_json::to_value(item).map_err(|error| {
                SourceError::permanent(format!("Tidal item serialization failed: {}", error))
            })
        })
        .collect()
}

pub struct TidalLibrarySource<'a> {
    client: &'a crate::services::TidalClient,
}

impl<'a> TidalLibrarySource<'a> {
    pub fn new(client: &'a crate::services::TidalClient) -> Self {
        Self { client }
    }

    /// Página de álbumes favoritos (`/users/{id}/favorites/albums`).
    pub async fn fetch_albums_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let page = self
            .client
            .get_favorite_albums(offset as i32, TIDAL_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(CollectionPage {
            items: serialized_items(page.items)?,
            reported_total: (page.total >= 0).then_some(page.total as i64),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }

    /// Página de artistas favoritos (`/users/{id}/favorites/artists`).
    pub async fn fetch_artists_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let page = self
            .client
            .get_favorite_artists(offset as i32, TIDAL_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(CollectionPage {
            items: serialized_items(page.items)?,
            reported_total: (page.total >= 0).then_some(page.total as i64),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }

    #[allow(dead_code)]
    // Huella de playlist: la cubren los tests de #66 y el sync de playlists la adoptará; el bin aún no la llama.
    /// Página de playlists. Tidal no expone `snapshot_id`: la huella es el
    /// conteo de pistas que declara cada playlist.
    pub async fn fetch_playlists_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let page = self
            .client
            .get_playlists(offset as i32, TIDAL_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        let fingerprints = page
            .items
            .iter()
            .map(|playlist| Some(playlist.track_count.to_string()))
            .collect();
        Ok(CollectionPage {
            items: serialized_items(page.items)?,
            reported_total: (page.total >= 0).then_some(page.total as i64),
            fingerprints,
            next_cursor: None,
        })
    }
}

impl LibrarySource for TidalLibrarySource<'_> {
    fn service_name(&self) -> &str {
        "tidal"
    }

    fn favorites_page_size(&self) -> i64 {
        TIDAL_PAGE_SIZE
    }

    async fn fetch_favorites_page(&self, offset: i64) -> Result<FavoritesPage, SourceError> {
        let page = self
            .client
            .get_favorites(offset as i32, TIDAL_PAGE_SIZE as i32)
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
    fn maps_auth_statuses_and_requires_auth_to_auth_expired() {
        assert_eq!(
            classify_tidal_error("RequiresAuth: Tidal API authentication failed (HTTP 401)"),
            SourceErrorKind::AuthExpired
        );
        assert_eq!(
            classify_tidal_error("Tidal API error 403: forbidden"),
            SourceErrorKind::AuthExpired
        );
    }

    #[test]
    fn maps_rate_limits_and_server_errors_to_retryable() {
        assert_eq!(
            classify_tidal_error("Tidal API error 429: Too Many Requests"),
            SourceErrorKind::Retryable
        );
        assert_eq!(
            classify_tidal_error("Tidal API error 503: unavailable"),
            SourceErrorKind::Retryable
        );
        assert_eq!(
            classify_tidal_error("Failed to parse albums: eof"),
            SourceErrorKind::Retryable
        );
        assert_eq!(
            classify_tidal_error("Tidal API error 404: gone"),
            SourceErrorKind::Permanent
        );
    }
}
