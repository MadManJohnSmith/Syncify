//! Issue #66 — adaptadores Spotify, Tidal y Qobuz.
//!
//! Fixtures grabadas (sin credenciales ni datos personales) servidas por un
//! servidor local: el adaptador real recorre todas las páginas, lee el total
//! una sola vez y expone la huella de cada playlist. También se fija el mapeo
//! de errores: 401/403 → `AuthExpired` y 429 → reintento tras `Retry-After`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use syncify_tauri_lib::services::{QobuzClient, SpotifyClient, TidalClient};
use syncify_tauri_lib::sync::collection::{collect_all_pages, collect_cursor_pages};
use syncify_tauri_lib::sync::qobuz_source::QobuzLibrarySource;
use syncify_tauri_lib::sync::source::{LibrarySource, SourceErrorKind};
use syncify_tauri_lib::sync::spotify_source::SpotifyLibrarySource;
use syncify_tauri_lib::sync::tidal_source::TidalLibrarySource;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ============================================================
// Servidor local de fixtures
// ============================================================

struct MockResponse {
    status: u16,
    body: String,
    headers: Vec<(&'static str, String)>,
}

impl MockResponse {
    fn ok(body: String) -> Self {
        Self {
            status: 200,
            body,
            headers: Vec::new(),
        }
    }

    fn status(status: u16, body: &str) -> Self {
        Self {
            status,
            body: body.to_string(),
            headers: Vec::new(),
        }
    }

    fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }
}

type Responder = Arc<dyn Fn(&str, &str) -> MockResponse + Send + Sync>;
type Seen = Arc<Mutex<Vec<(String, String)>>>;

struct MockServer {
    base: String,
    seen: Seen,
}

async fn serve_one(mut socket: TcpStream, responder: Responder, seen: Seen) {
    let mut buf = vec![0u8; 32768];
    let n = socket.read(&mut buf).await.unwrap_or(0);
    let raw = String::from_utf8_lossy(&buf[..n]).to_string();
    let request_line = raw.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    seen.lock()
        .expect("mock recorder")
        .push((method.clone(), target.clone()));

    let response = responder(&method, &target);
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        if (200..300).contains(&response.status) {
            "OK"
        } else {
            "Error"
        },
        response.body.len()
    );
    for (name, value) in &response.headers {
        head.push_str(&format!("{}: {}\r\n", name, value));
    }
    head.push_str("\r\n");
    head.push_str(&response.body);

    let _ = socket.write_all(head.as_bytes()).await;
    let _ = socket.flush().await;
}

async fn spawn_mock(responder: Responder) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_accept = seen.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let responder = responder.clone();
            let seen = seen_for_accept.clone();
            tokio::spawn(serve_one(socket, responder, seen));
        }
    });
    MockServer {
        base: format!("http://{}", addr),
        seen,
    }
}

impl MockServer {
    fn requests(&self) -> Vec<(String, String)> {
        self.seen.lock().expect("mock recorder").clone()
    }

    fn targets(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|(_, target)| target)
            .collect()
    }
}

// ============================================================
// Fixtures
// ============================================================

fn fixture(service: &str) -> Value {
    let raw = match service {
        "spotify" => include_str!("fixtures/sync_adapters/spotify.json"),
        "tidal" => include_str!("fixtures/sync_adapters/tidal.json"),
        "qobuz" => include_str!("fixtures/sync_adapters/qobuz.json"),
        other => panic!("fixture desconocida: {}", other),
    };
    serde_json::from_str(raw).expect("fixture JSON válida")
}

fn query_param<'a>(target: &'a str, name: &str) -> Option<&'a str> {
    target
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or("")
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{}=", name)))
}

/// Responde la página de la colección cuyo inicio acumulado coincide con el
/// `offset` pedido. El orden de las páginas es el de la fixture.
fn offset_page_responder(service: &str, collection: &str) -> Responder {
    let fixture = fixture(service);
    let pages: Vec<Value> = fixture[collection]
        .as_array()
        .expect("colección con páginas")
        .clone();
    let starts: Vec<usize> = {
        let mut starts = Vec::with_capacity(pages.len());
        let mut start = 0usize;
        for page in &pages {
            starts.push(start);
            start += page_items_count(service, collection, page);
        }
        starts
    };
    Arc::new(move |_method, target| {
        let offset: usize = query_param(target, "offset")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let index = starts
            .iter()
            .position(|start| *start == offset)
            .unwrap_or(pages.len().saturating_sub(1));
        MockResponse::ok(pages[index].to_string())
    })
}

fn page_items_count(service: &str, collection: &str, page: &Value) -> usize {
    let items = match (service, collection) {
        ("qobuz", "albums") => &page["albums"]["items"],
        ("qobuz", "artists") | ("spotify", "artists") => &page["artists"]["items"],
        ("qobuz", "playlists") => &page["playlists"]["items"],
        ("qobuz", "favorites") => &page["tracks"]["items"],
        _ => &page["items"],
    };
    items.as_array().map(Vec::len).unwrap_or(0)
}

