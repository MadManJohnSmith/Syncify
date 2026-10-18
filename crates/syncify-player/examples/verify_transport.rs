//! Verificación dirigida de transporte del motor nativo (Fase 1).
//!
//! Comprueba las dos conductas nuevas de las que depende el frontend:
//!
//! 1. `load(autoplay = false)` (restauración de sesión): la pista queda
//!    cargada y PAUSADA en la posición 0 — sin audio — y `resume()` arranca.
//! 2. Recargar el MISMO fichero (replay desde el frontend): el decodificador
//!    anterior se sustituye limpiamente y la nueva carga empieza desde 0.
//!
//! Además cambia a un segundo fichero para verificar el relevo de decodificador
//! entre formatos/tamaños. Imprime `MARK phase=... ms=...` para que el análisis
//! de audio (captura del monitor del sink) alinee las fases, y sale con código
//! distinto de cero si alguna comprobación falla.
//!
//! Uso: `cargo run -p syncify-player --example verify_transport -- <archivoA> [archivoB]`

use std::path::Path;
use std::time::{Duration, Instant};

use syncify_player::{EngineState, PlayerEngine};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file_a = args
        .get(1)
        .expect("uso: verify_transport <archivoA> [archivoB]");
    let file_b = args.get(2);

    let mut engine = PlayerEngine::new().expect("no hay salida de audio por defecto");
    let t0 = Instant::now();
    let mut failures = 0u32;

    macro_rules! check {
        ($cond:expr, $($arg:tt)*) => {
            if $cond {
                println!("PASS {}", format!($($arg)*));
            } else {
                failures += 1;
                println!("FAIL {}", format!($($arg)*));
            }
        };
    }
    macro_rules! mark {
        ($phase:expr) => {
            println!("MARK phase={} ms={}", $phase, t0.elapsed().as_millis());
        };
    }

    // ── Fase 1: load pausado (restauración) ─────────────────────────────────
    mark!("paused_load");
    let info = engine
        .load(Path::new(file_a), false)
        .expect("no se pudo cargar el archivo A");
    check!(
        engine.state() == EngineState::Paused,
        "load(autoplay=false) deja el motor Paused (estado={:?})",
        engine.state()
    );
    check!(
        matches!(info.duration, Some(d) if d > Duration::from_secs(19) && d < Duration::from_secs(21)),
        "duración A ≈ 20 s (duration={:?})",
        info.duration
    );
    std::thread::sleep(Duration::from_millis(1500));
    let paused_pos = engine.position();
    check!(
        engine.state() == EngineState::Paused && paused_pos <= Duration::from_millis(150),
        "1,5 s tras cargar en pausa sigue Paused y en ~0 (pos={paused_pos:?})"
    );

    // ── Fase 2: resume ──────────────────────────────────────────────────────
    mark!("resume");
    engine.resume().expect("resume falló");
    std::thread::sleep(Duration::from_millis(1500));
    let resumed_pos = engine.position();
    check!(
        engine.state() == EngineState::Playing,
        "resume deja el motor Playing (estado={:?})",
        engine.state()
    );
    check!(
        resumed_pos >= paused_pos + Duration::from_millis(1000),
        "la posición avanza tras resume (pos={resumed_pos:?})"
    );

    // ── Fase 3: pause sostiene la posición ─────────────────────────────────
    mark!("pause_hold");
    engine.pause();
    std::thread::sleep(Duration::from_millis(800));
    let hold_1 = engine.position();
    std::thread::sleep(Duration::from_millis(800));
    let hold_2 = engine.position();
    let drift = hold_2.saturating_sub(hold_1);
    check!(
        engine.state() == EngineState::Paused && drift <= Duration::from_millis(100),
        "pausado sostiene la posición (drift={drift:?})"
    );

    // ── Fase 4: recarga del MISMO fichero (replay del frontend) ────────────
    mark!("reload_same");
    engine
        .load_and_play(Path::new(file_a))
        .expect("no se pudo recargar el archivo A");
    let just_reloaded = engine.position();
    check!(
        engine.state() == EngineState::Playing && just_reloaded <= Duration::from_millis(250),
        "replay del mismo fichero reinicia desde ~0 (pos={just_reloaded:?})"
    );
    std::thread::sleep(Duration::from_millis(1200));
    let replay_pos = engine.position();
    check!(
        replay_pos >= Duration::from_millis(900),
        "el replay avanza con normalidad (pos={replay_pos:?})"
    );

    // ── Fase 5: relevo a otro fichero (si se pasó B) ───────────────────────
    if let Some(file_b) = file_b {
        mark!("swap_b");
        let info_b = engine
            .load_and_play(Path::new(file_b))
            .expect("no se pudo cargar el archivo B");
        let just_swapped = engine.position();
        check!(
            engine.state() == EngineState::Playing && just_swapped <= Duration::from_millis(250),
            "el relevo a B carga Playing desde ~0 (pos={just_swapped:?}, duration={:?})",
            info_b.duration
        );
        std::thread::sleep(Duration::from_millis(1200));
        let swap_pos = engine.position();
        check!(
            swap_pos >= Duration::from_millis(900),
            "B avanza con normalidad (pos={swap_pos:?})"
        );
    }

    // ── Fin ─────────────────────────────────────────────────────────────────
    mark!("end");
    engine.stop();
    check!(
        engine.state() == EngineState::Stopped,
        "stop deja el motor Stopped (estado={:?})",
        engine.state()
    );

    println!("RESULT failures={failures}");
    if failures > 0 {
        std::process::exit(1);
    }
}
