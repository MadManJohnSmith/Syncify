//! Transporte del motor contra el backend simulado: reloj virtual, sin audio
//! y sin red. Congela la semántica observable que Fase B va a mover.

mod common;

use std::path::PathBuf;
use std::time::Duration;

use syncify_player::sim::{SimulatedBackend, VirtualClock};
use syncify_player::{EngineError, EngineState, PlayerEngine};

/// Motor con backend simulado y reloj compartido.
fn sim_engine(clock: &VirtualClock) -> PlayerEngine {
    PlayerEngine::with_backend(Box::new(SimulatedBackend::new(clock.clone())))
}

/// WAV de 1 segundo a 8 kHz (decodificación real mínima para el `load`).
fn one_second_wav(dir: &std::path::Path, name: &str) -> (PathBuf, f64) {
    let path = dir.join(name);
    let duration = common::write_sine_wav(&path, 8000, 1.0);
    (path, duration)
}

#[test]
fn load_with_autoplay_reports_playing_at_position_zero() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, duration) = one_second_wav(dir.path(), "a.wav");

    let loaded = engine.load(&path, true).expect("load");

    assert_eq!(loaded.path, path);
    assert_eq!(loaded.duration, Some(Duration::from_secs_f64(duration)));
    assert_eq!(engine.state(), EngineState::Playing);
    assert_eq!(engine.position(), Duration::ZERO);
    assert_eq!(engine.duration(), Some(Duration::from_secs_f64(duration)));
    assert_eq!(engine.loaded().map(|l| l.path.clone()), Some(path));
    assert!(!engine.track_ended());
}

#[test]
fn load_without_autoplay_reports_paused_at_position_zero() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, _) = one_second_wav(dir.path(), "a.wav");

    engine.load(&path, false).expect("load");

    assert_eq!(engine.state(), EngineState::Paused);
    assert_eq!(engine.position(), Duration::ZERO);

    // La restauración de sesión reanuda desde la posición 0.
    engine.resume().expect("resume");
    assert_eq!(engine.state(), EngineState::Playing);
}

#[test]
fn virtual_clock_advances_position_while_playing() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, _) = one_second_wav(dir.path(), "a.wav");
    engine.load(&path, true).expect("load");

    clock.advance(Duration::from_millis(250));
    assert!(
        (engine.position().as_secs_f64() - 0.25).abs() < 1e-9,
        "position {}",
        engine.position().as_secs_f64()
    );

    clock.advance(Duration::from_millis(250));
    assert!(
        (engine.position().as_secs_f64() - 0.5).abs() < 1e-9,
        "position {}",
        engine.position().as_secs_f64()
    );

    // Sin avance de reloj la posición no se mueve: el tiempo es del test.
    assert!(
        (engine.position().as_secs_f64() - 0.5).abs() < 1e-9,
        "position {}",
        engine.position().as_secs_f64()
    );
}

#[test]
fn rate_multiplies_virtual_position() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, _) = one_second_wav(dir.path(), "a.wav");
    engine.load(&path, true).expect("load");

    engine.set_rate(2.0);
    clock.advance(Duration::from_millis(250));
    assert!(
        (engine.position().as_secs_f64() - 0.5).abs() < 1e-6,
        "position {}",
        engine.position().as_secs_f64()
    );

    // Volver a ritmo normal recupera el avance unitario desde la posición actual.
    engine.set_rate(1.0);
    clock.advance(Duration::from_millis(250));
    assert!(
        (engine.position().as_secs_f64() - 0.75).abs() < 1e-6,
        "position {}",
        engine.position().as_secs_f64()
    );
}

#[test]
fn pause_freezes_position_and_resume_continues() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, _) = one_second_wav(dir.path(), "a.wav");
    engine.load(&path, true).expect("load");
    clock.advance(Duration::from_millis(250));

    engine.pause();
    assert_eq!(engine.state(), EngineState::Paused);
    clock.advance(Duration::from_millis(500));
    assert_eq!(engine.position(), Duration::from_millis(250));

    engine.resume().expect("resume");
    assert_eq!(engine.state(), EngineState::Playing);
    clock.advance(Duration::from_millis(250));
    assert_eq!(engine.position(), Duration::from_millis(500));
}

#[test]
fn seek_is_deterministic_and_clamped_to_duration() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, duration) = one_second_wav(dir.path(), "a.wav");
    engine.load(&path, false).expect("load paused");

    engine.seek(Duration::from_secs_f64(0.4)).expect("seek");
    assert_eq!(engine.position(), Duration::from_secs_f64(0.4));
    // Seek en pausa no reanuda.
    assert_eq!(engine.state(), EngineState::Paused);

    // Fuera de rango se acota a la duración de la pista. Buscar (o acotarse)
    // al último instante consume la fuente: la pista queda terminada, igual
    // que en rodio (fin natural, no `stop()`).
    engine.seek(Duration::from_secs(30)).expect("seek");
    assert_eq!(engine.position(), Duration::from_secs_f64(duration));
    assert_eq!(engine.state(), EngineState::Stopped);
    assert!(engine.track_ended());
}

