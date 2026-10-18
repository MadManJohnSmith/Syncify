//! Fase 2 — Resolución de fuentes de streaming (Qobuz/Tidal) para el motor
//! nativo de reproducción.
//!
//! Este módulo NO descarga a la biblioteca: entrega una URL directa que
//! `commands::player` reproduce de forma progresiva contra una caché temporal
//! (`services::stream_cache`). La URL nunca viaja al webview.
//!
//! Reglas de calidad (plan Fase 2):
//! - Se prioriza la mejor fuente disponible (`track_sources.quality_score`).
//! - Se pide la máxima calidad y se acepta degradación con etiqueta honesta
//!   (`quality_label` refleja lo OBTENIDO, no lo pedido).
//! - Fuentes que no sean un fichero progresivo (manifiestos DASH/HLS) se
//!   rechazan: el motor local decodifica ficheros completos, no segmentos.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashSet;
use syncify_core_domain::byte_validators::AudioByteValidator;

use crate::download::qobuz::{QobuzAuthStatus, QobuzDownloader};
use crate::services::tidal_pipeline::resolve_and_refresh_gui_credentials_for_account;

/// Tamaño máximo leído al sondear la cabecera real del stream (64 KiB, igual que
/// `validate_staged_qobuz_payload`). Suficiente para `fLaC` + `STREAMINFO`,
/// cabeceras ID3v2 moderadas y átomos `ftyp`/`moov` iniciales.
const STREAM_PROBE_BYTES: usize = 64 * 1024;

/// Longitud exacta (en bytes) del preview MP3 de 30 s que sirve el CDN de Qobuz
/// cuando la cuenta no tiene suscripción de streaming activa.
const QOBUZ_PREVIEW_MP3_LEN: u64 = 1_200_588;

/// Descripción pública (webview) de la fuente resuelta. Sin URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSourceInfo {
    /// `qobuz` | `tidal`
    pub service_name: String,
    pub service_track_id: String,
    /// Etiqueta de contenedor: `FLAC`, `MP3`…
    pub format: String,
    pub codec: Option<String>,
    pub container: Option<String>,
    /// Extensión de fichero con la que se cachea (`flac`, `mp3`…).
    pub extension: String,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<i32>,
    /// Etiqueta honesta de la calidad obtenida: `FLAC 24-bit / 192 kHz`.
    pub quality_label: String,
    /// `hires` | `lossless` | `lossy`.
    pub quality_tier: String,
    /// Origen real (API oficial / proxy).
    pub source_name: String,
}

/// Resolución interna: la URL sólo vive en backend (sesión de streaming).
#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub url: String,
    pub info: StreamSourceInfo,
}

#[derive(Debug, Clone)]
struct Candidate {
    service_name: String,
    service_track_id: String,
}

const STREAMABLE_EXTENSIONS: &[&str] = &["flac", "mp3", "m4a", "mp4", "aac", "ogg", "opus", "wav"];

/// Resuelve la mejor fuente de streaming reproducible para `track_id`.
///
/// Orden de trabajo:
/// 1. Fuentes Qobuz/Tidal directas del track (`track_sources`, por `quality_score`).
/// 2. Si no hay lossless, **cascada ISRC** (Fase 3): busca la misma grabación en
///    Qobuz/Tidal por ISRC —preguntando a Spotify el ISRC cuando la pista no lo
///    tiene— para que una pista importada sólo de Spotify suene igual de bien.
/// 3. Reserva lossy de duración completa, con etiqueta honesta.
///
/// Cada stream se verifica por cabecera física (previews de 30 s rechazados).
pub async fn resolve_stream_for_track(
    db: &SqlitePool,
    http: &reqwest::Client,
    track_id: i64,
) -> Result<ResolvedStream, String> {
    let candidates = load_candidates(db, track_id).await?;

    let duration_ms = load_track_duration_ms(db, track_id).await;
    let mut failures: Vec<String> = Vec::with_capacity(candidates.len() + 1);
    let mut lossy_fallback: Option<ResolvedStream> = None;
    let mut tried: HashSet<(String, String)> = HashSet::with_capacity(candidates.len());

    for candidate in candidates {
        tried.insert((
            candidate.service_name.clone(),
            candidate.service_track_id.clone(),
        ));
        let outcome = match candidate.service_name.as_str() {
            "qobuz" => resolve_qobuz(db, http, &candidate, duration_ms).await,
            "tidal" => resolve_tidal(db, http, &candidate, duration_ms).await,
            other => Err(format!("Servicio de streaming no soportado: {}", other)),
        };
        match outcome {
            Ok(resolved) => {
                if resolved.info.quality_tier != "lossy" {
                    tracing::info!(
                        track_id,
                        service = %resolved.info.service_name,
                        quality = %resolved.info.quality_label,
                        "streaming: fuente lossless resuelta y verificada"
                    );
                    return Ok(resolved);
                }
                tracing::info!(
                    track_id,
                    service = %resolved.info.service_name,
                    quality = %resolved.info.quality_label,
                    "streaming: candidata lossy de duración completa en reserva mientras se buscan opciones lossless"
                );
                if lossy_fallback.is_none() {
                    lossy_fallback = Some(resolved);
                }
            }
            Err(err) => {
                tracing::warn!(
                    track_id,
                    service = %candidate.service_name,
                    error = %err,
                    "streaming: candidata rechazada"
                );
                failures.push(format!("{}: {}", candidate.service_name, err));
            }
        }
    }

    // Fase 3: sin fuente directa lossless, la identidad ISRC manda. Se pide una
    // fuente lossless; sólo se acepta un resultado lossy de la cascada cuando no
    // había ninguna reserva mejor.
    let allow_lossy = lossy_fallback.is_none();
    match isrc_cascade(db, http, track_id, &tried, allow_lossy).await {
        Ok(Some(resolved)) => {
            tracing::info!(
                track_id,
                service = %resolved.info.service_name,
                quality = %resolved.info.quality_label,
                "streaming: fuente encontrada por cascada ISRC"
            );
            if resolved.info.quality_tier != "lossy" || lossy_fallback.is_none() {
                return Ok(resolved);
            }
        }
        Ok(None) => {}
        Err(reason) => {
            tracing::info!(
                track_id,
                reason = %reason,
                "streaming: la cascada ISRC no encontró una fuente mejor"
            );
            failures.push(format!("cascada ISRC: {}", reason));
        }
    }

    if let Some(fallback) = lossy_fallback {
        tracing::info!(
            track_id,
            service = %fallback.info.service_name,
            quality = %fallback.info.quality_label,
            "streaming: usando reserva lossy verificada al no haber alternativa lossless"
        );
        return Ok(fallback);
    }

    if failures.is_empty() {
        return Err(
            "Esta pista no tiene fuentes de streaming disponibles (Qobuz/Tidal).".to_string(),
        );
    }

    Err(format!(
        "No se pudo resolver ninguna fuente reproducible: {}",
        failures.join(" · ")
    ))
}

