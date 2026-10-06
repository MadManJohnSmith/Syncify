//! Regresión de la migración 0091 (`tracks.play_count` / `tracks.last_played`
//! + tabla `playback_listens`) y del comando de estadísticas
//! `record_local_play` (fase 1 del player).
//!
//! Qué fija este archivo:
//!   1. 0091 aplica en BD nueva (migrador completo) y sobre una tabla
//!      `tracks` preexistente con filas (réplica mínima a mano del estado
//!      pre-0091 ejecutando el SQL REAL embebido, misma convención que
//!      `account_external_user_id_0089_test.rs`): las filas antiguas arrancan
//!      con play_count=0 / last_played=NULL y el CHECK rechaza negativos.
//!   2. Primer token → `counted: true` e incremento; reintento del mismo
//!      token → `counted: false` con play_count/last_played intactos
//!      (idempotencia ante respuesta IPC perdida).
//!   3. Token reutilizado con otra pista → Err sin estado parcial; pista
//!      inexistente/borrada → Err definitivo (FK activa como en producción).
//!   4. Cinturón: con FK desactivada (regresión futura simulada), un UPDATE
//!      sin fila NO persiste la sesión — rollback completo.
//!   5. Normalización del UUID: "ABC…", "abc…" y la forma simple son el
//!      MISMO token; un token inválido es Err definitivo.
//!   6. Carreras: N llamadas concurrentes con el mismo token cuentan UNA vez
//!      (lo decide el INSERT ON CONFLICT, nunca un SELECT-then-INSERT).

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

use syncify_tauri_lib::commands::playback::perform_record_local_play;

/// El archivo tal cual se publica. Los tests lo ejecutan con `raw_sql` para
/// que lo que se prueba sea el SQL que sqlx embebió, no una copia.
const M0091: &str = include_str!("../migrations/0091_local_play_stats.sql");

/// UUID v4 de sesión de escucha principal de los tests.
const TOKEN: &str = "0b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a5b";
/// Segundo token válido (para escuchas distintas).
const OTHER_TOKEN: &str = "f3a9c1d2-4b5e-4f60-9a1b-2c3d4e5f6a7b";

/// Pool en memoria con TODAS las migraciones aplicadas y FK activa (una sola
/// conexión para que `sqlite::memory:` sea una única BD estable; FK ON como
/// en producción, donde db.rs la activa en cada conexión del pool).
async fn migrated_memory_pool_fk_on() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("cadena de conexión en memoria")
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("pool en memoria");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones frescas");
    pool
}

/// Pool en memoria SIN migrar y SIN FK (para la réplica pre-0091, que solo
/// necesita la tabla `tracks` de la que parte el SQL de la migración).
async fn in_memory_fk_off() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("cadena de conexión en memoria")
        .foreign_keys(false)
        .busy_timeout(Duration::from_secs(30));
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("pool en memoria")
}

async fn seed_track(pool: &SqlitePool, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("insertar track de prueba")
}

