//! #132 (P01, parte de #130) — avance automático y crédito de escucha en el
//! backend, con el motor simulado de `syncify_player::sim` (reloj virtual:
//! ninguna prueba espera audio real) y BD `:memory:` migrada.
//!
//! Congela:
//!   1. El fin natural de una pista dispara el avance en el backend: suena la
//!      siguiente sin intervención de la UI, el agregado transiciona con los
//!      `perform_*` reales y `player:advanced` sale con la generación nueva.
//!   2. repeat one (repite la actual, sin duplicar historial), repeat all
//!      (reconstruye el ciclo completo), repeat off con cola vacía (no avanza)
//!      y el guard de generación (una carga más nueva retira el avance).
//!   3. El crédito de escucha: Rust acumula 30 s REALES por el motor (pausa
//!      suspende sin borrar, seek re-ancla sin acreditar el salto, la
//!      velocidad divide) y acredita UNA vez por sesión; la nueva pista abre
//!      sesión nueva.
//!   4. El volumen del avance es el estado persistido (silencio → 0).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use syncify_player::sim::{SimHandle, SimulatedBackend, VirtualClock};
use syncify_player::PlayerEngine;
use tauri::{Listener, Manager};

use super::run_auto_advance;
use crate::commands::playback::perform_record_local_play;
use crate::commands::playback_queue::{
    perform_enqueue, perform_get_playback_queue, perform_set_current, perform_set_modes,
    perform_set_volume,
};
use crate::commands::player::{
    player_load_track, player_pause, player_resume, player_seek, player_set_rate,
};
use crate::services::playback_session::{lock_core, PlayerState, LISTEN_THRESHOLD_SEC};
use crate::services::stream_cache::StreamCacheState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WAV_SAMPLE_RATE: u32 = 8000;

/// Escribe un WAV PCM mono de 16 bits (copiado del patrón de
/// `commands/player_tests.rs`) y devuelve la duración declarada.
fn write_wav_file(path: &Path, seconds: f64) -> f64 {
    let frames = (WAV_SAMPLE_RATE as f64 * seconds).round() as u32;
    let data_len = (frames as usize) * 2;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&WAV_SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(WAV_SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for i in 0..frames {
        let value = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / WAV_SAMPLE_RATE as f64).sin();
        bytes.extend_from_slice(&((value * 0.3 * i16::MAX as f64) as i16).to_le_bytes());
    }
    std::fs::write(path, bytes).expect("write wav");
    frames as f64 / WAV_SAMPLE_RATE as f64
}

fn sim_engine_with_handle(clock: &VirtualClock) -> (PlayerEngine, SimHandle) {
    let backend = SimulatedBackend::new(clock.clone());
    let handle = backend.handle();
    (PlayerEngine::with_backend(Box::new(backend)), handle)
}

/// App de prueba con asa del backend simulado: motor simulado inyectado,
/// `StreamCacheState` y `AppState` con BD `:memory:` migrada.
async fn mock_advance_app_with_handle(
    db: sqlx::SqlitePool,
    clock: &VirtualClock,
) -> (tauri::App<tauri::test::MockRuntime>, SimHandle) {
    let app = tauri::test::mock_app();
    let state = PlayerState::default();
    {
        let mut core = lock_core(&state).expect("player core");
        let (engine, handle) = sim_engine_with_handle(clock);
        core.engine = Some(engine);
    }
    app.manage(state);
    app.manage(StreamCacheState::default());
    app.manage(crate::AppState {
        db,
        worker_state: crate::worker::DownloadWorkerState::new(2),
        enrichment_state: crate::enrichment_worker::EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(crate::services::ConcurrencyManager::new()),
    });
    (app, handle)
}

/// App de prueba sin asa (no interroga el log del backend simulado).
async fn mock_advance_app(
    db: sqlx::SqlitePool,
    clock: &VirtualClock,
) -> tauri::App<tauri::test::MockRuntime> {
    mock_advance_app_with_handle(db, clock).await.0
}

async fn memory_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

