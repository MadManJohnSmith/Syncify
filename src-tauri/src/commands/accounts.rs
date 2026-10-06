#[allow(unused_imports)]
use super::*;

// Accounts Commands - submodule of crate::commands
//
// Account management, service connections

// ==============================================
// ACCOUNT MANAGEMENT COMMANDS
// ==============================================

use crate::crypto;
use std::sync::OnceLock;
use tauri::Emitter;

static GLOBAL_APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Store global AppHandle for auth event emission
pub fn set_global_app_handle(handle: tauri::AppHandle) {
    let _ = GLOBAL_APP_HANDLE.set(handle);
}

/// Retrieve global AppHandle if registered
pub fn get_global_app_handle() -> Option<&'static tauri::AppHandle> {
    GLOBAL_APP_HANDLE.get()
}

/// Helper to emit auth state change event across the application
pub fn emit_auth_state_updated(service: &str, action: &str, details: Option<&str>) {
    emit_auth_state_updated_for_account(service, action, None, details);
}

/// Same channel as [`emit_auth_state_updated`] but able to name the ACCOUNT the
/// transition happened on.
///
/// Multi-cuenta: `set_active_account` and `logout_service` act on one row, and
/// "conectado" is ambiguous with several accounts of the same service, so the
/// payload carries `account_id`. Kept as a separate helper (and not as an extra
/// parameter) so the dozens of existing `emit_auth_state_updated` call sites —
/// which have no account at hand — keep compiling unchanged.
pub fn emit_auth_state_updated_for_account(
    service: &str,
    action: &str,
    account_id: Option<i64>,
    details: Option<&str>,
) {
    if let Some(app) = get_global_app_handle() {
        let mut payload = serde_json::json!({
            "service": service,
            "action": action,
        });
        if let Some(id) = account_id {
            payload["account_id"] = serde_json::Value::from(id);
        }
        if let Some(d) = details {
            payload["details"] = serde_json::Value::String(d.to_string());
        }
        let _ = app.emit("auth-state-updated", &payload);
    }
}

/// Service info for frontend
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ServiceInfo {
    pub id: i64,
    pub name: String,
    pub supports_download: i64,
    pub max_quality: Option<String>,
}

/// Account info for frontend (credentials excluded)
#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountInfo {
    pub id: i64,
    pub service_id: i64,
    pub service_name: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    pub last_synced: Option<String>,
    pub created_at: Option<String>,
    pub credentials_invalid: bool,
    pub invalid_reason: Option<String>,
    pub last_auth_error: Option<String>,
}

/// Get all supported services
#[tauri::command]
pub async fn get_services(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let services = sqlx::query_as::<_, ServiceInfo>(
        "SELECT id, name, supports_download, max_quality FROM services ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(services)
}

/// Get all connected accounts (without credentials)
#[tauri::command]
pub async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<AccountInfo>, String> {
    let rows: Vec<(i64, i64, String, Option<String>, Option<String>, i64, Option<String>, Option<String>, i64, Option<String>, Option<String>)> =
        sqlx::query_as(
            r#"SELECT a.id, a.service_id, s.name, a.display_name, a.email, a.is_active, a.last_synced, a.created_at,
                      IFNULL(a.credentials_invalid, 0) as credentials_invalid, a.invalid_reason, a.last_auth_error
               FROM accounts a
               JOIN services s ON s.id = a.service_id
               ORDER BY s.name, a.created_at"#
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    let accounts = rows
        .into_iter()
        .map(
            |(
                id,
                service_id,
                service_name,
                display_name,
                email,
                is_active,
                last_synced,
                created_at,
                credentials_invalid,
                invalid_reason,
                last_auth_error,
            )| {
                AccountInfo {
                    id,
                    service_id,
                    service_name,
                    display_name,
                    email,
                    is_active: is_active != 0,
                    last_synced,
                    created_at,
                    credentials_invalid: credentials_invalid != 0,
                    invalid_reason,
                    last_auth_error,
                }
            },
        )
        .collect();

    Ok(accounts)
}

/// Add a new account with encrypted credentials
///
/// Multi-cuenta: la fila se inserta SIEMPRE con `is_active = 1` (import y
/// restore dependen de eso), pero dentro de una transacción que desactiva a las
/// hermanas del mismo servicio. Sin ese paso, añadir una segunda cuenta
/// dejaba dos filas activas y el backend resolvía 'la cuenta activa' de forma
/// arbitraria para el servicio.
#[tauri::command]
pub async fn add_account(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    service_id: i64,
    credentials_json: String,
    display_name: Option<String>,
    email: Option<String>,
) -> Result<i64, String> {
    // Encrypt credentials before storage
    let encrypted = crypto::encrypt(&credentials_json)?;

    let account_id = insert_account_exclusive(
        &state.db,
        service_id,
        &encrypted,
        display_name.as_deref(),
        email.as_deref(),
    )
    .await?;

    tracing::info!(
        "Added account {} for service_id={} (siblings deactivated)",
        account_id,
        service_id
    );

    let _ = app.emit(
        "auth-state-updated",
        serde_json::json!({
            "service_id": service_id,
            "action": "added",
            "account_id": account_id,
        }),
    );

    Ok(account_id)
}

/// DB half of [`add_account`]: insert the row active AND deactivate its siblings
/// in one transaction. Separate from the command so the invariant is testable
/// without a Tauri AppHandle. Returns the new account id.
pub async fn insert_account_exclusive(
    db: &sqlx::SqlitePool,
    service_id: i64,
    encrypted_credentials: &str,
    display_name: Option<&str>,
    email: Option<&str>,
) -> Result<i64, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| e.to_string())?;

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active, created_at)
           VALUES (?, ?, ?, ?, 1, CURRENT_TIMESTAMP) RETURNING id"#,
    )
    .bind(service_id)
    .bind(encrypted_credentials)
    .bind(display_name)
    .bind(email)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    // Exclusividad: la cuenta recién añadida es la activa del servicio.
    sqlx::query("UPDATE accounts SET is_active = 0 WHERE service_id = ? AND id != ?")
        .bind(service_id)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(account_id)
}