async fn load_track_duration_ms(db: &SqlitePool, track_id: i64) -> Option<i64> {
    sqlx::query_scalar::<_, Option<i64>>("SELECT duration_ms FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .flatten()
}

// ---------------------------------------------------------------------------
// Fase 3 — cascada ISRC
// ---------------------------------------------------------------------------

/// Metadatos mínimos para reconocer la misma grabación en otro proveedor.
#[derive(Debug, Clone)]
struct TrackLookupContext {
    title: String,
    artist: String,
    duration_ms: Option<i64>,
    isrc: Option<String>,
}

impl TrackLookupContext {
    fn expected_duration_sec(&self) -> i32 {
        expected_duration_sec(self.duration_ms)
    }
}

/// Duración esperada en segundos para el filtro de los buscadores (±10 s).
fn expected_duration_sec(duration_ms: Option<i64>) -> i32 {
    duration_ms.map(|ms| (ms.max(0) / 1000) as i32).unwrap_or(0)
}

/// Normaliza y valida un ISRC. Devuelve `None` si no es un ISRC de 12 caracteres
/// o si la pista lo trae vacío: un ISRC inventado haría que la cascada buscara
/// otra grabación, que es peor que no buscar.
fn normalize_isrc(raw: Option<&str>) -> Option<String> {
    let trimmed = raw?.trim().to_uppercase();
    syncify_core_domain::metadata::is_valid_isrc(&trimmed).then_some(trimmed)
}

async fn load_lookup_context(db: &SqlitePool, track_id: i64) -> Option<TrackLookupContext> {
    let row: Option<(String, Option<String>, Option<i64>, Option<String>)> =
        sqlx::query_as::<_, (String, Option<String>, Option<i64>, Option<String>)>(
            r#"
        SELECT
            t.title,
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) AS artist_name,
            t.duration_ms,
            t.isrc
        FROM tracks t
        WHERE t.id = ?
        LIMIT 1
        "#,
        )
        .bind(track_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten();

    let (title, artist, duration_ms, isrc) = row?;
    Some(TrackLookupContext {
        title,
        artist: artist.unwrap_or_default(),
        duration_ms,
        isrc: normalize_isrc(isrc.as_deref()),
    })
}

/// Pide a Spotify el ISRC de la pista cuando la fila local no lo tiene.
///
/// Es best-effort y sin efectos sobre la biblioteca: no escribe el ISRC en
/// `tracks` (una reproducción no debe mutar metadatos). El token se reutiliza
/// del almacén de cuentas y se refresca si caducó con el mismo helper que usa
/// la importación. Cada fallo se devuelve con su motivo para que el mensaje de
/// la cascada diga qué falta (cuenta, token, red o catálogo), no un genérico.
async fn spotify_isrc_for_track(
    db: &SqlitePool,
    http: &reqwest::Client,
    track_id: i64,
) -> Result<String, String> {
    let spotify_track_id: Option<String> = sqlx::query_scalar(
        r#"
        SELECT ts.service_track_id
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ? AND LOWER(s.name) = 'spotify'
        LIMIT 1
        "#,
    )
    .bind(track_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let spotify_track_id = spotify_track_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "la pista no tiene fuente de Spotify de la que leerlo".to_string())?;

    let account_id: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT a.id
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE LOWER(s.name) = 'spotify'
          AND COALESCE(a.credentials_invalid, 0) = 0
        ORDER BY a.is_active DESC, a.id DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let account_id = account_id.ok_or_else(|| "cuenta de Spotify desconectada".to_string())?;

    let ciphertext: Option<String> =
        sqlx::query_scalar("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();
    let ciphertext = ciphertext
        .filter(|c| !c.trim().is_empty())
        .ok_or_else(|| "cuenta de Spotify sin credenciales guardadas".to_string())?;
    let plaintext = crate::crypto::decrypt(&ciphertext).map_err(|e| {
        format!(
            "no se pudieron descifrar las credenciales de Spotify ({})",
            e
        )
    })?;
    let credentials: serde_json::Value = serde_json::from_str(&plaintext)
        .map_err(|e| format!("credenciales de Spotify malformadas ({})", e))?;

    let token = resolve_spotify_bearer_token(db, http, account_id, &credentials)
        .await
        .map_err(|e| format!("token de Spotify no utilizable ({})", e))?;

    let response = http
        .get(format!(
            "https://api.spotify.com/v1/tracks/{}",
            urlencoding::encode(&spotify_track_id)
        ))
        .bearer_auth(&token)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| format!("error de red al consultar Spotify ({})", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Spotify respondió HTTP {} al pedir la pista",
            response.status()
        ));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("respuesta de Spotify ilegible ({})", e))?;
    let isrc = normalize_isrc(
        payload
            .pointer("/external_ids/isrc")
            .and_then(|v| v.as_str()),
    )
    .ok_or_else(|| "Spotify no publica un ISRC válido para esta pista".to_string())?;

    tracing::info!(
        track_id,
        "streaming: ISRC obtenido de Spotify para la cascada"
    );
    Ok(isrc)
}

