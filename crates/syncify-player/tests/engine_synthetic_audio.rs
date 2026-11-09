//! Audio sintético (WAV PCM generado en el test, decodificado por symphonia a
//! través de rodio) sobre el backend simulado: la promesa de ADR-0011 «los
//! tests del motor usan audio sintético». Sin dispositivo de audio ni red.

mod common;

use std::time::Duration;

use syncify_player::sim::{SimulatedBackend, VirtualClock};
use syncify_player::{EngineState, PlayerEngine};

/// Motor con backend simulado y reloj compartido.
fn sim_engine(clock: &VirtualClock) -> PlayerEngine {
    PlayerEngine::with_backend(Box::new(SimulatedBackend::new(clock.clone())))
}

#[test]
fn synthetic_wav_reports_duration_state_and_position() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let path = dir.path().join("sine.wav");
    let expected = common::write_sine_wav(&path, 44100, 2.0);
    assert!((expected - 2.0).abs() < 1e-9);

    let loaded = engine.load(&path, true).expect("load");

    // La duración la reporta el decodificador real (symphonia), no el backend.
    assert_eq!(
        loaded.duration,
        Some(Duration::from_secs_f64(expected)),
        "symphonia debe derivar la duración del WAV"
    );
    assert_eq!(engine.duration(), Some(Duration::from_secs_f64(expected)));
    assert_eq!(engine.state(), EngineState::Playing);
    assert_eq!(engine.position(), Duration::ZERO);

    // El reloj virtual avanza la posición sobre la pista decodificada.
    clock.advance(Duration::from_millis(500));
    assert_eq!(engine.position(), Duration::from_millis(500));
}

#[test]
fn synthetic_wav_seek_is_deterministic_and_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let path = dir.path().join("sine.wav");
    let duration = common::write_sine_wav(&path, 44100, 2.0);
    engine.load(&path, true).expect("load");

    engine.seek(Duration::from_secs_f64(0.5)).expect("seek");
    assert_eq!(engine.position(), Duration::from_millis(500));
    clock.advance(Duration::from_millis(250));
    assert_eq!(engine.position(), Duration::from_millis(750));

    // Acotado al final declarado por el contenedor.
    engine.seek(Duration::from_secs(60)).expect("seek");
    assert_eq!(engine.position(), Duration::from_secs_f64(duration));
}

#[test]
fn zero_sample_wav_reports_no_declared_duration() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let path = dir.path().join("empty.wav");
    common::write_empty_wav(&path, 8000);

    let loaded = engine.load(&path, true).expect("load");

    // Sin muestras, symphonia no declara duración: rama `None` del motor.
    assert_eq!(loaded.duration, None);
    assert_eq!(engine.duration(), None);

    // Sin duración declarada el seek no se acota por el final.
    engine.seek(Duration::from_secs(10)).expect("seek");
    assert_eq!(engine.position(), Duration::from_secs(10));
}

#[test]
fn synthetic_wav_signals_end_of_track_after_eof() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let path = dir.path().join("sine.wav");
    let duration = common::write_sine_wav(&path, 8000, 0.5);
    engine.load(&path, true).expect("load");

    clock.advance(Duration::from_secs_f64(duration + 0.25));

    assert_eq!(engine.state(), EngineState::Stopped);
    assert!(engine.track_ended());
    assert_eq!(engine.position(), Duration::from_secs_f64(duration));
}

#[test]
fn synthetic_wav_pause_resume_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let clock = VirtualClock::new();
    let mut engine = sim_engine(&clock);
    let path = dir.path().join("sine.wav");
    common::write_sine_wav(&path, 8000, 2.0);
    engine.load(&path, true).expect("load");

    clock.advance(Duration::from_millis(400));
    engine.pause();
    clock.advance(Duration::from_millis(600));
    assert_eq!(engine.position(), Duration::from_millis(400));

    // Un seek en pausa mueve la posición sin reanudar.
    engine.seek(Duration::from_millis(1500)).expect("seek");
    assert_eq!(engine.state(), EngineState::Paused);

    engine.resume().expect("resume");
    clock.advance(Duration::from_millis(100));
    assert_eq!(engine.position(), Duration::from_millis(1600));
}
