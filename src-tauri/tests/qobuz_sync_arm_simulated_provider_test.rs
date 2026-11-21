//! D-08b (issue #41) — caracterización de la red del quinto brazo del motor
//! (`sync_qobuz_arm`, el último extraído) contra un proveedor simulado en
//! proceso, más el golden de las cadenas de i18n que viajan en el payload de
//! los eventos de progreso.
//!
//! Dos capas, siguiendo el molde S187/S189 que ya usan
//! `favorites_service_arms_test` y `soundcloud_unified_engine_test`:
//!
//! 1. **Red**: un servidor HTTP simulado en `127.0.0.1:0` al que apunta el
//!    cliente Qobuz mediante la costura de entorno `SYNCIFY_QOBUZ_API_BASE`
//!    (mismo molde que `SYNCIFY_DEEZER_API_BASE`). El mock graba cada petición
//!    (método + target con query) para caracterizar la forma exacta de la
//!    petición que el brazo envía: endpoint, paginación, firma. Sin red real.
//! 2. **Catálogo**: migraciones reales sobre SQLite `:memory:`; el resultado
//!    del brazo (contadores de `ServiceSyncResult`, filas escritas y secuencia
//!    de eventos de progreso) queda fijado por aserciones.
//!
//! El golden de i18n cubre las cadenas que viajan en el payload (`message` de
//! `SyncProgressEvent`): son las cadenas fuente en inglés que la UI presenta.
//! La comparación es exacta por plantilla (los dígitos se normalizan a `{}`),
//! así que una cadena traducida que se cuele en el payload hace fallar el
//! test — hay una prueba negativa que lo demuestra sin depender de la red.

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use syncify_tauri_lib::commands::{
    perform_sync_service_with_emitter, ImportPreferences, ServiceSyncResult, SyncProgressEmitter,
    SyncProgressEvent,
};
use syncify_tauri_lib::crypto;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ============================================================
// Mock HTTP server (molde S187/S189)
// ============================================================

/// Lo que el mock vio: `(método, target)` por cada petición respondida.
type Seen = Arc<Mutex<Vec<(String, String)>>>;

struct MockServer {
    base: String,
    seen: Seen,
}

impl MockServer {
    fn requests(&self) -> Vec<(String, String)> {
        self.seen.lock().expect("mock recorder").clone()
    }
}

type Responder = Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

/// Responde una conexión: lee la línea de petición, entrega `(método, target)`
/// al responder y escribe una respuesta JSON completa antes de cerrar.
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

/// Levanta el proveedor simulado; el responder recibe `(método, target-con-query)`.
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

// ============================================================
// Fixtures
// ============================================================

/// Serializa las mutaciones de entorno dentro del binario (molde de
/// `favorites_service_arms_test`): las variables de proceso son globales.
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
           VALUES (?, 'Simulated Qobuz', ?, 1) RETURNING id"#,
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

/// Dos favoritos de pista con la forma que el brazo lee (performer, álbum con
/// portada y fecha, ISRC, calidad hi-res): suficiente para el camino feliz del
/// motor unificado sin red (el brazo sincroniza con `query_musicbrainz: false`).
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
        "performer": {"id": 501, "name": "Simulated Artist"},
        "composer": {"id": 502, "name": "Simulated Composer"},
        "performers": "Simulated Artist (Main), Simulated Composer (Composer)",
        "album": {
            "id": 9001,
            "title": "Simulated Sessions",
            "released_at": 1_600_000_000i64,
            "image": {
                "small": "https://example.invalid/small.jpg",
                "large": "https://example.invalid/large.jpg"
            },
            "artist": {"id": 501, "name": "Simulated Artist"},
            "tracks": {"total": 2}
        }
    })
}

/// Página de `favorite/getUserFavorites?type=tracks` según el offset pedido:
/// una página con 2 favoritos y después el vacío que cierra la paginación.
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
            favorite_track(42_001, "Golden Arm One", "QZSIM00000001"),
            favorite_track(42_002, "Golden Arm Two", "QZSIM00000002"),
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
// Colector de eventos
// ============================================================

#[derive(Clone, Default)]
struct TestEventCollector {
    events: Arc<Mutex<Vec<SyncProgressEvent>>>,
}

