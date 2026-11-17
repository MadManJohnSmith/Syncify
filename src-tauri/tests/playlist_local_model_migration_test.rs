//! PL00 (#153) — upgrade real del modelo de playlist local (migración 0108).
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `sync_run_schema_migration_test` (FE-SYNC-01,
//! PR #304): el fixture ejecuta los ficheros REALES de migración 0001–0106
//! sobre una BD limpia, siembra playlists del esquema vigente (remota con
//! cuenta, locales `local_%` en cuenta 1, pistas y fuente remota) y después
//! aplica 0108, comprobando que el modelo nuevo queda operativo y que nada
//! previo se pierde. Las migraciones aplicadas nunca se editan ni se
//! reutilizan números.
//!
//! Número de migración: 0108. El PR original usó 0107 (siguiente libre en su
//! base); L02 (#306) fusionó antes su 0107_library_fts_v2.sql y esta se
//! renumeró a 0108 en el rebase, sin editar migraciones aplicadas.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const NEW_VERSION: i64 = 108;

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Siembra el esquema vigente antes del upgrade: una cuenta de Qobuz (el
/// servicio ya viene sembrado por las migraciones) y playlists de ambos tipos.
/// Devuelve (account_id, playlist_remota, playlist_local, track_a, track_b).
async fn seed_pre_upgrade(pool: &Pool<Sqlite>) -> (i64, i64, i64, i64, i64) {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("servicio qobuz sembrado por las migraciones");
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'PL00 Qobuz', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada");

    let track_a: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song A', 'PL00A') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("track A sembrada");
    let track_b: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song B', 'PL00B') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("track B sembrada");

    let remote_pl: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count)
         VALUES (?, 'sp_pl00_1', 'Remota Qobuz', 2) RETURNING id",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("playlist remota sembrada");

    // Local del modelo viejo: cuelga de la cuenta con service_playlist_id local_<nombre>.
    let local_pl: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name)
         VALUES (?, 'local_Mi Lista PL00', 'Mi Lista PL00') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("playlist local sembrada");

    // Posiciones canónicas 1..N (0077): la unicidad vigente es (playlist_id, position).
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1), (?, ?, 2)",
    )
    .bind(remote_pl)
    .bind(track_a)
    .bind(remote_pl)
    .bind(track_b)
    .execute(pool)
    .await
    .expect("pistas de la remota sembradas");
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(local_pl)
        .bind(track_a)
        .execute(pool)
        .await
        .expect("pista de la local sembrada");

    // Fuente remota enlazada: la FK a playlists debe sobrevivir al rebuild.
    sqlx::query(
        "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
         VALUES (?, ?, ?, 'sp_pl00_1')",
    )
    .bind(remote_pl)
    .bind(account_id)
    .bind(service_id)
    .execute(pool)
    .await
    .expect("playlist_sources sembrada");

    (account_id, remote_pl, local_pl, track_a, track_b)
}

/// El índice único de playlist_tracks cubre (playlist_id, position).
async fn assert_unique_playlist_position_index(pool: &Pool<Sqlite>) {
    let indexes: Vec<(String, i64)> =
        sqlx::query_as("SELECT name, \"unique\" FROM pragma_index_list('playlist_tracks')")
            .fetch_all(pool)
            .await
            .expect("pragma_index_list playlist_tracks");
    let mut found = false;
    for (name, is_unique) in indexes {
        if is_unique == 0 {
            continue;
        }
        let cols: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
                .bind(&name)
                .fetch_all(pool)
                .await
                .expect("pragma_index_info");
        if cols == vec![("playlist_id".to_string(),), ("position".to_string(),)] {
            found = true;
        }
    }
    assert!(
        found,
        "playlist_tracks debe tener un índice UNIQUE (playlist_id, position)"
    );
}

