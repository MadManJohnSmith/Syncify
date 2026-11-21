//! Issue #69 (T05, spec #64) — perfil de sincronización resuelto.
//!
//! **Hito 1 (caracterización, antes de tocar código).** Fija de dónde salen hoy
//! los defaults que el perfil hereda —`service_sync_settings` recién migrada,
//! leída por el camino existente de preferencias de importación— y deja
//! constancia de que una cuenta nueva no tiene todavía una capa de override
//! propia. El hito 2 extiende este mismo fichero con la resolución pura y la
//! persistencia del override.
//!
//! BD `:memory:` y fixtures desechables: nunca la BD personal.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::settings::{
    perform_get_service_import_preferences, perform_update_service_sync_settings,
};

async fn setup_test_db() -> sqlx::SqlitePool {
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
async fn seed_account(pool: &sqlx::SqlitePool, service: &str) -> (i64, i64) {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("servicio sembrado por migraciones");

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Perfil', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada");

    (account_id, service_id)
}

// ============================================================
// Hito 1 — caracterización del default por servicio
// ============================================================

/// El default por servicio que el perfil va a heredar es el que ya sirve el
/// camino de preferencias de importación: favoritos, playlists, álbumes y
/// artistas activos; compras, historial y apariciones apagados.
#[tokio::test]
async fn characterization_service_defaults_match_the_import_preferences() {
    let pool = setup_test_db().await;

    let prefs = perform_get_service_import_preferences(&pool, "qobuz")
        .await
        .expect("preferencias del servicio");

    assert!(prefs.favorite_tracks, "favoritos activos por defecto");
    assert!(prefs.playlists, "playlists activas por defecto");
    assert!(prefs.favorite_albums, "álbumes activos por defecto");
    assert!(prefs.favorite_artists, "artistas activos por defecto");
    assert!(!prefs.purchases);
    assert!(!prefs.library_history);
    assert!(!prefs.include_appearances);
}

/// Un servicio sin fila sembrada (Apple Music) hereda el `Default` del
/// contrato de importación: las cuatro colecciones del perfil, activas.
#[tokio::test]
async fn characterization_service_without_row_falls_back_to_import_defaults() {
    let pool = setup_test_db().await;

    let prefs = perform_get_service_import_preferences(&pool, "apple_music")
        .await
        .expect("fallback del servicio sin fila");

    assert!(prefs.favorite_tracks);
    assert!(prefs.playlists);
    assert!(prefs.favorite_albums);
    assert!(prefs.favorite_artists);
}

/// Cambiar las colecciones de un servicio no toca las demás banderas: la capa
/// por servicio es la única fuente de los defaults de colecciones.
#[tokio::test]
async fn characterization_updating_collections_keeps_the_other_flags() {
    let pool = setup_test_db().await;

    perform_update_service_sync_settings(&pool, "qobuz", true, false, false)
        .await
        .expect("actualización de colecciones");

    let prefs = perform_get_service_import_preferences(&pool, "qobuz")
        .await
        .expect("preferencias tras actualizar");

    assert!(prefs.favorite_tracks);
    assert!(!prefs.playlists);
    assert!(!prefs.favorite_albums);
    // No tocadas por la actualización de colecciones.
    assert!(prefs.favorite_artists);
    assert!(!prefs.purchases);
}

/// Una cuenta nueva no tiene capa de override: hoy no existe ninguna fila
/// account-scoped que el perfil pueda leer, así que todo se hereda del
/// servicio. El test se mantiene después del hito 2: la ausencia de fila es el
/// estado de herencia limpia.
#[tokio::test]
async fn characterization_a_fresh_account_has_no_account_scoped_profile_row() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;

    let key_prefix = format!("sync_profile:account:{}", account_id);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?")
        .bind(&key_prefix)
        .fetch_one(&pool)
        .await
        .expect("consulta de la KV de ajustes");

    assert_eq!(rows, 0, "una cuenta nueva no tiene override persistido");
}
