//! Tests for TASK-78: Playlist Identity Correction & Collision Decoupling
//!
//! Verifies:
//! 1. `upsert_playlist_and_source` prioritizes remote identity `(account_id, service_playlist_id)` over name.
//! 2. Two playlists with the same name but different `service_playlist_id` create distinct playlists in DB.
//! 3. Re-importing a playlist with the same `service_playlist_id` reuses and updates the existing playlist.
//! 4. Empty/null `service_playlist_id` falls back to name matching (for manual local playlists).
//! 5. Migration 0075 cleanly decouples existing colliding playlists, updates `playlist_sources.playlist_id`,
//!    and enforces recurrence prevention via SQLite triggers.
//! 6. Database integrity: `PRAGMA foreign_key_check` and `PRAGMA integrity_check` pass with 0 errors.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::upsert_playlist_and_source;

async fn setup_fresh_test_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply all migrations");

    pool
}

async fn create_test_account(pool: &sqlx::SqlitePool, service_name: &str) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service_name)
        .fetch_one(pool)
        .await
        .expect("Service not found in test DB");

    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email) VALUES (?, 'Test User', 'test@example.com') RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("Failed to create test account")
}

#[tokio::test]
async fn test_different_service_playlist_id_creates_distinct_playlists() {
    let pool = setup_fresh_test_db().await;
    let account_id = create_test_account(&pool, "spotify").await;

    // 1. First playlist "Chill Vibes" with remote ID "sp_101"
    let pid1 = upsert_playlist_and_source(
        &pool,
        account_id,
        "sp_101",
        "Chill Vibes",
        Some("First playlist description"),
        Some("CuratorA"),
        1,
        0,
        Some("https://example.com/cover1.jpg"),
        25,
    )
    .await
    .expect("First upsert must succeed");

    assert!(pid1 > 0);

    // 2. Second playlist with SAME name but DIFFERENT remote ID "sp_102"
    let pid2 = upsert_playlist_and_source(
        &pool,
        account_id,
        "sp_102",
        "Chill Vibes",
        Some("Second playlist description"),
        Some("CuratorB"),
        1,
        0,
        Some("https://example.com/cover2.jpg"),
        30,
    )
    .await
    .expect("Second upsert must succeed");

    assert!(pid2 > 0);
    assert_ne!(
        pid1, pid2,
        "Playlists with different service_playlist_id must create distinct playlists (TASK-78)"
    );

    // Verify 2 rows in playlists table
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlists WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "Both playlists must exist independently in playlists table");

    // Verify playlist_sources records are cleanly separated
    let ps1: (i64, String) = sqlx::query_as(
        "SELECT playlist_id, service_playlist_id FROM playlist_sources WHERE playlist_id = ?",
    )
    .bind(pid1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ps1.0, pid1);
    assert_eq!(ps1.1, "sp_101");

    let ps2: (i64, String) = sqlx::query_as(
        "SELECT playlist_id, service_playlist_id FROM playlist_sources WHERE playlist_id = ?",
    )
    .bind(pid2)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ps2.0, pid2);
    assert_eq!(ps2.1, "sp_102");

    // Verify NO collision in playlist_sources
    let collisions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT playlist_id FROM playlist_sources GROUP BY playlist_id HAVING COUNT(*) > 1)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(collisions, 0, "No playlist_id should have multiple colliding sources");
}

