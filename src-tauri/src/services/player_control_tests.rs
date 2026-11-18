//! #133 (P02) — acciones de transporte del backend con motor simulado.
//!
//! Congela la semántica que MPRIS ejecuta sin webview: play/pause, next,
//! previous (con la pista anterior devuelta a pendientes), seek y modos
//! persistidos. El motor es el backend simulado de `syncify_player::sim`
//! (reloj virtual, sin audio) y la BD es `:memory:` migrada.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use syncify_player::sim::{SimulatedBackend, VirtualClock};
use syncify_player::PlayerEngine;
use tauri::{Listener, Manager};

use super::{
    run_action, set_repeat, set_shuffle, BackendAction, RepeatMode, PLAYER_MODES_CHANGED_EVENT,
};
use crate::commands::playback_queue::{
    perform_enqueue, perform_get_playback_queue, perform_set_current,
};
use crate::commands::player::player_load_track;
use crate::services::mpris::PlaybackStatus;
use crate::services::playback_advance::PlayerAdvancedPayload;
use crate::services::playback_session::{lock_core, PlayerState};
use crate::services::stream_cache::StreamCacheState;

const WAV_SAMPLE_RATE: u32 = 8000;

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

async fn memory_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

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

async fn mock_app(
    db: sqlx::SqlitePool,
    clock: &VirtualClock,
) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let state = PlayerState::default();
    {
        let mut core = lock_core(&state).expect("player core");
        let backend = SimulatedBackend::new(clock.clone());
        core.engine = Some(PlayerEngine::with_backend(Box::new(backend)));
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

#[tokio::test]
async fn play_pause_alterna_el_motor_sin_webview() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 30.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db, &clock).await;
    load_playing(&app, a).await;

    let state = app.state::<PlayerState>();
    assert_eq!(state.playback_status(), PlaybackStatus::Playing);

    run_action(app.handle(), BackendAction::PlayPause)
        .await
        .unwrap();
    assert_eq!(state.playback_status(), PlaybackStatus::Paused);

    run_action(app.handle(), BackendAction::PlayPause)
        .await
        .unwrap();
    assert_eq!(state.playback_status(), PlaybackStatus::Playing);

    run_action(app.handle(), BackendAction::Pause)
        .await
        .unwrap();
    assert_eq!(state.playback_status(), PlaybackStatus::Paused);
    run_action(app.handle(), BackendAction::Play).await.unwrap();
    assert_eq!(state.playback_status(), PlaybackStatus::Playing);
}

#[tokio::test]
async fn next_consume_la_cabeza_y_previous_la_devuelve_a_pendientes() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 30.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 30.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db.clone(), &clock).await;

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    load_playing(&app, a).await;

    let next = run_action(app.handle(), BackendAction::Next)
        .await
        .unwrap()
        .expect("next abre la pista pendiente");
    assert_eq!(next.track_id, b);
    assert_eq!(next.entry_id, ids[1]);
    assert_eq!(next.previous_entry_id, None);
    assert_eq!(app.state::<PlayerState>().current_track_id(), Some(b));

    // Posición 0: previous salta al historial en vez de reiniciar.
    let previous = run_action(app.handle(), BackendAction::Previous)
        .await
        .unwrap()
        .expect("previous recupera A");
    assert_eq!(previous.track_id, a);
    assert_eq!(previous.entry_id, ids[0]);
    assert_eq!(
        previous.previous_entry_id.as_deref(),
        Some(ids[1].as_str()),
        "B vuelve al frente de los pendientes"
    );
    assert_eq!(app.state::<PlayerState>().current_track_id(), Some(a));

    let snapshot = perform_get_playback_queue(&db).await.unwrap();
    assert_eq!(
        snapshot.current.as_ref().expect("current A").entry_id,
        ids[0]
    );
    assert_eq!(
        snapshot
            .pending
            .iter()
            .map(|entry| entry.entry_id.clone())
            .collect::<Vec<_>>(),
        vec![ids[1].clone()]
    );
    assert!(snapshot.history.is_empty());
}

#[tokio::test]
async fn previous_con_mas_de_tres_segundos_reinicia_la_actual() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 60.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 60.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db.clone(), &clock).await;

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[1], b).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    load_playing(&app, a).await;

    clock.advance(Duration::from_secs(5));
    assert!(app.state::<PlayerState>().position().unwrap() >= Duration::from_secs(5));

    let payload = run_action(app.handle(), BackendAction::Previous)
        .await
        .unwrap();
    assert!(payload.is_none(), "previous reinicia, no cambia de pista");
    assert_eq!(
        app.state::<PlayerState>().position(),
        Some(Duration::from_secs(0))
    );
    assert_eq!(app.state::<PlayerState>().current_track_id(), Some(a));
}