/// Obtiene un token Bearer válido de Spotify reutilizando el almacenado o
/// refrescándolo con `refresh_token`. Si la base de datos está en modo
/// solo-lectura o bloqueada al persistir el refresco, usa igualmente el token
/// recién emitido en memoria para no abortar la reproducción.
async fn resolve_spotify_bearer_token(
    db: &SqlitePool,
    http: &reqwest::Client,
    account_id: i64,
    creds: &serde_json::Value,
) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_at = creds["expires_at"].as_i64().unwrap_or(0);
    if now + 300 < expires_at {
        if let Some(tok) = creds["access_token"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
        {
            return Ok(tok.to_string());
        }
    }

    let Some(refresh_token) = creds["refresh_token"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
    else {
        return creds["access_token"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| "sin refresh_token ni access_token".to_string());
    };

    let config = crate::commands::resolve_spotify_config(db)
        .await
        .map_err(|e| format!("configuración de Spotify: {}", e))?;
    let params = [
        ("client_id", config.client_id.as_str()),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];

    let resp = http
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("refresco Spotify: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("refresco Spotify HTTP {}", resp.status()));
    }

    let token_data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("respuesta de refresco ilegible: {}", e))?;
    let new_access_token = token_data["access_token"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "respuesta de refresco sin access_token".to_string())?
        .to_string();
    let new_refresh_token = token_data["refresh_token"]
        .as_str()
        .unwrap_or(refresh_token)
        .to_string();
    let expires_in = token_data["expires_in"].as_i64().unwrap_or(3600);
    let updated_creds = serde_json::json!({
        "token_type": "Bearer",
        "access_token": new_access_token,
        "refresh_token": new_refresh_token,
        "expires_at": now + expires_in,
    });

    if let Ok(encrypted) = crate::crypto::encrypt(&updated_creds.to_string()) {
        if let Err(e) = sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
            .bind(&encrypted)
            .bind(account_id)
            .execute(db)
            .await
        {
            tracing::debug!(
                account_id,
                error = %e,
                "streaming: no se pudo persistir el token de Spotify refrescado (usando en memoria)"
            );
        }
    }

    Ok(new_access_token)
}

/// `true` cuando este (`service`, `service_track_id`) no se ha intentado ya en
/// la pasada directa. Evita repetir la misma resolución (y el mismo fallo).
fn should_try(tried: &HashSet<(String, String)>, service: &str, id: &str) -> bool {
    !tried.contains(&(service.to_string(), id.to_string()))
}

/// Busca la grabación por ISRC en Qobuz con una cuenta que pueda consultar.
///
/// El endpoint `track/search` exige token de usuario (verificado en vivo:
/// HTTP 401 sin él), así que la cascada recorre las cuentas igual que la
/// resolución: busca con la primera que autentique y continúa si falla.
async fn qobuz_isrc_match(
    db: &SqlitePool,
    isrc: &str,
    context: &TrackLookupContext,
) -> Result<crate::download::qobuz::QobuzTrack, String> {
    let account_ids = candidate_account_ids(db, "qobuz").await;
    if account_ids.is_empty() {
        return Err("cuenta Qobuz desconectada".to_string());
    }

    let expected_sec = context.expected_duration_sec();
    let has_metadata = !context.title.trim().is_empty() && !context.artist.trim().is_empty();
    let downloader = QobuzDownloader::new();
    let mut reasons: Vec<String> = Vec::with_capacity(account_ids.len());

    for account_id in account_ids {
        let token = match downloader
            .resolve_token_for_account(db, Some(account_id))
            .await
        {
            Ok(token) => token,
            Err(QobuzAuthStatus::RequiresAuth(msg)) => {
                reasons.push(format!("cuenta {}: {}", account_id, msg));
                continue;
            }
        };

        match downloader
            .search_by_isrc(isrc, expected_sec, Some(&token))
            .await
        {
            Ok(track) => return Ok(track),
            Err(first_err) => {
                if has_metadata {
                    match downloader
                        .search_by_isrc_broad(
                            isrc,
                            &context.title,
                            &context.artist,
                            expected_sec,
                            Some(&token),
                        )
                        .await
                    {
                        Ok(track) => return Ok(track),
                        Err(second_err) => reasons.push(format!(
                            "cuenta {}: {} / {}",
                            account_id, first_err, second_err
                        )),
                    }
                } else {
                    reasons.push(format!("cuenta {}: {}", account_id, first_err));
                }
            }
        }
    }

    Err(reasons.join(" · "))
}

/// Lo mismo para Tidal. La búsqueda usa el token de usuario de la cuenta (la
/// ruta anónima por client-credentials no basta: en vivo devolvió
/// "Tidal authentication required for search").
async fn tidal_isrc_match(
    db: &SqlitePool,
    http: &reqwest::Client,
    isrc: &str,
    context: &TrackLookupContext,
) -> Result<syncify_core_domain::metadata::TidalTrack, String> {
    let account_ids = candidate_account_ids(db, "tidal").await;
    if account_ids.is_empty() {
        return Err("cuenta Tidal desconectada".to_string());
    }

    let expected_sec = context.expected_duration_sec();
    let has_metadata = !context.title.trim().is_empty() && !context.artist.trim().is_empty();
    let mut reasons: Vec<String> = Vec::with_capacity(account_ids.len());

    for account_id in account_ids {
        let (creds, _name) =
            resolve_and_refresh_gui_credentials_for_account(db, http, Some(account_id), false)
                .await;
        let Some(creds) = creds else {
            reasons.push(format!("cuenta {}: sin credenciales válidas", account_id));
            continue;
        };

        let downloader = syncify_tidal_downloader::TidalDownloader::new()
            .with_user_token(Some(creds.access_token.clone()));

        match downloader.search_by_isrc(isrc, expected_sec).await {
            Ok(track) => return Ok(track),
            Err(first_err) => {
                if has_metadata {
                    match downloader
                        .search_by_isrc_broad(isrc, &context.title, &context.artist, expected_sec)
                        .await
                    {
                        Ok(track) => return Ok(track),
                        Err(second_err) => reasons.push(format!(
                            "cuenta {}: {} / {}",
                            account_id, first_err, second_err
                        )),
                    }
                } else {
                    reasons.push(format!("cuenta {}: {}", account_id, first_err));
                }
            }
        }
    }

    Err(reasons.join(" · "))
}

