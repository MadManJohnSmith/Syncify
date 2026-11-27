//! Modo de conflicto por playlist (#403, ADR-0006 decisiones 15, 23, 27-28 y 34).
//!
//! Cubre los tres modos con las migraciones reales en SQLite `:memory:`:
//! `gana_proveedor` restaura nombre y pertenencia; `gana_local` no se pisa;
//! `editable` aplica cambios del proveedor cuando no hubo edición local y deja
//! los conflictos en `needs_review` sin sobrescribir. También fija el default
//! por servicio para las playlists nuevas y la persistencia de la elección.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::upsert_playlist_and_source;
use syncify_tauri_lib::services::playlist_canonical::{
    apply_provider_playlist_membership, apply_provider_playlist_membership_from_local_snapshot,
    get_playlist_conflict_mode, get_service_playlist_conflict_default,
    list_playlist_conflict_reviews, set_playlist_conflict_mode,
    set_service_playlist_conflict_default, PlaylistConflictMode, PlaylistConflictOutcome,
};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool :memory:");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones reales aplicadas");
    pool
}

async fn seed_account(pool: &SqlitePool, service: &str, label: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email)
         SELECT id, ?, ? FROM services WHERE name = ? RETURNING id",
    )
    .bind(label)
    .bind(format!("{label}@example.test"))
    .bind(service)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

async fn seed_tracks(pool: &SqlitePool, count: usize) -> Vec<i64> {
    let mut ids = Vec::new();
    for index in 0..count {
        let id = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Conflict track {index}"))
            .bind(format!("CONFLICT-{}-{index}", uuid::Uuid::new_v4()))
            .fetch_one(pool)
            .await
            .expect("track sembrada");
        ids.push(id);
    }
    ids
}

async fn seed_playlist(
    pool: &SqlitePool,
    account_id: i64,
    remote_id: &str,
    name: &str,
    provider_tracks: &[i64],
) -> i64 {
    let playlist_id = upsert_playlist_and_source(
        pool,
        account_id,
        remote_id,
        name,
        None,
        None,
        1,
        0,
        None,
        provider_tracks.len() as i32,
    )
    .await
    .expect("upsert de playlist")
    .expect("playlist no tombstoneada");
    apply_provider_playlist_membership(pool, playlist_id, account_id, remote_id, provider_tracks)
        .await
        .expect("pertenencia inicial del proveedor");
    playlist_id
}

async fn playlist_name(pool: &SqlitePool, playlist_id: i64) -> String {
    sqlx::query_scalar("SELECT name FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(pool)
        .await
        .expect("nombre de playlist")
}

async fn membership(pool: &SqlitePool, playlist_id: i64) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT track_id FROM playlist_tracks
         WHERE playlist_id = ? ORDER BY position ASC, id ASC",
    )
    .bind(playlist_id)
    .fetch_all(pool)
    .await
    .expect("pertenencia de playlist")
}

async fn set_local_membership(pool: &SqlitePool, playlist_id: i64, tracks: &[i64]) {
    let mut tx = pool.begin().await.expect("transacción local");
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .expect("limpiar pertenencia local");
    for (index, track_id) in tracks.iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind((index + 1) as i64)
        .execute(&mut *tx)
        .await
        .expect("pertenencia local");
    }
    tx.commit().await.expect("commit local");
}

async fn set_local_name(pool: &SqlitePool, playlist_id: i64, name: &str) {
    sqlx::query("UPDATE playlists SET name = ? WHERE id = ?")
        .bind(name)
        .bind(playlist_id)
        .execute(pool)
        .await
        .expect("renombrar local");
}

#[tokio::test]
async fn service_default_governs_new_playlists_and_does_not_rewrite_existing_ones() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "spotify", "Default Spotify").await;

    set_service_playlist_conflict_default(&pool, "spotify", PlaylistConflictMode::Editable)
        .await
        .expect("default editable");
    assert_eq!(
        get_service_playlist_conflict_default(&pool, "spotify")
            .await
            .expect("default leído"),
        PlaylistConflictMode::Editable
    );

    let editable = seed_playlist(&pool, account_id, "default-editable", "Editable", &[]).await;
    assert_eq!(
        get_playlist_conflict_mode(&pool, editable)
            .await
            .expect("modo de la nueva"),
        PlaylistConflictMode::Editable,
        "la playlist nueva hereda el default del servicio"
    );

    set_service_playlist_conflict_default(&pool, "spotify", PlaylistConflictMode::ProviderWins)
        .await
        .expect("default proveedor");
    let provider = seed_playlist(&pool, account_id, "default-provider", "Provider", &[]).await;
    assert_eq!(
        get_playlist_conflict_mode(&pool, provider)
            .await
            .expect("modo de la nueva"),
        PlaylistConflictMode::ProviderWins
    );
    assert_eq!(
        get_playlist_conflict_mode(&pool, editable)
            .await
            .expect("modo existente"),
        PlaylistConflictMode::Editable,
        "cambiar el default no reescribe playlists existentes"
    );
}

