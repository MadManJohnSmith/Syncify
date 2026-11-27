//! P10a (#402): gateway de búsqueda de catálogo y adaptador Qobuz.
//!
//! Este módulo es solo servicio: no registra comandos IPC ni conoce la UI. La
//! interfaz [`CatalogSearchProvider`] cubre los tres tipos de entidad que la
//! búsqueda expone (pistas, álbumes y artistas) y el
//! [`CatalogSearchGateway`] mantiene el registro de proveedores, resuelve la
//! cuenta preferida por servicio (#121) y reparte los fallos para que el
//! proveedor caído no oculte a los demás.
//!
//! El adaptador Qobuz reutiliza los tipos de respuesta de
//! [`crate::services::qobuz`] y la política compartida de límites
//! ([`crate::services::rate_limiter`]) y errores transitorios
//! ([`crate::services::http_retry`]). La clasificación de fallos sigue el
//! contrato de T02 (#66): `401/403` es credencial inválida y no se reintenta;
//! `429` respeta `Retry-After` y reintenta con backoff; un `5xx`, un `408` o un
//! fallo de transporte también reintenta, y cualquier otro `4xx` es permanente.
//!
//! La costura `SYNCIFY_QOBUZ_API_BASE` (la misma que usan el resto de brazos
//! Qobuz) permite apuntar el adaptador a un proveedor simulado en los tests.
//! Los fixtures viven en este fichero: ninguna prueba sale a la red.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::http_retry::is_transient_status;
use super::qobuz::{
    deserialize_artist, deserialize_id, deserialize_id_i64, deserialize_opt_id_i64,
    get_qobuz_app_id, QobuzArtist, QobuzImage, QobuzTrack, QOBUZ_API_BASE, QOBUZ_API_BASE_ENV,
};
use super::rate_limiter::{penalize_on_rate_limit, DEFAULT_429_PENALTY, GLOBAL_RATE_LIMITER};

/// Tamaño de página que se usa cuando el llamador no fija uno.
pub const DEFAULT_CATALOG_PAGE_SIZE: u32 = 20;

/// Techo de página del gateway. Un límite mayor se recorta: el proveedor no
/// tiene que defenderse de un llamador que pida una página de 10.000 ítems.
pub const MAX_CATALOG_PAGE_SIZE: u32 = 100;

/// Número máximo de intentos de una petición de búsqueda (T02: 3 intentos).
const MAX_SEARCH_ATTEMPTS: u32 = 3;

/// Petición de página, independiente del proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CatalogPageRequest {
    pub offset: u32,
    pub limit: u32,
}

impl CatalogPageRequest {
    pub fn new(offset: u32, limit: u32) -> Self {
        Self {
            offset,
            limit: limit.clamp(1, MAX_CATALOG_PAGE_SIZE),
        }
    }
}

impl Default for CatalogPageRequest {
    fn default() -> Self {
        Self::new(0, DEFAULT_CATALOG_PAGE_SIZE)
    }
}

/// Una página ya normalizada. `total` es `None` cuando el proveedor no publica
/// un total; nunca se inventa (misma regla que la cobertura de ADR-0006).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogPage<T> {
    pub items: Vec<T>,
    pub total: Option<u64>,
    pub offset: u32,
    pub limit: u32,
}

impl<T> CatalogPage<T> {
    /// `true` si el proveedor declara más resultados después de esta página.
    /// Sin total reportado solo puede responderse por la página: una página con
    /// ítems puede tener continuación, una vacía no.
    pub fn has_more(&self) -> bool {
        match self.total {
            Some(total) => (self.offset as u64 + self.items.len() as u64) < total,
            None => !self.items.is_empty(),
        }
    }
}

/// Pista de catálogo normalizada para la búsqueda.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: i64,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<f64>,
}

/// Álbum de catálogo normalizado para la búsqueda.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogAlbum {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub track_count: Option<i64>,
    /// Marca temporal Unix que publica Qobuz; `None` si no la trae.
    pub released_at: Option<i64>,
    pub cover_url: Option<String>,
}

/// Artista de catálogo normalizado para la búsqueda.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogArtist {
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
    pub album_count: Option<i64>,
}

/// Taxonomía de fallo de búsqueda. Los cuatro primeros valores siguen el
/// contrato de T02 (#66); `RateLimited` conserva el `Retry-After` aplicado para
/// que el llamador pueda esperar sin volver a consultar al proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSearchErrorKind {
    /// 401/403: solo esa cuenta queda detenida; las demás siguen.
    AuthExpired,
    /// 429 persistente tras los reintentos acotados.
    RateLimited,
    /// 5xx, timeout o fallo de transporte persistente.
    Retryable,
    /// Petición malformada o respuesta ilegible.
    Permanent,
    /// El proveedor no expone la búsqueda pedida.
    Unsupported,
    /// El servicio no tiene cuenta preferida ni activa utilizables.
    NoAccount,
}

impl CatalogSearchErrorKind {
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::RateLimited | Self::Retryable)
    }
}

/// Fallo de una búsqueda, con la clasificación que decide si se reintenta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSearchError {
    pub kind: CatalogSearchErrorKind,
    pub message: String,
    pub retry_after_sec: Option<u64>,
}

