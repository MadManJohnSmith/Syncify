//! PL01 (#154) — CRUD de la playlist local y borrado (spec padre #152).
//!
//! Molde de `playlist_local_model_migration_test` (PL00 #153): BD `:memory:`
//! con los ficheros REALES de migración aplicados en orden, sin fixture
//! sintético del esquema. PL00 dejó el modelo local (0110: `local_uuid`
//! único, `account_id` nullable, `deleted_at` como tombstone); PL01 define
//! su semántica:
//!   * crear local con UUID, nombre no único;
//!   * renombrar/editar descripción sin tocar la identidad;
//!   * borrar local = DELETE físico; borrar importada = tombstone;
//!   * el sync no resucita una importada tombstoneada (deletion wins);
//!   * borrar la cuenta elimina sus importadas y nunca las locales.
//!
//! Las migraciones aplicadas nunca se editan ni se reutilizan números.

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use syncify_tauri_lib::commands::{
    create_playlist_core, delete_playlist_core, perform_delete_account, update_playlist_core,
    upsert_playlist_and_source,
};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// BD `:memory:` con TODAS las migraciones reales aplicadas en orden.
async fn migrated_pool() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("pool :memory:");
    for migration in MIGRATOR.iter() {
        sqlx::raw_sql(&migration.sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
    pool
}

/// Cuenta activa de Qobuz (el servicio ya viene sembrado por las migraciones).
async fn seed_account(pool: &Pool<Sqlite>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active)
         SELECT id, 'PL01 Qobuz', 1 FROM services WHERE name = 'qobuz' RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

/// Playlist importada con identidad remota completa: fila en `playlists`,
/// fuente en `playlist_sources` y dos pistas enlazadas.
async fn seed_imported_playlist(
    pool: &Pool<Sqlite>,
    account_id: i64,
    sp_id: &str,
    name: &str,
) -> i64 {
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, track_count)
         VALUES (?, ?, ?, 2) RETURNING id",
    )
    .bind(account_id)
    .bind(sp_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("playlist importada sembrada");

    sqlx::query(
        "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
         SELECT ?, ?, service_id, ? FROM accounts WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(account_id)
    .bind(sp_id)
    .bind(account_id)
    .execute(pool)
    .await
    .expect("playlist_sources sembrada");

    for (pos, title) in ["PL01 Song A", "PL01 Song B"].iter().enumerate() {
        let track_id: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
                .bind(title)
                .bind(format!("PL01-{sp_id}-{pos}"))
                .fetch_one(pool)
                .await
                .expect("track sembrada");
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind((pos + 1) as i64)
        .execute(pool)
        .await
        .expect("playlist_tracks sembrada");
    }

    playlist_id
}

/// Smart playlist tal como la crea `create_smart_playlist_core`: con cuenta y
/// `is_smart = 1` (constructo local de reglas, sin contraparte remota).
async fn seed_smart_playlist(pool: &Pool<Sqlite>, account_id: i64, name: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO playlists (account_id, service_playlist_id, name, is_smart, rules_json)
         VALUES (?, 'smart_pl01', ?, 1, '[]') RETURNING id",
    )
    .bind(account_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("smart playlist sembrada")
}

async fn row_local_uuid(pool: &Pool<Sqlite>, playlist_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT local_uuid FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(pool)
        .await
        .expect("local_uuid leído")
}

async fn row_deleted_at(pool: &Pool<Sqlite>, playlist_id: i64) -> Option<String> {
    sqlx::query_scalar("SELECT deleted_at FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_one(pool)
        .await
        .expect("deleted_at leído")
}

#[tokio::test]
async fn crear_local_admite_nombres_repetidos_y_renombrar_preserva_identidad() {
    let pool = migrated_pool().await;

    let id1 = create_playlist_core(&pool, "Mi Lista".into(), None)
        .await
        .expect("primera local creada");
    let id2 = create_playlist_core(&pool, "Mi Lista".into(), None)
        .await
        .expect("segunda local con el MISMO nombre creada");

    assert_ne!(id1, id2, "dos filas distintas");
    let uuid1 = row_local_uuid(&pool, id1)
        .await
        .expect("local 1 con local_uuid");
    let uuid2 = row_local_uuid(&pool, id2)
        .await
        .expect("local 2 con local_uuid");
    assert_ne!(uuid1, uuid2, "identidades UUID independientes");
    let (sin_cuenta,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE id IN (?, ?) AND account_id IS NULL")
            .bind(id1)
            .bind(id2)
            .fetch_one(&pool)
            .await
            .expect("cuenta NULL en ambas");
    assert_eq!(sin_cuenta, 2, "ninguna local cuelga de una cuenta");

    // Renombrar y editar descripción: identidad intacta.
    let actualizada = update_playlist_core(
        &pool,
        id1,
        Some("Renombrada".into()),
        Some("nueva descripción".into()),
        None,
    )
    .await
    .expect("renombrado sin error");
    assert_eq!(actualizada.name, "Renombrada");
    assert_eq!(
        actualizada.description.as_deref(),
        Some("nueva descripción")
    );

    let uuid_tras_renombre = row_local_uuid(&pool, id1)
        .await
        .expect("fila sigue existiendo");
    assert_eq!(
        uuid1, uuid_tras_renombre,
        "renombrar no cambia la identidad"
    );

    // La otra homónima queda intacta y no se duplicó ninguna fila.
    let (nombre2,): (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(id2)
        .fetch_one(&pool)
        .await
        .expect("homónima intacta");
    assert_eq!(nombre2, "Mi Lista");
    let (homónimas,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE name = 'Mi Lista'")
            .fetch_one(&pool)
            .await
            .expect("recuento de homónimas");
    assert_eq!(homónimas, 1, "solo la segunda conserva el nombre original");
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists")
        .fetch_one(&pool)
        .await
        .expect("recuento total");
    assert_eq!(total, 2, "crear y renombrar no duplican filas");
}

#[tokio::test]
async fn borrar_local_elimina_la_fila_y_sus_pistas() {
    let pool = migrated_pool().await;
    let pid = create_playlist_core(&pool, "Local a borrar".into(), None)
        .await
        .expect("local creada");

    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('L', 'PL01-L') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("track sembrada");
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(pid)
        .bind(track_id)
        .execute(&pool)
        .await
        .expect("pista enlazada");

    delete_playlist_core(&pool, pid)
        .await
        .expect("borrado local sin error");

    let (filas,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE id = ?")
        .bind(pid)
        .fetch_one(&pool)
        .await
        .expect("recuento tras borrado");
    assert_eq!(filas, 0, "la local se borra de verdad, sin tombstone");
    let (pistas,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(pid)
            .fetch_one(&pool)
            .await
            .expect("recuento de pistas");
    assert_eq!(pistas, 0, "sus pistas también se van");
}

#[tokio::test]
async fn borrar_smart_playlist_elimina_la_fila() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let pid = seed_smart_playlist(&pool, account_id, "Smart PL01").await;

    delete_playlist_core(&pool, pid)
        .await
        .expect("borrado smart sin error");

    let (filas,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE id = ?")
        .bind(pid)
        .fetch_one(&pool)
        .await
        .expect("recuento tras borrado");
    assert_eq!(filas, 0, "una smart es local: borrado físico");
}

#[tokio::test]
async fn borrar_importada_deja_tombstone_y_el_sync_no_la_resucita() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let pid = seed_imported_playlist(&pool, account_id, "pl_rem_1", "Remota A").await;

    delete_playlist_core(&pool, pid)
        .await
        .expect("borrado importada sin error");

    // La fila sigue existiendo como tombstone; su contenido se fue.
    assert!(
        row_deleted_at(&pool, pid).await.is_some(),
        "la importada borrada queda tombstoneada"
    );
    let (pistas,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(pid)
            .fetch_one(&pool)
            .await
            .expect("recuento de pistas");
    assert_eq!(pistas, 0, "el contenido de la importada se borra");
    let (fuentes,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlist_sources WHERE playlist_id = ?")
            .bind(pid)
            .fetch_one(&pool)
            .await
            .expect("recuento de fuentes");
    assert_eq!(
        fuentes, 1,
        "la identidad remota se conserva en el tombstone"
    );
    let (visibles,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE deleted_at IS NULL")
            .fetch_one(&pool)
            .await
            .expect("recuento visible");
    assert_eq!(visibles, 0, "el tombstone no aparece en la biblioteca");

    // El sync reencuentra la misma identidad remota y NO la recrea.
    let res = upsert_playlist_and_source(
        &pool,
        account_id,
        "pl_rem_1",
        "Remota A",
        Some("desc"),
        None,
        1,
        0,
        None,
        2,
    )
    .await
    .expect("upsert sobre tombstone no debe fallar");
    assert!(res.is_none(), "deletion wins: el sync la salta");

    let (filas,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .expect("recuento tras re-sync");
    assert_eq!(filas, 1, "no se crea ninguna fila nueva");
    assert!(
        row_deleted_at(&pool, pid).await.is_some(),
        "el tombstone sigue ahí tras el sync"
    );

    // La vía por nombre (playlist sin service_playlist_id) también respeta el
    // tombstone.
    let res_por_nombre =
        upsert_playlist_and_source(&pool, account_id, "", "Remota A", None, None, 1, 0, None, 2)
            .await
            .expect("upsert por nombre sobre tombstone no debe fallar");
    assert!(res_por_nombre.is_none(), "el nombre tampoco resucita");
}

#[tokio::test]
async fn upsert_de_playlist_viva_sigue_resolviendo_id() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;

    let primera = upsert_playlist_and_source(
        &pool,
        account_id,
        "pl_viva_1",
        "Viva",
        None,
        None,
        1,
        0,
        None,
        0,
    )
    .await
    .expect("alta de playlist viva")
    .expect("alta de playlist viva devuelve id");
    let segunda = upsert_playlist_and_source(
        &pool,
        account_id,
        "pl_viva_1",
        "Viva",
        None,
        None,
        1,
        0,
        None,
        0,
    )
    .await
    .expect("re-import de playlist viva")
    .expect("re-import de playlist viva devuelve id");
    assert_eq!(primera, segunda, "sin PL01 el upsert vivo no cambia");

    // Y tras borrar una distinta, la viva sigue resolviendo.
    let otra = seed_imported_playlist(&pool, account_id, "pl_otra", "Otra").await;
    delete_playlist_core(&pool, otra)
        .await
        .expect("borrado otra");
    let tras_borrado = upsert_playlist_and_source(
        &pool,
        account_id,
        "pl_viva_1",
        "Viva",
        None,
        None,
        1,
        0,
        None,
        0,
    )
    .await
    .expect("upsert tras borrado de otra")
    .expect("la viva no se ve afectada por el tombstone ajeno");
    assert_eq!(primera, tras_borrado);
}

#[tokio::test]
async fn editar_una_importada_tombstoneada_falla() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;
    let pid = seed_imported_playlist(&pool, account_id, "pl_muerta", "Muerta").await;
    delete_playlist_core(&pool, pid).await.expect("borrado");

    let res = update_playlist_core(&pool, pid, Some("Zombi".into()), None, None).await;
    let err = res.expect_err("una playlist tombstoneada no se puede editar");
    assert!(
        err.contains("not found"),
        "el error la trata como inexistente: {}",
        err
    );
}

#[tokio::test]
async fn borrar_la_cuenta_lleva_sus_importadas_y_nunca_las_locales() {
    let pool = migrated_pool().await;
    let account_id = seed_account(&pool).await;

    seed_imported_playlist(&pool, account_id, "pl_cuenta_1", "De la cuenta").await;
    let importada_borrada =
        seed_imported_playlist(&pool, account_id, "pl_cuenta_2", "Borrada antes").await;
    delete_playlist_core(&pool, importada_borrada)
        .await
        .expect("tombstone previo");
    let local = create_playlist_core(&pool, "Local que sobrevive".into(), None)
        .await
        .expect("local creada");
    let local_uuid = row_local_uuid(&pool, local).await.expect("local con uuid");

    perform_delete_account(&pool, account_id, false)
        .await
        .expect("borrado de cuenta");

    let (importadas,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("recuento de importadas");
    assert_eq!(
        importadas, 0,
        "las importadas de la cuenta (vivas y tombstoneadas) se van"
    );
    let (locales,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE local_uuid = ?")
        .bind(&local_uuid)
        .fetch_one(&pool)
        .await
        .expect("recuento de locales");
    assert_eq!(locales, 1, "la local no tiene cuenta y sobrevive");
    assert_eq!(
        row_local_uuid(&pool, local).await.as_deref(),
        Some(local_uuid.as_str()),
        "la identidad de la local queda intacta"
    );
}
