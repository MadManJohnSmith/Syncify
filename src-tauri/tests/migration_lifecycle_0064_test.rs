//! Migration 0064 Lifecycle, Schema, and Hardening Integrity Test

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[tokio::test]
async fn test_migration_0064_clean_run_and_integrity_constraints() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0064_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // 1. Run canonical SQLx migrations (0001 -> 0064)
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("Canonical migrator must upgrade cleanly from 0001 to 0064");

    // 2. Verify migration version in _sqlx_migrations
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("Must fetch max migration version");
    assert!(max_v.0 >= 64, "Database must be at least migration version 64");

    // 3. Test playlist_tracks: allow duplicate track_id with different positions
    sqlx::query("INSERT INTO services (id, name) VALUES (999, 'test_svc') ON CONFLICT DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name) VALUES (999, 999, 'test_acc') ON CONFLICT DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES (100, 999, 'svc_pl_100', 'Test Playlist')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (501, 'Song A', 180000, 'USAAA2000001')")
        .execute(&pool)
        .await
        .unwrap();

    // Position 0 and Position 1 for the SAME track_id (previously forbidden by UNIQUE(playlist_id, track_id))
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (100, 501, 0)")
        .execute(&pool)
        .await
        .expect("Inserting track at position 0 must succeed");
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (100, 501, 1)")
        .execute(&pool)
        .await
        .expect("Inserting same track at position 1 must succeed with UNIQUE(playlist_id, position)");

    // Duplicate position for same playlist must fail
    let dup_pos_res = sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (100, 501, 0)")
        .execute(&pool)
        .await;
    assert!(dup_pos_res.is_err(), "Duplicate (playlist_id, position) must violate UNIQUE constraint");

    // 4. Test tracks case-insensitive ISRC uniqueness
    let dup_isrc_res = sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (502, 'Song A Lower', 180000, 'usaaa2000001')")
        .execute(&pool)
        .await;
    assert!(dup_isrc_res.is_err(), "Duplicate case-insensitive ISRC must violate idx_tracks_isrc_unique");

    // 5. Test track_sources unique index (service_id, service_track_id)
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (501, 999, 'ext_track_1')")
        .execute(&pool)
        .await
        .expect("First track_source must succeed");
    let dup_ts_res = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (501, 999, 'ext_track_1')")
        .execute(&pool)
        .await;
    assert!(dup_ts_res.is_err(), "Duplicate (service_id, service_track_id) must violate idx_track_sources_service_track_unique");

    // 6. Test SoundCloud defaults in services and quality_preferences
    let sc_service: (String,) = sqlx::query_as("SELECT max_quality FROM services WHERE name = 'soundcloud'")
        .fetch_one(&pool)
        .await
        .expect("SoundCloud must exist in services");
    assert_eq!(sc_service.0, "lossy");

    let sc_pref: (String, String) = sqlx::query_as("SELECT max_quality, preferred_format FROM quality_preferences WHERE service_name = 'soundcloud'")
        .fetch_one(&pool)
        .await
        .expect("SoundCloud must exist in quality_preferences");
    assert_eq!(sc_pref.0, "lossy");
    assert_eq!(sc_pref.1, "mp3");
}

#[tokio::test]
async fn test_migration_0064_on_real_user_db_copy() {
    let user_db_path = std::path::Path::new("/home/alan/.local/share/com.syncify.app/syncify.db");
    if !user_db_path.exists() {
        return;
    }

    let temp_dir = TempDir::new().unwrap();
    let test_db_path = temp_dir.path().join("real_user_copy.db");
    std::fs::copy(user_db_path, &test_db_path).expect("Failed to copy user db");

    let db_url = format!("sqlite:{}?mode=rwc", test_db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to copied user DB");

    // Run canonical migrator (will apply 0064 on the existing 0063 db)
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("Migrator must successfully upgrade real user db from 0063 to 0064");

    // Verify foreign key integrity
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check failed");
    assert!(fk_violations.is_empty(), "Foreign key check must have zero violations on user db");

    // Verify zero duplicate ISRCs
    let dup_isrc_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT isrc COLLATE NOCASE, count(*) c FROM tracks WHERE isrc IS NOT NULL GROUP BY 1 HAVING c > 1)"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dup_isrc_count.0, 0, "All duplicate ISRCs must be eliminated");

    // Verify max migration version is 64
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(max_v.0 >= 64, "Database must be at least migration version 64");
}