impl CatalogSearchError {
    pub fn new(kind: CatalogSearchErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retry_after_sec: None,
        }
    }

    pub fn auth_expired(message: impl Into<String>) -> Self {
        Self::new(CatalogSearchErrorKind::AuthExpired, message)
    }

    pub fn retryable(message: impl Into<String>) -> Self {
        Self::new(CatalogSearchErrorKind::Retryable, message)
    }

    pub fn permanent(message: impl Into<String>) -> Self {
        Self::new(CatalogSearchErrorKind::Permanent, message)
    }

    pub fn no_account(service: &str) -> Self {
        Self::new(
            CatalogSearchErrorKind::NoAccount,
            format!(
                "No preferred or active account is available for {}",
                service
            ),
        )
    }

    pub fn rate_limited(message: impl Into<String>, retry_after: Duration) -> Self {
        Self {
            kind: CatalogSearchErrorKind::RateLimited,
            message: message.into(),
            retry_after_sec: Some(retry_after.as_secs()),
        }
    }
}

impl std::fmt::Display for CatalogSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CatalogSearchError {}

/// Fallo ya atribuido a un proveedor concreto dentro de una respuesta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogSearchFailure {
    pub provider: String,
    pub kind: CatalogSearchErrorKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_sec: Option<u64>,
}

impl CatalogSearchFailure {
    fn from_error(provider: &str, error: CatalogSearchError) -> Self {
        Self {
            provider: provider.to_string(),
            kind: error.kind,
            message: error.message,
            retry_after_sec: error.retry_after_sec,
        }
    }
}

/// Resultado de una de las tres secciones. Un proveedor puede devolver pistas
/// y fallar en álbumes: esa mezcla es un fallo parcial visible, no un `Err`
/// que esconda lo que sí llegó.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogSection<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<CatalogPage<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CatalogSearchFailure>,
}

impl<T> CatalogSection<T> {
    pub fn ok(page: CatalogPage<T>) -> Self {
        Self {
            page: Some(page),
            error: None,
        }
    }

    pub fn failed(error: CatalogSearchFailure) -> Self {
        Self {
            page: None,
            error: Some(error),
        }
    }

    pub fn is_ok(&self) -> bool {
        self.page.is_some()
    }
}

/// Resultado de un proveedor para una búsqueda: las tres secciones, cada una
/// con su página o su fallo, y la cuenta que el gateway eligió.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogProviderResponse {
    pub provider: String,
    pub account_id: Option<i64>,
    pub tracks: CatalogSection<CatalogTrack>,
    pub albums: CatalogSection<CatalogAlbum>,
    pub artists: CatalogSection<CatalogArtist>,
}

impl CatalogProviderResponse {
    pub fn has_failures(&self) -> bool {
        self.tracks.error.is_some() || self.albums.error.is_some() || self.artists.error.is_some()
    }
}

/// Respuesta completa de una búsqueda: todos los proveedores registrados,
/// incluso los que fallaron.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogSearchResponse {
    pub query: String,
    pub page: CatalogPageRequest,
    pub providers: Vec<CatalogProviderResponse>,
}

/// Petición validada. El constructor rechaza una consulta vacía antes de que
/// ningún proveedor abra red.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSearchRequest {
    pub query: String,
    pub page: CatalogPageRequest,
}

impl CatalogSearchRequest {
    pub fn new(
        query: impl Into<String>,
        page: CatalogPageRequest,
    ) -> Result<Self, CatalogSearchError> {
        let query = query.into().trim().to_string();
        if query.is_empty() {
            return Err(CatalogSearchError::permanent(
                "Catalog search query cannot be empty",
            ));
        }
        Ok(Self { query, page })
    }
}

/// Futuro boxeado del contrato de búsqueda. El trait es object-safe para que
/// el registro guarde `Arc<dyn CatalogSearchProvider>` y añadir un proveedor
/// no obligue a tocar el gateway ni la UI.
pub type CatalogSearchFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, CatalogSearchError>> + Send + 'a>>;

/// Contrato de un proveedor de búsqueda de catálogo.
pub trait CatalogSearchProvider: Send + Sync {
    /// Nombre canónico del servicio (minúsculas, el mismo de `services.name`).
    fn service_name(&self) -> &'static str;

    fn search_tracks<'a>(
        &'a self,
        account_id: Option<i64>,
        query: &'a str,
        page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogTrack>>;

    fn search_albums<'a>(
        &'a self,
        account_id: Option<i64>,
        query: &'a str,
        page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogAlbum>>;

    fn search_artists<'a>(
        &'a self,
        account_id: Option<i64>,
        query: &'a str,
        page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogArtist>>;
}

/// Registro de proveedores y reparto de la búsqueda.
///
/// La cuenta se resuelve aquí, una vez por servicio: la preferida de
/// `service_preferred_accounts` (#121) y, si no hay, la activa más reciente.
/// El proveedor solo ve el id; cada adaptador resuelve sus credenciales.
pub struct CatalogSearchGateway {
    db: Option<SqlitePool>,
    providers: BTreeMap<String, Arc<dyn CatalogSearchProvider>>,
}

