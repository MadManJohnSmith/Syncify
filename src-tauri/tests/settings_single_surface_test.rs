//! SB01 (#199): cada dato en una sola superficie.
//!
//! El límite concurrente vive en `sync_settings.max_concurrent_downloads` y
//! la ruta de descarga en `folder_settings.base_folder`. Ningún guardado
//! deja copias en `advanced_settings` ni en la KV `settings`; las lecturas
//! toleran restos legacy de versiones anteriores.

use std::sync::Arc;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::settings::{
    perform_get_download_settings, perform_save_download_settings, perform_update_folder_settings,
    perform_update_sync_settings, resolve_effective_download_paths,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::models::{FolderSettings, SyncSettings};
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tempfile::TempDir;

async fn setup_app_state(initial_concurrency: usize) -> (AppState, SqlitePool, TempDir) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO service_preferences (service_name, priority, auto_import_enabled) VALUES ('spotify', 1, 1)")
        .execute(&pool).await.unwrap();

    let temp_dir = TempDir::new().expect("tempdir");
    let base_path = temp_dir.path().to_string_lossy().to_string();
    sqlx::query("UPDATE folder_settings SET base_folder = ? WHERE id = 1")
        .bind(&base_path)
        .execute(&pool)
        .await
        .unwrap();

    let state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(initial_concurrency),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    (state, pool, temp_dir)
}

async fn kv_value(db: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(db)
        .await
        .unwrap()
}

async fn sync_concurrency(db: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
        .fetch_one(db)
        .await
        .unwrap()
}

async fn advanced_concurrency(db: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT max_concurrent_downloads FROM advanced_settings WHERE id = 1")
        .fetch_one(db)
        .await
        .unwrap()
}

#[tokio::test]
async fn concurrency_saves_leave_a_single_trace() {
    let (state, pool, _dir) = setup_app_state(2).await;

    // Vía 1: perform_set_max_concurrent_downloads.
    syncify_tauri_lib::commands::perform_set_max_concurrent_downloads(&state, 7)
        .await
        .expect("set_max_concurrent_downloads must succeed");
    assert_eq!(sync_concurrency(&pool).await, 7);
    assert_eq!(
        state.worker_state.max_concurrent(),
        7,
        "worker knob follows live"
    );
    assert_eq!(
        advanced_concurrency(&pool).await,
        3,
        "advanced_settings must not receive a copy (seed default untouched)"
    );
    assert!(kv_value(&pool, "dl_concurrent_downloads").await.is_none());

    // Vía 2: perform_update_sync_settings.
    let mut settings = SyncSettings::default();
    settings.max_concurrent_downloads = 5;
    perform_update_sync_settings(&state, settings)
        .await
        .expect("update_sync_settings must succeed");
    assert_eq!(sync_concurrency(&pool).await, 5);
    assert_eq!(state.worker_state.max_concurrent(), 5);
    assert_eq!(advanced_concurrency(&pool).await, 3, "no copy");
    assert!(kv_value(&pool, "dl_concurrent_downloads").await.is_none());
}

#[tokio::test]
async fn dto_reports_the_canonical_surface_not_the_live_worker() {
    let (state, _pool, _dir) = setup_app_state(2).await;

    // El worker arrancó con 2 pero la semilla canónica dice 3: el DTO muestra
    // la superficie persistida, no el estado de ejecución.
    let dto = perform_get_download_settings(&state)
        .await
        .expect("get_download_settings must succeed");
    assert_eq!(dto.max_concurrent_downloads, 3);

    // Tras guardar por la vía canónica, DTO y worker coinciden.
    syncify_tauri_lib::commands::perform_set_max_concurrent_downloads(&state, 6)
        .await
        .unwrap();
    let dto = perform_get_download_settings(&state).await.unwrap();
    assert_eq!(dto.max_concurrent_downloads, 6);
    assert_eq!(state.worker_state.max_concurrent(), 6);
}

