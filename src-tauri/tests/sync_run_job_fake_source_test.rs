//! Issue #65 — seam 1 del spec (#64): el job de `SyncRun` se prueba contra el
//! proveedor falso en proceso (`FakeLibrarySource`), sin red y con BD
//! `:memory:`. Cubre los estados del contrato (criterio 3 del issue),
//! la cancelación en el límite de página conservando lo importado (criterio 2),
//! el reintento con backoff, el hueco registrado (decisión 17), la cobertura
//! desconocida cuando el proveedor no publica total (decisión 5/27) y la poda
//! del historial a 20 corridas por cuenta (decisión 22).

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use syncify_tauri_lib::commands::progress::{SyncCallback, SyncProgressEmitter};
use syncify_tauri_lib::commands::{ImportPreferences, SyncProgressEvent};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::sync::events::{SyncRunEmitter, SyncRunEvent};
use syncify_tauri_lib::sync::fake::FakeLibrarySource;
use syncify_tauri_lib::sync::job::{self, RunMeta};
use syncify_tauri_lib::sync::runs;
use syncify_tauri_lib::sync::source::{LibrarySource, SourceError, SourceErrorKind, SyncScope};

// ============================================================
// Infraestructura (molde qobuz_sync_arm_simulated_provider_test)
// ============================================================

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

/// Cuenta de prueba sobre el esquema migrado; devuelve (account_id, service_id).
async fn seed_qobuz_account(pool: &sqlx::SqlitePool) -> (i64, i64) {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("services sembrados por migraciones");

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Fake Qobuz', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada");

    (account_id, service_id)
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

/// Ítem con la forma de track de Qobuz que el importador decodifica
/// (mismo fixture que la caracterización de PR #298).
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

// ============================================================
// Colector de eventos (progreso del brazo + eventos de corrida)
// ============================================================

#[derive(Clone, Default)]
struct TestCollector {
    sync_events: Arc<Mutex<Vec<SyncProgressEvent>>>,
    run_events: Arc<Mutex<Vec<SyncRunEvent>>>,
}

impl TestCollector {
    fn new() -> Self {
        Self::default()
    }

    fn run_events(&self) -> Vec<SyncRunEvent> {
        self.run_events.lock().unwrap().clone()
    }
}

impl SyncProgressEmitter for TestCollector {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        self.sync_events.lock().unwrap().push(event.clone());
    }
}

impl SyncRunEmitter for TestCollector {
    fn emit_started(&self, event: &SyncRunEvent) {
        self.run_events.lock().unwrap().push(event.clone());
    }
    fn emit_progress(&self, event: &SyncRunEvent) {
        self.run_events.lock().unwrap().push(event.clone());
    }
    fn emit_finished(&self, event: &SyncRunEvent) {
        self.run_events.lock().unwrap().push(event.clone());
    }
    fn emit_error(&self, event: &SyncRunEvent) {
        self.run_events.lock().unwrap().push(event.clone());
    }
}

/// Envoltorio para pasar `Some(&collector)` como emisor de progreso del brazo
/// sin duplicar el colector: `SyncCallback` también implementa el trait.
fn as_progress_emitter(collector: &TestCollector) -> impl SyncProgressEmitter + '_ {
    SyncCallback(move |event: &SyncProgressEvent| {
        collector.emit_sync_progress(event);
    })
}

// ============================================================
// Arranque de la corrida (lo mismo que hace el comando start_sync_run)
// ============================================================

async fn start_run(pool: &sqlx::SqlitePool, account_id: i64, service_id: i64) -> i64 {
    runs::insert_running_run(
        pool,
        account_id,
        service_id,
        SyncScope::Favorites,
        "import_only",
    )
    .await
    .expect("corrida creada en running")
}

async fn read_run(pool: &sqlx::SqlitePool, run_id: i64) -> runs::SyncRunRecord {
    runs::get_run(pool, run_id)
        .await
        .expect("lectura de corrida")
        .expect("la corrida existe")
}

