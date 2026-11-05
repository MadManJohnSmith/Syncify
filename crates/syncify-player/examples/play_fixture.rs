//! Prueba dirigida de audio del motor nativo (Fase 1).
//!
//! Uso: `cargo run -p syncify-player --example play_fixture -- <archivo> [seg] [seek_seg]`
//! Reproduce el archivo real, imprime estado/posición, ejecuta un seek si se pide
//! y sale con código 0. La salida audible se verifica aparte capturando el monitor
//! del sink (los fixtures de prueba son tonos puros de frecuencia conocida).

use std::time::{Duration, Instant};

use syncify_player::PlayerEngine;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("uso: play_fixture <archivo> [segundos] [seek_seg]");
    let seconds: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5.0);
    let seek: Option<f64> = args.get(3).and_then(|s| s.parse().ok());

    let mut engine = PlayerEngine::new().expect("no hay salida de audio por defecto");
    let info = engine
        .load_and_play(std::path::Path::new(path))
        .expect("no se pudo cargar/reproducir el archivo");
    println!(
        "LOADED path={} duration={:?} state={:?}",
        info.path.display(),
        info.duration,
        engine.state()
    );

    let start = Instant::now();
    let deadline = start + Duration::from_secs_f64(seconds);
    let mut seeked = false;
    let mut last_reported = Duration::ZERO;

    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));

        if let Some(target) = seek {
            if !seeked && start.elapsed() >= Duration::from_millis(1200) {
                println!("BEFORE_SEEK pos={:?}", engine.position());
                engine
                    .seek(Duration::from_secs_f64(target))
                    .expect("seek falló");
                seeked = true;
            }
        }

        let pos = engine.position();
        if pos.saturating_sub(last_reported) >= Duration::from_millis(900) {
            println!("TICK pos={pos:?} state={:?}", engine.state());
            last_reported = pos;
        }
    }

    println!(
        "FINAL state={:?} pos={:?} duration={:?} ended={}",
        engine.state(),
        engine.position(),
        engine.duration(),
        engine.track_ended()
    );
    engine.pause();
    println!("AFTER_PAUSE state={:?}", engine.state());
}