#[tokio::test]
async fn test_migration_0064_upgrade_stepwise_with_existing_data() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0064_upgrade.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // 1. Run migrations up to 63
    let migrator = sqlx::migrate!("./migrations");
    let initial_migrations: Vec<_> = migrator
        .migrations
        .iter()
        .filter(|m| m.version <= 63)
        .cloned()
        .collect();

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(initial_migrations),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    partial_migrator
        .run(&pool)
        .await
        .expect("Migrations 1..=63 must succeed");

    // Populate existing data at schema v63
    sqlx::query("INSERT INTO services (id, name) VALUES (999, 'test_svc') ON CONFLICT DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name) VALUES (999, 999, 'test_acc') ON CONFLICT DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES (200, 999, 'svc_pl_200', 'Existing Playlist')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (601, 'Track 1', 120000, 'USAAA1000001')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (602, 'Track 2', 130000, 'USAAA1000002')")
        .execute(&pool)
        .await
        .unwrap();

    // Insert playlist_tracks with old schema
    sqlx::query("INSERT INTO playlist_tracks (id, playlist_id, track_id, position) VALUES (1, 200, 601, 0)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (id, playlist_id, track_id, position) VALUES (2, 200, 602, 1)")
        .execute(&pool)
        .await
        .unwrap();

    // Insert duplicate track_sources to test dedup in migration 0064
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (601, 999, 'dup_svc_trk')")
        .execute(&pool)
        .await
        .unwrap();
    // Another row with different id but same (service_id, service_track_id)
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (602, 999, 'dup_svc_trk')")
        .execute(&pool)
        .await
        .unwrap();

    // Insert tracks with casing and hyphen ISRC collisions to test migration 0064 dedup
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc, is_favorite) VALUES (603, 'Track 1 Lower', 120000, 'usaaa1000001', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (604, 'Track 2 Hyphen', 130000, 'US-AAA-10-00002')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (id, playlist_id, track_id, position) VALUES (3, 200, 603, 2)")
        .execute(&pool)
        .await
        .unwrap();

    // 2. Now run all migrations (including 0064)
    migrator
        .run(&pool)
        .await
        .expect("Canonical migrator must upgrade from 63 to 64 with existing data");

    // 3. Verify existing playlist tracks preserved and repointed
    let pt_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = 200")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pt_count.0, 3);

    // Verify track 603 was merged into 601 and repointed in playlist_tracks
    let pt_pos2: (i64,) = sqlx::query_as("SELECT track_id FROM playlist_tracks WHERE playlist_id = 200 AND position = 2")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(pt_pos2.0, 601, "Playlist track at pos 2 must be repointed from loser 603 to winner 601");

    // Verify winner 601 inherited is_favorite = 1 from loser 603
    let trk_601_fav: (i64,) = sqlx::query_as("SELECT is_favorite FROM tracks WHERE id = 601")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(trk_601_fav.0, 1, "Winner 601 must inherit is_favorite = 1 from loser 603");

    // Verify loser tracks 603 and 604 were removed
    let loser_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE id IN (603, 604)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(loser_count.0, 0, "Loser tracks must be removed after dedup");

    // Verify all ISRCs in tracks are normalized
    let unnormalized_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE isrc != UPPER(REPLACE(isrc, '-', ''))")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unnormalized_count.0, 0, "All ISRCs must be uppercase and without hyphens");

    // 4. Verify duplicate track_sources was deduplicated and unique index exists
    let ts_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM track_sources WHERE service_track_id = 'dup_svc_trk'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ts_count.0, 1, "Duplicate track_sources must be deduplicated to 1 row");

    // 5. Verify soundcloud values updated
    let sc_service: (String,) = sqlx::query_as("SELECT max_quality FROM services WHERE name = 'soundcloud'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sc_service.0, "lossy");
}