#[tokio::test]
async fn el_upgrade_a_0108_sobre_base_0107_deja_el_modelo_de_playlist_operativo() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");

    // 1. Base vigente: migraciones reales 0001–0106.
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    let (account_id, remote_pl, local_pl, track_a, track_b) = seed_pre_upgrade(&pool).await;

    // 2. El upgrade: el fichero real de la migración nueva, sin editar las
    //    aplicadas ni reutilizar números.
    let new_migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0108 está en el árbol");
    assert_eq!(
        new_migration.description, "pl00 local playlist model",
        "el número 0108 es nuevo y describe el modelo local de playlists"
    );
    sqlx::raw_sql(&new_migration.sql)
        .execute(&pool)
        .await
        .expect("el upgrade a 0108 aplica sobre la base 0107");

    // 3. Nada previo se pierde: la cuenta, la remota y su fuente siguen ahí.
    let remote_row: (Option<i64>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT account_id, local_uuid, deleted_at, service_playlist_id FROM playlists WHERE id = ?",
        )
    .bind(remote_pl)
    .fetch_one(&pool)
    .await
    .expect("playlist remota tras el upgrade");
    assert_eq!(
        remote_row.0,
        Some(account_id),
        "la remota conserva su cuenta"
    );
    assert_eq!(remote_row.1, None, "la remota no tiene local_uuid");
    assert_eq!(remote_row.2, None, "la remota no tiene tombstone");
    assert_eq!(
        remote_row.3.as_deref(),
        Some("sp_pl00_1"),
        "service_playlist_id de la remota intacto"
    );

    let sources: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM playlist_sources WHERE playlist_id = ?")
            .bind(remote_pl)
            .fetch_one(&pool)
            .await
            .expect("playlist_sources tras el upgrade");
    assert_eq!(sources, 1, "la fuente remota sobrevive al rebuild");

    // 4. La local queda sin cuenta y con UUID estable (v4: 36 chars, guiones
    //    en las posiciones canónicas, versión '4' y variante 8/9/a/b).
    let local_row: (Option<i64>, String, Option<String>) =
        sqlx::query_as("SELECT account_id, local_uuid, deleted_at FROM playlists WHERE id = ?")
            .bind(local_pl)
            .fetch_one(&pool)
            .await
            .expect("playlist local tras el upgrade");
    assert_eq!(local_row.0, None, "la local se reasigna sin cuenta");
    assert_eq!(local_row.2, None, "la local no tiene tombstone");
    let uuid = &local_row.1;
    assert_eq!(uuid.len(), 36, "local_uuid es un UUID textual");
    let dash_positions: Vec<usize> = uuid
        .char_indices()
        .filter(|(_, c)| *c == '-')
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        dash_positions,
        vec![8, 13, 18, 23],
        "local_uuid respeta el formato 8-4-4-4-12"
    );
    assert_eq!(
        uuid.chars().nth(14),
        Some('4'),
        "local_uuid lleva versión 4"
    );
    assert!(
        matches!(uuid.chars().nth(19), Some('8' | '9' | 'a' | 'b')),
        "local_uuid lleva variante RFC 4122"
    );

    // 5. Las playlists actuales siguen leyéndose: la lectura del listado
    //    (LEFT JOIN de cuenta y servicio) devuelve ambas; la remota con su
    //    servicio, la local sin servicio.
    let rows: Vec<(i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT p.id, COALESCE(p.source_service, s.name) as service_name
        FROM playlists p
        LEFT JOIN accounts a ON a.id = p.account_id
        LEFT JOIN services s ON s.id = a.service_id
        WHERE p.deleted_at IS NULL
        ORDER BY p.id ASC
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("lectura del catálogo tras el upgrade");
    assert_eq!(
        rows,
        vec![(remote_pl, Some("qobuz".to_string())), (local_pl, None),],
        "la remota lee con servicio y la local sin cuenta lee sin servicio"
    );

    // 6. Las pistas se conservan y se puede repetir una pista en la BD:
    //    el mismo track_a vuelve a añadirse a la remota en posición nueva.
    let kept: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(remote_pl)
    .fetch_all(&pool)
    .await
    .expect("pistas de la remota tras el upgrade");
    assert_eq!(
        kept,
        vec![(track_a, 1), (track_b, 2)],
        "posiciones canónicas 1..N conservadas"
    );

    let again = sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 3)",
    )
    .bind(remote_pl)
    .bind(track_a)
    .execute(&pool)
    .await
    .expect("la misma pista se puede repetir");
    assert_eq!(again.rows_affected(), 1);
    let dup_position = sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)",
    )
    .bind(remote_pl)
    .bind(track_b)
    .execute(&pool)
    .await;
    assert!(
        dup_position.is_err(),
        "la unicidad por (playlist_id, position) sigue vigente"
    );
    assert_unique_playlist_position_index(&pool).await;

    // 7. La identidad remota sigue protegida por la unicidad de cuenta+id de
    //    servicio; dos locales con el mismo nombre ya no chocan (los NULL son
    //    distintos y la identidad de la local es local_uuid).
    let remote_dup = sqlx::query(
        "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (?, 'sp_pl00_1', 'Otra')",
    )
    .bind(account_id)
    .execute(&pool)
    .await;
    assert!(
        remote_dup.is_err(),
        "UNIQUE(account_id, service_playlist_id) sigue vigente para las remotas"
    );
    let local_dup = sqlx::query(
        "INSERT INTO playlists (account_id, service_playlist_id, local_uuid, name)
         VALUES (NULL, 'local_Mi Lista PL00', ?, 'Mi Lista PL00')",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(&pool)
    .await
    .expect("dos locales con el mismo nombre coexisten");
    assert_eq!(local_dup.rows_affected(), 1);

    // 8. Índices y trigger recreados tras el rebuild.
    let objects: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type IN ('index', 'trigger')
         AND tbl_name = 'playlists' AND name IS NOT NULL
         AND name IN ('idx_playlists_account', 'idx_playlists_unique', 'idx_playlists_name_search',
                      'idx_playlists_external_id', 'idx_playlists_source_service', 'idx_playlists_is_smart',
                      'idx_playlists_local_uuid', 'idx_playlists_deleted_at', 'trg_playlists_sync_ins')",
    )
    .fetch_all(&pool)
    .await
    .expect("índices y trigger del esquema nuevo");
    assert_eq!(
        objects.len(),
        9,
        "los 8 índices y el trigger trg_playlists_sync_ins se recrean tras el DROP"
    );

    // 9. El trigger recreado sigue operando: una remota nueva rellena su
    //    source_service desde la cuenta; una local nueva no lo necesita.
    let remote_new: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name) VALUES (?, 'sp_pl00_2', 'Remota 2') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("playlist remota nueva insertada");
    let source_service: Option<String> =
        sqlx::query_scalar("SELECT source_service FROM playlists WHERE id = ?")
            .bind(remote_new)
            .fetch_one(&pool)
            .await
            .expect("source_service de la remota nueva");
    assert_eq!(
        source_service.as_deref(),
        Some("qobuz"),
        "el trigger trg_playlists_sync_ins sigue rellenando source_service"
    );

    // 10. El tombstone es operativo: marcar borrada la saca de las lecturas.
    sqlx::query("UPDATE playlists SET deleted_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(local_pl)
        .execute(&pool)
        .await
        .expect("tombstone de la local");
    let alive: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM playlists WHERE deleted_at IS NULL AND id IN (?, ?)",
    )
    .bind(remote_pl)
    .bind(local_pl)
    .fetch_one(&pool)
    .await
    .expect("conteo con filtro de tombstone");
    assert_eq!(alive, 1, "una playlist con deleted_at deja de leerse");

    // 11. La integridad referencial quedó intacta tras el rebuild con FK OFF.
    let violations: Vec<(String, i64, i64, String)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("foreign_key_check");
    assert!(
        violations.is_empty(),
        "ninguna violación de FK tras el upgrade: {:?}",
        violations
    );

    // 12. Y el esquema completo con la migración nueva también migra desde
    //     cero (la puerta de integridad de sqlx sigue aplicando).
    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("segunda BD en memoria");
    MIGRATOR
        .run(&fresh)
        .await
        .expect("migración completa desde cero con 0108 incluida");
}

