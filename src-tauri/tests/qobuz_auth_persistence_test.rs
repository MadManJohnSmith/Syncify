/// S127B — Qobuz auth token persistence & 401 recovery integration tests
///
/// Tests the complete flow:
///   1. Fresh Qobuz login saves `user_auth_token` and resolves to `connected_valid`
///   2. HTTP 401 mid-flight marks `credentials_invalid = 1` and resolves to `requires_auth`
///   3. Re-login resets the flag and resolves to `connected_valid` again
///   4. A token stored only as `auth_token` (not `user_auth_token`) still resolves correctly
///   5. `mark_account_credentials_invalid` with no account returns 0 rows without error
///   6. Marking Qobuz invalid does NOT touch accounts from other services (e.g. Spotify)

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    mark_account_credentials_invalid, perform_get_service_auth_status,
};
use syncify_tauri_lib::crypto;

// ── Helpers ────────────────────────────────────────────────────────────────

async fn setup_test_db() -> sqlx::SqlitePool {
    // Crypto must be initialised before any encrypt/decrypt call.
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

/// Insert a Qobuz account whose credentials contain `user_auth_token`.
/// Returns the inserted `account_id`.
async fn insert_qobuz_account_with_token(
    pool: &sqlx::SqlitePool,
    token: &str,
    display_name: &str,
) -> i64 {
    let qobuz_svc_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
            .fetch_one(pool)
            .await
            .expect("qobuz service row must exist");

    let creds = serde_json::json!({
        "user_auth_token": token,
        "auth_token": token,
        "user_id": "9876543"
    })
    .to_string();

    let encrypted = crypto::encrypt(&creds).expect("encryption must succeed");

    sqlx::query_scalar(
        r#"
        INSERT INTO accounts
            (service_id, display_name, credentials_json, credentials_invalid, is_active, created_at)
        VALUES (?, ?, ?, 0, 1, CURRENT_TIMESTAMP)
        RETURNING id
        "#,
    )
    .bind(qobuz_svc_id)
    .bind(display_name)
    .bind(&encrypted)
    .fetch_one(pool)
    .await
    .expect("account insertion must succeed")
}

// ── Tests ──────────────────────────────────────────────────────────────────

/// After a fresh Qobuz login the stored token resolves to `connected_valid`.
#[tokio::test]
async fn test_fresh_qobuz_login_token_resolves_connected_valid() {
    let pool = setup_test_db().await;

    let viable_token = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4"; // 32-char alphanumeric
    insert_qobuz_account_with_token(&pool, viable_token, "Test User").await;

    let status = perform_get_service_auth_status(&pool, "qobuz", None)
        .await
        .expect("auth status call must not fail");

    assert_eq!(status.service, "qobuz");
    assert_eq!(
        status.status, "connected_valid",
        "valid token should yield connected_valid, got: {}",
        status.status
    );
    assert!(status.is_authenticated);
    assert!(status.account_id.is_some());
}

/// After a 401 mid-flight `mark_account_credentials_invalid` flips the flag,
/// and auth status then returns `requires_auth`.
#[tokio::test]
async fn test_401_marks_credentials_invalid_and_requires_auth() {
    let pool = setup_test_db().await;

    let viable_token = "b1c2d3e4f5a6b1c2d3e4f5a6b1c2d3e4";
    insert_qobuz_account_with_token(&pool, viable_token, "Auth Test User").await;

    // Confirm we start as connected_valid
    let before = perform_get_service_auth_status(&pool, "qobuz", None)
        .await
        .unwrap();
    assert_eq!(before.status, "connected_valid");

    // Simulate HTTP 401 received mid-flight
    let rows = mark_account_credentials_invalid(
        &pool,
        "qobuz",
        "HTTP 401: User authentication required",
    )
    .await
    .expect("marking invalid must not fail");
    assert_eq!(rows, 1, "exactly one row should have been updated");

    // Auth status must now be requires_auth
    let after = perform_get_service_auth_status(&pool, "qobuz", None)
        .await
        .unwrap();
    assert_eq!(
        after.status, "requires_auth",
        "after 401 status must be requires_auth, got: {}",
        after.status
    );
    assert!(!after.is_authenticated);
}

/// Re-login after a 401: updating `credentials_invalid = 0` and writing fresh
/// credentials restores `connected_valid`.
#[tokio::test]
async fn test_relogin_after_401_restores_connected_valid() {
    let pool = setup_test_db().await;

    let initial_token = "c1d2e3f4a5b6c1d2e3f4a5b6c1d2e3f4";
    let account_id = insert_qobuz_account_with_token(&pool, initial_token, "ReLogin User").await;

    // Simulate 401
    mark_account_credentials_invalid(
        &pool,
        "qobuz",
        "HTTP 401: User authentication required",
    )
    .await
    .unwrap();

    // Simulate re-login: write new token and clear credentials_invalid
    let new_token = "d1e2f3a4b5c6d1e2f3a4b5c6d1e2f3a4";
    let new_creds = serde_json::json!({
        "user_auth_token": new_token,
        "auth_token": new_token,
        "user_id": "9876543"
    })
    .to_string();
    let new_encrypted = crypto::encrypt(&new_creds).unwrap();

    sqlx::query(
        r#"
        UPDATE accounts
        SET credentials_json     = ?,
            credentials_invalid  = 0,
            invalid_reason       = NULL,
            last_auth_error      = NULL,
            is_active            = 1
        WHERE id = ?
        "#,
    )
    .bind(&new_encrypted)
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("re-login update must succeed");

    // After re-login auth status must be connected_valid
    let after_relogin = perform_get_service_auth_status(&pool, "qobuz", None)
        .await
        .unwrap();
    assert_eq!(
        after_relogin.status, "connected_valid",
        "after re-login status should be connected_valid, got: {}",
        after_relogin.status
    );
    assert!(after_relogin.is_authenticated);
}

/// Token stored only as `auth_token` (not `user_auth_token`) must also resolve
/// correctly — covers the multi-field fallback in `perform_get_service_auth_status`.
#[tokio::test]
async fn test_token_stored_as_auth_token_field_resolves_connected_valid() {
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Store token only under `auth_token`, not `user_auth_token`
    let creds = serde_json::json!({
        "auth_token": "e1f2a3b4c5d6e1f2a3b4c5d6e1f2a3b4",
        "user_id": "1111111"
    })
    .to_string();
    let encrypted = crypto::encrypt(&creds).unwrap();

    sqlx::query(
        r#"
        INSERT INTO accounts
            (service_id, display_name, credentials_json, credentials_invalid, is_active, created_at)
        VALUES (?, 'Alt Token User', ?, 0, 1, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .execute(&pool)
    .await
    .unwrap();

    let status = perform_get_service_auth_status(&pool, "qobuz", None)
        .await
        .unwrap();

    assert_eq!(
        status.status, "connected_valid",
        "auth_token fallback must yield connected_valid, got: {}",
        status.status
    );
    assert!(status.is_authenticated);
}

/// `mark_account_credentials_invalid` with no active account returns 0 rows
/// and does NOT error (no account to update is a valid empty state).
#[tokio::test]
async fn test_mark_invalid_with_no_account_returns_zero_rows() {
    let pool = setup_test_db().await;
    // No account inserted — should return 0, not an Err

    let rows = mark_account_credentials_invalid(
        &pool,
        "qobuz",
        "HTTP 401: User authentication required",
    )
    .await
    .expect("should not error even when no account exists");

    assert_eq!(rows, 0, "no rows updated when no active account exists");
}

/// `mark_account_credentials_invalid` must NOT touch accounts from other services.
#[tokio::test]
async fn test_mark_invalid_does_not_affect_other_services() {
    let pool = setup_test_db().await;

    // Insert Spotify account
    let spotify_svc_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE name = 'spotify'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let spotify_creds = serde_json::json!({
        "access_token":  "spotify_token_1234567890abcdef1234",
        "refresh_token": "refresh_tok_1234567890abcdef1234",
        "expires_at":    9_999_999_999i64,
    })
    .to_string();
    let enc_spotify = crypto::encrypt(&spotify_creds).unwrap();

    sqlx::query(
        r#"
        INSERT INTO accounts (service_id, display_name, credentials_json, credentials_invalid, is_active, created_at)
        VALUES (?, 'Spotify User', ?, 0, 1, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(spotify_svc_id)
    .bind(&enc_spotify)
    .execute(&pool)
    .await
    .unwrap();

    // Insert Qobuz account
    insert_qobuz_account_with_token(&pool, "f1a2b3c4d5e6f1a2b3c4d5e6f1a2b3c4", "Qobuz User").await;

    // Mark only Qobuz invalid
    mark_account_credentials_invalid(
        &pool,
        "qobuz",
        "HTTP 401: User authentication required",
    )
    .await
    .unwrap();

    // Spotify account must remain untouched
    let spotify_invalid: i64 = sqlx::query_scalar(
        "SELECT IFNULL(credentials_invalid, 0) FROM accounts WHERE service_id = ?",
    )
    .bind(spotify_svc_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(spotify_invalid, 0, "Spotify account must NOT be marked invalid");

    // Qobuz must be invalid
    let qobuz_status = perform_get_service_auth_status(&pool, "qobuz", None)
        .await
        .unwrap();
    assert_eq!(qobuz_status.status, "requires_auth");
    assert!(!qobuz_status.is_authenticated);
}
