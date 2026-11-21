//! Refresco de credenciales de cuenta (issue #95, ADR-0007 decisión 14).
//!
//! Contrato: refresco **proactivo** antes de usar la credencial cuando el
//! proveedor publica la expiración (`expires_at` o el alias `token_expiry` de
//! Tidal) y **reactivo** al primer 401/403 cuando no la publica. Un fallo de
//! refresco rechazado por el proveedor —o un refresco imposible— marca la
//! cuenta para reautenticar y la puerta de `credentials_invalid` impide más
//! intentos: no hay bucle de reintentos. Un fallo **temporal** (red, 5xx,
//! respuesta ilegible) NO marca nada: una caída del proveedor no es una
//! credencial inválida (decisión 12) y la política es la misma que S185
//! aplicó al refresco de Tidal en `services/tidal_pipeline.rs`.
//!
//! Los dos puntos de llamada previstos ya consumen la puerta (adopción
//! A03-ter, issue #342): el sync por `get_or_refresh_spotify_token` (Spotify)
//! y `sync_reactive_credential_rejection` (reactivo del brazo Qobuz) en
//! `commands/service.rs`, y la descarga por la puerta proactiva de
//! `download_via_provider` y `persist_rejected_primary_account` en
//! `download/orchestrator.rs`. Los caminos que ya tenían refresco propio se
//! conservan: el brazo Tidal del sync refresca proactivamente por
//! `tidal_pipeline` (FIX 2026-08-25) y reacciona al 401 con UN refresh
//! forzado, y el brazo Qobuz del sync resuelve por el auto-login S186;
//! ambos convergen en la misma puerta anti-bucle (`credentials_invalid`).
//!
//! Garantías de artefacto: la persistencia es un `UPDATE ... WHERE id` (nunca
//! `INSERT`, así el refresco no duplica cuentas) y solo escribe tras un
//! transporte exitoso, así el artefacto anterior no se pierde si el refresco
//! falla. El payload nuevo se construye por **fusión** sobre el actual, de
//! modo que campos que el transporte no conoce (p. ej. `session_id` de Tidal)
//! sobreviven al refresco.

use serde_json::Value;

/// Ventana proactiva de refresco: paridad con los 300 s de
/// `get_or_refresh_spotify_token` (commands/service.rs) y del `is_expired`
/// de `TidalGuiCredentials` (crates/syncify-tidal-downloader).
pub const REFRESH_EXPIRY_BUFFER_SECS: i64 = 300;

/// Expiración publicada por el proveedor dentro del payload de credenciales.
///
/// Reconoce las dos claves que ya escribe el árbol: `expires_at` (Spotify,
/// Qobuz) y `token_expiry` (alias que el puente de Tidal puede traer). Un
/// valor ausente, no numérico o no positivo significa «el proveedor no
/// publica la expiración».
pub fn published_expiry_secs(creds: Option<&Value>) -> Option<f64> {
    let obj = creds?.as_object()?;
    let exp = obj
        .get("expires_at")
        .and_then(Value::as_f64)
        .or_else(|| obj.get("token_expiry").and_then(Value::as_f64))?;
    if exp > 0.0 {
        Some(exp)
    } else {
        None
    }
}

/// Decisión de refresco proactivo para una credencial almacenada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshDecision {
    /// La credencial vence (o está dentro del buffer): refrescar ANTES de
    /// usarla.
    Proactive,
    /// No hay vencimiento publicado, o aún está lejos: usar la credencial tal
    /// cual y reaccionar al primer 401/403.
    Trust,
    /// La cuenta ya está marcada para reautenticar: no tocar nada. Esta
    /// variante ES la puerta anti-bucle.
    Blocked,
}

/// Política proactiva pura: nada de red ni BD, reloj inyectado.
pub fn refresh_decision(
    creds: Option<&Value>,
    credentials_invalid: bool,
    now_secs: i64,
    buffer_secs: i64,
) -> RefreshDecision {
    if credentials_invalid {
        return RefreshDecision::Blocked;
    }
    match published_expiry_secs(creds) {
        Some(exp) if (now_secs as f64) >= exp - buffer_secs as f64 => RefreshDecision::Proactive,
        _ => RefreshDecision::Trust,
    }
}