/// PL00: consultas del modelo nuevo sobre una BD `:memory:` completamente
/// migrada desde cero: una playlist local sin cuenta conserva repeticiones de
/// pista, la sanitización ya no las purga y el import de archivo las admite.
#[tokio::test]
async fn las_consultas_del_modelo_local_conservan_repeticiones_y_sin_cuenta() {
    use syncify_tauri_lib::commands::{
        import_playlist_from_file_core, reconcile_playlist_track_counts, sanitize_playlists_in_pool,
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    MIGRATOR
        .run(&pool)
        .await
        .expect("migración completa desde cero");

    // Cuenta de servicio: solo para el import de archivo, que la usa para su
    // service_playlist_id de importación; la playlist local no la necesita.
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("servicio qobuz sembrado");
    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Qobuz', 1)",
    )
    .bind(service_id)
    .execute(&pool)
    .await
    .expect("cuenta sembrada");

    let track_a: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Song A', 'PL00Q') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("track sembrada");

    // Playlist local sin cuenta (modelo nuevo) con la misma pista DOS veces.
    let local_pl: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, local_uuid, name)
         VALUES (NULL, 'local_Q', ?, 'Con Repetición') RETURNING id",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .fetch_one(&pool)
    .await
    .expect("playlist local sin cuenta creada");
    sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1), (?, ?, 2)",
    )
    .bind(local_pl)
    .bind(track_a)
    .bind(local_pl)
    .bind(track_a)
    .execute(&pool)
    .await
    .expect("repetición insertada");

    // La sanitización global ya no purga duplicados: conserva ambas
    // ocurrencias, recompacta 1..N y sincroniza el contador.
    let stats = sanitize_playlists_in_pool(&pool)
        .await
        .expect("sanitización global");
    assert_eq!(
        stats.duplicate_tracks_purged, 0,
        "no se purgan repeticiones"
    );
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(local_pl)
    .fetch_all(&pool)
    .await
    .expect("filas de la local tras sanitizar");
    assert_eq!(
        rows,
        vec![(track_a, 1), (track_a, 2)],
        "la repetición sobrevive"
    );
    let count: i64 = sqlx::query_scalar("SELECT track_count FROM playlists WHERE id = ?")
        .bind(local_pl)
        .fetch_one(&pool)
        .await
        .expect("track_count tras sanitizar");
    assert_eq!(count, 2, "el contador cuenta las repeticiones");

    // La reconciliación repara un contador desincronizado sin tocar filas.
    sqlx::query("UPDATE playlists SET track_count = 999 WHERE id = ?")
        .bind(local_pl)
        .execute(&pool)
        .await
        .expect("contador desincronizado");
    let fixed = reconcile_playlist_track_counts(&pool)
        .await
        .expect("reconciliación");
    assert_eq!(fixed, 1, "una playlist con contador corrupto reparada");
    let rows_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(local_pl)
            .fetch_one(&pool)
            .await
            .expect("filas tras reconciliar");
    assert_eq!(
        rows_after, 2,
        "la reconciliación tampoco purga repeticiones"
    );

    // El import de archivo admite la misma pista dos veces en el archivo.
    let imported = import_playlist_from_file_core(
        &pool,
        "repeticiones.txt",
        "Song A\nSong A\n",
        Some("Importada con Repeticiones".to_string()),
        None,
    )
    .await
    .expect("import de archivo");
    assert_eq!(imported.matched_count, 2, "las dos entradas enlazan");
    let import_rows: Vec<i64> = sqlx::query_scalar(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(imported.playlist_id)
    .fetch_all(&pool)
    .await
    .expect("filas de la importada");
    assert_eq!(
        import_rows,
        vec![track_a, track_a],
        "el import conserva la repetición"
    );
}