impl CatalogSearchGateway {
    /// Gateway de producción: Qobuz registrado de fábrica.
    pub fn new(db: SqlitePool) -> Self {
        let mut gateway = Self {
            db: Some(db.clone()),
            providers: BTreeMap::new(),
        };
        gateway.register_provider(Arc::new(QobuzCatalogProvider::new(db)));
        gateway
    }

    /// Añade o sustituye un proveedor por su `service_name`. Es el punto de
    /// extensión para Tidal, Deezer o un proveedor falso de test.
    pub fn register_provider(&mut self, provider: Arc<dyn CatalogSearchProvider>) {
        self.providers
            .insert(provider.service_name().to_ascii_lowercase(), provider);
    }

    /// Busca en todos los proveedores registrados. Un fallo de un proveedor
    /// viaja dentro de su entrada; nunca elimina las entradas de los demás.
    pub async fn search(&self, request: &CatalogSearchRequest) -> CatalogSearchResponse {
        let mut jobs = Vec::with_capacity(self.providers.len());
        for (service, provider) in &self.providers {
            let account = self.resolve_account_id(service).await;
            jobs.push(search_provider(provider.as_ref(), account, request));
        }
        let providers = futures::future::join_all(jobs).await;
        CatalogSearchResponse {
            query: request.query.clone(),
            page: request.page,
            providers,
        }
    }

    /// Cuenta de búsqueda para un servicio:
    /// 1. la preferida (#121) si sigue existiendo, pertenece al servicio y está
    ///    habilitada;
    /// 2. la primera activa (la más reciente, mismo orden que el resto de
    ///    resoluciones de cuenta del repo).
    ///
    /// No se excluyen las cuentas con `credentials_invalid`: el adaptador las
    /// convierte en `AuthExpired`, que es un fallo visible de esa cuenta y no
    /// una razón para ocultar el servicio.
    async fn resolve_account_id(&self, service: &str) -> Result<Option<i64>, CatalogSearchError> {
        let Some(db) = &self.db else {
            return Ok(None);
        };

        let preferred: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT a.id
            FROM service_preferred_accounts p
            JOIN accounts a ON a.id = p.account_id
            JOIN services s ON s.id = a.service_id
            WHERE LOWER(p.service_name) = LOWER(?)
              AND LOWER(s.name) = LOWER(?)
              AND a.state = 'enabled'
              AND IFNULL(a.invalid_reason, '') <> 'logged_out'
            LIMIT 1
            "#,
        )
        .bind(service)
        .bind(service)
        .fetch_optional(db)
        .await
        .map_err(|e| {
            CatalogSearchError::permanent(format!(
                "Failed to read the preferred account for {}: {}",
                service, e
            ))
        })?;

        if preferred.is_some() {
            return Ok(preferred);
        }

        sqlx::query_scalar(
            r#"
            SELECT a.id
            FROM accounts a
            JOIN services s ON s.id = a.service_id
            WHERE LOWER(s.name) = LOWER(?)
              AND a.is_active = 1
              AND a.state = 'enabled'
              AND IFNULL(a.invalid_reason, '') <> 'logged_out'
            ORDER BY a.id DESC
            LIMIT 1
            "#,
        )
        .bind(service)
        .fetch_optional(db)
        .await
        .map_err(|e| {
            CatalogSearchError::permanent(format!(
                "Failed to read the active account for {}: {}",
                service, e
            ))
        })
    }
}

/// Ejecuta las tres búsquedas de un proveedor en paralelo y convierte cada
/// resultado en una sección. Un fallo de cuenta se replica en las tres
/// secciones para que la respuesta tenga siempre la misma forma.
async fn search_provider(
    provider: &dyn CatalogSearchProvider,
    account: Result<Option<i64>, CatalogSearchError>,
    request: &CatalogSearchRequest,
) -> CatalogProviderResponse {
    let provider_name = provider.service_name().to_string();

    let account_id = match account {
        Ok(account_id) => account_id,
        Err(error) => {
            let failure = CatalogSearchFailure::from_error(&provider_name, error);
            return CatalogProviderResponse {
                provider: provider_name,
                account_id: None,
                tracks: CatalogSection::failed(failure.clone()),
                albums: CatalogSection::failed(failure.clone()),
                artists: CatalogSection::failed(failure),
            };
        }
    };

    let (tracks, albums, artists) = tokio::join!(
        provider.search_tracks(account_id, &request.query, request.page),
        provider.search_albums(account_id, &request.query, request.page),
        provider.search_artists(account_id, &request.query, request.page),
    );

    CatalogProviderResponse {
        provider: provider_name.clone(),
        account_id,
        tracks: to_section(&provider_name, tracks),
        albums: to_section(&provider_name, albums),
        artists: to_section(&provider_name, artists),
    }
}

fn to_section<T>(
    provider: &str,
    result: Result<CatalogPage<T>, CatalogSearchError>,
) -> CatalogSection<T> {
    match result {
        Ok(page) => CatalogSection::ok(page),
        Err(error) => CatalogSection::failed(CatalogSearchFailure::from_error(provider, error)),
    }
}

