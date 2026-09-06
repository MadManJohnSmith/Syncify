//! Tests for TASK-108:
//! Service import pagination (Apple Music), purchases persistence (Qobuz is_purchased = 1),
//! and added_at timestamp normalization (no NULL, no 1970 epoch).

use std::sync::{Arc, Mutex};
use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::import_pagination::{
    next_apple_music_offset, normalize_added_at, parse_apple_music_next_offset,
};
use syncify_tauri_lib::services::{AppleMusicClient, QobuzClient};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

static DB_LOCK: Mutex<()> = Mutex::new(());

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

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct RecordedRequest {
    target: String,
    headers: Vec<(String, String)>,
}

async fn spawn_mock_server<F>(handler: F) -> (String, Arc<Mutex<Vec<RecordedRequest>>>)
where
    F: Fn(&str) -> (u16, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::<RecordedRequest>::new()));
    let reqs = requests.clone();
    let handler = Arc::new(handler);

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { break };
            let reqs = reqs.clone();
            let handler = handler.clone();

            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);

                let mut lines = raw.lines();
                let request_line = lines.next().unwrap_or("");
                let target = request_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string();

                let mut headers = Vec::new();
                for line in lines {
                    if line.is_empty() || line == "\r" {
                        break;
                    }
                    if let Some((k, v)) = line.split_once(':') {
                        headers.push((k.trim().to_lowercase(), v.trim().to_string()));
                    }
                }

                reqs.lock().unwrap().push(RecordedRequest {
                    target: target.clone(),
                    headers,
                });

                let (status, body) = handler(&target);
                let status_text = match status {
                    200 => "OK",
                    401 => "Unauthorized",
                    404 => "Not Found",
                    _ => "Response",
                };

                let resp = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status, status_text, body.len(), body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });

    (format!("http://{}", addr), requests)
}

#[tokio::test]
async fn test_apple_music_pagination_multiple_pages_imported() {
    let _guard = DB_LOCK.lock().unwrap();
    let pool = setup_test_db().await;

    // Ensure apple_music service exists
    let apple_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'apple_music'")
        .fetch_one(&pool)
        .await
        .expect("apple_music service must exist");

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Apple Music User', 1) RETURNING id",
    )
    .bind(apple_svc_id)
    .fetch_one(&pool)
    .await
    .expect("insert account must succeed");

    // Spawn mock server simulating 2 pages of library songs
    let (mock_url, recorded_requests) = spawn_mock_server(|target| {
        if target.contains("offset=0") {
            let json = serde_json::json!({
                "data": [
                    {
                        "id": "am_track_1",
                        "attributes": {
                            "name": "Song One",
                            "artistName": "Artist Alpha",
                            "albumName": "Album Alpha",
                            "durationInMillis": 210000,
                            "isrc": "USAM10000001",
                            "dateAdded": "2023-01-15T12:00:00Z"
                        }
                    },
                    {
                        "id": "am_track_2",
                        "attributes": {
                            "name": "Song Two",
                            "artistName": "Artist Alpha",
                            "albumName": "Album Alpha",
                            "durationInMillis": 180000,
                            "isrc": "USAM10000002",
                            "dateAdded": "2023-01-16T12:00:00Z"
                        }
                    }
                ],
                "next": "/v1/me/library/songs?offset=2&limit=2",
                "meta": {
                    "total": 3
                }
            });
            (200, json.to_string())
        } else if target.contains("offset=2") {
            let json = serde_json::json!({
                "data": [
                    {
                        "id": "am_track_3",
                        "attributes": {
                            "name": "Song Three",
                            "artistName": "Artist Beta",
                            "albumName": "Album Beta",
                            "durationInMillis": 240000,
                            "isrc": "USAM10000003",
                            "dateAdded": "1970-01-01T00:00:00Z" // Epoch date to be healed
                        }
                    }
                ],
                "next": null,
                "meta": {
                    "total": 3
                }
            });
            (200, json.to_string())
        } else {
            (404, r#"{"error":"not found"}"#.to_string())
        }
    }).await;

    let client = AppleMusicClient::new("fake_dev_token".into(), "fake_user_token".into())
        .with_base_url(mock_url);

    let result = client.import_library(&pool, account_id).await.expect("import_library succeeds");

    assert_eq!(result.imported, 3, "All 3 songs across 2 pages must be imported");
    assert_eq!(result.skipped, 0);

    // Verify pagination made 2 requests
    let reqs = recorded_requests.lock().unwrap();
    assert_eq!(reqs.len(), 2, "Expected 2 requests to follow pagination");

    // Verify entries in library_entries
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);

    // Verify added_at normalization healed the 1970 date
    let dates: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT t.title, le.added_at 
        FROM library_entries le 
        JOIN tracks t ON t.id = le.track_id 
        WHERE le.account_id = ?
        ORDER BY t.title ASC
        "#
    )
    .bind(account_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(dates.len(), 3);
    for (title, added_at) in &dates {
        assert!(
            !added_at.starts_with("1970"),
            "Track '{}' has unnormalized epoch added_at: {}",
            title,
            added_at
        );
        assert!(
            !added_at.trim().is_empty(),
            "Track '{}' has empty added_at",
            title
        );
    }
}

