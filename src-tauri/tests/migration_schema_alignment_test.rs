//! Tests for TASK-123: Schema Alignment for Migration Commands
//! Verifies:
//! 1. Migration 0067 runs cleanly on a fresh database.
//! 2. `library_items` table exists with all required columns and triggers.
//! 3. `migration_items.dest_track_id` exists and is synchronized.
//! 4. `playlists.external_id` and `playlists.source_service` exist.
//! 5. `accounts.credentials` and `accounts.service_name` exist.
//! 6. Exact queries from `src-tauri/src/commands/migration.rs` execute successfully without SQL errors.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn setup_migrated_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations 0001..=0067");

    pool
}

#[tokio::test]
async fn test_migration_0067_version_and_columns_exist() {
    let pool = setup_migrated_db().await;

    // Verify migration version is at least 67
    let (max_version,): (Option<i64>,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("Failed to query migration version");
    assert!(
        max_version.unwrap_or(0) >= 67,
        "Migration version must be >= 67, got {:?}",
        max_version
    );

    // Verify library_items columns
    let lib_columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(library_items)")
            .fetch_all(&pool)
            .await
            .expect("Failed to get library_items columns");
    let lib_col_names: Vec<String> = lib_columns.into_iter().map(|c| c.1).collect();

    for expected in &[
        "id",
        "service",
        "source_service",
        "item_type",
        "external_id",
        "title",
        "artist",
        "album",
        "duration_ms",
        "quality",
        "raw_json",
        "synced_at",
        "created_at",
        "updated_at",
    ] {
        assert!(
            lib_col_names.contains(&expected.to_string()),
            "library_items must contain column '{}', existing: {:?}",
            expected,
            lib_col_names
        );
    }

    // Verify migration_items columns include dest_track_id
    let mig_columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(migration_items)")
            .fetch_all(&pool)
            .await
            .expect("Failed to get migration_items columns");
    let mig_col_names: Vec<String> = mig_columns.into_iter().map(|c| c.1).collect();
    assert!(
        mig_col_names.contains(&"dest_track_id".to_string()),
        "migration_items must contain dest_track_id column"
    );
    assert!(
        mig_col_names.contains(&"destination_track_id".to_string()),
        "migration_items must contain destination_track_id column"
    );

    // Verify playlists columns include external_id and source_service
    let pl_columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(playlists)")
            .fetch_all(&pool)
            .await
            .expect("Failed to get playlists columns");
    let pl_col_names: Vec<String> = pl_columns.into_iter().map(|c| c.1).collect();
    assert!(
        pl_col_names.contains(&"external_id".to_string()),
        "playlists must contain external_id column"
    );
    assert!(
        pl_col_names.contains(&"source_service".to_string()),
        "playlists must contain source_service column"
    );

    // Verify accounts columns include credentials and service_name
    let acc_columns: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(accounts)")
            .fetch_all(&pool)
            .await
            .expect("Failed to get accounts columns");
    let acc_col_names: Vec<String> = acc_columns.into_iter().map(|c| c.1).collect();
    assert!(
        acc_col_names.contains(&"credentials".to_string()),
        "accounts must contain credentials column"
    );
    assert!(
        acc_col_names.contains(&"service_name".to_string()),
        "accounts must contain service_name column"
    );
}

#[tokio::test]
async fn test_library_items_exact_migration_queries() {
    let pool = setup_migrated_db().await;

    // Seed services and accounts
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO accounts (id, service_id, email, is_active, credentials_json) VALUES (1, 1, 'test@example.com', 1, '{\"token\":\"valid\"}')")
        .execute(&pool)
        .await
        .unwrap();

    // 1. Insert into library_items
    sqlx::query(
        r#"INSERT INTO library_items (source_service, item_type, external_id, title, artist, album, duration_ms, quality)
           VALUES ('spotify', 'track', 'sp-101', 'Test Song 1', 'Test Artist 1', 'Test Album 1', 180000, 'flac')"#,
    )
    .execute(&pool)
    .await
    .expect("Must insert into library_items with source_service");

    sqlx::query(
        r#"INSERT INTO library_items (service, item_type, external_id, title, artist, album, duration_ms, quality)
           VALUES ('qobuz', 'track', 'qb-202', 'Test Song 2', 'Test Artist 2', 'Test Album 2', 240000, 'hires')"#,
    )
    .execute(&pool)
    .await
    .expect("Must insert into library_items with service column");

    // Verify trigger synced service <-> source_service
    let (src_svc, svc): (String, String) =
        sqlx::query_as("SELECT source_service, service FROM library_items WHERE external_id = 'qb-202'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(src_svc, "qobuz");
    assert_eq!(svc, "qobuz");

    // 2. Exact query from migration.rs lines 201-204
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM library_items WHERE source_service = ?")
        .bind("spotify")
        .fetch_one(&pool)
        .await
        .expect("Exact query lines 201-204 must succeed");
    assert_eq!(count.0, 1);

    // 3. Exact query from migration.rs lines 263-267
    let tracks: Vec<(i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, external_id, title, artist, album FROM library_items WHERE source_service = ? LIMIT 1000",
    )
    .bind("spotify")
    .fetch_all(&pool)
    .await
    .expect("Exact query lines 263-267 must succeed");
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].1, "sp-101");
    assert_eq!(tracks[0].2, "Test Song 1");
    assert_eq!(tracks[0].3, "Test Artist 1");
    assert_eq!(tracks[0].4.as_deref(), Some("Test Album 1"));

    // 4. Exact query from migration.rs lines 948-960
    let search_results: Vec<(String, String, String, Option<String>, i64, Option<String>)> =
        sqlx::query_as(
            r#"SELECT external_id, title, artist, album, duration_ms, quality 
               FROM library_items 
               WHERE source_service = ? AND (title LIKE ? OR artist LIKE ?)
               ORDER BY title LIMIT 20"#,
        )
        .bind("qobuz")
        .bind("%Song 2%")
        .bind("%Song 2%")
        .fetch_all(&pool)
        .await
        .expect("Exact query lines 948-960 must succeed");
    assert_eq!(search_results.len(), 1);
    assert_eq!(search_results[0].0, "qb-202");
    assert_eq!(search_results[0].4, 240000);
    assert_eq!(search_results[0].5.as_deref(), Some("hires"));

    // 5. Exact query for playlist joins from migration.rs lines 182-185 & 252-257
    sqlx::query(
        "INSERT INTO playlists (id, account_id, external_id, name, track_count) VALUES (1, 1, 'pl-ext-999', 'Rock Classics', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let lib_item_id = tracks[0].0;
    // We also need a canonical track for foreign key reference in playlist_tracks
    sqlx::query(
        "INSERT INTO tracks (id, title, duration_ms) VALUES (?, 'Test Song 1', 180000)",
    )
    .bind(lib_item_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, ?, 1)",
    )
    .bind(lib_item_id)
    .execute(&pool)
    .await
    .unwrap();

    let pl_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM library_items li 
         JOIN playlist_tracks pt ON pt.track_id = li.id 
         WHERE pt.playlist_id = (SELECT id FROM playlists WHERE external_id = ?)",
    )
    .bind("pl-ext-999")
    .fetch_one(&pool)
    .await
    .expect("Exact query lines 182-185 must succeed");
    assert_eq!(pl_count.0, 1);

    let pl_tracks: Vec<(i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT li.id, li.external_id, li.title, li.artist, li.album FROM library_items li
         JOIN playlist_tracks pt ON pt.track_id = li.id
         JOIN playlists p ON p.id = pt.playlist_id
         WHERE p.source_service = ? LIMIT 1000",
    )
    .bind("spotify")
    .fetch_all(&pool)
    .await
    .expect("Exact query lines 252-257 must succeed");
    assert_eq!(pl_tracks.len(), 1);
    assert_eq!(pl_tracks[0].1, "sp-101");
}

