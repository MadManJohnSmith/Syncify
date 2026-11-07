//! Caracterización offline del camino de audio (P6 Fase A, delante del
//! refactor de ADR-0024).
//!
//! Congela, sin red y sin dispositivo de audio, lo que la Fase B va a mover:
//!
//! 1. La orquestación de `player_load_track`: rama local, cancelación por
//!    generación, error agregado del resolver y semántica del marcador de
//!    caché (`.done`).
//! 2. La frontera de streaming y los seeks pendientes (`player_seek` +
//!    `guard_stream_frontier`) con sus márgenes exactos.
//! 3. La descarga progresiva contra un servidor HTTP falso en 127.0.0.1
//!    (`spawn_stream_download`): objetivo de prime (piso 256 KiB, techo 8 MiB,
//!    fallback 4 MiB sin Content-Length), escritura de chunks, marcador al
//!    terminar, cancelación a mitad y error de red sin re-resolución.
//!
//! El motor se inyecta con el backend simulado de `syncify_player::sim`
//! (reloj virtual: ninguna prueba espera audio real); los ficheros son WAV
//! PCM sintéticos que symphonia decodifica de verdad.

use std::path::Path;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use syncify_player::sim::{SimulatedBackend, VirtualClock};
use tauri::test::MockRuntime;
use tauri::{Listener, Manager};
use tokio::io::AsyncReadExt as _;

use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WAV_SAMPLE_RATE: u32 = 8000;

fn sim_engine(clock: &VirtualClock) -> PlayerEngine {
    PlayerEngine::with_backend(Box::new(SimulatedBackend::new(clock.clone())))
}

/// Escribe un WAV PCM mono de 16 bits y devuelve la duración declarada.
fn write_wav_file(path: &Path, sample_rate: u32, seconds: f64) -> f64 {
    let frames = (sample_rate as f64 * seconds).round() as u32;
    let data_len = (frames as usize) * 2;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for i in 0..frames {
        let value = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sample_rate as f64).sin();
        bytes.extend_from_slice(&((value * 0.3 * i16::MAX as f64) as i16).to_le_bytes());
    }
    std::fs::write(path, bytes).expect("write wav");
    frames as f64 / sample_rate as f64
}

/// Prefijo de un WAV cuya cabecera declara `declared_len` bytes de datos pero
/// cuyo cuerpo mide `actual_len` (el parcial que el servidor falso envía).
fn wav_prefix(sample_rate: u32, declared_len: u64, actual_len: u64) -> Vec<u8> {
    let data_len = declared_len - 44;
    let mut bytes = Vec::with_capacity(actual_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(actual_len as usize, 0);
    bytes
}

fn marker_path_of(cache_path: &Path) -> std::path::PathBuf {
    cache_path.with_file_name(format!(
        "{}.done",
        cache_path.file_name().unwrap().to_string_lossy()
    ))
}

fn sample_info() -> StreamSourceInfo {
    StreamSourceInfo {
        service_name: "qobuz".to_string(),
        service_track_id: "1".to_string(),
        format: "FLAC".to_string(),
        codec: Some("flac".to_string()),
        container: Some("FLAC".to_string()),
        extension: "flac".to_string(),
        bit_depth: Some(24),
        sample_rate: Some(192000),
        quality_label: "FLAC 24 bits / 192 kHz".to_string(),
        quality_tier: "hires".to_string(),
        source_name: "Qobuz API oficial".to_string(),
        match_method: "direct".to_string(),
    }
}

async fn test_db() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

async fn insert_track(db: &sqlx::SqlitePool, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(db)
        .await
        .unwrap()
}

async fn insert_download(db: &sqlx::SqlitePool, track_id: i64, file_path: &Path) {
    sqlx::query("INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'WAV')")
        .bind(track_id)
        .bind(file_path.to_string_lossy().to_string())
        .execute(db)
        .await
        .unwrap();
}

async fn insert_qobuz_source(db: &sqlx::SqlitePool, track_id: i64) {
    sqlx::query(
        "INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) \
         VALUES (2, 'qobuz', 1, 'hires')",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, available) \
         VALUES (?, 2, '123', 1)",
    )
    .bind(track_id)
    .execute(db)
    .await
    .unwrap();
}

/// App de prueba con `PlayerState` (motor simulado ya inyectado),
/// `StreamCacheState` y `AppState` (BD migrada en memoria).
fn mock_player_app(db: sqlx::SqlitePool, clock: &VirtualClock) -> tauri::App<MockRuntime> {
    let app = tauri::test::mock_app();
    let state = PlayerState::default();
    {
        let mut core = lock_core(&state).expect("player core");
        core.engine = Some(sim_engine(clock));
    }
    app.manage(state);
    app.manage(StreamCacheState::default());
    app.manage(crate::AppState {
        db,
        worker_state: crate::worker::DownloadWorkerState::new(2),
        enrichment_state: crate::enrichment_worker::EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(crate::services::ConcurrencyManager::new()),
    });
    app
}