/// ¿El status HTTP es un rechazo de credencial? Solo 401 y 403: un 5xx o un
/// fallo de red es «proveedor no disponible» (decisión 12), no reautenticación.
pub fn is_credential_rejection(status: u16) -> bool {
    matches!(status, 401 | 403)
}

/// `now` de época en segundos para los puntos de llamada en producción; los
/// tests inyectan su reloj falso por parámetro en su lugar.
pub fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Por qué falló un transporte de refresco. La variante decide el destino de
/// la cuenta, siguiendo la distinción S185 de `tidal_pipeline.rs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
    /// El proveedor rechazó el material de refresco (p. ej. 400/401/403 al
    /// grant): la credencial está muerta. La cuenta se marca para
    /// reautenticar.
    ProviderRejected(String),
    /// No hay material de refresco ni transporte programático para este
    /// servicio: solo reautenticando se recupera. La cuenta se marca.
    NotRefreshable(String),
    /// Fallo temporal (red, DNS, 5xx, respuesta ilegible): la cuenta queda
    /// VÁLIDA y el próximo uso reintenta; marcarla envenenaría credenciales
    /// sanas durante una caída.
    Transient(String),
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProviderRejected(reason) => {
                write!(f, "provider rejected the credential refresh: {reason}")
            }
            Self::NotRefreshable(reason) => {
                write!(
                    f,
                    "credential cannot be refreshed programmatically: {reason}"
                )
            }
            Self::Transient(reason) => {
                write!(f, "credential refresh failed transiently: {reason}")
            }
        }
    }
}

/// Transporte de refresco: recibe el payload DESCIFRADO actual y produce el
/// payload nuevo (sin cifrar) o el motivo del fallo. La implementación de
/// producción (`ProductionRefreshTransport`) despacha por servicio; los tests
/// inyectan transportes falsos con tokens fabricados.
///
/// El futuro va encajonado y acotado a `Send`: los puntos de llamada de
/// producción viven en futuros de comandos Tauri, que deben ser `Send`, y el
/// lint `async_fn_in_trait` existe justamente porque RPITIT no hereda esa
/// cota. Implementar es una línea de `Box::pin(async move { … })`.
pub trait CredentialRefreshTransport: Send + Sync {
    fn refresh<'a>(
        &'a self,
        db: &'a sqlx::SqlitePool,
        service: &'a str,
        account_id: i64,
        creds: &'a Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, RefreshError>> + Send + 'a>>;
}

/// Resultado del refresco proactivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// Transporte exitoso y payload nuevo persistido en la MISMA fila.
    Refreshed,
    /// No procede refrescar (sin vencimiento publicado, o aún fresco).
    NotNeeded,
    /// La cuenta ya estaba marcada: nada se intentó (puerta anti-bucle).
    NeedsReauth,
    /// El intento se hizo y el proveedor lo rechazó (o era imposible): la
    /// cuenta queda marcada AHORA para reautenticar.
    MarkedForReauth,
    /// Fallo temporal: sin marca; el próximo uso reintenta.
    RetryLater,
}

/// Resultado del manejo reactivo de un rechazo del proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionOutcome {
    /// Refresco reactivo exitoso: payload nuevo persistido en la misma fila.
    Refreshed,
    /// La cuenta quedó marcada para reautenticar (rechazo definitivo del
    /// refresco, vencimiento publicado con token aún vigente, o credencial
    /// ausente/indescifrable).
    MarkedForReauth,
    /// La cuenta YA estaba marcada: nada se intentó y no se re-emitió el
    /// evento (puerta anti-bucle).
    AlreadyMarked,
    /// El status no es 401/403: sin escritura en BD (decisión 12).
    NotCredentialRejection,
    /// El refresco falló de forma temporal: sin marca; el próximo uso
    /// reintenta.
    RetryLater,
}

/// Fila de cuenta ya resuelta para las entradas de este módulo.
struct AccountCredentials {
    credentials_invalid: bool,
    /// `Err` = credencial presente pero indescifrable o no JSON (decisión 10:
    /// reautenticar); `Ok(None)` = sin credencial almacenada.
    credentials: Result<Option<Value>, String>,
}

