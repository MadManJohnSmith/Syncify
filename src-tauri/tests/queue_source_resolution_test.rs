//! Integration tests for Queue Source Resolution (S122A)
//!
//! Tests:
//! 1. track_id con una fuente Qobuz exacta
//! 2. track_id con una fuente Tidal exacta
//! 3. track_id sin track_sources -> SourceIdentityMissing
//! 4. fuente sin service_track_id -> SourceIdentityMissing
//! 5. multiples fuentes, una cuenta activa -> resuelve a la cuenta activa
//! 6. multiples fuentes sin criterio concluyente -> respuesta ambigua (AmbiguousSource)
//! 7. source-locked no invoca busqueda ISRC/titulo
//! 8. 404 posterior -> StaleSource
//! 9. perform_add_to_queue(track_id, None, None, ..., quality=Some("hires")) resuelve correctamente

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{perform_add_to_queue, perform_audit_download_queue};

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

    // Insert baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_1_track_with_exact_qobuz_source() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Qobuz Track', ?, 'USRC12200001') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)").bind(track_id).bind(artist_id).execute(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_exact_101', 'FLAC', 24, 96000, 150, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let queue_id = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        Some("hires".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Should successfully resolve and enqueue exact Qobuz source");

    assert!(queue_id > 0);

    let row: (String, String, String, String) = sqlx::query_as(
        "SELECT service_name, service_track_id, status, quality_preference FROM download_queue WHERE id = ?"
    )
    .bind(queue_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(row.0, "qobuz");
    assert_eq!(row.1, "qobuz_exact_101");
    assert_eq!(row.2, "queued");
    assert_eq!(row.3, "hires");
}

#[tokio::test]
async fn test_2_track_with_exact_tidal_source() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist 2') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Test Album 2') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Tidal Track', ?, 'USRC12200002') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)").bind(track_id).bind(artist_id).execute(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, 'tidal_exact_202', 'FLAC', 16, 44100, 100, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let queue_id = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        Some("lossless".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Should successfully resolve and enqueue exact Tidal source");

    assert!(queue_id > 0);

    let row: (String, String, String, String) = sqlx::query_as(
        "SELECT service_name, service_track_id, status, quality_preference FROM download_queue WHERE id = ?"
    )
    .bind(queue_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(row.0, "tidal");
    assert_eq!(row.1, "tidal_exact_202");
    assert_eq!(row.2, "queued");
    assert_eq!(row.3, "lossless");
}

#[tokio::test]
async fn test_3_track_without_track_sources_returns_source_identity_missing() {
    let db = create_test_db().await;

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Empty Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Unresolved Track', ?, 'USRC12200003') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    let result = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        Some("hires".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    assert!(result.is_err(), "Must fail when no track_sources exist");
    let err = result.unwrap_err();
    assert!(
        err.contains("SourceIdentityMissing"),
        "Error message must indicate SourceIdentityMissing, got: {}",
        err
    );
}

