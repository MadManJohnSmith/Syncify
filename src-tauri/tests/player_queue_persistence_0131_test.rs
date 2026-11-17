//! #131 (P00, parte de #130) — cola de reproducción persistente en SQLite.
//!
//! Qué fija este archivo:
//!   1. Upgrade REAL a 0109 sobre la base vigente sembrada (molde de
//!      `sync_run_schema_migration_test`, PR #304): una BD `:memory:` recién
//!      migrada NO demuestra que el upgrade funcione; se ejecutan los ficheros
//!      reales de migraciones hasta la vigente, se siembran datos del esquema vigente y
//!      después aplica 0109, comprobando que el esquema nuevo queda operativo
//!      (defaults, CHECKs, índice único de `current`) y que nada previo se
//!      pierde. La cola vieja de localStorage NO se migra — se descarta.
//!   2. Persistencia al reinicio sobre BD en FICHERO desechable: se escribe el
//!      agregado con los `perform_*` REALES del comando, se cierra el pool y
//!      se reabre (un reinicio de la app es exactamente eso: procesos nuevos
//!      sobre la misma BD); cola, historial, pista actual, posición, modos y
//!      volumen sobreviven.
//!   3. CRUD del agregado con los `perform_*` sobre BD `:memory:` migrada:
//!      orden de encolado, reordenación con validación de conjunto, reglas de
//!      `set_current` (pendiente→current, current→historial, previous desde el
//!      historial), recorte del historial a 100, clear/reset y validaciones.
//!
//! Las migraciones aplicadas nunca se editan ni se reutilizan números.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{migrate::Migrator, Pool, Sqlite};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tempfile::tempdir;

use syncify_tauri_lib::commands::playback_queue::{
    perform_clear_history, perform_clear_queue, perform_enqueue, perform_get_playback_queue,
    perform_remove_queue_entry, perform_reorder_queue, perform_reset, perform_set_current,
    perform_set_modes, perform_set_position, perform_set_volume,
};

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");
const NEW_VERSION: i64 = 109;

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Pool `:memory:` de UNA conexión con FK activa y migraciones completas,
/// igual que la producción (db.rs activa `PRAGMA foreign_keys` por conexión).
async fn memory_pool() -> Pool<Sqlite> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("cadena en memoria")
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("pool en memoria");
    MIGRATOR.run(&pool).await.expect("migraciones frescas");
    pool
}

/// Pool en FICHERO desechable (para el test de reinicio) con FK activa.
async fn file_pool(path: &std::path::Path) -> Pool<Sqlite> {
    let url = format!("sqlite://{}", path.to_string_lossy());
    let opts = SqliteConnectOptions::from_str(&url)
        .expect("cadena de fichero")
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("pool en fichero");
    pool
}

async fn seed_track(pool: &Pool<Sqlite>, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("insertar track de prueba")
}

// ---------------------------------------------------------------------------
// 1. Upgrade real a 0109
// ---------------------------------------------------------------------------

