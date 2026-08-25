//! S187 regression tests — "Tidal import truncado" (integration level).
//!
//! Owner symptom: favorites stopped at 91 and every playlist imported only its
//! first ~100 tracks. Root causes fixed in sprint S187:
//!   1. Pagination treated `items.len() < limit` as end-of-data. Tidal's real
//!      page shape is `{limit, offset, totalNumberOfItems, items}` with NO
//!      Spotify-style `next`, and short-but-non-empty mid-list pages happen.
//!   2. Playlist-tracks expansion fetched a single page (offset always 0).
//!   3. Transient page errors silently aborted the remaining import.
//!
//! These integration tests exercise the PUBLIC client surface
//! (`TidalClient::get_favorites_with_retry` + `should_continue_tidal_pagination`)
//! against a local mock HTTP server. The full legacy-flow E2E variants
//! (playlist of 250 tracks, flaky page recovery, auth propagation) live in
//! `src/services/tidal.rs` inside `mod s187_tests`.

use std::sync::{Arc, Mutex};
use syncify_tauri_lib::services::tidal::{TidalClient, TidalPaginated, should_continue_tidal_pagination};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Responder = Arc<dyn Fn(&str) -> (u16, String) + Send + Sync>;

/// Spawn a local mock Tidal server; returns (base_url, request log).
async fn spawn_mock_tidal(responder: Responder) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::<String>::new()));
    let reqs = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { break };
            let responder = responder.clone();
            let reqs = reqs.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);
                let request_line = raw.lines().next().unwrap_or("");
                let target = request_line.split_whitespace().nth(1).unwrap_or("").to_string();
                reqs.lock().unwrap().push(target.clone());
                let (status, body) = responder(&target);
                let reason = match status {
                    200 => "OK",
                    401 => "Unauthorized",
                    429 => "Too Many Requests",
                    _ => "Error",
                };
                let resp = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    reason,
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    (format!("http://{}", addr), requests)
}

fn query_param(query: &str, key: &str) -> Option<i32> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        if k == key { v.parse::<i32>().ok() } else { None }
    })
}

fn split_target(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    }
}

fn track_json(id: i64) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "title": format!("S187 Track {}", id),
        "duration": 200,
        "isrc": format!("ISRC{:08}", id),
        "audioQuality": "LOSSLESS",
        "artist": {"id": id % 7 + 1, "name": format!("Artist {}", id % 7 + 1)},
        "album": {
            "id": id % 13 + 1,
            "title": format!("Album {}", id % 13 + 1),
            "releaseDate": "2020-01-01",
            "numberOfTracks": 12,
            "cover": null
        },
        "trackNumber": 1,
        "volumeNumber": 1
    })
}

fn favorites_body(items: Vec<serde_json::Value>, total: i64) -> String {
    serde_json::json!({ "totalNumberOfItems": total, "items": items }).to_string()
}

fn mock_client(base: String) -> TidalClient {
    TidalClient::new("test-token".to_string())
        .with_user("1".to_string(), "US".to_string())
        .with_base_url(base)
}

