//! Contrato IPC de la ficha de conflicto de playlists (#410).
//!
//! Cubre el estado que consume la UI (`requires_mode_choice`), la persistencia
//! del modo, la cola global de revisiones y las tres resoluciones explícitas.
//! La decisión de sync de los tres modos ya está cubierta por
//! `playlist_conflict_modes_test.rs` (#403).

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::{
    create_playlist_core, get_playlist_conflict_state_core, list_playlist_conflict_reviews_core,
    resolve_playlist_conflict_review_core, set_playlist_conflict_mode_core,
};
use syncify_tauri_lib::services::playlist_canonical::{
    get_playlist_conflict_mode, PlaylistConflictMode,
};

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

async fn seed_remote_playlist(
    pool: &SqlitePool,
    account_id: i64,
    remote_id: &str,
    name: &str,
) -> i64 {
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count)
         VALUES (?, ?, ?, 0) RETURNING id",
    )
    .bind(account_id)
    .bind(remote_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("playlist remota sembrada");
    sqlx::query(
        "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
         SELECT ?, ?, service_id, ? FROM accounts WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(account_id)
    .bind(remote_id)
    .bind(account_id)
    .execute(pool)
    .await
    .expect("fuente de playlist sembrada");
    playlist_id
}

async fn seed_tracks(pool: &SqlitePool, count: usize) -> Vec<i64> {
    let mut ids = Vec::new();
    for index in 0..count {
        let id = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Conflict UI track {index}"))
            .bind(format!("CONFLICT-UI-{}-{index}", uuid::Uuid::new_v4()))
            .fetch_one(pool)
            .await
            .expect("track sembrada");
        ids.push(id);
    }
    ids
}

async fn seed_review(
    pool: &SqlitePool,
    playlist_id: i64,
    kind: &str,
    local_value: Option<&str>,
    provider_value: Option<&str>,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO playlist_conflict_reviews
            (playlist_id, conflict_kind, local_value, provider_value, status)
         VALUES (?, ?, ?, ?, 'needs_review') RETURNING id",
    )
    .bind(playlist_id)
    .bind(kind)
    .bind(local_value)
    .bind(provider_value)
    .fetch_one(pool)
    .await
    .expect("revisión sembrada")
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

#[tokio::test]
async fn remote_playlist_in_factory_mode_requires_a_choice_and_local_does_not() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "spotify", "UI contract Spotify").await;
    let remote_id = seed_remote_playlist(&pool, account_id, "remote-state", "Remota").await;
    let local_id = create_playlist_core(&pool, "Local".into(), None)
        .await
        .expect("playlist local");

    let remote = get_playlist_conflict_state_core(&pool, remote_id)
        .await
        .expect("estado remoto");
    assert_eq!(
        remote.conflict_mode,
        PlaylistConflictMode::ProviderWins.as_str()
    );
    assert!(remote.is_remote);
    assert!(
        remote.requires_mode_choice,
        "el default de fábrica avisa al editar"
    );
    assert_eq!(remote.service_name.as_deref(), Some("spotify"));
    assert!(remote.pending_reviews.is_empty());

    let local = get_playlist_conflict_state_core(&pool, local_id)
        .await
        .expect("estado local");
    assert_eq!(
        local.conflict_mode,
        PlaylistConflictMode::ProviderWins.as_str()
    );
    assert!(!local.is_remote);
    assert!(
        !local.requires_mode_choice,
        "una playlist local no sincroniza"
    );
}

#[tokio::test]
async fn choosing_a_mode_persists_and_stops_the_first_edit_warning() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "tidal", "UI contract Tidal").await;
    let playlist_id = seed_remote_playlist(&pool, account_id, "mode-persist", "Remota").await;

    let editable = set_playlist_conflict_mode_core(&pool, playlist_id, "editable")
        .await
        .expect("modo editable");
    assert_eq!(editable.conflict_mode, "editable");
    assert!(!editable.requires_mode_choice);
    assert_eq!(
        get_playlist_conflict_mode(&pool, playlist_id)
            .await
            .expect("modo leído"),
        PlaylistConflictMode::Editable
    );

    let local = set_playlist_conflict_mode_core(&pool, playlist_id, "gana_local")
        .await
        .expect("modo local");
    assert_eq!(local.conflict_mode, "gana_local");
    assert!(!local.requires_mode_choice);
    assert_eq!(
        get_playlist_conflict_mode(&pool, playlist_id)
            .await
            .expect("modo leído"),
        PlaylistConflictMode::LocalWins
    );

    let invalid = set_playlist_conflict_mode_core(&pool, playlist_id, "inventado").await;
    assert!(invalid.is_err(), "un modo desconocido no se persiste");
}

