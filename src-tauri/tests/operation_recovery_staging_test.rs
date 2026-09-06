//! Integration Test Suite for Staging Purge & Stuck Queue Recovery (TASK-148)
//!
//! Verifies:
//! 1. Abandoned staging files (15623.part, 15623.cover.jpg, 15623.lrc) are cleanly purged.
//! 2. Hidden files such as .nomedia are preserved.
//! 3. Stuck 'downloading' queue rows (e.g. item 15623) are transitioned to 'failed'.
//! 4. Explanatory error message is recorded in both error_message and last_error.
//! 5. Unrelated queue rows in 'queued' and 'complete' states are strictly unaltered.
//! 6. The cleanup routine is fully idempotent.
//! 7. Path traversal attempts and outside files are protected.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;
use syncify_tauri_lib::services::operation_recovery::{
    cleanup_staging_and_recover_stuck_queue,
    cleanup_staging_and_recover_stuck_queue_with_message,
};

fn create_dummy_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = File::create(path).expect("Failed to create dummy file");
    file.write_all(content).expect("Failed to write content");
    file.flush().expect("Failed to flush file");
}

#[tokio::test]
async fn test_cleanup_staging_and_recover_stuck_queue_item_15623() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("test_recovery_15623.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Seed tracks to satisfy FK constraints
    let _tid_stuck: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, duration_ms) VALUES (15623, 'Stuck In-Flight Track', 214000) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let _tid_queued: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, duration_ms) VALUES (20001, 'Queued Track', 180000) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let _tid_complete: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, duration_ms) VALUES (30001, 'Complete Track', 240000) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 2. Setup .staging directory and populate with trash files
    let staging_dir = temp.path().join(".staging");
    std::fs::create_dir_all(&staging_dir).unwrap();

    let part_file = staging_dir.join("15623.part");
    let cover_file = staging_dir.join("15623.cover.jpg");
    let lrc_file = staging_dir.join("15623.lrc");
    let nomedia_file = staging_dir.join(".nomedia");

    create_dummy_file(&part_file, b"INCOMPLETE_BINARY_AUDIO_STREAM_15623");
    create_dummy_file(&cover_file, b"\xFF\xD8\xFF\xE0\x00\x10JFIF_SAMPLE_COVER");
    create_dummy_file(&lrc_file, b"[00:01.00]Syncify Staging Orphan Lyric Line\n");
    create_dummy_file(&nomedia_file, b"");

    assert!(part_file.exists());
    assert!(cover_file.exists());
    assert!(lrc_file.exists());
    assert!(nomedia_file.exists());

    // 3. Seed download_queue rows
    // Item 15623: Stuck in 'downloading'
    sqlx::query(
        r#"
        INSERT INTO download_queue (
            id, track_id, status, priority, position, staging_path, started_at
        ) VALUES (15623, 15623, 'downloading', 50, 1, ?, CURRENT_TIMESTAMP)
        "#
    )
    .bind(part_file.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // Item 20001: Normal 'queued' item
    sqlx::query(
        r#"
        INSERT INTO download_queue (
            id, track_id, status, priority, position
        ) VALUES (20001, 20001, 'queued', 50, 2)
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    // Item 30001: Completed item
    sqlx::query(
        r#"
        INSERT INTO download_queue (
            id, track_id, status, priority, position, progress_percent, completed_at
        ) VALUES (30001, 30001, 'complete', 50, 3, 100.0, CURRENT_TIMESTAMP)
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    // 4. Execute recovery
    let summary = cleanup_staging_and_recover_stuck_queue(&pool, Some(&staging_dir))
        .await
        .expect("Recovery routine should execute cleanly");

    // 5. Verify staging file purge assertions
    assert_eq!(summary.purged_staging_files, 3, "Exactly 3 orphan staging files should be purged");
    assert_eq!(summary.recovered_stuck_items, 1, "Exactly 1 stuck queue item should be recovered");
    assert_eq!(summary.recovered_queue_ids, vec![15623], "Queue id 15623 must be the recovered item");

    assert!(!part_file.exists(), "15623.part must be deleted from staging");
    assert!(!cover_file.exists(), "15623.cover.jpg must be deleted from staging");
    assert!(!lrc_file.exists(), "15623.lrc must be deleted from staging");
    assert!(nomedia_file.exists(), ".nomedia must be preserved in .staging");
    assert!(staging_dir.exists(), ".staging root directory itself must NOT be deleted");

    // 6. Verify download_queue state mutations
    // Stuck item 15623 -> must be failed with explanatory error
    let (status_15623, err_msg_15623, last_err_15623): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, error_message, last_error FROM download_queue WHERE id = 15623"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(status_15623, "failed", "Stuck downloading item must transition to failed");
    assert_eq!(
        err_msg_15623.as_deref(),
        Some("Download interrupted by system restart"),
        "Error message must indicate restart recovery"
    );
    assert_eq!(
        last_err_15623.as_deref(),
        Some("Download interrupted by system restart"),
        "Last error must mirror error message"
    );

    // Queued item 20001 -> must NOT be altered
    let (status_20001, err_msg_20001): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_message FROM download_queue WHERE id = 20001"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(status_20001, "queued", "Queued item must remain queued");
    assert!(err_msg_20001.is_none(), "Queued item must not have error message set");

    // Complete item 30001 -> must NOT be altered
    let (status_30001, err_msg_30001): (String, Option<String>) = sqlx::query_as(
        "SELECT status, error_message FROM download_queue WHERE id = 30001"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(status_30001, "complete", "Completed item must remain complete");
    assert!(err_msg_30001.is_none(), "Completed item must not have error message set");

    // 7. Test Idempotency: Running recovery again must perform 0 mutations
    let idempotent_summary = cleanup_staging_and_recover_stuck_queue(&pool, Some(&staging_dir))
        .await
        .expect("Second run must succeed");

    assert_eq!(idempotent_summary.purged_staging_files, 0, "No new files should be purged");
    assert_eq!(idempotent_summary.recovered_stuck_items, 0, "No stuck items should remain");
}

#[tokio::test]
async fn test_cleanup_staging_custom_message_and_nested_directories() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("test_recovery_nested.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Seed track and stuck queue item
    sqlx::query("INSERT INTO tracks (id, title, duration_ms) VALUES (50001, 'Nested Test Track', 180000)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO download_queue (id, track_id, status) VALUES (50001, 50001, 'downloading')")
        .execute(&pool)
        .await
        .unwrap();

    // 2. Create nested subfolder in .staging
    let staging_dir = temp.path().join(".staging");
    let nested_session_dir = staging_dir.join("session_worker_alpha");
    std::fs::create_dir_all(&nested_session_dir).unwrap();

    let nested_part = nested_session_dir.join("chunk_01.part");
    let nested_tmp = nested_session_dir.join("chunk_01.tmp");
    let root_lrc = staging_dir.join("50001.lrc");

    create_dummy_file(&nested_part, b"PARTIAL_CHUNK_BYTES");
    create_dummy_file(&nested_tmp, b"TEMP_METADATA_BYTES");
    create_dummy_file(&root_lrc, b"[00:00.00]Nested lyrics");

    assert!(nested_part.exists());
    assert!(nested_tmp.exists());
    assert!(root_lrc.exists());

    // 3. Run recovery with custom reason
    let custom_msg = "Orphaned downloading item recovered on startup";
    let summary = cleanup_staging_and_recover_stuck_queue_with_message(
        &pool,
        Some(&staging_dir),
        custom_msg,
    )
    .await
    .unwrap();

    assert_eq!(summary.purged_staging_files, 3);
    assert_eq!(summary.recovered_stuck_items, 1);
    assert_eq!(summary.recovered_queue_ids, vec![50001]);

    assert!(!nested_part.exists());
    assert!(!nested_tmp.exists());
    assert!(!root_lrc.exists());
    // The empty subfolder should be pruned
    assert!(!nested_session_dir.exists(), "Empty subfolder inside staging must be pruned");
    // Root staging must remain
    assert!(staging_dir.exists(), "Staging root must remain");

    // 4. Verify custom error message in SQLite
    let (status, err_msg, last_err): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, error_message, last_error FROM download_queue WHERE id = 50001"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(status, "failed");
    assert_eq!(err_msg.as_deref(), Some(custom_msg));
    assert_eq!(last_err.as_deref(), Some(custom_msg));
}

#[tokio::test]
async fn test_cleanup_staging_does_not_purge_files_outside_staging() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("test_traversal.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let staging_dir = temp.path().join(".staging");
    std::fs::create_dir_all(&staging_dir).unwrap();

    // Legitimate staging file
    let staging_part = staging_dir.join("test_track.part");
    create_dummy_file(&staging_part, b"DIRTY_STAGING_PART");

    // Outside file in parent / music folder
    let outside_music = temp.path().join("important_user_audio.flac");
    create_dummy_file(&outside_music, b"IMPORTANT_SAVED_AUDIO_DO_NOT_DELETE");

    assert!(staging_part.exists());
    assert!(outside_music.exists());

    let summary = cleanup_staging_and_recover_stuck_queue(&pool, Some(&staging_dir))
        .await
        .unwrap();

    assert_eq!(summary.purged_staging_files, 1);
    assert!(!staging_part.exists(), "Staging part must be deleted");
    assert!(outside_music.exists(), "External file outside staging must remain completely untouched");
}