async fn load_account_credentials(
    db: &sqlx::SqlitePool,
    service: &str,
    account_id: i64,
) -> Result<AccountCredentials, String> {
    let row: Option<(Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT a.credentials_json, a.credentials_invalid \
         FROM accounts a JOIN services s ON s.id = a.service_id \
         WHERE a.id = ? AND s.name = ?",
    )
    .bind(account_id)
    .bind(service)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to load account {account_id} for refresh: {e}"))?;

    let Some((json_opt, invalid)) = row else {
        return Err(format!(
            "account {account_id} not found for service {service}"
        ));
    };

    let credentials = match json_opt.filter(|j| !j.trim().is_empty()) {
        None => Ok(None),
        Some(encrypted) => crate::crypto::decrypt(&encrypted)
            .map_err(|e| format!("credentials of account {account_id} cannot be decrypted: {e}"))
            .and_then(|plain| {
                serde_json::from_str::<Value>(&plain).map_err(|e| {
                    format!("credentials of account {account_id} are not valid JSON: {e}")
                })
            })
            .map(Some),
    };

    Ok(AccountCredentials {
        credentials_invalid: invalid.unwrap_or(0) != 0,
        credentials,
    })
}

/// Persistir el artefacto refrescado en la MISMA fila (nunca INSERT: el
/// refresco no duplica cuentas) y dejar la cuenta válida. Un refresco exitoso
/// también levanta una marca de credencial previa: el material es nuevo.
pub async fn apply_refreshed_credentials(
    db: &sqlx::SqlitePool,
    account_id: i64,
    payload: &Value,
) -> Result<(), String> {
    let serialized = serde_json::to_string(payload)
        .map_err(|e| format!("Failed to serialize refreshed credentials: {e}"))?;
    let encrypted = crate::crypto::encrypt(&serialized)?;
    sqlx::query(
        "UPDATE accounts SET credentials_json = ?, credentials_invalid = 0, \
         invalid_reason = NULL, last_auth_error = NULL WHERE id = ?",
    )
    .bind(&encrypted)
    .bind(account_id)
    .execute(db)
    .await
    .map_err(|e| {
        format!("Failed to persist refreshed credentials for account {account_id}: {e}")
    })?;
    Ok(())
}

async fn mark_refresh_failure(
    db: &sqlx::SqlitePool,
    service: &str,
    account_id: i64,
    reason: &str,
) -> Result<(), String> {
    // El motivo pasa por la frontera de persistencia de
    // `mark_account_credentials_invalid`, que lo sanea (tokens, cookies,
    // contraseñas) antes de escribirlo en `last_auth_error`.
    super::accounts::mark_account_credentials_invalid(db, service, reason, Some(account_id))
        .await
        .map(|_| ())
}

/// Un intento de transporte compartido por las dos entradas: escribe SOLO si
/// el transporte tuvo éxito, así el artefacto anterior no se pierde si falla.
/// El motivo concreto queda en los logs del brazo; las entradas solo deciden.
enum Attempt {
    Refreshed,
    TransientFailure,
    Rejected,
}

async fn attempt_refresh(
    db: &sqlx::SqlitePool,
    service: &str,
    account_id: i64,
    creds: &Value,
    transport: &dyn CredentialRefreshTransport,
) -> Result<Attempt, String> {
    match transport.refresh(db, service, account_id, creds).await {
        Ok(new_payload) => {
            apply_refreshed_credentials(db, account_id, &new_payload).await?;
            tracing::info!(
                service,
                account_id,
                "credential refreshed and persisted in place"
            );
            Ok(Attempt::Refreshed)
        }
        Err(RefreshError::Transient(reason)) => {
            tracing::warn!(
                service,
                account_id,
                reason = %reason,
                "credential refresh failed transiently; account left valid, next use will retry"
            );
            Ok(Attempt::TransientFailure)
        }
        Err(err @ (RefreshError::ProviderRejected(_) | RefreshError::NotRefreshable(_))) => {
            let reason = err.to_string();
            mark_refresh_failure(db, service, account_id, &reason).await?;
            tracing::warn!(
                service,
                account_id,
                reason = %reason,
                "credential refresh rejected; account marked for reauthentication (no retry loop)"
            );
            Ok(Attempt::Rejected)
        }
    }
}

