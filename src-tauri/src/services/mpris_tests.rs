//! #133 (P02) — capa MPRIS con estado simulado.
//!
//! No arranca D-Bus ni audio: construye el estado compartido del servicio,
//! publica transporte/pista/modos como lo hace el ticker y comprueba el
//! `Snapshot` que decide `PropertiesChanged` y los metadatos que se resuelven
//! desde la biblioteca. Las acciones de transporte se prueban en
//! `player_control_tests.rs` con el motor simulado.
#![cfg(target_os = "linux")]

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{lock_shared, MprisService, NowPlaying, PlaybackStatus, Shared, Snapshot};
use crate::commands::library::fetch_track_metadata;

fn test_service() -> MprisService {
    MprisService {
        shared: Arc::new(Mutex::new(Shared::default())),
        stop: Arc::new(AtomicBool::new(false)),
    }
}

fn snapshot(service: &MprisService) -> Snapshot {
    Snapshot::from(&*lock_shared(&service.shared))
}

async fn memory_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn update_playback_publica_transporte_posicion_y_pista() {
    let service = test_service();
    let app = tauri::test::mock_app();

    service.update_playback(
        app.handle(),
        PlaybackStatus::Playing,
        Duration::from_millis(42_500),
        Some(NowPlaying {
            track_id: 7,
            duration: Some(Duration::from_secs(180)),
        }),
    );

    let state = lock_shared(&service.shared);
    assert_eq!(state.playback, PlaybackStatus::Playing);
    assert_eq!(state.position_us, 42_500_000);
    assert_eq!(state.metadata.track_id, Some(7));
    assert_eq!(state.metadata.duration_us, Some(180_000_000));
}

#[tokio::test]
async fn cambiar_de_pista_limpia_los_metadatos_anteriores() {
    let service = test_service();
    let app = tauri::test::mock_app();
    {
        let mut state = lock_shared(&service.shared);
        state.metadata.track_id = Some(1);
        state.metadata.title = Some("Anterior".to_string());
        state.metadata.artist = Some("Artista".to_string());
    }

    service.update_playback(
        app.handle(),
        PlaybackStatus::Playing,
        Duration::ZERO,
        Some(NowPlaying {
            track_id: 2,
            duration: Some(Duration::from_secs(10)),
        }),
    );

    let state = lock_shared(&service.shared);
    assert_eq!(state.metadata.track_id, Some(2));
    assert_eq!(
        state.metadata.title, None,
        "no se hereda el título anterior"
    );
    assert_eq!(state.metadata.artist, None);
    assert_eq!(state.metadata.duration_us, Some(10_000_000));
}

#[tokio::test]
async fn apply_track_metadata_rellena_la_pista_vigente_y_ignora_la_vieja() {
    let db = memory_pool().await;
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Canción', 123456) RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let metadata = fetch_track_metadata(&db, track_id).await.unwrap();

    let service = test_service();
    {
        let mut state = lock_shared(&service.shared);
        state.metadata.track_id = Some(track_id);
    }
    service.apply_track_metadata(track_id, &metadata);
    {
        let state = lock_shared(&service.shared);
        assert_eq!(state.metadata.title.as_deref(), Some("Canción"));
        assert_eq!(state.metadata.duration_us, Some(123_456_000));
    }

    // Una consulta lenta de una pista ya sustituida no pisa la nueva.
    service.apply_track_metadata(track_id + 1, &metadata);
    let state = lock_shared(&service.shared);
    assert_eq!(state.metadata.track_id, Some(track_id));
    assert_eq!(state.metadata.title.as_deref(), Some("Canción"));
}

#[tokio::test]
async fn update_modes_cambia_shuffle_y_loop_status() {
    let service = test_service();
    service.update_modes(true, "one");
    {
        let state = lock_shared(&service.shared);
        assert!(state.shuffle);
        assert_eq!(state.loop_status.as_str(), "Track");
    }
    service.update_modes(false, "all");
    {
        let state = lock_shared(&service.shared);
        assert!(!state.shuffle);
        assert_eq!(state.loop_status.as_str(), "Playlist");
    }
    service.update_modes(false, "off");
    assert_eq!(lock_shared(&service.shared).loop_status.as_str(), "None");
}

#[tokio::test]
async fn el_snapshot_solo_cambia_con_lo_que_mpris_publica() {
    let service = test_service();
    let before = snapshot(&service);

    service.update_playback(
        tauri::test::mock_app().handle(),
        PlaybackStatus::Paused,
        Duration::from_secs(3),
        None,
    );
    let paused = snapshot(&service);
    assert_ne!(before, paused, "PlaybackStatus viaja en el diff");

    // La posición NO entra en el snapshot: los clientes la consultan aparte.
    service.update_playback(
        tauri::test::mock_app().handle(),
        PlaybackStatus::Paused,
        Duration::from_secs(9),
        None,
    );
    assert_eq!(paused, snapshot(&service));

    service.update_modes(true, "all");
    assert_ne!(paused, snapshot(&service));
}

#[tokio::test]
async fn los_modos_por_defecto_son_parados_sin_shuffle_y_loop_none() {
    let state = Shared::default();
    assert_eq!(state.playback, PlaybackStatus::Stopped);
    assert!(!state.shuffle);
    assert_eq!(state.loop_status.as_str(), "None");
    assert_eq!(state.metadata.track_id, None);
}