/// Busca la grabación por ISRC en Qobuz/Tidal y resuelve su stream.
///
/// Devuelve `Ok(Some)` con el mejor stream verificado (lossless siempre que
/// exista; lossy sólo si `allow_lossy`), `Ok(None)` si no hay nada reproducible o
/// `Err` con el motivo honesto cuando la cascada no pudo ni intentarse.
async fn isrc_cascade(
    db: &SqlitePool,
    http: &reqwest::Client,
    track_id: i64,
    tried: &HashSet<(String, String)>,
    allow_lossy: bool,
) -> Result<Option<ResolvedStream>, String> {
    let mut context = load_lookup_context(db, track_id)
        .await
        .ok_or_else(|| "la pista no tiene metadatos para buscarla".to_string())?;

    if context.isrc.is_none() {
        match spotify_isrc_for_track(db, http, track_id).await {
            Ok(isrc) => context.isrc = Some(isrc),
            Err(reason) => {
                return Err(format!("la pista no tiene ISRC y {}", reason));
            }
        }
    }

    let Some(isrc) = context.isrc.clone() else {
        return Err(
            "la pista no tiene ISRC y Spotify no pudo aportarlo (conecta Spotify o importa la pista con ISRC)"
                .to_string(),
        );
    };

    let duration_ms = context.duration_ms;
    let mut reasons: Vec<String> = Vec::with_capacity(4);
    let mut lossy_fallback: Option<ResolvedStream> = None;

    // ---- Qobuz -------------------------------------------------------------
    match qobuz_isrc_match(db, &isrc, &context).await {
        Ok(found) => {
            let service_id = found.id.to_string();
            if should_try(tried, "qobuz", &service_id) {
                let candidate = Candidate {
                    service_name: "qobuz".to_string(),
                    service_track_id: service_id,
                };
                match resolve_qobuz(db, http, &candidate, duration_ms).await {
                    Ok(resolved) if resolved.info.quality_tier != "lossy" => {
                        return Ok(Some(resolved));
                    }
                    Ok(resolved) => {
                        tracing::info!(
                            track_id,
                            quality = %resolved.info.quality_label,
                            "streaming: cascada ISRC encontró Qobuz lossy"
                        );
                        if allow_lossy && lossy_fallback.is_none() {
                            lossy_fallback = Some(resolved);
                        }
                    }
                    Err(err) => reasons.push(format!("Qobuz ISRC {}: {}", found.id, err)),
                }
            } else {
                reasons.push("Qobuz: esa identidad ya se intentó en la pasada directa".to_string());
            }
        }
        Err(reason) => reasons.push(format!("Qobuz: {}", reason)),
    }

    // ---- Tidal -------------------------------------------------------------
    match tidal_isrc_match(db, http, &isrc, &context).await {
        Ok(found) => {
            let service_id = found.id.to_string();
            if should_try(tried, "tidal", &service_id) {
                let candidate = Candidate {
                    service_name: "tidal".to_string(),
                    service_track_id: service_id,
                };
                match resolve_tidal(db, http, &candidate, duration_ms).await {
                    Ok(resolved) if resolved.info.quality_tier != "lossy" => {
                        return Ok(Some(resolved));
                    }
                    Ok(resolved) => {
                        tracing::info!(
                            track_id,
                            quality = %resolved.info.quality_label,
                            "streaming: cascada ISRC encontró Tidal lossy"
                        );
                        if allow_lossy && lossy_fallback.is_none() {
                            lossy_fallback = Some(resolved);
                        }
                    }
                    Err(err) => reasons.push(format!("Tidal ISRC {}: {}", found.id, err)),
                }
            } else {
                reasons.push("Tidal: esa identidad ya se intentó en la pasada directa".to_string());
            }
        }
        Err(reason) => reasons.push(format!("Tidal: {}", reason)),
    }

    if let Some(fallback) = lossy_fallback {
        return Ok(Some(fallback));
    }

    Err(if reasons.is_empty() {
        "las fuentes ya se habían intentado en la pasada directa".to_string()
    } else {
        reasons.join(" · ")
    })
}

async fn load_candidates(db: &SqlitePool, track_id: i64) -> Result<Vec<Candidate>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT LOWER(s.name) AS service_name, ts.service_track_id
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ?
          AND LOWER(s.name) IN ('qobuz', 'tidal')
          AND ts.available = 1
          AND ts.availability_status IN ('available', 'unknown_unchecked')
        ORDER BY ts.quality_score DESC, ts.id ASC
        "#,
    )
    .bind(track_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("No se pudieron leer las fuentes de la pista: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|(service_name, service_track_id)| Candidate {
            service_name,
            service_track_id,
        })
        .collect())
}

/// Cuentas candidatas para un servicio, priorizando cuentas con derecho de
/// streaming confirmado (`streaming_entitled = 1`), luego no determinado (`-1`),
/// activas primero y más recientes primero. Excluye credenciales inválidas.
async fn candidate_account_ids(db: &SqlitePool, service_name: &str) -> Vec<i64> {
    sqlx::query_scalar::<_, i64>(
        r#"
        SELECT a.id
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE LOWER(s.name) = ?
          AND COALESCE(a.credentials_invalid, 0) = 0
          AND COALESCE(a.streaming_entitled, -1) != 0
        ORDER BY
          CASE
            WHEN a.streaming_entitled = 1 THEN 2
            WHEN a.streaming_entitled = 0 THEN 0
            ELSE 1
          END DESC,
          a.is_active DESC,
          a.id DESC
        "#,
    )
    .bind(service_name)
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

async fn resolve_qobuz(
    db: &SqlitePool,
    http: &reqwest::Client,
    candidate: &Candidate,
    duration_ms: Option<i64>,
) -> Result<ResolvedStream, String> {
    let track_id: i64 = candidate
        .service_track_id
        .parse()
        .map_err(|_| "id de Qobuz inválido".to_string())?;
    let account_ids = candidate_account_ids(db, "qobuz").await;
    if account_ids.is_empty() {
        return Err("cuenta Qobuz desconectada o sin suscripción de streaming".to_string());
    }

    let downloader = QobuzDownloader::new();
    let mut last_err = String::new();
    let mut lossy_fallback: Option<ResolvedStream> = None;

    for account_id in account_ids {
        let token = match downloader
            .resolve_token_for_account(db, Some(account_id))
            .await
        {
            Ok(t) => t,
            Err(QobuzAuthStatus::RequiresAuth(msg)) => {
                last_err = msg;
                continue;
            }
        };

        // `hires` + fallback permitido: cascada 27 → 7 → 6 → 5 con etiqueta honesta.
        let resolution = match downloader
            .get_download_url(track_id, "hires", Some(&token), true)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                last_err = if msg.starts_with("QobuzPreviewPayload:") {
                    "solo entrega preview de 30 s (cuenta sin suscripción de streaming activa)"
                        .to_string()
                } else {
                    msg
                };
                continue;
            }
        };

        let (format, bit_depth, sample_rate) = qobuz_format_info(&resolution.format_id);
        let extension = if format == "MP3" { "mp3" } else { "flac" };
        if let Err(e) = ensure_streamable(&resolution.url, extension) {
            last_err = e;
            continue;
        }
        let source_name = match &resolution.source {
            crate::download::qobuz::StreamUrlSource::QobuzOfficial => {
                "Qobuz API oficial".to_string()
            }
            crate::download::qobuz::StreamUrlSource::ProxyFallback(_) => {
                "Qobuz (proxy de respaldo)".to_string()
            }
        };

        let expected = StreamSourceInfo {
            service_name: "qobuz".to_string(),
            service_track_id: candidate.service_track_id.clone(),
            format: format.to_string(),
            codec: Some(if format == "MP3" {
                "mp3".to_string()
            } else {
                "flac".to_string()
            }),
            container: Some(format.to_string()),
            extension: extension.to_string(),
            bit_depth,
            sample_rate,
            quality_label: quality_label(format, bit_depth, sample_rate, format == "MP3"),
            quality_tier: quality_tier(format, bit_depth).to_string(),
            source_name,
        };

        match probe_stream_candidate(http, &resolution.url, &expected, duration_ms).await {
            Ok(verified_info) => {
                let resolved = ResolvedStream {
                    url: resolution.url,
                    info: verified_info,
                };
                if resolved.info.quality_tier != "lossy" {
                    return Ok(resolved);
                }
                if lossy_fallback.is_none() {
                    lossy_fallback = Some(resolved);
                }
            }
            Err(reason) => {
                last_err = reason;
            }
        }
    }

    if let Some(fallback) = lossy_fallback {
        return Ok(fallback);
    }

    Err(if last_err.is_empty() {
        "no se pudo resolver stream en Qobuz".to_string()
    } else {
        last_err
    })
}