/// Remove an account
#[tauri::command]
pub async fn remove_account(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<(), String> {
    sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(account_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Removed account id={}", account_id);

    let _ = app.emit(
        "auth-state-updated",
        serde_json::json!({
            "action": "removed",
            "account_id": account_id,
        }),
    );

    Ok(())
}

/// Get decrypted credentials for an account (internal Rust backend use only; not exposed via IPC)
#[allow(dead_code)] // Cubierta por `tests/account_credentials_ipc_hygiene_test.rs` (el IPC no la expone).
pub async fn get_internal_account_credentials(
    pool: &sqlx::SqlitePool,
    account_id: i64,
) -> Result<String, String> {
    let encrypted: Option<(Option<String>,)> =
        sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    match encrypted {
        Some((Some(creds),)) if !creds.trim().is_empty() => match crypto::decrypt(&creds) {
            Ok(decrypted) => Ok(decrypted),
            Err(e)
                if e.contains("Decryption error")
                    || e.contains("aead")
                    || e.contains("Base64 decode error")
                    || e.contains("too short") =>
            {
                tracing::error!(
                    "Decryption error for account {}: {}. Clearing credentials.",
                    account_id,
                    e
                );
                let _ = sqlx::query("UPDATE accounts SET credentials_json = NULL WHERE id = ?")
                    .bind(account_id)
                    .execute(pool)
                    .await;
                Err("Service credentials expired. Please reconnect your account.".to_string())
            }
            Err(e) => Err(e),
        },
        Some(_) => Err("Credentials missing for account".into()),
        None => Err("Account not found".into()),
    }
}

/// Update account's last synced time
#[tauri::command]
pub async fn update_account_sync_time(
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<(), String> {
    sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(account_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Purge accounts with irrecoverable credentials (aeadError).
///
/// Called at startup and available as a Tauri command.
/// When switching machines, the OS Keychain key changes, making all
/// credentials encrypted on the old machine undecryptable.
/// This function detects and removes those stale accounts so the UI
/// correctly shows services as "disconnected" instead of "connected"
/// with aeadError on every import attempt.
///
/// Returns: (purged_count, vec of purged service names)
#[tauri::command]
pub async fn purge_stale_credentials(
    state: State<'_, AppState>,
) -> Result<(u32, Vec<String>), String> {
    tracing::info!("purge_stale_credentials: checking for irrecoverable credentials");

    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        r#"SELECT a.id, s.name, a.credentials_json
           FROM accounts a
           JOIN services s ON s.id = a.service_id
           WHERE a.credentials_json IS NOT NULL"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    let mut purged_count: u32 = 0;
    let mut purged_services: Vec<String> = Vec::new();

    for (account_id, service_name, ciphertext) in &rows {
        match crypto::decrypt(ciphertext) {
            Ok(_) => {
                // Credentials are valid — this account's encryption key matches
                tracing::debug!("Account {} ({}) credentials OK", account_id, service_name);
            }
            Err(e) if e.contains("Decryption error") || e.contains("aead") => {
                // Key mismatch — credentials are from a different machine's keychain
                tracing::warn!(
                    "Purging stale account {} ({}) — credentials irrecoverable: {}",
                    account_id,
                    service_name,
                    e
                );
                let _ = sqlx::query("UPDATE accounts SET credentials_invalid = 1 WHERE id = ?")
                    .bind(account_id)
                    .execute(&state.db)
                    .await;
                purged_count += 1;
                purged_services.push(service_name.clone());
            }
            Err(e) => {
                // Other error (Base64, UTF-8, etc.) — also stale, purge
                tracing::warn!(
                    "Purging account {} ({}) — credential error: {}",
                    account_id,
                    service_name,
                    e
                );
                let _ = sqlx::query("UPDATE accounts SET credentials_invalid = 1 WHERE id = ?")
                    .bind(account_id)
                    .execute(&state.db)
                    .await;
                purged_count += 1;
                purged_services.push(service_name.clone());
            }
        }
    }

    if purged_count > 0 {
        tracing::info!(
            "Purged {} stale accounts: {:?}. Re-authentication required.",
            purged_count,
            purged_services
        );
        emit_auth_state_updated("all", "stale_credentials_purged", None);
    } else {
        tracing::info!("All account credentials are valid — no purge needed");
    }

    Ok((purged_count, purged_services))
}

/// Toggle account active status
///
/// Multi-cuenta:
///   * `is_active = true` delega en [`set_active_account`]: activar una cuenta
///     es un CAMBIO DE CUENTA y, dentro de la misma transacción, desactiva a sus
///     hermanas del servicio. Activar sin ese paso es lo que dejaba servicios
///     con dos o más filas activas.
///   * `is_active = false` conserva el camino anterior y permite quedar con cero
///     cuentas activas: es la forma de "silenciar" un servicio. Si se desactiva la
///     ÚNICA cuenta activa del servicio, el servicio se queda sin cuenta que
///     usar hasta que se active alguna.
///
/// La firma no cambia: la UI existente (ServiceCard, SettingsServices) sigue
/// llamando a este comando.
#[tauri::command]
pub async fn toggle_account_active<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    account_id: i64,
    is_active: bool,
) -> Result<(), String> {
    if is_active {
        return set_active_account(app, state, account_id).await;
    }

    sqlx::query("UPDATE accounts SET is_active = ? WHERE id = ?")
        .bind(0)
        .bind(account_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit(
        "auth-state-updated",
        serde_json::json!({
            "action": "toggled",
            "account_id": account_id,
            "is_active": is_active,
        }),
    );

    Ok(())
}

/// Make `account_id` THE active account of its service ("usar esta cuenta").
///
/// Multi-cuenta: activa la fila y desactiva a sus hermanas del mismo servicio en
/// UNA transacción, que es la forma de mantener el invariante 'exactamente una
/// activa por servicio' que asumen todos los resolvedores del backend
/// (`is_active = 1 ORDER BY id DESC LIMIT 1`). Desactivar a las hermanas antes
/// que activar la nueva dejaría una ventana en la que el servicio no tiene
/// cuenta activa.
///
/// Emite `auth-state-updated` con action "activated" y `account_id` mediante
/// `app.emit` directo (mismo canal que add_account/toggle_account_active):
/// [`emit_auth_state_updated`] tiene firma fija (service, action, details) y no
/// puede llevar la cuenta.
///
/// Nota de refresco: el ÚNICO oyente de este evento en la UI hoy es
/// `AccountsView.vue`; App.vue y StatusBar no lo escuchan, así que las demás
/// vistas verán el cambio cuando hagan su propio fetchData/polling.
#[tauri::command]
pub async fn set_active_account<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<(), String> {
    let service_id = activate_account_exclusive(&state.db, account_id).await?;

    // The payload keeps the `service` key every other emission on this channel
    // carries (see `emit_auth_state_updated`), so a future listener can filter by
    // service without having to map account_id back to it.
    let service_name: String = sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
        .bind(service_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    tracing::info!(
        "Activated account {} (service_id={}); siblings deactivated",
        account_id,
        service_id
    );

    let _ = app.emit(
        "auth-state-updated",
        serde_json::json!({
            "service": service_name,
            "action": "activated",
            "account_id": account_id,
            "is_active": true,
        }),
    );

    Ok(())
}

/// DB half of [`set_active_account`] (and of `toggle_account_active(true)`).
///
/// Separate from the command so the exclusivity invariant is testable without a
/// Tauri AppHandle. Returns the `service_id` of the activated account.
pub async fn activate_account_exclusive(
    db: &sqlx::SqlitePool,
    account_id: i64,
) -> Result<i64, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to begin account activation: {}", e))?;

    let service_id: i64 = sqlx::query_scalar("SELECT service_id FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Account {} not found", account_id))?;

    sqlx::query("UPDATE accounts SET is_active = 1 WHERE id = ?")
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("UPDATE accounts SET is_active = 0 WHERE service_id = ? AND id != ?")
        .bind(service_id)
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit account activation: {}", e))?;

    Ok(service_id)
}

/// Query real service authentication status for an account/service
pub async fn perform_get_service_auth_status(
    db: &sqlx::SqlitePool,
    service_name: &str,
    account_id: Option<i64>,
) -> Result<ServiceAuthStatus, String> {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let now_iso = chrono::Utc::now().to_rfc3339();

    let account_row: Option<(
        i64,
        i64,
        String,
        Option<String>,
        Option<String>,
        i64,
        Option<String>,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = if let Some(aid) = account_id {
        // Membership check: an account_id of ANOTHER service must not answer for
        // this one. Without the `AND s.name = ?` filter the caller would get the
        // row's own `svc_name` back — a status for a service it never asked
        // about. `LOWER()` on both sides matches how every other resolver in the
        // codebase compares service names (`load_service_credentials`).
        sqlx::query_as(
            r#"SELECT a.id, a.service_id, s.name, a.display_name, a.email, a.is_active, a.credentials_json,
                      IFNULL(a.credentials_invalid, 0) as credentials_invalid, a.invalid_reason, a.last_auth_error,
                      a.last_auth_error_at
               FROM accounts a
               JOIN services s ON s.id = a.service_id
               WHERE a.id = ? AND LOWER(s.name) = LOWER(?)"#
        )
        .bind(aid)
        .bind(service_name)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(
            r#"SELECT a.id, a.service_id, s.name, a.display_name, a.email, a.is_active, a.credentials_json,
                      IFNULL(a.credentials_invalid, 0) as credentials_invalid, a.invalid_reason, a.last_auth_error,
                      a.last_auth_error_at
               FROM accounts a
               JOIN services s ON s.id = a.service_id
               WHERE s.name = ? AND a.is_active = 1
               ORDER BY a.id DESC LIMIT 1"#
        )
        .bind(service_name)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
    };

    let (
        id,
        _svc_id,
        svc_name,
        display_name,
        email,
        is_active,
        creds_json,
        credentials_invalid,
        invalid_reason,
        last_auth_err,
        last_auth_err_at,
    ) = match account_row {
        Some(row) => row,
        None => {
            return Ok(ServiceAuthStatus {
                service: service_name.to_string(),
                account_id: None,
                is_active: false,
                status: "missing".to_string(),
                is_authenticated: false,
                credentials_valid: false,
                credentials_expired: false,
                credentials_invalid: false,
                sync_available: false,
                download_entitled: false,
                download_auth_failed: false,
                display_name: None,
                email: None,
                error_message: Some(format!("No account found for {}", service_name)),
                last_auth_error: None,
                last_auth_error_at: None,
                last_checked: Some(now_iso),
            });
        }
    };

    // Multi-cuenta: el corte por `is_active == 0` solo aplica al camino SIN id.
    // Ese camino solo devuelve filas activas (su consulta filtra `is_active = 1`),
    // así que el corte era dead code allí; en la rama por-id, en cambio,
    // truncaría el diagnóstico de una cuenta ELEGIDA desactivada: el usuario
    // pidió el estado de esa fila concreta y merece el estado real de sus
    // credenciales, no un "vuelve a activarla" que no explica nada.
    if account_id.is_none() && is_active == 0 {
        return Ok(ServiceAuthStatus {
            service: svc_name,
            account_id: Some(id),
            is_active: is_active != 0,
            status: "requires_auth".to_string(),
            is_authenticated: false,
            credentials_valid: false,
            credentials_expired: false,
            credentials_invalid: false,
            sync_available: false,
            download_entitled: false,
            download_auth_failed: false,
            display_name,
            email,
            error_message: Some("Account is disabled / inactive. Re-activate to use.".to_string()),
            last_auth_error: last_auth_err,
            last_auth_error_at: last_auth_err_at,
            last_checked: Some(now_iso),
        });
    }

    if credentials_invalid != 0 {
        return Ok(ServiceAuthStatus {
            service: svc_name,
            account_id: Some(id),
            is_active: is_active != 0,
            status: "requires_auth".to_string(),
            is_authenticated: false,
            credentials_valid: false,
            credentials_expired: false,
            credentials_invalid: true,
            sync_available: false,
            download_entitled: false,
            download_auth_failed: false,
            display_name,
            email,
            error_message: invalid_reason.or(Some(
                "Account credentials marked invalid. Please re-authenticate.".to_string(),
            )),
            last_auth_error: last_auth_err,
            last_auth_error_at: last_auth_err_at,
            last_checked: Some(now_iso),
        });
    }

    let ciphertext = match creds_json {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            return Ok(ServiceAuthStatus {
                service: svc_name,
                account_id: Some(id),
                is_active: is_active != 0,
                status: "requires_auth".to_string(),
                is_authenticated: false,
                credentials_valid: false,
                credentials_expired: false,
                credentials_invalid: false,
                sync_available: false,
                download_entitled: false,
                download_auth_failed: false,
                display_name,
                email,
                error_message: Some("Missing credentials. Please re-authenticate.".to_string()),
                last_auth_error: last_auth_err,
                last_auth_error_at: last_auth_err_at,
                last_checked: Some(now_iso),
            });
        }
    };

    let decrypted = match crypto::decrypt(&ciphertext) {
        Ok(dec) => dec,
        Err(e) => {
            return Ok(ServiceAuthStatus {
                service: svc_name,
                account_id: Some(id),
                is_active: is_active != 0,
                status: "requires_auth".to_string(),
                is_authenticated: false,
                credentials_valid: false,
                credentials_expired: false,
                credentials_invalid: true,
                sync_available: false,
                download_entitled: false,
                download_auth_failed: false,
                display_name,
                email,
                error_message: Some(format!(
                    "Decryption error: {}. Please reconnect your account.",
                    e
                )),
                last_auth_error: last_auth_err,
                last_auth_error_at: last_auth_err_at,
                last_checked: Some(now_iso),
            });
        }
    };

    let creds: serde_json::Value = match serde_json::from_str(&decrypted) {
        Ok(v) => v,
        Err(e) => {
            return Ok(ServiceAuthStatus {
                service: svc_name,
                account_id: Some(id),
                is_active: is_active != 0,
                status: "error".to_string(),
                is_authenticated: false,
                credentials_valid: false,
                credentials_expired: false,
                credentials_invalid: true,
                sync_available: false,
                download_entitled: false,
                download_auth_failed: false,
                display_name,
                email,
                error_message: Some(format!("Malformed credentials payload: {}", e)),
                last_auth_error: last_auth_err,
                last_auth_error_at: last_auth_err_at,
                last_checked: Some(now_iso),
            });
        }
    };

    let has_download_err = last_auth_err
        .as_ref()
        .map(|e| {
            e.contains("entitlement")
                || e.contains("paywall")
                || e.contains("quality")
                || e.contains("stream")
        })
        .unwrap_or(false);

    match svc_name.to_lowercase().as_str() {
        "qobuz" => {
            let token = creds["user_auth_token"]
                .as_str()
                .or_else(|| creds["auth_token"].as_str())
                .or_else(|| creds["access_token"].as_str());

            match token {
                Some(tok) if !tok.is_empty() && tok != "browser_cookies" => {
                    if let Some(exp) = creds["expires_at"].as_i64() {
                        if exp > 0 && now_secs >= exp {
                            return Ok(ServiceAuthStatus {
                                service: svc_name,
                                account_id: Some(id),
                                is_active: is_active != 0,
                                status: "expired".to_string(),
                                is_authenticated: false,
                                credentials_valid: false,
                                credentials_expired: true,
                                credentials_invalid: false,
                                sync_available: false,
                                download_entitled: false,
                                download_auth_failed: false,
                                display_name,
                                email,
                                error_message: Some(
                                    "Qobuz session token expired. Please log in again.".to_string(),
                                ),
                                last_auth_error: last_auth_err,
                                last_auth_error_at: last_auth_err_at,
                                last_checked: Some(now_iso),
                            });
                        }
                    }
                    Ok(ServiceAuthStatus {
                        service: svc_name,
                        account_id: Some(id),
                        is_active: is_active != 0,
                        status: "connected_valid".to_string(),
                        is_authenticated: true,
                        credentials_valid: true,
                        credentials_expired: false,
                        credentials_invalid: false,
                        sync_available: true,
                        download_entitled: true,
                        download_auth_failed: has_download_err,
                        display_name,
                        email,
                        error_message: None,
                        last_auth_error: last_auth_err,
                        last_auth_error_at: last_auth_err_at,
                        last_checked: Some(now_iso),
                    })
                }
                _ => {
                    let has_user_pass = creds["username"].as_str().is_some()
                        && creds["password"].as_str().is_some();
                    if has_user_pass {
                        Ok(ServiceAuthStatus {
                            service: svc_name,
                            account_id: Some(id),
                            is_active: is_active != 0,
                            status: "connected_valid".to_string(),
                            is_authenticated: true,
                            credentials_valid: true,
                            credentials_expired: false,
                            credentials_invalid: false,
                            sync_available: true,
                            download_entitled: true,
                            download_auth_failed: has_download_err,
                            display_name,
                            email,
                            error_message: None,
                            last_auth_error: last_auth_err,
                            last_auth_error_at: last_auth_err_at,
                            last_checked: Some(now_iso),
                        })
                    } else {
                        Ok(ServiceAuthStatus {
                            service: svc_name,
                            account_id: Some(id),
                            is_active: is_active != 0,
                            status: "requires_auth".to_string(),
                            is_authenticated: false,
                            credentials_valid: false,
                            credentials_expired: false,
                            credentials_invalid: true,
                            sync_available: false,
                            download_entitled: false,
                            download_auth_failed: false,
                            display_name,
                            email,
                            error_message: Some("RequiresAuth: Qobuz user auth token missing. Please log in to Qobuz.".to_string()),
                            last_auth_error: last_auth_err,
                            last_auth_error_at: last_auth_err_at,
                            last_checked: Some(now_iso),
                        })
                    }
                }
            }
        }
        "spotify" => {
            let access_token = creds["access_token"].as_str();
            let refresh_token = creds["refresh_token"].as_str();
            if access_token.is_none() && refresh_token.is_none() {
                return Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "requires_auth".to_string(),
                    is_authenticated: false,
                    credentials_valid: false,
                    credentials_expired: false,
                    credentials_invalid: true,
                    sync_available: false,
                    download_entitled: false,
                    download_auth_failed: false,
                    display_name,
                    email,
                    error_message: Some(
                        "Spotify tokens missing. Please reconnect to Spotify.".to_string(),
                    ),
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                });
            }
            if let Some(exp) = creds["expires_at"].as_i64() {
                if exp > 0 && now_secs >= exp && refresh_token.is_none() {
                    return Ok(ServiceAuthStatus {
                        service: svc_name,
                        account_id: Some(id),
                        is_active: is_active != 0,
                        status: "expired".to_string(),
                        is_authenticated: false,
                        credentials_valid: false,
                        credentials_expired: true,
                        credentials_invalid: false,
                        sync_available: false,
                        download_entitled: false,
                        download_auth_failed: false,
                        display_name,
                        email,
                        error_message: Some(
                            "Spotify access token expired and no refresh token available."
                                .to_string(),
                        ),
                        last_auth_error: last_auth_err,
                        last_auth_error_at: last_auth_err_at,
                        last_checked: Some(now_iso),
                    });
                }
            }
            Ok(ServiceAuthStatus {
                service: svc_name,
                account_id: Some(id),
                is_active: is_active != 0,
                status: "connected_valid".to_string(),
                is_authenticated: true,
                credentials_valid: true,
                credentials_expired: false,
                credentials_invalid: false,
                sync_available: true,
                download_entitled: true,
                download_auth_failed: has_download_err,
                display_name,
                email,
                error_message: None,
                last_auth_error: last_auth_err,
                last_auth_error_at: last_auth_err_at,
                last_checked: Some(now_iso),
            })
        }
        "tidal" => {
            let access_token = creds["access_token"].as_str();
            let refresh_token = creds["refresh_token"].as_str();
            if access_token.is_none() && refresh_token.is_none() {
                return Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "requires_auth".to_string(),
                    is_authenticated: false,
                    credentials_valid: false,
                    credentials_expired: false,
                    credentials_invalid: true,
                    sync_available: false,
                    download_entitled: false,
                    download_auth_failed: false,
                    display_name,
                    email,
                    error_message: Some(
                        "Tidal access token missing. Please reconnect to Tidal.".to_string(),
                    ),
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                });
            }
            if let Some(exp) = creds["expires_at"].as_i64() {
                if exp > 0 && now_secs >= exp && refresh_token.is_none() {
                    return Ok(ServiceAuthStatus {
                        service: svc_name,
                        account_id: Some(id),
                        is_active: is_active != 0,
                        status: "expired".to_string(),
                        is_authenticated: false,
                        credentials_valid: false,
                        credentials_expired: true,
                        credentials_invalid: false,
                        sync_available: false,
                        download_entitled: false,
                        download_auth_failed: false,
                        display_name,
                        email,
                        error_message: Some("Tidal token expired.".to_string()),
                        last_auth_error: last_auth_err,
                        last_auth_error_at: last_auth_err_at,
                        last_checked: Some(now_iso),
                    });
                }
            }
            Ok(ServiceAuthStatus {
                service: svc_name,
                account_id: Some(id),
                is_active: is_active != 0,
                status: "connected_valid".to_string(),
                is_authenticated: true,
                credentials_valid: true,
                credentials_expired: false,
                credentials_invalid: false,
                sync_available: true,
                download_entitled: true,
                download_auth_failed: has_download_err,
                display_name,
                email,
                error_message: None,
                last_auth_error: last_auth_err,
                last_auth_error_at: last_auth_err_at,
                last_checked: Some(now_iso),
            })
        }
        "deezer" => {
            let arl = creds["arl"]
                .as_str()
                .or_else(|| creds["access_token"].as_str());
            // A4: no unwrap — map_or treats absent and blank ARL identically.
            if arl.is_none_or(|a| a.trim().is_empty()) {
                return Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "requires_auth".to_string(),
                    is_authenticated: false,
                    credentials_valid: false,
                    credentials_expired: false,
                    credentials_invalid: true,
                    sync_available: false,
                    download_entitled: false,
                    download_auth_failed: false,
                    display_name,
                    email,
                    error_message: Some(
                        "Deezer ARL missing. Please re-enter your ARL.".to_string(),
                    ),
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                });
            }
            Ok(ServiceAuthStatus {
                service: svc_name,
                account_id: Some(id),
                is_active: is_active != 0,
                status: "connected_valid".to_string(),
                is_authenticated: true,
                credentials_valid: true,
                credentials_expired: false,
                credentials_invalid: false,
                sync_available: true,
                download_entitled: true,
                download_auth_failed: has_download_err,
                display_name,
                email,
                error_message: None,
                last_auth_error: last_auth_err,
                last_auth_error_at: last_auth_err_at,
                last_checked: Some(now_iso),
            })
        }
        "apple_music" => {
            let has_dev_token = creds["developer_token"]
                .as_str()
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            let has_user_token = creds["music_user_token"]
                .as_str()
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            if has_dev_token && has_user_token {
                Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "connected_valid".to_string(),
                    is_authenticated: true,
                    credentials_valid: true,
                    credentials_expired: false,
                    credentials_invalid: false,
                    sync_available: true,
                    download_entitled: false,
                    download_auth_failed: false,
                    display_name,
                    email,
                    error_message: None,
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                })
            } else {
                Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "requires_auth".to_string(),
                    is_authenticated: false,
                    credentials_valid: false,
                    credentials_expired: false,
                    credentials_invalid: true,
                    sync_available: false,
                    download_entitled: false,
                    download_auth_failed: false,
                    display_name,
                    email,
                    error_message: Some("Apple Music requires developer_token and music_user_token in credentials. Please reconnect in Settings > Accounts.".to_string()),
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                })
            }
        }
        "soundcloud" => {
            // Un token guardado no prueba que siga sirviendo: se valida contra
            // la API (`GET /me`) antes de reportar la cuenta como conectada.
            let oauth_token = creds["oauth_token"]
                .as_str()
                .or_else(|| creds["access_token"].as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty());

            let token = match oauth_token {
                Some(token) => token,
                None => return Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "requires_auth".to_string(),
                    is_authenticated: false,
                    credentials_valid: false,
                    credentials_expired: false,
                    credentials_invalid: true,
                    sync_available: false,
                    download_entitled: false,
                    download_auth_failed: false,
                    display_name,
                    email,
                    error_message: Some(
                        "SoundCloud OAuth token missing. Please reconnect in Settings > Accounts."
                            .to_string(),
                    ),
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                }),
            };

            let client = crate::services::SoundCloudClient::new(token.to_string());
            match client.get_current_user().await {
                Ok(user) => Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "connected_valid".to_string(),
                    is_authenticated: true,
                    credentials_valid: true,
                    credentials_expired: false,
                    credentials_invalid: false,
                    sync_available: true,
                    download_entitled: false,
                    download_auth_failed: has_download_err,
                    display_name: display_name.or_else(|| {
                        let resolved = user.display_name().to_string();
                        (!resolved.is_empty()).then_some(resolved)
                    }),
                    email,
                    error_message: None,
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                }),
                // The API answered: 401/403 means the stored token no longer
                // works; anything else is a transport hiccup worth retrying.
                Err(err) => {
                    let rejected = err.contains("401") || err.contains("403");
                    Ok(ServiceAuthStatus {
                        service: svc_name,
                        account_id: Some(id),
                        is_active: is_active != 0,
                        status: if rejected {
                            "requires_auth".to_string()
                        } else {
                            "error".to_string()
                        },
                        is_authenticated: false,
                        credentials_valid: false,
                        credentials_expired: false,
                        credentials_invalid: rejected,
                        sync_available: false,
                        download_entitled: false,
                        download_auth_failed: false,
                        display_name,
                        email,
                        error_message: Some(if rejected {
                            format!(
                                "SoundCloud rejected the stored OAuth token ({}). Please reconnect in Settings > Accounts.",
                                err
                            )
                        } else {
                            format!("SoundCloud auth check failed: {}", err)
                        }),
                        last_auth_error: last_auth_err,
                        last_auth_error_at: last_auth_err_at,
                        last_checked: Some(now_iso),
                    })
                }
            }
        }
        _ => {
            let has_any_token = creds.as_object().map(|m| !m.is_empty()).unwrap_or(false);
            if has_any_token {
                Ok(ServiceAuthStatus {
                    service: svc_name,
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "connected_valid".to_string(),
                    is_authenticated: true,
                    credentials_valid: true,
                    credentials_expired: false,
                    credentials_invalid: false,
                    sync_available: true,
                    download_entitled: true,
                    download_auth_failed: has_download_err,
                    display_name,
                    email,
                    error_message: None,
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                })
            } else {
                Ok(ServiceAuthStatus {
                    service: svc_name.clone(),
                    account_id: Some(id),
                    is_active: is_active != 0,
                    status: "requires_auth".to_string(),
                    is_authenticated: false,
                    credentials_valid: false,
                    credentials_expired: false,
                    credentials_invalid: true,
                    sync_available: false,
                    download_entitled: false,
                    download_auth_failed: false,
                    display_name,
                    email,
                    error_message: Some(format!("No valid credentials found for {}", svc_name)),
                    last_auth_error: last_auth_err,
                    last_auth_error_at: last_auth_err_at,
                    last_checked: Some(now_iso),
                })
            }
        }
    }
}