#[tokio::test]
async fn editable_new_playlist_adopts_provider_membership_as_its_baseline() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "spotify", "Editable new Spotify").await;
    let tracks = seed_tracks(&pool, 2).await;
    set_service_playlist_conflict_default(&pool, "spotify", PlaylistConflictMode::Editable)
        .await
        .expect("default editable");

    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "editable-new",
        "Nueva",
        &[tracks[0], tracks[1]],
    )
    .await;

    assert_eq!(
        get_playlist_conflict_mode(&pool, playlist_id)
            .await
            .expect("modo editable"),
        PlaylistConflictMode::Editable
    );
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[1]],
        "la primera importación no es un conflicto"
    );
    assert!(list_playlist_conflict_reviews(&pool, playlist_id)
        .await
        .expect("revisiones")
        .is_empty());
}

#[tokio::test]
async fn provider_wins_restores_name_and_membership_from_the_source() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "spotify", "Provider Spotify").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "provider-wins",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;

    set_local_name(&pool, playlist_id, "Local edit").await;
    set_local_membership(&pool, playlist_id, &[tracks[0], tracks[2]]).await;

    upsert_playlist_and_source(
        &pool,
        account_id,
        "provider-wins",
        "Restored by provider",
        None,
        None,
        1,
        0,
        None,
        2,
    )
    .await
    .expect("reimportación")
    .expect("playlist existente");
    let outcome = apply_provider_playlist_membership(
        &pool,
        playlist_id,
        account_id,
        "provider-wins",
        &[tracks[0], tracks[1]],
    )
    .await
    .expect("restauración de pertenencia");

    assert_eq!(outcome, PlaylistConflictOutcome::Applied);
    assert_eq!(
        playlist_name(&pool, playlist_id).await,
        "Restored by provider"
    );
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[1]]
    );
    let track_count: i64 = sqlx::query_scalar("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(&pool)
        .await
        .expect("track_count restaurado");
    assert_eq!(track_count, 2);
    assert!(
        list_playlist_conflict_reviews(&pool, playlist_id)
            .await
            .expect("revisiones")
            .is_empty(),
        "gana el proveedor no deja revisión: aplica"
    );
}

#[tokio::test]
async fn local_wins_ignores_provider_name_and_membership_changes() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "tidal", "Local Tidal").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "local-wins",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::LocalWins)
        .await
        .expect("modo local");
    set_local_name(&pool, playlist_id, "Mi lista").await;
    set_local_membership(&pool, playlist_id, &[tracks[0], tracks[2]]).await;

    upsert_playlist_and_source(
        &pool,
        account_id,
        "local-wins",
        "Provider rename",
        None,
        None,
        1,
        0,
        None,
        2,
    )
    .await
    .expect("reimportación")
    .expect("playlist existente");
    let outcome = apply_provider_playlist_membership(
        &pool,
        playlist_id,
        account_id,
        "local-wins",
        &[tracks[0], tracks[1]],
    )
    .await
    .expect("aplicación del modo local");

    assert_eq!(outcome, PlaylistConflictOutcome::LocalKept);
    assert_eq!(playlist_name(&pool, playlist_id).await, "Mi lista");
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[2]]
    );
    assert!(list_playlist_conflict_reviews(&pool, playlist_id)
        .await
        .expect("revisiones")
        .is_empty());
}

