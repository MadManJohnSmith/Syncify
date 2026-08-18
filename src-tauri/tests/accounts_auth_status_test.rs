use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    perform_get_service_auth_status, perform_reset_database,
};
use syncify_tauri_lib::crypto;

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

#[tokio::test]
async fn test_qobuz_account_missing_returns_missing_status() {
    let pool = setup_test_db().await;

    let status = perform_get_service_auth_status(&pool, "qobuz", None)
        .await
        .expect("Auth status check should succeed");

    assert_eq!(status.service, "qobuz");
    assert_eq!(status.status, "missing");
    assert!(!status.is_authenticated);
    assert!(status.account_id.is_none());
}

#[tokio::test]
async fn test_qobuz_account_without_token_returns_requires_auth() {
    let pool = setup_test_db().await;

    // Insert Qobuz account without user_auth_token
    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let empty_creds = serde_json::json!({
        "user_id": "1234567"
    }).to_string();
    let encrypted = crypto::encrypt(&empty_creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Qobuz Test User', 'qobuz@example.com', ?, 1, 0) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status = perform_get_service_auth_status(&pool, "qobuz", Some(account_id))
        .await
        .unwrap();

    assert_eq!(status.status, "requires_auth");
    assert!(!status.is_authenticated);
    assert_eq!(status.account_id, Some(account_id));
    assert!(status.error_message.unwrap().contains("RequiresAuth"));
}

#[tokio::test]
async fn test_qobuz_account_with_expired_token_returns_expired() {
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let expired_creds = serde_json::json!({
        "user_auth_token": "expired_token_123",
        "expires_at": 1000 // Past timestamp
    }).to_string();
    let encrypted = crypto::encrypt(&expired_creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Qobuz Expired', 'expired@example.com', ?, 1, 0) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status = perform_get_service_auth_status(&pool, "qobuz", Some(account_id))
        .await
        .unwrap();

    assert_eq!(status.status, "expired");
    assert!(!status.is_authenticated);
    assert_eq!(status.account_id, Some(account_id));
}

#[tokio::test]
async fn test_qobuz_account_with_valid_token_returns_connected_valid() {
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let future_exp = chrono::Utc::now().timestamp() + 86400 * 30;
    let valid_creds = serde_json::json!({
        "user_auth_token": "valid_qobuz_user_auth_token_xyz987",
        "user_id": "998877",
        "expires_at": future_exp
    }).to_string();
    let encrypted = crypto::encrypt(&valid_creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Qobuz Hi-Res Fan', 'hires@example.com', ?, 1, 0) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status = perform_get_service_auth_status(&pool, "qobuz", Some(account_id))
        .await
        .unwrap();

    assert_eq!(status.status, "connected_valid");
    assert!(status.is_authenticated);
    assert_eq!(status.account_id, Some(account_id));
    assert_eq!(status.display_name, Some("Qobuz Hi-Res Fan".to_string()));
}

#[tokio::test]
async fn test_disabled_account_and_credentials_invalid_returns_requires_auth() {
    let pool = setup_test_db().await;

    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let creds = serde_json::json!({
        "access_token": "some_valid_looking_token"
    }).to_string();
    let encrypted = crypto::encrypt(&creds).unwrap();

    // 1. Inactive account
    let aid_inactive: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Inactive Account', ?, 0, 0) RETURNING id"#
    )
    .bind(tidal_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status_inactive = perform_get_service_auth_status(&pool, "tidal", Some(aid_inactive)).await.unwrap();
    assert_eq!(status_inactive.status, "requires_auth");
    assert!(!status_inactive.is_authenticated);

    // 2. credentials_invalid = 1
    let aid_invalid: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active, credentials_invalid, invalid_reason)
           VALUES (?, 'Invalid Account', ?, 1, 1, 'HTTP 401 Unauthorized') RETURNING id"#
    )
    .bind(tidal_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status_invalid = perform_get_service_auth_status(&pool, "tidal", Some(aid_invalid)).await.unwrap();
    assert_eq!(status_invalid.status, "requires_auth");
    assert!(!status_invalid.is_authenticated);
    assert!(status_invalid.error_message.unwrap().contains("HTTP 401"));
}

#[tokio::test]
async fn test_reset_database_preserves_accounts_and_credentials() {
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let valid_creds = serde_json::json!({
        "user_auth_token": "preserved_qobuz_token_abc"
    }).to_string();
    let encrypted = crypto::encrypt(&valid_creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Preserved Qobuz', 'preserve@example.com', ?, 1, 0) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert dummy library entries and playlists
    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Test Track') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1)")
        .bind(account_id).bind(track_id).execute(&pool).await.unwrap();

    // Execute perform_reset_database directly
    let msg = perform_reset_database(&pool).await.unwrap();
    assert!(msg.contains("Accounts and settings were preserved"));

    // Verify library data was cleared
    let tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let entries_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries").fetch_one(&pool).await.unwrap();
    assert_eq!(tracks_count, 0);
    assert_eq!(entries_count, 0);

    // Verify account and token are intact and still valid
    let status_after_reset = perform_get_service_auth_status(&pool, "qobuz", Some(account_id))
        .await
        .unwrap();
    assert_eq!(status_after_reset.status, "connected_valid");
    assert!(status_after_reset.is_authenticated);
    assert_eq!(status_after_reset.account_id, Some(account_id));
}