/// Tauri command to get real service authentication status
#[tauri::command]
pub async fn get_service_auth_status(
    state: State<'_, AppState>,
    service: String,
    account_id: Option<i64>,
) -> Result<ServiceAuthStatus, String> {
    perform_get_service_auth_status(&state.db, &service, account_id).await
}

// ==============================================
// CREDENTIAL INVALIDATION HELPER (S127B)
// ==============================================

/// Mark the active account for a service as having invalid credentials.
///
/// Called when an HTTP 401 is received mid-flight to ensure the account
/// is flagged before returning a RequiresAuth error to the caller.
/// Does NOT delete any library data or cascade.
///
/// Multi-cuenta: `account_id` acota la invalidación a la cuenta QUE FALLÓ.
/// `Some(id)` marca esa fila concreta (verificando además que pertenezca al
/// servicio); `None` conserva el comportamiento histórico —marcar las filas
/// ACTIVAS del servicio—, que es lo correcto mientras el sync va anclado a la
/// cuenta activa.
///
/// Returns the number of rows updated (0 if no account matched).
pub async fn mark_account_credentials_invalid(
    db: &sqlx::SqlitePool,
    service_name: &str,
    reason: &str,
    account_id: Option<i64>,
) -> Result<u64, String> {
    let rows_affected = match account_id {
        Some(aid) => {
            sqlx::query(
                r#"
                UPDATE accounts
                SET credentials_invalid = 1,
                    invalid_reason      = ?,
                    last_auth_error     = ?,
                    last_auth_error_at  = ?
                WHERE id = ?
                  AND service_id = (SELECT id FROM services WHERE name = ? LIMIT 1)
                "#,
            )
            .bind(reason)
            .bind(reason)
            // A-4: el latch se autocura por antigüedad (orchestrator.rs), y para eso
            // necesita saber CUÁNDO se marcó. Sin esta columna el veredicto no tiene
            // edad y el servicio quedaba bloqueado para siempre.
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(aid)
            .bind(service_name)
            .execute(db)
            .await
            .map_err(|e| {
                format!(
                    "Failed to mark {} account {} credentials invalid: {}",
                    service_name, aid, e
                )
            })?
            .rows_affected()
        }
        None => sqlx::query(
            r#"
            UPDATE accounts
            SET credentials_invalid = 1,
                invalid_reason      = ?,
                last_auth_error     = ?,
                last_auth_error_at  = ?
            WHERE service_id = (SELECT id FROM services WHERE name = ? LIMIT 1)
              AND is_active = 1
            "#,
        )
        .bind(reason)
        .bind(reason)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(service_name)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to mark {} credentials invalid: {}", service_name, e))?
        .rows_affected(),
    };

    if rows_affected > 0 {
        tracing::warn!(
            "[Auth] Marked {} account credentials invalid (account_id={:?}). Reason: {}",
            service_name,
            account_id,
            reason
        );
        emit_auth_state_updated_for_account(service_name, "invalidated", account_id, Some(reason));
    } else {
        tracing::warn!(
            "[Auth] mark_account_credentials_invalid called for {} (account_id={:?}) but no matching account found",
            service_name,
            account_id
        );
    }

    Ok(rows_affected)
}