fn core_mut<R>(app: &tauri::App<MockRuntime>, f: impl FnOnce(&mut PlayerCore) -> R) -> R {
    let state = app.state::<PlayerState>();
    let mut core = lock_core(&state).expect("player core");
    f(&mut core)
}

async fn load_track(
    app: &tauri::App<MockRuntime>,
    track_id: i64,
    autoplay: bool,
    volume: f64,
) -> Result<PlayerLoadResult, String> {
    player_load_track(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        app.state::<crate::AppState>(),
        track_id,
        autoplay,
        volume,
    )
    .await
}

fn engine_position(app: &tauri::App<MockRuntime>) -> Duration {
    core_mut(app, |core| core.engine.as_ref().expect("engine").position())
}

fn engine_state(app: &tauri::App<MockRuntime>) -> EngineState {
    core_mut(app, |core| core.engine.as_ref().expect("engine").state())
}

fn engine_loaded_path(app: &tauri::App<MockRuntime>) -> Option<std::path::PathBuf> {
    core_mut(app, |core| {
        core.engine
            .as_ref()
            .expect("engine")
            .loaded()
            .map(|l| l.path.clone())
    })
}

/// Registra un oyente de `player:stream-state` y devuelve los payloads crudos.
fn capture_stream_state(app: &tauri::App<MockRuntime>) -> Arc<std::sync::Mutex<Vec<String>>> {
    let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = captured.clone();
    app.listen("player:stream-state", move |event| {
        sink.lock().unwrap().push(event.payload().to_string());
    });
    captured
}

/// Repite una comprobación hasta 3 s antes de rendirse.
async fn wait_until(mut condition: impl FnMut() -> bool, what: &str) {
    for _ in 0..150 {
        if condition() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condición no alcanzada a tiempo: {what}");
}

// ---------------------------------------------------------------------------
// Servidor HTTP falso local (patrón de tests/songlink_native_engines_test.rs)
// ---------------------------------------------------------------------------

enum ServerStep {
    /// Enviar los siguientes N bytes del cuerpo.
    Send(usize),
    /// Enviar N bytes con codificación chunked (sin Content-Length).
    Chunk(usize),
    /// Barrera: esperar a que el test libere el paso.
    Wait(tokio::sync::oneshot::Receiver<()>),
    /// Cortar la conexión a lo bruto (error de red para el cliente).
    Abort,
    /// Terminar el cuerpo (chunk terminal si es chunked) y cerrar.
    Finish,
}

/// Sirve un cuerpo en 127.0.0.1 y devuelve (url, peticiones recibidas).
async fn spawn_fake_stream_server(
    content_length: Option<u64>,
    body: Vec<u8>,
    steps: Vec<ServerStep>,
) -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(AtomicUsize::new(0));
    let counted = requests.clone();
    tokio::spawn(async move {
        let mut pending_steps = Some(steps);
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            counted.fetch_add(1, Ordering::SeqCst);
            let steps = pending_steps.take().unwrap_or_default();
            serve_one(&mut stream, content_length, &body, steps).await;
        }
    });
    (format!("http://{addr}/stream"), requests)
}

async fn serve_one(
    stream: &mut tokio::net::TcpStream,
    content_length: Option<u64>,
    body: &[u8],
    steps: Vec<ServerStep>,
) {
    // Leer la petición hasta el final de las cabeceras.
    let mut buf = [0u8; 4096];
    let mut seen = 0usize;
    loop {
        let n = stream.read(&mut buf[seen..]).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        seen += n;
        if buf[..seen].windows(4).any(|w| w == b"\r\n\r\n") || seen == buf.len() {
            break;
        }
    }
    let mut head = String::from("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n");
    match content_length {
        Some(len) => head.push_str(&format!("Content-Length: {len}\r\n")),
        None => head.push_str("Transfer-Encoding: chunked\r\n"),
    }
    head.push_str("\r\n");
    if stream.write_all(head.as_bytes()).await.is_err() {
        return;
    }

    let mut offset = 0usize;
    for step in steps {
        match step {
            ServerStep::Send(n) => {
                let end = (offset + n).min(body.len());
                if stream.write_all(&body[offset..end]).await.is_err() {
                    return;
                }
                offset = end;
            }
            ServerStep::Chunk(n) => {
                let end = (offset + n).min(body.len());
                let slice = &body[offset..end];
                let frame = format!("{:x}\r\n", slice.len());
                if stream.write_all(frame.as_bytes()).await.is_err()
                    || stream.write_all(slice).await.is_err()
                    || stream.write_all(b"\r\n").await.is_err()
                {
                    return;
                }
                offset = end;
            }
            ServerStep::Wait(rx) => {
                let _ = rx.await;
            }
            ServerStep::Abort => return,
            ServerStep::Finish => {
                if content_length.is_none() {
                    let _ = stream.write_all(b"0\r\n\r\n").await;
                }
                return;
            }
        }
    }
    // Sin más pasos: mantener la conexión abierta (descarga estancada), como
    // un servidor que deja de enviar sin cerrar.
    std::future::pending::<()>().await;
}