fn spotify_artists_responder() -> Responder {
    let fixture = fixture("spotify");
    let pages = fixture["artists"].as_array().expect("artistas").clone();
    Arc::new(move |_method, target| {
        let index = if query_param(target, "after") == Some("cursor-page-1") {
            1
        } else {
            0
        };
        MockResponse::ok(pages[index].to_string())
    })
}

// ============================================================
// Clientes contra el mock
// ============================================================

fn spotify_client(server: &MockServer) -> SpotifyClient {
    SpotifyClient::new("fixture-token".to_string(), None, i64::MAX)
        .with_api_base(format!("{}/v1", server.base))
}

fn tidal_client(server: &MockServer) -> TidalClient {
    TidalClient::new("fixture-token".to_string())
        .with_user("42".to_string(), "MX".to_string())
        .with_base_url(format!("{}/v1", server.base))
}

fn qobuz_client(server: &MockServer) -> QobuzClient {
    QobuzClient::new_with_token(
        "fixture-app".to_string(),
        "fixture-secret".to_string(),
        "fixture-token".to_string(),
    )
    .with_base_url(format!("{}/0.2", server.base))
}

// ============================================================
// Paginación completa, orden y total
// ============================================================

#[tokio::test]
async fn spotify_albums_recorre_todas_las_paginas_y_asigna_el_total_una_vez() {
    let server = spawn_mock(offset_page_responder("spotify", "albums")).await;
    let client = spotify_client(&server);
    let source = SpotifyLibrarySource::new(&client);

    let collected = collect_all_pages(50, |offset| source.fetch_albums_page(offset))
        .await
        .expect("recorrido de álbumes");

    let ids: Vec<&str> = collected
        .items
        .iter()
        .map(|item| item["album"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["sp-alb-1", "sp-alb-2", "sp-alb-3"]);
    assert_eq!(collected.pages, 2, "deben pedirse las dos páginas");
    assert_eq!(
        collected.reported_total,
        Some(3),
        "el total se asigna una vez; el proveedor lo repite en cada página"
    );
    let targets = server.targets();
    assert_eq!(targets.len(), 2);
    assert!(targets[0].contains("offset=0"), "{}", targets[0]);
    assert!(targets[1].contains("offset=2"), "{}", targets[1]);
}

#[tokio::test]
async fn tidal_artists_recorre_todas_las_paginas_y_asigna_el_total_una_vez() {
    let server = spawn_mock(offset_page_responder("tidal", "artists")).await;
    let client = tidal_client(&server);
    let source = TidalLibrarySource::new(&client);

    let collected = collect_all_pages(50, |offset| source.fetch_artists_page(offset))
        .await
        .expect("recorrido de artistas");

    let names: Vec<&str> = collected
        .items
        .iter()
        .map(|item| item["item"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["Tidal Artist 1", "Tidal Artist 2", "Tidal Artist 3"]
    );
    assert_eq!(collected.pages, 2);
    assert_eq!(collected.reported_total, Some(3));
    assert_eq!(server.targets().len(), 2);
}

#[tokio::test]
async fn qobuz_albums_recorre_todas_las_paginas_y_asigna_el_total_una_vez() {
    let server = spawn_mock(offset_page_responder("qobuz", "albums")).await;
    let client = qobuz_client(&server);
    let source = QobuzLibrarySource::new(&client);

    let collected = collect_all_pages(50, |offset| source.fetch_albums_page(offset))
        .await
        .expect("recorrido de álbumes");

    let ids: Vec<&str> = collected
        .items
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["q-alb-1", "q-alb-2", "q-alb-3"]);
    assert_eq!(collected.pages, 2);
    assert_eq!(collected.reported_total, Some(3));
    assert_eq!(server.targets().len(), 2);
}

#[tokio::test]
async fn spotify_artists_recorre_el_cursor_hasta_la_ultima_pagina() {
    let server = spawn_mock(spotify_artists_responder()).await;
    let client = spotify_client(&server);
    let source = SpotifyLibrarySource::new(&client);

    let collected = collect_cursor_pages(50, |after| {
        let source = &source;
        async move { source.fetch_artists_page(after.as_deref()).await }
    })
    .await
    .expect("recorrido de artistas");

    let ids: Vec<&str> = collected
        .items
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["sp-art-1", "sp-art-2", "sp-art-3"]);
    assert_eq!(collected.reported_total, Some(3));
    let targets = server.targets();
    assert_eq!(targets.len(), 2);
    assert!(!targets[0].contains("after="), "{}", targets[0]);
    assert!(targets[1].contains("after=cursor-page-1"), "{}", targets[1]);
}

// ============================================================
// Huellas
// ============================================================

#[tokio::test]
async fn spotify_playlists_exponen_el_snapshot_id_como_huella() {
    let server = spawn_mock(offset_page_responder("spotify", "playlists")).await;
    let client = spotify_client(&server);
    let source = SpotifyLibrarySource::new(&client);

    let collected = collect_all_pages(50, |offset| source.fetch_playlists_page(offset))
        .await
        .expect("recorrido de playlists");

    assert_eq!(collected.reported_total, Some(3));
    assert_eq!(
        collected.fingerprints,
        vec![
            Some("snap-sp-1".to_string()),
            Some("snap-sp-2".to_string()),
            Some("snap-sp-3".to_string()),
        ]
    );
}

