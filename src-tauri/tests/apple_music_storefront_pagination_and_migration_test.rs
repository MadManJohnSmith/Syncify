//! Apple Music completeness (DO-1): configurable storefront, `next`-driven
//! pagination, albums/playlists in the import, and the migration transfer step.
//!
//! Every HTTP call goes to an in-process mock server, so the assertions are
//! about the exact request targets the client builds (storefront, cursor,
//! endpoint) and about what the import writes to the database.

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::apple_music::{
    normalize_storefront, resolve_storefront, APPLE_MUSIC_STOREFRONT_ENV,
};
use syncify_tauri_lib::services::AppleMusicClient;
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

async fn setup_apple_account(pool: &sqlx::SqlitePool) -> (i64, i64) {
    let service_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'apple_music'")
            .fetch_one(pool)
            .await
            .expect("apple_music service must exist");

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Apple Music User', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("insert account must succeed");

    (service_id, account_id)
}

#[derive(Clone, Debug)]
struct RecordedRequest {
    method: String,
    target: String,
}

async fn spawn_mock_server<F>(handler: F) -> (String, Arc<Mutex<Vec<RecordedRequest>>>)
where
    F: Fn(&str, &str) -> (u16, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::<RecordedRequest>::new()));
    let reqs = requests.clone();
    let handler = Arc::new(handler);

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let reqs = reqs.clone();
            let handler = handler.clone();

            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);

                let mut lines = raw.lines();
                let request_line = lines.next().unwrap_or("");
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let target = parts.next().unwrap_or("").to_string();

                let (status, body) = handler(&method, &target);

                reqs.lock().unwrap().push(RecordedRequest {
                    method,
                    target: target.clone(),
                });
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

fn song(id: &str, name: &str, artist: &str, isrc: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "attributes": {
            "name": name,
            "artistName": artist,
            "albumName": "Album One",
            "durationInMillis": 200000,
            "isrc": isrc,
            "dateAdded": "2024-01-02T10:00:00Z"
        }
    })
}

// ============================================================
// Storefront
// ============================================================

#[test]
fn storefront_resolution_prefers_credentials_then_env_then_default() {
    assert_eq!(normalize_storefront(Some("ES")), Some("es".to_string()));
    assert_eq!(normalize_storefront(Some(" es ")), Some("es".to_string()));
    // Unusable values must never leak into a /catalog/{sf}/ URL.
    assert_eq!(normalize_storefront(None), None);
    assert_eq!(normalize_storefront(Some("")), None);
    assert_eq!(normalize_storefront(Some("usa")), None);
    assert_eq!(normalize_storefront(Some("../us")), None);
    assert_eq!(normalize_storefront(Some("u1")), None);

    assert_eq!(resolve_storefront(Some("mx")), "mx");
    // A garbage credential value never reaches the URL: it is replaced by the
    // env override when it is a valid storefront, else by the Apple default.
    let from_garbage = resolve_storefront(Some("not-a-storefront"));
    assert_ne!(from_garbage, "not-a-storefront");
    let expected = std::env::var(APPLE_MUSIC_STOREFRONT_ENV)
        .ok()
        .filter(|v| normalize_storefront(Some(v)).is_some())
        .unwrap_or_else(|| "us".to_string());
    assert_eq!(from_garbage, expected);
    assert_eq!(resolve_storefront(None), expected);
}

