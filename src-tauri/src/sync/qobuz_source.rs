//! Adaptador Qobuz para el contrato de sincronización (seam 2): el
//! sincronizador de Qobuz pasa a implementar `LibrarySource` (issue #65).
//!
//! Envuelve el `QobuzClient` existente con la misma petición firmada que el
//! brazo del orquestador (`favorite/getUserFavorites?type=tracks` con
//! `offset`/`limit`), de modo que la costura de entorno `SYNCIFY_QOBUZ_API_BASE`
//! (PR #298) sigue aplicando y la regresión de forma de petición queda vigente.
//! Los ítems viajan crudos (`serde_json::Value`): el importador decodifica con
//! los tipos reales del cliente, como contra la API real.

use super::source::{FavoritesPage, LibrarySource, SourceError, SourceErrorKind};

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
        if matches!(status, "400" | "403" | "404" | "422") {
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
