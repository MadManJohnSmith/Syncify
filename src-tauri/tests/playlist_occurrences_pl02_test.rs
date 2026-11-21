//! PL02 (#155) — ocurrencias de la misma pista en una playlist local.
//!
//! Cubre la semántica de ocurrencias sobre el modelo de 0110:
//! - añadir una pista que ya está crea una ocurrencia nueva;
//! - quitar una entrada del payload quita UNA ocurrencia, no todas;
//! - reordenar mapea cada entrada a su ocurrencia y persiste 1..N;
//! - `track_count` cuenta ocurrencias, no pistas distintas;
//! - el import de archivo conserva las repeticiones.
//!
//! Todo corre contra el esquema real (`sqlx::migrate!`) en `:memory:` o en un
//! fichero temporal desechable; nunca contra la base personal.

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

/// Aceptación #155: quitar una ocurrencia deja la otra.
#[tokio::test]
async fn quitar_una_ocurrencia_deja_la_otra() {
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
    assert_eq!(
        removed, 1,
        "una entrada del payload quita una sola ocurrencia"
    );
    assert_eq!(
        remaining
            .iter()
            .map(|(_, track, pos)| (*track, *pos))
            .collect::<Vec<_>>(),
        vec![(track_b, 1), (track_a, 2)],
        "queda la otra ocurrencia y las posiciones se recompactan"
    );
    assert_eq!(
        track_count(&pool, playlist_id).await,
        2,
        "track_count sigue contando las ocurrencias que quedan"
    );
}

/// Si el payload repite la pista, cada repetición consume la siguiente
/// ocurrencia: dos entradas quitan dos de las tres.
#[tokio::test]
async fn quitar_la_misma_pista_dos_veces_quita_dos_ocurrencias() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let playlist_id = seed_playlist(&pool, account_id, "Quitar dos").await;
    let track_a = seed_track(&pool, "Track A").await;
    seed_occurrences(&pool, playlist_id, &[track_a, track_a, track_a]).await;

    let app = test_state(pool.clone());
    let state: State<'_, AppState> = app.state::<AppState>();

    let removed = remove_from_playlist(state, playlist_id, vec![track_a, track_a])
        .await
        .expect("remove must succeed");

    let remaining = occurrences(&pool, playlist_id).await;
    assert_eq!(removed, 2);
    assert_eq!(remaining.len(), 1);
    assert_eq!(
        remaining[0].2, 1,
        "la ocurrencia que queda vuelve a la posición 1"
    );
    assert_eq!(track_count(&pool, playlist_id).await, 1);
}

/// Aceptación #155: reordenar con repeticiones persiste el orden pedido y cada
/// entrada del payload mueve su ocurrencia, no cualquier repetición.
#[tokio::test]
async fn reordenar_con_repeticiones_persiste_el_orden_pedido() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let playlist_id = seed_playlist(&pool, account_id, "Reordenar").await;
    let track_a = seed_track(&pool, "Track A").await;
    let track_b = seed_track(&pool, "Track B").await;
    // Orden actual: [A1, B, A2]. Destino: [B, A1, A2].
    seed_occurrences(&pool, playlist_id, &[track_a, track_b, track_a]).await;
    let before = occurrences(&pool, playlist_id).await;

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
        vec![track_b, track_a, track_a],
        "el orden pedido [B, A, A] queda persistido"
    );
    assert_eq!(
        rows.iter().map(|(_, _, pos)| *pos).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "posiciones contiguas 1..N"
    );
    assert_eq!(
        rows.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
        vec![before[1].0, before[0].0, before[2].0],
        "cada entrada mueve su ocurrencia: B, luego A1 y A2 en su orden"
    );
    assert_eq!(track_count(&pool, playlist_id).await, 3);
}

/// Aceptación #155: el orden sobrevive a reabrir la base (reinicio de la app).
#[tokio::test]
async fn el_orden_reordenado_persiste_al_reabrir_la_base() {
    let path = std::env::temp_dir().join(format!(
        "syncify_pl02_{}_{}.sqlite",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let url = format!("sqlite://{}?mode=rwc", path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("file DB must open");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");
    let account_id = seed_account(&pool).await;
    let playlist_id = seed_playlist(&pool, account_id, "Persistente").await;
    let track_a = seed_track(&pool, "Track A").await;
    let track_b = seed_track(&pool, "Track B").await;
    seed_occurrences(&pool, playlist_id, &[track_a, track_b, track_a]).await;

    {
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
    }
    pool.close().await;

    let reopened = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("file DB must reopen");
    let rows = occurrences(&reopened, playlist_id).await;
    assert_eq!(
        rows.iter()
            .map(|(_, track, pos)| (*track, *pos))
            .collect::<Vec<_>>(),
        vec![(track_b, 1), (track_a, 2), (track_a, 3)],
        "el orden pedido sigue ahí tras reabrir"
    );
    reopened.close().await;

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("sqlite-shm"));
    let _ = std::fs::remove_file(path.with_extension("sqlite-wal"));
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