/// Credenciales de una cuenta Qobuz. Se inyecta para que el adaptador se pueda
/// probar con un token fijo y sin base de datos.
trait QobuzAccountTokenSource: Send + Sync {
    fn token_for_account<'a>(&'a self, account_id: i64) -> CatalogSearchFuture<'a, String>;
}

struct DbQobuzAccountTokenSource {
    db: SqlitePool,
}

impl QobuzAccountTokenSource for DbQobuzAccountTokenSource {
    fn token_for_account<'a>(&'a self, account_id: i64) -> CatalogSearchFuture<'a, String> {
        Box::pin(async move {
            // Reutiliza el cargador multi-cuenta y el resolutor S186: token
            // guardado primero, login con usuario/contraseña como fallback.
            let (_, credentials) = crate::commands::service::load_service_credentials_for_account(
                &self.db, "qobuz", account_id,
            )
            .await
            .map_err(classify_token_error)?;

            crate::commands::service::resolve_qobuz_user_auth_token(
                &self.db,
                account_id,
                &credentials,
            )
            .await
            .map_err(classify_token_error)
        })
    }
}

/// Un fallo de credenciales es terminal para esa cuenta; el resto de fallos al
/// leer/descifrar la fila también se comunican como permanentes para no
/// reintentar una petición que nunca podrá autenticarse.
fn classify_token_error(message: String) -> CatalogSearchError {
    if message.contains("RequiresAuth")
        || message.contains("401")
        || message.contains("403")
        || message.contains("login rejected")
        || message.contains("Login rejected")
    {
        CatalogSearchError::auth_expired(message)
    } else {
        CatalogSearchError::permanent(message)
    }
}

/// Adaptador Qobuz de búsqueda: tres endpoints (`track`, `album`, `artist`),
/// paginación por `offset`/`limit` y total por página.
pub struct QobuzCatalogProvider {
    client: Client,
    base_url: String,
    app_id: String,
    tokens: Arc<dyn QobuzAccountTokenSource>,
    /// Último token resuelto. Las tres búsquedas de una misma consulta corren
    /// en paralelo, así que sin esta caché cada una pediría credenciales y un
    /// login podría repetirse tres veces. Un 401 la invalida.
    cached_token: tokio::sync::Mutex<Option<(i64, String)>>,
    retry_base_delay: Duration,
}