/// The production pagination contract driven against the REAL client:
/// 341 favorites arrive as 50-item pages plus a short final page of 41
/// (341 = 6×50 + 41). The walker must advance the offset by the REAL page
/// length, never treat the short page as end-of-data, and stop exactly at the
/// provider total — collecting all 341 items. This is the bug class behind the
/// owner's "stops at 91".
#[tokio::test]
async fn test_s187_favorites_341_short_final_page_walks_every_offset() {
    const TOTAL: i64 = 341;
    let responder: Responder = Arc::new(move |target: &str| {
        let (_path, query) = split_target(target);
        let offset = query_param(query, "offset").unwrap_or(0);
        let limit = query_param(query, "limit").unwrap_or(50);
        let off = offset.max(0) as i64;
        let end = (off + limit.max(1) as i64).min(TOTAL);
        let items: Vec<serde_json::Value> = if off >= TOTAL {
            Vec::new()
        } else {
            (off..end).map(|i| serde_json::json!({ "item": track_json(i + 1) })).collect()
        };
        (200, favorites_body(items, TOTAL))
    });
    let (base, requests) = spawn_mock_tidal(responder).await;
    let client = mock_client(base);

    // Exact production loop skeleton (commands/service.rs Phase 1, post-S187):
    // fetch with retry, ingest, advance by REAL len, stop only per provider total.
    let limit: i32 = 50;
    let mut offset: i32 = 0;
    let mut collected: Vec<TidalPaginated> = Vec::new();
    let mut seen: u64 = 0;
    loop {
        let page = client.get_favorites_with_retry(offset, limit).await.expect("page fetch");
        if page.items.is_empty() {
            break;
        }
        seen += page.items.len() as u64;
        collected.push(page);
        offset += collected.last().unwrap().items.len() as i32;
        if !should_continue_tidal_pagination(collected.last().unwrap().items.len(), seen, collected.last().unwrap().total as i64) {
            break;
        }
    }

    assert_eq!(seen, 341, "all provider favorites must be walked");
    let offsets: Vec<i32> = requests
        .lock()
        .unwrap()
        .iter()
        .filter_map(|t| query_param(split_target(t).1, "offset"))
        .collect();
    assert_eq!(offsets, vec![0, 50, 100, 150, 200, 250, 300], "offsets must advance by the real page length");
    assert_eq!(collected.last().unwrap().items.len(), 41, "final page is short but NOT end-of-data until total is met");
}

/// Shape tolerance + transient recovery at client level:
///  - the endpoint may answer with a Spotify-style `"next": null` field or
///    extra unknown keys — deserialization must tolerate both shapes;
///  - a single 429 on the second page must be retried once (2 attempts) and
///    the walk must still complete instead of silently truncating.
#[tokio::test]
async fn test_s187_page_shape_tolerance_and_429_retried_then_complete() {
    const TOTAL: i64 = 120;
    let fail_once = Arc::new(Mutex::new(true));
    let fail_once_c = fail_once.clone();
    let responder: Responder = Arc::new(move |target: &str| {
        let (_path, query) = split_target(target);
        let offset = query_param(query, "offset").unwrap_or(0);
        let limit = query_param(query, "limit").unwrap_or(50);
        if offset >= 50 {
            let mut once = fail_once_c.lock().unwrap();
            if *once {
                *once = false;
                return (429, "{\"error\":\"slow down\"}".to_string());
            }
        }
        let off = offset.max(0) as i64;
        let end = (off + limit.max(1) as i64).min(TOTAL);
        let items: Vec<serde_json::Value> =
            (off..end).map(|i| serde_json::json!({ "item": track_json(i + 1) })).collect();
        // Page 1 carries a Spotify-style "next": null and extra keys;
        // page 2+ uses the plain Tidal shape. Both must deserialize.
        let mut body = serde_json::json!({
            "limit": limit,
            "offset": offset,
            "totalNumberOfItems": TOTAL,
            "items": items
        });
        if offset == 0 {
            body["next"] = serde_json::Value::Null;
            body["cursors"] = serde_json::json!({"after": null});
        }
        (200, body.to_string())
    });
    let (base, requests) = spawn_mock_tidal(responder).await;
    let client = mock_client(base);

    let limit: i32 = 50;
    let mut offset: i32 = 0;
    let mut seen: u64 = 0;
    let mut pages: usize = 0;
    loop {
        let page = client.get_favorites_with_retry(offset, limit).await.expect("page fetch must survive the 429");
        if page.items.is_empty() {
            break;
        }
        seen += page.items.len() as u64;
        offset += page.items.len() as i32;
        pages += 1;
        if !should_continue_tidal_pagination(page.items.len(), seen, page.total as i64) {
            break;
        }
    }

    assert_eq!(seen, 120, "walk completes despite injected 429 and mixed page shapes");
    assert_eq!(*fail_once.lock().unwrap(), false, "the injected failure was consumed");
    assert_eq!(pages, 3, "120 items at limit=50 => three successful pages");
    let fav_requests = requests.lock().unwrap().iter().filter(|t| t.contains("/favorites/tracks")).count();
    assert_eq!(fav_requests, 4, "three pages + exactly one retried attempt (2 attempts max)");
}
