//! SoundCloud service - Authentication and library import
//!
//! Handles SoundCloud API access and importing favorites.

use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const SOUNDCLOUD_API_V2: &str = "https://api-v2.soundcloud.com";

/// Environment variable that redirects every [`SoundCloudClient`] at a local
/// mock server. Test seam only (same pattern as `SYNCIFY_S197_TIDAL_BASE_URL`):
/// unset in production, where traffic always goes to `api-v2.soundcloud.com`.
pub const SOUNDCLOUD_API_BASE_ENV: &str = "SYNCIFY_SOUNDCLOUD_API_BASE";

/// Base URL every client starts from: the mock server when the test seam is set,
/// the public API otherwise.
fn default_api_base() -> String {
    std::env::var(SOUNDCLOUD_API_BASE_ENV)
        .ok()
        .map(|base| base.trim().trim_end_matches('/').to_string())
        .filter(|base| !base.is_empty())
        .unwrap_or_else(|| SOUNDCLOUD_API_V2.to_string())
}

/// SoundCloud track from API
#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudTrack {
    pub id: i64,
    pub title: String,
    pub duration: i64, // in milliseconds
    pub user: Option<SoundCloudUser>,
    pub permalink_url: Option<String>,
    /// Bloque de entrega de disco. Solo existe cuando el track se publicó a
    /// través de un distribuidora/sello; es la única fuente de artista
    /// intérprete, álbum e ISRC en SoundCloud.
    #[serde(default)]
    pub publisher_metadata: Option<SoundCloudPublisherMetadata>,
    /// Portada del track (rendición `-t500x500` en la respuesta de la API).
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudUser {
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    pub id: i64,
    pub username: String,
}

