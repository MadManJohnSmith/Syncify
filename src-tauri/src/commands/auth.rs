#[allow(unused_imports)]
use super::*;

// Auth Commands - submodule of crate::commands
// 
// Python auth bridge, session validation


// ==============================================
// AUTH CONCURRENCY LOCK
// ==============================================

static AUTH_IN_PROGRESS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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

/// Start auth flow for a service (spawns Python subprocess)
#[tauri::command]
pub async fn start_auth(service: String, action: String) -> Result<AuthResult, String> {
    tracing::info!("start_auth: service={} action={}", service, action);

    let project_root = get_project_root();
    let python_cmd = get_python_executable();
    let script_path = project_root.join("scripts").join("auth_bridge.py");

    tracing::debug!(
        "Auth bridge: python={}, script={:?}, cwd={:?}",
        python_cmd,
        script_path,
        project_root
    );

    // Run auth_bridge.py
    let output = crate::cmd_utils::create_tokio_command(&python_cmd)
        .arg(&script_path)
        .arg(&service)
        .arg(&action)
        .current_dir(&project_root)
        .output()
        .await
        .map_err(|e| format!("Failed to run Python: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let redacted_stdout = redact_auth_payload(&stdout);
    tracing::info!("Auth subprocess execution finished (output redacted): {}", redacted_stdout);
    if !stderr.is_empty() {
        tracing::warn!("Auth stderr (redacted): {}", redact_auth_payload(&stderr));
    }

    if !output.status.success() || stdout.trim().is_empty() {
        let err_detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            format!("Python exited with code {:?}", output.status.code())
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
        Ok(result) => Ok(result),
        Err(e) => Err(format!(
            "Failed to parse auth result: {} (raw output: {})",
            e, redacted_stdout
        )),
    }
}


/// Get auth status for a service
#[tauri::command]
pub async fn get_auth_status(service: String) -> Result<AuthResult, String> {
    start_auth(service, "status".to_string()).await
}

/// Logout from a service
#[tauri::command]
pub async fn logout_service(
    service: String,
    state: State<'_, AppState>,
) -> Result<AuthResult, String> {
    if service == "spotify" {
        tracing::info!("Spotify native logout: cleaning up database");
        
        // Find Spotify service ID
        let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'spotify'")
            .fetch_one(&state.db)
            .await
            .map_err(|e| format!("Failed to find spotify service: {}", e))?;

        // Delete all Spotify accounts
        sqlx::query("DELETE FROM accounts WHERE service_id = ?")
            .bind(service_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("Failed to delete spotify accounts: {}", e))?;

        crate::commands::emit_auth_state_updated(&service, "logout", None);

        return Ok(AuthResult {
            success: true,
            data: None,
            error: None,
        });
    }

    let res = start_auth(service.clone(), "logout".to_string()).await;
    if let Ok(ref r) = res {
        if r.success {
            crate::commands::emit_auth_state_updated(&service, "logout", None);
        }
    }
    res
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

/// Load Qobuz fallback auth data from the canonical encrypted SQLite database (AES-256-GCM).
/// Returns (token, username/email, password) when available from an existing active account.
pub async fn load_qobuz_db_fallback_auth(
    db: &sqlx::SqlitePool,
) -> (Option<String>, Option<String>, Option<String>) {
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
pub async fn load_tidal_db_cached_token_expiry(db: &sqlx::SqlitePool) -> Option<f64> {
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
        obj.insert("expires_in".to_string(), serde_json::Value::from(expires_in));
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
        let (cache_token, cache_username, cache_password) = load_qobuz_db_fallback_auth(&state.db).await;

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
                obj.insert("user_auth_token".to_string(), serde_json::Value::String(t.clone()));
                obj.insert("auth_token".to_string(), serde_json::Value::String(t.clone()));
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

    // Step 5: Upsert account — preserve existing row to avoid CASCADE deleting library_entries/playlists
    let final_display_name = display_name
        .or(user_id.clone())
        .unwrap_or_else(|| format!("{} User", service));

    upsert_service_account(&state.db, service_id, &final_display_name, email.as_deref(), &encrypted).await?;

    tracing::info!("Saved {} account: {}", service, final_display_name);

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
        "#
    )
    .bind(&service)
    .execute(&state.db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    if re_queued > 0 {
        tracing::info!("[Auth] Automatically re-queued {} failed downloads for {}", re_queued, service);
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
            "requeued_downloads": re_queued,
        })),
        error: None,
    })
}