#[tokio::test]
async fn test_qobuz_purchases_import_persists_is_purchased() {
    let _guard = DB_LOCK.lock().unwrap();
    let pool = setup_test_db().await;

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("qobuz service must exist");

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Qobuz Purchases User', 1) RETURNING id",
    )
    .bind(qobuz_svc_id)
    .fetch_one(&pool)
    .await
    .expect("insert account must succeed");

    // Spawn mock Qobuz server simulating purchase/getUserPurchases with an album and tracks
    let (mock_url, _recorded_requests) = spawn_mock_server(|target| {
        if target.contains("purchase/getUserPurchases") {
            let json = serde_json::json!({
                "albums": {
                    "total": 1,
                    "items": [
                        {
                            "id": "qobuz_purchased_alb_1",
                            "title": "Purchased Master Album",
                            "artist": {
                                "id": 101,
                                "name": "Hi-Res Master Artist"
                            },
                            "maximum_bit_depth": 24,
                            "maximum_sampling_rate": 96.0,
                            "tracks": {
                                "total": 2,
                                "items": [
                                    {
                                        "id": 888001,
                                        "title": "Purchased Master Track 1",
                                        "duration": 240,
                                        "isrc": "FRQOB2300001",
                                        "maximum_bit_depth": 24,
                                        "maximum_sampling_rate": 96.0,
                                        "performer": {
                                            "id": 101,
                                            "name": "Hi-Res Master Artist"
                                        }
                                    },
                                    {
                                        "id": 888002,
                                        "title": "Purchased Master Track 2",
                                        "duration": 300,
                                        "isrc": "FRQOB2300002",
                                        "maximum_bit_depth": 24,
                                        "maximum_sampling_rate": 96.0,
                                        "performer": {
                                            "id": 101,
                                            "name": "Hi-Res Master Artist"
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            });
            (200, json.to_string())
        } else {
            (404, r#"{"error":"not found"}"#.to_string())
        }
    }).await;

    let client = QobuzClient::new_with_token(
        "mock_app_id".into(),
        "mock_app_secret".into(),
        "mock_user_token".into(),
    )
    .with_base_url(mock_url);

    let result = client.import_purchases(&pool, account_id).await.expect("import_purchases succeeds");
    assert_eq!(result.imported, 2, "2 purchased tracks must be imported");

    // Assert that is_purchased is strictly 1 in library_entries
    let purchased_entries: Vec<(i64, i32, i32, String)> = sqlx::query_as(
        "SELECT track_id, is_liked, is_purchased, added_at FROM library_entries WHERE account_id = ?"
    )
    .bind(account_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(purchased_entries.len(), 2);
    for (t_id, _is_liked, is_purchased, added_at) in &purchased_entries {
        assert_eq!(*is_purchased, 1, "Track ID {} must have is_purchased = 1", t_id);
        assert!(!added_at.starts_with("1970"), "Track ID {} added_at cannot be 1970 epoch: {}", t_id, added_at);
        assert!(!added_at.is_empty(), "Track ID {} added_at cannot be empty", t_id);
    }

    // Verify track_sources was inserted with 24/96 FLAC
    let sources_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM track_sources WHERE service_id = ? AND format = 'FLAC' AND bit_depth = 24"
    )
    .bind(qobuz_svc_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sources_count, 2);
}

#[test]
fn test_pagination_and_added_at_invariants() {
    // 1. Pagination helpers
    assert_eq!(parse_apple_music_next_offset("/v1/me/library/albums?offset=150"), Some(150));
    assert_eq!(parse_apple_music_next_offset("/v1/me/library/playlists?offset=50&limit=25"), Some(50));
    assert_eq!(parse_apple_music_next_offset(""), None);

    assert_eq!(
        next_apple_music_offset(0, 100, 100, Some("/v1/me/library/songs?offset=100"), Some(300)),
        Some(100)
    );
    assert_eq!(
        next_apple_music_offset(100, 100, 100, Some("/v1/me/library/songs?offset=200"), Some(300)),
        Some(200)
    );
    assert_eq!(
        next_apple_music_offset(200, 100, 100, None, Some(300)),
        None
    );

    // 2. Added_at normalization
    let now_str = chrono::Utc::now().to_rfc3339();
    let current_year = &now_str[..4];

    // None -> Current UTC
    let r1 = normalize_added_at(None);
    assert!(r1.starts_with(current_year));

    // 1970-01-01 -> Current UTC
    let r2 = normalize_added_at(Some("1970-01-01T00:00:00Z"));
    assert!(!r2.starts_with("1970"));
    assert!(r2.starts_with(current_year));

    // Valid ISO RFC 3339 -> Preserved
    let r3 = normalize_added_at(Some("2024-05-10T15:45:00Z"));
    assert_eq!(r3, "2024-05-10T15:45:00+00:00");
}
