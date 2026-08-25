//! S189-Fase-1 regression tests — Deezer unified-engine coverage.
//!
//! The Deezer arm of `perform_sync_service_with_emitter` previously only ran
//! favorite tracks through the legacy raw-dedupe importer and emitted empty
//! progress events for albums/artists/playlists/history. Fase-1 adds real
//! public-API phases (albums / artists / playlists / playlist-tracks) with
//! auth parity and the shared import_pagination policy.
//!
//! These tests cover the client layer against a local mock HTTP server
//! (S187 pattern): response parsing, error-payload detection, pagination
//! sequences driven by `import_pagination::next_offset`, and the init()
//! auth contract that the sync arm relies on to emit RequiresAuth.

use std::sync::{Arc, Mutex};
use syncify_tauri_lib::services::deezer::DeezerClient;
use syncify_tauri_lib::services::import_pagination;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Responder = Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

/// Spawn a local mock server; responder gets (method, path-with-query).
async fn spawn_mock(responder: Responder) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { break };
            let responder = responder.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 32768];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);
                let mut lines = raw.lines();
                let request_line = lines.next().unwrap_or("");
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let target = parts.next().unwrap_or("").to_string();
                let (status, body) = responder(&method, &target);
                let reason = if status == 200 { "OK" } else { "Error" };
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
    format!("http://{}", addr)
}

fn json_page(data: serde_json::Value, total: i64) -> String {
    format!("{{\"data\":{},\"total\":{}}}", data, total)
}

#[tokio::test]
async fn test_s189_user_albums_parse_and_total() {
    let base = spawn_mock(Arc::new(|_method, target| {
        assert!(target.starts_with("/user/42/albums"), "unexpected path: {}", target);
        (200, json_page(serde_json::json!([
            {"id": 123, "title": "Discovery", "nb_tracks": 14,
             "cover_medium": "https://e-cdns-images.dzcdn.net/cover/md.jpg",
             "artist": {"id": 27, "name": "Daft Punk"}}
        ]), 1))
    }))
    .await;

    let client = DeezerClient::new("arl-test".into()).with_public_api_base(base);
    let (albums, total) = client.get_user_albums_public("42", 0, 100).await.expect("albums");
    assert_eq!(total, 1);
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].id, "123", "numeric ids normalize to string identity");
    assert_eq!(albums[0].title, "Discovery");
    assert_eq!(albums[0].artist_name.as_deref(), Some("Daft Punk"));
}

#[tokio::test]
async fn test_s189_error_payload_surfaces_as_err() {
    let base = spawn_mock(Arc::new(|_method, _target| {
        (200, "{\"error\":{\"type\":\"Exception\",\"message\":\"Quota exceeded\"}}".to_string())
    }))
    .await;

    let client = DeezerClient::new("arl-test".into()).with_public_api_base(base);
    let err = client
        .get_user_playlists_public("42", 0, 100)
        .await
        .expect_err("error payload must not parse as an empty page");
    assert!(err.contains("Quota exceeded"), "payload message preserved: {}", err);
}

#[tokio::test]
async fn test_s189_playlist_tracks_pagination_sequence() {
    // Two pages of 2 items each over a declared total of 4; the loop decision
    // mirrors the service.rs playlist-expansion loop.
    let base = spawn_mock(Arc::new(|_method, target| {
        if target.contains("index=0") {
            (200, json_page(serde_json::json!([
                {"id": 1, "title": "T1", "duration": 200, "isrc": "ISRC1",
                 "artist": {"name": "A1"}, "album": {"title": "AL1"}},
                {"id": 2, "title": "T2", "duration": 201, "isrc": "ISRC2",
                 "artist": {"name": "A2"}, "album": {"title": "AL2"}}
            ]), 4))
        } else if target.contains("index=2") {
            (200, json_page(serde_json::json!([
                {"id": 3, "title": "T3", "duration": 202, "isrc": "ISRC3",
                 "artist": {"name": "A3"}},
                {"id": 4, "title": "T4", "duration": 203}
            ]), 4))
        } else {
            (400, "{}".to_string())
        }
    }))
    .await;

    let client = DeezerClient::new("arl-test".into()).with_public_api_base(base);
    let mut offset: i32 = 0;
    let mut seen: Vec<String> = Vec::new();
    loop {
        let (tracks, total) = client
            .get_playlist_tracks_public("999", offset, 100)
            .await
            .expect("page fetch");
        if tracks.is_empty() {
            break;
        }
        for t in &tracks {
            seen.push(t.title.clone());
        }
        match import_pagination::next_offset(
            offset,
            tracks.len() as i32,
            100,
            (total > 0).then_some(total),
        ) {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(seen, vec!["T1", "T2", "T3", "T4"], "all pages consumed in order");
}

#[tokio::test]
async fn test_s189_album_tracks_embedded_isrc() {
    let base = spawn_mock(Arc::new(|_method, target| {
        assert!(target.starts_with("/album/777"), "unexpected path: {}", target);
        (
            200,
            serde_json::json!({
                "id": 777,
                "title": "Around The World",
                "tracks": {"data": [
                    {"id": 900, "title": "La La La", "duration": 210,
                     "isrc": "DEEZERISRC1", "artist": {"name": "ATC"},
                     "album": {"title": "Around The World"}},
                    {"id": 901, "title": "No ISRC Here", "duration": 180,
                     "artist": {"name": "ATC"}}
                ]}
            })
            .to_string(),
        )
    }))
    .await;

    let client = DeezerClient::new("arl-test".into()).with_public_api_base(base);
    let tracks = client.get_album_tracks_public("777").await.expect("album tracks");
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].isrc.as_deref(), Some("DEEZERISRC1"));
    assert_eq!(tracks[1].isrc, None, "missing isrc stays optional");
}

#[tokio::test]
async fn test_s189_init_failure_is_explicit_auth_rejection() {
    // The sync arm treats ANY init() error as RequiresAuth + credential
    // invalidation; verify init() actually fails on a rejected ARL payload.
    let base = spawn_mock(Arc::new(|_method, target| {
        assert!(target.starts_with("/ajax/gw-light.php"), "unexpected path: {}", target);
        (200, "{\"results\":{\"checkForm\":null,\"USER\":{}}}".to_string())
    }))
    .await;

    let mut client = DeezerClient::new("bad-arl".into()).with_api_base(base);
    let err = client.init().await.expect_err("missing checkForm must fail init");
    assert!(err.to_lowercase().contains("token") || err.to_lowercase().contains("arl"),
        "error must point at the ARL/token: {}", err);
}
