//! Issue #68 — Apple Music bajo el mismo contrato de sincronización.
//!
//! El importador de Apple Music deja de ser una caja aparte: el brazo
//! `apple_music` del motor unificado reporta fases y progreso, respeta las
//! colecciones activadas y la cuenta elegida, escribe cobertura y marca los
//! huecos como parciales. Los ids de biblioteca (`i.…`) y los de catálogo son
//! identidades distintas: la fuente descargable guarda el id de catálogo y la
//! pertenencia conserva el de biblioteca.
//!
//! Todo va contra un servidor HTTP en proceso mediante la costura
//! `SYNCIFY_APPLE_MUSIC_BASE_URL`, con migraciones reales sobre SQLite
//! `:memory:`. Sin red.

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use syncify_tauri_lib::commands::{
    perform_sync_service_with_emitter, ImportPreferences, SyncProgressEmitter, SyncProgressEvent,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::apple_music::APPLE_MUSIC_BASE_URL_ENV;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Serializa la costura de entorno y el servidor simulado dentro del binario.
fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

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

fn apple_credentials(music_user_token: &str) -> String {
    crypto::encrypt(
        &serde_json::json!({
            "developer_token": "developer-token",
            "music_user_token": music_user_token,
            "storefront": "us"
        })
        .to_string(),
    )
    .expect("encrypt Apple credentials")
}

async fn seed_apple_account(pool: &sqlx::SqlitePool, name: &str, token: &str) -> i64 {
    let service_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'apple_music'")
            .fetch_one(pool)
            .await
            .expect("apple_music service must exist");

    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, credentials_json, is_active) \
         VALUES (?, ?, ?, 1) RETURNING id",
    )
    .bind(service_id)
    .bind(name)
    .bind(apple_credentials(token))
    .fetch_one(pool)
    .await
    .expect("seed Apple account")
}

#[derive(Clone, Debug)]
struct MockRequest {
    method: String,
    target: String,
    media_user_token: Option<String>,
}

struct MockServer {
    base: String,
    requests: Arc<Mutex<Vec<MockRequest>>>,
}

type Responder = Arc<dyn Fn(&MockRequest) -> (u16, String) + Send + Sync>;

async fn spawn_mock(responder: Responder) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::<MockRequest>::new()));
    let requests_for_accept = requests.clone();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let responder = responder.clone();
            let requests = requests_for_accept.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 32768];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                let mut lines = raw.lines();
                let request_line = lines.next().unwrap_or("");
                let mut parts = request_line.split_whitespace();
                let method = parts.next().unwrap_or("").to_string();
                let target = parts.next().unwrap_or("").to_string();
                let media_user_token = lines
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.trim().eq_ignore_ascii_case("media-user-token"))
                    .map(|(_, value)| value.trim().to_string());

                let request = MockRequest {
                    method,
                    target,
                    media_user_token,
                };
                requests
                    .lock()
                    .expect("mock recorder")
                    .push(request.clone());

                let (status, body) = responder(&request);
                let reason = if (200..300).contains(&status) {
                    "OK"
                } else {
                    "Error"
                };
                let response = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    reason,
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });

    MockServer {
        base: format!("http://{}", addr),
        requests,
    }
}

#[derive(Clone, Default)]
struct ProgressCollector {
    events: Arc<Mutex<Vec<SyncProgressEvent>>>,
}

impl SyncProgressEmitter for ProgressCollector {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        self.events
            .lock()
            .expect("progress collector")
            .push(event.clone());
    }
}

fn song(id: &str, catalog_id: &str, name: &str, isrc: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "attributes": {
            "name": name,
            "artistName": "Contract Artist",
            "albumName": "Contract Album",
            "durationInMillis": 200000,
            "isrc": isrc,
            "dateAdded": "2024-01-02T10:00:00Z",
            "playParams": { "catalogId": catalog_id }
        }
    })
}

/// Solo canciones, la colección que el corte de favoritos recorre.
fn songs_only_prefs() -> ImportPreferences {
    ImportPreferences {
        service_name: "apple_music".to_string(),
        favorite_tracks: true,
        favorite_albums: false,
        favorite_artists: false,
        playlists: false,
        purchases: false,
        library_history: false,
        include_appearances: false,
        force_retry_unavailable: false,
    }
}

