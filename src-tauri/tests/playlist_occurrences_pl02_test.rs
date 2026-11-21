//! PL02 (#155) — ocurrencias de la misma pista en una playlist local.
//!
//! Hito 1 (caracterización): fotografía lo que el CRUD y el import HACEN HOY
//! sobre una playlist con la misma pista repetida, antes de tocar la semántica
//! de ocurrencias. Los dos últimos tests documentan los huecos conocidos y se
//! reescriben en el hito 2 con la semántica nueva:
//!
//! - `add_to_playlist` ya crea una ocurrencia nueva al repetir una pista.
//! - `import_playlist_from_file_core` ya conserva las repeticiones del archivo.
//! - `remove_from_playlist` borra HOY todas las ocurrencias de la pista.
//! - `reorder_playlist_tracks` pierde HOY el destino de una pista repetida
//!   cuando el payload la mueve al principio.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;
use syncify_tauri_lib::{
    commands::{
        add_to_playlist, import_playlist_from_file_core, remove_from_playlist,
        reorder_playlist_tracks,
    },
    enrichment_worker::EnrichmentWorkerState,
    services::ConcurrencyManager,
    worker::DownloadWorkerState,
    AppState,
};
use tauri::{Manager, State};

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    pool
}

async fn seed_account(pool: &SqlitePool) -> i64 {
    let (service_id,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'spotify'")
        .fetch_one(pool)
        .await
        .unwrap();
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active, credentials_json) \
         VALUES (?, 'Test User', 1, '{}') RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_track(pool: &SqlitePool, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_playlist(pool: &SqlitePool, account_id: i64, name: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(account_id)
    .bind(format!("pl_{}", name.to_lowercase().replace(' ', "_")))
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_occurrences(pool: &SqlitePool, playlist_id: i64, track_ids: &[i64]) {
    for (index, track_id) in track_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind(index as i64 + 1)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn occurrences(pool: &SqlitePool, playlist_id: i64) -> Vec<(i64, i64, i64)> {
    sqlx::query_as(
        "SELECT id, track_id, position FROM playlist_tracks \
         WHERE playlist_id = ? ORDER BY position ASC, id ASC",
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn track_count(pool: &SqlitePool, playlist_id: i64) -> i64 {
    sqlx::query_scalar("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn test_state(pool: SqlitePool) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(ConcurrencyManager::new()),
    });
    app
}

#[tokio::test]
async fn anadir_la_misma_pista_otra_vez_ya_crea_una_ocurrencia() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let playlist_id = seed_playlist(&pool, account_id, "Repetidas").await;
    let track_a = seed_track(&pool, "Track A").await;

    let app = test_state(pool.clone());
    let state: State<'_, AppState> = app.state::<AppState>();

    add_to_playlist(state.clone(), playlist_id, vec![track_a])
        .await
        .expect("first add must succeed");
    add_to_playlist(state.clone(), playlist_id, vec![track_a])
        .await
        .expect("second add must succeed");

    let rows = occurrences(&pool, playlist_id).await;
    assert_eq!(
        rows.iter().map(|(_, track, _)| *track).collect::<Vec<_>>(),
        vec![track_a, track_a],
        "la segunda alta de la misma pista es una ocurrencia nueva"
    );
    assert_eq!(
        rows.iter().map(|(_, _, pos)| *pos).collect::<Vec<_>>(),
        vec![1, 2],
        "las posiciones siguen siendo contiguas"
    );
    assert_eq!(
        track_count(&pool, playlist_id).await,
        2,
        "track_count cuenta ocurrencias"
    );
}

#[tokio::test]
async fn el_import_de_archivo_ya_conserva_las_repeticiones() {
    let pool = migrated_pool().await;
    seed_account(&pool).await;
    let track_a = seed_track(&pool, "One More Time").await;

    let content = "One More Time\nOne More Time\n";
    let result = import_playlist_from_file_core(&pool, "dupes.m3u", content, None, None)
        .await
        .expect("import must succeed");

    let rows = occurrences(&pool, result.playlist_id).await;
    assert_eq!(
        rows.iter().map(|(_, track, _)| *track).collect::<Vec<_>>(),
        vec![track_a, track_a],
        "las dos líneas del archivo enlazan a la misma pista y se conservan"
    );
    assert_eq!(
        track_count(&pool, result.playlist_id).await,
        2,
        "track_count del import cuenta ocurrencias"
    );
}

/// Caracterización del hueco de PL02: hoy quitar una pista borra TODAS sus
/// ocurrencias. El hito 2 invierte esta aserción a «queda la otra».
#[tokio::test]
async fn caracterizacion_quitar_hoy_borra_todas_las_ocurrencias() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let playlist_id = seed_playlist(&pool, account_id, "Quitar").await;
    let track_a = seed_track(&pool, "Track A").await;
    let track_b = seed_track(&pool, "Track B").await;
    seed_occurrences(&pool, playlist_id, &[track_a, track_b, track_a]).await;

    let app = test_state(pool.clone());
    let state: State<'_, AppState> = app.state::<AppState>();

    let removed = remove_from_playlist(state, playlist_id, vec![track_a])
        .await
        .expect("remove must succeed");

    let remaining = occurrences(&pool, playlist_id).await;
    assert_eq!(removed, 2, "caracterización: hoy caen las dos ocurrencias");
    assert_eq!(
        remaining
            .iter()
            .map(|(_, track, _)| *track)
            .collect::<Vec<_>>(),
        vec![track_b],
        "caracterización: no queda ninguna ocurrencia de la pista quitada"
    );
}

/// Caracterización del hueco de PL02: con la pista repetida, el reorden actual
/// reasigna la segunda ocurrencia al hueco negativo y la pista movida al
/// principio acaba al final. El hito 2 lo sustituye por un mapeo
/// ocurrencia-por-ocurrencia del orden completo.
#[tokio::test]
async fn caracterizacion_reordenar_con_repeticiones_hoy_desplaza_la_pista_movida() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let playlist_id = seed_playlist(&pool, account_id, "Reordenar").await;
    let track_a = seed_track(&pool, "Track A").await;
    let track_b = seed_track(&pool, "Track B").await;
    // Orden actual: [A, B, A]. Destino: [B, A, A].
    seed_occurrences(&pool, playlist_id, &[track_a, track_b, track_a]).await;

    let app = test_state(pool.clone());
    let state: State<'_, AppState> = app.state::<AppState>();

    reorder_playlist_tracks(
        state,
        playlist_id,
        vec![
            syncify_tauri_lib::commands::PlaylistTrackPosition {
                track_id: track_b,
                new_position: 1,
            },
            syncify_tauri_lib::commands::PlaylistTrackPosition {
                track_id: track_a,
                new_position: 2,
            },
            syncify_tauri_lib::commands::PlaylistTrackPosition {
                track_id: track_a,
                new_position: 3,
            },
        ],
    )
    .await
    .expect("reorder must succeed");

    let rows = occurrences(&pool, playlist_id).await;
    assert_eq!(
        rows.iter().map(|(_, track, _)| *track).collect::<Vec<_>>(),
        vec![track_a, track_b, track_a],
        "caracterización: el payload pedía [B, A, A] y hoy queda [A, B, A]"
    );
    assert_eq!(
        rows.iter().map(|(_, _, pos)| *pos).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

/// Guarda de humo: el esquema real admite la misma pista dos veces y solo es
/// único por (playlist_id, position); es la base de la que depende PL02.
#[tokio::test]
async fn el_esquema_admite_la_misma_pista_en_posiciones_distintas() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let playlist_id = seed_playlist(&pool, account_id, "Esquema").await;
    let track_a = seed_track(&pool, "Track A").await;

    seed_occurrences(&pool, playlist_id, &[track_a, track_a]).await;

    let duplicates: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ( \
             SELECT track_id FROM playlist_tracks WHERE playlist_id = ? \
             GROUP BY track_id HAVING COUNT(*) > 1)",
    )
    .bind(playlist_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(duplicates, 1);

    let err = sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)",
    )
    .bind(playlist_id)
    .bind(track_a)
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("UNIQUE"),
        "la unicidad es por posición, no por pista: {err}"
    );
}
