//! account_credential_state_ipc_test.rs
//!
//! SECURITY contract for the "masked stored credential state" the Accounts UI
//! renders for imported accounts.
//!
//! The UI needs to tell the user honestly WHAT an imported account has stored
//! (a password, a browser session cookie, an OAuth session, or nothing) — that
//! is the difference between "reconnect works again automatically" and "you
//! must log in in a browser every time". It must do so WITHOUT ever revealing
//! the credential: no password, cookie or token may appear in the IPC payload.
//!
//! Asserts that:
//! 1. `get_accounts` never serializes any credential VALUE.
//! 2. It never serializes the credential KEY NAMES either, so the payload shape
//!    itself discloses nothing about which provider secret is held.
//! 3. A cookie-only import is reported as `cookie` and claims no password.
//! 4. Adding the state does not break the existing secrets-free AccountInfo
//!    contract (email/display_name are still present, they are identity).

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::perform_get_accounts;
use syncify_tauri_lib::commands::types::StoredCredentialKind;
use syncify_tauri_lib::crypto;

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([7u8; 32]);

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

/// Every secret-looking string the fixture accounts hold. None of these may
/// appear in the serialized `get_accounts` response.
const SECRET_VALUES: &[&str] = &[
    "arla0b1c2d3e4f5a6b7c8d9e0f1a2b3c",
    "sp_dc_cookie_value_1234567890",
    "user_auth_token_value_abcdefghij",
    "access_secret_value_aaaaaaaaaaaa",
    "refresh_secret_value_bbbbbbbbbbbb",
    "super_secret_password_value",
    "developer_token_value_cccccccc",
    "music_user_token_value_dddddddd",
];

async fn insert_account(
    pool: &sqlx::SqlitePool,
    service: &str,
    display_name: &str,
    email: Option<&str>,
    creds: serde_json::Value,
) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .unwrap();
    let encrypted = crypto::encrypt(&creds.to_string()).unwrap();
    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, is_active, credentials_invalid)
           VALUES (?, ?, ?, ?, 1, 0) RETURNING id"#,
    )
    .bind(service_id)
    .bind(display_name)
    .bind(email)
    .bind(&encrypted)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn test_get_accounts_payload_contains_no_credential_value_or_key_name() {
    let pool = setup_test_db().await;

    // One account per credential shape the backend can actually store.
    insert_account(
        &pool,
        "deezer",
        "Deezer User",
        None,
        serde_json::json!({
            "arl": SECRET_VALUES[0],
            "access_token": SECRET_VALUES[0],
        }),
    )
    .await;
    insert_account(
        &pool,
        "spotify",
        "Spotify User",
        Some("ana@spotify.com"),
        serde_json::json!({
            "sp_dc": SECRET_VALUES[1],
            "access_token": SECRET_VALUES[3],
            "refresh_token": SECRET_VALUES[4],
        }),
    )
    .await;
    insert_account(
        &pool,
        "qobuz",
        "Qobuz User",
        Some("ana@example.com"),
        serde_json::json!({
            "user_auth_token": SECRET_VALUES[2],
            "username": "ana@example.com",
            "password": SECRET_VALUES[5],
        }),
    )
    .await;
    insert_account(
        &pool,
        "apple_music",
        "Apple Music User",
        None,
        serde_json::json!({
            "developer_token": SECRET_VALUES[6],
            "music_user_token": SECRET_VALUES[7],
        }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    assert_eq!(accounts.len(), 4, "every imported account must be listed");

    let serialized = serde_json::to_string(&accounts).unwrap();
    let lower = serialized.to_ascii_lowercase();

    for value in SECRET_VALUES {
        assert!(
            !serialized.contains(value),
            "credential value '{}…' must never reach the IPC payload",
            &value[..8]
        );
    }

    // Not just values: the key NAMES must not leak either, so the payload shape
    // itself says nothing about which provider secret is held.
    for key in [
        "arl",
        "sp_dc",
        "user_auth_token",
        "access_token",
        "refresh_token",
        "\"password\"",
        "developer_token",
        "music_user_token",
        "credentials_json",
        "username",
    ] {
        assert!(
            !lower.contains(&format!("\"{}\"", key)),
            "credential key name '{}' must never appear in the IPC payload",
            key
        );
    }
}

#[tokio::test]
async fn test_cookie_only_import_claims_no_password_and_no_auto_renewal() {
    let pool = setup_test_db().await;

    let deezer = insert_account(
        &pool,
        "deezer",
        "browser_session",
        None,
        serde_json::json!({
            "arl": SECRET_VALUES[0],
            "access_token": SECRET_VALUES[0],
        }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == deezer)
        .expect("deezer account row");

    assert_eq!(account.credential_state.kind, StoredCredentialKind::Cookie);
    assert!(account.credential_state.has_cookie);
    assert!(
        !account.credential_state.has_password,
        "a cookie-only import must never claim a password is stored — that would
         imply the password is recoverable, which it is not"
    );
    assert!(
        !account.credential_state.auto_refreshable,
        "a browser session cookie cannot be renewed by the app"
    );
}

#[tokio::test]
async fn test_password_account_reports_the_kind_but_never_the_value() {
    let pool = setup_test_db().await;

    let qobuz = insert_account(
        &pool,
        "qobuz",
        "Qobuz User",
        Some("ana@example.com"),
        serde_json::json!({
            "user_auth_token": SECRET_VALUES[2],
            "username": "ana@example.com",
            "password": SECRET_VALUES[5],
        }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    let account = accounts
        .iter()
        .find(|a| a.id == qobuz)
        .expect("qobuz account row");

    // The UI can say a password is on file…
    assert!(account.credential_state.has_password);
    assert_eq!(account.credential_state.kind, StoredCredentialKind::Mixed);
    // …and the payload proves the value itself never travelled.
    let serialized = serde_json::to_string(&account).unwrap();
    assert!(!serialized.contains(SECRET_VALUES[5]));
    assert!(!serialized.contains(SECRET_VALUES[2]));
}

#[tokio::test]
async fn test_get_accounts_still_exposes_identity_fields() {
    let pool = setup_test_db().await;

    insert_account(
        &pool,
        "spotify",
        "Ana Spotify",
        Some("ana@spotify.com"),
        serde_json::json!({ "access_token": SECRET_VALUES[3] }),
    )
    .await;

    let accounts = perform_get_accounts(&pool).await.unwrap();
    assert_eq!(accounts[0].display_name.as_deref(), Some("Ana Spotify"));
    assert_eq!(accounts[0].email.as_deref(), Some("ana@spotify.com"));
    assert!(accounts[0].is_active);
    // Adding the credential summary must not resurrect a credentials field.
    assert!(!serde_json::to_string(&accounts[0])
        .unwrap()
        .contains(SECRET_VALUES[3]));
}
