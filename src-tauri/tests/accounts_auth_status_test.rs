use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::types::{StoredCredentialKind, StoredCredentialState};
use syncify_tauri_lib::commands::{
    perform_get_accounts, perform_get_service_auth_status, perform_reset_database,
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
    })
    .to_string();
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
    })
    .to_string();
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
    })
    .to_string();
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
async fn test_disabled_account_keeps_its_real_status_and_invalid_still_requires_auth() {
    let pool = setup_test_db().await;

    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let creds = serde_json::json!({
        "access_token": "some_valid_looking_token"
    })
    .to_string();
    let encrypted = crypto::encrypt(&creds).unwrap();

    // 1. Inactive account, queried BY ID.
    //    Multi-cuenta: desactivarse ya NO es un problema de credenciales. El
    //    corte `is_active == 0 → requires_auth` solo aplica al camino SIN id
    //    (además dead code allí: su consulta solo trae filas activas), así que la
    //    cuenta ELEGIDA se evalúa con sus credenciales reales y `is_active` en el
    //    DTO dice que no es la activa del servicio.
    let aid_inactive: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Inactive Account', ?, 0, 0) RETURNING id"#
    )
    .bind(tidal_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status_inactive = perform_get_service_auth_status(&pool, "tidal", Some(aid_inactive))
        .await
        .unwrap();
    assert_eq!(status_inactive.status, "connected_valid");
    assert!(status_inactive.is_authenticated);
    assert!(
        !status_inactive.is_active,
        "el estado distingue 'credenciales sanas' de 'cuenta activa'"
    );

    // Y el camino SIN id sigue sin devolver esa cuenta (su consulta filtra
    // is_active = 1): sin cuenta activa el servicio no está conectado.
    let status_no_id = perform_get_service_auth_status(&pool, "tidal", None)
        .await
        .unwrap();
    assert_eq!(status_no_id.status, "missing");
    assert!(status_no_id.account_id.is_none());

    // 2. credentials_invalid = 1 — sigue siendo requires_auth por credenciales.
    let aid_invalid: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active, credentials_invalid, invalid_reason)
           VALUES (?, 'Invalid Account', ?, 1, 1, 'HTTP 401 Unauthorized') RETURNING id"#
    )
    .bind(tidal_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status_invalid = perform_get_service_auth_status(&pool, "tidal", Some(aid_invalid))
        .await
        .unwrap();
    assert_eq!(status_invalid.status, "requires_auth");
    assert!(!status_invalid.is_authenticated);
    assert!(status_invalid.is_active);
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
    })
    .to_string();
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
    let _artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Test Track') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1)")
        .bind(account_id)
        .bind(track_id)
        .execute(&pool)
        .await
        .unwrap();

    // Execute perform_reset_database directly
    let msg = perform_reset_database(&pool).await.unwrap();
    assert!(msg.contains("Accounts and settings were preserved"));

    // Verify library data was cleared
    let tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    let entries_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
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

// ==============================================
// STORED CREDENTIAL STATE (secret-free only)
// ==============================================

