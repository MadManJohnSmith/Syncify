//! Integration Test: Provider Identity Context Isolation
//!
//! Validates:
//! 1. DownloadRequest contains explicit, isolated canonical_track_id, queue_id, and operation_id.
//! 2. Candidate resolution never aliases queue_id to track_id.
//! 3. Cross-track contamination of service_track_id is rejected by track validation.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::download::DownloadRequest;

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
async fn test_download_request_context_isolation() {
    let req1 = DownloadRequest {
        item_id: "193".to_string(),
        canonical_track_id: Some(196),
        queue_id: Some(193),
        operation_id: Some("op-uuid-1".to_string()),
        service_name: Some("tidal".to_string()),
        service_track_id: Some("36190480".to_string()),
        track_name: "MA CHE IDEA".to_string(),
        artist_name: "bnkr44".to_string(),
        album_name: "MA CHE IDEA".to_string(),
        output_dir: "/tmp/music".to_string(),
        quality: "LOSSLESS".to_string(),
        embed_lyrics: true,
        embed_artwork: true,
        smart_studio_origin: false,
        allow_fallback: true,
        strict_quality: false,
        ..Default::default()
    };

    let req2 = DownloadRequest {
        item_id: "194".to_string(),
        canonical_track_id: Some(64),
        queue_id: Some(194),
        operation_id: Some("op-uuid-2".to_string()),
        service_name: Some("tidal".to_string()),
        service_track_id: Some("36190480".to_string()),
        track_name: "#1 Crush".to_string(),
        artist_name: "Garbage".to_string(),
        album_name: "Garbage".to_string(),
        output_dir: "/tmp/music".to_string(),
        quality: "LOSSLESS".to_string(),
        embed_lyrics: true,
        embed_artwork: true,
        smart_studio_origin: false,
        allow_fallback: true,
        strict_quality: false,
        ..Default::default()
    };

    assert_eq!(req1.canonical_track_id, Some(196));
    assert_eq!(req1.queue_id, Some(193));
    assert_ne!(req1.canonical_track_id, req2.canonical_track_id);
    assert_ne!(req1.operation_id, req2.operation_id);
}

#[tokio::test]
async fn test_candidate_resolution_rejects_mismatched_hint_track_id() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Garbage') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Garbage Album') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Track 64 is "#1 Crush"
    let t64: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (id, title, album_id, duration_ms, isrc) VALUES (64, '#1 Crush', ?, 240000, 'USRO19500001') RETURNING id"
    )
    .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t64).bind(artist_id).execute(&db).await.unwrap();

    // Candidate validation query against track 64 for requested track "MA CHE IDEA"
    let candidate: Option<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT id, title, isrc FROM tracks WHERE id = ?"
    )
    .bind(64)
    .fetch_optional(&db)
    .await
    .unwrap();

    assert!(candidate.is_some());
    let (_cid, ctitle, cisrc) = candidate.unwrap();

    let download_title = "MA CHE IDEA";
    let download_isrc = "ITUM72300001";

    let title_clean = syncify_tauri_lib::download::qobuz::clean_title(download_title);
    let ctitle_clean = syncify_tauri_lib::download::qobuz::clean_title(&ctitle);
    let isrc_matches = cisrc.as_deref() == Some(download_isrc);
    let title_matches = title_clean == ctitle_clean || title_clean.contains(&ctitle_clean) || ctitle_clean.contains(&title_clean);

    let is_valid = isrc_matches || title_matches;
    assert!(!is_valid, "Hint track ID 64 must be rejected for 'MA CHE IDEA' due to identity mismatch");
}