/// Track de biblioteca con su WAV descargado: la rama local de `open`.
async fn seed_track_with_file(db: &sqlx::SqlitePool, dir: &Path, title: &str, seconds: f64) -> i64 {
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(db)
        .await
        .unwrap();
    let path = dir.join(format!("{title}.wav"));
    write_wav_file(&path, seconds);
    sqlx::query("INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'WAV')")
        .bind(track_id)
        .bind(path.to_string_lossy().to_string())
        .execute(db)
        .await
        .unwrap();
    track_id
}

/// Carga y reproduce un track con el comando real (`seq` = generación).
async fn load_playing(app: &tauri::App<tauri::test::MockRuntime>, track_id: i64) -> u64 {
    player_load_track(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        app.state::<crate::AppState>(),
        track_id,
        true,
        1.0,
    )
    .await
    .expect("carga del motor")
    .seq
}

/// Un paso del ticker con el reloj virtual avanzado `step` segundos.
fn tick_step(
    state: &PlayerState,
    clock: &VirtualClock,
    step: f64,
) -> crate::services::playback_session::TickOutput {
    clock.advance(Duration::from_secs_f64(step));
    state.tick(None)
}

/// Reproduce `seconds` en pasos de 0,25 s y devuelve el crédito si cruzó.
fn play_seconds(state: &PlayerState, clock: &VirtualClock, seconds: f64) -> bool {
    let steps = (seconds / 0.25).round() as usize;
    let mut credited = false;
    for _ in 0..steps {
        if tick_step(state, clock, 0.25).listen_credit.is_some() {
            credited = true;
        }
    }
    credited
}

// ---------------------------------------------------------------------------
// 1. Fin natural → el backend avanza sin la UI
// ---------------------------------------------------------------------------

#[tokio::test]
async fn al_terminar_una_pista_suena_la_siguiente_sin_intervencion_de_la_ui() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 5.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 5.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    // Cola: A suena, B pendiente.
    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    let seq_a = load_playing(&app, a).await;

    // Fin natural de A: el ticker reporta `ended` con la generación de A.
    let out = tick_step(&state, &clock, 6.0);
    let ended = out.ended.expect("fin natural reportado");
    assert_eq!(ended.seq, seq_a);

    // El backend avanza: transición del agregado + carga en el motor.
    let payload = run_auto_advance(app.handle(), ended.seq)
        .await
        .expect("avance sin error")
        .expect("había pendiente que consumir");
    assert_eq!(payload.track_id, b);
    assert_eq!(payload.entry_id, ids[1]);

    // El motor ya suena la pista B en una generación nueva, posición 0.
    let out = tick_step(&state, &clock, 0.25);
    let tick = out.tick.expect("tick de la pista nueva");
    assert_eq!(tick.state, "playing");
    assert_eq!(tick.seq, payload.seq);
    assert!(tick.position_sec < 0.5, "la pista nueva empieza en 0");

    // El agregado: B es current, A al historial, posición a 0.
    let snapshot = perform_get_playback_queue(&db).await.unwrap();
    assert_eq!(
        snapshot.current.as_ref().expect("current B").entry_id,
        ids[1]
    );
    assert_eq!(snapshot.pending.len(), 0);
    assert_eq!(snapshot.history.last().expect("A en historial").track_id, a);
    assert_eq!(snapshot.position_sec, 0.0);
}

#[tokio::test]
async fn el_avance_emite_player_advanced_con_la_generacion_nueva() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 5.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 5.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    let seq_a = load_playing(&app, a).await;

    let (tx, rx) = std::sync::mpsc::channel::<(u64, i64, String)>();
    app.listen("player:advanced", move |event| {
        let payload: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
        tx.send((
            payload["seq"].as_u64().unwrap(),
            payload["track_id"].as_i64().unwrap(),
            payload["entry_id"].as_str().unwrap().to_string(),
        ))
        .unwrap();
    });

    let ended = tick_step(&state, &clock, 6.0).ended.expect("fin natural");
    super::spawn_auto_advance(app.handle().clone(), ended.seq);

    let (seq, track_id, entry_id) = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("evento player:advanced");
    assert_eq!(track_id, b);
    assert_eq!(entry_id, ids[1]);
    assert!(seq > seq_a, "el evento lleva la generación nueva");
}