#[tokio::test]
async fn test_migration_items_dest_track_id_queries() {
    let pool = setup_migrated_db().await;

    // Create migration job
    sqlx::query(
        r#"INSERT INTO migration_jobs (id, source_service, destination_service, options, status)
           VALUES ('job-task-123', 'spotify', 'qobuz', '{}', 'running')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert migration items
    sqlx::query(
        r#"INSERT INTO migration_items (job_id, source_track_id, source_track_title, source_track_artist, source_track_album, status)
           VALUES ('job-task-123', 'src-tr-001', 'Title 1', 'Artist 1', 'Album 1', 'pending')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // 1. Exact update query with dest_track_id from migration.rs line 695
    sqlx::query(
        "UPDATE migration_items SET status = ?, match_confidence = ?, match_method = ?, dest_track_id = ?, processed_at = CURRENT_TIMESTAMP WHERE job_id = ? AND source_track_id = ?"
    )
    .bind("transferred")
    .bind(0.95f64)
    .bind("isrc")
    .bind("dest-tr-001")
    .bind("job-task-123")
    .bind("src-tr-001")
    .execute(&pool)
    .await
    .expect("Update migration_items with dest_track_id must succeed");

    // Verify values in database
    let (dest_id, status, confidence): (Option<String>, String, Option<f64>) = sqlx::query_as(
        "SELECT dest_track_id, status, match_confidence FROM migration_items WHERE job_id = ? AND source_track_id = ?",
    )
    .bind("job-task-123")
    .bind("src-tr-001")
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(dest_id.as_deref(), Some("dest-tr-001"));
    assert_eq!(status, "transferred");
    assert_eq!(confidence, Some(0.95));

    // 2. Manual match query from migration.rs line 986
    sqlx::query(
        "UPDATE migration_items SET destination_track_id = ?, dest_track_id = ?, match_method = 'manual', match_confidence = 1.0, status = 'matched' WHERE id = 1"
    )
    .bind("manual-dest-999")
    .bind("manual-dest-999")
    .execute(&pool)
    .await
    .expect("Manual match update must succeed");

    let (dest_id2, destination_id2): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT dest_track_id, destination_track_id FROM migration_items WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(dest_id2.as_deref(), Some("manual-dest-999"));
    assert_eq!(destination_id2.as_deref(), Some("manual-dest-999"));
}

#[tokio::test]
async fn test_accounts_credentials_and_service_name_queries() {
    let pool = setup_migrated_db().await;

    // Insert account using credentials_json for qobuz service
    sqlx::query(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES ((SELECT id FROM services WHERE name = 'qobuz'), 'user@qobuz.com', '{\"user_auth_token\":\"token_abc_123\"}', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Exact query from migration.rs line 318, 354, etc.
    let creds: Option<(String,)> = sqlx::query_as(
        "SELECT credentials FROM accounts WHERE service_name = 'qobuz' AND is_active = 1",
    )
    .fetch_optional(&pool)
    .await
    .expect("Querying credentials by service_name must succeed");

    assert!(creds.is_some(), "Account credentials should be found");
    assert!(creds.unwrap().0.contains("token_abc_123"));
}