/// Entrada proactiva (ADR-0007 decisión 14): con vencimiento publicado y
/// dentro del buffer, refresca ANTES de usar la credencial.
///
/// El reloj va inyectado (`now_secs`): los tests deterministas no dependen de
/// la pared. Producción llama con [`now_epoch_secs`].
pub async fn refresh_account_credential(
    db: &sqlx::SqlitePool,
    service: &str,
    account_id: i64,
    transport: &dyn CredentialRefreshTransport,
    now_secs: i64,
) -> Result<RefreshOutcome, String> {
    let account = load_account_credentials(db, service, account_id).await?;

    if account.credentials_invalid {
        // Puerta anti-bucle: una cuenta marcada no recibe intentos.
        return Ok(RefreshOutcome::NeedsReauth);
    }

    let creds = match account.credentials {
        Ok(Some(creds)) => creds,
        Ok(None) => return Ok(RefreshOutcome::NotNeeded),
        Err(reason) => {
            // Decisión 10: credencial indescifrable pide reautenticar; no se
            // inventan credenciales.
            mark_refresh_failure(db, service, account_id, &reason).await?;
            return Ok(RefreshOutcome::MarkedForReauth);
        }
    };

    if refresh_decision(Some(&creds), false, now_secs, REFRESH_EXPIRY_BUFFER_SECS)
        != RefreshDecision::Proactive
    {
        return Ok(RefreshOutcome::NotNeeded);
    }

    match attempt_refresh(db, service, account_id, &creds, transport).await? {
        Attempt::Refreshed => Ok(RefreshOutcome::Refreshed),
        Attempt::TransientFailure => Ok(RefreshOutcome::RetryLater),
        Attempt::Rejected => Ok(RefreshOutcome::MarkedForReauth),
    }
}

/// Entrada reactiva (ADR-0007 decisión 14): cuando el proveedor NO publica
/// la expiración, el primer 401/403 dispara UN intento de refresco; si el
/// proveedor lo rechaza, la cuenta se marca y no se reintenta en bucle.
///
/// Con vencimiento publicado el refresco proactivo ya actuó antes de usar la
/// credencial, así que un 401/403 con token vigente es un rechazo de
/// credencial: se marca sin intento de refresco (decisión 12).
///
/// `reason` es texto arbitrario del pipeline de llamada y se sanea en la
/// frontera de persistencia, igual que en el resto del árbol.
pub async fn handle_credential_rejection(
    db: &sqlx::SqlitePool,
    service: &str,
    account_id: i64,
    status: u16,
    transport: &dyn CredentialRefreshTransport,
    reason: &str,
) -> Result<RejectionOutcome, String> {
    if !is_credential_rejection(status) {
        return Ok(RejectionOutcome::NotCredentialRejection);
    }

    let account = load_account_credentials(db, service, account_id).await?;

    if account.credentials_invalid {
        // Segunda (o enésima) llamada tras una marca: nada se intenta y no se
        // re-emite el evento.
        return Ok(RejectionOutcome::AlreadyMarked);
    }

    let creds = match account.credentials {
        Ok(Some(creds)) => creds,
        Ok(None) => {
            // Un 401 sin credencial almacenada es una cuenta sin sesión:
            // no hay nada que refrescar, procede reautenticar.
            mark_refresh_failure(db, service, account_id, "no credentials stored").await?;
            return Ok(RejectionOutcome::MarkedForReauth);
        }
        Err(reason) => {
            mark_refresh_failure(db, service, account_id, &reason).await?;
            return Ok(RejectionOutcome::MarkedForReauth);
        }
    };

    if published_expiry_secs(Some(&creds)).is_some() {
        let reason =
            format!("credential rejected with HTTP {status} despite a published expiry: {reason}");
        mark_refresh_failure(db, service, account_id, &reason).await?;
        return Ok(RejectionOutcome::MarkedForReauth);
    }

    // Sin vencimiento conocido: UN intento de refresco en el PRIMER rechazo.
    match attempt_refresh(db, service, account_id, &creds, transport).await? {
        Attempt::Refreshed => Ok(RejectionOutcome::Refreshed),
        Attempt::TransientFailure => Ok(RejectionOutcome::RetryLater),
        Attempt::Rejected => Ok(RejectionOutcome::MarkedForReauth),
    }
}