impl QobuzCatalogProvider {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            client: Self::build_client(),
            base_url: qobuz_base_from_env(),
            app_id: get_qobuz_app_id(),
            tokens: Arc::new(DbQobuzAccountTokenSource { db }),
            cached_token: tokio::sync::Mutex::new(None),
            retry_base_delay: Duration::from_millis(500),
        }
    }

    fn build_client() -> Client {
        Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new())
    }

    async fn token_for(&self, account_id: Option<i64>) -> Result<String, CatalogSearchError> {
        let account_id =
            account_id.ok_or_else(|| CatalogSearchError::no_account(self.service_name()))?;

        let mut cached = self.cached_token.lock().await;
        if let Some((cached_account, token)) = cached.as_ref() {
            if *cached_account == account_id {
                return Ok(token.clone());
            }
        }

        let token = self.tokens.token_for_account(account_id).await?;
        *cached = Some((account_id, token.clone()));
        Ok(token)
    }

    /// Tras un 401/403 el token cacheado ya no sirve: la siguiente búsqueda
    /// vuelve a resolver credenciales (y puede refrescar el login).
    async fn forget_token(&self) {
        *self.cached_token.lock().await = None;
    }

    fn retry_delay(&self, attempt: u32) -> Duration {
        self.retry_base_delay * 2u32.pow(attempt)
    }

    /// Petición GET con la política de T02: límite compartido, reintento
    /// acotado para 5xx/timeout/transporte, `Retry-After` para 429 y sin
    /// reintento para 401/403.
    async fn fetch_search_page(
        &self,
        endpoint: &str,
        token: &str,
        query: &str,
        page: CatalogPageRequest,
    ) -> Result<String, CatalogSearchError> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint);
        let limit = page.limit.to_string();
        let offset = page.offset.to_string();
        let mut attempt = 0u32;

        // `track/search`, `album/search` y `artist/search` se autentican con
        // `X-App-Id` + `X-User-Auth-Token`, igual que `QobuzClient::search_track`;
        // no llevan `request_sig`. El adaptador no reescribe el cliente: solo
        // añade la paginación y la clasificación de estados que ese método no
        // expone.
        loop {
            GLOBAL_RATE_LIMITER.acquire("qobuz").await;

            let response = self
                .client
                .get(&url)
                .header("X-App-Id", &self.app_id)
                .header("X-User-Auth-Token", token)
                .header("User-Agent", crate::download::http_client::get_user_agent())
                .query(&[
                    ("query", query),
                    ("limit", limit.as_str()),
                    ("offset", offset.as_str()),
                ])
                .send()
                .await;

            let response = match response {
                Ok(response) => response,
                Err(error) => {
                    if attempt + 1 < MAX_SEARCH_ATTEMPTS {
                        let delay = self.retry_delay(attempt);
                        attempt += 1;
                        tracing::warn!(
                            "[catalog_search] Qobuz {} transport failure (attempt {}/{}), retrying in {:?}: {}",
                            endpoint,
                            attempt + 1,
                            MAX_SEARCH_ATTEMPTS,
                            delay,
                            error
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(CatalogSearchError::retryable(format!(
                        "Qobuz {} request failed after {} attempts: {}",
                        endpoint, MAX_SEARCH_ATTEMPTS, error
                    )));
                }
            };

            let status = response.status();
            let headers = response.headers().clone();

            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after = penalize_on_rate_limit("qobuz", status, &headers)
                    .await
                    .unwrap_or(DEFAULT_429_PENALTY);
                if attempt + 1 < MAX_SEARCH_ATTEMPTS {
                    // La penalización del limitador compartido se respeta en
                    // el siguiente `acquire`; no se duerme dos veces.
                    attempt += 1;
                    tracing::warn!(
                        "[catalog_search] Qobuz {} rate limited (attempt {}/{}), retry after {:?}",
                        endpoint,
                        attempt,
                        MAX_SEARCH_ATTEMPTS,
                        retry_after
                    );
                    continue;
                }
                return Err(CatalogSearchError::rate_limited(
                    format!(
                        "Qobuz {} stayed rate limited after {} attempts",
                        endpoint, MAX_SEARCH_ATTEMPTS
                    ),
                    retry_after,
                ));
            }

            let body = response.text().await.map_err(|error| {
                CatalogSearchError::retryable(format!(
                    "Failed to read the Qobuz {} response: {}",
                    endpoint, error
                ))
            })?;

            if status.is_success() {
                return Ok(body);
            }

            if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
                self.forget_token().await;
                // El cuerpo de un 401 puede repetir el token; solo se
                // propaga el estado.
                return Err(CatalogSearchError::auth_expired(format!(
                    "Qobuz {} rejected the account credentials (HTTP {})",
                    endpoint, status
                )));
            }

            if is_transient_status(status) && attempt + 1 < MAX_SEARCH_ATTEMPTS {
                let delay = self.retry_delay(attempt);
                attempt += 1;
                tracing::warn!(
                    "[catalog_search] Qobuz {} transient status {} (attempt {}/{}), retrying in {:?}",
                    endpoint,
                    status,
                    attempt + 1,
                    MAX_SEARCH_ATTEMPTS,
                    delay
                );
                tokio::time::sleep(delay).await;
                continue;
            }

            if is_transient_status(status) {
                return Err(CatalogSearchError::retryable(format!(
                    "Qobuz {} failed with HTTP {} after {} attempts",
                    endpoint, status, MAX_SEARCH_ATTEMPTS
                )));
            }

            return Err(CatalogSearchError::permanent(format!(
                "Qobuz {} failed with HTTP {}: {}",
                endpoint,
                status,
                truncate_for_error(&body)
            )));
        }
    }

    async fn search_page(
        &self,
        endpoint: &str,
        account_id: Option<i64>,
        query: &str,
        page: CatalogPageRequest,
    ) -> Result<String, CatalogSearchError> {
        let token = self.token_for(account_id).await?;
        self.fetch_search_page(endpoint, &token, query, page).await
    }
}

impl CatalogSearchProvider for QobuzCatalogProvider {
    fn service_name(&self) -> &'static str {
        "qobuz"
    }

    fn search_tracks<'a>(
        &'a self,
        account_id: Option<i64>,
        query: &'a str,
        page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogTrack>> {
        Box::pin(async move {
            let body = self
                .search_page("track/search", account_id, query, page)
                .await?;
            parse_track_page(&body, page)
        })
    }

    fn search_albums<'a>(
        &'a self,
        account_id: Option<i64>,
        query: &'a str,
        page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogAlbum>> {
        Box::pin(async move {
            let body = self
                .search_page("album/search", account_id, query, page)
                .await?;
            parse_album_page(&body, page)
        })
    }

    fn search_artists<'a>(
        &'a self,
        account_id: Option<i64>,
        query: &'a str,
        page: CatalogPageRequest,
    ) -> CatalogSearchFuture<'a, CatalogPage<CatalogArtist>> {
        Box::pin(async move {
            let body = self
                .search_page("artist/search", account_id, query, page)
                .await?;
            parse_artist_page(&body, page)
        })
    }
}

fn qobuz_base_from_env() -> String {
    // Mismo contrato que el helper privado de `services/qobuz.rs`; este módulo
    // solo puede añadir su propio fichero, así que replica las cuatro líneas
    // en vez de tocar aquel.
    std::env::var(QOBUZ_API_BASE_ENV)
        .ok()
        .map(|base| base.trim().trim_end_matches('/').to_string())
        .filter(|base| !base.is_empty())
        .unwrap_or_else(|| QOBUZ_API_BASE.to_string())
}