#[tokio::test]
async fn el_upgrade_a_0109_sobre_base_vigente_deja_el_esquema_operativo() {
    let pool = Pool::<Sqlite>::connect("sqlite::memory:")
        .await
        .expect("BD en memoria");

    // 1. Base vigente: migraciones reales hasta la vigente (0107)
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    let track_id = seed_track(&pool, "Antes del upgrade").await;

    // 2. El upgrade: el fichero real de la migración nueva, sin editar las
    //    aplicadas ni reutilizar números.
    let new_migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0109 está en el árbol");
    assert_eq!(
        new_migration.description, "player queue",
        "el número 0109 es nuevo y describe la cola del reproductor"
    );
    sqlx::raw_sql(&new_migration.sql)
        .execute(&pool)
        .await
        .expect("el upgrade a 0109 aplica sobre la base vigente");

    // 3. Nada previo se pierde: el track sembrado sigue ahí.
    let title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .expect("track tras el upgrade");
    assert_eq!(title, "Antes del upgrade", "el upgrade conserva los datos");

    // 4. El esquema nuevo queda operativo: player_state arranca con la fila
    //    única y sus defaults del contrato.
    let (position_sec, shuffle, repeat, volume, muted): (f64, bool, String, f64, bool) =
        sqlx::query_as(
            "SELECT position_sec, shuffle, repeat, volume, muted FROM player_state WHERE id = 1",
        )
        .fetch_one(&pool)
        .await
        .expect("fila única de player_state tras el upgrade");
    assert_eq!(position_sec, 0.0);
    assert!(!shuffle);
    assert_eq!(repeat, "off");
    assert_eq!(volume, 1.0);
    assert!(!muted);

    // 5. Contrato de entradas: un INSERT mínimo funciona y los CHECK del
    //    contrato rechazan lo inválido.
    let entry_id = "0b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a5b";
    sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES (?, ?, 'pending', 0)",
    )
    .bind(entry_id)
    .bind(track_id)
    .execute(&pool)
    .await
    .expect("entrada pendiente tras el upgrade");
    let bad_track = sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES ('x', 999999, 'pending', 1)",
    )
    .execute(&pool)
    .await;
    assert!(bad_track.is_err(), "la FK rechaza un track inexistente");
    let bad_kind = sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES ('y', ?, 'inventado', 1)",
    )
    .bind(track_id)
    .execute(&pool)
    .await;
    assert!(bad_kind.is_err(), "el CHECK rechaza un kind inválido");

    // 6. Invariante de una sola pista actual (índice único parcial).
    sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES ('a', ?, 'current', 0)",
    )
    .bind(track_id)
    .execute(&pool)
    .await
    .expect("una pista actual");
    let second_current = sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES ('b', ?, 'current', 1)",
    )
    .bind(track_id)
    .execute(&pool)
    .await;
    assert!(second_current.is_err(), "solo puede haber una 'current'");

    // 7. El borrado de un track arrastra sus entradas (ON DELETE CASCADE).
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(track_id)
        .execute(&pool)
        .await
        .expect("borrar el track");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM player_queue_entries")
        .fetch_one(&pool)
        .await
        .expect("contar entradas");
    assert_eq!(
        remaining, 0,
        "las entradas de un track borrado se descartan"
    );
}

// ---------------------------------------------------------------------------
// 2. Persistencia al reinicio (BD en fichero, pool cerrado y reabierto)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn la_cola_y_los_modos_sobreviven_al_reinicio() {
    let dir = tempdir().expect("directorio desechable");
    let db_path: PathBuf = dir.path().join("syncify-p00.db");

    // -- Primera sesión de la app: migra, siembra y escribe el agregado.
    let pool = file_pool(&db_path).await;
    MIGRATOR
        .run(&pool)
        .await
        .expect("migraciones primera sesión");
    let t1 = seed_track(&pool, "Una").await;
    let t2 = seed_track(&pool, "Dos").await;
    let t3 = seed_track(&pool, "Tres").await;

    let ids = perform_enqueue(&pool, &[t2, t3], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], t2)
        .await
        .expect("pista actual");
    perform_enqueue(&pool, &[t1], false)
        .await
        .expect("encolar detrás");
    perform_set_position(&pool, 42.5).await.expect("posición");
    perform_set_modes(&pool, true, "one").await.expect("modos");
    perform_set_volume(&pool, 0.3, true).await.expect("volumen");
    let before = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert_eq!(before.current.as_ref().expect("current").track_id, t2);
    drop(pool); // -- El proceso «se cierra».

    // -- Segunda sesión: pool nuevo sobre la misma BD. Las migraciones ya
    //    están aplicadas y NO se tocan.
    let pool = file_pool(&db_path).await;
    let after = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea tras reinicio");

    let current = after.current.expect("la pista actual sobrevive");
    assert_eq!(current.track_id, t2);
    assert_eq!(
        after.pending.iter().map(|e| e.track_id).collect::<Vec<_>>(),
        vec![t3, t1],
        "la cola pendiente sobrevive en su orden"
    );
    assert_eq!(after.history.len(), 0, "sin historial previo, sigue vacío");
    assert!(
        (after.position_sec - 42.5).abs() < f64::EPSILON,
        "la posición sobrevive"
    );
    assert!(after.shuffle, "shuffle sobrevive");
    assert_eq!(after.repeat, "one", "repeat sobrevive");
    assert!((after.volume - 0.3).abs() < f64::EPSILON);
    assert!(after.muted);
}

// ---------------------------------------------------------------------------
// 3. CRUD del agregado con los perform_* sobre :memory:
// ---------------------------------------------------------------------------