async fn stats_of(pool: &SqlitePool, track_id: i64) -> (i64, Option<String>) {
    sqlx::query_as("SELECT play_count, last_played FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(pool)
        .await
        .expect("leer estadísticas del track")
}

async fn listen_rows(pool: &SqlitePool) -> Vec<(String, i64)> {
    sqlx::query_as(
        "SELECT listen_session_id, track_id FROM playback_listens \
         ORDER BY listen_session_id",
    )
    .fetch_all(pool)
    .await
    .expect("leer playback_listens")
}

// ============================================================================
// 1. Migración 0091
// ============================================================================

#[tokio::test]
async fn migration_0091_applies_on_fresh_db_with_defaults_and_index() {
    let pool = migrated_memory_pool_fk_on().await;

    // Columnas nuevas en `tracks` con su definición declarada.
    let mut conn = pool.acquire().await.expect("conexión");
    let columns: Vec<(String, i64, Option<String>)> = sqlx::query_as(
        "SELECT name, \"notnull\", dflt_value FROM pragma_table_info('tracks') \
         WHERE name IN ('play_count', 'last_played') ORDER BY name",
    )
    .fetch_all(&mut *conn)
    .await
    .expect("pragma_table_info(tracks)");
    drop(conn);

    assert_eq!(
        columns.len(),
        2,
        "0091 debe añadir play_count y last_played a tracks"
    );
    let play_count = columns.iter().find(|(n, _, _)| n == "play_count").unwrap();
    let last_played = columns.iter().find(|(n, _, _)| n == "last_played").unwrap();
    assert_eq!(play_count.1, 1, "play_count debe ser NOT NULL");
    assert_eq!(
        play_count.2.as_deref(),
        Some("0"),
        "play_count debe tener DEFAULT 0"
    );
    assert_eq!(last_played.1, 0, "last_played debe admitir NULL");
    assert_eq!(last_played.2, None, "last_played no debe tener default");

    // Tabla de sesiones e índice creados.
    let tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' AND name = 'playback_listens'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 1, "playback_listens debe existir");
    let indexes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'index' AND name = 'idx_playback_listens_track'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(indexes, 1, "idx_playback_listens_track debe existir");

    // Defaults sobre filas reales de tracks.
    let t = seed_track(&pool, "recién importada").await;
    let (play_count, last_played) = stats_of(&pool, t).await;
    assert_eq!(play_count, 0, "un track nuevo arranca con 0 escuchas");
    assert_eq!(last_played, None, "un track nuevo arranca sin last_played");
}

#[tokio::test]
async fn migration_0091_upgrades_preexisting_rows_and_enforces_check() {
    let pool = in_memory_fk_off().await;

    // Esquema pre-0091 mínimo (0002: tracks) CON filas ya existentes — la
    // migración debe alterar la tabla sin perderlas ni requerir defaults.
    sqlx::raw_sql(
        "CREATE TABLE tracks ( \
             id INTEGER PRIMARY KEY AUTOINCREMENT, \
             title TEXT NOT NULL, \
             created_at TEXT DEFAULT CURRENT_TIMESTAMP \
         ); \
         INSERT INTO tracks (title) VALUES ('antigua-1'), ('antigua-2');",
    )
    .execute(&pool)
    .await
    .expect("esquema pre-0091");

    // El SQL REAL embebido sobre la BD preexistente (mismo include_str! que
    // sqlx empaqueta).
    sqlx::raw_sql(M0091)
        .execute(&pool)
        .await
        .expect("0091 debe aplicar sobre una BD existente con filas");

    let rows: Vec<(String, i64, Option<String>)> =
        sqlx::query_as("SELECT title, play_count, last_played FROM tracks ORDER BY title")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 2, "las filas preexistentes se conservan");
    for (title, play_count, last_played) in rows {
        assert_eq!(play_count, 0, "{title}: default 0 para filas antiguas");
        assert_eq!(last_played, None, "{title}: NULL para filas antiguas");
    }

    // CHECK(play_count >= 0) añadido por ALTER: se aplica a escrituras nuevas.
    let negative = sqlx::query("UPDATE tracks SET play_count = -1 WHERE title = 'antigua-1'")
        .execute(&pool)
        .await;
    assert!(
        negative.is_err(),
        "el CHECK(play_count >= 0) debe rechazar contadores negativos"
    );
    let ok = sqlx::query("UPDATE tracks SET play_count = 3 WHERE title = 'antigua-1'")
        .execute(&pool)
        .await;
    assert!(ok.is_ok(), "un contador válido debe poder escribirse");

    // playback_listens creada con la PK y la FK declaradas.
    let mut conn = pool.acquire().await.unwrap();
    let pk_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('playback_listens') WHERE pk > 0")
            .fetch_all(&mut *conn)
            .await
            .unwrap();
    drop(conn);
    let pk_cols: Vec<String> = pk_cols.into_iter().map(|(name,)| name).collect();
    assert_eq!(pk_cols, vec!["listen_session_id"], "la PK es la sesión");
}

// ============================================================================
// 2. Semántica del conteo (comando sobre el esquema real)
// ============================================================================

#[tokio::test]
async fn first_token_counts_and_increments() {
    let pool = migrated_memory_pool_fk_on().await;
    let t = seed_track(&pool, "A").await;

    let stats = perform_record_local_play(&pool, t, TOKEN)
        .await
        .expect("la primera escucha debe contar");
    assert!(
        stats.counted,
        "primera invocación del token → counted: true"
    );
    assert_eq!(stats.track_id, t);
    assert_eq!(stats.play_count, 1);
    assert!(stats.last_played.is_some(), "last_played queda estampada");

    let (persisted_count, persisted_last) = stats_of(&pool, t).await;
    assert_eq!(persisted_count, 1);
    assert_eq!(persisted_last, stats.last_played);
    let listens = listen_rows(&pool).await;
    assert_eq!(listens, vec![(TOKEN.to_string(), t)]);
}

