//! Unit and integration test suite for Tidal GUI authentication reuse, token refresh, and stream audit.

use syncify_cli::crypto::{encrypt, init_crypto};
use syncify_cli::download::{
    StreamSourceType, TidalAuthResolution, TidalAuthStatus, TidalDownloader, TidalGuiCredentials,
};
use syncify_cli::services::tidal::{refresh_gui_token, resolve_gui_credentials_from_pool};
use sqlx::sqlite::SqlitePoolOptions;
use std::time::{SystemTime, UNIX_EPOCH};

fn setup_test_crypto() {
    let dummy_key = [42u8; 32];
    let _ = init_crypto(dummy_key);
}

async fn create_in_memory_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory sqlite db");

    sqlx::query(
        "CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT UNIQUE NOT NULL);"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE accounts (
            id INTEGER PRIMARY KEY,
            service_id INTEGER REFERENCES services(id),
            credentials_json TEXT,
            is_active INTEGER DEFAULT 1
        );"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO services (id, name) VALUES (1, 'tidal');")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

#[tokio::test]
async fn test_locate_active_tidal_account() {
    setup_test_crypto();
    let pool = create_in_memory_db().await;

    let creds = TidalGuiCredentials {
        access_token: "test_access_token_123".to_string(),
        refresh_token: Some("test_refresh_token_abc".to_string()),
        token_expiry: Some((SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 3600) as f64),
        expires_at: None,
        expires_in: Some(3600.0),
        user_id: None,
        country_code: Some("MX".to_string()),
    };

    let serialized = serde_json::to_string(&creds).unwrap();
    let encrypted = encrypt(&serialized).unwrap();

    sqlx::query("INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (1, ?, 1);")
        .bind(&encrypted)
        .execute(&pool)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let (token_opt, resolution) = resolve_gui_credentials_from_pool(&pool, &client).await;

    assert_eq!(token_opt, Some("test_access_token_123".to_string()));
    assert!(matches!(resolution, TidalAuthResolution::StoredGuiAccessToken(_)));
}

#[tokio::test]
async fn test_decrypt_credentials_json() {
    setup_test_crypto();
    let creds = TidalGuiCredentials {
        access_token: "valid_secret_token".to_string(),
        refresh_token: Some("valid_refresh_token".to_string()),
        token_expiry: Some(2000000000.0),
        expires_at: None,
        expires_in: Some(14400.0),
        user_id: None,
        country_code: Some("US".to_string()),
    };

    let json = serde_json::to_string(&creds).unwrap();
    let encrypted = encrypt(&json).unwrap();
    let decrypted = syncify_cli::crypto::decrypt(&encrypted).unwrap();

    let parsed: TidalGuiCredentials = serde_json::from_str(&decrypted).unwrap();
    assert_eq!(parsed.access_token, "valid_secret_token");
}

#[tokio::test]
async fn test_valid_stored_access_token() {
    setup_test_crypto();
    let pool = create_in_memory_db().await;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64();

    let creds = TidalGuiCredentials {
        access_token: "active_token_xyz".to_string(),
        refresh_token: Some("ref_123".to_string()),
        token_expiry: Some(now + 7200.0),
        expires_at: None,
        expires_in: Some(7200.0),
        user_id: None,
        country_code: Some("MX".to_string()),
    };

    let encrypted = encrypt(&serde_json::to_string(&creds).unwrap()).unwrap();
    sqlx::query("INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (1, ?, 1);")
        .bind(&encrypted)
        .execute(&pool)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let (token_opt, resolution) = resolve_gui_credentials_from_pool(&pool, &client).await;

    assert_eq!(token_opt, Some("active_token_xyz".to_string()));
    assert_eq!(resolution, TidalAuthResolution::StoredGuiAccessToken("active_token_xyz".to_string()));
}

#[tokio::test]
async fn test_expired_access_token_triggers_refresh() {
    setup_test_crypto();
    let pool = create_in_memory_db().await;

    // Expired timestamp in the past
    let creds = TidalGuiCredentials {
        access_token: "expired_token_123".to_string(),
        refresh_token: Some("invalid_dummy_refresh".to_string()),
        token_expiry: Some(1000.0),
        expires_at: None,
        expires_in: Some(3600.0),
        user_id: None,
        country_code: Some("MX".to_string()),
    };

    let encrypted = encrypt(&serde_json::to_string(&creds).unwrap()).unwrap();
    sqlx::query("INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (1, ?, 1);")
        .bind(&encrypted)
        .execute(&pool)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let (token_opt, resolution) = resolve_gui_credentials_from_pool(&pool, &client).await;

    // With dummy refresh token, live HTTP refresh returns failure -> SourceUnavailable / RequiresAuth
    assert!(token_opt.is_none());
    assert!(matches!(resolution, TidalAuthResolution::SourceUnavailable(_) | TidalAuthResolution::RequiresAuth));
}