#[tokio::test]
async fn catalog_requests_use_the_configured_storefront() {
    let (mock_url, recorded) =
        spawn_mock_server(
            |_method, target| match target.split('?').next().unwrap_or("") {
                "/catalog/es/songs" => (200, r#"{"data":[]}"#.to_string()),
                "/catalog/es/search" => (
                    200,
                    r#"{"results":{"songs":{"data":[],"next":null}}}"#.to_string(),
                ),
                other => (404, format!(r#"{{"error":"unexpected path {}"}}"#, other)),
            },
        )
        .await;

    let creds = serde_json::json!({ "storefront": "es" });
    let client = AppleMusicClient::from_credentials("dev".into(), "user".into(), &creds)
        .with_base_url(mock_url);
    assert_eq!(client.storefront(), "es");

    client
        .search_by_isrc("ESAAA1234567")
        .await
        .expect("ISRC lookup must reach the mock");
    client
        .search_track("album one", 5)
        .await
        .expect("search must reach the mock");

    let targets: Vec<String> = recorded
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.target.clone())
        .collect();

    assert!(
        targets
            .iter()
            .any(|t| t.starts_with("/catalog/es/songs?filter[isrc]=")),
        "ISRC filter must hit the configured storefront, got {:?}",
        targets
    );
    assert!(
        targets
            .iter()
            .any(|t| t.starts_with("/catalog/es/search?term=")),
        "search must hit the configured storefront, got {:?}",
        targets
    );
    assert!(
        !targets.iter().any(|t| t.contains("/catalog/us/")),
        "no request may fall back to the hardcoded us storefront: {:?}",
        targets
    );
}

#[tokio::test]
async fn isrc_filter_failure_falls_back_to_search_on_the_same_storefront() {
    let (mock_url, recorded) = spawn_mock_server(|_method, target| {
        if target.contains("/catalog/gb/songs?filter") {
            (404, r#"{"error":"filter unsupported"}"#.to_string())
        } else if target.contains("/catalog/gb/search") {
            (
                200,
                serde_json::json!({
                    "results": { "songs": { "data": [song("gb1", "Fallback Song", "Artist Beta", "GBGBB1234567")], "next": null } }
                })
                .to_string(),
            )
        } else {
            (404, r#"{"error":"unexpected"}"#.to_string())
        }
    })
    .await;

    let client = AppleMusicClient::new("dev".into(), "user".into())
        .with_storefront("gb")
        .with_base_url(mock_url);

    let found = client
        .search_by_isrc("GBGBB1234567")
        .await
        .expect("fallback must not error")
        .expect("fallback must find the ISRC");
    assert_eq!(found.track_id, "gb1");

    let targets: Vec<String> = recorded
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.target.clone())
        .collect();
    assert!(
        targets.iter().any(|t| t.contains("/catalog/gb/search")),
        "fallback search must stay on the configured storefront, got {:?}",
        targets
    );
}

// ============================================================
// Pagination with `next`
// ============================================================

#[tokio::test]
async fn search_track_follows_the_next_cursor_up_to_the_requested_limit() {
    let (mock_url, recorded) = spawn_mock_server(|_method, target| {
        if target.contains("offset=2") {
            (
                200,
                serde_json::json!({
                    "results": { "songs": { "data": [song("t3", "Third", "Artist Gamma", "ISRC0003")], "next": null } }
                })
                .to_string(),
            )
        } else {
            (
                200,
                serde_json::json!({
                    "results": {
                        "songs": {
                            "data": [song("t1", "First", "Artist Alpha", "ISRC0001"),
                                     song("t2", "Second", "Artist Beta", "ISRC0002")],
                            "next": "/v1/catalog/us/search?term=x&offset=2"
                        }
                    }
                })
                .to_string(),
            )
        }
    })
    .await;

    let client = AppleMusicClient::new("dev".into(), "user".into()).with_base_url(mock_url);
    let results = client
        .search_track("query", 3)
        .await
        .expect("search must succeed");

    assert_eq!(
        results
            .iter()
            .map(|r| r.track_id.as_str())
            .collect::<Vec<_>>(),
        vec!["t1", "t2", "t3"],
        "the third result can only come from following `next`"
    );
    assert_eq!(
        recorded.lock().unwrap().len(),
        2,
        "expected one request per page"
    );
}

#[tokio::test]
async fn search_track_stops_when_the_cursor_repeats() {
    let (mock_url, recorded) = spawn_mock_server(|_method, _target| {
        (
            200,
            serde_json::json!({
                "results": {
                    "songs": {
                        "data": [song("t1", "First", "Artist Alpha", "ISRC0001")],
                        // A catalog that keeps pointing at itself must not hang.
                        "next": "/v1/catalog/us/search?term=x&offset=1"
                    }
                }
            })
            .to_string(),
        )
    })
    .await;

    let client = AppleMusicClient::new("dev".into(), "user".into()).with_base_url(mock_url);
    let results = client
        .search_track("query", 50)
        .await
        .expect("search must succeed");

    assert_eq!(
        results.len(),
        2,
        "only the first page plus the single follow-up may be fetched"
    );
    assert_eq!(
        recorded.lock().unwrap().len(),
        2,
        "the repeated cursor must break the walk"
    );
}

// ============================================================
// Albums and playlists in the import
// ============================================================

#[tokio::test]
async fn import_albums_expands_a_truncated_relationship_and_stores_album_identity() {
    let _guard = DB_LOCK.lock().unwrap();
    let pool = setup_test_db().await;
    let (_service_id, account_id) = setup_apple_account(&pool).await;

    let (mock_url, recorded) = spawn_mock_server(|_method, target| {
        if target.contains("me/library/albums?offset=0") {
            (
                200,
                serde_json::json!({
                    "data": [{
                        "id": "album_1",
                        "attributes": {
                            "name": "Album One",
                            "artistName": "Artist Alpha",
                            "trackCount": 2,
                            "dateAdded": "2024-01-01T00:00:00Z",
                            "releaseDate": "2019-05-04",
                            "upc": "00602577551234"
                        },
                        // Truncated page: the relationship carries a cursor.
                        "relationships": {
                            "tracks": {
                                "data": [song("a1", "Album Track One", "Artist Alpha", "ISRCAAA0001")],
                                "next": "/v1/me/library/albums/album_1/tracks?offset=1&limit=100"
                            }
                        }
                    }],
                    "next": null,
                    "meta": { "total": 1 }
                })
                .to_string(),
            )
        } else if target.contains("me/library/albums/album_1/tracks?offset=0") {
            (
                200,
                serde_json::json!({
                    "data": [song("a1", "Album Track One", "Artist Alpha", "ISRCAAA0001")],
                    "next": "/v1/me/library/albums/album_1/tracks?offset=1&limit=100"
                })
                .to_string(),
            )
        } else if target.contains("me/library/albums/album_1/tracks?offset=1") {
            (
                200,
                serde_json::json!({
                    "data": [song("a2", "Album Track Two", "Artist Alpha", "ISRCAAA0002")],
                    "next": null
                })
                .to_string(),
            )
        } else {
            (404, r#"{"error":"unexpected"}"#.to_string())
        }
    })
    .await;

    let client = AppleMusicClient::new("dev".into(), "user".into()).with_base_url(mock_url);
    let result = client
        .import_albums(&pool, account_id)
        .await
        .expect("import_albums must succeed");

    assert_eq!(result.entities, 1);
    assert_eq!(result.imported, 2, "both album pages must land");

    let targets: Vec<String> = recorded
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.target.clone())
        .collect();
    assert!(
        targets
            .iter()
            .any(|t| t.contains("me/library/albums/album_1/tracks?offset=1")),
        "the truncated relationship must be expanded through the tracks endpoint: {:?}",
        targets
    );

    let album: (String, Option<String>, Option<i64>) =
        sqlx::query_as("SELECT title, upc, total_tracks FROM albums LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("album row");
    assert_eq!(album.0, "Album One");
    assert_eq!(album.1.as_deref(), Some("00602577551234"));
    assert_eq!(album.2, Some(2));

    let releases: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM albums WHERE title = 'Album One' AND release_date = '2019-05-04'",
    )
    .fetch_one(&pool)
    .await
    .expect("release_date query");
    assert_eq!(releases, 1);

    let library_tracks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("library entries");
    assert_eq!(library_tracks, 2);
}

#[tokio::test]
async fn import_playlists_paginates_tracks_and_registers_the_remote_identity() {
    let _guard = DB_LOCK.lock().unwrap();
    let pool = setup_test_db().await;
    let (service_id, account_id) = setup_apple_account(&pool).await;

    let (mock_url, recorded) = spawn_mock_server(|_method, target| {
        if target.contains("me/library/playlists?offset=0") {
            (
                200,
                serde_json::json!({
                    "data": [{
                        "id": "pl_1",
                        "attributes": {
                            "name": "Road Trip",
                            "description": { "standard": "Long drives" },
                            "dateAdded": "2024-02-03T09:00:00Z"
                        }
                    }],
                    "next": null,
                    "meta": { "total": 1 }
                })
                .to_string(),
            )
        } else if target.contains("me/library/playlists/pl_1/tracks?offset=0") {
            (
                200,
                serde_json::json!({
                    "data": [song("p1", "Playlist Track One", "Artist Alpha", "ISRCPPP0001")],
                    "next": "/v1/me/library/playlists/pl_1/tracks?offset=1&limit=100"
                })
                .to_string(),
            )
        } else if target.contains("me/library/playlists/pl_1/tracks?offset=1") {
            (
                200,
                serde_json::json!({
                    "data": [song("p2", "Playlist Track Two", "Artist Beta", "ISRCPPP0002")],
                    "next": null
                })
                .to_string(),
            )
        } else {
            (404, r#"{"error":"unexpected"}"#.to_string())
        }
    })
    .await;

    let client = AppleMusicClient::new("dev".into(), "user".into()).with_base_url(mock_url);
    let result = client
        .import_playlists(&pool, account_id)
        .await
        .expect("import_playlists must succeed");

    assert_eq!(result.entities, 1);
    assert_eq!(result.imported, 2, "both playlist pages must land");

    let targets: Vec<String> = recorded
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.target.clone())
        .collect();
    assert!(
        targets
            .iter()
            .any(|t| t.contains("me/library/playlists/pl_1/tracks?offset=1")),
        "a playlist longer than one page must be fully expanded: {:?}",
        targets
    );

    let (playlist_name, track_count): (String, i64) = sqlx::query_as(
        "SELECT name, track_count FROM playlists WHERE service_playlist_id = 'pl_1'",
    )
    .fetch_one(&pool)
    .await
    .expect("playlist row");
    assert_eq!(playlist_name, "Road Trip");
    assert_eq!(track_count, 2, "track_count must reflect the expansion");

    let source_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM playlist_sources WHERE service_playlist_id = 'pl_1' AND service_id = ?",
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .expect("playlist_sources query");
    assert_eq!(
        source_rows, 1,
        "the playlist must be registered in playlist_sources like every other service"
    );

    let positions: Vec<i64> = sqlx::query_scalar(
        "SELECT position FROM playlist_tracks pt
         JOIN playlists p ON p.id = pt.playlist_id
         WHERE p.service_playlist_id = 'pl_1' ORDER BY position ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("playlist_tracks query");
    assert_eq!(positions, vec![0, 1]);
}

// ============================================================
// Migration destination (transfer step)
// ============================================================

#[tokio::test]
async fn add_to_favorites_puts_the_track_in_the_library_and_skips_duplicates() {
    let (mock_url, recorded) = spawn_mock_server(|method, target| {
        if target.contains("me/library/songs?ids") {
            if target.contains("ids[i]=already_there") {
                (200, r#"{"data":[{"id":"already_there","attributes":{"name":"In Library","artistName":"Artist Alpha"}}]}"#.to_string())
            } else {
                (200, r#"{"data":[]}"#.to_string())
            }
        } else if target.contains("me/library?") {
            assert_eq!(method, "POST", "the library insert must be a POST");
            (200, "{}".to_string())
        } else {
            (404, r#"{"error":"unexpected"}"#.to_string())
        }
    })
    .await;

    let client = AppleMusicClient::new("dev".into(), "user".into()).with_base_url(mock_url);

    client
        .add_to_favorites("already_there")
        .await
        .expect("an existing library entry must not error");
    client
        .add_to_favorites("new_track")
        .await
        .expect("a new track must be added to the library");

    let requests = recorded.lock().unwrap().clone();
    let posts: Vec<&RecordedRequest> = requests.iter().filter(|r| r.method == "POST").collect();
    assert_eq!(
        posts.len(),
        1,
        "only the missing track may be inserted: {:?}",
        requests
    );
    assert!(
        posts[0].target.contains("ids[songs]=new_track"),
        "unexpected insert target {}",
        posts[0].target
    );
}

#[tokio::test]
async fn add_to_favorites_rejects_an_empty_track_id_without_calling_the_api() {
    let (mock_url, recorded) =
        spawn_mock_server(|_method, _target| (200, r#"{"data":[]}"#.to_string())).await;

    let client = AppleMusicClient::new("dev".into(), "user".into()).with_base_url(mock_url);
    let err = client
        .add_to_favorites("   ")
        .await
        .expect_err("an empty track id must fail");

    assert!(err.contains("empty"), "unexpected error: {}", err);
    assert!(
        recorded.lock().unwrap().is_empty(),
        "no request may be issued for an empty track id"
    );
}

#[tokio::test]
async fn add_to_favorites_surfaces_api_errors() {
    let (mock_url, _recorded) =
        spawn_mock_server(
            |_method, target| match target.split('?').next().unwrap_or("") {
                "/me/library/songs" => (200, r#"{"data":[]}"#.to_string()),
                "/me/library" => (401, r#"{"errors":[{"status":"401"}]}"#.to_string()),
                _ => (404, r#"{"error":"unexpected"}"#.to_string()),
            },
        )
        .await;

    let client = AppleMusicClient::new("dev".into(), "user".into()).with_base_url(mock_url);
    let err = client
        .add_to_favorites("track_1")
        .await
        .expect_err("a rejected insert must fail the transfer");

    assert!(
        err.contains("add to library failed"),
        "unexpected error: {}",
        err
    );
}