struct RunningDownload {
    session: Arc<StreamSession>,
    ready: tokio::sync::oneshot::Receiver<Result<Option<f64>, String>>,
    requests: Arc<AtomicUsize>,
    cache_path: std::path::PathBuf,
    /// Mantiene vivo el directorio temporal hasta el final del test.
    _dir: tempfile::TempDir,
}

/// Instala la sesión (seq 1 = generación vigente) y lanza
/// `spawn_stream_download` contra el servidor falso, como hace el comando.
async fn spawn_download(
    app: &tauri::App<MockRuntime>,
    content_length: Option<u64>,
    body: Vec<u8>,
    steps: Vec<ServerStep>,
) -> RunningDownload {
    core_mut(app, |core| core.seq = 1);
    let dir = tempfile::tempdir().unwrap();
    let cache_path = dir.path().join("track-1-qobuz-24bit-192000hz.flac");
    let session = StreamSession::new(1, 1, sample_info(), cache_path.clone());
    app.state::<StreamCacheState>().install(session.clone());
    let (url, requests) = spawn_fake_stream_server(content_length, body, steps).await;
    let http = reqwest::Client::new();
    let response = http.get(&url).send().await.expect("GET al servidor falso");
    let (ready_tx, ready) = tokio::sync::oneshot::channel();
    spawn_stream_download(
        app.handle().clone(),
        session.clone(),
        response,
        false,
        1.0,
        ready_tx,
    );
    RunningDownload {
        session,
        ready,
        requests,
        cache_path,
        _dir: dir,
    }
}

// ---------------------------------------------------------------------------
// 1. Orquestación offline de player_load_track
// ---------------------------------------------------------------------------

#[tokio::test]
async fn local_downloaded_file_loads_offline_without_stream_and_bumps_seq() {
    let clock = VirtualClock::new();
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("pista.wav");
    let duration = write_wav_file(&wav, WAV_SAMPLE_RATE, 20.0);
    let db = test_db().await;
    let track_id = insert_track(&db, "Pista local").await;
    insert_download(&db, track_id, &wav).await;
    let app = mock_player_app(db, &clock);

    let result = load_track(&app, track_id, false, 0.8)
        .await
        .expect("carga local");

    assert_eq!(result.seq, 1, "primera generación tras la carga");
    assert!(result.stream.is_none(), "carga local sin info de streaming");
    let loaded_duration = result.duration_sec.expect("duración del WAV");
    assert!((loaded_duration - duration).abs() < 1e-6);

    // El motor quedó cargado con el fichero local, pausado (autoplay = false)
    // y con el volumen pedido.
    assert_eq!(engine_state(&app), EngineState::Paused);
    assert_eq!(engine_loaded_path(&app), Some(wav));
    assert_eq!(engine_position(&app), Duration::ZERO);
    let volume = core_mut(&app, |core| core.engine.as_ref().unwrap().volume());
    assert_eq!(volume, 0.8);
    // Sin sesión de streaming activa.
    assert!(app.state::<StreamCacheState>().current().is_none());
}

#[tokio::test]
async fn second_local_load_cancels_previous_stream_session_and_bumps_generation() {
    let clock = VirtualClock::new();
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("pista.wav");
    write_wav_file(&wav, WAV_SAMPLE_RATE, 20.0);
    let db = test_db().await;
    let track_id = insert_track(&db, "Pista local").await;
    insert_download(&db, track_id, &wav).await;
    let app = mock_player_app(db, &clock);

    // Sesión de streaming de la generación 0, como si la pista anterior
    // viniera de resolver por red.
    let session = StreamSession::new(0, 999, sample_info(), dir.path().join("cache.flac"));
    app.state::<StreamCacheState>().install(session.clone());

    let result = load_track(&app, track_id, true, 1.0)
        .await
        .expect("carga local");

    assert_eq!(result.seq, 1);
    assert!(result.stream.is_none());
    // La sesión anterior se canceló y quedó inerte: for_seq la filtra.
    assert!(session.is_canceled());
    assert!(app.state::<StreamCacheState>().current().is_none());
    assert!(app.state::<StreamCacheState>().for_seq(0).is_none());
    // Y el motor fue sobrescrito por la carga nueva.
    assert_eq!(engine_state(&app), EngineState::Playing);
    assert_eq!(engine_loaded_path(&app), Some(wav));
}

#[tokio::test]
async fn deleted_local_file_is_not_taken_as_local_and_falls_to_resolver() {
    let clock = VirtualClock::new();
    let db = test_db().await;
    let track_id = insert_track(&db, "Pista borrada").await;
    insert_download(&db, track_id, Path::new("/no/existe/pista.flac")).await;
    let app = mock_player_app(db, &clock);

    let err = load_track(&app, track_id, true, 1.0)
        .await
        .expect_err("sin fichero local y sin fuentes");

    // Sin candidatas la cascada ISRC sigue evaluándose y su fallo entra en el
    // agregado (mensaje real congelado).
    assert!(
        err.starts_with("No se pudo resolver ninguna fuente reproducible"),
        "agregado del resolver sin candidatas: {err}"
    );
    assert!(err.contains("cascada:"), "motivo de la cascada: {err}");
    // Nada se cargó ni se publicó generación nueva.
    assert_eq!(core_mut(&app, |core| core.seq), 0);
    assert_eq!(engine_loaded_path(&app), None);
    assert!(app.state::<StreamCacheState>().current().is_none());
}

