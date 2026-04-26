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

// ==============================================
// SPOTIFY WEBVIEW AUTH (S65 + S66 PKCE)
// ==============================================

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
    
    let config = crate::services::spotify::SpotifyConfig::from_env()
        .map_err(|e| format!("Spotify config error: {}", e))?;
    let client_id = config.client_id;
    let redirect_uri = "http://127.0.0.1:8888/callback";
    let scope = "user-library-read playlist-read-private user-read-private user-read-email";
    
    let auth_url = format!(
        "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&code_challenge_method=S256&code_challenge={}&scope={}",
        client_id,
        urlencoding::encode(redirect_uri),
        code_challenge,
        urlencoding::encode(scope)
    );

    // 3. Bind TcpListener on 127.0.0.1:8888
    let listener = TcpListener::bind("127.0.0.1:8888")
        .await
        .map_err(|e| format!("Failed to bind port 8888 for callback: {}", e))?;

    // 4. Open WebView pointing to accounts.spotify.com
    let auth_window = tauri::WebviewWindowBuilder::new(
        &app,
        "spotify-auth",
        tauri::WebviewUrl::External(auth_url.parse().unwrap()),
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
            
            // Parse the GET request
            if request.starts_with("GET /callback") {
                if let Some(query) = request.lines().next().and_then(|l| l.split_whitespace().nth(1)) {
                    if let Some(pos) = query.find("code=") {
                        let code_part = &query[pos + 5..];
                        let code = code_part.split('&').next().unwrap_or(code_part);
                        code_opt = Some(code.to_string());
                        
                        // 7. Respond HTTP 200
                        let response = "HTTP/1.1 200 OK\r\n\
                                        Content-Type: text/html; charset=utf-8\r\n\
                                        Connection: close\r\n\
                                        \r\n\
                                        <html><body style=\"background:#121212;color:#1db954;display:flex;align-items:center;justify-content:center;height:100vh;font-family:sans-serif;font-size:24px;font-weight:bold;\">\
                                        Autenticado. Puedes cerrar esta ventana.</body></html>";
                        let _ = socket.write_all(response.as_bytes()).await;
                        let _ = socket.flush().await;
                        break;
                    }
                }
                
                let response = "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
            } else {
                let response = "HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
            }
        }
        Ok::<(), std::io::Error>(())
    }).await {
        Ok(Ok(_)) => {},
        Ok(Err(e)) => return Err(format!("Socket error: {}", e)),
        Err(_) => {
            let _ = auth_window.close();
            return Err("Authorization timed out".into());
        }
    }

    // 8. Close WebView
    let _ = auth_window.close();

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
    
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let expires_at = (now + expires_in) * 1000;

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
                                  credentials_invalid, is_active, last_synced, created_at)
            VALUES (?, ?, ?, ?, 0, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
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

