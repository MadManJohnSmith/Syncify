// Auth Commands - included via include!() in mod.rs
// 
// Python auth bridge, session validation


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

/// Start auth flow for a service (spawns Python subprocess)
#[tauri::command]
pub async fn start_auth(service: String, action: String) -> Result<AuthResult, String> {
    tracing::info!("start_auth: {} {}", service, action);

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
    let output = tokio::process::Command::new(&python_cmd)
        .arg(&script_path)
        .arg(&service)
        .arg(&action)
        .current_dir(&project_root)
        .output()
        .await
        .map_err(|e| format!("Failed to run Python: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    tracing::info!("Auth output: {}", stdout);
    if !stderr.is_empty() {
        tracing::warn!("Auth stderr: {}", stderr);
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
            "Failed to parse auth result: {} (raw: {})",
            e, stdout
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
pub async fn logout_service(service: String) -> Result<AuthResult, String> {
    start_auth(service, "logout".to_string()).await
}

/// Validate that a Qobuz auth token is usable (defensive filter against storage artifacts).
fn is_viable_qobuz_token_auth(token: &str) -> bool {
    let t = token.trim();
    if t.is_empty() || t == "browser_cookies" || t == "null" || t == "undefined" {
        return false;
    }
    if t.starts_with('{') || t.starts_with('[') {
        return false;
    }
    if t.len() < 16 {
        return false;
    }
    !t.chars().any(|c| c.is_whitespace())
}

/// Load Qobuz fallback auth data from scripts/.gui_credentials_cache.json.
/// Returns (token, username/email, password) when available.
fn load_qobuz_cache_fallback_auth() -> (Option<String>, Option<String>, Option<String>) {
    let cache_path = get_project_root().join("scripts").join(".gui_credentials_cache.json");

    let cache_text = match std::fs::read_to_string(&cache_path) {
        Ok(text) => text,
        Err(_) => return (None, None, None),
    };

    let parsed: serde_json::Value = match serde_json::from_str(&cache_text) {
        Ok(value) => value,
        Err(_) => return (None, None, None),
    };

    let session = parsed.get("qobuz_session").and_then(|v| v.as_object());
    let account = parsed.get("qobuz").and_then(|v| v.as_object());

    let token = session
        .and_then(|s| s.get("auth_token"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| is_viable_qobuz_token_auth(s));

    let username = session
        .and_then(|s| s.get("username"))
        .and_then(|v| v.as_str())
        .or_else(|| account.and_then(|a| a.get("username")).and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let password = session
        .and_then(|s| s.get("password"))
        .and_then(|v| v.as_str())
        .or_else(|| account.and_then(|a| a.get("password")).and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    (token, username, password)
}

/// Start auth flow and save credentials to database
/// This is the main command for UI-driven authentication
#[tauri::command]
pub async fn start_auth_and_save(
    service: String,
    state: State<'_, AppState>,
) -> Result<AuthResult, String> {
    tracing::info!("start_auth_and_save: {}", service);

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
        let (cache_token, cache_username, cache_password) = load_qobuz_cache_fallback_auth();

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
            .or(cache_username);

        let password = data
            .get("password")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
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

    // Try UPDATE first (preserves row + cascaded data, resets credentials_invalid)
    let update_result = sqlx::query(
        r#"
        UPDATE accounts
        SET display_name = ?,
            email = ?,
            credentials_json = ?,
            credentials_invalid = 0,
            is_active = 1,
            last_synced = CURRENT_TIMESTAMP
        WHERE service_id = ?
        "#
    )
    .bind(&final_display_name)
    .bind(&email)
    .bind(&encrypted)
    .bind(service_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to update account: {}", e))?;

    // If no existing row was updated, INSERT a new one
    if update_result.rows_affected() == 0 {
        sqlx::query(
            r#"
            INSERT INTO accounts (service_id, display_name, email, credentials_json, credentials_invalid, is_active, last_synced, created_at)
            VALUES (?, ?, ?, ?, 0, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#
        )
        .bind(service_id)
        .bind(&final_display_name)
        .bind(&email)
        .bind(&encrypted)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to save account: {}", e))?;
    }

    tracing::info!("Saved {} account: {}", service, final_display_name);

    // Return success with saved info
    Ok(AuthResult {
        success: true,
        data: Some(serde_json::json!({
            "message": format!("Connected as {}", final_display_name),
            "display_name": final_display_name,
            "email": email,
            "user_id": user_id,
        })),
        error: None,
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