#[tokio::test]
async fn resolver_error_is_aggregated_without_network_when_a_qobuz_source_has_no_account() {
    let clock = VirtualClock::new();
    let db = test_db().await;
    let track_id = insert_track(&db, "Pista sin cuenta").await;
    insert_qobuz_source(&db, track_id).await;
    let app = mock_player_app(db, &clock);

    let err = load_track(&app, track_id, true, 1.0)
        .await
        .expect_err("cuenta Qobuz inexistente: el resolver falla sin red");

    // Error agregado: la candidata qobuz falló (sin cuentas) y la cascada ISRC
    // no puede adivinar sin ISRC ni Spotify.
    assert!(
        err.starts_with("No se pudo resolver ninguna fuente reproducible"),
        "agregado del resolver: {err}"
    );
    assert!(err.contains("qobuz:"), "motivo por candidata: {err}");
    assert_eq!(core_mut(&app, |core| core.seq), 0);
    assert!(engine_loaded_path(&app).is_none());
    assert!(app.state::<StreamCacheState>().current().is_none());
}

#[tokio::test]
async fn engine_is_only_overwritten_by_the_current_generation() {
    let clock = VirtualClock::new();
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("primera.wav");
    let second = dir.path().join("segunda.wav");
    write_wav_file(&first, WAV_SAMPLE_RATE, 20.0);
    let second_duration = write_wav_file(&second, WAV_SAMPLE_RATE, 30.0);
    let db = test_db().await;
    let app = mock_player_app(db, &clock);

    // Generación vigente: 2, con la primera pista cargada.
    core_mut(&app, |core| {
        core.seq = 2;
        load_path_into_core(core, &first, false, 1.0).expect("carga de la pista vigente");
    });
    assert_eq!(engine_loaded_path(&app), Some(first.clone()));

    // Una sesión de una generación superada no puede tocar el motor.
    let stale = StreamSession::new(1, 7, sample_info(), second.clone());
    let err =
        load_stream_into_engine(app.handle(), &stale, true, 1.0).expect_err("generación superada");
    assert_eq!(err, "carga reemplazada por otra pista");
    assert!(!stale.started.load(Ordering::SeqCst));
    assert_eq!(engine_loaded_path(&app), Some(first.clone()));

    // La generación vigente sí.
    let current = StreamSession::new(2, 7, sample_info(), second.clone());
    let loaded = load_stream_into_engine(app.handle(), &current, true, 1.0)
        .expect("carga de la generación vigente");
    assert_eq!(loaded, Some(second_duration));
    assert!(current.started.load(Ordering::SeqCst));
    assert_eq!(engine_loaded_path(&app), Some(second));
}

#[tokio::test]
async fn complete_cache_marker_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("track-1-qobuz-24bit-192000hz.flac");
    std::fs::write(&path, vec![0u8; 1024]).unwrap();
    let session = StreamSession::new(1, 1, sample_info(), path.clone());
    session.downloaded_bytes.store(1024, Ordering::SeqCst);

    stream_cache::write_done_marker(&session).expect("marcador");

    // Tamaño exacto + marcador: caché completa reutilizable.
    let info = stream_cache::read_complete_cache(&path).expect("caché completa");
    assert_eq!(
        serde_json::to_value(&info).unwrap(),
        serde_json::to_value(sample_info()).unwrap()
    );

    // Un parcial (tamaño distinto del marcador) NUNCA se reutiliza.
    let mut partial = std::fs::read(&path).unwrap();
    partial.push(0);
    std::fs::write(&path, partial).unwrap();
    assert!(stream_cache::read_complete_cache(&path).is_none());

    // Sin marcador no hay caché completa, aunque el tamaño coincida.
    let other = dir.path().join("sin-marcador.flac");
    std::fs::write(&other, vec![0u8; 1024]).unwrap();
    assert!(stream_cache::read_complete_cache(&other).is_none());
}

#[test]
fn prime_and_frontier_constants_stay_frozen() {
    // Ventana de prime: 45 s (no se acorta: es la paciencia del buffer inicial).
    assert_eq!(STREAM_PRIME_TIMEOUT, Duration::from_secs(45));
    // Objetivo de prime en bytes: piso 256 KiB, techo 8 MiB, fallback 4 MiB.
    assert_eq!(STREAM_MIN_PRIME_BYTES, 256 * 1024);
    assert_eq!(STREAM_MAX_PRIME_BYTES, 8 * 1024 * 1024);
    assert_eq!(STREAM_FALLBACK_PRIME_BYTES, 4 * 1024 * 1024);
    // Márgenes de frontera: pausa a 5 s del borde, reanuda con +10 s.
    assert_eq!(FRONTIER_PAUSE_MARGIN_SECS, 5.0);
    assert_eq!(FRONTIER_RESUME_MARGIN_SECS, 10.0);
}

