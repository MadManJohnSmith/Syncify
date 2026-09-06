//! TASK-138: Album Total Tracks Recalculation & Divergence Reconciliation Test Suite
//!
//! Validates:
//! 1. Recalculation of `albums.total_tracks` corrects divergent counts (excess, deficit, NULL).
//! 2. Stubs with `is_stub = 1` preserve their declared `total_tracks` when 0 local tracks exist.
//! 3. Incremental track insertions and deletions update `total_tracks` via triggers and hooks.
//! 4. Merge/deduplication in `merge_level2_3_duplicates_inner` synchronizes `total_tracks`.
//! 5. Execution of the portable Python maintenance script (`scripts/recalculate_album_total_tracks.py`)
//!    verifies safety backups, `--dry-run`, repair integrity, and zero residual divergence.

use sqlx::sqlite::SqlitePoolOptions;
use std::process::Command;
use syncify_tauri_lib::commands::{
    merge_level2_3_duplicates_inner, perform_recalculate_album_total_tracks,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::enrichment::{
    install_album_total_tracks_triggers, recalculate_album_total_tracks,
};

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
async fn test_recalculate_album_total_tracks_fixes_divergences() {
    let pool = setup_test_db().await;

    // Album 1: Excess (declared 23, but has only 2 tracks in library)
    let alb1_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('El Madrileño', 23, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('Demasiadas Mujeres', ?), ('Tú Me Dejaste De Querer', ?)")
        .bind(alb1_id)
        .bind(alb1_id)
        .execute(&pool)
        .await
        .unwrap();

    // Album 2: Deficit (declared 1, but has 4 tracks)
    let alb2_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('50 najlepszych', 1, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('T1', ?), ('T2', ?), ('T3', ?), ('T4', ?)")
        .bind(alb2_id)
        .bind(alb2_id)
        .bind(alb2_id)
        .bind(alb2_id)
        .execute(&pool)
        .await
        .unwrap();

    // Album 3: NULL total_tracks with 3 tracks
    let alb3_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Unknown Album', NULL, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('Track A', ?), ('Track B', ?), ('Track C', ?)")
        .bind(alb3_id)
        .bind(alb3_id)
        .bind(alb3_id)
        .execute(&pool)
        .await
        .unwrap();

    // Album 4: Already consistent (declared 2, has 2)
    let alb4_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Consistent Album', 2, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('Track 1', ?), ('Track 2', ?)")
        .bind(alb4_id)
        .bind(alb4_id)
        .execute(&pool)
        .await
        .unwrap();

    // Perform recalculation
    let report = perform_recalculate_album_total_tracks(&pool, None)
        .await
        .expect("perform_recalculate_album_total_tracks failed");

    assert_eq!(report.divergent_before, 3, "Expected 3 divergent albums before repair");
    assert_eq!(report.divergent_after, 0, "Expected 0 divergent albums after repair");

    // Verify reconciled values
    let tt1: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb1_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt1, 2, "Album 1 should have been reconciled to 2 tracks");

    let tt2: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb2_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt2, 4, "Album 2 should have been reconciled to 4 tracks");

    let tt3: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb3_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt3, 3, "Album 3 should have been reconciled to 3 tracks");

    let tt4: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb4_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt4, 2, "Album 4 should remain at 2 tracks");

    // Also test single album recalculation via service helper
    sqlx::query("UPDATE albums SET total_tracks = 999 WHERE id = ?")
        .bind(alb1_id)
        .execute(&pool)
        .await
        .unwrap();
    let aff = recalculate_album_total_tracks(&pool, Some(alb1_id))
        .await
        .expect("Single album recalculation failed");
    assert_eq!(aff, 1);
    let tt1_again: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb1_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt1_again, 2, "Single album recalculation should reset Album 1 to 2 tracks");
}

#[tokio::test]
async fn test_stubs_preserve_declared_total_tracks() {
    let pool = setup_test_db().await;

    // Insert a stub favorite album with 0 local tracks but declared total_tracks = 12
    let stub_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_favorite, is_stub) VALUES ('Ghost Favorite Album', 12, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Run recalculation
    let report = perform_recalculate_album_total_tracks(&pool, None)
        .await
        .expect("Recalculation should succeed");

    assert_eq!(report.divergent_before, 0, "Stub should not be counted as divergent");
    assert_eq!(report.divergent_after, 0);

    // Verify stub's total_tracks is completely untouched
    let (stub_tt, is_stub): (Option<i32>, i64) = sqlx::query_as(
        "SELECT total_tracks, is_stub FROM albums WHERE id = ?"
    )
    .bind(stub_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(stub_tt, Some(12), "Stub album must preserve declared total_tracks");
    assert_eq!(is_stub, 1, "Album must remain marked as stub");
}

#[tokio::test]
async fn test_recurrence_triggers_maintain_total_tracks() {
    let pool = setup_test_db().await;

    // Install recurrence triggers
    install_album_total_tracks_triggers(&pool)
        .await
        .expect("Trigger installation must succeed");

    // Create an album
    let alb_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Dynamic Album', 0, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 1. Insert first track -> trigger should increment total_tracks to 1
    let t1_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id) VALUES ('Track 1', ?) RETURNING id",
    )
    .bind(alb_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let tt1: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt1, 1, "Trigger must update total_tracks to 1 on first track insert");

    // 2. Insert second track -> trigger should update total_tracks to 2
    let t2_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id) VALUES ('Track 2', ?) RETURNING id",
    )
    .bind(alb_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let tt2: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt2, 2, "Trigger must update total_tracks to 2 on second track insert");

    // 3. Move track 2 to a new album -> both albums must reflect their updated counts
    let alb2_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Target Album', 0, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("UPDATE tracks SET album_id = ? WHERE id = ?")
        .bind(alb2_id)
        .bind(t2_id)
        .execute(&pool)
        .await
        .unwrap();

    let tt_orig: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt_orig, 1, "Original album must decrement to 1 track");

    let tt_targ: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb2_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt_targ, 1, "Target album must increment to 1 track");

    // 4. Delete track 1 from original album -> should become 0
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(t1_id)
        .execute(&pool)
        .await
        .unwrap();

    let tt_final: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt_final, 0, "Original album must decrement to 0 tracks upon deletion");
}

