use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use syncify_tauri_lib::crypto;

async fn setup_test_db() -> SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);

    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to create in-memory SQLite pool");

    // Initialize required schema
    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            supports_download INTEGER DEFAULT 0,
            max_quality TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        INSERT INTO services (id, name, supports_download, max_quality) VALUES 
            (1, 'spotify', 0, 'lossy'),
            (2, 'qobuz', 1, 'hires'),
            (3, 'tidal', 1, 'hires'),
            (4, 'deezer', 1, 'lossless');

        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            service_id INTEGER NOT NULL,
            display_name TEXT,
            email TEXT,
            is_active INTEGER DEFAULT 1,
            credentials_json TEXT,
            last_synced TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            credentials_invalid INTEGER DEFAULT 0,
            invalid_reason TEXT,
            last_auth_error TEXT,
            FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE
        );

        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            duration_ms INTEGER,
            isrc TEXT,
            track_number INTEGER,
            disc_number INTEGER DEFAULT 1,
            album_id INTEGER,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            service_id INTEGER,
            service_name TEXT,
            service_track_id TEXT,
            service_album_id TEXT,
            target_title TEXT,
            target_artist TEXT,
            target_album TEXT,
            target_isrc TEXT,
            quality_preference TEXT,
            status TEXT NOT NULL DEFAULT 'queued',
            priority INTEGER NOT NULL DEFAULT 0,
            progress_percent REAL NOT NULL DEFAULT 0.0,
            bytes_downloaded INTEGER DEFAULT 0,
            total_bytes INTEGER DEFAULT 0,
            error_message TEXT,
            last_error TEXT,
            retry_count INTEGER NOT NULL DEFAULT 0,
            position INTEGER DEFAULT 0,
            resumable INTEGER DEFAULT 1,
            smart_studio_origin INTEGER NOT NULL DEFAULT 0,
            allow_fallback INTEGER NOT NULL DEFAULT 0,
            staging_path TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            started_at TEXT,
            completed_at TEXT,
            FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize test schema");

    pool
}

#[tokio::test]
async fn test_account_invalidation_on_auth_failure() {
    let pool = setup_test_db().await;

    // 1. Insert an active Qobuz account
    let creds = serde_json::json!({
        "user_auth_token": "expired_old_token_12345",
        "username": "test_user@syncify.io"
    });
    let encrypted = crypto::encrypt(&creds.to_string()).expect("Failed to encrypt");

    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, email, credentials_json, credentials_invalid) VALUES (2, 'Qobuz Test User', 'test@syncify.io', ?, 0)"
    )
    .bind(&encrypted)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Insert a queued track for Qobuz
    sqlx::query(
        r#"
        INSERT INTO tracks (id, title, isrc) VALUES (101, 'Test Qobuz Song', 'USWB12403464');
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, status, allow_fallback)
        VALUES (501, 101, 'qobuz', '12345678', 'queued', 0);
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    // 3. Simulate worker detecting RequiresAuth (HTTP 401)
    let auth_error = "Qobuz track/get failed for ID 12345678: HTTP 401 Unauthorized";
    let final_status = "requires_auth";

    // Mark permanent failure in queue
    sqlx::query(
        "UPDATE download_queue SET status = ?, last_error = ?, error_message = ? WHERE id = 501"
    )
    .bind(final_status)
    .bind(auth_error)
    .bind(auth_error)
    .execute(&pool)
    .await
    .unwrap();

    // Invalidate account in SQLite
    sqlx::query(
        r#"
        UPDATE accounts 
        SET credentials_invalid = 1,
            invalid_reason = 'token_expired',
            last_auth_error = ?
        WHERE service_id IN (SELECT id FROM services WHERE LOWER(name) = 'qobuz')
        "#
    )
    .bind(auth_error)
    .execute(&pool)
    .await
    .unwrap();

    // 4. Verify account state in DB
    let (invalid, reason, last_err): (i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT credentials_invalid, invalid_reason, last_auth_error FROM accounts WHERE service_id = 2"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(invalid, 1);
    assert_eq!(reason.as_deref(), Some("token_expired"));
    assert!(last_err.unwrap().contains("HTTP 401 Unauthorized"));

    // 5. Verify queue state
    let q_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = 501")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(q_status, "requires_auth");
}

