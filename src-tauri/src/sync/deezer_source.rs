//! Adaptador Deezer para el contrato de sincronización (seam 2): favoritos vía
//! `LibrarySource` y páginas de álbumes, artistas, playlists y pistas de
//! playlist para el recorrido completo (ADR-0006, decisiones 11-12 y 14).
//!
//! El total de cada colección lo publica la API en **cada** página; el
//! adaptador lo expone tal cual y el recorrido compartido
//! ([`super::collection`]) lo asigna una sola vez. Sumarlo por página
//! sobrecontaba el reporte de cobertura (#405).
//!
//! Errores: los clientes devuelven `Result<_, String>`; aquí se traducen a la
//! taxonomía del contrato con el mismo criterio que T02. `401/403` (y el
//! prefijo `RequiresAuth`) son `AuthExpired`; `400/404/422` son `Permanent`;
//! un `429` que el cliente ya reintentó respetando `Retry-After`, un `5xx` o
//! un fallo de transporte son `Retryable`.
//!
//! La cuota de aplicación (50 peticiones/5 s) la comparten todas las cuentas
//! del servicio: el limitador global de `rate_limiter` la aplica por servicio,
//! y las corridas de cuentas del mismo servicio se serializan en
//! `concurrency_manager::acquire_service_sync_gate`.

use super::source::{CollectionPage, FavoritesPage, LibrarySource, SourceError, SourceErrorKind};

/// Tamaño de página del adaptador: el mismo valor que usaba el brazo de sync
/// antes de mover la lectura al adaptador, para que la cuota no cambie.
pub const DEEZER_PAGE_SIZE: i64 = 100;

/// Estado HTTP embebido en los errores de `DeezerClient`
/// (`Deezer API error (429 Too Many Requests): …`).
fn deezer_error_status(message: &str) -> Option<u16> {
    let rest = message.strip_prefix("Deezer API error (")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() == 3 {
        digits.parse().ok()
    } else {
        None
    }
}

/// Taxonomía del contrato para los errores del cliente Deezer.
pub fn classify_deezer_error(message: &str) -> SourceErrorKind {
    if message.contains("RequiresAuth") {
        return SourceErrorKind::AuthExpired;
    }
    match deezer_error_status(message) {
        Some(401 | 403) => SourceErrorKind::AuthExpired,
        Some(400 | 404 | 422) => SourceErrorKind::Permanent,
        _ => SourceErrorKind::Retryable,
    }
}

fn source_error(message: String) -> SourceError {
    SourceError::new(classify_deezer_error(&message), message)
}

fn serialized_items<T: serde::Serialize>(
    items: Vec<T>,
) -> Result<Vec<serde_json::Value>, SourceError> {
    items
        .into_iter()
        .map(|item| {
            serde_json::to_value(item).map_err(|error| {
                SourceError::permanent(format!("Deezer item serialization failed: {}", error))
            })
        })
        .collect()
}

pub struct DeezerLibrarySource<'a> {
    client: &'a crate::services::DeezerClient,
}

impl<'a> DeezerLibrarySource<'a> {
    pub fn new(client: &'a crate::services::DeezerClient) -> Self {
        Self { client }
    }

    /// El listado público exige el id numérico que `init()` resolvió; sin él
    /// la sesión no sirve para leer la colección (mismo veredicto que el brazo
    /// de sync: RequiresAuth).
    fn user_id(&self) -> Result<String, SourceError> {
        self.client.user_id().ok_or_else(|| {
            SourceError::auth_expired("Deezer user id unavailable (session not initialized)")
        })
    }

    /// Página de álbumes favoritos (`/user/{id}/albums`).
    pub async fn fetch_albums_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let user_id = self.user_id()?;
        let (albums, total) = self
            .client
            .get_user_albums_public(&user_id, offset as i32, DEEZER_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(CollectionPage {
            items: serialized_items(albums)?,
            reported_total: (total > 0).then_some(total),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }

    /// Página de artistas seguidos (`/user/{id}/artists`).
    pub async fn fetch_artists_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let user_id = self.user_id()?;
        let (artists, total) = self
            .client
            .get_user_artists_public(&user_id, offset as i32, DEEZER_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(CollectionPage {
            items: serialized_items(artists)?,
            reported_total: (total > 0).then_some(total),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }

    #[allow(dead_code)]
    // Huella de playlist: la cubren los tests de #405 y el brazo de playlists
    // la adoptará al mover su recorrido al adaptador; el bin aún no la llama.
    /// Página de playlists de la cuenta. El listado público no publica
    /// `mod_date`; la huella disponible es el conteo de pistas que declara
    /// cada playlist, igual que en Tidal y Qobuz.
    pub async fn fetch_playlists_page(&self, offset: i64) -> Result<CollectionPage, SourceError> {
        let user_id = self.user_id()?;
        let (playlists, total) = self
            .client
            .get_user_playlists_public(&user_id, offset as i32, DEEZER_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        let fingerprints = playlists
            .iter()
            .map(|playlist| playlist.nb_tracks.map(|count| count.max(0).to_string()))
            .collect();
        Ok(CollectionPage {
            items: serialized_items(playlists)?,
            reported_total: (total > 0).then_some(total),
            fingerprints,
            next_cursor: None,
        })
    }

    #[allow(dead_code)]
    // Cubierta por los tests de #405; el recorrido de playlists del bin la
    // adoptará cuando mueva su expansión al adaptador.
    /// Página de pistas de una playlist (`/playlist/{id}/tracks`).
    pub async fn fetch_playlist_tracks_page(
        &self,
        playlist_id: &str,
        offset: i64,
    ) -> Result<CollectionPage, SourceError> {
        let (tracks, total) = self
            .client
            .get_playlist_tracks_public(playlist_id, offset as i32, DEEZER_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(CollectionPage {
            items: serialized_items(tracks)?,
            reported_total: (total > 0).then_some(total),
            fingerprints: Vec::new(),
            next_cursor: None,
        })
    }
}

impl LibrarySource for DeezerLibrarySource<'_> {
    fn service_name(&self) -> &str {
        "deezer"
    }

    fn favorites_page_size(&self) -> i64 {
        DEEZER_PAGE_SIZE
    }

    async fn fetch_favorites_page(&self, offset: i64) -> Result<FavoritesPage, SourceError> {
        let user_id = self.user_id()?;
        let (tracks, total) = self
            .client
            .get_favorites_public(&user_id, offset as i32, DEEZER_PAGE_SIZE as i32)
            .await
            .map_err(source_error)?;
        Ok(FavoritesPage {
            items: serialized_items(tracks)?,
            // `0` significa «no publicado», no «colección vacía»: la cobertura
            // queda `unknown` en vez de inventar un 100 %.
            reported_total: (total > 0).then_some(total as i64),
        })
    }
}