// ---------------------------------------------------------------------------
// 2. Frontera de streaming y seeks pendientes
// ---------------------------------------------------------------------------

/// Escenario frontera: pista de 20 s en el motor, sesión con frontera por
/// bytes a 12.0 s (600 de 1000 bytes) de la generación vigente.
struct FrontierBench {
    app: tauri::App<MockRuntime>,
    session: Arc<StreamSession>,
    clock: VirtualClock,
    _dir: tempfile::TempDir,
}

async fn frontier_bench(autoplay: bool) -> FrontierBench {
    let clock = VirtualClock::new();
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("pista.wav");
    write_wav_file(&wav, WAV_SAMPLE_RATE, 20.0);
    let db = test_db().await;
    let track_id = insert_track(&db, "Pista streaming").await;
    let app = mock_player_app(db, &clock);
    core_mut(&app, |core| {
        load_path_into_core(core, &wav, autoplay, 1.0).expect("motor cargado");
    });
    let session = StreamSession::new(0, track_id, sample_info(), dir.path().join("cache.flac"));
    session.total_bytes.store(1000, Ordering::SeqCst);
    session.downloaded_bytes.store(600, Ordering::SeqCst);
    app.state::<StreamCacheState>().install(session.clone());
    FrontierBench {
        app,
        session,
        clock,
        _dir: dir,
    }
}

impl FrontierBench {
    /// Un paso del ticker sobre la sesión de la generación vigente.
    fn guard(&self) {
        let session = self
            .app
            .state::<StreamCacheState>()
            .for_seq(0)
            .expect("sesión vigente");
        let mut dirty = false;
        let state = self.app.state::<PlayerState>();
        let mut core = lock_core(&state).unwrap();
        let engine = core.engine.as_mut().unwrap();
        guard_stream_frontier(self.app.handle(), engine, &session, &mut dirty);
    }

