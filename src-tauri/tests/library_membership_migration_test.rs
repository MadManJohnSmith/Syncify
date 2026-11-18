//! Issue #97 — upgrade real de la pertenencia de biblioteca
//! (`library_memberships` + `tracks.library_added_at`, migración 0112).
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `library_fts_migration_upgrade_test` (#99): el
//! fixture ejecuta los ficheros REALES de migración 0001–0111 sobre una BD
//! limpia, siembra datos del esquema vigente (favoritos, compras, playlists
//! con dueño, playlist local sin cuenta) y después aplica 0112, comprobando
//! que el backfill no pierde pertenencias ni cuentas, que `library_added_at`
//! se materializa con la regla del issue (máximo `added_at` no nulo y
//! distinto de 1970, `created_at` como respaldo) y que los índices mínimos
//! enumerados existen tras el upgrade. La 0112 no edita migraciones
//! aplicadas ni reutiliza números.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const NEW_VERSION: i64 = 112;

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Índices presentes en una tabla según sqlite_master.
async fn index_names(pool: &Pool<Sqlite>, table: &str) -> Vec<String> {
    sqlx::query_as::<_, (String,)>(
        "SELECT name FROM pragma_index_list(?) WHERE name NOT LIKE 'sqlite_autoindex%'",
    )
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(n,)| n)
    .collect()
}

