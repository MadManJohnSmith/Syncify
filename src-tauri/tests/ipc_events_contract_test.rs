//! TASK-118: IPC Events Contract & Symmetry Test
//!
//! Validates event naming alignment and payloads between backend emitters and frontend listeners.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    mark_account_credentials_invalid, SyncCallback, SyncProgressEmitter, SyncProgressEvent,
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
async fn test_mark_account_credentials_invalid_flips_flag_and_runs() {
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let encrypted_dummy = crypto::encrypt(r#"{"user_auth_token":"test-token"}"#).unwrap();

    sqlx::query(
        "INSERT INTO accounts (service_id, credentials_json, is_active, credentials_invalid) VALUES (?, ?, 1, 0)"
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted_dummy)
    .execute(&pool)
    .await
    .unwrap();

    // Call mark_account_credentials_invalid
    let affected = mark_account_credentials_invalid(&pool, "qobuz", "HTTP 401: Invalid session token")
        .await
        .expect("Should mark credentials invalid");

    assert_eq!(affected, 1, "Should affect 1 account row");

    let invalid: i64 = sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE service_id = ?")
        .bind(qobuz_svc_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(invalid, 1, "Account should have credentials_invalid = 1");
}

#[test]
fn test_sync_progress_event_terminal_failure_contract() {
    let failed_evt = SyncProgressEvent::failed("qobuz", Some(1), "error", "Network timed out", 10, 5);
    assert_eq!(failed_evt.service, "qobuz");
    assert_eq!(failed_evt.status, "failed");
    assert!(failed_evt.terminal);
    assert_eq!(failed_evt.message, "Network timed out");
    assert_eq!(failed_evt.imported_tracks_total, 10);
    assert_eq!(failed_evt.favorite_tracks_total, 5);
}

#[test]
fn test_sync_progress_event_requires_auth_contract() {
    let auth_evt = SyncProgressEvent::requires_auth("tidal", Some(2), "RequiresAuth: Token expired");
    assert_eq!(auth_evt.service, "tidal");
    assert_eq!(auth_evt.status, "requires_auth");
    assert!(auth_evt.terminal);
    assert_eq!(auth_evt.message, "RequiresAuth: Token expired");
}

#[test]
fn test_closure_emitter_receives_expected_payload() {
    use std::sync::{Arc, Mutex};
    let received = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();

    let emitter = SyncCallback(move |evt: &SyncProgressEvent| {
        received_clone.lock().unwrap().push(evt.clone());
    });

    let evt = SyncProgressEvent::failed("spotify", Some(3), "rate_limited", "Rate limited", 0, 0);
    emitter.emit_sync_progress(&evt);

    let list = received.lock().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].service, "spotify");
    assert_eq!(list[0].status, "failed");
}