// ---------------------------------------------------------------------------
// 2. repeat one / repeat all / repeat off resueltos en el backend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn repeat_one_repite_la_actual_con_sesion_nueva_y_sin_duplicar_historial() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 5.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    let ids = perform_enqueue(&db, &[a], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    perform_set_modes(&db, false, "one").await.unwrap();
    load_playing(&app, a).await;

    let ended = tick_step(&state, &clock, 6.0).ended.expect("fin natural");
    let payload = run_auto_advance(app.handle(), ended.seq)
        .await
        .expect("avance sin error")
        .expect("repeat one repite");
    assert_eq!(payload.track_id, a);
    assert_eq!(payload.entry_id, ids[0], "misma entrada, misma identidad");

    let out = tick_step(&state, &clock, 0.25);
    assert_eq!(out.tick.expect("tick").state, "playing", "vuelve a sonar");

    let snapshot = perform_get_playback_queue(&db).await.unwrap();
    assert!(
        snapshot.history.is_empty(),
        "repeat one no duplica historial"
    );
    assert_eq!(snapshot.position_sec, 0.0);
}

#[tokio::test]
async fn repeat_all_reconstruye_el_ciclo_completo_y_lo_vuelve_a_recorrer() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 5.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 5.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    perform_set_modes(&db, false, "all").await.unwrap();
    load_playing(&app, a).await;

    // A termina → suena B.
    let ended_a = tick_step(&state, &clock, 6.0).ended.expect("fin de A");
    let payload_b = run_auto_advance(app.handle(), ended_a.seq)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payload_b.track_id, b);

    // B termina → repeat all reconstruye el ciclo [A, B] y arranca por A.
    let ended_b = tick_step(&state, &clock, 6.0).ended.expect("fin de B");
    let payload_a = run_auto_advance(app.handle(), ended_b.seq)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payload_a.track_id, a);
    assert_eq!(payload_a.entry_id, ids[0], "el ciclo conserva identidades");

    let snapshot = perform_get_playback_queue(&db).await.unwrap();
    assert_eq!(
        snapshot.current.as_ref().expect("current A").entry_id,
        ids[0]
    );
    let pending: Vec<i64> = snapshot.pending.iter().map(|e| e.track_id).collect();
    assert_eq!(pending, vec![b], "B vuelve a la cola para cerrar el ciclo");
}

#[tokio::test]
async fn repeat_off_con_cola_vacia_deja_la_reproduccion_parada() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 5.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    let ids = perform_enqueue(&db, &[a], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    load_playing(&app, a).await;

    let ended = tick_step(&state, &clock, 6.0).ended.expect("fin natural");
    let result = run_auto_advance(app.handle(), ended.seq).await.unwrap();
    assert!(
        result.is_none(),
        "repeat off sin pendientes: nada que hacer"
    );
    // El motor sigue en la pista terminada; nadie cargó nada nuevo.
    let out = tick_step(&state, &clock, 0.25);
    assert!(
        out.ended.is_none(),
        "el fin ya se reportó; no hay carga nueva"
    );
}

#[tokio::test]
async fn una_carga_mas_nueva_retira_el_avance_automatico() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 5.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 5.0).await;
    let c = seed_track_with_file(&db, dir.path(), "C", 5.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    let seq_a = load_playing(&app, a).await;

    // El usuario (UI) carga otra pista mientras A sigue "vigente": generación
    // nueva para el motor.
    let _seq_c = load_playing(&app, c).await;

    // El avance automático de la generación de A llega tarde: se retira sin
    // tocar el agregado.
    let result = run_auto_advance(app.handle(), seq_a).await.unwrap();
    assert!(result.is_none(), "generación superada: nada que hacer");
    let snapshot = perform_get_playback_queue(&db).await.unwrap();
    assert_eq!(
        snapshot.current.as_ref().expect("current intacto").entry_id,
        ids[0],
        "el agregado no fue tocado por el avance retirado"
    );
    assert_eq!(snapshot.pending.len(), 1, "B sigue pendiente");
}