#[cfg(test)]
mod accounts_tests {

    use super::{activate_account_exclusive, insert_account_exclusive};
    use sqlx::sqlite::SqlitePoolOptions;

    /// Create an in-memory test database with schema
    async fn setup_test_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        // Create minimal schema for testing
        sqlx::query(
            r#"
            CREATE TABLE services (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                supports_download INTEGER DEFAULT 0,
                max_quality TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create services table");

        sqlx::query(
            r#"
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                display_name TEXT,
                email TEXT,
                is_active INTEGER DEFAULT 1,
                credentials_json TEXT,
                last_synced TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(service_id, email)
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create accounts table");

        // Seed services
        sqlx::query(
            r#"
            INSERT INTO services (name, supports_download, max_quality) VALUES
                ('spotify', 0, 'lossy'),
                ('qobuz', 1, 'hires'),
                ('tidal', 1, 'hires')
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to seed services");

        pool
    }

    #[tokio::test]
    async fn test_add_account_inserts_record() {
        let pool = setup_test_db().await;

        // Get spotify service id
        let (service_id,): (i64,) =
            sqlx::query_as("SELECT id FROM services WHERE name = 'spotify'")
                .fetch_one(&pool)
                .await
                .expect("Failed to get spotify id");

        // Insert account directly (simulating add_account command)
        let encrypted_creds = "encrypted_test_data";
        let account_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active, created_at)
               VALUES (?, ?, ?, ?, 1, CURRENT_TIMESTAMP) RETURNING id"#
        )
        .bind(service_id)
        .bind(encrypted_creds)
        .bind("Test Account")
        .bind("test@example.com")
        .fetch_one(&pool)
        .await
        .expect("Failed to insert account");

        assert!(account_id > 0);

        // Verify account exists
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts");

        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn test_get_accounts_returns_account_info() {
        let pool = setup_test_db().await;

        // Get qobuz service ID dynamically
        let (qobuz_id,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
            .fetch_one(&pool)
            .await
            .expect("Failed to get qobuz id");

        // Insert test account with correct service_id
        sqlx::query(
            "INSERT INTO accounts (service_id, display_name, email, is_active) VALUES (?, 'My Qobuz', 'qobuz@test.com', 1)"
        )
        .bind(qobuz_id)
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Query accounts with join (simulating get_accounts)
        let rows: Vec<(i64, i64, String, Option<String>, Option<String>, i64, Option<String>, Option<String>)> =
            sqlx::query_as(
                r#"SELECT a.id, a.service_id, s.name, a.display_name, a.email, a.is_active, a.last_synced, a.created_at
                   FROM accounts a
                   JOIN services s ON s.id = a.service_id
                   ORDER BY s.name, a.created_at"#
            )
            .fetch_all(&pool)
            .await
            .expect("Failed to fetch accounts");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2, "qobuz"); // service_name
        assert_eq!(rows[0].3, Some("My Qobuz".to_string())); // display_name
        assert_eq!(rows[0].4, Some("qobuz@test.com".to_string())); // email
    }

    #[tokio::test]
    async fn test_remove_account_deletes_record() {
        let pool = setup_test_db().await;

        // Insert account
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'delete@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Verify exists
        let count_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts before delete");
        assert_eq!(count_before.0, 1);

        // Delete account (simulating remove_account)
        sqlx::query("DELETE FROM accounts WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to delete account");

        // Verify deleted
        let count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts after delete");
        assert_eq!(count_after.0, 0);
    }

    #[tokio::test]
    async fn test_toggle_account_active() {
        let pool = setup_test_db().await;

        // Insert active account
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'toggle@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Toggle to inactive
        sqlx::query("UPDATE accounts SET is_active = 0 WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to toggle account");

        // Verify inactive
        let (is_active,): (i64,) = sqlx::query_as("SELECT is_active FROM accounts WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch account");

        assert_eq!(is_active, 0);

        // Toggle back to active
        sqlx::query("UPDATE accounts SET is_active = 1 WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to toggle account");

        let (is_active2,): (i64,) = sqlx::query_as("SELECT is_active FROM accounts WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch account after toggle back to active");

        assert_eq!(is_active2, 1);
    }

    #[tokio::test]
    async fn test_update_account_sync_time() {
        let pool = setup_test_db().await;

        // Insert account with no sync time
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active, last_synced) VALUES (1, 'sync@test.com', 1, NULL)"
        )
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Update sync time
        sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to update sync time");

        // Verify sync time is set
        let (last_synced,): (Option<String>,) =
            sqlx::query_as("SELECT last_synced FROM accounts WHERE id = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to fetch account");

        assert!(last_synced.is_some());
    }