#[tokio::test]
async fn encolar_respeta_el_orden_y_at_front() {
    let pool = memory_pool().await;
    let t1 = seed_track(&pool, "A").await;
    let t2 = seed_track(&pool, "B").await;
    let t3 = seed_track(&pool, "C").await;

    let tail = perform_enqueue(&pool, &[t1, t2], false)
        .await
        .expect("encolar al final");
    let head = perform_enqueue(&pool, &[t3], true)
        .await
        .expect("encolar delante");

    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert_eq!(
        snap.pending.iter().map(|e| e.track_id).collect::<Vec<_>>(),
        vec![t3, t1, t2],
        "at_front inserta delante; el resto conserva su orden"
    );
    assert_eq!(snap.pending[0].entry_id, head[0]);
    assert_eq!(snap.pending[1].entry_id, tail[0]);
}

#[tokio::test]
async fn encolar_rechaza_tracks_inexistentes_y_vacios() {
    let pool = memory_pool().await;
    assert!(
        perform_enqueue(&pool, &[], false).await.is_err(),
        "lista vacía"
    );
    assert!(
        perform_enqueue(&pool, &[999_999], false).await.is_err(),
        "track fantasma"
    );
    assert!(
        perform_enqueue(&pool, &[-1], false).await.is_err(),
        "track_id inválido"
    );
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert!(
        snap.pending.is_empty(),
        "un fallo de validación no encola nada"
    );
}

#[tokio::test]
async fn remove_y_reorder_operan_sobre_los_pendientes() {
    let pool = memory_pool().await;
    let t1 = seed_track(&pool, "A").await;
    let t2 = seed_track(&pool, "B").await;
    let ids = perform_enqueue(&pool, &[t1, t2], false)
        .await
        .expect("encolar");

    assert!(perform_remove_queue_entry(&pool, &ids[0]).await.is_ok());
    assert!(
        perform_remove_queue_entry(&pool, &ids[0]).await.is_err(),
        "quitar dos veces la misma entrada es error"
    );

    let t3 = seed_track(&pool, "C").await;
    let ids2 = perform_enqueue(&pool, &[t3], false).await.unwrap();
    // Reordenación que no coincide con el conjunto pendiente → error, sin tocar nada.
    assert!(perform_reorder_queue(&pool, &["no-existe".to_string()])
        .await
        .is_err());
    assert!(perform_reorder_queue(&pool, &[]).await.is_err());
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert_eq!(
        snap.pending.iter().map(|e| e.track_id).collect::<Vec<_>>(),
        vec![t2, t3],
        "un reorden fallido no toca la cola"
    );
    assert!(snap.pending.iter().any(|e| e.entry_id == ids2[0]));
}

#[tokio::test]
async fn set_current_mueve_el_current_previo_al_historial() {
    let pool = memory_pool().await;
    let t1 = seed_track(&pool, "A").await;
    let t2 = seed_track(&pool, "B").await;
    let t3 = seed_track(&pool, "C").await;
    let ids = perform_enqueue(&pool, &[t1, t2, t3], false)
        .await
        .expect("encolar");
    perform_set_position(&pool, 17.0)
        .await
        .expect("posición previa");

    // Avanza la cabeza de la cola.
    perform_set_current(&pool, &ids[0], t1)
        .await
        .expect("current 1");
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert_eq!(snap.current.as_ref().expect("current").entry_id, ids[0]);
    assert_eq!(
        snap.pending
            .iter()
            .map(|e| e.entry_id.as_str())
            .collect::<Vec<_>>(),
        vec![ids[1].as_str(), ids[2].as_str()],
        "la cabeza consumida sale de los pendientes"
    );
    assert_eq!(snap.history.len(), 0, "sin current previo no hay historial");
    assert_eq!(
        snap.position_sec, 0.0,
        "cambiar de pista resetea la posición"
    );

    // Segunda pista: la primera pasa al historial.
    perform_set_current(&pool, &ids[1], t2)
        .await
        .expect("current 2");
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert_eq!(
        snap.history
            .iter()
            .map(|e| e.entry_id.as_str())
            .collect::<Vec<_>>(),
        vec![ids[0].as_str()],
        "la pista anterior pasa al historial en orden de reproducción"
    );
    assert_eq!(snap.current.as_ref().expect("current").track_id, t2);
    assert_eq!(
        snap.pending[0].entry_id, ids[2],
        "el resto de la cola sigue"
    );
}