async fn resolve_tidal(
    db: &SqlitePool,
    http: &reqwest::Client,
    candidate: &Candidate,
    duration_ms: Option<i64>,
) -> Result<ResolvedStream, String> {
    let track_id: i64 = candidate
        .service_track_id
        .parse()
        .map_err(|_| "id de Tidal inválido".to_string())?;
    let account_ids = candidate_account_ids(db, "tidal").await;
    if account_ids.is_empty() {
        return Err("cuenta Tidal desconectada o sin suscripción de streaming".to_string());
    }

    let downloader = syncify_tidal_downloader::TidalDownloader::new();
    let mut last_err = String::new();
    let mut lossy_fallback: Option<ResolvedStream> = None;

    for account_id in account_ids {
        let (creds, _name) =
            resolve_and_refresh_gui_credentials_for_account(db, http, Some(account_id), false)
                .await;
        let Some(creds) = creds else {
            last_err = "cuenta Tidal sin credenciales válidas".to_string();
            continue;
        };

        // `hires` + fallback permitido: si Tidal no entrega hi-res se acepta
        // lossless (o lossy) con la etiqueta reflejando lo obtenido.
        let resolution = match downloader
            .get_stream_resolution_with_credentials(track_id, Some("hires"), Some(&creds), true)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };

        let extension = normalize_extension(&resolution.extension, &resolution.container);
        if let Err(e) = ensure_streamable(&resolution.url, &extension) {
            last_err = e;
            continue;
        }

        let format = resolution_format(&resolution.codec, &extension);
        let bit_depth = (resolution.bit_depth > 0).then_some(resolution.bit_depth);
        let sample_rate = (resolution.sample_rate > 0.0)
            .then_some(resolution.sample_rate.round() as i32)
            .filter(|_| bit_depth.is_some());

        let expected = StreamSourceInfo {
            service_name: "tidal".to_string(),
            service_track_id: candidate.service_track_id.clone(),
            format: format.to_string(),
            codec: Some(resolution.codec.to_lowercase()),
            container: Some(resolution.container.clone()),
            extension: extension.to_string(),
            bit_depth,
            sample_rate,
            quality_label: quality_label(format, bit_depth, sample_rate, bit_depth.is_none()),
            quality_tier: quality_tier(format, bit_depth).to_string(),
            source_name: resolution.source_name.clone(),
        };

        match probe_stream_candidate(http, &resolution.url, &expected, duration_ms).await {
            Ok(verified_info) => {
                let resolved = ResolvedStream {
                    url: resolution.url,
                    info: verified_info,
                };
                if resolved.info.quality_tier != "lossy" {
                    return Ok(resolved);
                }
                if lossy_fallback.is_none() {
                    lossy_fallback = Some(resolved);
                }
            }
            Err(reason) => {
                last_err = reason;
            }
        }
    }

    if let Some(fallback) = lossy_fallback {
        return Ok(fallback);
    }

    Err(if last_err.is_empty() {
        "no se pudo resolver stream en Tidal".to_string()
    } else {
        last_err
    })
}