/// Siembra el esquema vigente ANTES del upgrade (0001–0111): favoritos por
/// cuenta, compra, importación local (is_liked=0), playlists con dueño y una
/// playlist local sin cuenta (PL00), con added_at de 2024, 2020, 1970 y
/// NULL. T4 y T5 no tienen ninguna pertenencia: su materializado debe caer
/// al respaldo `created_at`.
async fn seed_pre_upgrade(pool: &Pool<Sqlite>) {
    // Servicios 1 (spotify) y 2 (qobuz) ya existen por migración.
    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email) VALUES \
         (1, 1, 'A1', 'a1@x'), (2, 2, 'A2', 'a2@x')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Alpha Band')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Nightfall')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, created_at) VALUES \
         (1, 'Sunrise', 1, '2025-06-01T10:00:00Z'), \
         (2, 'Island', NULL, '2025-06-02T10:00:00Z'), \
         (3, 'River', NULL, '2025-06-03T10:00:00Z'), \
         (4, 'Ghost', NULL, '2025-06-04T10:00:00Z'), \
         (5, 'Storm', NULL, '2025-06-05T10:00:00Z')",
    )
    .execute(pool)
    .await
    .unwrap();

    // library_entries: favorito 2024, favorito 2020, favorito 1970,
    // favorito NULL, compra y entrada de importación local (is_liked=0).
    sqlx::query(
        "INSERT INTO library_entries (account_id, track_id, added_at, is_liked, is_purchased) VALUES \
         (1, 1, '2024-03-10T08:00:00Z', 1, 0), \
         (2, 1, '2020-07-22T08:00:00Z', 1, 0), \
         (1, 2, '1970-01-01T00:00:00Z', 1, 0), \
         (2, 2, NULL, 1, 0), \
         (1, 3, '2024-01-05T08:00:00Z', 0, 1), \
         (1, 4, '2024-02-02T08:00:00Z', 0, 0)",
    )
    .execute(pool)
    .await
    .unwrap();

    // Playlist con dueño (2024) y playlist local sin cuenta con la misma
    // pista T4: la local no genera pertenencia (su pertenencia viva sigue
    // siendo playlist_tracks).
    sqlx::query(
        "INSERT INTO playlists (id, account_id, service_playlist_id, name) VALUES \
         (1, 1, 'pl-a1', 'A1 list'), (2, NULL, 'local_mix', 'Local mix')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) VALUES \
         (1, 1, 1, '2024-05-01T08:00:00Z'), \
         (2, 4, 1, '2024-05-02T08:00:00Z')",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn el_upgrade_a_0112_sobre_base_0111_rellena_pertenencia_y_materializado() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");

    // 1. Base vigente: migraciones reales hasta 0111 (0112 aún no existe).
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    seed_pre_upgrade(&pool).await;

    // El espejo de migración (0067) NO participa: ninguna fila suya se lee
    // como pertenencia por cuenta.
    let library_items_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_items")
        .fetch_one(&pool)
        .await
        .unwrap();

    // 2. El upgrade: el fichero REAL de la migración nueva.
    let new_migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0112 está en el árbol");
    assert_eq!(
        new_migration.description, "library memberships",
        "el número 0112 es nuevo y describe la pertenencia de biblioteca"
    );
    sqlx::raw_sql(&new_migration.sql)
        .execute(&pool)
        .await
        .expect("el upgrade a 0112 aplica sobre la base 0111");

    // 3. El backfill no pierde pertenencias ni cuentas.
    // T1: favorito A1 (2024) + favorito A2 (2020) + playlist A1 (2024).
    let rows: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT account_id, collection_type, collection_id FROM library_memberships \
         WHERE track_id = 1 ORDER BY account_id, collection_type",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (1, "favorite".into(), None),
            (1, "playlist".into(), Some(1)),
            (2, "favorite".into(), None),
        ],
        "T1: dos favoritos por cuenta y la playlist con dueño"
    );

    // T2: favorito con added_at 1970 y favorito con NULL — el backfill los
    // conserva CRUDOS (el materializado los descarta, no la pertenencia).
    let raw: Vec<(Option<String>,)> =
        sqlx::query_as("SELECT added_at FROM library_memberships WHERE track_id = 2 ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        raw,
        vec![(Some("1970-01-01T00:00:00Z".into()),), (None,)],
        "T2: added_at crudo conservado (1970 y NULL)"
    );

    // T3: compra (is_purchased=1, is_liked=0) → 'purchase'; T4: importación
    // local (is_liked=0) → 'library'.
    let kinds: Vec<String> = sqlx::query_as(
        "SELECT collection_type FROM library_memberships WHERE track_id IN (3, 4) ORDER BY track_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(k,): (String,)| k)
    .collect();
    assert_eq!(kinds, vec!["purchase", "library"]);

    // T5 no tiene pertenencia: cero filas.
    let orphan: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_memberships WHERE track_id = 5")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(orphan, 0, "T5 sin pertenencia no genera filas");

    // La playlist LOCAL (sin cuenta) no genera pertenencia de playlist:
    // su pertenencia viva sigue siendo playlist_tracks.
    let local_pl: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM library_memberships \
         WHERE collection_type = 'playlist' AND collection_id = 2",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(local_pl, 0, "playlist local sin cuenta: sin pertenencia");

    // Las filas previas no se tocan.
    let le_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(le_count, 6, "library_entries intacto tras el backfill");
    let library_items_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        library_items_before, library_items_after,
        "library_items (0067) intacto"
    );

    // 4. Materialización de library_added_at: máximo added_at vivo no nulo
    //    y distinto de 1970, con created_at como respaldo.
    let added: Vec<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, library_added_at FROM tracks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    let added = added
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();
    // T1: el máximo vivo es la playlist 2024-05-01 (la le 2024-03-10 es
    // menor; la de 2020 no).
    assert_eq!(
        added.get(&1).unwrap().as_deref(),
        Some("2024-05-01T08:00:00Z"),
        "T1: máximo added_at vivo de las pertenencias"
    );
    // T2: añadidos 1970 y NULL → respaldo created_at.
    assert_eq!(
        added.get(&2).unwrap().as_deref(),
        Some("2025-06-02T10:00:00Z"),
        "T2: added_at 1970/NULL descartados, respaldo created_at"
    );
    // T3 y T4: su added_at vivo (2024) materializa. T4 es la MAX de su le
    // (02-02) y su pertenencia de playlist local (05-02) — la unión de
    // pertenencias vivas incluye las playlists sin cuenta, igual que la
    // vista que sustituye el cálculo en runtime. T5 sin pertenencia →
    // respaldo created_at.
    assert_eq!(
        added.get(&3).unwrap().as_deref(),
        Some("2024-01-05T08:00:00Z"),
        "T3: compra con added_at vivo materializado"
    );
    assert_eq!(
        added.get(&4).unwrap().as_deref(),
        Some("2024-05-02T08:00:00Z"),
        "T4: máximo vivo (le 02-02 y playlist local 05-02)"
    );
    assert_eq!(
        added.get(&5).unwrap().as_deref(),
        Some("2025-06-05T10:00:00Z"),
        "T5 sin pertenencia: respaldo created_at"
    );

    // 5. Los índices mínimos enumerados por el issue existen tras el
    //    upgrade — los nuevos los crea 0112; los que ya existían siguen.
    let lm = index_names(&pool, "library_memberships").await;
    for expected in [
        "uq_library_memberships_identity",
        "idx_library_memberships_track_added",
        "idx_library_memberships_account_track",
        "idx_library_memberships_collection",
    ] {
        assert!(
            lm.iter().any(|n| n == expected),
            "library_memberships: {lm:?}"
        );
    }
    let tracks_idx = index_names(&pool, "tracks").await;
    assert!(
        tracks_idx
            .iter()
            .any(|n| n == "idx_tracks_library_added_at"),
        "tracks: {tracks_idx:?}"
    );
    assert!(
        tracks_idx.iter().any(|n| n == "idx_tracks_album"),
        "tracks(album_id) ya existía (0002): {:?}",
        tracks_idx
    );
    let ta = index_names(&pool, "track_artists").await;
    assert!(
        ta.iter().any(|n| n == "idx_track_artists_artist_role"),
        "track_artists: {ta:?}"
    );
    let pt = index_names(&pool, "playlist_tracks").await;
    assert!(
        pt.iter().any(|n| n == "idx_playlist_tracks_track"),
        "playlist_tracks(track_id) ya existía: {pt:?}"
    );
    let ts = index_names(&pool, "track_sources").await;
    assert!(
        ts.iter().any(|n| n == "idx_track_sources_track"),
        "track_sources(track_id) ya existía: {ts:?}"
    );
    let dl = index_names(&pool, "downloads").await;
    assert!(
        dl.iter().any(|n| n == "idx_downloads_track"),
        "downloads(track_id) ya existía: {dl:?}"
    );

    // 6. La identidad de la pertenencia deduplica: el mismo par (cuenta,
    //    track, colección) con collection_id NULL no se inserta dos veces
    //    (en SQLite los NULL son distintos en un UNIQUE; el índice usa
    //    ifnull).
    let before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_memberships WHERE track_id = 1 AND collection_type = 'favorite' AND account_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT OR IGNORE INTO library_memberships \
         (account_id, track_id, collection_type, collection_id, provider_item, added_at) \
         VALUES (1, 1, 'favorite', NULL, 'sp-1', '2024-03-10T08:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_memberships WHERE track_id = 1 AND collection_type = 'favorite' AND account_id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, 1, "la pertenencia de T1/A1 existe tras el backfill");
    assert_eq!(
        after, 1,
        "la misma grabación NO produce una segunda pertenencia"
    );
}

#[tokio::test]
async fn la_migracion_0112_compone_con_las_aplicadas_desde_cero() {
    // Segunda puerta del molde L02: la 0112 aplica también sobre una BD
    // recién migrada completa (tablas vacías: el backfill no falla).
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    MIGRATOR
        .run(&pool)
        .await
        .expect("migración completa desde cero");
    let table: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'library_memberships'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table, 1);
}