impl TestEventCollector {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_events(&self) -> Vec<SyncProgressEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl SyncProgressEmitter for TestEventCollector {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

// ============================================================
// Golden de i18n: las cadenas que viajan en el payload
// ============================================================

/// Golden de las cadenas de i18n del payload del quinto brazo, en el orden
/// emitido y por plantilla: cada dígito corrido se normaliza a `{}` porque los
/// contadores que interpolan no son parte del contrato de traducción. Lo que
/// sí es contrato: el texto fuente en inglés exacto que la UI recibe.
///
/// - `authenticating`: el orquestador abre presentando la conexión.
/// - `fetching_favorite_tracks` / `importing_favorite_tracks`: cadenas del
///   brazo Qobuz (fase 1, favoritos).
/// - `persisting` / `completed`: cierre del orquestador.
const GOLDEN_PAYLOAD_I18N: &[&str] = &[
    "Authenticating qobuz connection...",
    "Importing favorite tracks...",
    "Importing favorite tracks ({}/{})",
    "Importing favorite tracks ({}/{})",
    "Persisting sync metadata and updating timestamps...",
    "Sync completed for qobuz: {} tracks new global, {} sources new, {} library entries new ({} already present, {} favorites, {} albums, {} playlists)",
];

/// Normaliza una cadena del payload para compararla contra el golden: los
/// dígitos corridos se sustituyen por `{}`. Una traducción (texto distinto al
/// fuente) no sobrevive a esta comparación ni por omisión ni por plantilla.
fn normalize_payload_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_digits = false;
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            if !in_digits {
                out.push('{');
                out.push('}');
                in_digits = true;
            }
        } else {
            in_digits = false;
            out.push(ch);
        }
    }
    out
}

/// Compara las cadenas observadas en el payload contra el golden. Devuelve el
/// primer desajuste (índice, observado, plantilla esperada) para que el fallo
/// del test señale la cadena que se coló.
fn payload_matches_golden(observed: &[String]) -> Result<(), String> {
    if observed.len() != GOLDEN_PAYLOAD_I18N.len() {
        return Err(format!(
            "el payload trae {} cadenas y el golden espera {}: observadas = {:?}",
            observed.len(),
            GOLDEN_PAYLOAD_I18N.len(),
            observed
        ));
    }
    for (i, observed_str) in observed.iter().enumerate() {
        let normalized = normalize_payload_string(observed_str);
        if normalized != GOLDEN_PAYLOAD_I18N[i] {
            return Err(format!(
                "cadena {} del payload no está en el golden: observado {:?} (normalizado {:?}), esperado {:?}",
                i, observed_str, normalized, GOLDEN_PAYLOAD_I18N[i]
            ));
        }
    }
    Ok(())
}

// ============================================================
// Pruebas
// ============================================================

/// Criterio (3) del issue: una cadena traducida en el payload hace fallar el
/// test. Demostración directa del comparador: la misma secuencia del golden con
/// una sola cadena traducida interpolada no pasa.
#[test]
fn una_cadena_traducida_en_el_payload_hace_fallar_el_golden() {
    let traducida = "Importando pistas favoritas...".to_string();
    let payload_con_traduccion = vec![
        "Authenticating qobuz connection...".to_string(),
        traducida.clone(),
        "Importing favorite tracks (1/2)".to_string(),
        "Importing favorite tracks (2/2)".to_string(),
        "Persisting sync metadata and updating timestamps...".to_string(),
        "Sync completed for qobuz: 2 tracks new global, 2 sources new, 2 library entries new (0 already present, 2 favorites, 0 albums, 0 playlists)".to_string(),
    ];
    let error = payload_matches_golden(&payload_con_traduccion)
        .expect_err("una cadena traducida en el payload debe hacer fallar el golden");
    assert!(
        error.contains(&traducida),
        "el fallo debe señalar la cadena traducida: {}",
        error
    );

    // Y también una traducción colada en una cadena interpolada del cierre.
    let payload_con_cierre_traducido = vec![
        "Authenticating qobuz connection...".to_string(),
        "Importing favorite tracks...".to_string(),
        "Importing favorite tracks (1/2)".to_string(),
        "Importing favorite tracks (2/2)".to_string(),
        "Persisting sync metadata and updating timestamps...".to_string(),
        "Sincronización completada para qobuz: 2 pistas nuevas globales, 2 fuentes nuevas, 2 entradas de biblioteca nuevas (0 ya presentes, 2 favoritos, 0 álbumes, 0 listas)".to_string(),
    ];
    assert!(
        payload_matches_golden(&payload_con_cierre_traducido).is_err(),
        "un cierre traducido en el payload también debe fallar"
    );
}

/// Sanidad del comparador: el golden pasa por su propia puerta (y los dígitos
/// interpolados se normalizan a la plantilla).
#[test]
fn el_golden_acepta_el_payload_caracterizado() {
    let payload_caracterizado = vec![
        "Authenticating qobuz connection...".to_string(),
        "Importing favorite tracks...".to_string(),
        "Importing favorite tracks (1/2)".to_string(),
        "Importing favorite tracks (2/2)".to_string(),
        "Persisting sync metadata and updating timestamps...".to_string(),
        "Sync completed for qobuz: 2 tracks new global, 2 sources new, 2 library entries new (0 already present, 2 favorites, 0 albums, 0 playlists)".to_string(),
    ];
    payload_matches_golden(&payload_caracterizado)
        .expect("el payload caracterizado debe pasar el golden");
}