    fn set_frontier(&self, downloaded: u64) {
        self.session
            .downloaded_bytes
            .store(downloaded, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn seek_within_frontier_goes_straight_to_engine() {
    let bench = frontier_bench(true).await;
    let app = &bench.app;

    // Frontera 12.0: el gate es `sec > frontier - 1.0`, así que 11.0 va directo.
    player_seek(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        11.0,
    )
    .expect("seek directo");
    assert_eq!(engine_position(app), Duration::from_secs_f64(11.0));
    assert_eq!(engine_state(app), EngineState::Playing);
    assert_eq!(bench.session.take_pending_seek(), None);
    assert!(!bench.session.paused_for_buffer.load(Ordering::SeqCst));

    // Y un seek cualquiera por debajo de la frontera, igual.
    player_seek(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        3.0,
    )
    .expect("seek directo");
    assert_eq!(engine_position(app), Duration::from_secs_f64(3.0));
}

#[tokio::test]
async fn seek_beyond_frontier_is_gated_into_pending_pause() {
    let bench = frontier_bench(true).await;
    let app = &bench.app;

    // 11.5 > 12.0 - 1.0: no toca el motor, lo aparca y pausa.
    player_seek(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        11.5,
    )
    .expect("seek aparcado");

    assert_eq!(bench.session.take_pending_seek(), Some(11.5));
    assert!(
        bench.session.take_resume_after_seek(),
        "el usuario buscaba sonando"
    );
    assert!(bench.session.paused_for_buffer.load(Ordering::SeqCst));
    // El motor no se movió: quedó pausado donde estaba.
    assert_eq!(engine_state(app), EngineState::Paused);
    assert_eq!(engine_position(app), Duration::ZERO);
}

#[tokio::test]
async fn ticker_applies_pending_seek_when_frontier_reaches_target_plus_three() {
    let bench = frontier_bench(true).await;
    let app = &bench.app;
    player_seek(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        11.5,
    )
    .expect("seek aparcado");

    // Frontera 12.0 < 11.5 + 3.0: sigue aparcado y pausado por buffer.
    bench.guard();
    assert_eq!(engine_state(app), EngineState::Paused);
    assert_eq!(engine_position(app), Duration::ZERO);
    assert!(bench.session.paused_for_buffer.load(Ordering::SeqCst));

    // Margen congelado: destino + 3.0 = 14.5; la frontera lo alcanza y el
    // seek se aplica, con reanudación (el usuario buscaba sonando).
    bench.set_frontier(725);
    bench.guard();
    assert_eq!(engine_position(app), Duration::from_secs_f64(11.5));
    assert_eq!(engine_state(app), EngineState::Playing);
    assert_eq!(bench.session.take_pending_seek(), None);
    assert!(!bench.session.paused_for_buffer.load(Ordering::SeqCst));
}

#[tokio::test]
async fn pending_seek_stays_parked_below_target_plus_three() {
    let bench = frontier_bench(true).await;
    let app = &bench.app;
    player_seek(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        11.5,
    )
    .expect("seek aparcado");

    // 14.4 < 11.5 + 3.0: el destino no es alcanzable y el seek vuelve a la cola.
    bench.set_frontier(722);
    bench.guard();
    assert_eq!(engine_state(app), EngineState::Paused);
    assert_eq!(engine_position(app), Duration::ZERO);
    assert_eq!(
        bench.session.take_pending_seek(),
        Some(11.5),
        "sigue pendiente"
    );
}

#[tokio::test]
async fn pending_seek_does_not_resume_if_user_sought_while_paused() {
    let bench = frontier_bench(false).await; // restauración: estaba pausada
    let app = &bench.app;
    player_seek(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        11.5,
    )
    .expect("seek aparcado");
    assert!(
        !bench.session.take_resume_after_seek(),
        "buscaba en pausa: no hay reanudación pendiente"
    );

    bench.set_frontier(900);
    bench.guard();
    assert_eq!(engine_position(app), Duration::from_secs_f64(11.5));
    assert_eq!(engine_state(app), EngineState::Paused, "no se reanuda sola");
}

#[tokio::test]
async fn guard_pauses_playback_exactly_five_seconds_before_frontier() {
    let bench = frontier_bench(true).await;
    let app = &bench.app;

    // Posición 6.9: aún fuera del margen (frontera 12.0 - 5.0 = 7.0).
    bench.clock.advance(Duration::from_secs_f64(6.9));
    bench.guard();
    assert_eq!(engine_state(app), EngineState::Playing);
    assert!(!bench.session.paused_for_buffer.load(Ordering::SeqCst));

    // Posición 7.0: pausa por buffer exactamente en el margen de 5 s.
    bench.clock.advance(Duration::from_secs_f64(0.1));
    bench.guard();
    assert_eq!(engine_state(app), EngineState::Paused);
    assert!(bench.session.paused_for_buffer.load(Ordering::SeqCst));
}

#[tokio::test]
async fn guard_resumes_when_frontier_passes_position_plus_ten() {
    let bench = frontier_bench(true).await;
    let app = &bench.app;
    bench.clock.advance(Duration::from_secs_f64(7.0));
    bench.guard();
    assert_eq!(engine_state(app), EngineState::Paused);

    // Reanudar exige frontera >= posición + 10.0 (17.0); con 16.9 no.
    bench.set_frontier(845);
    bench.guard();
    assert_eq!(engine_state(app), EngineState::Paused);

    bench.set_frontier(850);
    bench.guard();
    assert_eq!(engine_state(app), EngineState::Playing);
    assert!(!bench.session.paused_for_buffer.load(Ordering::SeqCst));
}

#[tokio::test]
async fn guard_is_inert_for_complete_or_canceled_sessions() {
    let clock = VirtualClock::new();
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("pista.wav");
    write_wav_file(&wav, WAV_SAMPLE_RATE, 20.0);
    let db = test_db().await;
    let track_id = insert_track(&db, "Pista streaming").await;
    let app = mock_player_app(db, &clock);
    core_mut(&app, |core| {
        load_path_into_core(core, &wav, true, 1.0).expect("motor cargado");
    });

    let install = |complete: bool, canceled: bool| {
        let session = StreamSession::new(0, track_id, sample_info(), dir.path().join("c.flac"));
        session.total_bytes.store(1000, Ordering::SeqCst);
        session.downloaded_bytes.store(950, Ordering::SeqCst);
        if complete {
            session.complete.store(true, Ordering::SeqCst);
        }
        if canceled {
            session.canceled.store(true, Ordering::SeqCst);
        }
        session
    };
    for session in [install(true, false), install(false, true)] {
        app.state::<StreamCacheState>().install(session.clone());
        // El guard del ticker toma el lock del core; al terminar el paso,
        // el guard debe estar liberado para la lectura de estado.
        {
            let session = app.state::<StreamCacheState>().current().unwrap();
            let mut dirty = false;
            let state = app.state::<PlayerState>();
            let mut core = lock_core(&state).unwrap();
            let engine = core.engine.as_mut().unwrap();
            guard_stream_frontier(app.handle(), engine, &session, &mut dirty);
        }
        assert_eq!(engine_state(&app), EngineState::Playing);
        assert!(!session.paused_for_buffer.load(Ordering::SeqCst));
    }
}

// ---------------------------------------------------------------------------
// 3. Descarga progresiva contra servidor falso local
// ---------------------------------------------------------------------------

#[tokio::test]
async fn prime_fires_at_the_256k_floor_not_before() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    // Total declarado 2 MiB: total/10 = 204.8 KiB < 256 KiB, manda el piso.
    let body = wav_prefix(WAV_SAMPLE_RATE, 2 * 1024 * 1024, 320 * 1024);
    let mut download = spawn_download(
        &app,
        Some(2 * 1024 * 1024),
        body,
        vec![
            ServerStep::Send(128 * 1024),
            ServerStep::Wait(gate_rx),
            ServerStep::Send(192 * 1024),
        ],
    )
    .await;

    // Con 128 KiB descargados el prime no se dispara: el piso es 256 KiB.
    let not_yet = tokio::time::timeout(Duration::from_millis(300), &mut download.ready).await;
    assert!(
        not_yet.is_err(),
        "el prime no debe dispararse antes del piso"
    );
    assert!(!download.session.started.load(Ordering::SeqCst));

    // Al superar 256 KiB, el motor se abre y responde por `ready`.
    drop(gate_tx);
    let duration = tokio::time::timeout(Duration::from_secs(5), download.ready)
        .await
        .expect("prime en 5 s")
        .expect("oneshot vivo")
        .expect("carga en el motor ok");
    let expected = (2.0 * 1024.0 * 1024.0 - 44.0) / 2.0 / WAV_SAMPLE_RATE as f64;
    let duration = duration.expect("duración del WAV parcial");
    assert!((duration - expected).abs() < 1e-6);
    assert!(download.session.started.load(Ordering::SeqCst));
    assert_eq!(engine_loaded_path(&app), Some(download.cache_path.clone()));
    assert!(download.session.downloaded_bytes.load(Ordering::SeqCst) >= 256 * 1024);
}

#[tokio::test]
async fn prime_caps_at_8mib_for_large_files() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    // Total 100 MiB: total/10 = 10 MiB > techo, manda el techo de 8 MiB.
    let body = wav_prefix(
        WAV_SAMPLE_RATE,
        100 * 1024 * 1024,
        8 * 1024 * 1024 + 64 * 1024,
    );
    let mut download = spawn_download(
        &app,
        Some(100 * 1024 * 1024),
        body,
        vec![
            ServerStep::Send(8 * 1024 * 1024 - 1),
            ServerStep::Wait(gate_rx),
            ServerStep::Send(64 * 1024),
        ],
    )
    .await;

    // A 8 MiB - 1 todavía no: el techo es exactamente 8 MiB.
    let not_yet = tokio::time::timeout(Duration::from_millis(300), &mut download.ready).await;
    assert!(not_yet.is_err(), "el techo de 8 MiB debe respetarse");

    drop(gate_tx);
    let duration = tokio::time::timeout(Duration::from_secs(5), download.ready)
        .await
        .expect("prime en 5 s")
        .expect("oneshot vivo")
        .expect("carga en el motor ok");
    assert!(duration.is_some());
    assert_eq!(engine_loaded_path(&app), Some(download.cache_path.clone()));
}

#[tokio::test]
async fn prime_falls_back_to_4mib_without_content_length() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let body = wav_prefix(WAV_SAMPLE_RATE, 10 * 1024 * 1024, 4 * 1024 * 1024);
    let download = spawn_download(
        &app,
        None,
        body,
        vec![
            ServerStep::Chunk(2 * 1024 * 1024),
            ServerStep::Wait(gate_rx),
            ServerStep::Chunk(2 * 1024 * 1024),
            ServerStep::Finish,
        ],
    )
    .await;