#[tokio::test]
async fn repeated_token_is_idempotent_and_keeps_stats() {
    let pool = migrated_memory_pool_fk_on().await;
    let t = seed_track(&pool, "A").await;

    let first = perform_record_local_play(&pool, t, TOKEN)
        .await
        .expect("primera invocación");
    assert!(first.counted);

    // Reintento del MISMO token (respuesta IPC perdida): dedupe, sin UPDATE.
    let second = perform_record_local_play(&pool, t, TOKEN)
        .await
        .expect("reintento del mismo token");
    assert!(!second.counted, "reintento NO vuelve a contar");
    assert_eq!(second.play_count, first.play_count, "play_count intacto");
    assert_eq!(
        second.last_played, first.last_played,
        "last_played NO cambia en el reintento"
    );

    let (persisted_count, persisted_last) = stats_of(&pool, t).await;
    assert_eq!(persisted_count, 1, "exactamente un incremento");
    assert_eq!(persisted_last, first.last_played);
    assert_eq!(listen_rows(&pool).await.len(), 1);
}

#[tokio::test]
async fn token_reused_with_other_track_is_rejected_without_partial_state() {
    let pool = migrated_memory_pool_fk_on().await;
    let t1 = seed_track(&pool, "A").await;
    let t2 = seed_track(&pool, "B").await;

    perform_record_local_play(&pool, t1, TOKEN)
        .await
        .expect("escucha original de t1");

    // El mismo token pretendiendo contar t2: ROLLBACK + Err.
    let err = perform_record_local_play(&pool, t2, TOKEN).await;
    assert!(err.is_err(), "token reutilizado con otra pista → Err");

    // Sin estado parcial: t2 no contó, el token sigue mapeado a t1.
    let (count_t2, _) = stats_of(&pool, t2).await;
    assert_eq!(count_t2, 0, "t2 no debe quedar incrementado");
    let (count_t1, _) = stats_of(&pool, t1).await;
    assert_eq!(count_t1, 1, "t1 conserva su escucha original");
    let listens = listen_rows(&pool).await;
    assert_eq!(listens, vec![(TOKEN.to_string(), t1)]);

    // Y reinvocar con la pista correcta sigue siendo idempotente.
    let again = perform_record_local_play(&pool, t1, TOKEN).await.unwrap();
    assert!(!again.counted);
    assert_eq!(again.play_count, 1);
}

#[tokio::test]
async fn nonexistent_track_is_a_definitive_error() {
    let pool = migrated_memory_pool_fk_on().await;
    let _t = seed_track(&pool, "A").await;

    // FK activa (producción): la INSERT muere por clave foránea.
    let err = perform_record_local_play(&pool, 999_999, TOKEN).await;
    assert!(err.is_err(), "pista inexistente → Err definitivo");
    assert_eq!(
        listen_rows(&pool).await.len(),
        0,
        "no debe quedar rastro de la sesión fallida"
    );

    // Validación de entrada: track_id <= 0 se rechaza antes de tocar la BD.
    assert!(perform_record_local_play(&pool, 0, TOKEN).await.is_err());
    assert!(perform_record_local_play(&pool, -3, OTHER_TOKEN)
        .await
        .is_err());
    assert_eq!(listen_rows(&pool).await.len(), 0);
}

#[tokio::test]
async fn deleted_track_cascades_listen_and_further_plays_fail() {
    let pool = migrated_memory_pool_fk_on().await;
    let t = seed_track(&pool, "A").await;

    perform_record_local_play(&pool, t, TOKEN)
        .await
        .expect("escucha previa al borrado");
    assert_eq!(listen_rows(&pool).await.len(), 1);

    // Borrar la pista debe arrastrar la fila de escucha (ON DELETE CASCADE).
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(t)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        listen_rows(&pool).await.len(),
        0,
        "ON DELETE CASCADE debe limpiar playback_listens"
    );

    // Reproducir la pista borrada con un token nuevo: Err definitivo
    // (la UI descarta el pendiente, no reintenta).
    let err = perform_record_local_play(&pool, t, OTHER_TOKEN).await;
    assert!(err.is_err());
    assert_eq!(listen_rows(&pool).await.len(), 0);
}

#[tokio::test]
async fn update_miss_fails_and_rolls_back_completely() {
    // Cinturón (punto 3 de la semántica transaccional): con FK DESACTIVADA —
    // la regresión futura que este cinturón anticipa — el INSERT prospera con
    // una pista que no existe, el UPDATE no afecta filas y la sesión NO debe
    // persistirse: ROLLBACK completo.
    let pool = in_memory_fk_off().await;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones frescas");

    let err = perform_record_local_play(&pool, 4242, TOKEN).await;
    assert!(err.is_err(), "UPDATE sin fila → Err");
    assert_eq!(
        listen_rows(&pool).await.len(),
        0,
        "rollback completo: la sesión no debe persistirse sin su incremento"
    );
}

