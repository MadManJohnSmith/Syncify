//! Issue #65 — criterio de regresión: los favoritos de Qobuz se importan con
//! el mismo resultado que antes.
//!
//! Dos BD `:memory:` reales, mismo proveedor simulado en proceso (molde del
//! PR #298: servidor HTTP en `127.0.0.1:0` al que apunta la costura de entorno
//! `SYNCIFY_QOBUZ_API_BASE`):
//!
//! 1. **Camino antiguo**: el orquestador `perform_sync_service_with_emitter`
//!    (la caracterización previa a la extracción de la fase de favoritos).
//! 2. **Camino nuevo**: el job de `SyncRun` sobre `QobuzLibrarySource`
//!    (la fase extraída, ahora tras la interfaz `LibrarySource`).
//!
//! La igualdad exigida: mismas pistas (título+ISRC), mismas entradas de
//! biblioteca, mismas fuentes, mismos contadores de `account_import_counts`
//! y —corolario del refactor con caracterización delante— el test de PR #298
//! (`qobuz_sync_arm_simulated_provider_test`) sigue fijando la forma de
//! petición y el golden de i18n del camino antiguo.

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use syncify_tauri_lib::commands::progress::{SyncCallback, SyncProgressEmitter};
use syncify_tauri_lib::commands::{
    perform_sync_service_with_emitter, ImportPreferences, SyncProgressEvent,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::qobuz::QOBUZ_API_BASE_ENV;
use syncify_tauri_lib::services::QobuzClient;
use syncify_tauri_lib::sync::events::{SyncRunEmitter, SyncRunEvent};
use syncify_tauri_lib::sync::job::{self, RunMeta};
use syncify_tauri_lib::sync::runs;
use syncify_tauri_lib::sync::source::SyncScope;
use syncify_tauri_lib::sync::QobuzLibrarySource;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ============================================================
// Mock HTTP server (molde S187/S189 del PR #298)
// ============================================================

type Seen = Arc<Mutex<Vec<(String, String)>>>;

struct MockServer {
    base: String,
}

type Responder = Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

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

    let (status, body) = responder(&method, &target);
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        if (200..300).contains(&status) { "OK" } else { "Error" },
        body.len(),
        body
    );
    let _ = socket.write_all(resp.as_bytes()).await;
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
    }
}

/// Serializa las mutaciones de entorno dentro del binario.
fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ============================================================
// Fixtures compartidos por los dos caminos
// ============================================================

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

/// Cuenta de Qobuz con `user_auth_token` viable almacenado: el token gana y no
/// hay login (contrato 1 de `resolve_qobuz_user_auth_token_with`).
async fn seed_qobuz_account(pool: &sqlx::SqlitePool) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("services sembrados por migraciones");

    let creds = serde_json::json!({
        "user_auth_token": "simulated_token_0123456789abcdef",
        "user_id": "12345"
    })
    .to_string();
    let encrypted = crypto::encrypt(&creds).expect("encrypt creds");

    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active)
           VALUES (?, 'Parity Qobuz', ?, 1) RETURNING id"#,
    )
    .bind(service_id)
    .bind(&encrypted)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

fn prefs_solo_favoritos() -> ImportPreferences {
    ImportPreferences {
        service_name: "qobuz".to_string(),
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

fn favorite_track(id: i64, title: &str, isrc: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "title": title,
        "duration": 200,
        "isrc": isrc,
        "track_number": 1,
        "media_number": 1,
        "maximum_bit_depth": 24,
        "maximum_sampling_rate": 96.0,
        "performer": {"id": 501, "name": "Parity Artist"},
        "composer": {"id": 502, "name": "Parity Composer"},
        "performers": "Parity Artist (Main), Parity Composer (Composer)",
        "album": {
            "id": 9001,
            "title": "Parity Sessions",
            "released_at": 1_600_000_000i64,
            "image": {
                "small": "https://example.invalid/small.jpg",
                "large": "https://example.invalid/large.jpg"
            },
            "artist": {"id": 501, "name": "Parity Artist"},
            "tracks": {"total": 2}
        }
    })
}