    // 2 MiB descargados sin Content-Length: el objetivo es el fallback de 4 MiB.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !download.session.started.load(Ordering::SeqCst),
        "el fallback de 4 MiB debe respetarse"
    );

    // Al llegar a 4 MiB se abre el motor; el cierre ordenado del cuerpo
    // completa la descarga y escribe el marcador.
    drop(gate_tx);
    // El marcador se escribe al cerrar el cuerpo: su presencia acota la carrera.
    wait_until(
        || marker_path_of(&download.cache_path).exists(),
        "marcador .done escrito",
    )
    .await;
    assert!(download.session.complete.load(Ordering::SeqCst));
    assert!(download.session.started.load(Ordering::SeqCst));
    assert_eq!(
        download.session.downloaded_bytes.load(Ordering::SeqCst),
        4 * 1024 * 1024
    );
    assert!(
        stream_cache::read_complete_cache(&download.cache_path).is_some(),
        "la caché completada es reutilizable"
    );
}

#[tokio::test]
async fn completed_download_primes_at_eof_and_writes_reusable_marker() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    // Fichero pequeño: llega completo antes del objetivo de prime.
    let body = wav_prefix(WAV_SAMPLE_RATE, 100 * 1024, 100 * 1024);
    let download = spawn_download(
        &app,
        Some(100 * 1024),
        body,
        vec![ServerStep::Send(100 * 1024), ServerStep::Finish],
    )
    .await;

    let duration = tokio::time::timeout(Duration::from_secs(5), download.ready)
        .await
        .expect("prime en 5 s")
        .expect("oneshot vivo")
        .expect("carga en el motor ok");
    let expected = (100.0 * 1024.0 - 44.0) / 2.0 / WAV_SAMPLE_RATE as f64;
    assert!((duration.expect("duración") - expected).abs() < 1e-6);
    wait_until(
        || marker_path_of(&download.cache_path).exists(),
        "marcador .done escrito",
    )
    .await;
    assert!(download.session.complete.load(Ordering::SeqCst));
    assert_eq!(
        download.session.downloaded_bytes.load(Ordering::SeqCst),
        100 * 1024
    );
    assert!(
        stream_cache::read_complete_cache(&download.cache_path).is_some(),
        "el marcador deja la caché reutilizable"
    );
    assert_eq!(engine_loaded_path(&app), Some(download.cache_path.clone()));
}

