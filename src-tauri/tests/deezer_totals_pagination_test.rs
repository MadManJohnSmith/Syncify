//! Issue #405 — adaptador Deezer: totales y paginación.
//!
//! Fixtures grabadas (sin credenciales ni datos personales) servidas por un
//! servidor local: el adaptador recorre todas las páginas, lee el total del
//! proveedor **una sola vez** aunque la API lo repita en cada página y avanza
//! por la longitud real de la página. También se fija el mapeo de errores de
//! T02 (401/403 → `AuthExpired`, 429 → reintento tras `Retry-After` y
//! `Retryable` si persiste, 404 → `Permanent`) y el perfil de ritmo
//! (50 peticiones/5 s).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use syncify_tauri_lib::services::deezer::DeezerClient;
use syncify_tauri_lib::services::rate_limiter::default_rate_limits;
use syncify_tauri_lib::sync::collection::collect_all_pages;
use syncify_tauri_lib::sync::deezer_source::{classify_deezer_error, DeezerLibrarySource};
use syncify_tauri_lib::sync::source::{LibrarySource, SourceErrorKind};
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

    /// Solo los GET de una colección, en orden: el POST de `init()` no cuenta
    /// para la paginación.
    fn get_targets_containing(&self, needle: &str) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|(method, target)| method == "GET" && target.contains(needle))
            .map(|(_, target)| target)
            .collect()
    }
}

fn query_param<'a>(target: &'a str, name: &str) -> Option<&'a str> {
    target
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or("")
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{}=", name)))
}

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/sync_adapters/deezer.json")).expect("fixture JSON")
}

/// Respuesta de `deezer.getUserData`: el adaptador necesita el id numérico que
/// resuelve `init()` antes de pedir cualquier colección.
const DEEZER_USER_DATA: &str =
    r#"{"results":{"checkForm":"fixture-token","USER":{"USER_ID":4242,"BLOG_NAME":"fixture"}}}"#;

/// Responde la página cuya posición acumulada coincide con el `index` pedido.
/// Las páginas de la fixture son cortas a propósito: si el recorrido avanzara
/// por el límite (100) en vez de por la longitud real, la segunda petición
/// pediría `index=100` y el mock no encontraría página.
fn page_responder(collection: &str, path: &str) -> Responder {
    let pages: Vec<Value> = fixture()[collection]
        .as_array()
        .expect("colección con páginas")
        .clone();
    let starts: Vec<usize> = {
        let mut starts = Vec::with_capacity(pages.len());
        let mut start = 0usize;
        for page in &pages {
            starts.push(start);
            start += page["data"].as_array().map(Vec::len).unwrap_or(0);
        }
        starts
    };
    let path = path.to_string();
    Arc::new(move |method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            return MockResponse::ok(DEEZER_USER_DATA.to_string());
        }
        assert!(
            target.starts_with(&path),
            "petición inesperada: {} {}",
            method,
            target
        );
        let index: usize = query_param(target, "index")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let position = starts
            .iter()
            .position(|start| *start == index)
            .unwrap_or_else(|| {
                panic!(
                    "el recorrido pidió index={index}, que no es el inicio de ninguna página \
                     de la fixture (avance por longitud real, no por límite)"
                )
            });
        MockResponse::ok(pages[position].to_string())
    })
}

/// Cliente Deezer apuntado al mock y ya inicializado (id 4242).
async fn deezer_client(server: &MockServer) -> DeezerClient {
    let mut client = DeezerClient::new("arl-fixture".to_string())
        .with_api_base(server.base.clone())
        .with_public_api_base(server.base.clone());
    client.init().await.expect("init contra el mock");
    client
}

// ============================================================
// Paginación completa y total asignado una vez
// ============================================================

