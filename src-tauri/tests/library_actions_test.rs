//! Acciones por lotes de la biblioteca — L05 (issue #102, spec #96).
//!
//! Fixture determinista sobre SQLite `:memory:` recién migrada (el mismo
//! patrón de la serie L01-L04). La costura probada es el núcleo con el pool,
//! no el comando de Tauri.
//!
//! Cobertura de los criterios del issue:
//! - «todo el filtro» usa el total, no la página (`limit` 2 frente a 6);
//! - los ids explícitos son literales, se deduplican y respetan exclusión;
//! - la vista previa da elegibles y omisiones por acción;
//! - el recuento previo coincide con el ejecutado en fixture estable;
//! - descarga, favorito y playlist son idempotentes;
//! - un bloque fallido no impide el siguiente y el resumen lo refleja;
//! - `library-action-progress`/`library-action-finished` son agregados;
//! - un cambio de biblioteca entre preview y run se recalcula;
//! - las acciones destructivas se rechazan: son por pista.

use std::sync::{Arc, Mutex};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::library_actions::{
    preview_library_action_inner, run_library_action_inner, LibraryActionCallback,
    LibraryActionPreview, LibraryActionProgress, LibraryActionSummary, LibraryBatchAction,
    LibrarySelection, LIBRARY_ACTION_BLOCK_SIZE,
};
use syncify_tauri_lib::commands::library_query::LibraryQuery;

async fn fresh_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

