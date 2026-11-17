//! Issue #99 — upgrade real del índice de búsqueda (`library_fts` v2,
//! migración 0107 nueva).
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `sync_run_schema_migration_test` (#304): el
//! fixture ejecuta los ficheros REALES de migración 0001–0106 sobre una BD
//! limpia, siembra datos del esquema vigente y después aplica 0107,
//! comprobando que el índice queda reconstruido con género e ISRC y que los
//! triggers cubren los cambios. La 0019 aplicada no se edita; el número 0107
//! no se reutiliza (0106 lo ocupa sync_runs del PR #304).

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const NEW_VERSION: i64 = 107;

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Siembra el esquema vigente antes del upgrade: álbum, artistas y tres
/// pistas con género e ISRC (con guiones, como llegan del proveedor).
async fn seed_tracks(pool: &Pool<Sqlite>) {
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Alpha Band'), (2, 'Beta Trio')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Nightfall')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, genre, isrc) VALUES \
         (1, 'Sunrise', 1, 'Rock', 'US-UM7-1712345'), \
         (2, 'Island', NULL, 'Jazz', NULL), \
         (3, 'River', NULL, NULL, 'GB-ABC-1234567')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES \
         (1, 1, 'primary'), (2, 2, 'primary')",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Coincidencias de una consulta MATCH sobre el índice nuevo.
async fn fts_ids(pool: &Pool<Sqlite>, fts_query: &str) -> Vec<i64> {
    sqlx::query_as::<_, (i64,)>(
        "SELECT rowid FROM library_fts WHERE library_fts MATCH ? ORDER BY rowid",
    )
    .bind(fts_query)
    .fetch_all(pool)
    .await
    .unwrap_or_else(|e| panic!("MATCH {fts_query:?} falló: {e}"))
    .into_iter()
    .map(|(id,)| id)
    .collect()
}

#[tokio::test]
async fn el_upgrade_a_0107_sobre_base_0105_reconstruye_el_indice() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");

    // 1. Base vigente: migraciones reales hasta 0106 (0107 aún no existe).
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    seed_tracks(&pool).await;

    // 2. El upgrade: el fichero real de la migración nueva, sin editar las
    //    aplicadas ni reutilizar números.
    let new_migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0107 está en el árbol");
    assert_eq!(
        new_migration.description, "library fts v2",
        "el número 0107 es nuevo y describe el índice de búsqueda v2"
    );
    sqlx::raw_sql(&new_migration.sql)
        .execute(&pool)
        .await
        .expect("el upgrade a 0107 aplica sobre la base 0105/0106");

    // 3. Nada previo se pierde: el rebuild deja una fila por track.
    let (tracks, fts): (i64, i64) =
        sqlx::query_as("SELECT (SELECT COUNT(*) FROM tracks), (SELECT COUNT(*) FROM library_fts)")
            .fetch_one(&pool)
            .await
            .expect("recuentos tras el upgrade");
    assert_eq!(tracks, 3, "las pistas sembradas siguen ahí");
    assert_eq!(fts, tracks, "el rebuild deja una fila FTS por pista");

    // 4. El índice nuevo queda operativo: título, artista y álbum (0019) más
    //    género e ISRC — el ISRC normalizado sin guiones.
    assert_eq!(fts_ids(&pool, "\"sunrise\"").await, vec![1]);
    assert_eq!(fts_ids(&pool, "\"alpha\"").await, vec![1]);
    assert_eq!(fts_ids(&pool, "\"nightfall\"").await, vec![1]);
    assert_eq!(fts_ids(&pool, "\"rock\"").await, vec![1], "género indexado");
    assert_eq!(fts_ids(&pool, "\"jazz\"").await, vec![2], "género indexado");
    assert_eq!(
        fts_ids(&pool, "\"usum71712345\"").await,
        vec![1],
        "ISRC indexado normalizado (sin guiones)"
    );
    assert_eq!(
        fts_ids(&pool, "\"gbabc1234567\"").await,
        vec![3],
        "ISRC con guiones normalizado"
    );

    // 5. Triggers: cambio de género e ISRC en tracks.
    sqlx::query("UPDATE tracks SET genre = 'Metal' WHERE id = 2")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        fts_ids(&pool, "\"metal\"").await,
        vec![2],
        "trigger de género"
    );
    assert!(
        fts_ids(&pool, "\"jazz\"").await.is_empty(),
        "género viejo fuera"
    );
    sqlx::query("UPDATE tracks SET isrc = 'JP-PD1-9876543' WHERE id = 2")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        fts_ids(&pool, "\"jppd19876543\"").await,
        vec![2],
        "trigger de ISRC con normalización"
    );

    // 6. Triggers: vínculo de artistas (track_artists) en alta y baja.
    sqlx::query("INSERT INTO artists (id, name) VALUES (3, 'Delta Woman')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (3, 3, 'feature')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        fts_ids(&pool, "\"delta\"").await,
        vec![3],
        "alta de vínculo actualiza el artista"
    );
    sqlx::query("DELETE FROM track_artists WHERE track_id = 2 AND artist_id = 2")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        fts_ids(&pool, "\"beta\"").await.is_empty(),
        "baja de vínculo vacía la cadena de artistas (sin NULL)"
    );

    // 7. Triggers: título del álbum y nombre del artista.
    sqlx::query("UPDATE albums SET title = 'Daybreak' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        fts_ids(&pool, "\"daybreak\"").await,
        vec![1],
        "trigger de álbum"
    );
    assert!(
        fts_ids(&pool, "\"nightfall\"").await.is_empty(),
        "álbum viejo fuera"
    );
    sqlx::query("UPDATE artists SET name = 'Epsilon Group' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        fts_ids(&pool, "\"epsilon\"").await,
        vec![1],
        "trigger de nombre de artista"
    );

    // 8. Triggers: alta y baja de tracks.
    sqlx::query("INSERT INTO tracks (id, title) VALUES (4, 'Ghost')")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        fts_ids(&pool, "\"ghost\"").await,
        vec![4],
        "alta de pista indexada"
    );
    sqlx::query("DELETE FROM tracks WHERE id = 4")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        fts_ids(&pool, "\"ghost\"").await.is_empty(),
        "baja de pista desindexa"
    );

    // 9. Y el esquema completo con la migración nueva también migra desde
    //    cero (la puerta de integridad de sqlx sigue aplicando).
    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("segunda BD en memoria");
    MIGRATOR
        .run(&fresh)
        .await
        .expect("migración completa desde cero con 0107 incluida");
}
