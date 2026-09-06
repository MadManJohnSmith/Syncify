//! Integration tests for TASK-56: Real URL import enqueuing in `commands/url_import.rs`.
//!
//! Validates:
//! 1. Parsing URLs from Spotify, Tidal, Qobuz, Deezer and error handling for malformed URLs.
//! 2. Malformed URLs return descriptive errors without creating ghost entries in `tracks` or `download_queue`.
//! 3. Tidal track URL resolution and insertion into `download_queue` with status 'queued'.
//! 4. Spotify track URL cross-service resolution via SongLink into native engine (Tidal/Qobuz) in `download_queue`.
//! 5. Graceful fallback when SongLink is unreachable.
//! 6. Idempotent enqueuing (re-importing the same URL reuses existing queue row).
//! 7. Direct `DownloadOrchestrator::enqueue_from_url` integration.
//! 8. Non-track URLs (albums/playlists) rejected safely without creating ghost entries.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;
use syncify_tauri_lib::commands::url_import::{
    parse_streaming_url, perform_import_from_url, perform_import_from_url_with_quality,
};
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::songlink::SongLinkClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Helper to set up an in-memory SQLite database with schema
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (4, 'deezer', 1, 'lossless')")
        .execute(&pool).await.unwrap();

    pool
}

/// Spawns a local HTTP server that serves mock SongLink responses
async fn spawn_mock_songlink_server() -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, mut rx) = oneshot::channel();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok((mut stream, _)) = listener.accept() => {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    let req_str = String::from_utf8_lossy(&buf[..n]);

                    let (status, body) = if req_str.contains("spotify_tidal_match") {
                        (
                            "200 OK",
                            r#"{
                                "entityUniqueId": "TIDAL_SONG::34782012",
                                "linksByPlatform": {
                                    "tidal": {
                                        "country": "US",
                                        "url": "https://tidal.com/browse/track/34782012",
                                        "entityUniqueId": "TIDAL_SONG::34782012"
                                    },
                                    "spotify": {
                                        "country": "US",
                                        "url": "https://open.spotify.com/track/spotify_tidal_match",
                                        "entityUniqueId": "SPOTIFY_SONG::spotify_tidal_match"
                                    }
                                },
                                "entitiesByUniqueId": {
                                    "TIDAL_SONG::34782012": {
                                        "id": "34782012",
                                        "type": "song",
                                        "title": "SongLink Resolved Title",
                                        "artistName": "SongLink Resolved Artist",
                                        "apiProvider": "tidal"
                                    }
                                }
                            }"#,
                        )
                    } else if req_str.contains("spotify_qobuz_match") {
                        (
                            "200 OK",
                            r#"{
                                "entityUniqueId": "QOBUZ_SONG::19827364",
                                "linksByPlatform": {
                                    "qobuz": {
                                        "country": "US",
                                        "url": "https://open.qobuz.com/track/19827364",
                                        "entityUniqueId": "QOBUZ_SONG::19827364"
                                    }
                                },
                                "entitiesByUniqueId": {
                                    "QOBUZ_SONG::19827364": {
                                        "id": "19827364",
                                        "type": "song",
                                        "title": "Qobuz Resolved Track",
                                        "artistName": "Qobuz HiRes Artist",
                                        "apiProvider": "qobuz"
                                    }
                                }
                            }"#,
                        )
                    } else {
                        (
                            "404 Not Found",
                            r#"{"statusCode": 404, "message": "Not found"}"#,
                        )
                    };

                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.flush().await;
                }
                _ = &mut rx => {
                    break;
                }
            }
        }
    });

    (format!("http://{}", addr), tx)
}

#[tokio::test]
async fn test_parse_streaming_url_matrix() {
    // Spotify URLs
    let p_spot = parse_streaming_url("https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT?si=abc")
        .expect("Spotify track URL must parse");
    assert_eq!(p_spot.service, "spotify");
    assert_eq!(p_spot.content_type, "track");
    assert_eq!(p_spot.id, "4cOdK2wGLETKBW3PvgPWqT");

    // Tidal URLs
    let p_tidal1 = parse_streaming_url("https://tidal.com/browse/track/34782012")
        .expect("Tidal browse track URL must parse");
    assert_eq!(p_tidal1.service, "tidal");
    assert_eq!(p_tidal1.content_type, "track");
    assert_eq!(p_tidal1.id, "34782012");

    let p_tidal2 = parse_streaming_url("https://listen.tidal.com/track/12345678")
        .expect("Tidal direct track URL must parse");
    assert_eq!(p_tidal2.service, "tidal");
    assert_eq!(p_tidal2.content_type, "track");
    assert_eq!(p_tidal2.id, "12345678");

    // Qobuz URLs
    let p_qobuz = parse_streaming_url("https://open.qobuz.com/track/19827364")
        .expect("Qobuz track URL must parse");
    assert_eq!(p_qobuz.service, "qobuz");
    assert_eq!(p_qobuz.content_type, "track");
    assert_eq!(p_qobuz.id, "19827364");

    // Deezer URLs
    let p_deezer = parse_streaming_url("https://www.deezer.com/track/987654321")
        .expect("Deezer track URL must parse");
    assert_eq!(p_deezer.service, "deezer");
    assert_eq!(p_deezer.content_type, "track");
    assert_eq!(p_deezer.id, "987654321");

    // Malformed and unsupported URLs
    assert!(parse_streaming_url("https://google.com/search?q=song").is_err());
    assert!(parse_streaming_url("https://open.spotify.com/").is_err());
    assert!(parse_streaming_url("not a url at all").is_err());
    assert!(parse_streaming_url("").is_err());
}