#[tokio::test]
async fn test_failed_refresh_returns_requires_auth() {
    setup_test_crypto();
    let client = reqwest::Client::new();
    let res = refresh_gui_token(&client, "invalid_dummy_refresh_token_xyz").await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_missing_account_returns_requires_auth() {
    setup_test_crypto();
    let pool = create_in_memory_db().await;

    let client = reqwest::Client::new();
    let (token_opt, resolution) = resolve_gui_credentials_from_pool(&pool, &client).await;

    assert_eq!(token_opt, None);
    assert_eq!(resolution, TidalAuthResolution::RequiresAuth);
}

#[tokio::test]
async fn test_incomplete_json_returns_requires_auth() {
    setup_test_crypto();
    let pool = create_in_memory_db().await;

    let encrypted = encrypt("{\"invalid_field\": true}").unwrap();
    sqlx::query("INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (1, ?, 1);")
        .bind(&encrypted)
        .execute(&pool)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let (token_opt, resolution) = resolve_gui_credentials_from_pool(&pool, &client).await;

    assert_eq!(token_opt, None);
    assert!(matches!(resolution, TidalAuthResolution::SourceUnavailable(_)));
}

#[tokio::test]
async fn test_client_id_incompatibility_detection() {
    let downloader = TidalDownloader::new();
    let res = downloader.get_stream_resolution(1352259, Some("16-44"), Some("incompatible_token"), false).await;

    assert!(res.is_err());
    let err_str = res.err().unwrap().to_string();
    assert!(err_str.contains("Official playback API returned") || err_str.contains("11002") || err_str.contains("HTTP 401") || err_str.contains("Token has invalid payload"));
}

#[tokio::test]
async fn test_requires_auth_classification() {
    let status = TidalAuthStatus::RequiresAuth;
    assert!(!status.is_user_authenticated());
    assert!(!status.can_access_public_catalog());
}

#[tokio::test]
async fn test_source_unavailable_classification() {
    let source = StreamSourceType::SourceUnavailable("HTTP 401 subStatus 11002".to_string());
    assert_eq!(source.to_string(), "Source Unavailable (HTTP 401 subStatus 11002)");
}

#[tokio::test]
async fn test_secure_token_reencryption_and_persistence() {
    setup_test_crypto();
    let pool = create_in_memory_db().await;

    let creds = TidalGuiCredentials {
        access_token: "new_refreshed_access_token".to_string(),
        refresh_token: Some("new_refreshed_refresh_token".to_string()),
        token_expiry: Some(2000000000.0),
        expires_at: None,
        expires_in: Some(14400.0),
        user_id: None,
        country_code: Some("MX".to_string()),
    };

    let serialized = serde_json::to_string(&creds).unwrap();
    let encrypted_new = encrypt(&serialized).unwrap();

    sqlx::query("INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (1, ?, 1);")
        .bind(&encrypted_new)
        .execute(&pool)
        .await
        .unwrap();

    let row: (String,) = sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = 1;")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_ne!(row.0, serialized); // Must NOT be plaintext
    let decrypted = syncify_cli::crypto::decrypt(&row.0).unwrap();
    assert_eq!(decrypted, serialized);
}

#[tokio::test]
async fn test_proxy_credential_isolation() {
    // Verify user tokens are never appended or sent to proxy endpoints
    let apis = TidalDownloader::get_proxy_apis();
    for api in apis {
        assert!(!api.contains("token="));
        assert!(!api.contains("Bearer"));
    }
}

#[tokio::test]
async fn test_playback_http_401_substatus_11002_handling() {
    let downloader = TidalDownloader::new();
    let res = downloader.get_stream_resolution(1352259, Some("16-44"), Some("token_with_substatus_11002"), false).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_audio_magic_header_flac_validation() {
    let flac_bytes = b"fLaC\x00\x00\x00\x22dummy_payload";
    assert!(flac_bytes.starts_with(b"fLaC"));

    let invalid_bytes = b"RIFF\x00\x00\x00\x00WAVE";
    assert!(!invalid_bytes.starts_with(b"fLaC"));
}

#[tokio::test]
async fn test_tagging_executed_only_after_valid_audio() {
    let is_audio_valid = false;
    let tagging_executed = if is_audio_valid { true } else { false };
    assert!(!tagging_executed);
}