#[derive(Debug, Deserialize, Default)]
struct QobuzTrackSearchContainer {
    #[serde(default)]
    items: Vec<QobuzTrack>,
    #[serde(default)]
    total: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
struct QobuzTrackSearchEnvelope {
    #[serde(default)]
    tracks: Option<QobuzTrackSearchContainer>,
}

#[derive(Debug, Deserialize, Default)]
struct QobuzAlbumSearchItem {
    #[serde(deserialize_with = "deserialize_id")]
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_artist")]
    artist: Option<QobuzArtist>,
    #[serde(default)]
    tracks_count: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id_i64")]
    released_at: Option<i64>,
    #[serde(default)]
    image: Option<QobuzImage>,
}

#[derive(Debug, Deserialize, Default)]
struct QobuzAlbumSearchContainer {
    #[serde(default)]
    items: Vec<QobuzAlbumSearchItem>,
    #[serde(default)]
    total: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
struct QobuzAlbumSearchEnvelope {
    #[serde(default)]
    albums: Option<QobuzAlbumSearchContainer>,
}

#[derive(Debug, Deserialize, Default)]
struct QobuzArtistSearchItem {
    #[serde(deserialize_with = "deserialize_id_i64")]
    id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    image: Option<QobuzImage>,
    #[serde(default)]
    albums_count: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
struct QobuzArtistSearchContainer {
    #[serde(default)]
    items: Vec<QobuzArtistSearchItem>,
    #[serde(default)]
    total: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
struct QobuzArtistSearchEnvelope {
    #[serde(default)]
    artists: Option<QobuzArtistSearchContainer>,
}

fn parse_track_page(
    body: &str,
    page: CatalogPageRequest,
) -> Result<CatalogPage<CatalogTrack>, CatalogSearchError> {
    let envelope: QobuzTrackSearchEnvelope = serde_json::from_str(body).map_err(|error| {
        CatalogSearchError::permanent(format!("Qobuz track search payload is invalid: {}", error))
    })?;
    let container = envelope.tracks.unwrap_or_default();

    let items = container
        .items
        .into_iter()
        .map(|track| {
            let artist = track
                .performer
                .as_ref()
                .and_then(|performer| performer.name.clone())
                .or_else(|| track.performers.clone())
                .unwrap_or_default();
            let album = track.album.as_ref().and_then(|album| album.title.clone());
            CatalogTrack {
                id: track.id.to_string(),
                title: track.title.unwrap_or_default(),
                artist,
                album,
                isrc: track.isrc,
                duration_ms: track.duration.saturating_mul(1000),
                bit_depth: track.maximum_bit_depth,
                sample_rate: track.maximum_sampling_rate,
            }
        })
        .collect();

    Ok(CatalogPage {
        items,
        total: container.total.map(|total| total.max(0) as u64),
        offset: page.offset,
        limit: page.limit,
    })
}

fn parse_album_page(
    body: &str,
    page: CatalogPageRequest,
) -> Result<CatalogPage<CatalogAlbum>, CatalogSearchError> {
    let envelope: QobuzAlbumSearchEnvelope = serde_json::from_str(body).map_err(|error| {
        CatalogSearchError::permanent(format!("Qobuz album search payload is invalid: {}", error))
    })?;
    let container = envelope.albums.unwrap_or_default();

    let items = container
        .items
        .into_iter()
        .map(|album| CatalogAlbum {
            id: album.id,
            title: album.title.unwrap_or_default(),
            artist: album.artist.and_then(|artist| artist.name),
            track_count: album.tracks_count,
            released_at: album.released_at,
            cover_url: album.image.and_then(|image| image.large.or(image.small)),
        })
        .collect();

    Ok(CatalogPage {
        items,
        total: container.total.map(|total| total.max(0) as u64),
        offset: page.offset,
        limit: page.limit,
    })
}

fn parse_artist_page(
    body: &str,
    page: CatalogPageRequest,
) -> Result<CatalogPage<CatalogArtist>, CatalogSearchError> {
    let envelope: QobuzArtistSearchEnvelope = serde_json::from_str(body).map_err(|error| {
        CatalogSearchError::permanent(format!("Qobuz artist search payload is invalid: {}", error))
    })?;
    let container = envelope.artists.unwrap_or_default();

    let items = container
        .items
        .into_iter()
        .map(|artist| CatalogArtist {
            id: artist.id.to_string(),
            name: artist.name.unwrap_or_default(),
            image_url: artist.image.and_then(|image| image.large.or(image.small)),
            album_count: artist.albums_count,
        })
        .collect();

    Ok(CatalogPage {
        items,
        total: container.total.map(|total| total.max(0) as u64),
        offset: page.offset,
        limit: page.limit,
    })
}

fn truncate_for_error(body: &str) -> String {
    body.chars().take(200).collect()
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const TRACKS_FIXTURE: &str = r#"
    {
      "tracks": {
        "total": 57,
        "items": [
          {
            "id": 4001,
            "title": "One More Time",
            "duration": 320,
            "isrc": "GBDUW0000059",
            "maximum_bit_depth": 24,
            "maximum_sampling_rate": 96.0,
            "performer": { "id": 501, "name": "Daft Punk" },
            "album": { "id": "q-alb-1", "title": "Discovery" }
          },
          {
            "id": 4002,
            "title": "Aerodynamic",
            "duration": 212,
            "isrc": "GBDUW0000060",
            "performers": "Daft Punk",
            "maximum_bit_depth": 16,
            "maximum_sampling_rate": 44.1
          }
        ]
      }
    }"#;

    const ALBUMS_FIXTURE: &str = r#"
    {
      "albums": {
        "total": 12,
        "items": [
          {
            "id": "q-alb-1",
            "title": "Discovery",
            "artist": { "id": 501, "name": "Daft Punk" },
            "tracks_count": 14,
            "released_at": 981158400,
            "image": {
              "small": "https://example.invalid/discovery-s.jpg",
              "large": "https://example.invalid/discovery-l.jpg"
            }
          }
        ]
      }
    }"#;

    const ARTISTS_FIXTURE: &str = r#"
    {
      "artists": {
        "total": 3,
        "items": [
          {
            "id": 501,
            "name": "Daft Punk",
            "albums_count": 8,
            "image": {
              "small": "https://example.invalid/daft-s.jpg",
              "large": "https://example.invalid/daft-l.jpg"
            }
          }
        ]
      }
    }"#;

    type Seen = Arc<Mutex<Vec<String>>>;
    type Responder = Arc<dyn Fn(&str) -> (u16, &'static str) + Send + Sync>;

    struct MockServer {
        base: String,
        seen: Seen,
    }

    impl MockServer {
        fn targets(&self) -> Vec<String> {
            self.seen.lock().expect("mock recorder").clone()
        }
    }

    async fn serve_one(mut socket: tokio::net::TcpStream, responder: Responder, seen: Seen) {
        let mut buf = vec![0u8; 16384];
        let n = socket.read(&mut buf).await.unwrap_or(0);
        let raw = String::from_utf8_lossy(&buf[..n]).to_string();
        let request_line = raw.lines().next().unwrap_or("");
        let target = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_string();
        seen.lock().expect("mock recorder").push(target.clone());

        let (status, body) = responder(&target);
        let retry_after = if status == 429 {
            "Retry-After: 0\r\n"
        } else {
            ""
        };
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
            status,
            if (200..300).contains(&status) { "OK" } else { "Error" },
            body.len(),
            retry_after,
            body
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.flush().await;
    }

    async fn spawn_mock(responder: Responder) -> MockServer {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
        let addr = listener.local_addr().expect("mock address");
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        let seen_for_accept = seen.clone();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let responder = responder.clone();
                let seen = seen_for_accept.clone();
                tokio::spawn(serve_one(socket, responder, seen));
            }
        });
        MockServer {
            base: format!("http://{}", addr),
            seen,
        }
    }