/// `publisher_metadata` de un track de SoundCloud: los metadatos que aporta el
/// sello al entregar la masters, no la cuenta que subió el audio.
///
/// Limitación de capacidad del servicio, no una integración pendiente: SoundCloud
/// solo rellena este bloque para material publicado por distribuidora. Los tracks
/// subidos directamente (remixes, bootlegs, sets de DJ, muestras) llegan sin él y
/// sin ISRC, de modo que para esa parte de la biblioteca la identidad canónica
/// del motor se resuelve por Check A (`track_sources` con el id de SoundCloud).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SoundCloudPublisherMetadata {
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album_title: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    // Campo del contrato de datos (serde): lo puebla la deserialización, no el código Rust.
    pub title: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub label_name: Option<String>,
    #[serde(default)]
    pub year: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    // Campo del contrato de datos (serde): lo puebla la deserialización, no el código Rust.
    pub contains_music: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudCollection {
    pub collection: Vec<SoundCloudLike>,
    pub next_href: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudLike {
    pub track: Option<SoundCloudTrack>,
    /// Instante en que se marcó el like ("YYYY-MM-DDTHH:MM:SSZ"): es el
    /// `added_at` que el motor de importación persiste en `library_entries`.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Colección paginada de la API v2: `next_href` es la URL completa de la
/// página siguiente (o `null` en la última), igual que en los likes.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Cubierta por `tests/soundcloud_playlists_client_test.rs`.
pub struct SoundCloudPlaylistCollection {
    pub collection: Vec<SoundCloudPlaylistItem>,
    pub next_href: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Cubierta por `tests/soundcloud_playlists_client_test.rs`.
pub struct SoundCloudPlaylistItem {
    pub playlist: Option<SoundCloudPlaylist>,
}

/// Playlist propia de una cuenta. La API v2 las expone en
/// `/me/library/playlists_without_albums`; el mismo recorrido ya lo hacía el
/// servicio Python (`scripts/services/soundcloud_service.py:454`), así que el
/// endpoint es real y no una suposición.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Cubierta por `tests/soundcloud_playlists_client_test.rs`.
pub struct SoundCloudPlaylist {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub track_count: i64,
    /// Duración acumulada en milisegundos, igual que `SoundCloudTrack::duration`.
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub user: Option<SoundCloudUser>,
}

impl SoundCloudPlaylist {
    #[allow(dead_code)] // Cubierta por `tests/soundcloud_playlists_client_test.rs`.
    pub fn display_name(&self) -> &str {
        self.title.trim()
    }
}

impl SoundCloudTrack {
    /// Artista credited por el sello, si el track tiene `publisher_metadata`.
    pub fn publisher_artist(&self) -> Option<&str> {
        self.publisher_metadata
            .as_ref()?
            .artist
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
    }

    /// Cuenta que subió el audio. NO es el artista intérprete: un sello, un
    /// remaster o una cuenta de_curator pueden subir el track.
    pub fn uploader_name(&self) -> Option<&str> {
        self.user
            .as_ref()
            .map(|u| u.username.as_str())
            .map(str::trim)
            .filter(|u| !u.is_empty())
    }

    /// Artista al que se atribuye el track en el catálogo: el que publica el
    /// sello cuando existe y, si no, la cuenta que lo subió.
    pub fn attributed_artist(&self) -> Option<&str> {
        self.publisher_artist().or_else(|| self.uploader_name())
    }

    /// Título del álbum de origen, cuando el sello lo aporta.
    pub fn album_title(&self) -> Option<&str> {
        self.publisher_metadata
            .as_ref()?
            .album_title
            .as_deref()
            .map(str::trim)
            .filter(|a| !a.is_empty())
    }

    /// ISRC de la master entregada por el sello. `None` en los tracks subidos
    /// directamente (ver limitation de capacidad en
    /// [`SoundCloudPublisherMetadata`]).
    pub fn isrc(&self) -> Option<&str> {
        self.publisher_metadata
            .as_ref()?
            .isrc
            .as_deref()
            .map(str::trim)
            .filter(|i| !i.is_empty())
    }

    pub fn label(&self) -> Option<&str> {
        self.publisher_metadata
            .as_ref()?
            .label_name
            .as_deref()
            .map(str::trim)
            .filter(|l| !l.is_empty())
    }

    pub fn release_year(&self) -> Option<i64> {
        self.publisher_metadata.as_ref()?.year
    }

    /// Portada normalizada a la rendición más grande que sirve SoundCloud. La
    /// API devuelve `-t500x500`; `-large` es la misma imagen a resolución
    /// completa y es lo que persisten el resto de brazos del motor.
    pub fn cover_art_url(&self) -> Option<String> {
        let raw = self.artwork_url.as_deref()?.trim();
        if raw.is_empty() {
            return None;
        }
        let (stem, extension) = match raw.rsplit_once('.') {
            Some((stem, ext)) => (stem, Some(ext)),
            None => (raw, None),
        };
        for rendition in ["-t500x500", "-t67x67", "-badge", "-small"] {
            if let Some(bigger) = stem.strip_suffix(rendition) {
                return Some(match extension {
                    Some(ext) => format!("{}-large.{}", bigger, ext),
                    None => format!("{}-large", bigger),
                });
            }
        }
        Some(raw.to_string())
    }
}

/// SoundCloud API client
pub struct SoundCloudClient {
    client: Client,
    oauth_token: String,
    user_id: Option<i64>,
    api_base: String,
}

impl SoundCloudClient {
    pub fn new(oauth_token: String) -> Self {
        Self {
            client: Client::new(),
            oauth_token,
            user_id: None,
            api_base: default_api_base(),
        }
    }

    pub fn with_user_id(mut self, user_id: i64) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Redirige el cliente a otro host. La costura explícita gana sobre la
    /// variable de entorno; la producción siempre habla con la API pública de
    /// SoundCloud, igual que `DeezerClient::with_api_base`.
    #[allow(dead_code)] // Cubierta por `tests/soundcloud_unified_engine_test.rs`.
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into().trim_end_matches('/').to_string();
        self
    }

    /// Convierte el `next_href` de una página en una URL absoluta.
    ///
    /// `reqwest` rechaza una ruta relativa con `builder error`, y el fallo ocurre
    /// a mitad de la paginación: aborta el resto del sync y deja un error que no
    /// dice qué pasa. Resolver la ruta contra `api_base` cubre las dos formas sin
    /// depender de cuál devuelva cada endpoint.
    fn resolve_next_href(&self, next_href: &str) -> String {
        if next_href.starts_with("http://") || next_href.starts_with("https://") {
            return next_href.to_string();
        }
        format!("{}/{}", self.api_base, next_href.trim_start_matches('/'))
    }

    /// Petición GET autenticada a una colección paginada de la API v2.
    ///
    /// Likes, playlists y pistas de playlist comparten autenticación, limitador
    /// global y forma de error; la única diferencia es la URL y el tipo que se
    /// deserializa, así que viven aquí en vez de triplicarse.
    async fn get_collection<T: for<'de> Deserialize<'de>>(
        &self,
        request_url: &str,
    ) -> Result<T, String> {
        crate::services::rate_limiter::GLOBAL_RATE_LIMITER
            .acquire("soundcloud")
            .await;

        let response = self
            .client
            .get(request_url)
            .header("Authorization", format!("OAuth {}", self.oauth_token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            crate::services::rate_limiter::penalize_on_rate_limit(
                "soundcloud",
                status,
                response.headers(),
            )
            .await;
            let body = response.text().await.unwrap_or_default();
            return Err(format!("SoundCloud API error {}: {}", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    /// Get user's liked tracks (paginated via next_href)
    pub async fn get_likes(&self, url: Option<&str>) -> Result<SoundCloudCollection, String> {
        let user_id = self.user_id.ok_or("User ID not set")?;

        let request_url = url
            .map(|s| self.resolve_next_href(s))
            .unwrap_or_else(|| format!("{}/users/{}/likes?limit=100", self.api_base, user_id));

        self.get_collection(&request_url).await
    }

    /// Playlists propias de la cuenta (paginado vía `next_href`).
    ///
    /// El brazo del motor unificado avisaba de que SoundCloud no expone playlists
    /// de usuario; el endpoint `/me/library/playlists_without_albums` sí existe
    /// y es el que ya usaba el servicio Python para la misma cuenta.
    #[allow(dead_code)] // Cubierta por `tests/soundcloud_playlists_client_test.rs`.
    pub async fn get_playlists(
        &self,
        url: Option<&str>,
    ) -> Result<SoundCloudPlaylistCollection, String> {
        let request_url = url.map(|s| self.resolve_next_href(s)).unwrap_or_else(|| {
            format!(
                "{}/me/library/playlists_without_albums?limit=50",
                self.api_base
            )
        });

        self.get_collection(&request_url).await
    }

    /// Pistas de una playlist concreta, en el orden en que las guarda el usuario.
    #[allow(dead_code)] // Cubierta por `tests/soundcloud_playlists_client_test.rs`.
    pub async fn get_playlist_tracks(
        &self,
        playlist_id: &str,
        url: Option<&str>,
    ) -> Result<SoundCloudCollection, String> {
        let request_url = url.map(|s| self.resolve_next_href(s)).unwrap_or_else(|| {
            format!(
                "{}/playlists/{}/tracks?limit=100",
                self.api_base, playlist_id
            )
        });

        self.get_collection(&request_url).await
    }

    // Helper methods for database operations
    pub async fn get_service_id(&self, db: &SqlitePool, name: &str) -> Result<i64, String> {
        let result: (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
            .map_err(|e| format!("Service not found: {}", e))?;
        Ok(result.0)
    }

    /// Search for tracks by query string
    pub async fn search_track(
        &self,
        query: &str,
        limit: i32,
    ) -> Result<Vec<SoundCloudSearchResult>, String> {
        let url = format!("{}/search/tracks", self.api_base);

        crate::services::rate_limiter::GLOBAL_RATE_LIMITER
            .acquire("soundcloud")
            .await;

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("OAuth {}", self.oauth_token))
            .query(&[("q", query), ("limit", &limit.to_string())])
            .send()
            .await
            .map_err(|e| format!("Search request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            crate::services::rate_limiter::penalize_on_rate_limit(
                "soundcloud",
                status,
                response.headers(),
            )
            .await;
            let text = response.text().await.unwrap_or_default();
            return Err(format!(
                "SoundCloud search failed ({}): {}",
                status,
                &text[..text.len().min(200)]
            ));
        }

        #[derive(Deserialize)]
        struct SearchResponse {
            collection: Option<Vec<SoundCloudTrack>>,
        }

        let search_resp: SearchResponse = response
            .json()
            .await
            .map_err(|e| format!("Parse search response: {}", e))?;

        let results = search_resp
            .collection
            .unwrap_or_default()
            .into_iter()
            .map(|t| SoundCloudSearchResult {
                track_id: t.id.to_string(),
                title: t.title.clone(),
                // Mismo criterio de atribución que el import: el sello cuando
                // existe, la cuenta que subió el audio si no.
                artist: t.attributed_artist().unwrap_or_default().to_string(),
                duration_ms: t.duration,
                permalink_url: t.permalink_url,
            })
            .collect();

        Ok(results)
    }

    /// Match a track by metadata
    pub async fn match_by_metadata(
        &self,
        title: &str,
        artist: &str,
    ) -> Result<Option<SoundCloudSearchResult>, String> {
        let query = format!("{} {}", artist, title);
        let results = self.search_track(&query, 10).await?;

        let normalize = |s: &str| {
            s.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
        };
        let target_title = normalize(title);
        let target_artist = normalize(artist);

        let best_match = results.into_iter().find(|r| {
            let r_title = normalize(&r.title);
            let r_artist = normalize(&r.artist);
            r_title.contains(&target_title)
                || target_title.contains(&r_title)
                || (r_artist.contains(&target_artist) && !r_title.is_empty())
        });

        Ok(best_match)
    }

    /// Identity the stored OAuth token resolves to on the API (`GET /me`).
    ///
    /// Used by the auth-status probe: having a token is not the same as having a
    /// working one, so the account view is read back from SoundCloud itself.
    pub async fn get_current_user(&self) -> Result<SoundCloudUserSummary, String> {
        let url = format!("{}/me", self.api_base);

        crate::services::rate_limiter::GLOBAL_RATE_LIMITER
            .acquire("soundcloud")
            .await;

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("OAuth {}", self.oauth_token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            crate::services::rate_limiter::penalize_on_rate_limit(
                "soundcloud",
                status,
                response.headers(),
            )
            .await;
            let body = response.text().await.unwrap_or_default();
            return Err(format!("SoundCloud API error {}: {}", status, body));
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse: {}", e))
    }

    /// Like a track (add to favorites)
    pub async fn add_to_favorites(&self, track_id: &str) -> Result<(), String> {
        self.set_track_like(track_id, Method::PUT).await
    }

    /// Unlike a track (remove from favorites)
    pub async fn remove_from_favorites(&self, track_id: &str) -> Result<(), String> {
        self.set_track_like(track_id, Method::DELETE).await
    }

    /// Shared `PUT`/`DELETE /users/{id}/track_likes/{track}` path: same
    /// rate-limit profile, same transient criterion, same backoff for both
    /// directions of the like.
    async fn set_track_like(&self, track_id: &str, method: Method) -> Result<(), String> {
        let user_id = self.user_id.ok_or("User ID not set")?;

        let url = format!(
            "{}/users/{}/track_likes/{}",
            self.api_base, user_id, track_id
        );

        let max_retries = 3;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            crate::services::rate_limiter::GLOBAL_RATE_LIMITER
                .acquire("soundcloud")
                .await;

            let response = self
                .client
                .request(method.clone(), &url)
                .header("Authorization", format!("OAuth {}", self.oauth_token))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        tracing::info!(
                            "{} track {} on SoundCloud likes ({} {})",
                            if method == Method::PUT {
                                "Added"
                            } else {
                                "Removed"
                            },
                            track_id,
                            method,
                            status
                        );
                        return Ok(());
                    } else if crate::services::http_retry::is_transient_status(status) {
                        crate::services::rate_limiter::penalize_on_rate_limit(
                            "soundcloud",
                            status,
                            resp.headers(),
                        )
                        .await;
                        let text = resp.text().await.unwrap_or_default();
                        last_error =
                            format!("API error ({}): {}", status, &text[..text.len().min(100)]);
                        tracing::warn!(
                            "SoundCloud track like attempt {} failed ({}), retrying...",
                            attempt + 1,
                            status
                        );
                    } else {
                        let text = resp.text().await.unwrap_or_default();
                        return Err(format!(
                            "Track like failed ({}): {}",
                            status,
                            &text[..text.len().min(200)]
                        ));
                    }
                }
                Err(e) => {
                    last_error = format!("Request failed: {}", e);
                    tracing::warn!(
                        "SoundCloud track like attempt {} failed: {}, retrying...",
                        attempt + 1,
                        e
                    );
                }
            }

            if attempt < max_retries - 1 {
                let delay = 500 * (1 << attempt);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }
        }

        Err(format!(
            "Track like failed after {} retries: {}",
            max_retries, last_error
        ))
    }
}

/// The account a stored SoundCloud OAuth token resolves to.
#[derive(Debug, Clone, Deserialize)]
pub struct SoundCloudUserSummary {
    #[allow(dead_code)]
    // Campo del contrato de datos (serde): lo puebla la deserialización, no el código Rust.
    pub id: i64,
    #[serde(default)]
    pub username: Option<String>,
}

impl SoundCloudUserSummary {
    /// Best display name available for the account, empty when the API omits it.
    pub fn display_name(&self) -> &str {
        self.username.as_deref().unwrap_or("").trim()
    }
}

/// Search result for migration matching
#[derive(Debug, Clone, Serialize)]
pub struct SoundCloudSearchResult {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
    pub permalink_url: Option<String>,
}