async fn coverage_of(pool: &sqlx::SqlitePool, run_id: i64) -> (Option<i64>, i64, String) {
    sqlx::query_as(
        "SELECT reported, imported, coverage FROM sync_run_collections WHERE run_id = ? AND collection = 'favorites'",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .expect("cobertura de favoritos")
}

async fn run_errors(pool: &sqlx::SqlitePool, run_id: i64) -> Vec<String> {
    sqlx::query_as::<_, (String,)>(
        "SELECT message FROM sync_run_errors WHERE run_id = ? ORDER BY id",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .expect("huecos leídos")
    .into_iter()
    .map(|(m,)| m)
    .collect()
}

// ============================================================
// Pruebas
// ============================================================

/// Recorrido feliz: dos páginas (2 + 1 ítems) recorridas por el job contra el
/// proveedor falso, sin red. La corrida queda `completed` con su cobertura
/// `imported`, las marcas de tiempo cerradas y la secuencia de eventos
/// started → progreso → finished (criterios 1, 5 y 6 del issue).
#[tokio::test]
async fn corrida_feliz_completa_con_cobertura_importada() {
    let pool = setup_test_db().await;
    let (account_id, service_id) = seed_qobuz_account(&pool).await;
    let run_id = start_run(&pool, account_id, service_id).await;

    let source = FakeLibrarySource::with_pages(vec![
        vec![
            favorite_track(42_001, "Fake One", "QZFAKE00000001"),
            favorite_track(42_002, "Fake Two", "QZFAKE00000002"),
        ],
        vec![favorite_track(42_003, "Fake Three", "QZFAKE00000003")],
    ])
    .with_service_name("qobuz")
    .with_reported_total(Some(3));
    // Con tamaño de página 1 declarado por el adaptador el job recorre las
    // tres páginas; la paginación la declara el adaptador (seam 2).
    let wrapped = WithPageSize {
        inner: &source,
        page_size: 1,
    };

    let collector = TestCollector::new();
    let progress = as_progress_emitter(&collector);

    let meta = RunMeta {
        run_id,
        account_id,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let cancel = job::register_live_run(run_id);
    let result = job::run_favorites_job(
        &pool,
        &meta,
        service_id,
        &wrapped,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("la corrida completa sin huecos");

    assert_eq!(result.status, "completed");
    assert_eq!(result.reported_total, Some(3));
    assert_eq!(result.imported_total, 3, "los 3 ítems importados");
    assert_eq!(result.error_count, 0);

    // Recorrido: el job pidió exactamente las páginas del adaptador.
    assert_eq!(source.offsets_seen(), vec![0, 1, 2]);

    // La fila de la corrida quedó con estado y marcas de tiempo.
    let run = read_run(&pool, run_id).await;
    assert_eq!(run.status, "completed");
    assert_eq!(run.reported_total, Some(3));
    assert_eq!(run.imported_total, 3);
    assert_eq!(run.error_count, 0);
    assert!(
        run.finished_at.is_some(),
        "la corrida cerrada tiene finished_at"
    );

    // Cobertura por colección: eje separado, `imported` porque cuadra.
    let (reported, imported, coverage) = coverage_of(&pool, run_id).await;
    assert_eq!(reported, Some(3));
    assert_eq!(imported, 3);
    assert_eq!(coverage, "imported");

    // Lo importado es el mismo resultado del importador real: 3 pistas y 3
    // entradas de biblioteca.
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 3);
    let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entries, 3);

    // Eventos: started, progreso de fase, y cierre terminal.
    let events = collector.run_events();
    assert_eq!(
        events[0].phase, "started",
        "el primer evento de corrida es el arranque"
    );
    assert!(!events[0].terminal);
    let progress_events: Vec<&SyncRunEvent> = events
        .iter()
        .filter(|e| e.phase == "importing_favorite_tracks")
        .collect();
    assert!(
        progress_events.len() >= 3,
        "un evento de progreso por ítem visto: {}",
        progress_events.len()
    );
    assert_eq!(progress_events[0].seen, 1);
    let terminal = events.last().expect("evento terminal");
    assert!(terminal.terminal);
    assert_eq!(terminal.status, "completed");
    assert_eq!(terminal.seen, 3);
    assert_eq!(terminal.imported, 3);
    assert_eq!(terminal.reported_total, Some(3));
}

/// Cancelación en el límite de página: la bandera se marca tras servir la
/// primera página, la siguiente no se pide y lo importado se conserva
/// (criterio 2 del issue; regla derivada del ADR-0006).
#[tokio::test]
async fn cancelar_detiene_en_el_limite_de_pagina_y_conserva_lo_importado() {
    let pool = setup_test_db().await;
    let (account_id, service_id) = seed_qobuz_account(&pool).await;
    let run_id = start_run(&pool, account_id, service_id).await;

    // La bandera es la del registro vivo: la misma que el job atiende.
    let cancel = job::register_live_run(run_id);
    let flag_for_hook = cancel.clone();
    let source = FakeLibrarySource::with_pages(vec![
        vec![favorite_track(42_001, "Keep One", "QZCAN00000001")],
        vec![favorite_track(42_002, "Never Fetched", "QZCAN00000002")],
        vec![favorite_track(42_003, "Never Fetched", "QZCAN00000003")],
    ])
    .with_service_name("qobuz")
    .with_reported_total(Some(3))
    .on_page_served(move |index| {
        if index == 0 {
            // Cancelación externa mientras se procesaba la primera página.
            flag_for_hook.store(true, Ordering::Relaxed);
        }
    });
    let wrapped = WithPageSize {
        inner: &source,
        page_size: 1,
    };

    let collector = TestCollector::new();
    let progress = as_progress_emitter(&collector);
    let meta = RunMeta {
        run_id,
        account_id,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let result = job::run_favorites_job(
        &pool,
        &meta,
        service_id,
        &wrapped,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("la cancelación no es un fallo");

    // El recorrido se detuvo: solo se pidió la primera página.
    assert_eq!(source.offsets_seen(), vec![0]);

    assert_eq!(result.status, "cancelled");
    assert_eq!(
        result.imported_total, 1,
        "lo importado de la página completa"
    );

    let run = read_run(&pool, run_id).await;
    assert_eq!(run.status, "cancelled", "criterio 3: corrida cancelada");
    assert_eq!(run.imported_total, 1);
    assert!(run.finished_at.is_some());

    // Lo importado se conserva en la biblioteca: 1 pista, no se revierte.
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 1, "la pista de la página completa sobrevive");

    // Cobertura parcial visible: 1 de 3 con total reportado.
    let (reported, imported, coverage) = coverage_of(&pool, run_id).await;
    assert_eq!(reported, Some(3));
    assert_eq!(imported, 1);
    assert_eq!(coverage, "reported");

    // El evento terminal llegó como cancelled.
    let terminal = collector.run_events().last().expect("terminal").clone();
    assert!(terminal.terminal);
    assert_eq!(terminal.status, "cancelled");
    assert_eq!(terminal.imported, 1);
}

/// Credencial inválida (401/403 → `AuthExpired`): la corrida queda `failed`
/// (criterio 3), el error queda registrado y la cuenta se marca para
/// reautenticar (decisión 20 del ADR-0006).
#[tokio::test]
async fn credencial_invalida_deja_la_corrida_failed() {
    let pool = setup_test_db().await;
    let (account_id, service_id) = seed_qobuz_account(&pool).await;
    let run_id = start_run(&pool, account_id, service_id).await;

    let source = FakeLibrarySource::with_pages(vec![vec![favorite_track(
        42_001,
        "Never Imported",
        "QZAUTH00000001",
    )]])
    .with_service_name("qobuz")
    .with_reported_total(Some(1))
    .with_failure(SourceError::auth_expired(
        "Qobuz API error (401 Unauthorized): user authentication required",
    ));

    let collector = TestCollector::new();
    let progress = as_progress_emitter(&collector);
    let meta = RunMeta {
        run_id,
        account_id,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let cancel = job::register_live_run(run_id);
    let result = job::run_favorites_job(
        &pool,
        &meta,
        service_id,
        &source,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("AuthExpired no es un error de entorno del job");

    assert_eq!(result.status, "failed");

    let run = read_run(&pool, run_id).await;
    assert_eq!(run.status, "failed");
    assert_eq!(run.imported_total, 0, "nada se importó");
    assert_eq!(run.error_count, 1);

    // El hueco queda registrado con su colección.
    let errors = run_errors(&pool, run_id).await;
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].contains("401") || errors[0].contains("Unauthorized"),
        "el error registra el rechazo de sesión: {}",
        errors[0]
    );

    // Nada se importó.
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 0);

    // La cuenta quedó marcada para reautenticar (decisión 20).
    let (invalid, reason): (i64, Option<String>) =
        sqlx::query_as("SELECT credentials_invalid, invalid_reason FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("estado de credencial");
    assert_eq!(invalid, 1, "la credencial queda marcada inválida");
    assert!(reason.is_some(), "el motivo del rechazo queda registrado");

    // El evento terminal llegó por el canal de error.
    let terminal = collector.run_events().last().expect("terminal").clone();
    assert!(terminal.terminal);
    assert_eq!(terminal.status, "failed");
}

/// Página que persiste el fallo (`Permanent`): se registra el hueco, la
/// corrida continúa con lo ya importado y queda `partial` (decisiones 17 y 36
/// del spec; criterio 3 del issue).
#[tokio::test]
async fn hueco_en_pagina_deja_la_corrida_partial_con_lo_importado() {
    let pool = setup_test_db().await;
    let (account_id, service_id) = seed_qobuz_account(&pool).await;
    let run_id = start_run(&pool, account_id, service_id).await;

    let source = FakeLibrarySource::with_pages(vec![
        vec![
            favorite_track(42_001, "First Page One", "QZPART00000001"),
            favorite_track(42_002, "First Page Two", "QZPART00000002"),
        ],
        vec![
            favorite_track(42_003, "Gap Page", "QZPART00000003"),
            favorite_track(42_004, "Gap Page", "QZPART00000004"),
        ],
    ])
    .with_service_name("qobuz")
    .with_reported_total(Some(4))
    // La primera petición entra; la segunda persiste el fallo.
    .with_failure_at(
        1,
        SourceError::permanent("Qobuz API error (422 Unprocessable Entity): boom"),
    );

    // Página de 2 declarada por el adaptador: la segunda petición falla.
    let wrapped = WithPageSize {
        inner: &source,
        page_size: 2,
    };

    let collector = TestCollector::new();
    let progress = as_progress_emitter(&collector);
    let meta = RunMeta {
        run_id,
        account_id,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let cancel = job::register_live_run(run_id);
    let result = job::run_favorites_job(
        &pool,
        &meta,
        service_id,
        &wrapped,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("la corrida termina con hueco, no con error de entorno");

    assert_eq!(result.status, "partial");
    assert_eq!(result.imported_total, 2, "solo la primera página entró");
    assert_eq!(result.error_count, 1);

    let run = read_run(&pool, run_id).await;
    assert_eq!(run.status, "partial");
    let (reported, imported, coverage) = coverage_of(&pool, run_id).await;
    assert_eq!(reported, Some(4));
    assert_eq!(imported, 2);
    assert_eq!(
        coverage, "reported",
        "hay huecos: no se presenta como completo"
    );

    let errors = run_errors(&pool, run_id).await;
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0].contains("422"),
        "el hueco conserva el error crudo"
    );

    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks, 2, "lo importado antes del hueco se conserva");
}

/// Un fallo `Retryable` se reintenta con backoff y la corrida completa
/// (decisión 17): la misma página se pide dos veces en el mismo offset.
#[tokio::test]
async fn fallo_transitorio_se_reintenta_y_la_corrida_completa() {
    let pool = setup_test_db().await;
    let (account_id, service_id) = seed_qobuz_account(&pool).await;
    let run_id = start_run(&pool, account_id, service_id).await;

    let source = FakeLibrarySource::with_pages(vec![vec![favorite_track(
        42_001,
        "Retried One",
        "QZRETR00000001",
    )]])
    .with_service_name("qobuz")
    .with_reported_total(Some(1))
    .with_failure(SourceError::retryable(
        "Qobuz API error (503 Service Unavailable): transient",
    ));

    let collector = TestCollector::new();
    let progress = as_progress_emitter(&collector);
    let meta = RunMeta {
        run_id,
        account_id,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let cancel = job::register_live_run(run_id);
    let result = job::run_favorites_job(
        &pool,
        &meta,
        service_id,
        &source,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("el reintento supera el fallo transitorio");

    assert_eq!(result.status, "completed");
    // El reintento repite el offset: [0 (falla), 0 (éxito)].
    assert_eq!(source.offsets_seen(), vec![0, 0]);

    let run = read_run(&pool, run_id).await;
    assert_eq!(run.status, "completed");
    assert_eq!(run.imported_total, 1);
    assert_eq!(run.error_count, 0);
}

/// Total ausente: el proveedor no publica total y la cobertura es
/// «desconocida», no un porcentaje inventado (decisión 27 del spec).
#[tokio::test]
async fn total_ausente_deja_la_cobertura_desconocida() {
    let pool = setup_test_db().await;
    let (account_id, service_id) = seed_qobuz_account(&pool).await;
    let run_id = start_run(&pool, account_id, service_id).await;

    let source = FakeLibrarySource::with_pages(vec![vec![favorite_track(
        42_001,
        "Unknown Total",
        "QZUNKW00000001",
    )]])
    .with_service_name("qobuz")
    .with_reported_total(None);

    let collector = TestCollector::new();
    let progress = as_progress_emitter(&collector);
    let meta = RunMeta {
        run_id,
        account_id,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let cancel = job::register_live_run(run_id);
    let result = job::run_favorites_job(
        &pool,
        &meta,
        service_id,
        &source,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("corrida sin total reportado completa");

    assert_eq!(result.status, "completed");
    assert_eq!(result.reported_total, None);

    let (reported, imported, coverage) = coverage_of(&pool, run_id).await;
    assert_eq!(reported, None);
    assert_eq!(imported, 1);
    assert_eq!(coverage, "unknown", "un total ausente no es un 100 %");
}

/// Historial: se conservan las últimas 20 corridas por cuenta (decisión 22).
#[tokio::test]
async fn el_historial_se_poda_a_las_ultimas_20_por_cuenta() {
    let pool = setup_test_db().await;
    let (account_id, service_id) = seed_qobuz_account(&pool).await;

    // 25 corridas viejas sembradas con marcas de tiempo crecientes.
    for i in 0..25 {
        sqlx::query(
            "INSERT INTO sync_runs (account_id, service_id, mode, scope, status, started_at, finished_at)
             VALUES (?, ?, 'import_only', 'favorites', 'completed', ?, ?)",
        )
        .bind(account_id)
        .bind(service_id)
        .bind(format!("2025-01-01 00:{:02}:00", i))
        .bind(format!("2025-01-01 00:{:02}:30", i))
        .execute(&pool)
        .await
        .expect("corrida vieja sembrada");
    }

    let run_id = start_run(&pool, account_id, service_id).await;
    let source = FakeLibrarySource::with_pages(vec![vec![favorite_track(
        42_001,
        "Pruned",
        "QZPRUN00000001",
    )]])
    .with_service_name("qobuz")
    .with_reported_total(Some(1));

    let collector = TestCollector::new();
    let progress = as_progress_emitter(&collector);
    let meta = RunMeta {
        run_id,
        account_id,
        service: "qobuz".to_string(),
        scope: "favorites".to_string(),
    };
    let cancel = job::register_live_run(run_id);
    job::run_favorites_job(
        &pool,
        &meta,
        service_id,
        &source,
        &prefs_solo_favoritos(),
        Some(&progress),
        &collector,
        cancel,
    )
    .await
    .expect("corrida con poda");

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_runs WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        total <= runs::KEEP_RUNS_PER_ACCOUNT,
        "el historial queda en las {} más recientes: hay {}",
        runs::KEEP_RUNS_PER_ACCOUNT,
        total
    );
    // La corrida nueva siempre sobrevive a su propia poda.
    assert!(runs::get_run(&pool, run_id).await.unwrap().is_some());
}

/// Envoltorio de tamaño de página: el job no conoce offsets concretos y el
/// adaptador declara su cursor; el proveedor falso lo sobreescribe para que
/// los tests recorran varias páginas con pocos ítems.
struct WithPageSize<'a> {
    inner: &'a FakeLibrarySource,
    page_size: i64,
}

impl LibrarySource for WithPageSize<'_> {
    fn service_name(&self) -> &str {
        self.inner.service_name()
    }

    fn favorites_page_size(&self) -> i64 {
        self.page_size
    }

    async fn fetch_favorites_page(
        &self,
        offset: i64,
    ) -> Result<syncify_tauri_lib::sync::source::FavoritesPage, SourceError> {
        self.inner.fetch_favorites_page(offset).await
    }
}

/// Sanidad de la taxonomía de errores del adaptador: los tipos existen y el
/// `Unsupported` es declara capacidad ausente (decisión 7 del ADR-0006).
#[test]
fn la_taxonomia_de_errores_mantiene_los_tipos_del_contrato() {
    assert_eq!(SourceError::retryable("x").kind, SourceErrorKind::Retryable);
    assert_eq!(
        SourceError::auth_expired("x").kind,
        SourceErrorKind::AuthExpired
    );
    assert_eq!(
        SourceError::unsupported("x").kind,
        SourceErrorKind::Unsupported
    );
    assert_eq!(SourceError::permanent("x").kind, SourceErrorKind::Permanent);
    assert_eq!(SyncScope::parse("favorites"), Some(SyncScope::Favorites));
    assert_eq!(SyncScope::parse("cualquier-cosa"), None);
}