#[test]
fn track_ended_after_eof_and_position_freezes() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, duration) = one_second_wav(dir.path(), "a.wav");
    engine.load(&path, true).expect("load");

    clock.advance(Duration::from_secs_f64(duration));
    assert_eq!(engine.state(), EngineState::Stopped);
    assert!(engine.track_ended());
    // La posición se congela en el final, aunque el reloj siga corriendo.
    clock.advance(Duration::from_secs(10));
    assert_eq!(engine.position(), Duration::from_secs_f64(duration));
    assert!(engine.track_ended());

    // `stop()` descarga la pista: el fin ya no está reportado.
    engine.stop();
    assert!(!engine.track_ended());
    assert!(engine.loaded().is_none());
}

#[test]
fn stop_clears_the_track() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let (path, _) = one_second_wav(dir.path(), "a.wav");
    engine.load(&path, false).expect("load");

    engine.stop();
    assert_eq!(engine.state(), EngineState::Stopped);
    assert_eq!(engine.duration(), None);
    assert!(matches!(engine.resume(), Err(EngineError::NothingLoaded)));
    assert!(matches!(
        engine.seek(Duration::ZERO),
        Err(EngineError::NothingLoaded)
    ));
}

#[test]
fn volume_and_rate_are_reapplied_on_every_load() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let backend = SimulatedBackend::new(clock.clone());
    let handle = backend.handle();
    let mut engine = PlayerEngine::with_backend(Box::new(backend));
    let (first, _) = one_second_wav(dir.path(), "a.wav");
    let (second, _) = one_second_wav(dir.path(), "b.wav");

    engine.set_volume(0.5);
    engine.set_rate(1.5);
    engine.load(&first, false).expect("load 1");
    engine.load(&second, true).expect("load 2");

    // El motor conserva los valores entre cargas…
    assert_eq!(engine.volume(), 0.5);
    assert_eq!(engine.rate(), 1.5);
    assert_eq!(handle.speed(), 1.5);
    // …y el registro del backend congela la coreografía completa de `load`:
    // stop → [pause] → append → set_volume → set_speed → try_seek(0) → [play].
    assert_eq!(
        handle.log(),
        vec![
            "set_volume(0.5)",
            "set_speed(1.5)",
            "stop",
            "pause",
            "append",
            "set_volume(0.5)",
            "set_speed(1.5)",
            "try_seek(0)",
            "stop",
            "append",
            "set_volume(0.5)",
            "set_speed(1.5)",
            "try_seek(0)",
            "play",
        ]
    );
}

#[test]
fn load_missing_file_returns_file_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);

    let err = engine
        .load(&dir.path().join("no-existe.wav"), true)
        .expect_err("missing file");
    assert!(matches!(err, EngineError::FileNotFound));
    assert_eq!(engine.state(), EngineState::Stopped);

    // El motor sigue utilizable tras el error.
    let (path, _) = one_second_wav(dir.path(), "a.wav");
    engine.load(&path, true).expect("load after error");
    assert_eq!(engine.state(), EngineState::Playing);
}

#[test]
fn fresh_engine_is_stopped_and_transport_errors() {
    let clock = VirtualClock::new();
    let engine = sim_engine(&clock);

    assert_eq!(engine.state(), EngineState::Stopped);
    assert_eq!(engine.position(), Duration::ZERO);
    assert_eq!(engine.duration(), None);
    assert!(engine.loaded().is_none());
    assert!(!engine.track_ended());
    assert!(matches!(engine.resume(), Err(EngineError::NothingLoaded)));
    assert!(matches!(
        engine.seek(Duration::from_secs(1)),
        Err(EngineError::NothingLoaded)
    ));
    // `toggle` sin pista es un no-op.
    engine.toggle();
    assert_eq!(engine.state(), EngineState::Stopped);
}

#[test]
fn volume_and_rate_inputs_are_sanitized() {
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);

    engine.set_volume(-1.0);
    assert_eq!(engine.volume(), 0.0);
    engine.set_volume(0.75);
    assert_eq!(engine.volume(), 0.75);

    engine.set_rate(0.0);
    assert_eq!(engine.rate(), 1.0);
    engine.set_rate(-2.0);
    assert_eq!(engine.rate(), 1.0);
    engine.set_rate(f32::NAN);
    assert_eq!(engine.rate(), 1.0);
    engine.set_rate(1.25);
    assert_eq!(engine.rate(), 1.25);
}
