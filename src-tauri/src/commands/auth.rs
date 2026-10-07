#[allow(unused_imports)]
use super::*;

// Auth Commands - submodule of crate::commands
//
// Python auth bridge, session validation

// ==============================================
// AUTH CONCURRENCY LOCK
// ==============================================

static AUTH_IN_PROGRESS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Tope del subproceso de autenticación, en segundos.
///
/// Los flujos de navegador del puente dan al usuario hasta 300 s para completar
/// el login (`scripts/services/tidal_auth.py:260`, `scripts/services/qobuz_auth.py:162`,
/// `scripts/services/deezer_auth.py:67`). Con 120 s Rust mataba el proceso a
/// mitad del login interactivo y devolvía un timeout genérico, de modo que un
/// login lento nunca llegaba a completarse y la cuenta quedaba sin guardar.
/// El margen sobre 300 deja que sea el propio Python el que emita su JSON de
/// "Authorization timed out", que sí explica qué pasó.
pub const AUTH_BRIDGE_TIMEOUT_SECS: u64 = 330;

struct AuthGuard;

impl Drop for AuthGuard {
    fn drop(&mut self) {
        AUTH_IN_PROGRESS.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

// ==============================================
// PYTHON AUTH BRIDGE COMMANDS
// ==============================================

/// Auth result from Python subprocess
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Redact sensitive tokens and credentials from output strings before logging or surfacing in errors
pub fn redact_auth_payload(raw: &str) -> String {
    let mut sanitized = raw.to_string();
    for key in &[
        "access_token",
        "refresh_token",
        "client_secret",
        "secret",
        "password",
        "user_token",
        "session_id",
        "sp_dc",
    ] {
        let pattern = format!(r#""{}":\s*"[^"]+""#, key);
        if let Ok(re) = regex::Regex::new(&pattern) {
            sanitized = re
                .replace_all(&sanitized, format!(r#""{}": "[REDACTED]""#, key))
                .to_string();
        }
    }
    sanitized
}

/// Helper to run auth_bridge.py subprocess safely.
/// If `stdin_payload` is provided, it is securely piped via child stdin, preventing
/// sensitive tokens (such as Spotify sp_dc) from appearing in process arguments (/proc/<pid>/cmdline).
pub async fn run_auth_bridge_subprocess(
    service: &str,
    action: &str,
    stdin_payload: Option<&str>,
) -> Result<AuthResult, String> {
    tracing::info!(
        "run_auth_bridge_subprocess: service={} action={}",
        service,
        action
    );

    let project_root = get_project_root();
    let python_cmd = get_python_executable();
    let script_path = crate::cmd_utils::find_scripts_dir(&project_root).join("auth_bridge.py");

    tracing::debug!(
        "Auth bridge: python={}, script={:?}, cwd={:?}, has_stdin={}",
        python_cmd,
        script_path,
        project_root,
        stdin_payload.is_some()
    );

    let mut cmd = crate::cmd_utils::create_tokio_command(&python_cmd);
    cmd.kill_on_drop(true);
    cmd.arg(&script_path)
        .arg(service)
        .arg(action)
        .current_dir(&project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if stdin_payload.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    } else {
        cmd.stdin(std::process::Stdio::null());
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn Python auth_bridge subprocess: {}", e))?;

    if let Some(payload) = stdin_payload {
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                stdin.write_all(payload.as_bytes()),
            )
            .await
            .map_err(|_| "Timed out writing credentials to auth_bridge stdin".to_string())?
            .map_err(|e| {
                format!(
                    "Failed to write credentials payload to auth_bridge stdin: {}",
                    e
                )
            })?;
            let _ = stdin.flush().await;
            drop(stdin); // Close child stdin to send EOF
        }
    }

    let output = tokio::time::timeout(
        std::time::Duration::from_secs(AUTH_BRIDGE_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        format!(
            "Authentication bridge timed out after {} seconds",
            AUTH_BRIDGE_TIMEOUT_SECS
        )
    })?
    .map_err(|e| format!("Failed to wait for auth_bridge: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let redacted_stdout = redact_auth_payload(&stdout);
    tracing::info!(
        "Auth subprocess execution finished (output redacted): {}",
        redacted_stdout
    );
    if !stderr.is_empty() {
        tracing::warn!("Auth stderr (redacted): {}", redact_auth_payload(&stderr));
    }

    parse_auth_bridge_output(
        service,
        &stdout,
        &stderr,
        output.status.success(),
        output.status.code(),
    )
}

/// Convierte la salida de `auth_bridge.py` en `AuthResult`.
///
/// `json_response` cierra con código 1 en cuanto `success` es false, de modo que
/// el payload JSON que explica el fallo ya está escrito en stdout cuando el
/// proceso termina. Se deserializa siempre que exista: descartarlo obligaba a
/// inventar un error genérico y perdía el mensaje real ("Abre esta URL para
/// conectar Tidal: …", credenciales ausentes…). Cubierto por
/// `tests/auth_bridge_json_error_preserved_test.rs`.
pub fn parse_auth_bridge_output(
    service: &str,
    stdout: &str,
    stderr: &str,
    exit_ok: bool,
    exit_code: Option<i32>,
) -> Result<AuthResult, String> {
    let redacted_stdout = redact_auth_payload(stdout);

    if stdout.trim().is_empty() {
        let err_detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("Python exited with code {:?}", exit_code)
        };
        return Err(format!(
            "Auth bridge error for service '{}': {}",
            service, err_detail
        ));
    }

    // Parse JSON result - extract JSON from output (may have debug lines before it)
    // The auth result JSON always starts with {"success" - find that marker
    let json_str = stdout.trim();
    let json_result = if let Some(start) = json_str.find(r#"{"success""#) {
        // Extract from {"success" to the end
        let potential_json = &json_str[start..];
        serde_json::from_str::<AuthResult>(potential_json)
    } else {
        // Fallback: try parsing the whole string
        serde_json::from_str::<AuthResult>(json_str)
    };

    match json_result {
        Ok(mut result) => {
            if !exit_ok && result.error.is_none() {
                result.error = Some(if stderr.trim().is_empty() {
                    format!("Python exited with code {:?}", exit_code)
                } else {
                    stderr.trim().to_string()
                });
            }
            Ok(result)
        }
        Err(e) if !exit_ok => {
            let err_detail = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                format!("Python exited with code {:?}", exit_code)
            };
            Err(format!(
                "Auth bridge error for service '{}': {} (unparsable output: {})",
                service, err_detail, e
            ))
        }
        Err(e) => Err(format!(
            "Failed to parse auth result: {} (raw output: {})",
            e, redacted_stdout
        )),
    }
}

/// Start auth flow for a service (spawns Python subprocess)
#[tauri::command]
pub async fn start_auth(service: String, action: String) -> Result<AuthResult, String> {
    run_auth_bridge_subprocess(&service, &action, None).await
}

/// Refresh Spotify session access token using sp_dc cookie securely transmitted via stdin (SEC-022)
#[tauri::command]
pub async fn refresh_spotify_session(sp_dc: String) -> Result<AuthResult, String> {
    if sp_dc.trim().is_empty() {
        return Err("sp_dc cookie cannot be empty".to_string());
    }

    let payload = serde_json::json!({
        "sp_dc": sp_dc.trim()
    })
    .to_string();

    run_auth_bridge_subprocess("spotify", "refresh", Some(&payload)).await
}

/// Get auth status for a service
#[tauri::command]
pub async fn get_auth_status(service: String) -> Result<AuthResult, String> {
    start_auth(service, "status".to_string()).await
}

/// Logout from a service.
///
/// Multi-cuenta: `account_id` selecciona QUÉ cuenta se cierra sesión. `None`
/// conserva el contrato histórico ("la cuenta activa del servicio"). Ninguna
/// rama borra la fila: solo `remove_account` elimina cuentas y sus datos CASCADE.
///
/// Limitación conocida del puente: `scripts/auth_bridge.py` guarda UN solo
/// archivo de sesión POR SERVICIO (p.ej. `scripts/services/tidal_auth.py`), así
/// que el logout del puente solo se ejecuta cuando no queda ninguna otra cuenta
/// del servicio en BD. Con cuentas coexistentes se limpian las credenciales de la
/// fila afectada y el archivo compartido se deja intacto.
#[tauri::command]
pub async fn logout_service(
    service: String,
    account_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<AuthResult, String> {
    // Resolve the row to act on: explicit account_id (validated to belong to this
    // service) or the active one.
    let service_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = LOWER(?)")
            .bind(&service)
            .fetch_one(&state.db)
            .await
            .map_err(|e| format!("Failed to find {} service: {}", service, e))?;

    let target_id: Option<i64> = match account_id {
        Some(aid) => {
            let owned: Option<i64> =
                sqlx::query_scalar("SELECT id FROM accounts WHERE id = ? AND service_id = ?")
                    .bind(aid)
                    .bind(service_id)
                    .fetch_optional(&state.db)
                    .await
                    .map_err(|e| format!("Failed to resolve account: {}", e))?;
            Some(owned.ok_or_else(|| {
                format!("Account {} does not belong to the {} service", aid, service)
            })?)
        }
        None => sqlx::query_scalar(
            r#"SELECT id FROM accounts
               WHERE service_id = ? AND is_active = 1
               ORDER BY id DESC LIMIT 1"#,
        )
        .bind(service_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("Failed to resolve active account: {}", e))?,
    };

    // "Desconectar" NO borra la cuenta: limpia las credenciales/flags de ESA
    // fila y conserva la fila con su biblioteca y sus playlists (eliminar la fila
    // dispara CASCADE sobre library_entries/playlists y es una acción separada,
    // `remove_account`). Antes, la rama spotify borraba TODAS las filas del
    // servicio y las demás solo llamaban al puente, dejando la fila con unas
    // credenciales que el puente ya había invalidado.
    let Some(target_id) = target_id else {
        // Retained library rows do not keep a bridge session alive. Only
        // credentialed accounts can still use the shared provider session.
        let row_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ? AND credentials_json IS NOT NULL AND IFNULL(credentials_invalid, 0) = 0")
                .bind(service_id)
                .fetch_one(&state.db)
                .await
                .map_err(|e| format!("Failed to count {} accounts: {}", service, e))?;
        if row_count > 0 {
            return Ok(AuthResult {
                success: true,
                data: Some(serde_json::json!({
                    "account_id": null,
                    "remaining_accounts": row_count,
                    "bridge_logout_skipped": true,
                })),
                error: None,
            });
        }
        if service.eq_ignore_ascii_case("spotify") {
            return Ok(AuthResult {
                success: true,
                data: Some(serde_json::json!({ "account_id": null, "remaining_accounts": 0 })),
                error: None,
            });
        }
        return bridge_logout(&service).await;
    };

    // Clearing credentials, deactivating this row and handing over the active
    // slot (if a viable sister exists) happen in ONE transaction. With no
    // credentialed successor, the service has zero active accounts after logout.
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to begin logout for {}: {}", service, e))?;

    let was_active: Option<i64> = sqlx::query_scalar("SELECT is_active FROM accounts WHERE id = ?")
        .bind(target_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Failed to read account {}: {}", target_id, e))?;

    sqlx::query(
        r#"
        UPDATE accounts
        SET credentials_json = NULL,
            credentials = NULL,
            credentials_invalid = 1,
            invalid_reason = 'logged_out',
            last_auth_error = 'Session closed by the user',
            last_auth_error_at = ?,
            is_active = 0
        WHERE id = ?
        "#,
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(target_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to clear {} account credentials: {}", service, e))?;

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ? AND id != ? AND credentials_json IS NOT NULL AND IFNULL(credentials_invalid, 0) = 0")
            .bind(service_id)
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("Failed to count remaining accounts: {}", e))?;

    // A previously logged-out sister remains in the database for its library,
    // but must NEVER be resurrected as the active account by a later logout.
    // Choose only a sister with stored, non-invalidated credentials. If none
    // exists, the service has zero active accounts until a new login/activation.
    let viable_successor: Option<i64> = if remaining > 0 && was_active.unwrap_or(0) != 0 {
        sqlx::query_scalar(
            "SELECT id FROM accounts
             WHERE service_id = ? AND id != ?
               AND credentials_json IS NOT NULL
               AND IFNULL(credentials_invalid, 0) = 0
             ORDER BY id DESC LIMIT 1",
        )
        .bind(service_id)
        .bind(target_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Failed to pick the next {} account: {}", service, e))?
    } else {
        None
    };
    let successor: Option<i64> = if let Some(successor) = viable_successor {
        // Deactivate every other row and then activate the successor, so the
        // service never keeps two active rows even if it arrived with residues.
        sqlx::query("UPDATE accounts SET is_active = 0 WHERE service_id = ? AND id != ?")
            .bind(service_id)
            .bind(successor)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to deactivate sibling accounts: {}", e))?;

        sqlx::query("UPDATE accounts SET is_active = 1 WHERE id = ?")
            .bind(successor)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to activate account {}: {}", successor, e))?;

        tracing::info!(
            "[Auth] {} account {} logged out; account {} is now the active one",
            service,
            target_id,
            successor
        );

        Some(successor)
    } else {
        None
    };

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit logout for {}: {}", service, e))?;

    crate::commands::emit_auth_state_updated_for_account(&service, "logout", Some(target_id), None);

    if remaining > 0 {
        // At least one other authenticated account can still use the shared
        // bridge session. Retained but disconnected library rows do not count.
        return Ok(AuthResult {
            success: true,
            data: Some(serde_json::json!({
                "account_id": target_id,
                "remaining_accounts": remaining,
                "new_active_account_id": successor,
                "bridge_logout_skipped": true,
            })),
            error: None,
        });
    }

    if service.eq_ignore_ascii_case("spotify") {
        tracing::info!("Spotify native logout: credentials cleared, no bridge session to drop");

        return Ok(AuthResult {
            success: true,
            data: Some(serde_json::json!({
                "account_id": target_id,
                "remaining_accounts": remaining,
            })),
            error: None,
        });
    }

    let res = bridge_logout(&service).await;
    if let Ok(ref r) = res {
        if r.success {
            crate::commands::emit_auth_state_updated_for_account(
                &service,
                "logout_bridge",
                Some(target_id),
                None,
            );
        }
    }
    res
}

/// Drop the bridge session file of a service.
///
/// Only safe when no other account has usable stored credentials: the bridge
/// keeps ONE session per service (`scripts/auth_bridge.py`). Retained library
/// rows without credentials do not need that session.
async fn bridge_logout(service: &str) -> Result<AuthResult, String> {
    start_auth(service.to_string(), "logout".to_string()).await
}

/// Cached secrets may only supplement a bridge response for the SAME Qobuz user.
/// An absent username is not evidence of identity, even when just one row exists.
fn qobuz_cache_matches_login(bridge_username: Option<&str>, cached_username: Option<&str>) -> bool {
    bridge_username
        .is_some_and(|name| cached_username.is_some_and(|cached| cached.eq_ignore_ascii_case(name)))
}

/// Validate that a Qobuz auth token is usable (defensive filter against storage artifacts).
pub fn is_viable_qobuz_token_auth(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() || t == "browser_cookies" || t == "null" || t == "undefined" {
        return false;
    }
    if t.starts_with('{') || t.starts_with('[') || t.starts_with("eyJ") {
        return false;
    }
    if t.len() < 16 {
        return false;
    }
    !t.chars().any(|c| c.is_whitespace())
}

/// Check whether a service has one retained row. This alone does not prove
/// the bridge login belongs to that row: callers must compare identities before
/// inheriting authentication secrets.
pub async fn service_has_exactly_one_account(db: &sqlx::SqlitePool, service_name: &str) -> bool {
    let count: Option<i64> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounts a JOIN services s ON s.id = a.service_id
         WHERE LOWER(s.name) = LOWER(?)",
    )
    .bind(service_name)
    .fetch_one(db)
    .await
    .ok();

    count == Some(1)
}

/// Load Qobuz fallback auth data from the canonical encrypted SQLite database (AES-256-GCM).
/// Returns (token, username/email, password) when available from an existing active account.
///
/// Returns data only for a single active row; login callers must also check
/// that the bridge's authenticated username agrees before reusing secrets.
pub async fn load_qobuz_db_fallback_auth(
    db: &sqlx::SqlitePool,
) -> (Option<String>, Option<String>, Option<String>) {
    if !service_has_exactly_one_account(db, "qobuz").await {
        return (None, None, None);
    }

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT a.credentials_json FROM accounts a
         JOIN services s ON s.id = a.service_id
         WHERE LOWER(s.name) = 'qobuz' AND a.is_active = 1
         ORDER BY a.id DESC LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let enc_json = match row {
        Some((enc,)) => enc,
        None => return (None, None, None),
    };

    let decrypted = match crate::crypto::decrypt(&enc_json) {
        Ok(d) => d,
        Err(_) => return (None, None, None),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&decrypted) {
        Ok(v) => v,
        Err(_) => return (None, None, None),
    };

    let token = parsed
        .get("user_auth_token")
        .or_else(|| parsed.get("auth_token"))
        .or_else(|| parsed.get("access_token"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| is_viable_qobuz_token_auth(s));

    let username = parsed
        .get("username")
        .or_else(|| parsed.get("email"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| is_plausible_qobuz_credential_value(s));

    let password = parsed
        .get("password")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .filter(|s| is_plausible_qobuz_credential_value(s));

    (token, username, password)
}

/// S186: Reject console-error artifacts that were captured as credential values.
///
/// Forensics (syncify-dev.log:283332): the string `[Error] [Tauri] Command
/// "sync_service" failed: – "RequiresAuth: …"` was entered into the Qobuz web login
/// form's password field; the browser bridge saved it as the account password,
/// producing credentials that can never auto-login and poisoning every later
/// reconnect through the cache fallback. Real passwords never look like console
/// output; these patterns are safe to reject at both capture and save time.
pub fn is_plausible_qobuz_credential_value(value: &str) -> bool {
    let v = value.trim();
    if v.is_empty() || v.len() > 128 {
        return false;
    }
    if v.starts_with('[') {
        return false;
    }
    if v.contains("invokeCommand") || v.contains("failed:") || v.contains(r#"Command ""#) {
        return false;
    }
    true
}

/// S185: Read tidal.token_expiry (epoch seconds) from the encrypted SQLite database if available.
///
/// Multi-account: only when Tidal has exactly one row (see
/// [`service_has_exactly_one_account`]); with two or more rows the expiry would
/// belong to another user's session.
pub async fn load_tidal_db_cached_token_expiry(db: &sqlx::SqlitePool) -> Option<f64> {
    if !service_has_exactly_one_account(db, "tidal").await {
        return None;
    }

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT a.credentials_json FROM accounts a
         JOIN services s ON s.id = a.service_id
         WHERE LOWER(s.name) = 'tidal' AND a.is_active = 1
         ORDER BY a.id DESC LIMIT 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let (enc,) = row?;
    let decrypted = crate::crypto::decrypt(&enc).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&decrypted).ok()?;
    parsed.get("token_expiry").and_then(|v| v.as_f64())
}

/// S185: Ensure saved Tidal credentials always carry an expiry timestamp.
///
/// Without one, TidalGuiCredentials::is_expired() treats every freshly-logged-in
/// account as immediately expired (it has a refresh_token), forcing a network OAuth
/// refresh on the very first download. When that round-trip hit a transport error
/// the account used to be permanently invalidated — the "login ok → RequiresAuth
/// anyway" loop. Injects the real cached expiry when available; otherwise falls back
/// to a conservative +1h so a fresh login stays usable without ever trusting an
/// already-dead token. An expiry already present in the payload is preserved.
pub fn inject_tidal_expiry(
    credentials_payload: &mut serde_json::Value,
    cached_token_expiry: Option<f64>,
) {
    // FIX 2026-08-25: 1 h fabricaba expiraciones falsas cuando el caché de
    // Python no se podía leer → ciclos de re-auth percibidos. Tidal emite
    // tokens de días; 4 h es un piso conservador honesto hasta el próximo
    // refresh proactivo.
    const CONSERVATIVE_TIDAL_EXPIRY_SECS: f64 = 14400.0;

    if credentials_payload
        .get("token_expiry")
        .and_then(|v| v.as_f64())
        .is_some()
        || credentials_payload
            .get("expires_at")
            .and_then(|v| v.as_f64())
            .is_some()
    {
        return; // payload already carries an expiry — preserve it untouched
    }

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let expiry = match cached_token_expiry {
        Some(exp) if exp > now_secs => exp,
        _ => now_secs + CONSERVATIVE_TIDAL_EXPIRY_SECS,
    };
    let expires_in = (expiry - now_secs).max(0.0);

    if let Some(obj) = credentials_payload.as_object_mut() {
        obj.insert("token_expiry".to_string(), serde_json::Value::from(expiry));
        obj.insert("expires_at".to_string(), serde_json::Value::from(expiry));
        obj.insert(
            "expires_in".to_string(),
            serde_json::Value::from(expires_in),
        );
    }
}

/// Start auth flow and save credentials to database
/// This is the main command for UI-driven authentication
#[tauri::command]
pub async fn start_auth_and_save(
    service: String,
    state: State<'_, AppState>,
) -> Result<AuthResult, String> {
    tracing::info!("start_auth_and_save: {}", service);

    // FIX 2026-08-25: el rechazo temprano quedó obsoleto — el brazo
    // "apple_music" del motor unificado ahora delega al importador de
    // biblioteca (captura ISRC). Conectar vuelve a tener sentido; si faltan
    // tokens al sincronizar, el sync lo reporta con RequiresAuth.

    // Step 1: Run auth flow via Python bridge
    let auth_result = start_auth(service.clone(), "login".to_string()).await?;

    if !auth_result.success {
        return Ok(auth_result);
    }

    // Step 2: Extract user info from auth result
    let data = auth_result
        .data
        .as_ref()
        .ok_or("Auth succeeded but returned no data")?;

    let mut credentials_payload = data.clone();

    if service.eq_ignore_ascii_case("qobuz") {
        let (cache_token, cache_username, cache_password) =
            load_qobuz_db_fallback_auth(&state.db).await;
        // A single existing row is not proof of identity. Never attach its
        // token or password to a newly authenticated bridge username.
        let bridge_username = data
            .get("username")
            .or_else(|| data.get("email"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let same_identity = qobuz_cache_matches_login(bridge_username, cache_username.as_deref());
        let cache_token = if same_identity { cache_token } else { None };
        let cache_password = if same_identity { cache_password } else { None };
        let cache_username = if same_identity { cache_username } else { None };

        let token = data
            .get("user_auth_token")
            .or_else(|| data.get("auth_token"))
            .or_else(|| data.get("access_token"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| is_viable_qobuz_token_auth(s))
            .or(cache_token);

        let username = data
            .get("username")
            .or_else(|| data.get("email"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| is_plausible_qobuz_credential_value(s))
            .or(cache_username);

        let password = data
            .get("password")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| is_plausible_qobuz_credential_value(s))
            .or(cache_password);

        if let Some(obj) = credentials_payload.as_object_mut() {
            if let Some(t) = &token {
                obj.insert(
                    "user_auth_token".to_string(),
                    serde_json::Value::String(t.clone()),
                );
                obj.insert(
                    "auth_token".to_string(),
                    serde_json::Value::String(t.clone()),
                );
            }
            if let Some(u) = &username {
                obj.insert("username".to_string(), serde_json::Value::String(u.clone()));
            }
            if let Some(p) = &password {
                obj.insert("password".to_string(), serde_json::Value::String(p.clone()));
            }
        }

        let has_env_fallback = std::env::var("QOBUZ_PASSWORD").is_ok()
            && (std::env::var("QOBUZ_USERNAME").is_ok() || std::env::var("QOBUZ_EMAIL").is_ok());

        if token.is_none() && (username.is_none() || password.is_none()) && !has_env_fallback {
            return Ok(AuthResult {
                success: false,
                data: None,
                error: Some(
                    "Qobuz reconnect completed in browser but did not provide API token or fallback credentials. Please login manually in the Qobuz form (without auto-skip), or configure QOBUZ_USERNAME/QOBUZ_EMAIL + QOBUZ_PASSWORD.".to_string(),
                ),
            });
        }
    }

    // S185: Tidal device-flow payload arrives without expiry fields; inject the real
    // cached token_expiry (or a conservative fallback) so the first download after
    // login does not force an immediate OAuth refresh that could fail transiently.
    if service.eq_ignore_ascii_case("tidal") {
        let cached_expiry = load_tidal_db_cached_token_expiry(&state.db).await;
        inject_tidal_expiry(&mut credentials_payload, cached_expiry);
    }

    // Extract common fields (services return slightly different shapes)
    let display_name = data
        .get("display_name")
        .or_else(|| data.get("user"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let email = data
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let user_id = data.get("user_id").and_then(|v| {
        v.as_str()
            .map(|s| s.to_string())
            .or_else(|| v.as_i64().map(|n| n.to_string()))
    });

    // The Qobuz bridge sometimes reports the literal "browser_session" when
    // it could not discover a real user id. That is a shared placeholder, NOT
    // an identity: treating it as external_user_id would upsert every Qobuz
    // login into the same row, even when the captured usernames differ.
    let external_user_id = login_external_user_id(&service, &data, user_id.as_deref());

    // Step 3: Look up service ID
    let service_row: Option<(i64,)> = sqlx::query_as("SELECT id FROM services WHERE name = ?")
        .bind(&service)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

    let service_id = service_row
        .ok_or_else(|| format!("Unknown service: {}", service))?
        .0;

    // Step 4: Encrypt credentials
    let credentials_json = credentials_payload.to_string();
    let encrypted = crate::crypto::encrypt(&credentials_json)?;

    // Step 5: Upsert account — resolve by identity (email and/or external_user_id)
    // and keep every other row of the service so its CASCADE-linked library data
    // survives a re-login or a second account of the same service.
    let final_display_name = display_name
        .or(user_id.clone())
        .unwrap_or_else(|| format!("{} User", service));

    let account_id = upsert_service_account(
        &state.db,
        service_id,
        &final_display_name,
        email.as_deref(),
        external_user_id.as_deref(),
        &encrypted,
    )
    .await?;

    tracing::info!(
        "Saved {} account: {} (account_id={})",
        service,
        final_display_name,
        account_id
    );

    // Re-queue de descargas: por SERVICIO, no por cuenta — download_queue no
    // tiene dimensión de cuenta en esta iteración, así que las descargas
    // pendientes de otra cuenta del mismo servicio también vuelven a la cola.
    // Es correcto para el caso mayoritario (un 401 por servicio), pero un
    // re-login de la cuenta A puede re-encolar descargas que eran de la B.

    // Auto-retry downloads stuck in requires_auth / failed for this service
    let re_queued = sqlx::query(
        r#"
        UPDATE download_queue
        SET status = 'queued',
            last_error = NULL,
            error_message = NULL,
            retry_count = 0,
            started_at = NULL,
            completed_at = NULL
        WHERE status IN ('requires_auth', 'failed')
          AND (LOWER(service_name) = LOWER(?) OR service_name IS NULL)
        "#,
    )
    .bind(&service)
    .execute(&state.db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    if re_queued > 0 {
        tracing::info!(
            "[Auth] Automatically re-queued {} failed downloads for {}",
            re_queued,
            service
        );
    }

    crate::commands::emit_auth_state_updated(&service, "connected", Some(&final_display_name));

    // Return success with saved info
    Ok(AuthResult {
        success: true,
        data: Some(serde_json::json!({
            "message": format!("Connected as {}", final_display_name),
            "display_name": final_display_name,
            "email": email,
            "user_id": user_id,
            "account_id": account_id,
            "requeued_downloads": re_queued,
        })),
        error: None,
    })
}

/// S185 + multi-cuenta: upsert de cuenta por IDENTIDAD, no pisacuentas.
///
/// Garantiza el invariante que la UI y el pipeline siguen esperando — exactamente
/// UNA cuenta activa por servicio, seleccionable con
/// `WHERE is_active = 1 ORDER BY id DESC LIMIT 1` — conservando TODAS las filas,
/// de modo que la biblioteca CASCADE de cada cuenta sobrevive a un re-login:
///
///   1. La fila destino se resuelve por identidad, no por fecha:
///      `LOWER(TRIM(email))` y/o `external_user_id`. Antes, un email que no
///      coincidía con ninguna fila tomaba la MÁS NUEVA del servicio y le
///      sobrescribía email y credenciales, destruyendo por CASCADE la biblioteca
///      de la cuenta anterior. Ahora, una identidad nueva (email o user_id) que
///      no casa con ninguna fila INSERTA una cuenta nueva.
///   2. Si email y user_id apuntan a filas DISTINTAS gana `user_match`: el user_id
///      es la identidad estable del proveedor y sobrevive a un cambio de email.
///      Además elimina por construcción el conflicto con el índice parcial
///      `idx_accounts_service_external_user`.
///   3. Sin ninguna identidad (login sin email ni user_id — deezer, soundcloud)
///      se conserva el comportamiento histórico: reutilizar la fila más nueva.
///   4. `ORDER BY id DESC LIMIT 1` en las dos consultas NO es cosmético: la
///      pasada de normalización de 0089 no puede fusionar parejas de email
///      case-variantes que una BD trajera ya, y ambas casan igual por
///      `LOWER(TRIM(email))`. Sin el ORDER BY, `fetch_optional` devolvería una
///      fila arbitraria y podría revivir la hermana equivocada.
///   5. La fila destino recibe credenciales limpias, `is_active = 1` y su
///      `external_user_id`. Si reescribir el email choca con
///      UNIQUE(service_id, email) (hermana case-variante) se reintenta SIN tocar
///      la columna email: la identidad ya casó case-insensitive.
///   6. Las hermanas SOLO se desactivan (`is_active = 0`). Ya no se les borran
///      `credentials_invalid`/`last_auth_error`: una cuenta desactivada
///      conserva su estado `requires_auth` histórico para cuando vuelva a
///      activarse.
///
/// Devuelve el `account_id` de la fila insertada o actualizada.
pub async fn upsert_service_account(
    db: &sqlx::SqlitePool,
    service_id: i64,
    display_name: &str,
    email: Option<&str>,
    external_user_id: Option<&str>,
    encrypted_credentials: &str,
) -> Result<i64, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to begin account upsert: {}", e))?;

    // 1) Normalize the identity coming from the login. `email` is user-typed, so
    //    'A@X.com ' and 'a@x.com' are the same account; `external_user_id` is an
    //    opaque provider id, so only whitespace is trimmed.
    let email_norm = email
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty());
    let email_ref = email_norm.as_deref();
    let user_norm = external_user_id
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    let user_ref = user_norm.as_deref();

    // SoundCloud profile failures return user_id=0. With no other identity,
    // treating the login as a relogin of the newest row overwrites a stranger's
    // credentials, so create a distinct row until the provider identifies it.
    let anonymous_soundcloud: bool =
        sqlx::query_scalar::<_, String>("SELECT name FROM services WHERE id = ?")
            .bind(service_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to inspect account service: {e}"))?
            .is_some_and(|name| name.eq_ignore_ascii_case("soundcloud"))
            && email_ref.is_none()
            && user_ref.is_none();

    // 2) Resolve the target row by identity. Both lookups are evaluated whenever
    //    the login carries the corresponding value: an account that changed its
    //    email at the provider still has a stable user_id, and falling back to
    //    "newest row" for it would be the pisacuentas regression again.
    let email_match: Option<i64> = if email_ref.is_some() {
        sqlx::query_scalar(
            r#"
            SELECT id FROM accounts
            WHERE service_id = ? AND LOWER(TRIM(email)) = ?
            ORDER BY id DESC LIMIT 1
            "#,
        )
        .bind(service_id)
        .bind(email_ref)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Failed to look up account by email: {}", e))?
    } else {
        None
    };

    let user_match: Option<i64> = if user_ref.is_some() {
        sqlx::query_scalar(
            r#"
            SELECT id FROM accounts
            WHERE service_id = ? AND external_user_id = ?
            ORDER BY id DESC LIMIT 1
            "#,
        )
        .bind(service_id)
        .bind(user_ref)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Failed to look up account by user_id: {}", e))?
    } else {
        None
    };

    // 3) Reconciliation of both identities.
    //    * both hit the same row            → that row.
    //    * both hit DIFFERENT rows          → user_id wins (stable identity; the
    //      email of that row is refreshed below). This is also what makes the
    //      partial index collision impossible by construction.
    //    * only one hit                      → that row.
    //    * identity present but NOTHING matched → 0 (INSERT): un email o un
    //      user_id que no casan con ninguna fila son una cuenta NUEVA. falling
    //      back to "the newest row" here would overwrite an unrelated account
    //      (the pisacuentas regression) and, with services that return no email,
    //      would grow the table on every login.
    //    * no identity at all (no email AND no user_id) → newest row for legacy
    //      services such as Deezer. SoundCloud's user_id=0 means profile lookup
    //      failed, so each such login must insert rather than overwrite.
    let target_id = match (email_match, user_match) {
        (Some(email_id), Some(user_id)) if email_id != user_id => user_id,
        (Some(email_id), _) => email_id,
        (None, Some(user_id)) => user_id,
        (None, None) if email_ref.is_none() && user_ref.is_none() && !anonymous_soundcloud => {
            sqlx::query_scalar(
                r#"
                SELECT id FROM accounts
                WHERE service_id = ?
                ORDER BY id DESC LIMIT 1
                "#,
            )
            .bind(service_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to look up account: {}", e))?
            .unwrap_or(0)
        }
        (None, None) => 0,
    };

    // 0 = identity that matches nothing → a genuinely new account (multi-account).
    let resolved_id = if target_id == 0 {
        let inserted: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO accounts (service_id, display_name, email, external_user_id, credentials_json, credentials_invalid, invalid_reason, last_auth_error, is_active, last_synced, created_at)
            VALUES (?, ?, ?, ?, ?, 0, NULL, NULL, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING id
            "#,
        )
        .bind(service_id)
        .bind(display_name)
        .bind(email_ref)
        .bind(user_ref)
        .bind(encrypted_credentials)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed") {
                format!(
                    "Could not create the account: another row of this service already uses \
                     that email or user id ({}). Try reconnecting, or remove the duplicate account.",
                    msg
                )
            } else {
                format!("Failed to save account: {}", msg)
            }
        })?;

        tracing::info!(
            "New account row {} created for service_id={} (multi-account login)",
            inserted,
            service_id
        );
        inserted
    } else {
        // 4) Activate and clean ONLY the target row (preserves its CASCADE-linked data).
        let full_update = sqlx::query(
            r#"
            UPDATE accounts
            SET display_name = ?,
                email = ?,
                external_user_id = ?,
                credentials_json = ?,
                credentials_invalid = 0,
                invalid_reason = NULL,
                last_auth_error = NULL,
                is_active = 1,
                last_synced = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(display_name)
        .bind(email_ref)
        .bind(user_ref)
        .bind(encrypted_credentials)
        .bind(target_id)
        .execute(&mut *tx)
        .await;

        match full_update {
            Ok(_) => {}
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("accounts.service_id") && msg.contains("accounts.email") {
                    // Case-variant sibling: the identity already matched
                    // case-insensitively, so rewriting the column would only
                    // collide with the binary UNIQUE of 0002. Retry keeping the
                    // stored email untouched.
                    tracing::warn!(
                        "Account {} keeps its stored email (case-variant sibling present): {}",
                        target_id,
                        msg
                    );
                    sqlx::query(
                        r#"
                        UPDATE accounts
                        SET display_name = ?,
                            external_user_id = ?,
                            credentials_json = ?,
                            credentials_invalid = 0,
                            invalid_reason = NULL,
                            last_auth_error = NULL,
                            is_active = 1,
                            last_synced = CURRENT_TIMESTAMP
                        WHERE id = ?
                        "#,
                    )
                    .bind(display_name)
                    .bind(user_ref)
                    .bind(encrypted_credentials)
                    .bind(target_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to update account: {}", e))?;
                } else if msg.contains("external_user_id") {
                    return Err(format!(
                        "Another account of this service already uses that provider user id. \
                         Remove it before logging in again ({})",
                        msg
                    ));
                } else {
                    return Err(format!("Failed to update account: {}", msg));
                }
            }
        }

        target_id
    };

    // 5) Exclusividad: la cuenta conectada queda como la ÚNICA activa del
    //    servicio, tanto si se reutilizó una fila como si se insertó una nueva.
    //    Los flags de las hermanas no se tocan a propósito: una cuenta
    //    desactivada conserva su estado requires_auth histórico para cuando
    //    vuelva a activarse.
    sqlx::query(
        r#"
        UPDATE accounts
        SET is_active = 0
        WHERE service_id = ? AND id != ?
        "#,
    )
    .bind(service_id)
    .bind(resolved_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to deactivate sibling accounts: {}", e))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit account upsert: {}", e))?;

    Ok(resolved_id)
}

/// Resolve a provider identity without confusing a bridge session marker with
/// a user. The Qobuz browser bridge returns `browser_session` if its user/get
/// probe found no id; use the captured username instead when present. Keep the
/// provider's real id case-sensitive, and do not borrow a cached username from
/// another account (only the raw bridge payload is consulted here).
fn login_external_user_id(
    service: &str,
    data: &serde_json::Value,
    user_id: Option<&str>,
) -> Option<String> {
    let provider_id = user_id.map(str::trim).filter(|s| !s.is_empty());
    if service.eq_ignore_ascii_case("soundcloud") && provider_id == Some("0") {
        // The bridge uses 0 when the profile request fails, not a user ID.
        return None;
    }
    if !service.eq_ignore_ascii_case("qobuz") {
        return provider_id.map(str::to_string);
    }

    provider_id
        .filter(|id| *id != "browser_session")
        .map(str::to_string)
        .or_else(|| {
            data.get("username")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// Session status for a single service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatus {
    pub service: String,
    pub connected: bool,
    pub valid: bool,
    pub message: String,
    pub user_info: Option<String>,
}

/// Traduce la respuesta del puente de autenticación a un estado de sesión.
///
/// A-3: antes esta lógica estaba en línea y solo tenía DOS salidas posibles
/// (`true` o `false`), más un `connected: true` INCONDICIONAL en
/// `validate_all_sessions` — la UI veía sesiones no válidas pintadas como
/// conectadas. Y un `Err(e)` de un subproceso caído (Python no está, el bridge
/// falló) se leía como "sesión inválida", que es una afirmación que ese error
/// no puede sostener.
///
/// Ahora se separa el caso "no se pudo comprobar" del caso "caducada":
/// - `valid: true`  → comprobada y válida.
/// - `connected: false` → el proveedor la rechazó explícitamente.
/// - `valid: false` + mensaje de comprobación → no verificable; la UI debe
///   ofrecer reintentar, no re-loguear.
///
/// Multi-cuenta: `validate_all_sessions` ya no lo usa —el veredicto por cuenta
/// sale de `perform_get_service_auth_status`, porque el probe del puente describe
/// la sesión compartida del servicio, no una fila concreta— y el flujo de login
/// (`get_auth_status`) devuelve el `AuthResult` crudo a la UI sin traducirlo.
/// Sus únicos usos restantes son sus propias pruebas unitarias, de ahí el
/// `cfg(test)` (sin él, `cargo clippy -- -D warnings` lo marcaría dead_code).
#[cfg(test)]
fn session_status_from_probe(result: Result<AuthResult, String>) -> SessionStatus {
    match result {
        Ok(res) if res.success => {
            let connected = res
                .data
                .as_ref()
                .and_then(|d| d.get("connected"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if connected {
                SessionStatus {
                    service: String::new(),
                    connected: true,
                    valid: true,
                    message: "Session valid".to_string(),
                    user_info: None,
                }
            } else {
                SessionStatus {
                    service: String::new(),
                    connected: false,
                    valid: false,
                    message: "Session expired or invalid".to_string(),
                    user_info: None,
                }
            }
        }
        // El puente respondió, pero con error: puede ser un rechazo real del
        // proveedor o un fallo de su propio entorno. Se propaga el mensaje.
        Ok(res) => SessionStatus {
            service: String::new(),
            connected: false,
            valid: false,
            message: res.error.unwrap_or_else(|| "Unknown error".to_string()),
            user_info: None,
        },
        // El subproceso ni siquiera arrancó: NO sabemos nada de la sesión.
        Err(e) => SessionStatus {
            service: String::new(),
            connected: false,
            valid: false,
            message: format!("Could not verify session (check failed: {})", e),
            user_info: None,
        },
    }
}

/// Validate all connected service sessions.
///
/// Multi-cuenta: el veredicto sale de la BD por cuenta
/// ([`perform_get_service_auth_status`]), NO del probe del puente. El puente
/// tiene UN archivo de sesión por servicio, así que su "Session valid/invalid"
/// describe la última sesión de navegador del servicio, no la fila A o la fila B:
/// con dos cuentas conectadas ya no puede afirmar nada por cuenta. El probe del
/// puente sigue siendo la fuente del flujo de login (`get_auth_status`), donde
/// solo hay una sesión en curso.
#[tauri::command]
pub async fn validate_all_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<SessionStatus>, String> {
    tracing::info!("validate_all_sessions called");

    // Get all connected accounts (active ones: an inactive account is not a
    // session the app is using).
    let accounts: Vec<(i64, String, String)> = sqlx::query_as(
        r#"
        SELECT a.id, s.name, IFNULL(a.display_name, a.email)
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE a.is_active = 1
        ORDER BY s.name, a.id DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    let mut statuses = Vec::new();

    for (account_id, service_name, display_name) in accounts {
        let verdict =
            match perform_get_service_auth_status(&state.db, &service_name, Some(account_id)).await
            {
                Ok(status) => SessionStatus {
                    service: service_name.clone(),
                    connected: status.account_id.is_some(),
                    valid: status.is_authenticated && status.credentials_valid,
                    message: status
                        .error_message
                        .unwrap_or_else(|| format!("status: {}", status.status)),
                    user_info: Some(display_name),
                },
                Err(e) => SessionStatus {
                    service: service_name.clone(),
                    connected: true,
                    valid: false,
                    message: format!("Could not read account state (check failed: {})", e),
                    user_info: Some(display_name),
                },
            };

        statuses.push(verdict);
    }

    tracing::info!(
        "Session validation complete: {} accounts checked",
        statuses.len()
    );
    Ok(statuses)
}

// ==============================================
// SPOTIFY WEBVIEW AUTH (S65 + S66 PKCE + SEC-019 STATE CSRF)
// ==============================================

/// Error types encountered when validating a Spotify OAuth callback request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyCallbackError {
    NotCallback,
    MissingState,
    InvalidState,
    MissingCode,
    OAuthError(String),
}

impl std::fmt::Display for SpotifyCallbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotCallback => write!(f, "Invalid path or HTTP method"),
            Self::MissingState => write!(f, "Missing state parameter"),
            Self::InvalidState => write!(f, "State parameter mismatch (potential CSRF)"),
            Self::MissingCode => write!(f, "Missing authorization code"),
            Self::OAuthError(err) => write!(f, "Spotify returned OAuth error: {}", err),
        }
    }
}

impl std::error::Error for SpotifyCallbackError {}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Parse and validate Spotify OAuth callback request.
/// Returns Ok(auth_code) if both code and state are valid and match expected_state.
pub fn validate_spotify_callback(
    raw_request: &str,
    expected_state: &str,
) -> Result<String, SpotifyCallbackError> {
    let first_line = raw_request.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");

    if method != "GET" || !target.starts_with("/callback") {
        return Err(SpotifyCallbackError::NotCallback);
    }

    let query_str = match target.split_once('?') {
        Some((_, q)) => q,
        None => return Err(SpotifyCallbackError::MissingState),
    };

    let mut code = None;
    let mut state = None;
    let mut error = None;

    for pair in query_str.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((key, val)) => (key, val),
            None => (pair, ""),
        };
        let decoded_v = urlencoding::decode(v)
            .map(|cow| cow.into_owned())
            .unwrap_or_else(|_| v.to_string());

        match k {
            "code" => code = Some(decoded_v),
            "state" => state = Some(decoded_v),
            "error" => error = Some(decoded_v),
            _ => {}
        }
    }

    let received_state = state.ok_or(SpotifyCallbackError::MissingState)?;
    if received_state != expected_state {
        return Err(SpotifyCallbackError::InvalidState);
    }

    if let Some(err_name) = error {
        return Err(SpotifyCallbackError::OAuthError(err_name));
    }

    let auth_code = code.ok_or(SpotifyCallbackError::MissingCode)?;
    if auth_code.is_empty() {
        return Err(SpotifyCallbackError::MissingCode);
    }

    Ok(auth_code)
}

/// Build the HTTP response tuple (status_code, raw_http_response) for a Spotify callback result.
pub fn build_spotify_callback_response(
    result: &Result<String, SpotifyCallbackError>,
) -> (u16, String) {
    match result {
        Ok(_) => (
            200,
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Connection: close\r\n\
             \r\n\
             <html><body style=\"background:#121212;color:#1db954;display:flex;align-items:center;justify-content:center;height:100vh;font-family:sans-serif;font-size:24px;font-weight:bold;\">\
             Autenticado. Puedes cerrar esta ventana.</body></html>"
                .to_string(),
        ),
        Err(SpotifyCallbackError::NotCallback) => (
            404,
            "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n".to_string(),
        ),
        Err(SpotifyCallbackError::MissingState) => (
            400,
            "HTTP/1.1 400 Bad Request\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Connection: close\r\n\
             \r\n\
             <html><body style=\"background:#121212;color:#e22134;display:flex;align-items:center;justify-content:center;height:100vh;font-family:sans-serif;font-size:20px;font-weight:bold;\">\
             Error de autenticaci&oacute;n: Par&aacute;metro state ausente (posible ataque CSRF).</body></html>"
                .to_string(),
        ),
        Err(SpotifyCallbackError::InvalidState) => (
            400,
            "HTTP/1.1 400 Bad Request\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Connection: close\r\n\
             \r\n\
             <html><body style=\"background:#121212;color:#e22134;display:flex;align-items:center;justify-content:center;height:100vh;font-family:sans-serif;font-size:20px;font-weight:bold;\">\
             Error de autenticaci&oacute;n: Par&aacute;metro state inv&aacute;lido (posible ataque CSRF).</body></html>"
                .to_string(),
        ),
        Err(SpotifyCallbackError::MissingCode) => (
            400,
            "HTTP/1.1 400 Bad Request\r\n\
             Content-Type: text/html; charset=utf-8\r\n\
             Connection: close\r\n\
             \r\n\
             <html><body style=\"background:#121212;color:#e22134;display:flex;align-items:center;justify-content:center;height:100vh;font-family:sans-serif;font-size:20px;font-weight:bold;\">\
             Error de autenticaci&oacute;n: C&oacute;digo de autorizaci&oacute;n ausente.</body></html>"
                .to_string(),
        ),
        Err(SpotifyCallbackError::OAuthError(e)) => (
            400,
            format!(
                "HTTP/1.1 400 Bad Request\r\n\
                 Content-Type: text/html; charset=utf-8\r\n\
                 Connection: close\r\n\
                 \r\n\
                 <html><body style=\"background:#121212;color:#e22134;display:flex;align-items:center;justify-content:center;height:100vh;font-family:sans-serif;font-size:20px;font-weight:bold;\">\
                 Error de Spotify: {}</body></html>",
                html_escape(e)
            ),
        ),
    }
}

/// Helper to parse, validate and formulate response for incoming Spotify callback requests.
pub fn process_spotify_callback_request(
    raw_request: &str,
    expected_state: &str,
) -> (u16, Result<String, SpotifyCallbackError>, String) {
    let result = validate_spotify_callback(raw_request, expected_state);
    let (status, response) = build_spotify_callback_response(&result);
    (status, result, response)
}

/// Authenticate Spotify using native Tauri WebView2 window and PKCE OAuth2 flow.
#[tauri::command]
pub async fn spotify_auth_webview(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AuthResult, String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use rand::{rngs::OsRng, RngCore};
    use sha2::{Digest, Sha256};
    use tauri::Manager;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    tracing::info!("spotify_auth_webview: starting PKCE auth flow");

    if AUTH_IN_PROGRESS.swap(true, std::sync::atomic::Ordering::SeqCst) {
        tracing::warn!("spotify_auth_webview: Auth already in progress, blocking concurrent call");
        return Err("Auth already in progress".to_string());
    }
    let _guard = AuthGuard;

    // Close any existing auth window to avoid label collision
    if let Some(existing) = app.get_webview_window("spotify-auth") {
        let _ = existing.close();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // 1. Generate code_verifier (random 64 bytes base64url)
    let mut verifier_bytes = [0u8; 64];
    OsRng.fill_bytes(&mut verifier_bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

    // 2. Calculate code_challenge (SHA256 of verifier, base64url)
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let challenge_bytes = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(challenge_bytes);

    // 3. Generate CSRF state token (random 32 bytes base64url)
    let mut state_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut state_bytes);
    let expected_state = URL_SAFE_NO_PAD.encode(state_bytes);

    let config = crate::services::spotify::SpotifyConfig::from_env()
        .map_err(|e| format!("Spotify config error: {}", e))?;
    let client_id = config.client_id;
    let redirect_uri = "http://127.0.0.1:8888/callback";
    // user-follow-read: sin él, /me/following (artistas seguidos, fase S189-F2)
    // responde 403 "Insufficient client scope" para tokens emitidos antes de
    // pedirlo; debe coincidir con SPOTIFY_SCOPES en services/spotify.rs.
    let scope = "user-library-read playlist-read-private user-read-private user-read-email user-follow-read";

    let auth_url = format!(
        "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&code_challenge_method=S256&code_challenge={}&scope={}&state={}",
        // A4: encode client_id too — a pasted value with reserved characters
        // must never break URL parsing below.
        urlencoding::encode(&client_id),
        urlencoding::encode(redirect_uri),
        code_challenge,
        urlencoding::encode(scope),
        urlencoding::encode(&expected_state)
    );

    // 3. Bind TcpListener on 127.0.0.1:8888
    let listener = TcpListener::bind("127.0.0.1:8888")
        .await
        .map_err(|e| format!("Failed to bind port 8888 for callback: {}", e))?;

    // 4. Open WebView pointing to accounts.spotify.com
    // A4: no unwrap — malformed URLs return an actionable error instead of a panic.
    let parsed_url: tauri::Url = auth_url
        .parse()
        .map_err(|e| format!("Failed to build Spotify authorize URL: {}", e))?;
    let auth_window = tauri::WebviewWindowBuilder::new(
        &app,
        "spotify-auth",
        tauri::WebviewUrl::External(parsed_url),
    )
    .title("Connect Spotify")
    .inner_size(500.0, 700.0)
    .resizable(false)
    .closable(true)
    .build()
    .map_err(|e| format!("Failed to create auth window: {}", e))?;

    let timeout_duration = std::time::Duration::from_secs(300);
    let mut code_opt = None;

    // 6. TcpListener captures the GET request
    match tokio::time::timeout(timeout_duration, async {
        loop {
            let (mut socket, _) = listener.accept().await?;
            let mut buf = [0; 1024];
            let n = socket.read(&mut buf).await?;
            if n == 0 {
                continue;
            }
            let request = String::from_utf8_lossy(&buf[..n]);

            let (status, callback_result, response) =
                process_spotify_callback_request(&request, &expected_state);

            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;

            match callback_result {
                Ok(code) => {
                    tracing::info!(
                        "spotify_auth_webview: valid callback with matching state received"
                    );
                    code_opt = Some(code);
                    break;
                }
                Err(e) => {
                    tracing::warn!(
                        "spotify_auth_webview: invalid callback attempt: {:?} (HTTP {})",
                        e,
                        status
                    );
                }
            }
        }
        Ok::<(), std::io::Error>(())
    })
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => return Err(format!("Socket error: {}", e)),
        Err(_) => {
            let _ = auth_window.close();
            if let Ok(profile_dir) = app.path().app_local_data_dir() {
                let _ = crate::crypto::audit_and_purge_webview_localstorage(&profile_dir);
                let _ = crate::crypto::ensure_secure_profile_permissions(&profile_dir);
            }
            return Err("Authorization timed out".into());
        }
    }

    // 8. Close WebView
    let _ = auth_window.close();

    // Audit and purge residual OAuth webview localstorage (TASK-112)
    if let Ok(profile_dir) = app.path().app_local_data_dir() {
        let _ = crate::crypto::audit_and_purge_webview_localstorage(&profile_dir);
        let _ = crate::crypto::ensure_secure_profile_permissions(&profile_dir);
    }

    let code = match code_opt {
        Some(c) => c,
        None => return Err("No authorization code received".into()),
    };

    // 9. POST reqwest → accounts.spotify.com/api/token
    let http_client = reqwest::Client::new();
    let params = [
        ("client_id", client_id),
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri.to_string()),
        ("code_verifier", code_verifier),
    ];

    let token_resp = http_client
        .post("https://accounts.spotify.com/api/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {}", e))?;

    if !token_resp.status().is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        return Err(format!("Token exchange failed: {}", body));
    }

    let token_data: serde_json::Value = token_resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    let access_token = token_data["access_token"]
        .as_str()
        .ok_or("Missing access_token")?
        .to_string();

    let refresh_token = token_data["refresh_token"]
        .as_str()
        .ok_or("Missing refresh_token")?
        .to_string();

    let expires_in = token_data["expires_in"].as_i64().unwrap_or(3600);

    // A4: no unwrap — a clock before UNIX_EPOCH degrades to 0 instead of panicking.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let expires_at = now + expires_in;

    tracing::info!(
        "Spotify PKCE auth: token obtained (expires_at={})",
        expires_at
    );

    // Get user profile via Spotify API
    let mut display_name = String::from("Spotify User");
    let mut user_id: Option<String> = None;
    let mut email: Option<String> = None;

    match http_client
        .get("https://api.spotify.com/v1/me")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(profile) = resp.json::<serde_json::Value>().await {
                display_name = profile["display_name"]
                    .as_str()
                    .or(profile["id"].as_str())
                    .unwrap_or("Spotify User")
                    .to_string();
                user_id = profile["id"].as_str().map(|s| s.to_string());
                email = profile["email"].as_str().map(|s| s.to_string());
                tracing::info!(
                    "Spotify PKCE auth: authenticated as {} ({})",
                    display_name,
                    user_id.as_deref().unwrap_or("?")
                );
            }
        }
        Ok(resp) => {
            tracing::warn!("Spotify profile fetch HTTP {}", resp.status());
        }
        Err(e) => {
            tracing::warn!("Spotify profile fetch error: {}", e);
        }
    }

    // Save credentials to database
    let service_row: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM services WHERE name = 'spotify'")
            .fetch_optional(&state.db)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

    let service_id = service_row
        .ok_or("Spotify service not found in database")?
        .0;

    let credentials = serde_json::json!({
        "token_type": "Bearer",
        "access_token": access_token,
        "refresh_token": refresh_token,
        "expires_at": expires_at,
    });

    let encrypted = crate::crypto::encrypt(&credentials.to_string())?;

    // Multi-cuenta: mismo upsert por identidad que el resto de servicios.
    // El UPDATE masivo anterior (`WHERE service_id = ?`) escribía el email en
    // TODAS las filas spotify: con dos cuentas violaba UNIQUE(service_id, email)
    // y fallaba el login entero, y con una sola le pisaba el email.
    let final_display_name = if display_name.is_empty() {
        user_id
            .clone()
            .unwrap_or_else(|| "Spotify User".to_string())
    } else {
        display_name.clone()
    };

    let account_id = upsert_service_account(
        &state.db,
        service_id,
        &final_display_name,
        email.as_deref(),
        user_id.as_deref(),
        &encrypted,
    )
    .await?;

    tracing::info!(
        "Spotify PKCE auth: saved account {} for {}",
        account_id,
        final_display_name
    );

    // Auto-retry downloads stuck in requires_auth / failed for Spotify
    let re_queued = sqlx::query(
        r#"
        UPDATE download_queue
        SET status = 'queued',
            last_error = NULL,
            error_message = NULL,
            retry_count = 0,
            started_at = NULL,
            completed_at = NULL
        WHERE status IN ('requires_auth', 'failed')
          AND (LOWER(service_name) = 'spotify' OR service_name IS NULL)
        "#,
    )
    .execute(&state.db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    if re_queued > 0 {
        tracing::info!(
            "[Auth] Automatically re-queued {} failed downloads for spotify",
            re_queued
        );
    }

    crate::commands::emit_auth_state_updated("spotify", "connected", Some(&final_display_name));

    Ok(AuthResult {
        success: true,
        data: Some(serde_json::json!({
            "service": "spotify",
            "display_name": final_display_name,
            "email": email,
        })),
        error: None,
    })
}

#[cfg(test)]
mod auth_security_tests {
    use super::*;

    #[test]
    fn test_qobuz_browser_session_marker_is_not_an_account_identity() {
        let first = serde_json::json!({"username": "first@qobuz.test"});
        let second = serde_json::json!({"username": "second@qobuz.test"});
        assert_eq!(
            login_external_user_id("qobuz", &first, Some("browser_session")),
            Some("first@qobuz.test".to_string())
        );
        assert_eq!(
            login_external_user_id("qobuz", &second, Some("browser_session")),
            Some("second@qobuz.test".to_string())
        );
        assert_eq!(
            login_external_user_id("qobuz", &first, Some("stable-id")),
            Some("stable-id".to_string())
        );
        assert_eq!(
            login_external_user_id("qobuz", &serde_json::json!({}), Some("browser_session")),
            None
        );
        assert_eq!(
            login_external_user_id("tidal", &first, Some("tidal-id")),
            Some("tidal-id".to_string())
        );
    }

    #[test]
    fn qobuz_fallback_secrets_require_a_matching_login_identity() {
        assert!(qobuz_cache_matches_login(
            Some("alice@example.org"),
            Some("ALICE@example.org")
        ));
        assert!(!qobuz_cache_matches_login(
            Some("bob@example.org"),
            Some("alice@example.org")
        ));
        assert!(!qobuz_cache_matches_login(Some("bob@example.org"), None));
        assert!(!qobuz_cache_matches_login(None, Some("alice@example.org")));
    }

    #[test]
    fn soundcloud_zero_is_a_placeholder_not_an_identity() {
        assert_eq!(
            login_external_user_id("soundcloud", &serde_json::json!({}), Some("0")),
            None
        );
        assert_eq!(
            login_external_user_id("soundcloud", &serde_json::json!({}), Some("42")),
            Some("42".to_string())
        );
    }

    #[test]
    fn test_auth_response_and_logs_never_leak_tokens() {
        let sensitive_json = r#"{"success": true, "data": {"access_token": "secret_access_tok_12345", "refresh_token": "secret_refresh_tok_67890", "client_secret": "super_secret_client", "password": "my_secret_pass"}}"#;
        let redacted = redact_auth_payload(sensitive_json);

        assert!(
            !redacted.contains("secret_access_tok_12345"),
            "access_token was not redacted"
        );
        assert!(
            !redacted.contains("secret_refresh_tok_67890"),
            "refresh_token was not redacted"
        );
        assert!(
            !redacted.contains("super_secret_client"),
            "client_secret was not redacted"
        );
        assert!(
            !redacted.contains("my_secret_pass"),
            "password was not redacted"
        );
        assert!(redacted.contains(r#""access_token": "[REDACTED]""#));
        assert!(redacted.contains(r#""refresh_token": "[REDACTED]""#));
        assert!(redacted.contains(r#""client_secret": "[REDACTED]""#));
        assert!(redacted.contains(r#""password": "[REDACTED]""#));
    }
}

#[cfg(test)]
mod session_status_tri_state_tests {
    use super::*;

    fn ok_connected() -> AuthResult {
        AuthResult {
            success: true,
            data: Some(serde_json::json!({ "connected": true })),
            error: None,
        }
    }

    fn ok_disconnected() -> AuthResult {
        AuthResult {
            success: true,
            data: Some(serde_json::json!({ "connected": false })),
            error: None,
        }
    }

    /// A-3: una sesión caducada NO puede aparecer como conectada. Antes
    /// `connected` era `true` incondicional, así que este aserto falla contra
    /// la versión previa.
    #[test]
    fn a_rejected_session_is_never_reported_as_connected() {
        let s = session_status_from_probe(Ok(ok_disconnected()));
        assert!(
            !s.connected,
            "una sesión rechazada no puede venir marcada como conectada"
        );
        assert!(!s.valid);
    }

    /// El camino feliz no se toca.
    #[test]
    fn a_valid_session_stays_connected_and_valid() {
        let s = session_status_from_probe(Ok(ok_connected()));
        assert!(s.connected);
        assert!(s.valid);
        assert_eq!(s.message, "Session valid");
    }

    /// A-3: un subproceso que no arrancó NO prueba que la sesión esté
    /// caducada. Antes `Err(e)` producía exactamente la misma salida que un
    /// rechazo real ("Check failed: ..." con valid:false y connected:true),
    /// así que la UI mandaba al usuario a re-loguear sin motivo.
    #[test]
    fn a_failed_subprocess_is_unverified_not_rejected() {
        let s = session_status_from_probe(Err("python3: not found".to_string()));
        assert!(!s.valid, "no se pudo comprobar, luego no es 'válida'");
        assert!(
            !s.connected,
            "no se comprobó nada, luego no se afirma conexión"
        );
        assert!(
            s.message.contains("Could not verify"),
            "el mensaje debe distinguir 'no verificable' de 'caducada': {}",
            s.message
        );
        assert!(
            !s.message.contains("expired") && !s.message.contains("invalid"),
            "no debe insinuar caducidad cuando solo falló la comprobación: {}",
            s.message
        );
    }

    /// Un error del propio puente (que sí consultaró al proveedor) se propaga
    /// tal cual: aquí sí hay información de la sesión.
    #[test]
    fn a_bridge_error_is_propagated_verbatim() {
        let s = session_status_from_probe(Ok(AuthResult {
            success: false,
            data: None,
            error: Some("Token expired".to_string()),
        }));
        assert_eq!(s.message, "Token expired");
        assert!(!s.valid);
        assert!(!s.connected);
    }

    /// La ausencia de `connected` en la respuesta no puede volverse `true`.
    #[test]
    fn a_missing_connected_field_does_not_mean_connected() {
        let s = session_status_from_probe(Ok(AuthResult {
            success: true,
            data: Some(serde_json::json!({ "otro_campo": 1 })),
            error: None,
        }));
        assert!(!s.connected);
        assert!(!s.valid);
    }
}
