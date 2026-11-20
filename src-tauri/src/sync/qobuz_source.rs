//! Adaptador Qobuz para el contrato de sincronización (seam 2): el
//! sincronizador de Qobuz pasa a implementar `LibrarySource` (issue #65).
//!
//! Envuelve el `QobuzClient` existente con la misma petición firmada que el
//! brazo del orquestador (`favorite/getUserFavorites?type=tracks` con
//! `offset`/`limit`), de modo que la costura de entorno `SYNCIFY_QOBUZ_API_BASE`
//! (PR #298) sigue aplicando y la regresión de forma de petición queda vigente.
//! Los ítems viajan crudos (`serde_json::Value`): el importador decodifica con
//! los tipos reales del cliente, como contra la API real.
//!
//! Además de favoritos, el adaptador expone las páginas de playlists, álbumes
//! y artistas para el recorrido completo (ADR-0006, decisiones 11-12). Qobuz
//! no publica versión por playlist: la huella es el conteo que declara cada
//! playlist.

use super::source::{CollectionPage, FavoritesPage, LibrarySource, SourceError, SourceErrorKind};

/// Tamaño de página de favoritos del brazo Qobuz caracterizado (PR #298):
/// una página con `limit 50` cierra la paginación.
pub const QOBUZ_FAVORITES_PAGE_SIZE: i64 = 50;

/// Clasificación del error crudo del cliente Qobuz según la taxonomía del
/// contrato: auth/permiso/malformada/no-encontrada son terminales (misma
/// regla que `qobuz_error_is_retryable` del orquestador), el resto reintentan.
fn classify_qobuz_error(message: &str) -> SourceErrorKind {
    if crate::commands::service::is_qobuz_auth_error(message)
        || message.contains("login rejected")
        || message.contains("Failed to parse Qobuz login")
    {
        return SourceErrorKind::AuthExpired;
    }
    if let Some(status) = crate::commands::service::qobuz_error_status(message) {
        if status == "403" {
            return SourceErrorKind::AuthExpired;
        }
        if matches!(status, "400" | "404" | "422") {
            return SourceErrorKind::Permanent;
        }
    }
    SourceErrorKind::Retryable
}

pub struct QobuzLibrarySource<'a> {
    client: &'a crate::services::QobuzClient,
}

impl<'a> QobuzLibrarySource<'a> {
    pub fn new(client: &'a crate::services::QobuzClient) -> Self {
        Self { client }
    }

    async fn fetch_typed_page(
        &self,
        collection: &str,
        offset: i64,
    ) -> Result<serde_json::Value, SourceError> {
        let params = vec![
            ("type", collection.to_string()),
            ("offset", offset.to_string()),
            ("limit", QOBUZ_FAVORITES_PAGE_SIZE.to_string()),
        ];
        self.client
            .api_request("favorite/getUserFavorites", params, true)
            .await
            .map_err(|e| SourceError::new(classify_qobuz_error(&e), e))
    }

    /// Página de álbumes favoritos (`type=albums`).
    pub async fn fetch_albums_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let page = self.fetch_typed_page("albums", offset).await?;
        Ok(CollectionPage {
            items: page["albums"]["items"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            reported_total: page["albums"]["total"].as_i64(),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }

    /// Página de artistas favoritos (`type=artists`).
    pub async fn fetch_artists_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let page = self.fetch_typed_page("artists", offset).await?;
        Ok(CollectionPage {
            items: page["artists"]["items"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            reported_total: page["artists"]["total"].as_i64(),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }

    #[allow(dead_code)]
    // Huella de playlist: la cubren los tests de #66 y el sync de playlists la adoptará; el bin aún no la llama.
    /// Página de playlists del usuario con la huella de conteo de cada una.
    pub async fn fetch_playlists_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let params = vec![
            ("offset", offset.to_string()),
            ("limit", QOBUZ_FAVORITES_PAGE_SIZE.to_string()),
        ];
        let page: serde_json::Value = self
            .client
            .api_request("playlist/getUserPlaylists", params, true)
            .await
            .map_err(|e| SourceError::new(classify_qobuz_error(&e), e))?;
        let items = page["playlists"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let fingerprints = items
            .iter()
            .map(|playlist| {
                playlist["tracks_count"]
                    .as_i64()
                    .map(|count| count.to_string())
            })
            .collect();
        Ok(CollectionPage {
            items,
            reported_total: page["playlists"]["total"].as_i64(),
            fingerprints,
            next_cursor: None,
        })
    }
}

impl LibrarySource for QobuzLibrarySource<'_> {
    fn service_name(&self) -> &str {
        "qobuz"
    }

    fn favorites_page_size(&self) -> i64 {
        QOBUZ_FAVORITES_PAGE_SIZE
    }

    async fn fetch_favorites_page(&self, offset: i64) -> Result<FavoritesPage, SourceError> {
        let params = vec![
            ("type", "tracks".to_string()),
            ("offset", offset.to_string()),
            ("limit", QOBUZ_FAVORITES_PAGE_SIZE.to_string()),
        ];
        let page: serde_json::Value = self
            .client
            .api_request("favorite/getUserFavorites", params, true)
            .await
            .map_err(|e| SourceError::new(classify_qobuz_error(&e), e))?;

        let items = page["tracks"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let reported_total = page["tracks"]["total"].as_i64();
        Ok(FavoritesPage {
            items,
            reported_total,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_auth_statuses_to_auth_expired() {
        assert_eq!(
            classify_qobuz_error("Qobuz API error (401): unauthorized"),
            SourceErrorKind::AuthExpired
        );
        assert_eq!(
            classify_qobuz_error("Qobuz API error (403): forbidden"),
            SourceErrorKind::AuthExpired
        );
        assert_eq!(
            classify_qobuz_error("Authentication required"),
            SourceErrorKind::AuthExpired
        );
    }

    #[test]
    fn maps_withdrawn_requests_to_permanent_and_rate_limits_to_retryable() {
        assert_eq!(
            classify_qobuz_error("Qobuz API error (400): bad request"),
            SourceErrorKind::Permanent
        );
        assert_eq!(
            classify_qobuz_error("Qobuz API error (429): slow down"),
            SourceErrorKind::Retryable
        );
        assert_eq!(
            classify_qobuz_error("Request failed: connection reset"),
            SourceErrorKind::Retryable
        );
    }
}
