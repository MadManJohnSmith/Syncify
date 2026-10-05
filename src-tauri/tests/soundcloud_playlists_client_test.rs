//! R3: el cliente de SoundCloud tiene que poder leer las playlists de la cuenta.
//!
//! El brazo del motor unificado afirmaba que la API pública de SoundCloud sólo
//! expone los likes, y la UI usaba ese supuesto para desactivar el botón de
//! sincronizar. El endpoint `/me/library/playlists_without_albums` sí existe —
//! el servicio Python ya lo recorre para la misma cuenta
//! (`scripts/services/soundcloud_service.py:454`) —, así que el cliente Rust
//! debe poder paginarlo igual que los likes.
//!
//! HTTP contra un servidor en proceso: lo que se comprueba es la URL que el
//! cliente construye, la paginación por `next_href` y el orden de las pistas.

use std::sync::Arc;
use syncify_tauri_lib::services::SoundCloudClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type Responder = Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

async fn serve_one(mut socket: TcpStream, responder: Responder) {
    let mut buf = vec![0u8; 32768];
    let n = socket.read(&mut buf).await.unwrap_or(0);
    let raw = String::from_utf8_lossy(&buf[..n]);
    let mut parts = raw.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    let (status, body) = responder(&method, &target);
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        if status == 200 { "OK" } else { "Error" },
        body.len(),
        body
    );
    let _ = socket.write_all(resp.as_bytes()).await;
    let _ = socket.flush().await;
}

async fn spawn_mock(responder: Responder) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let responder = responder.clone();
            tokio::spawn(serve_one(socket, responder));
        }
    });
    format!("http://{}", addr)
}

fn playlist(id: i64, title: &str, tracks: i64) -> serde_json::Value {
    serde_json::json!({
        "playlist": {
            "id": id,
            "title": title,
            "description": format!("{} description", title),
            "track_count": tracks,
            "duration": tracks * 200_000,
            "user": {"id": 42, "username": "curator"}
        }
    })
}

#[tokio::test]
async fn playlists_are_read_from_the_library_endpoint() {
    let base = spawn_mock(Arc::new(|_method, target| {
        assert!(
            target.starts_with("/me/library/playlists_without_albums"),
            "unexpected path: {}",
            target
        );
        (
            200,
            serde_json::json!({
                "collection": [playlist(1, "Favorites", 12), playlist(2, "Radio", 3)],
                "next_href": serde_json::Value::Null
            })
            .to_string(),
        )
    }))
    .await;

    let client = SoundCloudClient::new("token".to_string())
        .with_user_id(42)
        .with_api_base(&base);

    let page = client.get_playlists(None).await.expect("playlists");

    assert_eq!(page.collection.len(), 2);
    assert_eq!(
        page.collection[0]
            .playlist
            .as_ref()
            .expect("primera playlist")
            .display_name(),
        "Favorites"
    );
    assert_eq!(page.collection[1].playlist.as_ref().unwrap().track_count, 3);
}

#[tokio::test]
async fn playlists_follow_the_next_href_cursor() {
    let base = spawn_mock(Arc::new(|_method, target| {
        if target.contains("cursor=2") {
            (
                200,
                serde_json::json!({
                    "collection": [playlist(2, "Radio", 3)],
                    "next_href": serde_json::Value::Null
                })
                .to_string(),
            )
        } else {
            (
                200,
                serde_json::json!({
                    "collection": [playlist(1, "Favorites", 12)],
                    "next_href": "/me/library/playlists_without_albums?limit=50&cursor=2"
                })
                .to_string(),
            )
        }
    }))
    .await;

    let client = SoundCloudClient::new("token".to_string())
        .with_user_id(42)
        .with_api_base(&base);

    let first = client.get_playlists(None).await.expect("primera pagina");
    assert_eq!(first.collection.len(), 1);

    let next = first.next_href.expect("debe haber siguiente pagina");
    let second = client
        .get_playlists(Some(next.as_str()))
        .await
        .expect("segunda pagina");

    assert_eq!(second.collection.len(), 1);
    assert!(
        second.next_href.is_none(),
        "la ultima pagina no debe anunciar otra"
    );
}

#[tokio::test]
async fn playlist_tracks_keep_the_order_of_the_collection() {
    let base = spawn_mock(Arc::new(|_method, target| {
        assert!(
            target.starts_with("/playlists/77/tracks"),
            "unexpected path: {}",
            target
        );
        (
            200,
            serde_json::json!({
                "collection": [
                    {"created_at": "2024-01-01T00:00:00Z", "track": {
                        "id": 1, "title": "First", "duration": 1000,
                        "user": {"id": 9, "username": "uploader"}
                    }},
                    {"created_at": "2024-01-02T00:00:00Z", "track": {
                        "id": 2, "title": "Second", "duration": 2000,
                        "user": {"id": 9, "username": "uploader"}
                    }}
                ],
                "next_href": serde_json::Value::Null
            })
            .to_string(),
        )
    }))
    .await;

    let client = SoundCloudClient::new("token".to_string())
        .with_user_id(42)
        .with_api_base(&base);

    let page = client
        .get_playlist_tracks("77", None)
        .await
        .expect("pistas de la playlist");

    let titles: Vec<&str> = page
        .collection
        .iter()
        .filter_map(|like| like.track.as_ref())
        .map(|track| track.title.as_str())
        .collect();

    assert_eq!(titles, vec!["First", "Second"]);
}

#[tokio::test]
async fn unauthorized_playlists_surface_the_api_error_body() {
    let base = spawn_mock(Arc::new(|_method, _target| {
        (401, r#"{"errors":[{"error_code":"401"}]}"#.to_string())
    }))
    .await;

    let client = SoundCloudClient::new("expired".to_string())
        .with_user_id(42)
        .with_api_base(&base);

    let err = client
        .get_playlists(None)
        .await
        .expect_err("un 401 debe ser error, no una lista vacía");

    assert!(err.contains("401"), "{}", err);
}