#[tokio::test]
async fn seek_relativo_y_absoluto_mueven_el_motor() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 60.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db, &clock).await;
    load_playing(&app, a).await;

    run_action(app.handle(), BackendAction::SeekByMicros(5_000_000))
        .await
        .unwrap();
    assert_eq!(
        app.state::<PlayerState>().position(),
        Some(Duration::from_secs(5))
    );

    run_action(
        app.handle(),
        BackendAction::SetPositionMicros {
            track_id: a,
            position_us: 12_000_000,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        app.state::<PlayerState>().position(),
        Some(Duration::from_secs(12))
    );

    // Track ajeno: la spec MPRIS manda ignorarlo.
    run_action(
        app.handle(),
        BackendAction::SetPositionMicros {
            track_id: a + 999,
            position_us: 30_000_000,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        app.state::<PlayerState>().position(),
        Some(Duration::from_secs(12))
    );
}

#[tokio::test]
async fn shuffle_y_loop_persisten_y_emiten_el_orden_nuevo() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 30.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 30.0).await;
    let c = seed_track_with_file(&db, dir.path(), "C", 30.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db.clone(), &clock).await;

    let ids = perform_enqueue(&db, &[a, b, c], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<(String, Vec<String>)>();
    app.listen(PLAYER_MODES_CHANGED_EVENT, move |event| {
        let payload: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
        tx.send((
            payload["repeat"].as_str().unwrap().to_string(),
            payload["pendingEntryIds"]
                .as_array()
                .map(|entries| {
                    entries
                        .iter()
                        .map(|value| value.as_str().unwrap().to_string())
                        .collect()
                })
                .unwrap_or_default(),
        ))
        .unwrap();
    });

    set_shuffle(app.handle(), true).await.unwrap();
    let (_, pending) = rx.recv_timeout(Duration::from_secs(5)).expect("evento");
    let mut sorted = pending.clone();
    sorted.sort();
    let mut expected = vec![ids[1].clone(), ids[2].clone()];
    expected.sort();
    assert_eq!(sorted, expected, "el shuffle conserva todas las entradas");
    assert_eq!(perform_get_playback_queue(&db).await.unwrap().shuffle, true);

    set_shuffle(app.handle(), false).await.unwrap();
    let (_, pending) = rx.recv_timeout(Duration::from_secs(5)).expect("evento");
    assert_eq!(
        pending,
        vec![ids[1].clone(), ids[2].clone()],
        "apagar shuffle restaura el orden de encolado"
    );

    set_repeat(app.handle(), RepeatMode::All).await.unwrap();
    let (repeat, _) = rx.recv_timeout(Duration::from_secs(5)).expect("evento");
    assert_eq!(repeat, "all");
    let snapshot = perform_get_playback_queue(&db).await.unwrap();
    assert_eq!(snapshot.repeat, "all");
    assert!(!snapshot.shuffle);
}

#[tokio::test]
async fn next_sin_pendientes_con_repeat_all_reconstruye_el_ciclo() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 30.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 30.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db.clone(), &clock).await;

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    perform_set_current(&db, &ids[1], b).await.unwrap();
    set_repeat(app.handle(), RepeatMode::All).await.unwrap();

    let payload = run_action(app.handle(), BackendAction::Next)
        .await
        .unwrap()
        .expect("repeat all reconstruye y avanza");
    assert_eq!(payload.track_id, a);
    assert_eq!(payload.entry_id, ids[0]);
}

#[tokio::test]
async fn next_sin_pendientes_con_repeat_one_es_no_op() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 30.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db.clone(), &clock).await;

    let ids = perform_enqueue(&db, &[a], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    set_repeat(app.handle(), RepeatMode::One).await.unwrap();
    let seq_before = app.state::<PlayerState>().current_track_id();

    let payload = run_action(app.handle(), BackendAction::Next).await.unwrap();
    assert!(payload.is_none(), "next manual ignora repeat one");
    assert_eq!(app.state::<PlayerState>().current_track_id(), seq_before);
}

#[tokio::test]
async fn play_sin_pista_cargada_arranca_la_cabeza_de_la_cola() {
    let db = memory_pool().await;
    let dir = tempfile::tempdir().expect("tmp");
    let a = seed_track_with_file(&db, dir.path(), "A", 30.0).await;
    let b = seed_track_with_file(&db, dir.path(), "B", 30.0).await;
    let clock = VirtualClock::new();
    let app = mock_app(db.clone(), &clock).await;

    let ids = perform_enqueue(&db, &[a, b], false).await.unwrap();
    perform_set_current(&db, &ids[0], a).await.unwrap();
    // El motor está vacío (la UI aún no restauró nada).
    let payload: Option<PlayerAdvancedPayload> =
        run_action(app.handle(), BackendAction::Play).await.unwrap();
    assert!(payload.is_none(), "play de la actual no es un avance");
    assert_eq!(
        app.state::<PlayerState>().current_track_id(),
        Some(a),
        "play abre la pista actual persistida"
    );
}