#[tokio::test]
async fn test_malformed_url_returns_error_and_no_ghost_queue() {
    let pool = setup_test_db().await;

    // Verify initial state is completely clean
    let init_queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    let init_tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(init_queue_count, 0);
    assert_eq!(init_tracks_count, 0);

    // Attempt import with unsupported domain
    let err_unsupported = perform_import_from_url(&pool, None, "https://youtube.com/watch?v=dQw4w9WgXcQ").await;
    assert!(err_unsupported.is_err(), "Unsupported URL must return error");
    let msg = err_unsupported.unwrap_err();
    assert!(
        msg.contains("Unsupported URL"),
        "Error message must be descriptive: {}",
        msg
    );

    // Attempt import with malformed Spotify URL
    let err_malformed = perform_import_from_url(&pool, None, "https://open.spotify.com/invalid_shape").await;
    assert!(err_malformed.is_err());

    // Attempt import with empty string
    let err_empty = perform_import_from_url(&pool, None, "").await;
    assert!(err_empty.is_err());

    // Invariant: zero ghost entries in download_queue or tracks
    let final_queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    let final_tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(
        final_queue_count, 0,
        "No ghost entries may be created in download_queue upon error"
    );
    assert_eq!(
        final_tracks_count, 0,
        "No ghost entries may be created in tracks upon error"
    );
}

#[tokio::test]
async fn test_import_tidal_url_resolves_and_enqueues_into_download_queue() {
    let pool = setup_test_db().await;

    let res = perform_import_from_url(
        &pool,
        None,
        "https://tidal.com/browse/track/34782012",
    )
    .await
    .expect("Tidal track import must succeed");

    assert_eq!(res.service, "tidal");
    assert_eq!(res.content_type, "track");
    assert_eq!(res.id, "34782012");
    assert!(res.queue_id.is_some(), "queue_id must be populated");
    assert!(res.track_id.is_some(), "track_id must be populated");
    assert!(res.title.is_some(), "title must be populated");
    assert!(res.artist.is_some(), "artist must be populated");
    assert_eq!(
        res.status.as_deref(),
        Some("queued"),
        "Initial queue status must be 'queued'"
    );

    let qid = res.queue_id.unwrap();
    let tid = res.track_id.unwrap();

    // Verify row in download_queue
    let queue_row: (i64, String, String, String, i64, Option<String>) = sqlx::query_as(
        "SELECT track_id, service_name, service_track_id, status, priority, requested_quality FROM download_queue WHERE id = ?"
    )
    .bind(qid)
    .fetch_one(&pool)
    .await
    .expect("Row must exist in download_queue");

    assert_eq!(queue_row.0, tid);
    assert_eq!(queue_row.1, "tidal");
    assert_eq!(queue_row.2, "34782012");
    assert_eq!(queue_row.3, "queued");
    assert_eq!(queue_row.4, 50);
    assert_eq!(queue_row.5.as_deref(), Some("lossless"));

    // Verify row in tracks table
    let track_title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
        .bind(tid)
        .fetch_one(&pool)
        .await
        .expect("Row must exist in tracks");
    assert!(!track_title.is_empty());

    // Verify track_sources table
    let source_track_id: String = sqlx::query_scalar(
        "SELECT service_track_id FROM track_sources WHERE track_id = ? AND service_id = 3",
    )
    .bind(tid)
    .fetch_one(&pool)
    .await
    .expect("track_sources must record Tidal mapping");
    assert_eq!(source_track_id, "34782012");
}