#[tokio::test]
async fn test_same_service_playlist_id_reuses_and_updates() {
    let pool = setup_fresh_test_db().await;
    let account_id = create_test_account(&pool, "qobuz").await;

    // 1. Initial import
    let pid1 = upsert_playlist_and_source(
        &pool,
        account_id,
        "qobuz_pl_999",
        "Hi-Res Masters",
        Some("Initial description"),
        Some("QobuzTeam"),
        1,
        0,
        None,
        15,
    )
    .await
    .expect("Initial upsert must succeed");

    // 2. Re-import with changed title and metadata but SAME service_playlist_id
    let pid2 = upsert_playlist_and_source(
        &pool,
        account_id,
        "qobuz_pl_999",
        "Hi-Res Masters (Updated Edition)",
        Some("Updated description"),
        Some("QobuzTeam"),
        1,
        0,
        Some("https://example.com/new_cover.jpg"),
        18,
    )
    .await
    .expect("Re-import upsert must succeed");

    assert_eq!(
        pid1, pid2,
        "Re-import with matching service_playlist_id must reuse existing playlist ID"
    );

    // Verify metadata was updated
    let row: (String, Option<String>, i32) = sqlx::query_as(
        "SELECT name, description, track_count FROM playlists WHERE id = ?",
    )
    .bind(pid1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.0, "Hi-Res Masters (Updated Edition)");
    assert_eq!(row.1.as_deref(), Some("Updated description"));
    assert_eq!(row.2, 18);

    // Verify playlists count is still 1
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlists WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_empty_service_playlist_id_falls_back_to_name_matching() {
    let pool = setup_fresh_test_db().await;
    let account_id = create_test_account(&pool, "spotify").await;

    // 1. Local manual playlist (empty service_playlist_id)
    let pid1 = upsert_playlist_and_source(
        &pool,
        account_id,
        "",
        "My Roadtrip Jams",
        Some("Local custom playlist"),
        None,
        0,
        0,
        None,
        10,
    )
    .await
    .expect("First manual upsert must succeed");

    // 2. Second upsert with same name (differing case/whitespace) and empty service_playlist_id
    let pid2 = upsert_playlist_and_source(
        &pool,
        account_id,
        "   ",
        "  my roadtrip jams  ",
        Some("Updated local description"),
        None,
        0,
        0,
        None,
        12,
    )
    .await
    .expect("Second manual upsert must succeed");

    assert_eq!(
        pid1, pid2,
        "Empty service_playlist_id must fall back to name matching and reuse playlist ID"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlists WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "Only one playlist row should exist for manual playlist");
}

#[tokio::test]
async fn test_migration_0075_decoupling_and_triggers() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    // 1. Run migrations 0001 through 0074
    let migrator = sqlx::migrate!("./migrations");
    let migrations: Vec<_> = migrator.iter().collect();

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrations
                .iter()
                .filter(|m| m.version <= 74)
                .map(|m| (*m).clone())
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    partial_migrator
        .run(&pool)
        .await
        .expect("Run migrations through 0074");

    // 2. Create service and account
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email) VALUES (?, 'Qobuz Test', 'qobuz@test.com') RETURNING id",
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 3. Simulate historical bug state before migration 0075:
    // Create one primary playlist 'Arroba' (id=10)
    sqlx::query(
        r#"
        INSERT INTO playlists (id, account_id, service_playlist_id, name, track_count)
        VALUES (10, ?, '68251486', 'Arroba', 50)
        "#,
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .unwrap();

    // Also simulate another colliding playlist row already in playlists (id=20)
    sqlx::query(
        r#"
        INSERT INTO playlists (id, account_id, service_playlist_id, name, track_count)
        VALUES (20, ?, '68247576', 'Arroba', 30)
        "#,
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .unwrap();

    // In playlist_sources, the bug caused BOTH sources (and a 3rd source with no existing playlist row)
    // to point to playlist_id = 10:
    sqlx::query(
        r#"
        INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
        VALUES 
            (10, ?, ?, '68251486'),
            (10, ?, ?, '68247576'),
            (10, ?, ?, '68249999')
        "#,
    )
    .bind(account_id)
    .bind(service_id)
    .bind(account_id)
    .bind(service_id)
    .bind(account_id)
    .bind(service_id)
    .execute(&pool)
    .await
    .unwrap();

    // Pre-migration assertion: playlist_id=10 has 3 colliding sources
    let pre_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM playlist_sources WHERE playlist_id = 10",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pre_count, 3, "Pre-migration state must have 3 sources on playlist 10");

    // 4. Run migration 0075 (full migrator)
    let full_migrator = sqlx::migrate!("./migrations");
    full_migrator
        .run(&pool)
        .await
        .expect("Run migration 0075 cleanly");

    // 5. Post-migration verification:
    // a) No playlist_id in playlist_sources has more than 1 source
    let collisions_post: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT playlist_id FROM playlist_sources GROUP BY playlist_id HAVING COUNT(*) > 1)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(collisions_post, 0, "Post-migration collisions count must be exactly 0");

    // b) '68251486' still points to 10
    let pl_1: i64 = sqlx::query_scalar(
        "SELECT playlist_id FROM playlist_sources WHERE account_id = ? AND service_playlist_id = '68251486'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pl_1, 10);

    // c) '68247576' decoupled and points to 20
    let pl_2: i64 = sqlx::query_scalar(
        "SELECT playlist_id FROM playlist_sources WHERE account_id = ? AND service_playlist_id = '68247576'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pl_2, 20);

    // d) '68249999' was newly created in playlists and pointed to its new row
    let pl_3: i64 = sqlx::query_scalar(
        "SELECT playlist_id FROM playlist_sources WHERE account_id = ? AND service_playlist_id = '68249999'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(pl_3 != 10 && pl_3 != 20, "Newly created playlist must have a distinct ID");

    // Verify newly created playlist row metadata
    let new_pl_name: String = sqlx::query_scalar(
        "SELECT name FROM playlists WHERE id = ?",
    )
    .bind(pl_3)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(new_pl_name, "Arroba");

    // 6. Verification of structural and relational integrity
    let fk_violations: Vec<(String, i64, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(fk_violations.is_empty(), "PRAGMA foreign_key_check must return 0 violations");

    let integrity_check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(integrity_check, "ok", "PRAGMA integrity_check must be ok");

    // 7. Verify recurrence-prevention trigger:
    // Attempting to insert another colliding service_playlist_id pointing to pl_1 under account_id MUST fail
    let collision_attempt = sqlx::query(
        "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id) VALUES (?, ?, ?, 'illegal_collision')",
    )
    .bind(pl_1)
    .bind(account_id)
    .bind(service_id)
    .execute(&pool)
    .await;

    assert!(
        collision_attempt.is_err(),
        "Trigger must abort any future insertion of colliding playlist_sources on the same account"
    );
    let err_msg = collision_attempt.unwrap_err().to_string();
    assert!(
        err_msg.contains("Collision detected"),
        "Error message must indicate collision prevention: {}",
        err_msg
    );
}