#[tokio::test]
async fn el_volumen_del_avance_es_el_estado_persistido() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 5.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 5.0).await;
    let c = seed_track_with_file(&db, dir.path(), "C", 5.0).await;
    let clock = VirtualClock::new();
    let (app, handle) = mock_advance_app_with_handle(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    let ids = perform_enqueue(&db, &[a, b, c], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    perform_set_volume(&db, 0.42, true).await.unwrap();
    let _ = load_playing(&app, a).await;

    // Avance con silencio persistido: el motor se carga con volumen 0.
    let ended = tick_step(&state, &clock, 11.0).ended.expect("fin de A");
    let payload = run_auto_advance(app.handle(), ended.seq)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payload.track_id, b);
    assert!(
        handle.log().iter().any(|op| op == "set_volume(0)"),
        "el silencio persistido carga volumen 0; log: {:?}",
        handle.log()
    );

    // Sin silencio: el volumen persistido (0.42) viaja al motor.
    perform_set_volume(&db, 0.42, false).await.unwrap();
    let ended_b = tick_step(&state, &clock, 6.0).ended.expect("fin de B");
    let payload_c = run_auto_advance(app.handle(), ended_b.seq)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payload_c.track_id, c);
    assert!(
        handle.log().iter().any(|op| op == "set_volume(0.42)"),
        "el volumen persistido carga en el avance; log: {:?}",
        handle.log()
    );
}

// ---------------------------------------------------------------------------
// 3. Crédito de escucha: 30 s reales acumulados por Rust, una vez por sesión
// ---------------------------------------------------------------------------

#[tokio::test]
async fn treinta_segundos_reales_acreditan_una_unica_vez_por_sesion() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 40.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    load_playing(&app, a).await;

    // 29,75 s acreditados: nada.
    assert!(!play_seconds(&state, &clock, 29.75), "29,75 s no acreditan");
    // El paso que cruza 30,0 s: exactamente un crédito.
    let out = tick_step(&state, &clock, 0.25);
    let credit = out.listen_credit.expect("30,0 s acreditan");
    assert_eq!(credit.track_id, a);
    // Y no vuelve a acreditar en el resto de la pista.
    assert!(!play_seconds(&state, &clock, 5.0), "una vez por sesión");

    // El crédito real llega a play_count UNA vez (idempotente por sesión).
    let stats = perform_record_local_play(&db, credit.track_id, &credit.listen_session_id)
        .await
        .expect("crédito aplicado");
    assert!(stats.counted);
    assert_eq!(stats.play_count, 1);
    let replay = perform_record_local_play(&db, credit.track_id, &credit.listen_session_id)
        .await
        .expect("reintento idempotente");
    assert!(!replay.counted, "la misma sesión nunca cuenta dos veces");
    assert_eq!(replay.play_count, 1);
}

#[tokio::test]
async fn la_pausa_suspende_la_acumulacion_sin_borrarla() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 120.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    load_playing(&app, a).await;
    assert!(!play_seconds(&state, &clock, 10.0), "10 s: nada aún");

    // 2 s en pausa: ni acredita ni borra el acumulado.
    player_pause(app.state::<PlayerState>()).expect("pausa");
    assert!(!play_seconds(&state, &clock, 2.0), "la pausa no acredita");

    player_resume(app.state::<PlayerState>()).expect("reanudar");
    // El primer tick tras reanudar sólo re-ancla; quedan 19,5 s reales por
    // acumular hasta el umbral... 10 + 19,5 = 29,5: nada aún.
    let out = tick_step(&state, &clock, 0.25);
    assert!(
        out.listen_credit.is_none(),
        "el tick de reanudación re-ancla"
    );
    assert!(!play_seconds(&state, &clock, 19.5), "29,5 s: nada aún");
    // +0,25 → 29,75; +0,25 → 30,0.
    assert!(!play_seconds(&state, &clock, 0.25));
    let out = tick_step(&state, &clock, 0.25);
    assert!(out.listen_credit.is_some(), "30 s REALES acreditan");
}