#[tokio::test]
async fn test_reauth_and_auto_queue_recovery() {
    let pool = setup_test_db().await;

    // 1. Insert an invalid/expired account
    let old_creds = serde_json::json!({
        "user_auth_token": "expired_token_abc",
        "username": "user@qobuz.com"
    });
    let old_encrypted = crypto::encrypt(&old_creds.to_string()).unwrap();

    sqlx::query(
        r#"
        INSERT INTO accounts (service_id, display_name, email, credentials_json, credentials_invalid, invalid_reason, last_auth_error)
        VALUES (2, 'Old Qobuz User', 'user@qobuz.com', ?, 1, 'token_expired', 'HTTP 401 Unauthorized')
        "#
    )
    .bind(&old_encrypted)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Insert tracks that were stuck in requires_auth / failed
    sqlx::query(
        r#"
        INSERT INTO tracks (id, title) VALUES (201, 'Track 1'), (202, 'Track 2');
        INSERT INTO download_queue (id, track_id, service_name, status, last_error) VALUES
            (601, 201, 'qobuz', 'requires_auth', 'HTTP 401 Unauthorized'),
            (602, 202, 'qobuz', 'failed', 'HTTP 401 Unauthorized');
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    // 3. Simulate Re-Authentication (new fresh token)
    let new_token = "fresh_valid_qobuz_user_auth_token_999";
    let new_creds = serde_json::json!({
        "user_auth_token": new_token,
        "auth_token": new_token,
        "username": "user@qobuz.com"
    });
    let new_encrypted = crypto::encrypt(&new_creds.to_string()).unwrap();

    // Perform UPDATE account (simulating start_auth_and_save)
    sqlx::query(
        r#"
        UPDATE accounts
        SET display_name = 'Re-authenticated Qobuz User',
            email = 'user@qobuz.com',
            credentials_json = ?,
            credentials_invalid = 0,
            invalid_reason = NULL,
            last_auth_error = NULL,
            is_active = 1,
            last_synced = CURRENT_TIMESTAMP
        WHERE service_id = 2
        "#
    )
    .bind(&new_encrypted)
    .execute(&pool)
    .await
    .unwrap();

    // Perform auto-requeue of failed downloads for Qobuz
    let requeued = sqlx::query(
        r#"
        UPDATE download_queue
        SET status = 'queued',
            last_error = NULL,
            error_message = NULL,
            retry_count = 0,
            started_at = NULL,
            completed_at = NULL
        WHERE status IN ('requires_auth', 'failed')
          AND (LOWER(service_name) = 'qobuz' OR service_name IS NULL)
        "#
    )
    .execute(&pool)
    .await
    .unwrap()
    .rows_affected();

    assert_eq!(requeued, 2);

    // 4. Verify account credentials decrypted cleanly with new token
    let (enc_json, invalid_flag, reason): (String, i64, Option<String>) = sqlx::query_as(
        "SELECT credentials_json, credentials_invalid, invalid_reason FROM accounts WHERE service_id = 2"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(invalid_flag, 0);
    assert!(reason.is_none());

    let decrypted = crypto::decrypt(&enc_json).expect("Decryption failed");
    let parsed: serde_json::Value = serde_json::from_str(&decrypted).unwrap();
    assert_eq!(parsed["user_auth_token"].as_str().unwrap(), new_token);

    // 5. Verify all queue items transitioned from requires_auth/failed -> queued
    let statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM download_queue ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(statuses, vec!["queued".to_string(), "queued".to_string()]);
}