/// Fixture de la serie:
/// - servicios: 1=spotify (sin descarga), 2=qobuz, 3=tidal;
/// - cuentas: 1=spotify, 2=qobuz;
/// - T1 favorita, descargada de qobuz y ya en la playlist local 1;
/// - T2 qobuz disponible, sin descargar;
/// - T3 tidal, ya en la cola;
/// - T4 solo spotify: sin fuente descargable;
/// - T5 qobuz disponible y ya favorita;
/// - T6 qobuz disponible.
/// Todas tienen pertenencia viva (`library_entries` de la cuenta 2).
async fn seed() -> SqlitePool {
    let pool = fresh_pool().await;

    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email) VALUES \
         (1, 1, 'A1', 'a1@x'), (2, 2, 'A2', 'a2@x')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Alpha Band')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Nightfall')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, is_favorite, created_at) VALUES \
         (1, 'Sunrise', 1, 1, '2025-12-01T09:00:00Z'), \
         (2, 'Island', NULL, 0, '2025-12-01T09:00:00Z'), \
         (3, 'River', NULL, 0, '2025-12-01T09:00:00Z'), \
         (4, 'Ghost', NULL, 0, '2025-12-01T09:00:00Z'), \
         (5, 'Echoes', NULL, 1, '2025-12-01T09:00:00Z'), \
         (6, 'Storm', NULL, 0, '2025-12-01T09:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) \
         SELECT id, 1, 'primary' FROM tracks",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_sources \
         (track_id, service_id, service_track_id, format, available, availability_status) VALUES \
         (1, 2, 'qb-1', 'FLAC', 1, 'available'), \
         (2, 2, 'qb-2', 'FLAC', 1, 'available'), \
         (3, 3, 'td-3', 'FLAC', 1, 'available'), \
         (4, 1, 'sp-4', 'MP3', 1, 'available'), \
         (5, 2, 'qb-5', 'FLAC', 1, 'available'), \
         (6, 2, 'qb-6', 'FLAC', 1, 'available')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_entries (account_id, track_id, added_at, is_liked) VALUES \
         (2, 1, '2026-01-03T10:00:00Z', 1), \
         (2, 2, NULL, 0), \
         (2, 3, NULL, 0), \
         (2, 4, NULL, 0), \
         (2, 5, NULL, 1), \
         (2, 6, NULL, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format) \
         VALUES (1, 2, '/tmp/l05-fixture.flac', 'FLAC')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (3, 'queued')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES \
         (1, NULL, 'local_l05', 'Local list'), (2, 2, 'pl-l05', 'Account list')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (1, 1, 1)")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

fn filter_selection(limit: Option<i64>, excluded_ids: Vec<i64>) -> LibrarySelection {
    LibrarySelection::Filter {
        query: LibraryQuery {
            limit,
            ..Default::default()
        },
        excluded_ids,
    }
}

fn favorite_action(favorite: bool, account_id: Option<i64>) -> LibraryBatchAction {
    LibraryBatchAction::SetFavorite {
        favorite,
        account_id,
    }
}

fn download_action() -> LibraryBatchAction {
    LibraryBatchAction::QueueDownload {
        priority: Some(50),
        quality_preference: Some("hires".to_string()),
        allow_fallback: Some(true),
    }
}

fn noop_progress(_: &LibraryActionProgress) {}
fn noop_finished(_: &LibraryActionSummary) {}

async fn run_preview(
    pool: &SqlitePool,
    selection: &LibrarySelection,
    action: &LibraryBatchAction,
) -> LibraryActionPreview {
    preview_library_action_inner(pool, selection, action)
        .await
        .expect("preview")
}

async fn run_action(
    pool: &SqlitePool,
    selection: LibrarySelection,
    action: LibraryBatchAction,
    expected: Option<(i64, i64)>,
) -> LibraryActionSummary {
    let emitter = LibraryActionCallback(
        noop_progress as fn(&LibraryActionProgress),
        noop_finished as fn(&LibraryActionSummary),
    );
    run_library_action_inner(
        pool,
        selection,
        action,
        expected.map(|(total, _)| total),
        expected.map(|(_, eligible)| eligible),
        &emitter,
    )
    .await
    .expect("run")
}

fn omission_count(preview: &LibraryActionPreview, reason: &str) -> i64 {
    preview
        .omissions
        .iter()
        .find(|omission| omission.reason == reason)
        .map_or(0, |omission| omission.count)
}

#[tokio::test]
async fn select_all_filter_uses_the_filter_total_not_the_loaded_page() {
    let pool = seed().await;

    // La página pedida trae 2 filas, pero la selección «todo el filtro» son 6.
    let preview = run_preview(
        &pool,
        &filter_selection(Some(2), Vec::new()),
        &favorite_action(true, None),
    )
    .await;
    assert_eq!(preview.selection_mode, "filter");
    assert_eq!(
        preview.total, 6,
        "«seleccionar todo» no es la página cargada"
    );
    assert_eq!(preview.eligible, 4);
    assert_eq!(omission_count(&preview, "already_favorite"), 2);

    // Exclusiones: fuera la 2 y la 3.
    let excluded = run_preview(
        &pool,
        &filter_selection(Some(2), vec![2, 3]),
        &favorite_action(true, None),
    )
    .await;
    assert_eq!(excluded.total, 4);
    assert_eq!(excluded.eligible, 2, "2 y 3 eran elegibles y ya no están");
    assert_eq!(omission_count(&excluded, "already_favorite"), 2);
}

#[tokio::test]
async fn explicit_ids_are_literal_deduplicated_and_not_a_filter() {
    let pool = seed().await;

    // 1 es favorita y está descargada; 2 es elegible; 999 no existe; el 2
    // repetido no cuenta dos veces.
    let preview = run_preview(
        &pool,
        &LibrarySelection::Ids {
            ids: vec![1, 2, 2, 999],
        },
        &download_action(),
    )
    .await;
    assert_eq!(preview.selection_mode, "ids");
    assert_eq!(preview.total, 3);
    assert_eq!(preview.eligible, 1);
    assert_eq!(omission_count(&preview, "already_downloaded"), 1);
    assert_eq!(omission_count(&preview, "not_in_library"), 1);
}

#[tokio::test]
async fn preview_reports_eligibility_and_omission_reasons_per_action() {
    let pool = seed().await;

    let download = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &download_action(),
    )
    .await;
    assert_eq!(download.total, 6);
    assert_eq!(download.eligible, 3);
    assert_eq!(download.omitted, 3);
    assert_eq!(omission_count(&download, "already_downloaded"), 1);
    assert_eq!(omission_count(&download, "already_queued"), 1);
    assert_eq!(omission_count(&download, "no_downloadable_source"), 1);

    let favorite = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &favorite_action(true, None),
    )
    .await;
    assert_eq!(favorite.eligible, 4);
    assert_eq!(omission_count(&favorite, "already_favorite"), 2);

    let playlist = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &LibraryBatchAction::AddToPlaylist { playlist_id: 1 },
    )
    .await;
    assert_eq!(playlist.eligible, 5);
    assert_eq!(omission_count(&playlist, "already_in_playlist"), 1);
}