#[tokio::test]
async fn test_import_spotify_url_resolves_via_songlink_and_enqueues_matched_native_engine() {
    let pool = setup_test_db().await;
    let (base_url, _tx) = spawn_mock_songlink_server().await;

    let songlink = Arc::new(SongLinkClient::new().with_base_url(base_url));
    let orchestrator = DownloadOrchestrator::new().with_songlink(songlink);

    let res = perform_import_from_url_with_quality(
        &pool,
        Some(&orchestrator),
        "https://open.spotify.com/track/spotify_tidal_match",
        Some("hires"),
    )
    .await
    .expect("Spotify import with SongLink mock must succeed");

    assert_eq!(res.service, "spotify");
    assert_eq!(res.id, "spotify_tidal_match");
    assert_eq!(res.title.as_deref(), Some("SongLink Resolved Title"));
    assert_eq!(res.artist.as_deref(), Some("SongLink Resolved Artist"));
    assert_eq!(res.status.as_deref(), Some("queued"));

    let qid = res.queue_id.unwrap();
    let tid = res.track_id.unwrap();

    // Verify row in download_queue routed to matched native engine (Tidal ID 34782012)
    let queue_row: (
        i64,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        r#"
        SELECT track_id, service_name, service_track_id, status,
               origin_service, origin_service_track_id, effective_service, match_method
        FROM download_queue
        WHERE id = ?
        "#,
    )
    .bind(qid)
    .fetch_one(&pool)
    .await
    .expect("Row must exist in download_queue");

    assert_eq!(queue_row.0, tid);
    assert_eq!(queue_row.1, "tidal", "Must be routed to native Tidal engine");
    assert_eq!(queue_row.2, "34782012", "Must use Tidal track ID from SongLink");
    assert_eq!(queue_row.3, "queued");
    assert_eq!(queue_row.4.as_deref(), Some("spotify"));
    assert_eq!(queue_row.5.as_deref(), Some("spotify_tidal_match"));
    assert_eq!(queue_row.6.as_deref(), Some("tidal"));
    assert_eq!(queue_row.7.as_deref(), Some("songlink_cross_platform"));
}

#[tokio::test]
async fn test_import_spotify_url_without_songlink_falls_back_gracefully() {
    let pool = setup_test_db().await;

    // Point SongLink to an unreachable port to test offline / failure resilience
    let unreachable_client = Arc::new(SongLinkClient::new().with_base_url("http://127.0.0.1:1".to_string()));
    let orchestrator = DownloadOrchestrator::new().with_songlink(unreachable_client);

    let res = perform_import_from_url(
        &pool,
        Some(&orchestrator),
        "https://open.spotify.com/track/spotify_offline_123",
    )
    .await
    .expect("Should fallback gracefully when SongLink is offline");

    assert_eq!(res.service, "spotify");
    assert_eq!(res.id, "spotify_offline_123");
    assert!(res.queue_id.is_some());
    assert_eq!(res.status.as_deref(), Some("queued"));

    let qid = res.queue_id.unwrap();
    let q_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q_status, "queued");
}

#[tokio::test]
async fn test_import_url_idempotency_reuses_existing_queue_item() {
    let pool = setup_test_db().await;

    let res1 = perform_import_from_url(
        &pool,
        None,
        "https://tidal.com/browse/track/99887766",
    )
    .await
    .expect("First import must succeed");

    let res2 = perform_import_from_url(
        &pool,
        None,
        "https://tidal.com/browse/track/99887766",
    )
    .await
    .expect("Second import of same URL must succeed");

    assert_eq!(
        res1.queue_id, res2.queue_id,
        "Subsequent imports of identical track must reuse existing queue_id"
    );

    let total_queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        total_queued, 1,
        "Exactly 1 row must exist in download_queue without duplicate entries"
    );
}

#[tokio::test]
async fn test_orchestrator_enqueue_from_url_integration() {
    let pool = setup_test_db().await;
    let orchestrator = DownloadOrchestrator::new();

    let res = orchestrator
        .enqueue_from_url(&pool, "https://tidal.com/browse/track/55443322")
        .await
        .expect("orchestrator.enqueue_from_url must succeed");

    assert_eq!(res.service, "tidal");
    assert_eq!(res.id, "55443322");
    assert!(res.queue_id.is_some());
    assert_eq!(res.status.as_deref(), Some("queued"));

    let q_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE id = ?")
        .bind(res.queue_id.unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q_count, 1);
}

#[tokio::test]
async fn test_unsupported_content_type_rejected_without_queueing() {
    let pool = setup_test_db().await;

    // Album URL should be rejected for single-track queue import
    let err = perform_import_from_url(&pool, None, "https://open.spotify.com/album/4aawyAB9vmqN3uQ7FjRGTy").await;
    assert!(err.is_err(), "Album URL must be rejected for track queue import");
    let msg = err.unwrap_err();
    assert!(
        msg.contains("supports individual tracks"),
        "Error should explain only tracks are supported: {}",
        msg
    );

    let q_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q_count, 0, "No rows added on non-track URL import");
}