#[tokio::test]
async fn deezer_favoritos_recorre_todas_las_paginas_y_asigna_el_total_una_vez() {
    let server = spawn_mock(page_responder("favorites", "/user/4242/tracks")).await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    // `FavoritesPage` no comparte tipo con `CollectionPage`; el recorrido usa
    // la misma decisión de continuación y la misma regla de total único que
    // `collect_all_pages`.
    let mut items = Vec::new();
    let mut reported_total = None;
    let mut pages = 0usize;
    let mut offset = 0i64;
    loop {
        let page = source
            .fetch_favorites_page(offset)
            .await
            .expect("página de favoritos");
        syncify_tauri_lib::sync::collection::assign_total(&mut reported_total, page.reported_total);
        if page.items.is_empty() {
            break;
        }
        pages += 1;
        let page_len = page.items.len();
        items.extend(page.items);
        match syncify_tauri_lib::services::import_pagination::next_offset(
            offset as i32,
            page_len as i32,
            source.favorites_page_size() as i32,
            reported_total,
        ) {
            Some(next) => offset = next as i64,
            None => break,
        }
    }

    let ids: Vec<&str> = items
        .iter()
        .map(|item| item["SNG_ID"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["1001", "1002", "1003"]);
    assert_eq!(pages, 2, "deben pedirse las dos páginas");
    assert_eq!(
        reported_total,
        Some(3),
        "el total se asigna una vez; la API lo repite en cada página"
    );

    let targets = server.get_targets_containing("/user/4242/tracks");
    assert_eq!(targets.len(), 2);
    assert!(targets[0].contains("index=0"), "{}", targets[0]);
    assert!(
        targets[1].contains("index=2"),
        "avanza por la longitud real de la página, no por el límite: {}",
        targets[1]
    );
}

#[tokio::test]
async fn deezer_albumes_recorre_todas_las_paginas_y_asigna_el_total_una_vez() {
    let server = spawn_mock(page_responder("albums", "/user/4242/albums")).await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let collected = collect_all_pages(100, |offset| source.fetch_albums_page(offset))
        .await
        .expect("recorrido de álbumes");

    let ids: Vec<&str> = collected
        .items
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["3001", "3002", "3003"]);
    assert_eq!(collected.pages, 2);
    assert_eq!(collected.reported_total, Some(3));
    assert_eq!(server.get_targets_containing("/user/4242/albums").len(), 2);
}

#[tokio::test]
async fn deezer_artistas_recorre_todas_las_paginas_y_asigna_el_total_una_vez() {
    let server = spawn_mock(page_responder("artists", "/user/4242/artists")).await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let collected = collect_all_pages(100, |offset| source.fetch_artists_page(offset))
        .await
        .expect("recorrido de artistas");

    let names: Vec<&str> = collected
        .items
        .iter()
        .map(|item| item[1].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["Fixture Artist A", "Fixture Artist B", "Fixture Artist C"]
    );
    assert_eq!(collected.reported_total, Some(3));
    assert_eq!(server.get_targets_containing("/user/4242/artists").len(), 2);
}

#[tokio::test]
async fn deezer_playlists_expone_el_conteo_como_huella_y_un_solo_total() {
    let server = spawn_mock(page_responder("playlists", "/user/4242/playlists")).await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let collected = collect_all_pages(100, |offset| source.fetch_playlists_page(offset))
        .await
        .expect("recorrido de playlists");

    assert_eq!(collected.items.len(), 3);
    assert_eq!(collected.reported_total, Some(3));
    assert_eq!(
        collected.fingerprints,
        vec![
            Some("11".to_string()),
            Some("22".to_string()),
            Some("33".to_string()),
        ]
    );
}

#[tokio::test]
async fn deezer_pistas_de_playlist_paginan_y_asignan_el_total_una_vez() {
    let server = spawn_mock(page_responder("playlist_tracks", "/playlist/555/tracks")).await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let collected = collect_all_pages(100, |offset| {
        source.fetch_playlist_tracks_page("555", offset)
    })
    .await
    .expect("recorrido de pistas de playlist");

    assert_eq!(collected.items.len(), 3);
    assert_eq!(collected.pages, 2);
    assert_eq!(collected.reported_total, Some(3));
    let targets = server.get_targets_containing("/playlist/555/tracks");
    assert!(targets[1].contains("index=2"), "{}", targets[1]);
}

#[tokio::test]
async fn deezer_sin_total_reporta_desconocido_no_cero() {
    let server = spawn_mock(page_responder(
        "favorites_without_total",
        "/user/4242/tracks",
    ))
    .await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let page = source
        .fetch_favorites_page(0)
        .await
        .expect("página sin total");

    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.reported_total, None,
        "un total ausente es cobertura desconocida, nunca 0 %"
    );
}

// ============================================================
// Errores: 401/403, 429 y 404
// ============================================================