#[tokio::test]
async fn tidal_y_qobuz_playlists_usan_el_conteo_como_huella() {
    let tidal_server = spawn_mock(offset_page_responder("tidal", "playlists")).await;
    let tidal = tidal_client(&tidal_server);
    let tidal_source = TidalLibrarySource::new(&tidal);
    let tidal_collected = collect_all_pages(50, |offset| tidal_source.fetch_playlists_page(offset))
        .await
        .expect("playlists Tidal");
    assert_eq!(
        tidal_collected.fingerprints,
        vec![
            Some("1".to_string()),
            Some("2".to_string()),
            Some("3".to_string()),
        ]
    );

    let qobuz_server = spawn_mock(offset_page_responder("qobuz", "playlists")).await;
    let qobuz = qobuz_client(&qobuz_server);
    let qobuz_source = QobuzLibrarySource::new(&qobuz);
    let qobuz_collected = collect_all_pages(50, |offset| qobuz_source.fetch_playlists_page(offset))
        .await
        .expect("playlists Qobuz");
    assert_eq!(
        qobuz_collected.fingerprints,
        vec![
            Some("1".to_string()),
            Some("2".to_string()),
            Some("3".to_string()),
        ]
    );
}

// ============================================================
// Errores: 401/403 y 429
// ============================================================

#[tokio::test]
async fn spotify_401_se_mapea_a_credencial_invalida() {
    let server = spawn_mock(Arc::new(|_method, _target| {
        MockResponse::status(401, "{\"error\":{\"status\":401}}")
    }))
    .await;
    let client = spotify_client(&server);
    let source = SpotifyLibrarySource::new(&client);

    let error = source
        .fetch_favorites_page(0)
        .await
        .expect_err("401 debe fallar");
    assert_eq!(error.kind, SourceErrorKind::AuthExpired);
}

#[tokio::test]
async fn tidal_401_se_mapea_a_credencial_invalida() {
    let server = spawn_mock(Arc::new(|_method, _target| {
        MockResponse::status(401, "{\"status\":401}")
    }))
    .await;
    let client = tidal_client(&server);
    let source = TidalLibrarySource::new(&client);

    let error = source
        .fetch_favorites_page(0)
        .await
        .expect_err("401 debe fallar");
    assert_eq!(error.kind, SourceErrorKind::AuthExpired);
}

#[tokio::test]
async fn qobuz_401_se_mapea_a_credencial_invalida() {
    let server = spawn_mock(Arc::new(|_method, _target| {
        MockResponse::status(401, "{\"error\":\"unauthorized\"}")
    }))
    .await;
    let client = qobuz_client(&server);
    let source = QobuzLibrarySource::new(&client);

    let error = source
        .fetch_favorites_page(0)
        .await
        .expect_err("401 debe fallar");
    assert_eq!(error.kind, SourceErrorKind::AuthExpired);
}

fn rate_limited_then_page(service: &str, collection: &str) -> (Responder, Arc<AtomicUsize>) {
    let fixture = fixture(service);
    let page = fixture[collection][0].to_string();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_closure = hits.clone();
    let responder: Responder = Arc::new(move |_method, _target| {
        let hit = hits_for_closure.fetch_add(1, Ordering::SeqCst);
        if hit == 0 {
            MockResponse::status(429, "{\"error\":\"slow down\"}").with_header("Retry-After", "0")
        } else {
            MockResponse::ok(page.clone())
        }
    });
    (responder, hits)
}

#[tokio::test]
async fn spotify_429_respeta_retry_after_y_reintenta() {
    let (responder, hits) = rate_limited_then_page("spotify", "favorites");
    let server = spawn_mock(responder).await;
    let client = spotify_client(&server);
    let source = SpotifyLibrarySource::new(&client);

    let page = source
        .fetch_favorites_page(0)
        .await
        .expect("el 429 debe reintentarse");
    assert_eq!(page.items.len(), 2);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn tidal_429_respeta_retry_after_y_reintenta() {
    let (responder, hits) = rate_limited_then_page("tidal", "favorites");
    let server = spawn_mock(responder).await;
    let client = tidal_client(&server);
    let source = TidalLibrarySource::new(&client);

    let page = source
        .fetch_favorites_page(0)
        .await
        .expect("el 429 debe reintentarse");
    assert_eq!(page.items.len(), 2);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn qobuz_429_respeta_retry_after_y_reintenta() {
    let (responder, hits) = rate_limited_then_page("qobuz", "favorites");
    let server = spawn_mock(responder).await;
    let client = qobuz_client(&server);
    let source = QobuzLibrarySource::new(&client);

    let page = source
        .fetch_favorites_page(0)
        .await
        .expect("el 429 debe reintentarse");
    assert_eq!(page.items.len(), 2);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}