#[tokio::test]
async fn el_seek_no_acredita_el_tramo_saltado() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 120.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    load_playing(&app, a).await;
    assert!(!play_seconds(&state, &clock, 2.0), "2 s escuchados");

    // Seek de 2 s a 20 s: el salto de 18 s NO acredita (re-anclaje).
    player_seek(
        app.handle().clone(),
        app.state::<PlayerState>(),
        app.state::<StreamCacheState>(),
        20.0,
    )
    .expect("seek");
    let out = tick_step(&state, &clock, 0.25);
    assert!(out.listen_credit.is_none(), "el salto no acredita");

    // Desde el ancla: 2 + 27,75 = 29,75 s reales → nada.
    assert!(!play_seconds(&state, &clock, 27.75), "29,75 s: nada aún");
    // +0,25 → 30,0 reales.
    let out = tick_step(&state, &clock, 0.25);
    assert!(
        out.listen_credit.is_some(),
        "sólo los segundos realmente reproducidos acreditan"
    );
}

#[tokio::test]
async fn la_velocidad_acredita_segundos_de_reloj_no_de_medio() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 200.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    load_playing(&app, a).await;
    player_set_rate(app.state::<PlayerState>(), 2.0).expect("rate 2x");

    // A 2×, el medio avanza 0,5 s por tick; el crédito acumula 0,25 s reales
    // por tick: 30 s de reloj (120 ticks), NO 15 s (60 ticks) ni 60 de medio.
    assert!(
        !play_seconds(&state, &clock, 14.75),
        "29,5 s de reloj: nada"
    );
    let out = tick_step(&state, &clock, 0.25);
    assert!(out.listen_credit.is_none(), "29,75 s de reloj: nada aún");
    let out = tick_step(&state, &clock, 0.25);
    assert!(out.listen_credit.is_some(), "30 s de reloj acreditan");
}

#[tokio::test]
async fn cada_pista_avanzada_abre_sesion_de_escucha_nueva() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 40.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 40.0).await;
    let clock = VirtualClock::new();
    let app = mock_advance_app(db.clone(), &clock).await;
    let state = app.state::<PlayerState>();

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    load_playing(&app, a).await;

    // A acumula 30 s y acredita su sesión.
    assert!(!play_seconds(&state, &clock, 29.75));
    let credit_a = tick_step(&state, &clock, 0.25)
        .listen_credit
        .expect("crédito A");
    assert_eq!(credit_a.track_id, a);

    // A termina y el backend avanza a B; nueva sesión de escucha.
    let ended = tick_step(&state, &clock, 11.0).ended.expect("fin de A");
    let payload = run_auto_advance(app.handle(), ended.seq)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payload.track_id, b);

    // B acredita con SU propia sesión (30 s reales de B).
    assert!(
        !play_seconds(&state, &clock, 29.75),
        "sesión nueva parte de 0"
    );
    let credit_b = tick_step(&state, &clock, 0.25)
        .listen_credit
        .expect("crédito B");
    assert_eq!(credit_b.track_id, b);
    assert_ne!(
        credit_b.listen_session_id, credit_a.listen_session_id,
        "cada pista abre sesión nueva"
    );

    perform_record_local_play(&db, credit_a.track_id, &credit_a.listen_session_id)
        .await
        .expect("A contada");
    let stats_b = perform_record_local_play(&db, credit_b.track_id, &credit_b.listen_session_id)
        .await
        .expect("B contada");
    assert!(stats_b.counted);
    assert_eq!(stats_b.play_count, 1);
}

#[tokio::test]
async fn el_umbral_es_treinta_segundos_exactos() {
    assert_eq!(LISTEN_THRESHOLD_SEC, 30.0);
}