#[tokio::test]
async fn cancel_after_prime_deletes_partial_and_marks_session() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let body = wav_prefix(WAV_SAMPLE_RATE, 2 * 1024 * 1024, 384 * 1024);
    let mut download = spawn_download(
        &app,
        Some(2 * 1024 * 1024),
        body,
        vec![
            ServerStep::Send(320 * 1024),
            ServerStep::Wait(gate_rx),
            ServerStep::Send(64 * 1024),
        ],
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), &mut download.ready)
        .await
        .expect("prime en 5 s")
        .expect("oneshot vivo")
        .expect("carga ok");

    // Reemplazo a mitad de descarga: la sesión se cancela y el parcial muere.
    app.state::<StreamCacheState>().cancel_current();
    drop(gate_tx);
    wait_until(
        || !download.cache_path.exists(),
        "parcial borrado tras cancelar",
    )
    .await;
    assert!(download.session.is_canceled());
    assert!(!download.session.complete.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancel_before_prime_answers_replacement_error_and_cleans_partial() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let body = wav_prefix(WAV_SAMPLE_RATE, 2 * 1024 * 1024, 128 * 1024);
    let download = spawn_download(
        &app,
        Some(2 * 1024 * 1024),
        body,
        vec![
            ServerStep::Send(64 * 1024),
            ServerStep::Wait(gate_rx),
            ServerStep::Send(64 * 1024),
        ],
    )
    .await;

    app.state::<StreamCacheState>().cancel_current();
    drop(gate_tx);
    let outcome = tokio::time::timeout(Duration::from_secs(5), download.ready)
        .await
        .expect("respuesta en 5 s")
        .expect("oneshot vivo");
    assert_eq!(
        outcome,
        Err("carga reemplazada".to_string()),
        "la carga reemplazada termina en Err"
    );
    wait_until(
        || !download.cache_path.exists(),
        "parcial borrado tras cancelar",
    )
    .await;
    assert!(download.session.is_canceled());
}

#[tokio::test]
async fn network_error_midway_reports_error_state_without_retry() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    let events = capture_stream_state(&app);
    let (gate_tx, gate_rx) = tokio::sync::oneshot::channel::<()>();
    let body = wav_prefix(WAV_SAMPLE_RATE, 2 * 1024 * 1024, 320 * 1024);
    let mut download = spawn_download(
        &app,
        Some(2 * 1024 * 1024),
        body,
        vec![
            ServerStep::Send(320 * 1024),
            ServerStep::Wait(gate_rx),
            ServerStep::Abort,
        ],
    )
    .await;

    tokio::time::timeout(Duration::from_secs(5), &mut download.ready)
        .await
        .expect("prime en 5 s")
        .expect("oneshot vivo")
        .expect("carga ok");

    drop(gate_tx);
    wait_until(
        || {
            events
                .lock()
                .unwrap()
                .iter()
                .any(|payload| payload.contains("Se interrumpió la descarga"))
        },
        "estado de error emitido",
    )
    .await;

    // Sin re-resolución: una sola petición HTTP, sin reintento, y el parcial
    // se conserva sin marcador (la re-resolución llega en la Fase C).
    assert_eq!(download.requests.load(Ordering::SeqCst), 1);
    assert!(!download.session.complete.load(Ordering::SeqCst));
    assert!(download.cache_path.exists());
    assert!(!marker_path_of(&download.cache_path).exists());
}

#[tokio::test]
async fn unwritable_cache_path_reports_error_before_reading_the_stream() {
    let app = mock_player_app(test_db().await, &VirtualClock::new());
    let events = capture_stream_state(&app);
    core_mut(&app, |core| core.seq = 1);
    let dir = tempfile::tempdir().unwrap();
    let session = StreamSession::new(
        1,
        1,
        sample_info(),
        dir.path().join("no/existe/track-1.flac"),
    );
    app.state::<StreamCacheState>().install(session.clone());
    let (url, _requests) = spawn_fake_stream_server(Some(1024), vec![0u8; 1024], Vec::new()).await;
    let http = reqwest::Client::new();
    let response = http.get(&url).send().await.unwrap();
    let (ready_tx, ready) = tokio::sync::oneshot::channel();
    spawn_stream_download(
        app.handle().clone(),
        session.clone(),
        response,
        false,
        1.0,
        ready_tx,
    );

    let outcome = tokio::time::timeout(Duration::from_secs(5), ready)
        .await
        .expect("respuesta en 5 s")
        .expect("oneshot vivo");
    assert!(
        outcome
            .as_ref()
            .err()
            .is_some_and(|msg| msg.contains("No se pudo crear la caché")),
        "error de creación de caché: {outcome:?}"
    );
    wait_until(
        || {
            events
                .lock()
                .unwrap()
                .iter()
                .any(|payload| payload.contains("No se pudo crear la caché"))
        },
        "error emitido",
    )
    .await;
}