async fn insert_account_with_creds(
    pool: &sqlx::SqlitePool,
    service: &str,
    display_name: &str,
    creds: serde_json::Value,
) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .unwrap();
    let encrypted = crypto::encrypt(&creds.to_string()).unwrap();
    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active, credentials_invalid)
           VALUES (?, ?, ?, 1, 0) RETURNING id"#,
    )
    .bind(service_id)
    .bind(display_name)
    .bind(&encrypted)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn test_cookie_only_account_reports_a_cookie_kind_without_revealing_it() {
    let pool = setup_test_db().await;
    // Deezer only ever captures an ARL cookie: there is no password to recover.
    let secret_arl = "<SECRET_64fbed9b>";
    let account_id = insert_account_with_creds(
        &pool,
        "deezer",
        "Deezer User",
        serde_json::json!({ "arl": secret_arl, "access_token": secret_arl }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .expect("account row");

    assert_eq!(account.credential_state.kind, StoredCredentialKind::Cookie);
    assert!(account.credential_state.has_cookie);
    assert!(
        !account.credential_state.has_password,
        "a cookie import must never claim a password is stored"
    );
    assert!(!account.credential_state.auto_refreshable);
    assert!(account.credential_state.has_credential);

    // SECURITY: the serialized payload must not contain the credential value.
    let serialized = serde_json::to_string(&account).unwrap();
    assert!(
        !serialized.contains(&secret_arl),
        "credential material must never reach the IPC payload"
    );
    assert!(!serialized.contains("arl"));
}

#[tokio::test]
async fn test_password_only_account_reports_a_password_kind() {
    let pool = setup_test_db().await;
    let account_id = insert_account_with_creds(
        &pool,
        "qobuz",
        "Qobuz User",
        serde_json::json!({
            "username": "ana@example.com",
            "password": "super-secret-password"
        }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .expect("account row");

    assert_eq!(
        account.credential_state.kind,
        StoredCredentialKind::Password
    );
    assert!(account.credential_state.has_password);
    assert!(!account.credential_state.has_cookie);
    assert!(!account.credential_state.auto_refreshable);

    let serialized = serde_json::to_string(&account).unwrap();
    assert!(!serialized.contains("super-secret-password"));
    assert!(!serialized.contains("password\":"));
}

#[tokio::test]
async fn test_oauth_account_reports_auto_refreshable() {
    let pool = setup_test_db().await;
    let account_id = insert_account_with_creds(
        &pool,
        "spotify",
        "Spotify User",
        serde_json::json!({
            "token_type": "<SECRET_e47c8c25>",
            "access_token": "<SECRET_140872ef>",
            "refresh_token": "refresh_secret_value",
            "expires_at": 1_800_000_000
        }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .expect("account row");

    assert_eq!(account.credential_state.kind, StoredCredentialKind::Oauth);
    assert!(
        account.credential_state.auto_refreshable,
        "an OAuth refresh token means the app can renew without the user"
    );

    let serialized = serde_json::to_string(&account).unwrap();
    assert!(!serialized.contains("access_secret_value"));
    assert!(!serialized.contains("refresh_secret_value"));
}

#[tokio::test]
async fn test_account_without_credentials_reports_no_credential() {
    let pool = setup_test_db().await;
    let account_id = insert_account_with_creds(
        &pool,
        "tidal",
        "Tidal User",
        serde_json::json!({ "user_id": "1234567" }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .expect("account row");

    assert_eq!(account.credential_state.kind, StoredCredentialKind::None);
    assert!(!account.credential_state.has_credential);
    assert_eq!(account.credential_state, StoredCredentialState::default());
}

#[tokio::test]
async fn test_account_with_null_credentials_reports_no_credential() {
    let pool = setup_test_db().await;
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'soundcloud'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'SoundCloud User', NULL, 1, 0) RETURNING id"#,
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .expect("account row");

    assert_eq!(account.credential_state.kind, StoredCredentialKind::None);
    assert!(!account.credential_state.has_credential);
}

#[tokio::test]
async fn test_token_plus_fallback_password_reports_mixed_and_no_secret_leak() {
    let pool = setup_test_db().await;
    let account_id = insert_account_with_creds(
        &pool,
        "qobuz",
        "Qobuz User",
        serde_json::json!({
            "user_auth_token": "user_auth_token_value_1234567890",
            "username": "ana@example.com",
            "password": "fallback_password_value"
        }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .expect("account row");

    assert_eq!(account.credential_state.kind, StoredCredentialKind::Mixed);
    assert!(account.credential_state.has_password);
    assert!(account.credential_state.has_token);
    assert!(account.credential_state.has_credential);

    let serialized = serde_json::to_string(&account).unwrap();
    assert!(!serialized.contains("user_auth_token_value_1234567890"));
    assert!(!serialized.contains("fallback_password_value"));
}

#[tokio::test]
async fn test_corrupted_credentials_do_not_break_account_listing() {
    let pool = setup_test_db().await;
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Tidal User', 'not-a-ciphertext', 1, 0) RETURNING id"#,
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == account_id)
        .expect("account row");

    assert_eq!(account.credential_state.kind, StoredCredentialKind::None);
    assert!(!account.credential_state.has_credential);
}