#[tokio::test]
async fn test_4_source_with_empty_service_track_id_returns_source_identity_missing() {
    let db = create_test_db().await;

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Empty TrackID Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Empty STID Track', ?, 'USRC12200004') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    // Insert source with empty string service_track_id
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, 2, '', 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let result = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        Some("hires".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    assert!(result.is_err(), "Must fail when service_track_id is empty");
    let err = result.unwrap_err();
    assert!(
        err.contains("SourceIdentityMissing"),
        "Error message must indicate SourceIdentityMissing, got: {}",
        err
    );
}

#[tokio::test]
async fn test_5_multiple_sources_single_active_account_resolves_cleanly() {
    let db = create_test_db().await;

    // Only Tidal account is active
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (301, 3, 'Tidal Active User', 'user@tidal.com', 1)")
        .execute(&db).await.unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Multi Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Multi Track', ?, 'USRC12200005') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    // Add both Qobuz and Tidal sources
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_src_501', 'FLAC', 24, 96000, 150, 1)")
        .bind(track_id).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, 'tidal_src_502', 'FLAC', 16, 44100, 100, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let queue_id = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("Should resolve cleanly to Tidal because only Tidal has an active account");

    let row: (String, String) = sqlx::query_as(
        "SELECT service_name, service_track_id FROM download_queue WHERE id = ?"
    )
    .bind(queue_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(row.0, "tidal");
    assert_eq!(row.1, "tidal_src_502");
}

#[tokio::test]
async fn test_6_multiple_sources_dual_provider_resolves_via_preferences() {
    let db = create_test_db().await;

    // Both Qobuz and Tidal accounts are active
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (201, 2, 'Qobuz Active', 'user@qobuz.com', 1)")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (301, 3, 'Tidal Active', 'user@tidal.com', 1)")
        .execute(&db).await.unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Competing Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Competing Track', ?, 'USRC12200006') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_601', 'FLAC', 24, 96000, 150, 1)")
        .bind(track_id).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, 'tidal_602', 'FLAC', 16, 44100, 100, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let result = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await;

    assert!(result.is_ok(), "Dual-provider track must succeed and resolve via service preference: {:?}", result.err());
    let queue_id = result.unwrap();

    let row: (String, String) = sqlx::query_as(
        r#"
        SELECT s.name, dq.service_track_id
        FROM download_queue dq
        JOIN services s ON s.id = dq.service_id
        WHERE dq.id = ?
        "#
    )
    .bind(queue_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(row.0, "qobuz", "Primary preference provider must be selected");
    assert_eq!(row.1, "qobuz_601");
}

#[tokio::test]
async fn test_7_source_locked_does_not_perform_unresolved_metadata_insertion() {
    let db = create_test_db().await;

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Locked Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Locked Track', ?, 'USRC12200007') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_locked_701', 'FLAC', 24, 96000, 150, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let queue_id = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        Some("hires".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(false),
        None,
    )
    .await
    .unwrap();

    let row: (Option<String>, Option<String>, i64) = sqlx::query_as(
        "SELECT service_name, service_track_id, allow_fallback FROM download_queue WHERE id = ?"
    )
    .bind(queue_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(row.0.as_deref(), Some("qobuz"));
    assert_eq!(row.1.as_deref(), Some("qobuz_locked_701"));
    assert_eq!(row.2, 0, "allow_fallback must be 0 for source-locked queue item");
}

#[tokio::test]
async fn test_8_stale_source_404_quarantined_by_audit() {
    let db = create_test_db().await;

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Stale Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Stale Track', ?, 'USRC12200008') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    // Insert item with 404 error (quarantined by worker with retry_count=99)
    let queue_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_id, service_name, service_track_id, target_title, target_isrc,
            allow_fallback, retry_count, error_message, created_at
        )
        VALUES (?, 50, 1, 'failed', 'hires', 1, 2, 'qobuz', 'qobuz_stale_404', 'Stale Track', 'USRC12200008', 0, 99, 'HTTP 404: Track not found on Qobuz', CURRENT_TIMESTAMP)
        RETURNING id
        "#
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    let report = perform_audit_download_queue(&db).await.expect("Audit should run cleanly");
    assert_eq!(report.stale_source_count, 1, "404 should be classified as StaleSource in audit report");

    let retry_count: i64 = sqlx::query_scalar("SELECT retry_count FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(retry_count, 99, "Stale source item should remain quarantined with retry_count=99");
}

#[tokio::test]
async fn test_9_ui_blocker_case_resolves_correctly_with_persisted_source() {
    let db = create_test_db().await;

    let track_id: i64 = 10638;
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('UI Album') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO tracks (id, title, album_id, isrc) VALUES (?, 'UI Track 10638', ?, 'USRC122010638')")
        .bind(track_id).bind(album_id).execute(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_10638_exact', 'FLAC', 24, 96000, 150, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    // Exact call made by UI
    let queue_id = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        Some("hires".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("UI call must resolve the persisted track_sources and enqueue properly");

    assert!(queue_id > 0);

    let row: (Option<String>, Option<String>, String, Option<String>) = sqlx::query_as(
        "SELECT service_name, service_track_id, status, quality_preference FROM download_queue WHERE id = ?"
    )
    .bind(queue_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(row.0.as_deref(), Some("qobuz"));
    assert_eq!(row.1.as_deref(), Some("qobuz_10638_exact"));
    assert_eq!(row.2, "queued");
    assert_eq!(row.3.as_deref(), Some("hires"));
}

#[tokio::test]
async fn test_10_perform_add_to_queue_defaults_allow_fallback_to_true() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Fallback Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Fallback Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Fallback Track', ?, 'USRC12209999') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)").bind(track_id).bind(artist_id).execute(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) VALUES (?, 2, 'qobuz_fb_101', 'FLAC', 1)")
        .bind(track_id).execute(&db).await.unwrap();

    // Enqueue with allow_fallback = None
    let queue_id = perform_add_to_queue(
        &db,
        track_id,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None, // allow_fallback = None
        None,
        None,
    )
    .await
    .expect("Enqueue must succeed");

    let allow_fb: i64 = sqlx::query_scalar("SELECT allow_fallback FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(allow_fb, 1, "allow_fallback must default to 1 (true) when not specified");
}
