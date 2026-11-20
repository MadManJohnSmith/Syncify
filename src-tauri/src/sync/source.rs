//! Seam 2 (ADR-0006, decisión del spec «Adaptador de proveedor»): la interfaz
//! única de proveedor para el contrato de sincronización.
//!
//! Reglas del contrato que viven aquí:
//!
//! - Cada adaptador declara su tamaño de página y su cursor; el orquestador
//!   (el job del seam 1) no conoce offsets concretos.
//! - Los errores son tipados: `Retryable` reintenta con backoff,
//!   `AuthExpired` (401/403) detiene solo esa cuenta, `Unsupported` declara
//!   una capacidad ausente, `Permanent` es un hueco que se registra y la
//!   corrida continúa.
//! - Los ítems viajan como `serde_json::Value` crudos: el importador por
//!   servicio los decodifica con sus tipos propios, igual que haría contra la
//!   API real. Así un proveedor falso alimenta el mismo camino sin red.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Colecciones del contrato (ADR-0006, decisión 7): favoritos y playlists
/// siempre; álbumes y artistas según capacidades declaradas. Este ticket solo
/// recorre `Favorites`; el resto queda declarado para sus tickets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncScope {
    Favorites,
    Playlists,
    Albums,
    Artists,
}

impl SyncScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncScope::Favorites => "favorites",
            SyncScope::Playlists => "playlists",
            SyncScope::Albums => "albums",
            SyncScope::Artists => "artists",
        }
    }

    /// Acepta el literal del contrato; cualquier otra cosa es inválida (la UI
    /// no promete colecciones que el contrato no expone).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "favorites" => Some(SyncScope::Favorites),
            "playlists" => Some(SyncScope::Playlists),
            "albums" => Some(SyncScope::Albums),
            "artists" => Some(SyncScope::Artists),
            _ => None,
        }
    }
}

/// Taxonomía de errores del adaptador (ADR-0006 «Errores tipados»).
/// `Unsupported` es la declaración de capacidad ausente (decisión 7): los
/// adaptadores de colecciones futuras la devuelven hasta sus tickets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // La taxonomía completa es contrato (ADR-0006); `Unsupported` y los constructores la consumen los tests y los tickets siguientes.
pub enum SourceErrorKind {
    /// Reintenta con backoff; si persiste, hueco registrado y la corrida
    /// continúa (decisión 17).
    Retryable,
    /// 401/403: detiene solo esa cuenta y la marca para reautenticar
    /// (decisión 20).
    AuthExpired,
    /// Capacidad ausente: el proveedor no expone la colección (decisión 7).
    Unsupported,
    /// Petición malformada o no recuperable: hueco registrado.
    Permanent,
}

/// Error del adaptador, con la taxonomía y el mensaje crudo del proveedor.
/// El mensaje conserva el texto original del cliente para que la
/// clasificación y los avisos existentes (p. ej. `is_qobuz_auth_error`)
/// sigan aplicando sobre la misma cadena.
#[derive(Debug, Clone)]
pub struct SourceError {
    pub kind: SourceErrorKind,
    pub message: String,
}

#[allow(dead_code)] // Constructores del contrato tipado; cubiertos por `tests/sync_run_job_fake_source_test.rs`.
impl SourceError {
    pub fn new(kind: SourceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn retryable(message: impl Into<String>) -> Self {
        Self::new(SourceErrorKind::Retryable, message)
    }

    pub fn auth_expired(message: impl Into<String>) -> Self {
        Self::new(SourceErrorKind::AuthExpired, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(SourceErrorKind::Unsupported, message)
    }

    pub fn permanent(message: impl Into<String>) -> Self {
        Self::new(SourceErrorKind::Permanent, message)
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SourceError {}

/// Una página de favoritos: ítems crudos del proveedor y el total que este
/// reporta para la colección, si lo publica (decisión de cobertura: un total
/// ausente es «desconocido», nunca un 100 % inventado). El total se asigna
/// una vez por página, no se acumula ni se sobrescribe entre páginas.
#[derive(Debug, Clone, Default)]
pub struct FavoritesPage {
    pub items: Vec<serde_json::Value>,
    pub reported_total: Option<i64>,
}

/// Una página de una colección paginada más allá de favoritos (playlists,
/// álbumes y artistas). Los ítems viajan crudos, igual que en
/// [`FavoritesPage`]; `reported_total` es el total de la colección que publica
/// el proveedor y `fingerprints` lleva una huella por ítem cuando el
/// proveedor la expone (`snapshot_id` en Spotify; el conteo en Qobuz y Tidal).
///
/// `next_cursor` solo se usa en colecciones con cursor (Spotify `/me/following`);
/// en las de offset queda en `None`.
#[derive(Debug, Clone, Default)]
pub struct CollectionPage {
    pub items: Vec<serde_json::Value>,
    pub reported_total: Option<i64>,
    pub fingerprints: Vec<Option<String>>,
    pub next_cursor: Option<String>,
}

/// La interfaz única de proveedor (seam 2). Cada colección es una operación
/// por página; el adaptador declara el tamaño de página y el job decide el
/// recorrido y la cancelación en el límite de página.
pub trait LibrarySource: Send + Sync {
    fn service_name(&self) -> &str;

    /// Tamaño de página declarado por el adaptador.
    fn favorites_page_size(&self) -> i64 {
        50
    }

    /// Página de favoritos en el cursor pedido. El error tipado decide:
    /// `Retryable` reintenta, `AuthExpired` aborta la cuenta, `Permanent` o
    /// `Unsupported` registran el hueco.
    fn fetch_favorites_page(
        &self,
        offset: i64,
    ) -> impl std::future::Future<Output = Result<FavoritesPage, SourceError>> + Send;
}

/// Reintento de una página con backoff exponencial, misma cadencia que
/// `qobuz_sync_with_retry` (3 intentos, 500 ms duplicando). Solo `Retryable`
/// reintenta; los demás tipos se devuelven tal cual.
pub(crate) async fn retry_source_page<F, Fut>(mut fetch: F) -> Result<FavoritesPage, SourceError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<FavoritesPage, SourceError>>,
{
    const MAX_ATTEMPTS: u32 = 3;
    let mut attempt: u32 = 0;
    loop {
        match fetch().await {
            Ok(page) => return Ok(page),
            Err(error)
                if attempt + 1 < MAX_ATTEMPTS && error.kind == SourceErrorKind::Retryable =>
            {
                let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt));
                tracing::warn!(
                    "library source page fetch failed (attempt {}/{}), retrying in {:?}: {}",
                    attempt + 1,
                    MAX_ATTEMPTS,
                    delay,
                    error
                );
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(error) => {
                tracing::error!(
                    "library source page fetch failed after {} attempt(s): {}",
                    attempt + 1,
                    error
                );
                return Err(error);
            }
        }
    }
}