#[tokio::test]
async fn pending_reviews_carry_playlist_context_and_decoded_membership() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "qobuz", "UI contract Qobuz").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id = seed_remote_playlist(&pool, account_id, "reviews", "Mi lista").await;
    seed_review(
        &pool,
        playlist_id,
        "name",
        Some("Mi lista"),
        Some("Del proveedor"),
    )
    .await;
    let local_membership = serde_json::to_string(&vec![tracks[0], tracks[1]]).unwrap();
    let provider_membership = serde_json::to_string(&vec![tracks[0], tracks[2]]).unwrap();
    seed_review(
        &pool,
        playlist_id,
        "membership",
        Some(&local_membership),
        Some(&provider_membership),
    )
    .await;

    let pending = list_playlist_conflict_reviews_core(&pool, None, None)
        .await
        .expect("cola global");
    assert_eq!(pending.len(), 2);
    assert!(pending
        .iter()
        .all(|review| review.playlist_name == "Mi lista"));
    assert!(pending
        .iter()
        .all(|review| review.service_name.as_deref() == Some("qobuz")));

    let name = pending
        .iter()
        .find(|review| review.conflict_kind == "name")
        .expect("revisión de nombre");
    assert_eq!(name.local_track_ids, None);
    let membership = pending
        .iter()
        .find(|review| review.conflict_kind == "membership")
        .expect("revisión de pertenencia");
    assert_eq!(
        membership.local_track_ids.as_deref(),
        Some(&[tracks[0], tracks[1]][..])
    );
    assert_eq!(
        membership.provider_track_ids.as_deref(),
        Some(&[tracks[0], tracks[2]][..])
    );

    let resolved = list_playlist_conflict_reviews_core(
        &pool,
        Some(playlist_id),
        Some("resolved_local".to_string()),
    )
    .await
    .expect("filtro por estado");
    assert!(resolved.is_empty());

    let invalid = list_playlist_conflict_reviews_core(&pool, None, Some("otro".to_string())).await;
    assert!(invalid.is_err(), "un estado desconocido no se consulta");
}

#[tokio::test]
async fn resolving_with_the_provider_applies_it_and_keeps_the_evidence() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "deezer", "UI contract Deezer").await;
    let playlist_id = seed_remote_playlist(&pool, account_id, "resolve-name", "Mi nombre").await;
    let review_id = seed_review(
        &pool,
        playlist_id,
        "name",
        Some("Mi nombre"),
        Some("Nombre del proveedor"),
    )
    .await;

    let resolved = resolve_playlist_conflict_review_core(&pool, review_id, "provider")
        .await
        .expect("resolución proveedor");
    assert_eq!(resolved.status, "resolved_provider");
    assert!(resolved.resolved_at.is_some());
    let name: String = sqlx::query_scalar("SELECT name FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(&pool)
        .await
        .expect("nombre aplicado");
    assert_eq!(name, "Nombre del proveedor");

    let pending = list_playlist_conflict_reviews_core(&pool, Some(playlist_id), None)
        .await
        .expect("cola pendiente");
    assert!(pending.is_empty(), "resolver saca la fila de pendientes");

    let history = list_playlist_conflict_reviews_core(
        &pool,
        Some(playlist_id),
        Some("resolved_provider".to_string()),
    )
    .await
    .expect("historial");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, review_id);

    let repeated = resolve_playlist_conflict_review_core(&pool, review_id, "local").await;
    assert!(repeated.is_err(), "una revisión cerrada no se reabre");
}

#[tokio::test]
async fn resolving_with_local_or_dismiss_never_overwrites() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "soundcloud", "UI contract SoundCloud").await;
    let playlist_id = seed_remote_playlist(&pool, account_id, "resolve-local", "Mi nombre").await;
    let local_review = seed_review(
        &pool,
        playlist_id,
        "name",
        Some("Mi nombre"),
        Some("Nombre del proveedor"),
    )
    .await;

    let resolved = resolve_playlist_conflict_review_core(&pool, local_review, "local")
        .await
        .expect("resolución local");
    assert_eq!(resolved.status, "resolved_local");
    let name: String = sqlx::query_scalar("SELECT name FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(&pool)
        .await
        .expect("nombre conservado");
    assert_eq!(name, "Mi nombre");

    let dismiss_review = seed_review(
        &pool,
        playlist_id,
        "name",
        Some("Mi nombre"),
        Some("Otro nombre"),
    )
    .await;
    let dismissed = resolve_playlist_conflict_review_core(&pool, dismiss_review, "dismiss")
        .await
        .expect("descarte");
    assert_eq!(dismissed.status, "dismissed");
    let still_local: String = sqlx::query_scalar("SELECT name FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(&pool)
        .await
        .expect("nombre intacto");
    assert_eq!(still_local, "Mi nombre");

    let invalid = resolve_playlist_conflict_review_core(&pool, dismiss_review, "otra").await;
    assert!(invalid.is_err(), "una resolución desconocida no escribe");
}

#[tokio::test]
async fn resolving_membership_with_the_provider_replaces_order_and_count() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool, "spotify", "UI contract membership").await;
    let tracks = seed_tracks(&pool, 3).await;
    let playlist_id =
        seed_remote_playlist(&pool, account_id, "resolve-membership", "Mi lista").await;
    for (index, track_id) in [tracks[0], tracks[1]].iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind((index + 1) as i64)
        .execute(&pool)
        .await
        .expect("pertenencia local");
    }
    sqlx::query("UPDATE playlists SET track_count = 2 WHERE id = ?")
        .bind(playlist_id)
        .execute(&pool)
        .await
        .expect("contador local");

    let local_membership = serde_json::to_string(&vec![tracks[0], tracks[1]]).unwrap();
    let provider_membership = serde_json::to_string(&vec![tracks[2], tracks[0]]).unwrap();
    let review_id = seed_review(
        &pool,
        playlist_id,
        "membership",
        Some(&local_membership),
        Some(&provider_membership),
    )
    .await;

    let resolved = resolve_playlist_conflict_review_core(&pool, review_id, "provider")
        .await
        .expect("resolución de pertenencia");
    assert_eq!(resolved.status, "resolved_provider");
    assert_eq!(
        membership(&pool, playlist_id).await,
        vec![tracks[2], tracks[0]],
        "el orden del proveedor es el que queda"
    );
    let track_count: i64 = sqlx::query_scalar("SELECT track_count FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(&pool)
        .await
        .expect("contador reconciliado");
    assert_eq!(track_count, 2);
}