#[tokio::test]
async fn test_adversarial_qa_epic_1_constraints() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("migration_0064_adversarial_qa.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // Fresh DB migrated with sqlx::migrate!("./migrations")
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migration failed");

    // PRAGMA foreign_key_check must return 0 rows
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check failed");
    assert!(fk_violations.is_empty(), "Foreign key check must have zero violations");

    // 1. playlist_tracks verification:
    // Setup dummy playlist and tracks
    sqlx::query("INSERT INTO services (id, name) VALUES (10, 'qa_svc') ON CONFLICT DO NOTHING")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name) VALUES (10, 10, 'qa_acc') ON CONFLICT DO NOTHING")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES (1, 10, 'pl_qa_1', 'Playlist 1')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES (2, 10, 'pl_qa_2', 'Playlist 2')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (id, title, duration_ms) VALUES (1, 'Track 1', 1000)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (id, title, duration_ms) VALUES (2, 'Track 2', 2000)")
        .execute(&pool).await.unwrap();

    // 1.a Track 1 at pos 0 in playlist 1
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, 1, 0)")
        .execute(&pool).await.unwrap();

    // 1.b Repeated track: Track 1 at pos 5 in playlist 1 -> MUST SUCCEED
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, 1, 5)")
        .execute(&pool).await.expect("Repeated track at different position must succeed");

    // 1.c Same position collision: Track 2 at pos 0 in playlist 1 -> MUST FAIL (UNIQUE(playlist_id, position))
    let dup_pos_err = sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, 2, 0)")
        .execute(&pool).await;
    assert!(dup_pos_err.is_err(), "Same position must fail");

    // 1.d Same position in different playlist -> MUST SUCCEED
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (2, 2, 0)")
        .execute(&pool).await.expect("Same position in different playlist must succeed");

    // 2. tracks.isrc case-insensitive uniqueness:
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (3, 'Track 3', 3000, 'US-ABC-20-00001')")
        .execute(&pool).await.unwrap();
    let isrc_case_err = sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (4, 'Track 4', 4000, 'us-abc-20-00001')")
        .execute(&pool).await;
    assert!(isrc_case_err.is_err(), "Case-insensitive ISRC collision must fail");

    // Multiple NULL ISRCs must be allowed
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (5, 'Track 5', 5000, NULL)")
        .execute(&pool).await.expect("NULL ISRC 1 must succeed");
    sqlx::query("INSERT INTO tracks (id, title, duration_ms, isrc) VALUES (6, 'Track 6', 6000, NULL)")
        .execute(&pool).await.expect("NULL ISRC 2 must succeed");

    // 3. track_sources unique on (service_id, service_track_id):
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (1, 10, 'qa_ext_id_1')")
        .execute(&pool).await.expect("First track_source must succeed");

    // Collide on (service_id, service_track_id) with different track_id
    let ts_err = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (2, 10, 'qa_ext_id_1')")
        .execute(&pool).await;
    assert!(ts_err.is_err(), "Duplicate (service_id, service_track_id) must fail");

    // Different service_id with same service_track_id -> MUST SUCCEED
    sqlx::query("INSERT INTO services (id, name) VALUES (11, 'qa_svc_2') ON CONFLICT DO NOTHING")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (2, 11, 'qa_ext_id_1')")
        .execute(&pool).await.expect("Same service_track_id on different service_id must succeed");

    // 4. services and quality_preferences for soundcloud:
    let sc_srv: (String,) = sqlx::query_as("SELECT max_quality FROM services WHERE name = 'soundcloud'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(sc_srv.0, "lossy", "SoundCloud service max_quality must be 'lossy'");

    let sc_pref: (String, String) = sqlx::query_as("SELECT max_quality, preferred_format FROM quality_preferences WHERE service_name = 'soundcloud'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(sc_pref.0, "lossy", "SoundCloud preference max_quality must be 'lossy'");
    assert_eq!(sc_pref.1, "mp3", "SoundCloud preference preferred_format must be 'mp3'");
}