/// Descarga de forma acotada los primeros `STREAM_PROBE_BYTES` del stream y
/// verifica contenedor físico, duración y ausencia de previews de 30 s.
async fn probe_stream_candidate(
    http: &reqwest::Client,
    url: &str,
    expected: &StreamSourceInfo,
    duration_ms: Option<i64>,
) -> Result<StreamSourceInfo, String> {
    let range_header = format!("bytes=0-{}", STREAM_PROBE_BYTES - 1);
    let mut resp = http
        .get(url)
        .header(reqwest::header::RANGE, range_header)
        .send()
        .await
        .map_err(|e| format!("no se pudo conectar al servidor de audio ({})", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("el servidor de audio respondió HTTP {}", status));
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let total_bytes = extract_total_bytes(status, resp.headers(), resp.content_length());

    let mut head = Vec::with_capacity(STREAM_PROBE_BYTES.min(8192));
    while head.len() < STREAM_PROBE_BYTES {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                let remaining = STREAM_PROBE_BYTES - head.len();
                let take = chunk.len().min(remaining);
                head.extend_from_slice(&chunk[..take]);
                if head.len() >= STREAM_PROBE_BYTES {
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => {
                return Err(format!(
                    "lectura interrumpida al verificar cabecera ({})",
                    e
                ));
            }
        }
    }

    verify_stream_header(
        expected,
        &head,
        content_type.as_deref(),
        total_bytes,
        duration_ms,
    )
}

/// Extrae el tamaño total del fichero remoto tanto si el CDN responde `206 Partial Content`
/// (cabecera `Content-Range: bytes 0-N/TOTAL`) como si ignora `Range` y responde `200 OK`
/// (`Content-Length: TOTAL`).
fn extract_total_bytes(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    content_length: Option<u64>,
) -> Option<u64> {
    if let Some(range_val) = headers
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
    {
        if let Some((_span, total_str)) = range_val.rsplit_once('/') {
            let trimmed = total_str.trim();
            if trimmed != "*" {
                if let Ok(total) = trimmed.parse::<u64>() {
                    return Some(total);
                }
            }
        }
    }
    if status == reqwest::StatusCode::OK {
        return content_length;
    }
    None
}

/// Verifica los bytes iniciales del stream contra el formato esperado,
/// rechaza previews de ~30 s y ajusta `StreamSourceInfo` a la calidad física
/// realmente entregada por el servidor.
fn verify_stream_header(
    expected: &StreamSourceInfo,
    head: &[u8],
    content_type: Option<&str>,
    total_bytes: Option<u64>,
    duration_ms: Option<i64>,
) -> Result<StreamSourceInfo, String> {
    if head.len() < 4 {
        return Err("el servidor devolvió una respuesta de audio vacía o truncada".to_string());
    }

    let ctype_lower = content_type.unwrap_or("").to_ascii_lowercase();
    if ctype_lower.contains("mpegurl")
        || ctype_lower.contains("dash+xml")
        || head.starts_with(b"#EXTM3U")
        || head.starts_with(b"<?xml")
        || head.starts_with(b"<MPD")
    {
        return Err(
            "la fuente devolvió un manifiesto DASH/HLS en lugar de un fichero progresivo"
                .to_string(),
        );
    }

    if AudioByteValidator::is_flac_magic(head) {
        let mut bit_depth = expected.bit_depth;
        let mut sample_rate = expected.sample_rate;
        if let Some(info) = AudioByteValidator::parse_flac_streaminfo(head) {
            if info.sample_rate > 0 && info.total_samples > 0 && duration_ms.unwrap_or(0) >= 60_000
            {
                let dur_ms = duration_ms.unwrap_or(0) as u128;
                let samples_ms = (info.total_samples as u128) * 1_000;
                let rate = info.sample_rate as u128;
                if samples_ms < (dur_ms / 2) * rate && samples_ms <= 45_000 * rate {
                    return Err(
                        "solo entrega preview FLAC de ~30 s (cuenta sin suscripción activa)"
                            .to_string(),
                    );
                }
            }
            if info.bits_per_sample > 0 {
                bit_depth = Some(i32::from(info.bits_per_sample));
            }
            if info.sample_rate > 0 {
                sample_rate = Some(info.sample_rate as i32);
            }
        }

        let mut adjusted = expected.clone();
        adjusted.format = "FLAC".to_string();
        adjusted.codec = Some("flac".to_string());
        adjusted.container = Some("FLAC".to_string());
        adjusted.extension = "flac".to_string();
        adjusted.bit_depth = bit_depth;
        adjusted.sample_rate = sample_rate;
        adjusted.quality_label = quality_label("FLAC", bit_depth, sample_rate, false);
        adjusted.quality_tier = quality_tier("FLAC", bit_depth).to_string();
        return Ok(adjusted);
    }

    if AudioByteValidator::is_mp3_magic(head) || ctype_lower.contains("audio/mpeg") {
        if is_short_lossy_preview(total_bytes, duration_ms) {
            return Err(
                "solo entrega preview MP3 de 30 s (cuenta sin suscripción de streaming activa)"
                    .to_string(),
            );
        }
        let mut adjusted = expected.clone();
        adjusted.format = "MP3".to_string();
        adjusted.codec = Some("mp3".to_string());
        adjusted.container = Some("MP3".to_string());
        adjusted.extension = "mp3".to_string();
        adjusted.bit_depth = None;
        adjusted.sample_rate = None;
        adjusted.quality_label = quality_label("MP3", None, None, true);
        adjusted.quality_tier = "lossy".to_string();
        return Ok(adjusted);
    }

    if AudioByteValidator::is_m4a_magic(head)
        || ctype_lower.contains("audio/mp4")
        || ctype_lower.contains("audio/aac")
    {
        let has_flac_atom = head.windows(4).any(|w| w == b"fLaC");
        if has_flac_atom && expected.format == "FLAC" {
            let mut adjusted = expected.clone();
            adjusted.format = "FLAC".to_string();
            adjusted.codec = Some("flac".to_string());
            adjusted.container = Some("M4A".to_string());
            adjusted.extension = "m4a".to_string();
            adjusted.quality_label =
                quality_label("FLAC", adjusted.bit_depth, adjusted.sample_rate, false);
            adjusted.quality_tier = quality_tier("FLAC", adjusted.bit_depth).to_string();
            return Ok(adjusted);
        }

        if is_short_lossy_preview(total_bytes, duration_ms) {
            return Err(
                "solo entrega preview AAC de 30 s (cuenta sin suscripción de streaming activa)"
                    .to_string(),
            );
        }
        let mut adjusted = expected.clone();
        adjusted.format = "AAC".to_string();
        adjusted.codec = Some("aac".to_string());
        adjusted.container = Some("M4A".to_string());
        adjusted.extension = "m4a".to_string();
        adjusted.bit_depth = None;
        adjusted.sample_rate = None;
        adjusted.quality_label = quality_label("AAC", None, None, true);
        adjusted.quality_tier = "lossy".to_string();
        return Ok(adjusted);
    }

    Err(format!(
        "cabecera de audio no reconocida (tipo {:?})",
        content_type.unwrap_or("desconocido")
    ))
}

fn is_short_lossy_preview(total_bytes: Option<u64>, duration_ms: Option<i64>) -> bool {
    let Some(bytes) = total_bytes else {
        return false;
    };
    if bytes == QOBUZ_PREVIEW_MP3_LEN {
        return true;
    }
    if let Some(dur_ms) = duration_ms {
        if dur_ms >= 60_000 {
            // Un corte de 30 s a ≤320 kbps ocupa ≤1.5 MB; cualquier pista completa
            // de ≥60 s supera holgadamente un piso conservador de 160 kbps sobre la
            // mitad de su duración catalogada.
            let min_full_bytes = ((dur_ms as u128 / 2) * 160) / 8;
            if (bytes as u128) < min_full_bytes && bytes <= 2_000_000 {
                return true;
            }
        }
    }
    false
}

/// `format_id` de Qobuz → (contenedor, bit depth, sample rate).
fn qobuz_format_info(format_id: &str) -> (&'static str, Option<i32>, Option<i32>) {
    match format_id {
        "27" => ("FLAC", Some(24), Some(192000)),
        "7" => ("FLAC", Some(24), Some(96000)),
        "6" => ("FLAC", Some(16), Some(44100)),
        "5" => ("MP3", None, None),
        _ => ("FLAC", None, None),
    }
}

fn resolution_format(codec: &str, extension: &str) -> &'static str {
    let codec = codec.to_lowercase();
    if codec.contains("flac") || extension == "flac" {
        "FLAC"
    } else if codec.contains("aac") || extension == "m4a" || extension == "mp4" {
        "AAC"
    } else if codec.contains("mp3") || extension == "mp3" {
        "MP3"
    } else {
        "AUDIO"
    }
}