/// Criterios (1), (2), (4) y (5): el quinto brazo recorre su fase de favoritos
/// contra un proveedor simulado en proceso, sin red; el golden cubre las
/// cadenas que viajan en el payload y la forma de la petición queda fijada.
#[tokio::test]
async fn quinto_brazo_qobuz_importa_favoritos_contra_proveedor_simulado() {
    let _env = lock_env();
    let pool = setup_test_db().await;
    let account_id = seed_qobuz_account(&pool).await;
    let collector = TestEventCollector::new();

    let mock = spawn_mock(Arc::new(respond_favorites_page)).await;
    std::env::set_var(
        syncify_tauri_lib::services::qobuz::QOBUZ_API_BASE_ENV,
        &mock.base,
    );

    let result: Result<ServiceSyncResult, String> = perform_sync_service_with_emitter(
        &pool,
        "qobuz",
        Some(account_id),
        Some(prefs_solo_favoritos()),
        Some(&collector),
    )
    .await;

    let result = result.expect("el brazo debe completar contra el proveedor simulado");

    // --- Lo que el mock vio: la petición que el brazo envía de verdad ---
    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        1,
        "una sola página: 2 favoritos con limit 50 cierran la paginación"
    );
    let (method, target) = &requests[0];
    assert_eq!(method, "GET");
    let (path, query) = target.split_once('?').expect("la petición lleva query");
    assert_eq!(
        path, "/favorite/getUserFavorites",
        "endpoint exacto de favoritos de Qobuz"
    );
    for param in ["type=tracks", "offset=0", "limit=50"] {
        assert!(
            query.contains(param),
            "la petición debe llevar {}: query = {}",
            param,
            query
        );
    }
    assert!(
        query.contains("app_id=") && query.contains("request_sig="),
        "la petición va firmada como la API real exige: query = {}",
        query
    );

    // --- Contadores del resultado (contrato D-08a: el SyncOutcome único) ---
    assert!(result.success, "resultado con éxito: {}", result.message);
    assert_eq!(result.sync_outcome.as_deref(), Some("success"));
    assert_eq!(result.favorite_tracks_total, 2, "favoritos procesados");
    // `favorites_seen` lo alimentan los brazos que pasan por el helper común
    // de resultado por pista (deezer/spotify); el brazo Qobuz incrementa
    // `favorite_tracks_total` por su propio camino. Caracterizado tal cual.
    assert_eq!(
        result.favorites_seen, 0,
        "el brazo Qobuz no pasa por el contador compartido favorites_seen"
    );
    assert_eq!(result.imported_tracks_total, 2, "pistas importadas");
    assert_eq!(result.tracks_new_global, 2, "pistas nuevas en el catálogo");
    assert_eq!(
        result.library_entries_new_for_account, 2,
        "entradas de biblioteca nuevas"
    );
    assert_eq!(
        result.tracks_already_present, 0,
        "primera pasada: nada preexistente"
    );
    assert!(result.errors.is_empty(), "sin errores: {:?}", result.errors);
    assert!(
        result.warnings.is_empty(),
        "sin avisos: {:?}",
        result.warnings
    );

    // --- Lo que queda escrito en la BD tras el brazo ---
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 2, "las dos pistas del proveedor simulado");
    let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entries, 2, "las dos entradas de biblioteca del brazo");
    let (reported, imported): (i64, i64) = sqlx::query_as(
        "SELECT reported_total, imported_total FROM account_import_counts \
         WHERE account_id = ? AND collection = 'favorites'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reported, 2, "el proveedor declaró 2 favoritos");
    assert_eq!(imported, 2, "el brazo importó los 2 declarados");

    // --- Secuencia de fases emitidas ---
    let events = collector.get_events();
    let fases: Vec<String> = events.iter().map(|e| e.phase.clone()).collect();
    assert_eq!(
        fases,
        vec![
            "authenticating".to_string(),
            "fetching_favorite_tracks".to_string(),
            "importing_favorite_tracks".to_string(),
            "importing_favorite_tracks".to_string(),
            "persisting".to_string(),
            "completed".to_string(),
        ],
        "secuencia completa de fases del quinto brazo"
    );

    // --- Golden de i18n: las cadenas que viajan en el payload ---
    let cadenas: Vec<String> = events.iter().map(|e| e.message.clone()).collect();
    payload_matches_golden(&cadenas)
        .expect("las cadenas del payload deben ser exactamente las del golden");

    // --- Idempotencia: segunda pasada contra el mismo proveedor simulado ---
    let collector2 = TestEventCollector::new();
    let result2 = perform_sync_service_with_emitter(
        &pool,
        "qobuz",
        Some(account_id),
        Some(prefs_solo_favoritos()),
        Some(&collector2),
    )
    .await
    .expect("la segunda pasada también completa");
    assert_eq!(
        result2.imported_tracks_total, 0,
        "nada nuevo la segunda vez"
    );
    assert_eq!(
        result2.tracks_already_present, 2,
        "las 2 pistas ya estaban y no se duplican"
    );
    let tracks2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks2, 2, "sin duplicados tras la segunda pasada");

    // La costura vuelve a su default al salir: este test es el único del
    // binario que la fija y la guardaia la libera al terminar.
    std::env::remove_var(syncify_tauri_lib::services::qobuz::QOBUZ_API_BASE_ENV);
}