/// S185: Stable per-service account upsert used by login flows.
///
/// Guarantees the post-login invariant the download pipeline relies on — exactly
/// ONE clean active row for the service, selectable with
/// `WHERE is_active = 1 ORDER BY id DESC LIMIT 1` — while preserving every row so
/// CASCADE-linked library data survives re-login:
///   1. Target row = the service row matching the new email, else the newest row.
///   2. Target row gets the fresh encrypted credentials, clean flags, is_active = 1.
///   3. Every OTHER row of the service gets its stale invalidation flags cleared and
///      is deactivated, so no leftover poisoned row can shadow the fresh login.
///
/// The previous implementation ran one blanket `UPDATE … WHERE service_id = ?` that
/// (a) collided with the UNIQUE(service_id, email) schema constraint whenever two
/// rows existed for the service — failing the whole login with a SQL error — and
/// (b) left multiple active rows behind.
pub async fn upsert_service_account(
    db: &sqlx::SqlitePool,
    service_id: i64,
    display_name: &str,
    email: Option<&str>,
    encrypted_credentials: &str,
) -> Result<(), String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to begin account upsert: {}", e))?;

    // 1) Pick the target row: prefer matching email, else the newest row.
    let email_match: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT id FROM accounts
        WHERE service_id = ? AND email IS ?
        ORDER BY id DESC LIMIT 1
        "#,
    )
    .bind(service_id)
    .bind(email)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("Failed to look up account: {}", e))?;

    let newest_row: Option<i64> = if email_match.is_some() {
        None
    } else {
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
    };

    // 0 = no existing row for this service at all → INSERT path below.
    let target_id = email_match.or(newest_row).unwrap_or(0);

    if target_id == 0 {
        // Fresh connect: INSERT a clean, active row.
        sqlx::query(
            r#"
            INSERT INTO accounts (service_id, display_name, email, credentials_json, credentials_invalid, invalid_reason, last_auth_error, is_active, last_synced, created_at)
            VALUES (?, ?, ?, ?, 0, NULL, NULL, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(service_id)
        .bind(display_name)
        .bind(email)
        .bind(encrypted_credentials)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to save account: {}", e))?;
    } else {
        // 2) Activate and clean ONLY the target row (preserves its CASCADE-linked data).
        sqlx::query(
            r#"
            UPDATE accounts
            SET display_name = ?,
                email = ?,
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
        .bind(email)
        .bind(encrypted_credentials)
        .bind(target_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update account: {}", e))?;

        // 3) Clear stale flags on sibling rows and deactivate them.
        sqlx::query(
            r#"
            UPDATE accounts
            SET credentials_invalid = 0,
                invalid_reason = NULL,
                last_auth_error = NULL,
                is_active = 0
            WHERE service_id = ? AND id != ?
            "#,
        )
        .bind(service_id)
        .bind(target_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to clean sibling accounts: {}", e))?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit account upsert: {}", e))?;

    Ok(())
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

/// Validate all connected service sessions
#[tauri::command]
pub async fn validate_all_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<SessionStatus>, String> {
    tracing::info!("validate_all_sessions called");

    // Get all connected accounts
    let accounts: Vec<(i64, String, String)> = sqlx::query_as(
        r#"
        SELECT a.id, s.name, a.display_name 
        FROM accounts a 
        JOIN services s ON s.id = a.service_id 
        WHERE a.is_active = 1
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    let mut statuses = Vec::new();

    for (_account_id, service_name, display_name) in accounts {
        // Call Python bridge to check status
        let status_result = start_auth(service_name.clone(), "status".to_string()).await;

        let (valid, message) = match status_result {
            Ok(result) => {
                if result.success {
                    let connected = result
                        .data
                        .as_ref()
                        .and_then(|d| d.get("connected"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    if connected {
                        (true, "Session valid".to_string())
                    } else {
                        (false, "Session expired or invalid".to_string())
                    }
                } else {
                    (
                        false,
                        result.error.unwrap_or_else(|| "Unknown error".to_string()),
                    )
                }
            }
            Err(e) => (false, format!("Check failed: {}", e)),
        };

        statuses.push(SessionStatus {
            service: service_name,
            connected: true,
            valid,
            message,
            user_info: Some(display_name),
        });
    }

    tracing::info!(
        "Session validation complete: {} services checked",
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
    use tauri::Manager;
    use sha2::{Digest, Sha256};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use rand::{RngCore, rngs::OsRng};
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
    let code_verifier = URL_SAFE_NO_PAD.encode(&verifier_bytes);

    // 2. Calculate code_challenge (SHA256 of verifier, base64url)
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let challenge_bytes = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(&challenge_bytes);

    // 3. Generate CSRF state token (random 32 bytes base64url)
    let mut state_bytes = [0u8; 32];
    OsRng.fill_bytes(&mut state_bytes);
    let expected_state = URL_SAFE_NO_PAD.encode(&state_bytes);
    
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
            if n == 0 { continue; }
            let request = String::from_utf8_lossy(&buf[..n]);
            
            let (status, callback_result, response) = process_spotify_callback_request(&request, &expected_state);

            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;

            match callback_result {
                Ok(code) => {
                    tracing::info!("spotify_auth_webview: valid callback with matching state received");
                    code_opt = Some(code);
                    break;
                }
                Err(e) => {
                    tracing::warn!("spotify_auth_webview: invalid callback attempt: {:?} (HTTP {})", e, status);
                }
            }
        }
        Ok::<(), std::io::Error>(())
    }).await {
        Ok(Ok(_)) => {},
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

    tracing::info!("Spotify PKCE auth: token obtained (expires_at={})", expires_at);

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
                tracing::info!("Spotify PKCE auth: authenticated as {} ({})",
                    display_name, user_id.as_deref().unwrap_or("?"));
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

    // Upsert: UPDATE first to preserve CASCADE data, INSERT if no row
    let final_display_name = if display_name.is_empty() {
        user_id
            .clone()
            .unwrap_or_else(|| "Spotify User".to_string())
    } else {
        display_name.clone()
    };

    let update_result = sqlx::query(
        r#"
        UPDATE accounts
        SET display_name = ?,
            email = ?,
            credentials_json = ?,
            credentials_invalid = 0,
            invalid_reason = NULL,
            last_auth_error = NULL,
            is_active = 1,
            last_synced = CURRENT_TIMESTAMP
        WHERE service_id = ?
        "#,
    )
    .bind(&final_display_name)
    .bind(&email)
    .bind(&encrypted)
    .bind(service_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to update account: {}", e))?;

    if update_result.rows_affected() == 0 {
        sqlx::query(
            r#"
            INSERT INTO accounts (service_id, display_name, email, credentials_json,
                                  credentials_invalid, invalid_reason, last_auth_error, is_active, last_synced, created_at)
            VALUES (?, ?, ?, ?, 0, NULL, NULL, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(service_id)
        .bind(&final_display_name)
        .bind(&email)
        .bind(&encrypted)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to save account: {}", e))?;
    }

    tracing::info!("Spotify PKCE auth: saved account for {}", final_display_name);

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
        "#
    )
    .execute(&state.db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);

    if re_queued > 0 {
        tracing::info!("[Auth] Automatically re-queued {} failed downloads for spotify", re_queued);
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
    fn test_auth_response_and_logs_never_leak_tokens() {
        let sensitive_json = r#"{"success": true, "data": {"access_token": "secret_access_tok_12345", "refresh_token": "secret_refresh_tok_67890", "client_secret": "super_secret_client", "password": "my_secret_pass"}}"#;
        let redacted = redact_auth_payload(sensitive_json);

        assert!(!redacted.contains("secret_access_tok_12345"), "access_token was not redacted");
        assert!(!redacted.contains("secret_refresh_tok_67890"), "refresh_token was not redacted");
        assert!(!redacted.contains("super_secret_client"), "client_secret was not redacted");
        assert!(!redacted.contains("my_secret_pass"), "password was not redacted");
        assert!(redacted.contains(r#""access_token": "[REDACTED]""#));
        assert!(redacted.contains(r#""refresh_token": "[REDACTED]""#));
        assert!(redacted.contains(r#""client_secret": "[REDACTED]""#));
        assert!(redacted.contains(r#""password": "[REDACTED]""#));
    }
}