fn normalize_extension(extension: &str, container: &str) -> String {
    let ext = extension.trim().trim_start_matches('.').to_lowercase();
    if ext.is_empty() {
        let container = container.to_lowercase();
        if container.contains("flac") {
            "flac".to_string()
        } else if container.contains("mp3") {
            "mp3".to_string()
        } else if container.contains("mp4") || container.contains("m4a") {
            "m4a".to_string()
        } else {
            "audio".to_string()
        }
    } else {
        ext
    }
}

/// Sólo se reproduce lo que sea un fichero progresivo: los manifiestos
/// (DASH `.mpd`, HLS `.m3u8`) o extensiones desconocidas se rechazan.
fn ensure_streamable(url: &str, extension: &str) -> Result<(), String> {
    let lower = url.to_lowercase();
    if lower.contains(".mpd") || lower.contains(".m3u8") {
        return Err(
            "la fuente es un manifiesto DASH/HLS, no reproducible de forma progresiva".to_string(),
        );
    }
    if !STREAMABLE_EXTENSIONS.contains(&extension) {
        return Err(format!(
            "contenedor no reproducible ({}); usa la descarga si necesitas esta pista",
            extension
        ));
    }
    Ok(())
}

fn quality_tier(format: &str, bit_depth: Option<i32>) -> &'static str {
    if format == "FLAC" && bit_depth == Some(24) {
        "hires"
    } else if format == "FLAC" {
        "lossless"
    } else {
        "lossy"
    }
}

/// Etiqueta honesta de la calidad obtenida. `lossy` fuerza el formato de
/// bits perdidos cuando la resolución no expone profundidad.
fn quality_label(
    format: &str,
    bit_depth: Option<i32>,
    sample_rate: Option<i32>,
    lossy: bool,
) -> String {
    match (bit_depth, sample_rate) {
        (Some(bits), Some(rate)) => format!(
            "{} {} bits / {} kHz",
            format,
            bits,
            format_sample_rate(rate)
        ),
        (None, _) if format == "MP3" || lossy => format!("{} 320 kbps", format),
        _ => format.to_string(),
    }
}

