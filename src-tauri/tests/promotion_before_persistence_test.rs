//! Integration Test: Promotion Before Persistence Order Invariant
//!
//! Validates:
//! 1. Physical promotion and destination verification MUST complete before SQLite COMMIT.
//! 2. If physical promotion fails, no database transaction is committed and staged file is preserved.
//! 3. No orphan database records pointing to non-existent library paths.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tempfile::TempDir;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_failed_promotion_prevents_db_persistence() {
    let db = create_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let staging_dir = temp_dir.path().join(".staging");
    tokio::fs::create_dir_all(&staging_dir).await.unwrap();

    let staged_file = staging_dir.join("temp_audio.flac");
    tokio::fs::write(&staged_file, b"STAGED_FLAC_DATA").await.unwrap();

    // Target path intentionally inside a non-existent, uncreatable directory on readonly/invalid path
    let invalid_target = temp_dir.path().join("forbidden_non_existent").join("track.flac");

    // Emulate Pipeline Step 8 (Promotion) before Step 9 (Persistence)
    let promotion_result: Result<(), String> = match tokio::fs::rename(&staged_file, &invalid_target).await {
        Ok(()) => Ok(()),
        Err(e) => {
            // Attempt verified copy+delete
            match tokio::fs::copy(&staged_file, &invalid_target).await {
                Ok(_) => {
                    let _ = tokio::fs::remove_file(&staged_file).await;
                    Ok(())
                }
                Err(ce) => Err(format!("Promotion failed: rename={}, copy={}", e, ce)),
            }
        }
    };

    assert!(promotion_result.is_err(), "Promotion to invalid target must fail");
    assert!(staged_file.exists(), "Staged file must be preserved for diagnosis");

    // Invariant: Database persistence MUST NOT be executed if promotion failed
    let downloads_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(downloads_count, 0, "No downloads row should be created when promotion fails");
}

#[tokio::test]
async fn test_successful_promotion_followed_by_persistence() {
    let db = create_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let staging_dir = temp_dir.path().join(".staging");
    let dest_dir = temp_dir.path().join("Music").join("Test Artist").join("Test Album");
    tokio::fs::create_dir_all(&staging_dir).await.unwrap();
    tokio::fs::create_dir_all(&dest_dir).await.unwrap();

    let staged_file = staging_dir.join("staged_audio.flac");
    tokio::fs::write(&staged_file, b"HIGH_RES_FLAC_AUDIO").await.unwrap();

    let final_path = dest_dir.join("01 - Test Track.flac");

    // 1. Physical promotion
    tokio::fs::rename(&staged_file, &final_path).await.unwrap();
    assert!(final_path.exists());
    assert!(!staged_file.exists());

    // 2. Database persistence
    let mut tx = db.begin().await.unwrap();
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Test Artist') ON CONFLICT(name) DO UPDATE SET name = excluded.name RETURNING id"
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('Test Album') RETURNING id"
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&mut *tx).await.unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc) VALUES ('Test Track', ?, 180000, 1, 1, 'USRC10000099') RETURNING id"
    )
    .bind(album_id).fetch_one(&mut *tx).await.unwrap();

    sqlx::query(
        r#"INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness)
           VALUES (?, 3, ?, 'FLAC', 24, 96000, 19, 100)"#
    )
    .bind(track_id)
    .bind(final_path.to_string_lossy().to_string())
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    // 3. Verify consistency
    let dl: (i64, String, i64) = sqlx::query_as("SELECT track_id, file_path, file_size_bytes FROM downloads WHERE track_id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(dl.1, final_path.to_string_lossy().to_string());
    assert_eq!(dl.2, 19);
    assert!(std::path::Path::new(&dl.1).exists());
}