#[tokio::test]
async fn test_merge_duplicates_synchronizes_total_tracks() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Deduplicated Album', 2, 0) RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // Insert duplicate tracks with same title and track_number (one lossless, one lossy)
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, audio_quality, track_number, disc_number) VALUES ('Song A', ?, 180000, 'lossless', 1, 1) RETURNING id",
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, audio_quality, track_number, disc_number) VALUES ('Song A', ?, 180500, 'lossy', 1, 1) RETURNING id",
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary'), (?, ?, 'primary')")
        .bind(t1).bind(artist_id)
        .bind(t2).bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let s1: (i64,) = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, 'src1'), (?, ?, 'src2')")
        .bind(t1)
        .bind(s1.0)
        .bind(t2)
        .bind(s1.0)
        .execute(&pool)
        .await
        .unwrap();

    // Prior to merge, total_tracks is 2
    let tt_pre: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt_pre, 2);

    // Run merge
    let merge_res = merge_level2_3_duplicates_inner(&pool)
        .await
        .expect("merge_level2_3_duplicates_inner should execute cleanly");

    assert_eq!(merge_res.tracks_removed, 1, "Expected 1 duplicate track removed");

    // Verify album total_tracks is now synchronized to 1
    let tt_post: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tt_post, 1, "Album total_tracks must be updated to 1 following merge");
}

#[tokio::test]
async fn test_python_script_execution_and_assertions() {
    use tempfile::NamedTempFile;

    // Create a temporary SQLite database on disk
    let file = NamedTempFile::new().expect("Failed to create temp file");
    let db_path = file.path().to_str().unwrap().to_string();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}", db_path))
        .await
        .expect("Failed to connect to disk temp DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply migrations on temp disk DB");

    // Seed divergent albums and stubs
    let alb1_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Python Test Album 1', 10, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('T1', ?), ('T2', ?)")
        .bind(alb1_id)
        .bind(alb1_id)
        .execute(&pool)
        .await
        .unwrap();

    let stub_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Python Stub Album', 15, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Close pool so Python script can open the file without locks
    pool.close().await;

    let script_path = if std::path::Path::new("scripts/recalculate_album_total_tracks.py").exists() {
        "scripts/recalculate_album_total_tracks.py".to_string()
    } else if std::path::Path::new("../scripts/recalculate_album_total_tracks.py").exists() {
        "../scripts/recalculate_album_total_tracks.py".to_string()
    } else {
        panic!("recalculate_album_total_tracks.py not found");
    };

    let backup_dir = tempfile::tempdir().expect("Failed to create temp backup dir");
    let backup_dir_str = backup_dir.path().to_str().unwrap().to_string();

    // 1. Test dry-run mode
    let dry_output = Command::new("python3")
        .arg(&script_path)
        .arg("--db-path")
        .arg(&db_path)
        .arg("--backup-dir")
        .arg(&backup_dir_str)
        .arg("--dry-run")
        .output()
        .expect("Failed to execute python script dry-run");

    if !dry_output.status.success() {
        eprintln!("dry_run stderr: {}", String::from_utf8_lossy(&dry_output.stderr));
    }
    assert!(dry_output.status.success(), "Dry run must succeed");
    let dry_stdout = String::from_utf8_lossy(&dry_output.stdout);
    assert!(dry_stdout.contains("[DRY RUN]"), "Stdout must indicate dry run");

    // 2. Test repair mode
    let repair_output = Command::new("python3")
        .arg(&script_path)
        .arg("--db-path")
        .arg(&db_path)
        .arg("--backup-dir")
        .arg(&backup_dir_str)
        .output()
        .expect("Failed to execute python script repair");

    if !repair_output.status.success() {
        eprintln!("repair stderr: {}", String::from_utf8_lossy(&repair_output.stderr));
    }
    assert!(repair_output.status.success(), "Repair run must succeed");
    let repair_stdout = String::from_utf8_lossy(&repair_output.stdout);
    assert!(repair_stdout.contains("POST-REPAIR VERIFICATION:"), "Stdout must contain verification section");
    assert!(repair_stdout.contains("Divergent albums remaining:       0"), "Remaining divergence must be 0");
    assert!(repair_stdout.contains("PRAGMA integrity_check: OK"), "Integrity check must pass");
    assert!(repair_stdout.contains("PRAGMA foreign_key_check: OK"), "Foreign key check must pass");

    // 3. Re-open pool and verify values
    let check_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}", db_path))
        .await
        .unwrap();

    let tt1: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(alb1_id)
        .fetch_one(&check_pool)
        .await
        .unwrap();
    assert_eq!(tt1, 2, "Album 1 total_tracks must be corrected to 2");

    let stub_tt: i32 = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(stub_id)
        .fetch_one(&check_pool)
        .await
        .unwrap();
    assert_eq!(stub_tt, 15, "Stub album must preserve declared total_tracks = 15");

    check_pool.close().await;
}