fn format_sample_rate(rate: i32) -> String {
    if rate % 1000 == 0 {
        (rate / 1000).to_string()
    } else {
        format!("{:.1}", rate as f64 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qobuz_format_ids_map_to_honest_quality() {
        assert_eq!(qobuz_format_info("27"), ("FLAC", Some(24), Some(192000)));
        assert_eq!(qobuz_format_info("7"), ("FLAC", Some(24), Some(96000)));
        assert_eq!(qobuz_format_info("6"), ("FLAC", Some(16), Some(44100)));
        assert_eq!(qobuz_format_info("5"), ("MP3", None, None));
    }

    #[test]
    fn quality_labels_are_honest() {
        assert_eq!(
            quality_label("FLAC", Some(24), Some(192000), false),
            "FLAC 24 bits / 192 kHz"
        );
        assert_eq!(
            quality_label("FLAC", Some(16), Some(44100), false),
            "FLAC 16 bits / 44.1 kHz"
        );
        assert_eq!(quality_label("MP3", None, None, true), "MP3 320 kbps");
    }

    #[test]
    fn manifests_are_rejected() {
        assert!(ensure_streamable("https://x/y.manifest.mpd", "mp4").is_err());
        assert!(ensure_streamable("https://x/y.m3u8", "m4a").is_err());
        assert!(ensure_streamable("https://x/y.flac", "flac").is_ok());
    }

    #[test]
    fn extensions_are_normalized() {
        assert_eq!(normalize_extension(".FLAC", ""), "flac");
        assert_eq!(normalize_extension("", "audio/flac"), "flac");
        assert_eq!(normalize_extension("", "audio/mp4"), "m4a");
    }

    fn expected_hires_info() -> StreamSourceInfo {
        StreamSourceInfo {
            service_name: "qobuz".to_string(),
            service_track_id: "123".to_string(),
            format: "FLAC".to_string(),
            codec: Some("flac".to_string()),
            container: Some("FLAC".to_string()),
            extension: "flac".to_string(),
            bit_depth: Some(24),
            sample_rate: Some(96000),
            quality_label: "FLAC 24 bits / 96 kHz".to_string(),
            quality_tier: "hires".to_string(),
            source_name: "Qobuz API oficial".to_string(),
        }
    }

    #[test]
    fn verify_rejects_qobuz_30s_mp3_preview_even_when_flac_was_reported() {
        let head = [0xff, 0xfb, 0xe2, 0x64, 0x00, 0x00, 0x00, 0x00];
        let err = verify_stream_header(
            &expected_hires_info(),
            &head,
            Some("audio/mpeg"),
            Some(QOBUZ_PREVIEW_MP3_LEN),
            Some(223_000),
        )
        .expect_err("preview MP3 must be rejected");
        assert!(err.contains("preview"), "unexpected error: {err}");
    }

    #[test]
    fn verify_relabels_full_length_mp3_honestly_as_lossy() {
        let head = [0xff, 0xfb, 0xe2, 0x64, 0x00, 0x00, 0x00, 0x00];
        let info = verify_stream_header(
            &expected_hires_info(),
            &head,
            Some("audio/mpeg"),
            Some(8_920_000),
            Some(223_000),
        )
        .expect("full-length MP3 should be accepted as lossy fallback");
        assert_eq!(info.format, "MP3");
        assert_eq!(info.extension, "mp3");
        assert_eq!(info.quality_tier, "lossy");
        assert_eq!(info.quality_label, "MP3 320 kbps");
        assert_eq!(info.bit_depth, None);
        assert_eq!(info.sample_rate, None);
    }

    #[test]
    fn verify_extracts_physical_flac_streaminfo_and_rejects_short_flac_preview() {
        // Construye cabecera fLaC + bloque STREAMINFO (34 bytes) para 24-bit / 48 kHz.
        let build_flac_header = |sample_rate: u32, bits: u8, total_samples: u64| -> Vec<u8> {
            let mut buf = Vec::with_capacity(42);
            buf.extend_from_slice(b"fLaC");
            buf.extend_from_slice(&[0x00, 0x00, 0x00, 34]); // STREAMINFO (type 0), len 34
            buf.extend_from_slice(&[0x10, 0x00, 0x10, 0x00]); // min/max block
            buf.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // min/max frame
            let sr_hi = ((sample_rate >> 4) & 0xFFFF) as u16;
            buf.extend_from_slice(&sr_hi.to_be_bytes());
            let channels_minus_one = 1u8; // 2 canales
            let bps_minus_one = bits - 1;
            let byte12 = (((sample_rate & 0x0F) as u8) << 4)
                | ((channels_minus_one & 0x07) << 1)
                | ((bps_minus_one >> 4) & 0x01);
            let byte13 = ((bps_minus_one & 0x0F) << 4) | (((total_samples >> 32) as u8) & 0x0F);
            buf.push(byte12);
            buf.push(byte13);
            buf.extend_from_slice(&(total_samples as u32).to_be_bytes());
            buf.extend_from_slice(&[0u8; 16]); // MD5
            buf
        };

        // Pista completa (200 s a 48 kHz = 9_600_000 muestras): etiqueta física 24/48 aunque se esperaba 24/96.
        let full_head = build_flac_header(48_000, 24, 9_600_000);
        let verified = verify_stream_header(
            &expected_hires_info(),
            &full_head,
            Some("audio/flac"),
            Some(QOBUZ_PREVIEW_MP3_LEN),
            Some(200_000),
        )
        .expect("full FLAC should pass even with small byte size");
        assert_eq!(verified.format, "FLAC");
        assert_eq!(verified.bit_depth, Some(24));
        assert_eq!(verified.sample_rate, Some(48_000));
        assert_eq!(verified.quality_label, "FLAC 24 bits / 48 kHz");
        assert_eq!(verified.quality_tier, "hires");

        // Preview FLAC de 30 s (1_440_000 muestras a 48 kHz) para pista de 200 s: se rechaza.
        let preview_head = build_flac_header(48_000, 24, 1_440_000);
        let err = verify_stream_header(
            &expected_hires_info(),
            &preview_head,
            Some("audio/flac"),
            Some(3_000_000),
            Some(200_000),
        )
        .expect_err("30s FLAC preview must be rejected");
        assert!(err.contains("preview"), "unexpected error: {err}");
    }

    #[test]
    fn extract_total_bytes_reads_content_range_on_206_and_content_length_on_200() {
        let mut headers_206 = reqwest::header::HeaderMap::new();
        headers_206.insert(
            reqwest::header::CONTENT_RANGE,
            reqwest::header::HeaderValue::from_static("bytes 0-65535/1200588"),
        );
        assert_eq!(
            extract_total_bytes(
                reqwest::StatusCode::PARTIAL_CONTENT,
                &headers_206,
                Some(65536)
            ),
            Some(1_200_588)
        );

        let headers_200 = reqwest::header::HeaderMap::new();
        assert_eq!(
            extract_total_bytes(reqwest::StatusCode::OK, &headers_200, Some(1_200_588)),
            Some(1_200_588)
        );
    }

    // ---- Fase 3: cascada ISRC -------------------------------------------------

    #[test]
    fn isrc_normalization_accepts_only_real_isrcs() {
        assert_eq!(
            normalize_isrc(Some("  usum71607007 ")).as_deref(),
            Some("USUM71607007")
        );
        // 12 chars but wrong shape (registrant/year/designation).
        assert_eq!(normalize_isrc(Some("US-UM7-16-07007")), None);
        assert_eq!(normalize_isrc(Some("USUM7160700")), None); // 11
        assert_eq!(normalize_isrc(Some("USUM716070077")), None); // 13
        assert_eq!(normalize_isrc(Some("")), None);
        assert_eq!(normalize_isrc(None), None);
    }

    #[test]
    fn expected_duration_never_invents_a_filter() {
        assert_eq!(expected_duration_sec(None), 0);
        assert_eq!(expected_duration_sec(Some(0)), 0);
        assert_eq!(expected_duration_sec(Some(-5_000)), 0);
        assert_eq!(expected_duration_sec(Some(223_000)), 223);
        assert_eq!(expected_duration_sec(Some(999)), 0);
    }

    #[test]
    fn isrc_cascade_skips_identities_already_tried() {
        let mut tried = HashSet::new();
        tried.insert(("qobuz".to_string(), "102570409".to_string()));
        assert!(!should_try(&tried, "qobuz", "102570409"));
        assert!(should_try(&tried, "qobuz", "102570410"));
        assert!(should_try(&tried, "tidal", "102570409"));
    }

    #[tokio::test]
    async fn lookup_context_reads_title_artist_duration_and_valid_isrc() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        let artist_id: i64 =
            sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Veri Fy') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, duration_ms, isrc) VALUES ('Pista Demo', 223000, 'verif2600001') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

        let context = load_lookup_context(&pool, track_id).await.expect("context");
        assert_eq!(context.title, "Pista Demo");
        assert_eq!(context.artist, "Veri Fy");
        assert_eq!(context.duration_ms, Some(223_000));
        assert_eq!(context.isrc.as_deref(), Some("VERIF2600001"));
        assert_eq!(context.expected_duration_sec(), 223);

        // A placeholder ISRC must never drive a search for another recording.
        let invalid_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, isrc) VALUES ('Otra', 'NOT_FOUND') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let invalid = load_lookup_context(&pool, invalid_id)
            .await
            .expect("context");
        assert_eq!(invalid.isrc, None);

        assert!(load_lookup_context(&pool, 999_999).await.is_none());
    }

    #[tokio::test]
    async fn isrc_cascade_is_honest_without_isrc_or_spotify() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let track_id: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Sin ISRC') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();

        let http = reqwest::Client::new();
        let err = isrc_cascade(&pool, &http, track_id, &HashSet::new(), false)
            .await
            .expect_err("no ISRC and no Spotify account: the cascade cannot guess");
        assert!(
            err.contains("ISRC"),
            "the message must name the missing identity: {err}"
        );
    }
}