#[tokio::test]
async fn preview_count_matches_run_and_favorite_membership_is_idempotent() {
    let pool = seed().await;

    let preview = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &favorite_action(true, Some(2)),
    )
    .await;
    assert_eq!(preview.eligible, 4);

    let first = run_action(
        &pool,
        filter_selection(None, Vec::new()),
        favorite_action(true, Some(2)),
        Some((preview.total, preview.eligible)),
    )
    .await;
    assert_eq!(first.total, 6);
    assert_eq!(first.processed, 6);
    assert_eq!(
        first.succeeded, 4,
        "el recuento previo coincide con el ejecutado"
    );
    assert_eq!(first.failed, 0);
    assert_eq!(first.skipped, 2);
    assert!(!first.recalculated);

    let memberships: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM library_memberships \
         WHERE account_id = 2 AND collection_type = 'favorite'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(memberships, 4);

    // Repetir no duplica la pertenencia ni vuelve a "acertar" pistas.
    let preview_again = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &favorite_action(true, Some(2)),
    )
    .await;
    assert_eq!(preview_again.eligible, 0);
    let second = run_action(
        &pool,
        filter_selection(None, Vec::new()),
        favorite_action(true, Some(2)),
        Some((preview_again.total, preview_again.eligible)),
    )
    .await;
    assert_eq!(second.succeeded, 0);
    assert_eq!(second.skipped, 6);
    let memberships_again: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM library_memberships \
         WHERE account_id = 2 AND collection_type = 'favorite'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(memberships_again, 4, "la pertenencia no se duplica");

    // Desmarcar es la operación inversa y también es idempotente.
    let unfavorite = run_action(
        &pool,
        LibrarySelection::Ids { ids: vec![2, 6] },
        favorite_action(false, Some(2)),
        None,
    )
    .await;
    assert_eq!(unfavorite.succeeded, 2);
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM library_memberships \
         WHERE account_id = 2 AND collection_type = 'favorite'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 2);
}

#[tokio::test]
async fn download_queue_and_playlist_actions_do_not_duplicate_entries() {
    let pool = seed().await;

    // Descarga: la cola ya tenía T3; T1 descargada y T4 sin fuente se omiten.
    let preview = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &download_action(),
    )
    .await;
    let first = run_action(
        &pool,
        filter_selection(None, Vec::new()),
        download_action(),
        Some((preview.total, preview.eligible)),
    )
    .await;
    assert_eq!(first.succeeded, 3);
    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        queued, 4,
        "una entrada nueva por pista elegible, ninguna más"
    );

    let with_source: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM download_queue \
         WHERE track_id IN (2, 5, 6) AND service_name = 'qobuz' \
           AND service_track_id IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(with_source, 3, "la identidad de fuente se resuelve en SQL");

    let second = run_action(
        &pool,
        filter_selection(None, Vec::new()),
        download_action(),
        None,
    )
    .await;
    assert_eq!(
        second.succeeded, 0,
        "repetir la descarga usa la cola existente"
    );
    let queued_again: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued_again, 4);

    // Playlist: 5 elegibles, T1 ya estaba. Repetir no añade ni duplica.
    let playlist_preview = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &LibraryBatchAction::AddToPlaylist { playlist_id: 1 },
    )
    .await;
    let playlist_first = run_action(
        &pool,
        filter_selection(None, Vec::new()),
        LibraryBatchAction::AddToPlaylist { playlist_id: 1 },
        Some((playlist_preview.total, playlist_preview.eligible)),
    )
    .await;
    assert_eq!(playlist_first.succeeded, 5);
    let playlist_second = run_action(
        &pool,
        filter_selection(None, Vec::new()),
        LibraryBatchAction::AddToPlaylist { playlist_id: 1 },
        None,
    )
    .await;
    assert_eq!(playlist_second.succeeded, 0);

    let rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 6);
    let duplicates: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ( \
             SELECT track_id FROM playlist_tracks WHERE playlist_id = 1 \
             GROUP BY track_id HAVING COUNT(*) > 1)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(duplicates, 0, "la playlist no duplica pistas");
    let count: i64 = sqlx::query_scalar("SELECT track_count FROM playlists WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 6);
}

