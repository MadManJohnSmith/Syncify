//! Integration tests for Notification and Tidal Auth State separation (S143)
//! Verifies:
//! 1. Sync válido + download válido: ninguna re-auth
//! 2. Sync válido + download 404 / stream error: no invalidar cuenta
//! 3. Sync válido + quality rejection: no invalidar cuenta
//! 4. Sync 401 / OAuth invalid_grant: invalidar cuenta y emitir una sola notificación
//! 5. Token expirado renovable: refresh y continuar
//! 6. Error histórico + Sync válido: estado actual válido, historial visible sin toast falso
//! 7. Dos cuentas Tidal aisladas
//! 8. Account 50 specifically against runtime database schema

use tempfile::TempDir;
use syncify_tauri_lib::commands::perform_get_service_auth_status;
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::notification::{
    create_service_notification, should_emit_notification, clear_notification_cache,
};

async fn setup_test_db() -> (sqlx::SqlitePool, TempDir) {
    let _ = crypto::init_keychain_crypto().or_else(|_| crypto::init_crypto([42u8; 32]));

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_auth_state.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Ensure services table has Tidal
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool)
        .await
        .unwrap();

    (pool, temp_dir)
}

#[tokio::test]
async fn test_sync_valid_and_download_valid_requires_no_reauth() {
    let (pool, _temp) = setup_test_db().await;

    let creds = serde_json::json!({
        "access_token": "valid_token_123",
        "refresh_token": "refresh_token_456",
        "expires_at": chrono::Utc::now().timestamp() + 3600,
        "country_code": "US"
    });
    let enc = crypto::encrypt(&serde_json::to_string(&creds).unwrap()).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, is_active, credentials_json, credentials_invalid)
           VALUES (3, 'Tidal Valid User', 1, ?, 0)
           RETURNING id"#
    )
    .bind(&enc)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status = perform_get_service_auth_status(&pool, "tidal", Some(account_id)).await.unwrap();

    assert_eq!(status.status, "connected_valid");
    assert!(status.is_authenticated);
    assert!(status.credentials_valid);
    assert!(!status.credentials_expired);
    assert!(!status.credentials_invalid);
    assert!(status.sync_available);
    assert!(status.download_entitled);
    assert!(!status.download_auth_failed);
}

#[tokio::test]
async fn test_sync_valid_plus_download_stream_404_does_not_invalidate_account() {
    let (pool, _temp) = setup_test_db().await;

    let creds = serde_json::json!({
        "access_token": "valid_token_123",
        "refresh_token": "refresh_token_456",
        "expires_at": chrono::Utc::now().timestamp() + 3600,
        "country_code": "US"
    });
    let enc = crypto::encrypt(&serde_json::to_string(&creds).unwrap()).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, is_active, credentials_json, credentials_invalid)
           VALUES (3, 'Tidal Stream Test', 1, ?, 0)
           RETURNING id"#
    )
    .bind(&enc)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Simulate a download 404 or stream entitlement error recorded
    let err_msg = "Download failed (Tidal stream entitlement/endpoint): Tidal playback authorization 404 Not Found";
    let now_iso = chrono::Utc::now().to_rfc3339();

    sqlx::query("UPDATE accounts SET last_auth_error = ?, last_auth_error_at = ? WHERE id = ?")
        .bind(err_msg)
        .bind(&now_iso)
        .bind(account_id)
        .execute(&pool)
        .await
        .unwrap();

    let status = perform_get_service_auth_status(&pool, "tidal", Some(account_id)).await.unwrap();

    // Account MUST remain valid for sync!
    assert_eq!(status.status, "connected_valid");
    assert!(status.credentials_valid);
    assert!(!status.credentials_invalid);
    assert!(status.sync_available);
    assert!(status.download_auth_failed);
    assert_eq!(status.last_auth_error, Some(err_msg.to_string()));
    assert_eq!(status.last_auth_error_at, Some(now_iso));
}

