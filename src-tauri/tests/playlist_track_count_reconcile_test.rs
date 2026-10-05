//! Auditoría 23: `playlists.track_count` se queda congelado cuando una pista
//! desaparece de la biblioteca.
//!
//! `playlist_tracks.track_id` es `ON DELETE CASCADE` (migración 0064), así que
//! borrar una pista borra en cascada su fila de `playlist_tracks`, pero SQLite
//! no dispara ningún trigger sobre `playlists`: `get_playlists` sigue leyendo
//! `p.track_count` y la UI muestra un número que ya no existe. La cascada
//! además deja huecos en `position`.
//!
//! Valida que `reconcile_playlist_track_counts`:
//! 1. Repone el contador tras un borrado en cascada.
//! 2. Recompacta las posiciones que la cascada dejó con huecos.
//! 3. Deja en 0 el contador de una playlist que se quedó vacía.
//! 4. No toca las playlists ya sanas (idempotencia).
//! 5. Corrige solo lo que está mal cuando conviven playlists sanas y rotas.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::reconcile_playlist_track_counts;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    sqlx::query(
        "INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) \
         VALUES (1, 'spotify', 0, 'lossy')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) \
         VALUES (1, 1, 'Spotify User', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

async fn add_track(pool: &SqlitePool, title: &str) -> i64 {
    sqlx::query("INSERT INTO tracks (title) VALUES (?)")
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query_scalar("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn add_playlist(pool: &SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO playlists (account_id, service_playlist_id, name, is_public, track_count, created_at) \
         VALUES (1, ?, ?, 0, 0, CURRENT_TIMESTAMP) RETURNING id",
    )
    .bind(format!("svc_{name}"))
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn add_to_playlist(pool: &SqlitePool, playlist_id: i64, track_id: i64, position: i64) {
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) \
         VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
    )
    .bind(playlist_id)
    .bind(track_id)
    .bind(position)
    .execute(pool)
    .await
    .unwrap();
}

async fn stored_count(pool: &SqlitePool, playlist_id: i64) -> i64 {
    sqlx::query_scalar("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn stored_positions(pool: &SqlitePool, playlist_id: i64) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn real_rows(pool: &SqlitePool, playlist_id: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
        .bind(playlist_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Reproduce el defecto: borrar una pista de la biblioteca deja el contador
/// de la playlist congelado.
#[tokio::test]
async fn cascade_delete_leaves_track_count_stale() {
    let pool = create_test_db().await;

    let playlist_id = add_playlist(&pool, "Stale").await;
    let t1 = add_track(&pool, "Track 1").await;
    let t2 = add_track(&pool, "Track 2").await;
    add_to_playlist(&pool, playlist_id, t1, 1).await;
    add_to_playlist(&pool, playlist_id, t2, 2).await;
    sqlx::query("UPDATE playlists SET track_count = 2 WHERE id = ?")
        .bind(playlist_id)
        .execute(&pool)
        .await
        .unwrap();

    // El borrado que dispara la cascada.
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(real_rows(&pool, playlist_id).await, 1);
    // El contador persistido ya no describe el contenido: esto es lo que ve la UI.
    assert_eq!(stored_count(&pool, playlist_id).await, 2);
}

#[tokio::test]
async fn reconcile_restores_count_after_cascade_delete() {
    let pool = create_test_db().await;

    let playlist_id = add_playlist(&pool, "Cascade").await;
    let t1 = add_track(&pool, "Track 1").await;
    let t2 = add_track(&pool, "Track 2").await;
    add_to_playlist(&pool, playlist_id, t1, 1).await;
    add_to_playlist(&pool, playlist_id, t2, 2).await;
    sqlx::query("UPDATE playlists SET track_count = 2 WHERE id = ?")
        .bind(playlist_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    let fixed = reconcile_playlist_track_counts(&pool).await.unwrap();
    assert_eq!(fixed, 1);
    assert_eq!(stored_count(&pool, playlist_id).await, 1);
    // La cascada dejó un hueco: la posición debe volver a ser 1..N.
    assert_eq!(stored_positions(&pool, playlist_id).await, vec![1]);
}

#[tokio::test]
async fn reconcile_zeroes_a_playlist_emptied_by_cascade() {
    let pool = create_test_db().await;

    let playlist_id = add_playlist(&pool, "Emptied").await;
    let t1 = add_track(&pool, "Only track").await;
    add_to_playlist(&pool, playlist_id, t1, 1).await;
    sqlx::query("UPDATE playlists SET track_count = 1 WHERE id = ?")
        .bind(playlist_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    reconcile_playlist_track_counts(&pool).await.unwrap();
    assert_eq!(stored_count(&pool, playlist_id).await, 0);
    assert_eq!(real_rows(&pool, playlist_id).await, 0);
}

#[tokio::test]
async fn reconcile_is_idempotent_on_healthy_playlists() {
    let pool = create_test_db().await;

    let playlist_id = add_playlist(&pool, "Healthy").await;
    let t1 = add_track(&pool, "A").await;
    let t2 = add_track(&pool, "B").await;
    add_to_playlist(&pool, playlist_id, t1, 1).await;
    add_to_playlist(&pool, playlist_id, t2, 2).await;
    sqlx::query("UPDATE playlists SET track_count = 2 WHERE id = ?")
        .bind(playlist_id)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(reconcile_playlist_track_counts(&pool).await.unwrap(), 0);
    assert_eq!(stored_count(&pool, playlist_id).await, 2);
    assert_eq!(stored_positions(&pool, playlist_id).await, vec![1, 2]);
}

#[tokio::test]
async fn reconcile_only_repairs_the_broken_playlist() {
    let pool = create_test_db().await;

    let healthy_id = add_playlist(&pool, "Healthy").await;
    let broken_id = add_playlist(&pool, "Broken").await;

    let h1 = add_track(&pool, "H1").await;
    let h2 = add_track(&pool, "H2").await;
    add_to_playlist(&pool, healthy_id, h1, 1).await;
    add_to_playlist(&pool, healthy_id, h2, 2).await;
    sqlx::query("UPDATE playlists SET track_count = 2 WHERE id = ?")
        .bind(healthy_id)
        .execute(&pool)
        .await
        .unwrap();

    let b1 = add_track(&pool, "B1").await;
    add_to_playlist(&pool, broken_id, b1, 1).await;
    sqlx::query("UPDATE playlists SET track_count = 7 WHERE id = ?")
        .bind(broken_id)
        .execute(&pool)
        .await
        .unwrap();

    let fixed = reconcile_playlist_track_counts(&pool).await.unwrap();
    assert_eq!(fixed, 1);
    assert_eq!(stored_count(&pool, broken_id).await, 1);
    assert_eq!(stored_count(&pool, healthy_id).await, 2);
    assert_eq!(stored_positions(&pool, healthy_id).await, vec![1, 2]);
}