// ═══════════════════════════════════════════════════════
// Constructores de payload puros (probados sin red)
// ═══════════════════════════════════════════════════════

/// Payload de Spotify desde la respuesta del token endpoint. Espeja el wire
/// format de `get_or_refresh_spotify_token` (grant `refresh_token`, buffer de
/// 5 min): `access_token` obligatorio, `refresh_token` conservado si la
/// respuesta no rota, `expires_in` con piso 3600.
pub fn spotify_refresh_payload(
    response: &Value,
    fallback_refresh_token: &str,
    now_secs: i64,
) -> Result<Value, String> {
    let access_token = response["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Missing access_token in refresh response")?;
    let refresh_token = response["refresh_token"]
        .as_str()
        .unwrap_or(fallback_refresh_token);
    let expires_in = response["expires_in"].as_i64().unwrap_or(3600);

    Ok(serde_json::json!({
        "token_type": "Bearer",
        "access_token": access_token,
        "refresh_token": refresh_token,
        "expires_at": now_secs + expires_in,
    }))
}

/// Payload de Tidal por FUSIÓN: los campos del struct actualizado pisan los
/// del payload actual y TODO campo que el struct no conoce (`session_id`,
/// `username`, …) sobrevive al refresco. Espeja la persistencia de
/// `resolve_and_refresh_gui_credentials_for_account`, que serializa el struct
/// actualizado y lo re-cifra.
pub fn tidal_refresh_payload(
    current: &Value,
    updated: &syncify_tidal_downloader::TidalGuiCredentials,
) -> Result<Value, String> {
    let mut payload = current.clone();
    let updated_json = serde_json::to_value(updated)
        .map_err(|e| format!("Failed to serialize refreshed Tidal credentials: {e}"))?;
    if let (Some(obj), Some(new_fields)) = (payload.as_object_mut(), updated_json.as_object()) {
        for (key, value) in new_fields {
            obj.insert(key.clone(), value.clone());
        }
    }
    Ok(payload)
}

// ═══════════════════════════════════════════════════════
// Transporte de producción (red viva; los tests lo sustituyen)
// ═══════════════════════════════════════════════════════

/// Transporte de producción: Spotify por PKCE (`grant_type=refresh_token`,
/// mismo wire format que `get_or_refresh_spotify_token`) y Tidal por el
/// device-flow OAuth del crate `syncify-tidal-downloader`. El resto de
/// servicios no tiene transporte programático: el intento termina en
/// [`RefreshError::NotRefreshable`] y la cuenta se marca para reautenticar,
/// que es el comportamiento contratado para Qobuz, Deezer, SoundCloud y
/// Apple Music (re-login).
pub struct ProductionRefreshTransport {
    pub http: reqwest::Client,
}

impl Default for ProductionRefreshTransport {
    fn default() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }
}

impl CredentialRefreshTransport for ProductionRefreshTransport {
    fn refresh<'a>(
        &'a self,
        db: &'a sqlx::SqlitePool,
        service: &'a str,
        account_id: i64,
        creds: &'a Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, RefreshError>> + Send + 'a>>
    {
        Box::pin(async move {
            match service.to_lowercase().as_str() {
                "spotify" => self.refresh_spotify(db, account_id, creds).await,
                "tidal" => self.refresh_tidal(creds).await,
                other => Err(RefreshError::NotRefreshable(format!(
                    "no programmatic refresh for service '{other}'; reauthentication required"
                ))),
            }
        })
    }
}

impl ProductionRefreshTransport {
    async fn refresh_spotify(
        &self,
        db: &sqlx::SqlitePool,
        _account_id: i64,
        creds: &Value,
    ) -> Result<Value, RefreshError> {
        let refresh_token = creds["refresh_token"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                RefreshError::NotRefreshable(
                    "Missing refresh token - please reconnect to Spotify".to_string(),
                )
            })?;

        // S196: credenciales desde ajustes de BD primero, env como fallback.
        let config = crate::commands::resolve_spotify_config(db)
            .await
            .map_err(|e| RefreshError::Transient(format!("Spotify config error: {e}")))?;