    fn fixed_token_provider(base: String) -> QobuzCatalogProvider {
        struct FixedToken(String);

        impl QobuzAccountTokenSource for FixedToken {
            fn token_for_account<'a>(&'a self, account_id: i64) -> CatalogSearchFuture<'a, String> {
                Box::pin(async move {
                    assert_eq!(account_id, 7, "fixture account");
                    Ok(self.0.clone())
                })
            }
        }

        QobuzCatalogProvider {
            client: QobuzCatalogProvider::build_client(),
            base_url: base,
            app_id: "test-app".to_string(),
            tokens: Arc::new(FixedToken("fixed-token".to_string())),
            cached_token: tokio::sync::Mutex::new(None),
            retry_base_delay: Duration::from_millis(1),
        }
    }

    fn page() -> CatalogPageRequest {
        CatalogPageRequest::new(20, 10)
    }

    #[tokio::test]
    async fn qobuz_parses_the_three_kinds_and_forwards_pagination() {
        let mock = spawn_mock(Arc::new(|target| match target {
            t if t.starts_with("/track/search") => (200, TRACKS_FIXTURE),
            t if t.starts_with("/album/search") => (200, ALBUMS_FIXTURE),
            t if t.starts_with("/artist/search") => (200, ARTISTS_FIXTURE),
            _ => (404, "{}"),
        }))
        .await;
        let provider = fixed_token_provider(mock.base.clone());

        let tracks = provider
            .search_tracks(Some(7), "daftpunk", page())
            .await
            .expect("track page");
        assert_eq!(tracks.total, Some(57));
        assert_eq!(tracks.offset, 20);
        assert_eq!(tracks.limit, 10);
        assert_eq!(tracks.items.len(), 2);
        assert_eq!(tracks.items[0].id, "4001");
        assert_eq!(tracks.items[0].artist, "Daft Punk");
        assert_eq!(tracks.items[0].album.as_deref(), Some("Discovery"));
        assert_eq!(tracks.items[0].duration_ms, 320_000);
        assert_eq!(tracks.items[0].bit_depth, Some(24));
        // `performers` es el fallback cuando no hay objeto `performer`.
        assert_eq!(tracks.items[1].artist, "Daft Punk");
        assert_eq!(tracks.items[1].album, None);

        let albums = provider
            .search_albums(Some(7), "daftpunk", page())
            .await
            .expect("album page");
        assert_eq!(albums.total, Some(12));
        assert_eq!(albums.items[0].id, "q-alb-1");
        assert_eq!(albums.items[0].track_count, Some(14));
        assert_eq!(
            albums.items[0].cover_url.as_deref(),
            Some("https://example.invalid/discovery-l.jpg")
        );

        let artists = provider
            .search_artists(Some(7), "daftpunk", page())
            .await
            .expect("artist page");
        assert_eq!(artists.total, Some(3));
        assert_eq!(artists.items[0].name, "Daft Punk");
        assert_eq!(artists.items[0].album_count, Some(8));

        let targets = mock.targets();
        assert_eq!(targets.len(), 3);
        assert!(targets.iter().any(|t| t.starts_with("/track/search")));
        assert!(targets.iter().any(|t| t.starts_with("/album/search")));
        assert!(targets.iter().any(|t| t.starts_with("/artist/search")));
        for target in targets {
            assert!(target.contains("query=daftpunk"), "target: {target}");
            assert!(target.contains("limit=10"), "target: {target}");
            assert!(target.contains("offset=20"), "target: {target}");
        }
    }