#[tokio::test]
async fn set_current_trae_de_vuelta_entradas_del_historial() {
    let pool = memory_pool().await;
    let t1 = seed_track(&pool, "A").await;
    let t2 = seed_track(&pool, "B").await;
    let ids = perform_enqueue(&pool, &[t1, t2], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], t1)
        .await
        .expect("current 1");
    perform_set_current(&pool, &ids[1], t2)
        .await
        .expect("current 2");

    // `previous`: la entrada vuelve del historial a current; la que sonaba
    // pasa delante de los pendientes (la UI la re-encola antes).
    let back = perform_enqueue(&pool, &[t2], true)
        .await
        .expect("re-encolar la que sonaba");
    perform_set_current(&pool, &ids[0], t1)
        .await
        .expect("previous");

    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert_eq!(snap.current.as_ref().expect("current").entry_id, ids[0]);
    assert_eq!(
        snap.pending
            .iter()
            .map(|e| e.entry_id.as_str())
            .collect::<Vec<_>>(),
        vec![back[0].as_str()],
        "la pista abandonada espera delante de la cola"
    );
    assert!(
        snap.history.iter().all(|e| e.entry_id != ids[0]),
        "la entrada ya no está en el historial"
    );
}

#[tokio::test]
async fn el_historial_se_recorta_al_limite_de_100() {
    let pool = memory_pool().await;
    let mut ids = Vec::new();
    for i in 0..110 {
        let t = seed_track(&pool, &format!("T{i}")).await;
        let id = perform_enqueue(&pool, &[t], false)
            .await
            .expect("encolar")
            .remove(0);
        ids.push((id, t));
    }
    for (id, t) in &ids {
        perform_set_current(&pool, id, *t).await.expect("avanzar");
    }
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert_eq!(snap.history.len(), 100, "el historial se recorta al límite");
    // Se eliminó por el extremo más antiguo: 109 promociones (la última pista
    // es current y no entra al historial), sobreviven las 100 más recientes.
    assert!(
        snap.history.iter().all(|e| e.track_id >= ids[9].1),
        "sobreviven las más recientes"
    );
}

#[tokio::test]
async fn clear_y_reset_consus_alcances() {
    let pool = memory_pool().await;
    let t1 = seed_track(&pool, "A").await;
    let t2 = seed_track(&pool, "B").await;
    let ids = perform_enqueue(&pool, &[t1, t2], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], t1)
        .await
        .expect("current");
    perform_set_modes(&pool, true, "all").await.expect("modos");
    perform_set_volume(&pool, 0.5, false)
        .await
        .expect("volumen");

    // clear: solo pendientes; current, historial, modos y volumen intactos.
    perform_clear_queue(&pool).await.expect("clear");
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert!(snap.pending.is_empty());
    assert!(snap.current.is_some());
    assert!(snap.shuffle);

    // clear history: solo historial.
    perform_clear_history(&pool).await.expect("clear history");
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert!(snap.history.is_empty());
    assert!(snap.current.is_some());

    // reset: vuelca entradas y posición; modos y volumen se conservan.
    perform_set_position(&pool, 9.0).await.expect("posición");
    perform_reset(&pool).await.expect("reset");
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert!(snap.pending.is_empty() && snap.history.is_empty() && snap.current.is_none());
    assert_eq!(snap.position_sec, 0.0);
    assert!(snap.shuffle && snap.repeat == "all");
    assert!((snap.volume - 0.5).abs() < f64::EPSILON && !snap.muted);
}

#[tokio::test]
async fn validaciones_de_posicion_modos_y_volumen() {
    let pool = memory_pool().await;
    assert!(perform_set_position(&pool, -1.0).await.is_err());
    assert!(perform_set_position(&pool, f64::NAN).await.is_err());
    assert!(perform_set_modes(&pool, false, "inventado").await.is_err());
    assert!(perform_set_volume(&pool, 1.5, false).await.is_err());
    assert!(perform_set_volume(&pool, -0.1, false).await.is_err());
    assert!(perform_set_position(&pool, 30.0).await.is_ok());
    assert!(perform_set_modes(&pool, false, "one").await.is_ok());
    assert!(perform_set_volume(&pool, 0.75, true).await.is_ok());
    let snap = perform_get_playback_queue(&pool)
        .await
        .expect("instantánea");
    assert!((snap.position_sec - 30.0).abs() < f64::EPSILON);
    assert_eq!(snap.repeat, "one");
    assert!((snap.volume - 0.75).abs() < f64::EPSILON && snap.muted);
}