        let params = [
            ("client_id", config.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        let token_resp = self
            .http
            .post("https://accounts.spotify.com/api/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| RefreshError::Transient(format!("Refresh request failed: {e}")))?;

        let status = token_resp.status();
        if !status.is_success() {
            let error = token_resp.text().await.unwrap_or_default();
            // Un rechazo del grant (400/401/403) mata la credencial; un 5xx es
            // temporal (decisión 12).
            if matches!(status.as_u16(), 400 | 401 | 403) {
                return Err(RefreshError::ProviderRejected(format!(
                    "Token refresh failed: {error}"
                )));
            }
            return Err(RefreshError::Transient(format!(
                "Token refresh failed: {error}"
            )));
        }

        let token_data: Value = token_resp.json().await.map_err(|e| {
            RefreshError::Transient(format!("Failed to parse refresh response: {e}"))
        })?;

        spotify_refresh_payload(&token_data, refresh_token, now_epoch_secs())
            .map_err(RefreshError::Transient)
    }

    async fn refresh_tidal(&self, creds: &Value) -> Result<Value, RefreshError> {
        let parsed: syncify_tidal_downloader::TidalGuiCredentials =
            serde_json::from_value(creds.clone()).map_err(|e| {
                RefreshError::Transient(format!(
                    "Stored Tidal credentials do not match the payload schema: {e}"
                ))
            })?;

        let (_new_token, updated_creds) =
            syncify_tidal_downloader::refresh_gui_token(&self.http, &parsed)
                .await
                .map_err(|e| match e {
                    syncify_tidal_downloader::PipelineError::RequiresAuth(_) => {
                        RefreshError::ProviderRejected(e.to_string())
                    }
                    _ => RefreshError::Transient(e.to_string()),
                })?;

        tidal_refresh_payload(creds, &updated_creds).map_err(RefreshError::Transient)
    }
}

#[cfg(test)]
mod account_refresh_tests {
    use super::{
        is_credential_rejection, published_expiry_secs, refresh_decision, spotify_refresh_payload,
        tidal_refresh_payload, RefreshDecision,
    };
    use serde_json::json;

    const NOW: i64 = 1_800_000_000;

    /// Con vencimiento publicado y vencido: proactivo.
    #[test]
    fn vencimiento_pasado_es_proactivo() {
        let creds = json!({ "access_token": "t", "expires_at": NOW - 1 });
        assert_eq!(
            refresh_decision(Some(&creds), false, NOW, 300),
            RefreshDecision::Proactive
        );
    }

    /// Dentro del buffer (300 s) también es proactivo: la paridad con Spotify
    /// no emite llamadas con un token a segundos de vencer.
    #[test]
    fn dentro_del_buffer_es_proactivo() {
        let creds = json!({ "expires_at": NOW + 299 });
        assert_eq!(
            refresh_decision(Some(&creds), false, NOW, 300),
            RefreshDecision::Proactive
        );
    }

    /// Fuera del buffer: confiar y reaccionar al 401/403 si llegara.
    #[test]
    fn vencimiento_lejano_es_confianza() {
        let creds = json!({ "expires_at": NOW + 301 });
        assert_eq!(
            refresh_decision(Some(&creds), false, NOW, 300),
            RefreshDecision::Trust
        );
    }

    /// Sin vencimiento publicado NO se refresca proactivamente: el camino es
    /// reactivo al primer 401/403 (criterio del issue).
    #[test]
    fn sin_vencimiento_es_confianza() {
        let creds = json!({ "user_auth_token": "qobuz-token" });
        assert_eq!(
            refresh_decision(Some(&creds), false, NOW, 300),
            RefreshDecision::Trust
        );
        assert_eq!(
            refresh_decision(None, false, NOW, 300),
            RefreshDecision::Trust
        );
    }

    /// Una cuenta ya marcada está bloqueada sea cual sea el vencimiento: esa
    /// variante es la puerta anti-bucle.
    #[test]
    fn cuenta_marcada_esta_bloqueada() {
        let creds = json!({ "expires_at": NOW - 1 });
        assert_eq!(
            refresh_decision(Some(&creds), true, NOW, 300),
            RefreshDecision::Blocked
        );
        assert_eq!(
            refresh_decision(None, true, NOW, 300),
            RefreshDecision::Blocked
        );
    }

    /// El alias `token_expiry` de Tidal también publica vencimiento.
    #[test]
    fn alias_token_expiry_de_tidal_publica_vencimiento() {
        let creds = json!({ "token_expiry": (NOW + 100) as f64 });
        assert_eq!(
            published_expiry_secs(Some(&creds)),
            Some((NOW + 100) as f64)
        );
        assert_eq!(
            refresh_decision(Some(&creds), false, NOW, 300),
            RefreshDecision::Proactive
        );
    }

    /// Un `expires_at` no positivo NO es vencimiento publicado.
    #[test]
    fn vencimiento_no_positivo_no_cuenta() {
        assert_eq!(
            published_expiry_secs(Some(&json!({ "expires_at": 0 }))),
            None
        );
        assert_eq!(
            published_expiry_secs(Some(&json!({ "expires_at": -5 }))),
            None
        );
        assert_eq!(
            published_expiry_secs(Some(&json!({ "expires_at": "mañana" }))),
            None
        );
    }

    /// Solo 401 y 403 son rechazos de credencial; 5xx y otros 4xx no (decisión
    /// 12: una caída del proveedor no invalida credenciales).
    #[test]
    fn solo_401_y_403_son_rechazos() {
        assert!(is_credential_rejection(401));
        assert!(is_credential_rejection(403));
        assert!(!is_credential_rejection(500));
        assert!(!is_credential_rejection(429));
        assert!(!is_credential_rejection(400));
        assert!(!is_credential_rejection(200));
    }

    /// Payload de Spotify: access_token nuevo, refresh_token rotado respetado,
    /// expires_at absoluto.
    #[test]
    fn payload_de_spotify_con_rotacion() {
        let response = json!({
            "access_token": "nuevo",
            "refresh_token": "rotado",
            "expires_in": 3600,
            "token_type": "Bearer",
            "scope": "user-read-email",
        });
        let payload = spotify_refresh_payload(&response, "viejo", NOW).expect("payload construido");
        assert_eq!(payload["access_token"], "nuevo");
        assert_eq!(payload["refresh_token"], "rotado");
        assert_eq!(payload["expires_at"], NOW + 3600);
    }

    /// Si la respuesta no rota el refresh_token, se conserva el anterior.
    #[test]
    fn payload_de_spotify_sin_rotacion_conserva_el_anterior() {
        let response = json!({ "access_token": "nuevo", "expires_in": 1800 });
        let payload = spotify_refresh_payload(&response, "viejo", NOW).expect("payload construido");
        assert_eq!(payload["refresh_token"], "viejo");
        assert_eq!(payload["expires_at"], NOW + 1800);
    }

    /// Una respuesta sin access_token no produce payload: el fallo es
    /// temporal, no un rechazo.
    #[test]
    fn payload_de_spotify_sin_access_token_falla() {
        let response = json!({ "error": "invalid_grant" });
        assert!(spotify_refresh_payload(&response, "viejo", NOW).is_err());
    }

    /// Payload de Tidal por fusión: los campos nuevos pisan y los ajenos al
    /// struct (`session_id`) sobreviven.
    #[test]
    fn payload_de_tidal_fusiona_preservando_ajenos() {
        let current = json!({
            "access_token": "viejo",
            "refresh_token": "rt",
            "token_expiry": 100.0,
            "session_id": "sesion-viva",
            "username": "cuenta",
        });
        let updated = syncify_tidal_downloader::TidalGuiCredentials {
            access_token: "nuevo".to_string(),
            refresh_token: Some("rt".to_string()),
            token_expiry: Some(200.0),
            expires_at: None,
            expires_in: None,
            user_id: None,
            country_code: None,
            client_id: None,
            client_secret: None,
            session_id: None,
        };
        let payload = tidal_refresh_payload(&current, &updated).expect("payload fusionado");
        assert_eq!(payload["access_token"], "nuevo");
        assert_eq!(payload["token_expiry"], 200.0);
        // Campos ajenos al struct: intactos.
        assert_eq!(payload["session_id"], "sesion-viva");
        assert_eq!(payload["username"], "cuenta");
    }
}