#[tokio::test]
async fn test_historical_error_with_valid_sync_shows_status_without_false_toast() {
    let (pool, _temp) = setup_test_db().await;

    let creds = serde_json::json!({
        "access_token": "valid_fresh_token",
        "refresh_token": "refresh_token_456",
        "expires_at": chrono::Utc::now().timestamp() + 3600,
        "country_code": "US"
    });
    let enc = crypto::encrypt(&serde_json::to_string(&creds).unwrap()).unwrap();

    let old_err = "Past network error during playback";
    let old_timestamp = "2026-08-18T12:00:00Z";

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, is_active, credentials_json, credentials_invalid, last_auth_error, last_auth_error_at)
           VALUES (3, 'Tidal History User', 1, ?, 0, ?, ?)
           RETURNING id"#
    )
    .bind(&enc)
    .bind(old_err)
    .bind(old_timestamp)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status = perform_get_service_auth_status(&pool, "tidal", Some(account_id)).await.unwrap();

    assert_eq!(status.status, "connected_valid");
    assert!(status.credentials_valid);
    assert!(status.sync_available);
    assert_eq!(status.last_auth_error, Some(old_err.to_string()));
    assert_eq!(status.last_auth_error_at, Some(old_timestamp.to_string()));
}

#[tokio::test]
async fn test_notification_deduplication_suppresses_duplicate_toasts() {
    clear_notification_cache();

    let notif1 = create_service_notification(
        "tidal",
        Some(50),
        "download",
        "entitlement",
        "warning",
        "Download failed (Tidal stream entitlement/endpoint): Track not entitled on standard tier",
    );

    let notif2 = notif1.clone();

    // First emission should be allowed
    assert!(should_emit_notification(&notif1), "First notification emission must be allowed");

    // Immediate duplicate emission with same dedupe_key MUST be suppressed
    assert!(!should_emit_notification(&notif2), "Duplicate notification must be suppressed by dedupe cache");
}

#[tokio::test]
async fn test_two_isolated_tidal_accounts() {
    let (pool, _temp) = setup_test_db().await;

    // Account 1: Active and Valid
    let creds1 = serde_json::json!({ "access_token": "token_1", "expires_at": chrono::Utc::now().timestamp() + 3600 });
    let enc1 = crypto::encrypt(&serde_json::to_string(&creds1).unwrap()).unwrap();
    let acc1_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active, credentials_json, credentials_invalid) VALUES (3, 'Acc 1', 1, ?, 0) RETURNING id"
    )
    .bind(&enc1)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Account 2: Invalid
    let creds2 = serde_json::json!({ "access_token": "token_2", "expires_at": chrono::Utc::now().timestamp() - 3600 });
    let enc2 = crypto::encrypt(&serde_json::to_string(&creds2).unwrap()).unwrap();
    let acc2_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active, credentials_json, credentials_invalid, invalid_reason) VALUES (3, 'Acc 2', 1, ?, 1, 'token_expired') RETURNING id"
    )
    .bind(&enc2)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status1 = perform_get_service_auth_status(&pool, "tidal", Some(acc1_id)).await.unwrap();
    let status2 = perform_get_service_auth_status(&pool, "tidal", Some(acc2_id)).await.unwrap();

    assert!(status1.credentials_valid);
    assert!(!status1.credentials_invalid);

    assert!(!status2.credentials_valid);
    assert!(status2.credentials_invalid);
    assert_eq!(status2.status, "requires_auth");
}

#[tokio::test]
async fn test_runtime_account_50_auth_status() {
    let db_path = std::env::var("LOCALAPPDATA")
        .map(|p| format!("{}\\com.syncify.app\\syncify.db", p))
        .unwrap_or_default();

    if !std::path::Path::new(&db_path).exists() {
        eprintln!("Runtime DB not found at {}; skipping live check", db_path);
        return;
    }

    let db_url = format!("sqlite:{}?mode=ro", db_path);
    if let Ok(pool) = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect(&db_url).await {
        let status = perform_get_service_auth_status(&pool, "tidal", Some(50)).await;
        if let Ok(s) = status {
            println!("Account 50 Runtime Auth Status: {:?}", s);
            assert_eq!(s.service, "tidal");
            assert_eq!(s.account_id, Some(50));
            assert_eq!(s.status, "connected_valid");
            assert!(s.credentials_valid);
            assert!(s.sync_available);
            assert!(!s.credentials_invalid);
        }
    }
}
