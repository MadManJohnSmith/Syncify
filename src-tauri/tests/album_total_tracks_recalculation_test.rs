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

/// Reads the counts migration 0088 put in play for one album: what
/// `total_tracks` advertises, what the release declares, and how many of those
/// tracks are imported here.
async fn album_counts(pool: &sqlx::SqlitePool, album_id: i64) -> (Option<i32>, i32, i32) {
    sqlx::query_as(
        "SELECT total_tracks, declared_total_tracks, local_track_count FROM albums WHERE id = ?",
    )
    .bind(album_id)
    .fetch_one(pool)
    .await
    .unwrap()
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

    sqlx::query(
        "INSERT INTO tracks (title, album_id) VALUES ('T1', ?), ('T2', ?), ('T3', ?), ('T4', ?)",
    )
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

    // The 0085 triggers keep total_tracks = COUNT(*) on every insert, so the
    // legacy divergences this test exercises are seeded via UPDATE: this is
    // exactly the state of rows written before 0085 or edited by hand.
    sqlx::query("UPDATE albums SET total_tracks = 23 WHERE id = ?")
        .bind(alb1_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE albums SET total_tracks = 1 WHERE id = ?")
        .bind(alb2_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE albums SET total_tracks = NULL WHERE id = ?")
        .bind(alb3_id)
        .execute(&pool)
        .await
        .unwrap();

    // Perform recalculation
    let report = perform_recalculate_album_total_tracks(&pool, None)
        .await
        .expect("perform_recalculate_album_total_tracks failed");

    // Migration 0088 (BD-5) split the album total, so the reconciliation works
    // on the EFFECTIVE total: an album that DECLARES a release total is not out
    // of sync just because it is partially imported, and repairing it must not
    // overwrite that declaration with the local count. The seed above captured
    // those declarations through the 0088 triggers — album 1 declares 23 and
    // album 2 declares 1 — so only album 3, whose total_tracks is NULL and which
    // declares nothing, is actually divergent here.
    assert_eq!(
        report.divergent_before, 1,
        "A declared release total that exceeds the local count is not a divergence"
    );
    assert_eq!(
        report.divergent_after, 0,
        "Expected 0 divergent albums after repair"
    );
    assert_eq!(
        report.updated_albums, 1,
        "Only the genuinely divergent album should be rewritten"
    );

    // Verify BOTH counts on every album: the declared total and the local one.
    assert_eq!(
        album_counts(&pool, alb1_id).await,
        (Some(23), 23, 2),
        "Album 1 must keep advertising its declared 23 and its local 2"
    );

    assert_eq!(
        album_counts(&pool, alb2_id).await,
        (Some(1), 1, 4),
        "Album 2 must keep advertising its declared 1 and its local 4"
    );

    // Album 3 declares nothing, so its total is derived from the library: this
    // is the legacy NULL divergence this reconciliation exists to repair.
    assert_eq!(
        album_counts(&pool, alb3_id).await,
        (Some(3), 0, 3),
        "Album 3 must derive both counts from its 3 imported tracks"
    );

    // Album 4 declared 2 and had all 2 imported: consistent before and after.
    assert_eq!(
        album_counts(&pool, alb4_id).await,
        (Some(2), 2, 2),
        "Album 4 must remain consistent"
    );

    // Scoped recalculation of a single album. The drift seeded here is on the LOCAL
    // column on purpose: `trg_albums_capture_declared_total_tracks_upd` (0088)
    // treats a write to `total_tracks` as the service publishing a release total,
    // so corrupting `total_tracks` by hand would simply re-declare the album and
    // leave nothing to repair. Album 1 DECLARES 23 with 2 of them imported, so
    // the scoped repair must fix `local_track_count` and leave the declaration in
    // `total_tracks` alone — writing the local count into `total_tracks` is
    // exactly what 0088 replaced.
    sqlx::query("UPDATE albums SET local_track_count = 7 WHERE id = ?")
        .bind(alb1_id)
        .execute(&pool)
        .await
        .unwrap();
    let scoped = perform_recalculate_album_total_tracks(&pool, Some(vec![alb1_id]))
        .await
        .expect("Single album recalculation failed");
    assert_eq!(scoped.updated_albums, 1);
    assert_eq!(
        album_counts(&pool, alb1_id).await,
        (Some(23), 23, 2),
        "Scoped recalculation must repair the local count and keep the declared 23"
    );
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

    assert_eq!(
        report.divergent_before, 0,
        "Stub should not be counted as divergent"
    );
    assert_eq!(report.divergent_after, 0);

    // Verify stub's total_tracks is completely untouched
    let (stub_tt, is_stub): (Option<i32>, i64) =
        sqlx::query_as("SELECT total_tracks, is_stub FROM albums WHERE id = ?")
            .bind(stub_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        stub_tt,
        Some(12),
        "Stub album must preserve declared total_tracks"
    );
    assert_eq!(is_stub, 1, "Album must remain marked as stub");
}

#[tokio::test]
async fn test_recurrence_triggers_maintain_total_tracks() {
    let pool = setup_test_db().await;

    // The recurrence triggers are installed by migration 0085 (BD-12); they no
    // longer need a runtime installer.

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
    assert_eq!(
        tt1, 1,
        "Trigger must update total_tracks to 1 on first track insert"
    );

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
    assert_eq!(
        tt2, 2,
        "Trigger must update total_tracks to 2 on second track insert"
    );

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
    assert_eq!(
        tt_final, 0,
        "Original album must decrement to 0 tracks upon deletion"
    );
}

#[tokio::test]
async fn test_merge_duplicates_synchronizes_total_tracks() {
    let pool = setup_test_db().await;

    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
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

    let s1: (i64,) = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, 'src1'), (?, ?, 'src2')")
        .bind(t1)
        .bind(s1.0)
        .bind(t2)
        .bind(s1.0)
        .execute(&pool)
        .await
        .unwrap();

    // Second album, this one with NO declared total, and its own duplicate pair with a distinct
    // title so it forms a separate merge component. It keeps the "merge derives a
    // missing total" behaviour covered now that a declared total is preserved instead.
    let album2_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Undeclared Album', NULL, 0) RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let t3: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, audio_quality, track_number, disc_number) VALUES ('Other Song', ?, 200000, 'lossless', 1, 1) RETURNING id",
    )
    .bind(album2_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let t4: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, audio_quality, track_number, disc_number) VALUES ('Other Song', ?, 200500, 'lossy', 1, 1) RETURNING id",
    )
    .bind(album2_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary'), (?, ?, 'primary')")
        .bind(t3).bind(artist_id)
        .bind(t4).bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, 'src3'), (?, ?, 'src4')")
        .bind(t3)
        .bind(s1.0)
        .bind(t4)
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

    assert_eq!(
        merge_res.tracks_removed, 2,
        "Expected 1 duplicate track removed per album (2 albums)"
    );

    // BD-5 (audit item 21): a declared total survives the merge. 0088 splits
    // the release total (albums.declared_total_tracks, here 2 — the album was
    // created with it) from the imported count (albums.local_track_count), so
    // removing the duplicate row no longer rewrites what the release declares.
    // The honest count is still there, in local_track_count.
    let tt_post: Option<i32> = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        tt_post,
        Some(2),
        "Post-merge total_tracks must keep the declared release total"
    );

    let local_post: Option<i32> =
        sqlx::query_scalar("SELECT local_track_count FROM albums WHERE id = ?")
            .bind(album_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        local_post,
        Some(1),
        "BD-5: the imported count must reflect the single surviving track row"
    );

    let declared_post: Option<i32> =
        sqlx::query_scalar("SELECT declared_total_tracks FROM albums WHERE id = ?")
            .bind(album_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        declared_post,
        Some(2),
        "BD-5: the declaration is recorded apart from the derived count"
    );

    // The duplicate really was removed: the album now holds a single row for that track.
    let survivors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(survivors, 1, "Merge must leave one surviving track row");

    // An album with no declared count still gets it derived from the library.
    let tt2_post: Option<i32> = sqlx::query_scalar("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(album2_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        tt2_post,
        Some(1),
        "Album without a declared total must be synchronized to its surviving track count"
    );
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

    // The 0085 triggers already recounted to 2 on insert; re-create the legacy
    // divergence the maintenance script exists to repair.
    sqlx::query("UPDATE albums SET total_tracks = 10 WHERE id = ?")
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

    let script_path = if std::path::Path::new("scripts/recalculate_album_total_tracks.py").exists()
    {
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
        eprintln!(
            "dry_run stderr: {}",
            String::from_utf8_lossy(&dry_output.stderr)
        );
    }
    assert!(dry_output.status.success(), "Dry run must succeed");
    let dry_stdout = String::from_utf8_lossy(&dry_output.stdout);
    assert!(
        dry_stdout.contains("[DRY RUN]"),
        "Stdout must indicate dry run"
    );

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
        eprintln!(
            "repair stderr: {}",
            String::from_utf8_lossy(&repair_output.stderr)
        );
    }
    assert!(repair_output.status.success(), "Repair run must succeed");
    let repair_stdout = String::from_utf8_lossy(&repair_output.stdout);
    assert!(
        repair_stdout.contains("POST-REPAIR VERIFICATION:"),
        "Stdout must contain verification section"
    );
    assert!(
        repair_stdout.contains("Divergent albums remaining:       0"),
        "Remaining divergence must be 0"
    );
    assert!(
        repair_stdout.contains("PRAGMA integrity_check: OK"),
        "Integrity check must pass"
    );
    assert!(
        repair_stdout.contains("PRAGMA foreign_key_check: OK"),
        "Foreign key check must pass"
    );

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
    assert_eq!(
        stub_tt, 15,
        "Stub album must preserve declared total_tracks = 15"
    );

    check_pool.close().await;
}