// ============================================================================
// 3. Normalización del token de sesión
// ============================================================================

#[tokio::test]
async fn uuid_case_variants_normalize_to_the_same_token() {
    let pool = migrated_memory_pool_fk_on().await;
    let t = seed_track(&pool, "A").await;

    // "ABC…" (mayúsculas) cuenta; "abc…" (minúsculas) es el MISMO token.
    let first = perform_record_local_play(&pool, t, TOKEN.to_uppercase().as_str())
        .await
        .expect("token en mayúsculas");
    assert!(first.counted);

    let second = perform_record_local_play(&pool, t, TOKEN)
        .await
        .expect("token en minúsculas");
    assert!(!second.counted, "case-variantes del mismo UUID → dedupe");
    assert_eq!(second.play_count, 1);
    assert_eq!(listen_rows(&pool).await.len(), 1);

    // La forma simple (sin guiones) también normaliza al token canónico.
    let simple = TOKEN.replace('-', "");
    let third = perform_record_local_play(&pool, t, &simple)
        .await
        .expect("forma simple del token");
    assert!(!third.counted);
    assert_eq!(listen_rows(&pool).await.len(), 1);

    // La forma persistida es la canónica: minúsculas con guiones.
    let (stored,): (String,) = sqlx::query_as("SELECT listen_session_id FROM playback_listens")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, TOKEN.to_lowercase());

    // Token inválido → Err definitivo (la UI descarta el pendiente).
    for bad in [
        "no-es-un-uuid",
        "",
        "0b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a5",
        "zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz",
    ] {
        let err = perform_record_local_play(&pool, t, bad)
            .await
            .err()
            .unwrap_or_else(|| panic!("el token {bad:?} debe ser rechazado"));
        assert!(
            err.contains("listen_session_id"),
            "el error debe atribuirse al token, got: {err}"
        );
    }
    assert_eq!(
        listen_rows(&pool).await.len(),
        1,
        "los tokens inválidos no tocan la BD"
    );
}

// ============================================================================
// 4. Carreras: el INSERT ON CONFLICT decide
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_calls_with_same_token_count_once() {
    let dir = tempdir().expect("tempdir");
    let opts = SqliteConnectOptions::new()
        .filename(dir.path().join("stats.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));
    let pool = SqlitePoolOptions::new()
        .max_connections(6)
        .connect_with(opts)
        .await
        .expect("pool de archivo compartido");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones frescas");
    let t = seed_track(&pool, "A").await;

    let pool = Arc::new(pool);
    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            perform_record_local_play(&pool, t, TOKEN).await
        }));
    }

    let mut counted = 0;
    for handle in handles {
        let stats = handle
            .await
            .expect("tarea sin panic")
            .expect("cada llamada concurrente debe resolver sin error");
        if stats.counted {
            counted += 1;
        }
        assert!(stats.play_count >= 1);
    }

    assert_eq!(
        counted, 1,
        "exactamente UNA llamada cuenta: lo decide el INSERT ON CONFLICT"
    );
    let (persisted_count, _) = stats_of(&pool, t).await;
    assert_eq!(persisted_count, 1);
    assert_eq!(listen_rows(&pool).await.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_distinct_tokens_each_count() {
    let dir = tempdir().expect("tempdir");
    let opts = SqliteConnectOptions::new()
        .filename(dir.path().join("stats_distinct.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(30));
    let pool = SqlitePoolOptions::new()
        .max_connections(6)
        .connect_with(opts)
        .await
        .expect("pool de archivo compartido");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones frescas");
    let t = seed_track(&pool, "A").await;

    let pool = Arc::new(pool);
    let mut handles = Vec::new();
    for i in 0..8u8 {
        let pool = pool.clone();
        // 8 UUIDs válidos distintos: TOKEN sin sus 2 últimos hex + índice.
        let token = format!("{TOKEN_CORE}{i:02x}");
        handles.push(tokio::spawn(async move {
            perform_record_local_play(&pool, t, &token).await
        }));
    }

    let mut counted = 0;
    for handle in handles {
        let stats = handle
            .await
            .expect("tarea sin panic")
            .expect("cada sesión distinta cuenta sin error");
        assert!(stats.counted);
        counted += 1;
    }
    assert_eq!(counted, 8);

    let (persisted_count, _) = stats_of(&pool, t).await;
    assert_eq!(persisted_count, 8, "cada sesión de escucha cuenta una vez");
}

/// TOKEN sin sus dos últimos hex: base para derivar 8 UUIDs canónicos
/// distintos de 36 caracteres (TOKEN_CORE + 2 hex).
const TOKEN_CORE: &str = "0b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a";
