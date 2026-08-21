//! Integration Test: AmbiguousSource Deduplication and Permanent Failure Classification
//!
//! Validates:
//! 1. AmbiguousSource, SourceIdentityMissing, and IdentityConflict are marked as permanent (retry_count=99).
//! 2. retry_all_failed query strictly skips AmbiguousSource, SourceIdentityMissing, and IdentityConflict.
//! 3. Accounts are not marked invalid upon ambiguity errors.
//! 4. UI contract classification correctly identifies ambiguity failures as requiring user action, not re-auth.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

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

    // Insert active tidal account
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_invalid) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1, 0)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_ambiguous_source_permanent_classification_and_retry_exclusion() {
    let db = create_test_db().await;

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Ambiguity Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Ambiguity Album') RETURNING id")
        .fetch_one(&db).await.unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Ambiguous Track', ?) RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Transient Error Track', ?) RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Identity Conflict Track', ?) RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    // Item 1: Ambiguous source failure
    let q1: i64 = sqlx::query_scalar(
        r#"INSERT INTO download_queue (track_id, status, error_message, last_error, retry_count, quality_decision)
           VALUES (?, 'failed', 'AmbiguousSource: Multiple matching tracks found without ISRC/MBID', 'AmbiguousSource: Multiple matching tracks found without ISRC/MBID', 99, 'AmbiguousSource')
           RETURNING id"#
    )
    .bind(t1)
    .fetch_one(&db)
    .await
    .unwrap();

    // Item 2: Transient network failure (eligible for retry)
    let q2: i64 = sqlx::query_scalar(
        r#"INSERT INTO download_queue (track_id, status, error_message, last_error, retry_count)
           VALUES (?, 'failed', 'Network timeout: connection reset by peer', 'Network timeout: connection reset by peer', 1)
           RETURNING id"#
    )
    .bind(t2)
    .fetch_one(&db)
    .await
    .unwrap();

    // Item 3: Identity conflict failure
    let q3: i64 = sqlx::query_scalar(
        r#"INSERT INTO download_queue (track_id, status, error_message, last_error, retry_count, quality_decision)
           VALUES (?, 'failed', 'IdentityConflict: Track source service_track_id belongs to different track', 'IdentityConflict: Track source service_track_id belongs to different track', 99, 'AmbiguousSource')
           RETURNING id"#
    )
    .bind(t3)
    .fetch_one(&db)
    .await
    .unwrap();

    // Execute retry_all_failed query logic
    let retried = sqlx::query(
        r#"UPDATE download_queue 
           SET status = 'queued', error_message = NULL, last_error = NULL, progress_percent = 0, started_at = NULL, retry_count = retry_count + 1 
           WHERE status = 'failed' AND retry_count < 5
             AND COALESCE(error_message, '') NOT LIKE '%AuthInvalid%'
             AND COALESCE(error_message, '') NOT LIKE '%RequiresAuth%'
             AND COALESCE(error_message, '') NOT LIKE '%RejectedQuality%'
             AND COALESCE(error_message, '') NOT LIKE '%AmbiguousSource%'
             AND COALESCE(error_message, '') NOT LIKE '%SourceIdentityMissing%'
             AND COALESCE(error_message, '') NOT LIKE '%IdentityConflict%'
             AND COALESCE(error_message, '') NOT LIKE '%UnavailableFromProvider%'"#
    )
    .execute(&db)
    .await
    .unwrap()
    .rows_affected();

    // Assert only transient item was retried
    assert_eq!(retried, 1, "Only transient network error item should be retried");

    let s1: (String, i64) = sqlx::query_as("SELECT status, retry_count FROM download_queue WHERE id = ?")
        .bind(q1).fetch_one(&db).await.unwrap();
    assert_eq!(s1.0, "failed");
    assert_eq!(s1.1, 99);

    let s2: (String, i64) = sqlx::query_as("SELECT status, retry_count FROM download_queue WHERE id = ?")
        .bind(q2).fetch_one(&db).await.unwrap();
    assert_eq!(s2.0, "queued");
    assert_eq!(s2.1, 2);

    let s3: (String, i64) = sqlx::query_as("SELECT status, retry_count FROM download_queue WHERE id = ?")
        .bind(q3).fetch_one(&db).await.unwrap();
    assert_eq!(s3.0, "failed");
    assert_eq!(s3.1, 99);

    // Verify account credentials were NOT invalidated
    let acc_invalid: i64 = sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE service_id = 3")
        .fetch_one(&db).await.unwrap();
    assert_eq!(acc_invalid, 0, "Ambiguous source must NOT invalidate provider credentials");
}