    #[tokio::test]
    async fn test_credentials_stored_encrypted() {
        let pool = setup_test_db().await;

        // Initialize crypto for test (OnceLock may already be set by another test — absorb)
        let key = crate::crypto::generate_random_key();
        let _ = crate::crypto::init_crypto(key);

        // Simulate encrypted credentials
        let plaintext = r#"{"access_token": "secret123"}"#;
        let encrypted = crate::crypto::encrypt(plaintext).expect("Encryption failed");

        // Insert with encrypted creds
        sqlx::query(
            "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'creds@test.com', ?, 1)"
        )
        .bind(&encrypted)
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Fetch and verify encrypted value is different from plaintext
        let (stored_creds,): (String,) =
            sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to fetch credentials");

        assert_ne!(stored_creds, plaintext);

        // Decrypt and verify
        let decrypted = crate::crypto::decrypt(&stored_creds).expect("Decryption failed");
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_unique_constraint_service_email() {
        let pool = setup_test_db().await;

        // Insert first account
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'dupe@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert first account");

        // Try to insert duplicate - should fail
        let result = sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'dupe@test.com', 1)",
        )
        .execute(&pool)
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_accounts_per_service() {
        let pool = setup_test_db().await;

        // Insert multiple accounts for same service with different emails
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'user1@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert first account");

        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'user2@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert second account");

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts");

        assert_eq!(count.0, 2);

        // Multi-cuenta: varias filas por servicio SÍ, pero exactamente UNA activa.
        // El seed las deja a las dos activas a propósito (el estado que el upsert,
        // add_account y set_active_account ya no producen) y se comprueba que
        // activar una de ellas deja el invariante entero.
        let (first_id,): (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = 1 ORDER BY id LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to read first account");
        let (second_id,): (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = 1 ORDER BY id DESC LIMIT 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to read second account");

        let active_before: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = 1 AND is_active = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to count active accounts");
        assert_eq!(
            active_before.0, 2,
            "el seed parte de dos activas (estado que la app ya no produce)"
        );

        activate_account_exclusive(&pool, first_id)
            .await
            .expect("set_active_account must succeed");

        let (active_after,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = 1 AND is_active = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to count active accounts");
        assert_eq!(
            active_after, 1,
            "activar una cuenta deja exactamente UNA activa por servicio"
        );

        let (which_active,): (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = 1 AND is_active = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to read the active account");
        assert_eq!(which_active, first_id);
        let (still_there,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to count accounts");
        assert_eq!(still_there, 2, "activar NO borra ninguna fila del servicio");

        // Volver a la segunda la deja activa; el servicio nunca queda con dos.
        activate_account_exclusive(&pool, second_id)
            .await
            .expect("switching back must succeed");
        let (active_after2,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = 1 AND is_active = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to count active accounts");
        assert_eq!(active_after2, 1);
    }

    /// Multi-cuenta: `add_account` inserta la fila activa y desactiva a sus
    /// hermanas en la MISMA transacción. Antes las dos quedaban activas y el
    /// resolvedor `is_active = 1 ORDER BY id DESC LIMIT 1` saltaba entre ellas.
    #[tokio::test]
    async fn test_add_account_deactivates_sibling_accounts() {
        let pool = setup_test_db().await;

        let (service_id,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
            .fetch_one(&pool)
            .await
            .expect("Failed to get qobuz id");

        let first = insert_account_exclusive(
            &pool,
            service_id,
            "enc_1",
            Some("One"),
            Some("one@test.com"),
        )
        .await
        .expect("first account insert");
        let second = insert_account_exclusive(
            &pool,
            service_id,
            "enc_2",
            Some("Two"),
            Some("two@test.com"),
        )
        .await
        .expect("second account insert");

        let (active_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = ? AND is_active = 1")
                .bind(service_id)
                .fetch_one(&pool)
                .await
                .expect("Failed to count active accounts");
        assert_eq!(
            active_count, 1,
            "añadir una cuenta no puede dejar dos activas"
        );

        let (active_id,): (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = ? AND is_active = 1")
                .bind(service_id)
                .fetch_one(&pool)
                .await
                .expect("Failed to read active account");
        assert_eq!(active_id, second, "la cuenta recién añadida es la activa");

        let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
            .bind(service_id)
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts");
        assert_eq!(total, 2, "la cuenta previa se conserva");

        // Y la cuenta anterior se puede volver a activar.
        activate_account_exclusive(&pool, first)
            .await
            .expect("re-activate");
        let (back,): (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = ? AND is_active = 1")
                .bind(service_id)
                .fetch_one(&pool)
                .await
                .expect("Failed to read active account");
        assert_eq!(back, first);
    }
}