fn respond_songs(request: &MockRequest) -> (u16, String) {
    match request.target.as_str() {
        target if target.starts_with("/me/library/songs?") => (
            200,
            serde_json::json!({
                "data": [
                    song("i.one", "1000000001", "Contract One", "APPLECON0001"),
                    song("i.two", "1000000002", "Contract Two", "APPLECON0002")
                ],
                "next": null,
                "meta": { "total": 2 }
            })
            .to_string(),
        ),
        _ => (404, r#"{"error":"unexpected request"}"#.to_string()),
    }
}

// ============================================================
// Cuenta elegida y fases de progreso
// ============================================================

/// La corrida usa la cuenta pedida, no la activa más reciente, y emite las
/// mismas fases que el resto de brazos.
#[tokio::test]
async fn apple_run_uses_the_chosen_account_and_emits_phases() {
    let _env = lock_env();
    let pool = setup_test_db().await;

    // La cuenta elegida es la primera; la más reciente (la que el camino
    // legacy cargaría) tiene un token que el proveedor rechaza.
    let chosen = seed_apple_account(&pool, "Chosen Apple", "chosen-token").await;
    let decoy = seed_apple_account(&pool, "Newest Apple", "decoy-token").await;

    let server = spawn_mock(Arc::new(|request: &MockRequest| {
        if request.media_user_token.as_deref() != Some("chosen-token") {
            return (401, r#"{"errors":[{"status":"401"}]}"#.to_string());
        }
        respond_songs(request)
    }))
    .await;
    std::env::set_var(APPLE_MUSIC_BASE_URL_ENV, &server.base);

    let collector = ProgressCollector::default();
    let result = perform_sync_service_with_emitter(
        &pool,
        "apple_music",
        Some(chosen),
        Some(songs_only_prefs()),
        Some(&collector),
    )
    .await
    .expect("the Apple run completes against the mock");

    std::env::remove_var(APPLE_MUSIC_BASE_URL_ENV);

    assert_eq!(result.account_id, Some(chosen));
    assert!(result.success, "the run must not be a hard failure");
    assert_eq!(result.favorite_tracks_total, 2);

    // Los datos pertenecen a la cuenta elegida y la otra no se tocó.
    let chosen_entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(chosen)
            .fetch_one(&pool)
            .await
            .expect("chosen library entries");
    let decoy_entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(decoy)
            .fetch_one(&pool)
            .await
            .expect("decoy library entries");
    assert_eq!(chosen_entries, 2);
    assert_eq!(decoy_entries, 0);

    // Mismas fases que el resto: descarga de favoritos e importación por pista.
    let events = collector.events.lock().expect("progress events").clone();
    assert!(
        events.iter().any(|e| e.phase == "fetching_favorite_tracks"),
        "missing fetch phase: {:?}",
        events.iter().map(|e| e.phase.clone()).collect::<Vec<_>>()
    );
    let importing: Vec<&SyncProgressEvent> = events
        .iter()
        .filter(|e| e.phase == "importing_favorite_tracks")
        .collect();
    assert!(
        importing.len() >= 2,
        "expected per-track progress, got {:?}",
        importing.iter().map(|e| e.current).collect::<Vec<_>>()
    );
    assert_eq!(importing.last().map(|e| e.current), Some(2));
    assert_eq!(importing.last().and_then(|e| e.total), Some(2));
    assert!(importing.iter().all(|e| !e.terminal));
}

// ============================================================
// Colecciones activadas
// ============================================================

/// Con favoritos desactivado no se pide la colección de canciones; solo la
/// colección activada se recorre.
#[tokio::test]
async fn apple_run_respects_the_activated_collections() {
    let _env = lock_env();
    let pool = setup_test_db().await;
    let account = seed_apple_account(&pool, "Collections Apple", "token").await;

    let server = spawn_mock(Arc::new(|request: &MockRequest| {
        if request.target.starts_with("/me/library/albums?offset=0") {
            return (
                200,
                serde_json::json!({
                    "data": [{
                        "id": "l.album1",
                        "attributes": {
                            "name": "Contract Album",
                            "artistName": "Contract Artist",
                            "trackCount": 1,
                            "dateAdded": "2024-01-02T10:00:00Z"
                        }
                    }],
                    "next": null,
                    "meta": { "total": 1 }
                })
                .to_string(),
            );
        }
        if request
            .target
            .starts_with("/me/library/albums/l.album1/tracks?offset=0")
        {
            return (
                200,
                serde_json::json!({
                    "data": [song("i.album", "1000000010", "Album Track", "APPLEALB001")],
                    "next": null
                })
                .to_string(),
            );
        }
        (404, r#"{"error":"unexpected request"}"#.to_string())
    }))
    .await;
    std::env::set_var(APPLE_MUSIC_BASE_URL_ENV, &server.base);

    let prefs = ImportPreferences {
        service_name: "apple_music".to_string(),
        favorite_tracks: false,
        favorite_albums: true,
        favorite_artists: false,
        playlists: false,
        purchases: false,
        library_history: false,
        include_appearances: false,
        force_retry_unavailable: false,
    };

    let result = perform_sync_service_with_emitter(
        &pool,
        "apple_music",
        Some(account),
        Some(prefs),
        Option::<&ProgressCollector>::None,
    )
    .await
    .expect("the album-only run completes");

    let targets: Vec<String> = server
        .requests
        .lock()
        .expect("mock requests")
        .iter()
        .map(|r| r.target.clone())
        .collect();
    std::env::remove_var(APPLE_MUSIC_BASE_URL_ENV);

    assert!(
        targets.iter().all(|t| t.starts_with("/me/library/albums")),
        "a disabled collection must not be requested: {:?}",
        targets
    );
    assert!(
        !targets.iter().any(|t| t.contains("me/library/songs")),
        "favorites were disabled: {:?}",
        targets
    );
    assert!(
        !targets.iter().any(|t| t.contains("me/library/playlists")),
        "playlists were disabled: {:?}",
        targets
    );
    assert_eq!(result.favorite_albums_total, 1);
    assert_eq!(result.favorite_tracks_total, 0);

    let entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(account)
            .fetch_one(&pool)
            .await
            .expect("album tracks imported");
    assert_eq!(entries, 1);
}

// ============================================================
// Cobertura y huecos
// ============================================================

/// Un hueco registrado deja la cobertura en `partial`, nunca en `complete`.
#[tokio::test]
async fn apple_gaps_are_reported_as_partial() {
    let _env = lock_env();
    let pool = setup_test_db().await;
    let account = seed_apple_account(&pool, "Gaps Apple", "token").await;

    let server = spawn_mock(Arc::new(|request: &MockRequest| {
        if request.target.starts_with("/me/library/songs?") {
            return respond_songs(request);
        }
        if request.target.starts_with("/me/library/playlists?offset=0") {
            return (
                200,
                serde_json::json!({
                    "data": [{
                        "id": "l.playlist1",
                        "attributes": {
                            "name": "Contract Playlist",
                            "dateAdded": "2024-01-02T10:00:00Z"
                        }
                    }],
                    "next": null,
                    "meta": { "total": 1 }
                })
                .to_string(),
            );
        }
        if request
            .target
            .starts_with("/me/library/playlists/l.playlist1/tracks")
        {
            return (500, r#"{"error":"playlist expansion failed"}"#.to_string());
        }
        (404, r#"{"error":"unexpected request"}"#.to_string())
    }))
    .await;
    std::env::set_var(APPLE_MUSIC_BASE_URL_ENV, &server.base);

    let prefs = ImportPreferences {
        service_name: "apple_music".to_string(),
        favorite_tracks: true,
        favorite_albums: false,
        favorite_artists: false,
        playlists: true,
        purchases: false,
        library_history: false,
        include_appearances: false,
        force_retry_unavailable: false,
    };

    let result = perform_sync_service_with_emitter(
        &pool,
        "apple_music",
        Some(account),
        Some(prefs),
        Option::<&ProgressCollector>::None,
    )
    .await
    .expect("the failed playlist is a gap, not a dead run");
    std::env::remove_var(APPLE_MUSIC_BASE_URL_ENV);

    assert!(
        result
            .errors
            .iter()
            .any(|e| e.contains("Apple Music") && e.contains("playlist")),
        "the gap must be recorded: {:?}",
        result.errors
    );

    let (reported, imported, completeness): (Option<i64>, i64, String) = sqlx::query_as(
        "SELECT reported_total, imported_total, completeness FROM account_import_counts \
         WHERE account_id = ? AND collection = 'favorites'",
    )
    .bind(account)
    .fetch_one(&pool)
    .await
    .expect("favorites coverage row");
    assert_eq!(reported, Some(2));
    assert_eq!(imported, 2);
    assert_eq!(completeness, "partial");
}

// ============================================================
// Identidad de biblioteca vs identidad de catálogo
// ============================================================

/// La fuente descargable guarda el id de catálogo; la pertenencia conserva el
/// id de biblioteca. Nunca se mezclan.
#[tokio::test]
async fn apple_library_ids_are_not_stored_as_catalog_ids() {
    let _env = lock_env();
    let pool = setup_test_db().await;
    let account = seed_apple_account(&pool, "Identity Apple", "token").await;

    let server = spawn_mock(Arc::new(|request: &MockRequest| {
        if request.target.starts_with("/me/library/songs?") {
            return (
                200,
                serde_json::json!({
                    "data": [song("i.libsong", "2000000001", "Library Song", "APPLEID0001")],
                    "next": null,
                    "meta": { "total": 1 }
                })
                .to_string(),
            );
        }
        if request.target.starts_with("/me/library/playlists?offset=0") {
            return (
                200,
                serde_json::json!({
                    "data": [{
                        "id": "l.playlist1",
                        "attributes": {
                            "name": "Identity Playlist",
                            "dateAdded": "2024-01-02T10:00:00Z"
                        }
                    }],
                    "next": null,
                    "meta": { "total": 1 }
                })
                .to_string(),
            );
        }
        if request
            .target
            .starts_with("/me/library/playlists/l.playlist1/tracks")
        {
            return (
                200,
                serde_json::json!({
                    "data": [song("i.libplaylist", "2000000002", "Playlist Song", "APPLEID0002")],
                    "next": null
                })
                .to_string(),
            );
        }
        (404, r#"{"error":"unexpected request"}"#.to_string())
    }))
    .await;
    std::env::set_var(APPLE_MUSIC_BASE_URL_ENV, &server.base);

    let prefs = ImportPreferences {
        service_name: "apple_music".to_string(),
        favorite_tracks: true,
        favorite_albums: false,
        favorite_artists: false,
        playlists: true,
        purchases: false,
        library_history: false,
        include_appearances: false,
        force_retry_unavailable: false,
    };

    perform_sync_service_with_emitter(
        &pool,
        "apple_music",
        Some(account),
        Some(prefs),
        Option::<&ProgressCollector>::None,
    )
    .await
    .expect("the identity run completes");
    std::env::remove_var(APPLE_MUSIC_BASE_URL_ENV);

    let mut source_ids: Vec<String> = sqlx::query_scalar(
        "SELECT ts.service_track_id FROM track_sources ts \
         JOIN services s ON s.id = ts.service_id WHERE LOWER(s.name) = 'apple_music'",
    )
    .fetch_all(&pool)
    .await
    .expect("apple track sources");
    source_ids.sort();
    assert_eq!(
        source_ids,
        vec!["2000000001".to_string(), "2000000002".to_string()],
        "the downloadable source carries the catalog id, not the library id"
    );

    let provider_items: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT provider_item FROM library_memberships WHERE account_id = ? ORDER BY provider_item",
    )
    .bind(account)
    .fetch_all(&pool)
    .await
    .expect("library memberships");
    let provider_items: Vec<String> = provider_items.into_iter().flatten().collect();
    assert!(
        provider_items.iter().any(|item| item == "i.libsong"),
        "the favorite membership keeps the library id: {:?}",
        provider_items
    );
    assert!(
        provider_items.iter().any(|item| item == "i.libplaylist"),
        "the playlist membership keeps the library id: {:?}",
        provider_items
    );
    assert!(
        !provider_items
            .iter()
            .any(|item| item.starts_with("200000000")),
        "catalog ids must not leak into library membership: {:?}",
        provider_items
    );
}
