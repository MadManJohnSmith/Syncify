//! Integration test suite for TASK-100 (SEC-016):
//! 1. Masking of sensitive API keys and secrets in `get_kv_settings`.
//! 2. Log sanitization of session cookies (`sp_dc`, `sp_key`, `arl`, etc.) and `Cookie: ...` headers.
//! 3. Removal verification of obsolete script `src-tauri/get_token.py`.

use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use syncify_tauri_lib::commands::{
    is_sensitive_setting_key, mask_sensitive_setting_value, perform_get_kv_settings,
    perform_save_setting,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::logging::sanitize_log_message;

async fn setup_test_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    pool
}

#[tokio::test]
async fn test_get_kv_settings_masks_sensitive_keys() {
    let _ = crypto::init_crypto([42u8; 32]);
    let pool = setup_test_db().await;

    // 1. Spotify client secret (encrypted at rest, masked on read)
    perform_save_setting(
        &pool,
        "spotify_client_secret".to_string(),
        "super_spotify_secret_1234".to_string(),
    )
    .await
    .unwrap();

    // 2. Last.fm API key (plain KV, masked on read)
    perform_save_setting(
        &pool,
        "lastfm_api_key".to_string(),
        "b25b959554ed76058ac220b7b2e0a026".to_string(),
    )
    .await
    .unwrap();

    // 3. Generic sensitive keys containing token, secret, password, key
    perform_save_setting(
        &pool,
        "custom_auth_token".to_string(),
        "bearer_token_secret_value_xyz".to_string(),
    )
    .await
    .unwrap();

    perform_save_setting(
        &pool,
        "admin_password".to_string(),
        "my_super_secret_password".to_string(),
    )
    .await
    .unwrap();

    perform_save_setting(
        &pool,
        "short_key".to_string(),
        "secret".to_string(),
    )
    .await
    .unwrap();

    // 4. Non-sensitive key (should NOT be masked)
    perform_save_setting(
        &pool,
        "dl_concurrent_downloads".to_string(),
        "5".to_string(),
    )
    .await
    .unwrap();

    let keys = vec![
        "spotify_client_secret".to_string(),
        "lastfm_api_key".to_string(),
        "custom_auth_token".to_string(),
        "admin_password".to_string(),
        "short_key".to_string(),
        "dl_concurrent_downloads".to_string(),
    ];

    let result = perform_get_kv_settings(&pool, keys).await.unwrap();

    // Spotify client secret verification: format is ****<last4>
    let spotify_secret = result.get("spotify_client_secret").expect("must exist");
    assert!(
        spotify_secret.starts_with("****"),
        "Spotify secret must start with **** mask prefix: {}",
        spotify_secret
    );
    assert!(
        spotify_secret.ends_with("1234"),
        "Spotify secret must expose only last 4 chars: {}",
        spotify_secret
    );
    assert!(
        !spotify_secret.contains("super_spotify_secret"),
        "Spotify secret must never leak plaintext"
    );

    // Last.fm API key verification: prefix and suffix preserved, middle masked
    let lastfm = result.get("lastfm_api_key").expect("must exist");
    assert!(
        lastfm.starts_with("b2"),
        "Lastfm key should show prefix: {}",
        lastfm
    );
    assert!(
        lastfm.ends_with("26"),
        "Lastfm key should show suffix: {}",
        lastfm
    );
    assert!(
        lastfm.contains("****"),
        "Lastfm key must contain mask sentinel: {}",
        lastfm
    );
    assert!(
        !lastfm.contains("959554ed76058ac220b7b2e0a0"),
        "Lastfm key must never leak the full secret"
    );

    // Generic auth token verification
    let token = result.get("custom_auth_token").expect("must exist");
    assert!(token.contains("****"), "Auth token must be masked: {}", token);
    assert!(!token.contains("secret_value"), "Auth token must not leak value");

    // Generic password verification
    let pass = result.get("admin_password").expect("must exist");
    assert!(pass.contains("****"), "Password must be masked: {}", pass);
    assert!(!pass.contains("secret_password"), "Password must not leak");

    // Short secret verification (<= 6 chars)
    let short = result.get("short_key").expect("must exist");
    assert_eq!(short, "********", "Short secret should be fully masked");

    // Non-sensitive key verification
    let concurrent = result.get("dl_concurrent_downloads").expect("must exist");
    assert_eq!(concurrent, "5", "Non-sensitive setting must not be masked");

    // Masked save protection: saving back the masked value must not corrupt the real stored value
    perform_save_setting(
        &pool,
        "lastfm_api_key".to_string(),
        lastfm.clone(),
    )
    .await
    .unwrap();

    let raw_db_val: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'lastfm_api_key'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        raw_db_val, "b25b959554ed76058ac220b7b2e0a026",
        "Saving masked placeholder must never overwrite the original stored secret"
    );
}