#[tokio::test]
async fn deezer_401_se_mapea_a_credencial_caducada() {
    let server = spawn_mock(Arc::new(|method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            return MockResponse::ok(DEEZER_USER_DATA.to_string());
        }
        MockResponse::status(
            401,
            r#"{"error":{"type":"Exception","message":"Invalid arl"}}"#,
        )
    }))
    .await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let error = source
        .fetch_albums_page(0)
        .await
        .expect_err("401 no puede pasar como página vacía");

    assert_eq!(error.kind, SourceErrorKind::AuthExpired);
    assert!(error.message.contains("401"), "{}", error.message);
}

#[tokio::test]
async fn deezer_404_se_mapea_a_error_permanente() {
    let server = spawn_mock(Arc::new(|method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            return MockResponse::ok(DEEZER_USER_DATA.to_string());
        }
        MockResponse::status(404, r#"{"error":{"message":"no such playlist"}}"#)
    }))
    .await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let error = source
        .fetch_playlist_tracks_page("999", 0)
        .await
        .expect_err("404 no puede pasar como página vacía");

    assert_eq!(error.kind, SourceErrorKind::Permanent);
}

#[tokio::test]
async fn deezer_429_reintenta_respestando_retry_after_y_devuelve_la_pagina() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_responder = attempts.clone();
    let server = spawn_mock(Arc::new(move |method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            return MockResponse::ok(DEEZER_USER_DATA.to_string());
        }
        let attempt = attempts_for_responder.fetch_add(1, Ordering::SeqCst);
        if attempt == 0 {
            return MockResponse::status(429, "{}").with_header("Retry-After", "0");
        }
        MockResponse::ok(
            r#"{"data":[{"id":3001,"title":"A","nb_tracks":1}],"total":1}"#.to_string(),
        )
    }))
    .await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let page = source
        .fetch_albums_page(0)
        .await
        .expect("un 429 puntual se reintenta y no falla la página");

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.reported_total, Some(1));
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "un 429 + un 200: la página no se pierde"
    );
}

#[tokio::test]
async fn deezer_429_persistente_es_reintentable_tras_agotar_los_intentos() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_responder = attempts.clone();
    let server = spawn_mock(Arc::new(move |method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            return MockResponse::ok(DEEZER_USER_DATA.to_string());
        }
        attempts_for_responder.fetch_add(1, Ordering::SeqCst);
        MockResponse::status(429, "{}").with_header("Retry-After", "0")
    }))
    .await;
    let client = deezer_client(&server).await;
    let source = DeezerLibrarySource::new(&client);

    let error = source
        .fetch_albums_page(0)
        .await
        .expect_err("429 persistente");

    assert_eq!(error.kind, SourceErrorKind::Retryable);
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        3,
        "el cliente respeta MAX_ATTEMPTS antes de devolver el error"
    );
}

#[test]
fn deezer_clasifica_401_y_403_como_auth_y_el_resto_por_taxonomia() {
    assert_eq!(
        classify_deezer_error("Deezer API error (401 Unauthorized): x"),
        SourceErrorKind::AuthExpired
    );
    assert_eq!(
        classify_deezer_error("Deezer API error (403 Forbidden): x"),
        SourceErrorKind::AuthExpired
    );
    assert_eq!(
        classify_deezer_error("Deezer API error (400 Bad Request): x"),
        SourceErrorKind::Permanent
    );
    assert_eq!(
        classify_deezer_error("Deezer API error (429 Too Many Requests): x"),
        SourceErrorKind::Retryable
    );
    assert_eq!(
        classify_deezer_error("Deezer API error (503 Service Unavailable): x"),
        SourceErrorKind::Retryable
    );
    // Un id de 3 dígitos dentro de un mensaje no se confunde con un status.
    assert_eq!(
        classify_deezer_error("Deezer parse error: track 401"),
        SourceErrorKind::Retryable
    );
}

// ============================================================
// Ritmo: 50 peticiones / 5 s
// ============================================================

#[test]
fn deezer_conserva_el_perfil_de_50_peticiones_por_5_segundos() {
    let limits = default_rate_limits();
    let deezer = limits.get("deezer").expect("perfil de Deezer");
    assert_eq!(deezer.requests_per_window, 50);
    assert_eq!(deezer.window_duration, std::time::Duration::from_secs(5));
    assert_eq!(deezer.min_delay, None);
}