#[tokio::test]
async fn local_wins_removes_provider_rows_inserted_during_the_sync_scan() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "soundcloud", "Local scan SoundCloud").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "local-scan",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::LocalWins)
        .await
        .expect("modo local");

    // El brazo del sync inserta lo observado mientras pagina; el snapshot
    // previo evita que esa alta del proveedor se confunda con una edición local.
    set_local_membership(&pool, playlist_id, &[tracks[0], tracks[1], tracks[2]]).await;
    let outcome = apply_provider_playlist_membership_from_local_snapshot(
        &pool,
        playlist_id,
        account_id,
        "local-scan",
        &[tracks[0], tracks[1], tracks[2]],
        &[tracks[0], tracks[1]],
        true,
    )
    .await
    .expect("modo local con snapshot previo");

    assert_eq!(outcome, PlaylistConflictOutcome::LocalKept);
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[1]],
        "una alta del proveedor no puede sobrevivir en gana_local"
    );
}

#[tokio::test]
async fn local_wins_restores_local_rows_even_when_the_scan_was_partial() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "deezer", "Partial local Deezer").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "partial-local",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::LocalWins)
        .await
        .expect("modo local");
    set_local_membership(&pool, playlist_id, &[tracks[0], tracks[1], tracks[2]]).await;

    let outcome = apply_provider_playlist_membership_from_local_snapshot(
        &pool,
        playlist_id,
        account_id,
        "partial-local",
        &[tracks[0], tracks[1]],
        &[tracks[0], tracks[1]],
        false,
    )
    .await
    .expect("observación parcial en modo local");

    assert_eq!(outcome, PlaylistConflictOutcome::LocalKept);
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[1]],
        "lo insertado por un recorrido parcial no puede quedarse"
    );
}

#[tokio::test]
async fn editable_name_conflict_goes_to_needs_review_without_overwriting() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "qobuz", "Editable name Qobuz").await;
    let playlist_id = seed_playlist(&pool, account_id, "editable-name", "Original", &[]).await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::Editable)
        .await
        .expect("modo editable");
    set_local_name(&pool, playlist_id, "Mi nombre").await;

    upsert_playlist_and_source(
        &pool,
        account_id,
        "editable-name",
        "Nombre del proveedor",
        None,
        None,
        1,
        0,
        None,
        0,
    )
    .await
    .expect("reimportación")
    .expect("playlist existente");

    assert_eq!(playlist_name(&pool, playlist_id).await, "Mi nombre");
    let reviews = list_playlist_conflict_reviews(&pool, playlist_id)
        .await
        .expect("revisiones");
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].conflict_kind, "name");
    assert_eq!(reviews[0].status, "needs_review");
    assert_eq!(reviews[0].local_value.as_deref(), Some("Mi nombre"));
    assert_eq!(
        reviews[0].provider_value.as_deref(),
        Some("Nombre del proveedor")
    );
}

#[tokio::test]
async fn editable_membership_conflict_goes_to_needs_review_without_overwriting() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "deezer", "Editable membership Deezer").await;
    let tracks = seed_tracks(&pool, 4).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "editable-membership",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::Editable)
        .await
        .expect("modo editable");
    set_local_membership(&pool, playlist_id, &[tracks[0], tracks[1], tracks[2]]).await;

    let outcome = apply_provider_playlist_membership(
        &pool,
        playlist_id,
        account_id,
        "editable-membership",
        &[tracks[0], tracks[1], tracks[3]],
    )
    .await
    .expect("conflicto de pertenencia");

    assert_eq!(outcome, PlaylistConflictOutcome::ReviewQueued);
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[1], tracks[2]],
        "la pertenencia local no se pisa"
    );
    let reviews = list_playlist_conflict_reviews(&pool, playlist_id)
        .await
        .expect("revisiones");
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].conflict_kind, "membership");
    assert_eq!(reviews[0].status, "needs_review");
    assert_eq!(
        reviews[0].local_value.as_deref(),
        Some(
            serde_json::to_string(&vec![tracks[0], tracks[1], tracks[2]])
                .unwrap()
                .as_str()
        )
    );
}

#[tokio::test]
async fn editable_applies_provider_changes_when_local_was_not_edited() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "soundcloud", "Editable clean SoundCloud").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "editable-clean",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::Editable)
        .await
        .expect("modo editable");

    upsert_playlist_and_source(
        &pool,
        account_id,
        "editable-clean",
        "Renamed by provider",
        None,
        None,
        1,
        0,
        None,
        3,
    )
    .await
    .expect("reimportación")
    .expect("playlist existente");
    let outcome = apply_provider_playlist_membership(
        &pool,
        playlist_id,
        account_id,
        "editable-clean",
        &[tracks[0], tracks[1], tracks[2]],
    )
    .await
    .expect("cambio del proveedor");

    assert_eq!(outcome, PlaylistConflictOutcome::Applied);
    assert_eq!(
        playlist_name(&pool, playlist_id).await,
        "Renamed by provider"
    );
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[1], tracks[2]]
    );
    assert!(list_playlist_conflict_reviews(&pool, playlist_id)
        .await
        .expect("revisiones")
        .is_empty());
}