/// Fixture de 501 pistas para ejercitar el corte de bloque (500 + 1).
async fn seed_bulk_tracks() -> SqlitePool {
    let pool = fresh_pool().await;
    sqlx::query("INSERT INTO accounts (id, service_id, display_name) VALUES (2, 2, 'A2')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 501) \
         INSERT INTO tracks (id, title, created_at) \
         SELECT n, 'Bulk ' || n, '2025-12-01T09:00:00Z' FROM seq",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 501) \
         INSERT INTO track_sources \
         (track_id, service_id, service_track_id, format, available, availability_status) \
         SELECT n, 2, 'qb-bulk-' || n, 'FLAC', 1, 'available' FROM seq",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 501) \
         INSERT INTO library_entries (account_id, track_id, added_at, is_liked) \
         SELECT 2, n, NULL, 0 FROM seq",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

#[tokio::test]
async fn failed_block_does_not_abort_the_next_block_and_events_are_aggregated() {
    let pool = seed_bulk_tracks().await;
    sqlx::query(
        "CREATE TRIGGER force_block_failure BEFORE INSERT ON download_queue \
         WHEN NEW.track_id = 250 \
         BEGIN SELECT RAISE(ABORT, 'forced block failure'); END",
    )
    .execute(&pool)
    .await
    .unwrap();

    let preview = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &download_action(),
    )
    .await;
    assert_eq!(preview.total, 501);
    assert_eq!(preview.eligible, 501);

    let progress = Arc::new(Mutex::new(Vec::new()));
    let finished = Arc::new(Mutex::new(Vec::new()));
    let progress_sink = Arc::clone(&progress);
    let finished_sink = Arc::clone(&finished);
    let emitter: LibraryActionCallback<_, _> = LibraryActionCallback(
        move |event: &LibraryActionProgress| {
            progress_sink.lock().unwrap().push(event.clone());
        },
        move |event: &LibraryActionSummary| {
            finished_sink.lock().unwrap().push(event.clone());
        },
    );

    let summary = run_library_action_inner(
        &pool,
        filter_selection(None, Vec::new()),
        download_action(),
        Some(preview.total),
        Some(preview.eligible),
        &emitter,
    )
    .await
    .expect("run");

    assert_eq!(summary.total, 501);
    assert_eq!(summary.processed, 501);
    assert_eq!(summary.succeeded, 1, "el segundo bloque sí se ejecuta");
    assert_eq!(summary.failed, 500, "el primer bloque se revierte entero");
    assert_eq!(summary.blocks, 2);
    assert_eq!(summary.blocks_failed, 1);
    assert_eq!(summary.errors.len(), 1);
    assert_eq!(summary.errors[0].tracks, 500);
    assert!(!summary.recalculated);

    let queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 1, "solo el bloque que no falló dejó su entrada");

    // 501 pistas no producen 501 eventos: uno por bloque (2) y uno final.
    let progress = progress.lock().unwrap();
    assert_eq!(progress.len(), 2, "un evento agregado por bloque");
    assert_eq!(progress[0].processed, 500);
    assert_eq!(progress[0].failed, 500);
    assert_eq!(progress[1].processed, 501);
    assert_eq!(progress[1].succeeded, 1);
    assert_eq!(progress[0].total, 501);
    assert_eq!(progress[0].blocks_total, 2);
    assert_eq!(finished.lock().unwrap().len(), 1);
    assert_eq!(LIBRARY_ACTION_BLOCK_SIZE, 500);
}

#[tokio::test]
async fn library_change_between_preview_and_run_is_recalculated() {
    let pool = seed().await;
    let preview = run_preview(
        &pool,
        &filter_selection(None, Vec::new()),
        &favorite_action(true, None),
    )
    .await;
    assert_eq!(preview.total, 6);
    assert_eq!(preview.eligible, 4);

    // El sync añade una pista nueva con pertenencia antes de ejecutar.
    sqlx::query("INSERT INTO tracks (id, title, is_favorite) VALUES (7, 'New', 0)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (2, 7, 0)")
        .execute(&pool)
        .await
        .unwrap();

    let summary = run_action(
        &pool,
        filter_selection(None, Vec::new()),
        favorite_action(true, None),
        Some((preview.total, preview.eligible)),
    )
    .await;
    assert_eq!(summary.total, 7, "el servidor recalcula la selección");
    assert_eq!(summary.processed, 7);
    assert_eq!(summary.eligible, 5);
    assert_eq!(summary.succeeded, 5);
    assert!(summary.recalculated, "la UI debe actualizar el resumen");
}

#[tokio::test]
async fn destructive_actions_are_rejected_as_per_track_only() {
    let pool = seed().await;
    let emitter = LibraryActionCallback(
        noop_progress as fn(&LibraryActionProgress),
        noop_finished as fn(&LibraryActionSummary),
    );

    for action in [
        LibraryBatchAction::DeleteAudio,
        LibraryBatchAction::Restore,
        LibraryBatchAction::QuarantineTouch,
    ] {
        let error =
            preview_library_action_inner(&pool, &filter_selection(None, Vec::new()), &action)
                .await
                .expect_err("no se ofrece en «seleccionar todo»");
        assert!(error.contains("por pista"), "{error}");

        let error = run_library_action_inner(
            &pool,
            LibrarySelection::Ids { ids: vec![1] },
            action,
            None,
            None,
            &emitter,
        )
        .await
        .expect_err("tampoco por lotes de ids");
        assert!(error.contains("por pista"), "{error}");
    }
}