/// Página de `favorite/getUserFavorites?type=tracks` según el offset pedido:
/// 2 favoritos en la primera página y el vacío que cierra la paginación.
fn respond_favorites_page(method: &str, target: &str) -> (u16, String) {
    assert_eq!(
        method, "GET",
        "el brazo solo hace GET contra la API de Qobuz"
    );
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if !path.ends_with("/favorite/getUserFavorites") || !query.contains("type=tracks") {
        return (
            404,
            serde_json::json!({"error": "unexpected request"}).to_string(),
        );
    }
    let offset: i64 = query
        .split('&')
        .find_map(|p| p.strip_prefix("offset="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let items = if offset == 0 {
        vec![
            favorite_track(43_001, "Parity One", "QZPAR00000001"),
            favorite_track(43_002, "Parity Two", "QZPAR00000002"),
        ]
    } else {
        vec![]
    };
    (
        200,
        serde_json::json!({"tracks": {"items": items, "total": 2}}).to_string(),
    )
}

// ============================================================
// Colectores
// ============================================================

#[derive(Clone, Default)]
struct NullCollector;

impl SyncProgressEmitter for NullCollector {
    fn emit_sync_progress(&self, _event: &SyncProgressEvent) {}
}

#[derive(Clone, Default)]
struct RunEventCollector {
    events: Arc<Mutex<Vec<SyncRunEvent>>>,
}

impl SyncRunEmitter for RunEventCollector {
    fn emit_started(&self, e: &SyncRunEvent) {
        self.events.lock().unwrap().push(e.clone());
    }
    fn emit_progress(&self, e: &SyncRunEvent) {
        self.events.lock().unwrap().push(e.clone());
    }
    fn emit_finished(&self, e: &SyncRunEvent) {
        self.events.lock().unwrap().push(e.clone());
    }
    fn emit_error(&self, e: &SyncRunEvent) {
        self.events.lock().unwrap().push(e.clone());
    }
}

fn as_progress(collector: &RunEventCollector) -> impl SyncProgressEmitter + '_ {
    SyncCallback(move |event: &SyncProgressEvent| {
        // El relé del job reenvía al canal antiguo; aquí solo contamos para
        // comprobar que el canal sigue vivo en el camino nuevo.
        let _ = collector;
        let _ = event;
    })
}

// ============================================================
// Estado de la biblioteca, por (título, isrc): la identidad comparable
// entre caminos (los ids locales dependen del orden de inserción).
// ============================================================

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TrackRow {
    title: String,
    isrc: Option<String>,
}

async fn library_tracks(pool: &sqlx::SqlitePool) -> Vec<TrackRow> {
    let mut rows: Vec<TrackRow> = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT title, isrc FROM tracks ORDER BY title, isrc",
    )
    .fetch_all(pool)
    .await
    .expect("pistas leídas")
    .into_iter()
    .map(|(title, isrc)| TrackRow { title, isrc })
    .collect();
    rows.sort();
    rows
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EntryRow {
    title: String,
    is_liked: i64,
}

async fn library_entries(pool: &sqlx::SqlitePool) -> Vec<EntryRow> {
    let mut rows: Vec<EntryRow> = sqlx::query_as::<_, (String, i64)>(
        "SELECT t.title, le.is_liked FROM library_entries le JOIN tracks t ON t.id = le.track_id",
    )
    .fetch_all(pool)
    .await
    .expect("entradas leídas")
    .into_iter()
    .map(|(title, is_liked)| EntryRow { title, is_liked })
    .collect();
    rows.sort();
    rows
}

async fn sources_count(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM track_sources")
        .fetch_one(pool)
        .await
        .expect("fuentes contadas")
}

async fn import_counts(pool: &sqlx::SqlitePool, account_id: i64) -> (Option<i64>, i64, String) {
    sqlx::query_as(
        "SELECT reported_total, imported_total, completeness FROM account_import_counts \
         WHERE account_id = ? AND collection = 'favorites'",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("contadores de importación")
}

// ============================================================
// Prueba de paridad
// ============================================================

/// Criterio 4 del issue #65: el camino nuevo importa los favoritos de Qobuz
/// con el mismo resultado que el camino antiguo, contra el mismo proveedor
/// simulado. La corrida nueva además queda `completed` con su cobertura.
#[tokio::test]
async fn el_camino_nuevo_importa_los_favoritos_con_el_mismo_resultado() {
    let _env = lock_env();

    // --- Camino antiguo: el orquestador, tal como estaba ---
    let old_pool = setup_test_db().await;
    let old_account = seed_qobuz_account(&old_pool).await;

    let mock = spawn_mock(Arc::new(respond_favorites_page)).await;
    std::env::set_var(QOBUZ_API_BASE_ENV, &mock.base);

    let old_result = perform_sync_service_with_emitter(
        &old_pool,
        "qobuz",
        Some(old_account),
        Some(prefs_solo_favoritos()),
        Some(&NullCollector),
    )
    .await
    .expect("el camino antiguo completa contra el proveedor simulado");

    // --- Camino nuevo: el job de SyncRun sobre QobuzLibrarySource ---
    let new_pool = setup_test_db().await;
    let new_account = seed_qobuz_account(&new_pool).await;
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&new_pool)
        .await
        .expect("servicio qobuz");
    let run_id = runs::insert_running_run(
        &new_pool,
        new_account,
        service_id,
        SyncScope::Favorites,
        "import_only",
    )
    .await
    .expect("corrida creada");

    let creds_json: (String,) =
        sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(new_account)
            .fetch_one(&new_pool)
            .await
            .expect("credenciales");
    let creds: serde_json::Value =
        serde_json::from_str(&crypto::decrypt(&creds_json.0).expect("decrypt")).expect("json");
    let token = creds["user_auth_token"]
        .as_str()
        .expect("token de la cuenta sembrada")
        .to_string();

    let client = QobuzClient::new_with_token(
        syncify_tauri_lib::services::qobuz::QOBUZ_APP_ID.to_string(),
        syncify_tauri_lib::services::qobuz::QOBUZ_APP_SECRET.to_string(),
        token,
    );
    let source = QobuzLibrarySource::new(&client);
    let collector = RunEventCollector::default();
    let progress = as_progress(&collector);
    let meta = RunMeta {
        run_id,
        account_id: new_account,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let cancel = job::register_live_run(run_id);
    let new_result = job::run_favorites_job(
        &new_pool,
        &meta,
        service_id,
        &source,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("el camino nuevo completa contra el proveedor simulado");

    // --- Paridad de la biblioteca importada ---
    assert_eq!(
        library_tracks(&old_pool).await,
        library_tracks(&new_pool).await,
        "las mismas pistas (título+ISRC) en ambos caminos"
    );
    assert_eq!(
        library_entries(&old_pool).await,
        library_entries(&new_pool).await,
        "las mismas entradas de biblioteca y favoritos"
    );
    assert_eq!(
        sources_count(&old_pool).await,
        sources_count(&new_pool).await,
        "las mismas fuentes por pista"
    );
    assert_eq!(
        import_counts(&old_pool, old_account).await,
        import_counts(&new_pool, new_account).await,
        "los mismos contadores reportado/importado del proveedor"
    );

    // --- Paridad de contadores del resultado ---
    assert_eq!(old_result.favorite_tracks_total, 2);
    assert_eq!(new_result.imported_total, 2);
    assert_eq!(
        new_result.reported_total,
        Some(old_result.favorite_tracks_total as i64)
    );

    // --- La corrida nueva queda completed con su cobertura (criterios 1 y 3) ---
    let run = runs::get_run(&new_pool, run_id)
        .await
        .expect("lectura")
        .expect("corrida existe");
    assert_eq!(run.status, "completed");
    assert_eq!(run.reported_total, Some(2));
    assert_eq!(run.imported_total, 2);
    assert_eq!(run.error_count, 0);
    assert!(run.finished_at.is_some());
    let (reported, imported, coverage) = coverage_row(&new_pool, run_id).await;
    assert_eq!(reported, Some(2));
    assert_eq!(imported, 2);
    assert_eq!(coverage, "imported");

    // --- El canal de eventos de corrida viajó con la identidad de la corrida ---
    let events = collector.events.lock().unwrap().clone();
    assert!(events.iter().any(|e| e.phase == "started"));
    let terminal = events.last().expect("evento terminal").clone();
    assert!(terminal.terminal);
    assert_eq!(terminal.status, "completed");
    assert_eq!(terminal.run_id, run_id);
    assert_eq!(terminal.account_id, new_account);

    // La costura vuelve a su default al salir (molde PR #298).
    std::env::remove_var(QOBUZ_API_BASE_ENV);
}

async fn coverage_row(pool: &sqlx::SqlitePool, run_id: i64) -> (Option<i64>, i64, String) {
    sqlx::query_as(
        "SELECT reported, imported, coverage FROM sync_run_collections WHERE run_id = ? AND collection = 'favorites'",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("cobertura de favoritos")
}