#[tokio::test]
async fn editable_keeps_local_edits_when_provider_did_not_change() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "spotify", "Editable local only").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "editable-local-only",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::Editable)
        .await
        .expect("modo editable");
    set_local_name(&pool, playlist_id, "Solo mía").await;
    set_local_membership(&pool, playlist_id, &[tracks[0], tracks[1], tracks[2]]).await;

    upsert_playlist_and_source(
        &pool,
        account_id,
        "editable-local-only",
        "Original",
        None,
        None,
        1,
        0,
        None,
        2,
    )
    .await
    .expect("reimportación")
    .expect("playlist existente");
    let outcome = apply_provider_playlist_membership(
        &pool,
        playlist_id,
        account_id,
        "editable-local-only",
        &[tracks[0], tracks[1]],
    )
    .await
    .expect("proveedor sin cambios");

    assert_eq!(outcome, PlaylistConflictOutcome::LocalKept);
    assert_eq!(playlist_name(&pool, playlist_id).await, "Solo mía");
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[0], tracks[1], tracks[2]]
    );
    assert!(list_playlist_conflict_reviews(&pool, playlist_id)
        .await
        .expect("revisiones")
        .is_empty());
}

#[tokio::test]
async fn migration_0120_upgrades_populated_databases_without_touching_rows() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool :memory:");
    for migration in MIGRATOR.iter().filter(|migration| migration.version < 120) {
        sqlx::raw_sql(&migration.sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|error| panic!("migración {} falló: {error}", migration.version));
    }

    let account_id = seed_account(&pool, "spotify", "Upgrade Spotify").await;
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count)
         VALUES (?, 'upgrade-403', 'Nombre existente', 1) RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("playlist previa a 0120");
    sqlx::query(
        "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
         SELECT ?, ?, service_id, 'upgrade-403' FROM accounts WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(account_id)
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("fuente previa a 0120");

    for migration in MIGRATOR.iter().filter(|migration| migration.version == 120) {
        sqlx::raw_sql(&migration.sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|error| panic!("migración 0120 falló: {error}"));
    }

    let (name, mode): (String, String) =
        sqlx::query_as("SELECT name, conflict_mode FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .fetch_one(&pool)
            .await
            .expect("playlist conservada");
    assert_eq!(name, "Nombre existente");
    assert_eq!(mode, "gana_proveedor");
    let default: String = sqlx::query_scalar(
        "SELECT playlist_conflict_mode FROM service_preferences WHERE service_name = 'spotify'",
    )
    .fetch_one(&pool)
    .await
    .expect("default de servicio migrado");
    assert_eq!(default, "gana_proveedor");
    let reviews: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlist_conflict_reviews")
        .fetch_one(&pool)
        .await
        .expect("tabla de revisión creada");
    assert_eq!(reviews, 0);
}

#[tokio::test]
async fn repeated_membership_conflicts_refresh_the_same_review_row() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "qobuz", "Repeat Qobuz").await;
    let tracks = seed_tracks(&pool, 4).await;
    let playlist_id = seed_playlist(
        &pool,
        account_id,
        "repeat-conflict",
        "Original",
        &[tracks[0], tracks[1]],
    )
    .await;
    set_playlist_conflict_mode(&pool, playlist_id, PlaylistConflictMode::Editable)
        .await
        .expect("modo editable");
    set_local_membership(&pool, playlist_id, &[tracks[0], tracks[2]]).await;

    apply_provider_playlist_membership(
        &pool,
        playlist_id,
        account_id,
        "repeat-conflict",
        &[tracks[0], tracks[1], tracks[3]],
    )
    .await
    .expect("primer conflicto");
    apply_provider_playlist_membership(
        &pool,
        playlist_id,
        account_id,
        "repeat-conflict",
        &[tracks[0], tracks[1]],
    )
    .await
    .expect("segundo conflicto");

    let reviews = list_playlist_conflict_reviews(&pool, playlist_id)
        .await
        .expect("revisiones");
    assert_eq!(reviews.len(), 1, "una fila pendiente por playlist y tipo");
    assert_eq!(reviews[0].conflict_kind, "membership");
}
