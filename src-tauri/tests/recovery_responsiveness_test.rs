//! S222 (#222) — puerta offline: la recuperación de la cola no debe bloquear el
//! runtime async.
//!
//! El síntoma reportado (ticker/IPC congelados ~83 s con audio sonando durante
//! la tormenta de reintentos de Qobuz, evidencia en #221/#222) venía de IO sync
//! dentro del runtime: `is_valid_audio_file` leía el fichero entero con
//! `std::fs::read` y `inspect_physical_audio_file` (metaflac) corría inline en
//! `mark_complete` y en la reconciliación.
//!
//! Este test ejercita la reconciliación del journal sobre ficheros reales
//! grandes en un runtime `current_thread` (el que usa `#[tokio::test]` por
//! defecto) mientras una task de interval cuenta ticks. Si el único hilo del
//! runtime se bloquea en IO sync, los ticks dejan de emitirse y la tasa
//! (ticks/segundo de pared) se hunde; con el fix, el trabajo pesado corre en el
//! pool de blocking y el runtime queda libre para atender el interval.
//!
//! La correspondencia con `player:tick` es directa: el ticker del reproductor
//! es otra task del MISMO runtime, así que lo que congela a este interval lo
//! congela a aquél.

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::services::operation_recovery::reconcile_startup_operations;

/// Tamaño por fichero de prueba: grande a propósito — el coste pre-fix era
/// proporcional al tamaño del fichero leído entero.
const FILE_BYTES: usize = 96 * 1024 * 1024;
const FILES: usize = 6;
/// Interval del ticker-sonda.
const TICK: Duration = Duration::from_millis(5);
/// Tasa mínima de ticks/s para considerar al runtime "respirando". Con el
/// runtime libre la tasa se acerca al máximo del intervalo (~200/s); con el
/// hilo bloqueado en IO sync cae muy por debajo.
const MIN_TICK_RATE: f64 = 150.0;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    for (id, name) in [(1, "spotify"), (3, "tidal")] {
        sqlx::query(
            "INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (?, ?, 1, 'hires')",
        )
        .bind(id)
        .bind(name)
        .execute(&pool)
        .await
        .expect("services baseline");
    }

    pool
}

/// Escribe un fichero FLAC "real": cabecera `fLaC` válida para los checks de
/// magic + relleno. El cuerpo no necesita ser FLAC parseable: lo que ejercita
/// el test es el coste de IO (lectura completa pre-fix, inspección post-fix).
fn write_flac_fixture(path: &std::path::Path) {
    let mut f = std::fs::File::create(path).expect("crear fixture");
    f.write_all(b"fLaC").expect("cabecera fLaC");
    let chunk = vec![0xAAu8; 1024 * 1024];
    for _ in 0..(FILE_BYTES / chunk.len()) {
        f.write_all(&chunk).expect("escribir fixture");
    }
    f.sync_all().ok();
}

#[tokio::test]
async fn recovery_keeps_the_runtime_ticking_during_reconciliation() {
    let dir = tempfile::tempdir().expect("directorio aislado de prueba");
    for i in 0..FILES {
        write_flac_fixture(&dir.path().join(format!("track_{i}.flac")));
    }

    let pool = setup_test_db().await;

    // Filas de journal activas apuntando a los ficheros: cada una pasa por
    // `is_valid_audio_file` y `measured_audio_facts` en el camino Case 1.
    for i in 0..FILES {
        let path = dir.path().join(format!("track_{i}.flac"));
        sqlx::query(
            r#"
            INSERT INTO operation_journal (
                operation_id, operation_type, entity_id, account_id, track_id,
                download_id, provider, phase, attempt, started_at, checkpoint_at,
                status, input_identity, expected_output_path, staging_path,
                file_baseline, db_transaction_state, rollback_state,
                error_taxonomy, retry_policy, result_summary
            ) VALUES (
                ?, 'download_qobuz', NULL, NULL, NULL,
                NULL, 'qobuz', 'transfer', 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP,
                'started', '{}', ?, NULL,
                NULL, NULL, NULL,
                NULL, NULL, NULL
            )
            "#,
        )
        .bind(format!("op-s222-{i}"))
        .bind(path.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("insertar fila de journal");
    }

    let ticks = Arc::new(AtomicUsize::new(0));
    let tick_counter = Arc::clone(&ticks);
    let ticker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(TICK);
        loop {
            interval.tick().await;
            tick_counter.fetch_add(1, Ordering::SeqCst);
        }
    });

    let start = Instant::now();
    let summary = reconcile_startup_operations(&pool, None)
        .await
        .expect("la reconciliación debe completar sin error");
    let elapsed = start.elapsed();
    ticker.abort();

    assert_eq!(
        summary.total_journal_scanned, FILES,
        "todas las filas del journal deben revisarse"
    );

    let secs = elapsed.as_secs_f64().max(0.001);
    let rate = ticks.load(Ordering::SeqCst) as f64 / secs;
    assert!(
        rate >= MIN_TICK_RATE,
        "S222: el runtime dejó de atender tareas durante la recuperación \
         ({rate:.1} ticks/s en {elapsed:?}); IO sync bloqueando el hilo del runtime?"
    );
}