#[test]
fn test_sensitive_key_detection_and_masking() {
    assert!(is_sensitive_setting_key("spotify_client_secret"));
    assert!(is_sensitive_setting_key("lastfm_api_key"));
    assert!(is_sensitive_setting_key("user_auth_token"));
    assert!(is_sensitive_setting_key("api_key"));
    assert!(is_sensitive_setting_key("login_password"));
    assert!(is_sensitive_setting_key("app_secret"));
    assert!(is_sensitive_setting_key("PRIVATE_KEY"));

    assert!(!is_sensitive_setting_key("dl_download_path"));
    assert!(!is_sensitive_setting_key("dl_concurrent_downloads"));
    assert!(!is_sensitive_setting_key("download_dir"));
    assert!(!is_sensitive_setting_key("preferred_quality"));
    assert!(!is_sensitive_setting_key("global_max_quality"));

    let masked_long = mask_sensitive_setting_value("lastfm_api_key", "1234567890abcdef");
    assert_eq!(masked_long, "12****ef");

    let masked_short = mask_sensitive_setting_value("token", "12345");
    assert_eq!(masked_short, "********");

    let masked_empty = mask_sensitive_setting_value("token", "   ");
    assert_eq!(masked_empty, "");
}

#[test]
fn test_log_sanitization_for_session_cookies_and_headers() {
    // 1. Plain Cookie header
    let raw_cookie = "Outgoing HTTP Request Cookie: sp_dc=AQBAEPG1234567890abcdef; sp_key=12345678-1234; other=normal";
    let sanitized_cookie = sanitize_log_message(raw_cookie);
    assert!(!sanitized_cookie.contains("AQBAEPG1234567890abcdef"));
    assert!(!sanitized_cookie.contains("12345678-1234"));
    assert!(sanitized_cookie.contains("[REDACTED]"));

    // 2. Deezer ARL token in key-value format
    let raw_arl = "Deezer authentication session: arl=abcdef0123456789abcdef0123456789";
    let sanitized_arl = sanitize_log_message(raw_arl);
    assert!(!sanitized_arl.contains("abcdef0123456789abcdef0123456789"));
    assert!(sanitized_arl.contains("[REDACTED]"));

    // 3. Spotify sp_dc session cookie in assignment format
    let raw_sp_dc = "Spotify active session sp_dc=AQB_secret_session_token_9999";
    let sanitized_sp_dc = sanitize_log_message(raw_sp_dc);
    assert!(!sanitized_sp_dc.contains("AQB_secret_session_token_9999"));
    assert!(sanitized_sp_dc.contains("[REDACTED]"));

    // 4. Case-insensitive lowercase cookie header
    let raw_lower = "header cookie: session_id=sess_abcdef123456789";
    let sanitized_lower = sanitize_log_message(raw_lower);
    assert!(!sanitized_lower.contains("sess_abcdef123456789"));
    assert!(sanitized_lower.contains("[REDACTED]"));

    // 5. JSON formatted Cookie
    let raw_json = r#"{"headers": {"Cookie": "sp_dc=AQB_json_cookie_val; arl=json_arl_val"}}"#;
    let sanitized_json = sanitize_log_message(raw_json);
    assert!(!sanitized_json.contains("AQB_json_cookie_val"));
    assert!(!sanitized_json.contains("json_arl_val"));
    assert!(sanitized_json.contains("[REDACTED]"));

    // 6. URL query parameters with session cookies
    let raw_url = "https://api.syncify.local/stream?sp_dc=AQB_query_cookie_123&arl=query_arl_456&normal=ok";
    let sanitized_url = sanitize_log_message(raw_url);
    assert!(!sanitized_url.contains("AQB_query_cookie_123"));
    assert!(!sanitized_url.contains("query_arl_456"));
    assert!(sanitized_url.contains("[REDACTED]"));
}

#[test]
fn test_get_token_py_purged() {
    let root_path = Path::new(env!("CARGO_MANIFEST_DIR"));
    let get_token_script = root_path.join("get_token.py");
    assert!(
        !get_token_script.exists(),
        "src-tauri/get_token.py must not exist in repository (SEC-016)"
    );

    let relative_path = Path::new("src-tauri/get_token.py");
    assert!(
        !relative_path.exists(),
        "src-tauri/get_token.py relative path must not exist"
    );
}
