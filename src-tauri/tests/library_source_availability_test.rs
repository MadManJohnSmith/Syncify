// S123B: Provider Availability vs Historical Provenance Tests
// Tests distinction between:
// 1. Imported from (historical provenance)
// 2. Available on (verified active availability status: available, stale_404, region_unavailable, requires_auth, unknown_unchecked)
// 3. Downloaded from (effective download provider)

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{perform_check_track_availability, TrackSourceAvailability};

async fn setup_test_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

#[tokio::test]
async fn test_unchecked_historical_source_is_unknown_unchecked() {
    let db = setup_test_db().await;

    // 1. Create a track
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, isrc) VALUES ('Test Song', 200000, 'US1234567890') RETURNING id"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    // 2. Link imported source from Qobuz (service_id = 2) without running availability check
    let source_id: i64 = sqlx::query_scalar(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) 
         VALUES (?, 2, 'qobuz_12345', 'FLAC', 24, 96000, 95, 0) RETURNING id"
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    // 3. Verify initial status in DB defaults to unknown_unchecked
    let status: String = sqlx::query_scalar("SELECT availability_status FROM track_sources WHERE id = ?")
        .bind(source_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(status, "unknown_unchecked", "Historical source must be unknown_unchecked until verified");

    // 4. Verify that querying available sources does NOT claim it is available
    let available_cnt: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM track_sources WHERE track_id = ? AND availability_status = 'available'"
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(available_cnt, 0, "Unchecked source must NOT be listed as available");
}

#[tokio::test]
async fn test_check_availability_stale_404() {
    let db = setup_test_db().await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Stale Track', 180000) RETURNING id"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    // Insert source with 404 / stale indicator in service_track_id
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) 
         VALUES (?, 2, 'stale_404_track', 'FLAC', 1, 'unknown_unchecked')"
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    // Perform check
    let results: Vec<TrackSourceAvailability> = perform_check_track_availability(&db, track_id, Some("qobuz".to_string()))
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    let src = &results[0];
    assert_eq!(src.availability_status, "stale_404");
    assert_eq!(src.available, 0);
    assert!(src.availability_reason.as_ref().unwrap().contains("404"));
    assert!(src.last_checked.is_some());

    // Verify DB persistence
    let db_status: (String, i64) = sqlx::query_as(
        "SELECT availability_status, available FROM track_sources WHERE track_id = ? AND service_id = 2"
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(db_status.0, "stale_404");
    assert_eq!(db_status.1, 0);
}

#[tokio::test]
async fn test_check_availability_region_unavailable() {
    let db = setup_test_db().await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Geo Blocked Track', 210000) RETURNING id"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) 
         VALUES (?, 3, 'region_locked_track', 'FLAC', 1, 'unknown_unchecked')"
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let results: Vec<TrackSourceAvailability> = perform_check_track_availability(&db, track_id, Some("tidal".to_string()))
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    let src = &results[0];
    assert_eq!(src.availability_status, "region_unavailable");
    assert_eq!(src.available, 0);
    assert!(src.availability_reason.as_ref().unwrap().contains("region"));
}

#[tokio::test]
async fn test_check_availability_requires_auth() {
    let db = setup_test_db().await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Unauthenticated Track', 220000) RETURNING id"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    // Insert source with no active accounts for Tidal (service_id = 3)
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status) 
         VALUES (?, 3, 'track_requires_auth_123', 'FLAC', 1, 'unknown_unchecked')"
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let results: Vec<TrackSourceAvailability> = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    let src = &results[0];
    assert_eq!(src.availability_status, "requires_auth");
    assert_eq!(src.available, 0);
    assert!(src.availability_reason.as_ref().unwrap().contains("auth"));
}

#[tokio::test]
async fn test_provenance_separation_imported_vs_effective_download() {
    let db = setup_test_db().await;

    // 1. Insert Artist & Track
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('Provenance Song', ?, 240000, 'USXYZ1234567') RETURNING id"
    )
    .bind(album_id).fetch_one(&db).await.unwrap();

    // Link artist to track
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id).bind(artist_id).execute(&db).await.unwrap();

    // 2. Historical Provenance: Imported from Spotify via user account
    let spot_acc_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (1, 'User Spotify', 1) RETURNING id"
    ).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, 1)")
        .bind(spot_acc_id).bind(track_id).execute(&db).await.unwrap();

    // 3. Provider Availability: Qobuz source verified available, Tidal source stale_404
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available, availability_status, availability_reason) 
         VALUES (?, 2, 'qobuz_valid_999', 'FLAC', 24, 96000, 95, 1, 'available', 'Verified available on provider')"
    )
    .bind(track_id).execute(&db).await.unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available, availability_status, availability_reason) 
         VALUES (?, 3, 'tidal_old_404', 'FLAC', 16, 44100, 70, 0, 'stale_404', 'HTTP 404')"
    )
    .bind(track_id).execute(&db).await.unwrap();

    // 4. Effective Download: Downloaded from Tidal via previous fallback
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, origin_service, effective_service) 
         VALUES (?, 3, '/music/Test Artist/Test Album/01 - Provenance Song.flac', 'FLAC', 16, 44100, 'qobuz', 'tidal')"
    )
    .bind(track_id).execute(&db).await.unwrap();

    // 5. Query Library / Track Metadata and assert strict separation
    let sources: Vec<TrackSourceAvailability> = sqlx::query_as(
        r#"
        SELECT ts.id, ts.track_id, ts.service_id, s.name as service_name, ts.service_track_id,
               ts.format, ts.bit_depth, ts.sample_rate, ts.quality_score, ts.available,
               ts.availability_status, ts.availability_reason, ts.last_checked
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ?
        ORDER BY ts.quality_score DESC
        "#
    )
    .bind(track_id)
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(sources.len(), 2);
    let qobuz_src = sources.iter().find(|s| s.service_name == "qobuz").unwrap();
    let tidal_src = sources.iter().find(|s| s.service_name == "tidal").unwrap();

    assert_eq!(qobuz_src.availability_status, "available");
    assert_eq!(tidal_src.availability_status, "stale_404");

    // Check imported_from and downloaded_from
    let imported_from: String = sqlx::query_scalar(
        "SELECT s_imp.name FROM library_entries le 
         JOIN accounts acc ON acc.id = le.account_id 
         JOIN services s_imp ON s_imp.id = acc.service_id 
         WHERE le.track_id = ?"
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    let downloaded_from: String = sqlx::query_scalar(
        "SELECT effective_service FROM downloads WHERE track_id = ?"
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(imported_from, "spotify", "Historical import must report Spotify");
    assert_eq!(downloaded_from, "tidal", "Effective download provider must report Tidal");
}