#[tokio::test]
async fn download_path_leaves_a_single_trace() {
    let (state, pool, temp_dir) = setup_app_state(2).await;
    let new_path = temp_dir.path().join("nueva-ruta");
    std::fs::create_dir_all(&new_path).unwrap();
    let new_path_str = new_path.to_string_lossy().to_string();

    let updated = perform_update_folder_settings(
        &state.db,
        FolderSettings {
            base_folder: new_path_str.clone(),
            ..FolderSettings::default()
        },
    )
    .await
    .expect("update_folder_settings must succeed");
    assert_eq!(updated.base_folder, new_path_str);

    let base: String = sqlx::query_scalar("SELECT base_folder FROM folder_settings WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(base, new_path_str);

    // Rastro único: ninguna clave legacy recibe copia.
    for key in ["dl_download_path", "download_dir", "download_path"] {
        assert!(
            kv_value(&pool, key).await.is_none(),
            "KV key {key} must not be written anymore"
        );
    }

    // El resolvedor canónico devuelve la ruta nueva.
    let effective = resolve_effective_download_paths(&state.db).await.unwrap();
    assert_eq!(effective.library_root, new_path_str);
}

#[tokio::test]
async fn legacy_kv_values_are_still_resolved_when_canonical_is_empty() {
    // BD "antigua": canonical vacío, valores solo en la KV legacy.
    let (state, pool, temp_dir) = setup_app_state(2).await;
    let legacy_path = temp_dir.path().join("legacy-ruta");
    std::fs::create_dir_all(&legacy_path).unwrap();
    let legacy_path_str = legacy_path.to_string_lossy().to_string();

    sqlx::query("UPDATE folder_settings SET base_folder = '' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO settings (key, value) VALUES ('dl_download_path', ?)")
        .bind(&legacy_path_str)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE sync_settings SET max_concurrent_downloads = 9 WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();

    // Lectura tolerante de la ruta.
    let effective = resolve_effective_download_paths(&state.db).await.unwrap();
    assert_eq!(
        effective.library_root, legacy_path_str,
        "legacy KV path must be resolved when the canonical surface is empty"
    );

    // Lectura tolerante del límite: la consulta de arranque del worker
    // (COALESCE sync_settings → KV legacy) devuelve el valor del usuario.
    let restored: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(
            (SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1),
            (SELECT CAST(value AS INTEGER) FROM settings
             WHERE key = 'dl_concurrent_downloads' AND value IS NOT NULL AND TRIM(value) != '')
        )",
    )
    .fetch_optional(&state.db)
    .await
    .unwrap();
    assert_eq!(restored, Some(9));
}

#[tokio::test]
async fn legacy_kv_concurrency_is_used_only_when_canonical_row_is_missing() {
    let (state, pool, _dir) = setup_app_state(2).await;
    sqlx::query("INSERT INTO settings (key, value) VALUES ('dl_concurrent_downloads', '4')")
        .execute(&pool)
        .await
        .unwrap();

    // Con la fila canónica presente, manda sync_settings (3 por semilla).
    let val: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(
            (SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1),
            (SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'dl_concurrent_downloads')
        )",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(val, Some(3));

    // Sin fila canónica, el valor legacy converge (no se pierde el dato).
    sqlx::query("DELETE FROM sync_settings WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    let val: Option<i64> = sqlx::query_scalar(
        "SELECT COALESCE(
            (SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1),
            (SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'dl_concurrent_downloads')
        )",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(val, Some(4));

    // El DTO también cae al valor legacy cuando no hay fila canónica.
    let dto = perform_get_download_settings(&state).await.unwrap();
    assert_eq!(dto.max_concurrent_downloads, 4);
}

#[tokio::test]
async fn save_download_settings_no_longer_mirrors_path_or_concurrency() {
    let (state, pool, temp_dir) = setup_app_state(2).await;
    let base_path = temp_dir.path().to_string_lossy().to_string();

    let dto = perform_get_download_settings(&state).await.unwrap();
    let dto = perform_save_download_settings(
        &state,
        syncify_tauri_lib::commands::settings::DownloadSettingsDto {
            download_path: base_path.clone(),
            max_concurrent_downloads: 8,
            ..dto
        },
    )
    .await
    .expect("save_download_settings must succeed");

    assert_eq!(dto.max_concurrent_downloads, 8);
    assert_eq!(
        sync_concurrency(&pool).await,
        8,
        "canonical surface updated"
    );
    assert!(
        kv_value(&pool, "dl_concurrent_downloads").await.is_none(),
        "no KV copy"
    );
    assert!(
        kv_value(&pool, "dl_download_path").await.is_none(),
        "no legacy path mirror"
    );
    // Las claves que SÍ tienen la KV como única superficie siguen escribiéndose.
    assert!(kv_value(&pool, "dl_generate_cover_art").await.is_some());
}