    #[tokio::test]
    async fn maps_401_to_auth_expired_without_retrying() {
        let mock = spawn_mock(Arc::new(|_| (401, r#"{"error":"unauthorized"}"#))).await;
        let provider = fixed_token_provider(mock.base.clone());

        let error = provider
            .search_tracks(Some(7), "daftpunk", page())
            .await
            .expect_err("401 must fail");
        assert_eq!(error.kind, CatalogSearchErrorKind::AuthExpired);
        assert_eq!(mock.targets().len(), 1, "401 must not be retried");
    }

    #[tokio::test]
    async fn retries_429_honouring_retry_after_then_succeeds() {
        let calls = Arc::new(AtomicUsize::new(0));
        let responder_calls = calls.clone();
        let mock = spawn_mock(Arc::new(move |_| {
            let call = responder_calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                (429, r#"{"error":"rate limited"}"#)
            } else {
                (200, TRACKS_FIXTURE)
            }
        }))
        .await;
        let provider = fixed_token_provider(mock.base.clone());

        let page_result = provider
            .search_tracks(Some(7), "daftpunk", page())
            .await
            .expect("429 then 200");
        assert_eq!(page_result.total, Some(57));
        assert_eq!(mock.targets().len(), 2);
    }

    #[tokio::test]
    async fn persistent_429_surfaces_rate_limited_after_three_attempts() {
        let mock = spawn_mock(Arc::new(|_| (429, r#"{"error":"rate limited"}"#))).await;
        let provider = fixed_token_provider(mock.base.clone());

        let error = provider
            .search_tracks(Some(7), "daftpunk", page())
            .await
            .expect_err("persistent 429 must fail");
        assert_eq!(error.kind, CatalogSearchErrorKind::RateLimited);
        assert_eq!(error.retry_after_sec, Some(0));
        assert_eq!(mock.targets().len(), MAX_SEARCH_ATTEMPTS as usize);
    }

    #[tokio::test]
    async fn retries_transient_5xx_then_succeeds() {
        let calls = Arc::new(AtomicUsize::new(0));
        let responder_calls = calls.clone();
        let mock = spawn_mock(Arc::new(move |_| {
            let call = responder_calls.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                (503, r#"{"error":"busy"}"#)
            } else {
                (200, ALBUMS_FIXTURE)
            }
        }))
        .await;
        let provider = fixed_token_provider(mock.base.clone());

        let page_result = provider
            .search_albums(Some(7), "daftpunk", page())
            .await
            .expect("503 then 200");
        assert_eq!(page_result.total, Some(12));
        assert_eq!(mock.targets().len(), 2);
    }

    #[tokio::test]
    async fn malformed_payload_is_permanent() {
        let mock = spawn_mock(Arc::new(|_| (200, "not-json"))).await;
        let provider = fixed_token_provider(mock.base.clone());

        let error = provider
            .search_artists(Some(7), "daftpunk", page())
            .await
            .expect_err("invalid JSON must fail");
        assert_eq!(error.kind, CatalogSearchErrorKind::Permanent);
        assert_eq!(mock.targets().len(), 1);
    }

    #[tokio::test]
    async fn missing_account_fails_before_opening_network() {
        let provider = fixed_token_provider("http://127.0.0.1:9".to_string());

        let error = provider
            .search_tracks(None, "daftpunk", page())
            .await
            .expect_err("no account must fail");
        assert_eq!(error.kind, CatalogSearchErrorKind::NoAccount);
    }

    #[test]
    fn page_request_clamps_the_limit() {
        assert_eq!(CatalogPageRequest::new(0, 0).limit, 1);
        assert_eq!(
            CatalogPageRequest::new(0, MAX_CATALOG_PAGE_SIZE + 50).limit,
            MAX_CATALOG_PAGE_SIZE
        );
        assert_eq!(
            CatalogPageRequest::default().limit,
            DEFAULT_CATALOG_PAGE_SIZE
        );
    }

    #[test]
    fn empty_query_is_rejected_before_network() {
        let error = CatalogSearchRequest::new("   ", page()).expect_err("empty query");
        assert_eq!(error.kind, CatalogSearchErrorKind::Permanent);
    }

    #[test]
    fn page_reports_whether_another_page_exists() {
        let complete = CatalogPage {
            items: vec![1, 2],
            total: Some(2),
            offset: 0,
            limit: 2,
        };
        assert!(!complete.has_more());

        let partial = CatalogPage {
            items: vec![1, 2],
            total: Some(5),
            offset: 2,
            limit: 2,
        };
        assert!(partial.has_more());
    }
}
