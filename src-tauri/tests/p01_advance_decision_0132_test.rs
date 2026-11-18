//! #132 (P01, parte de #130) — avance automático de cola en el backend.
//!
//! Qué fija este archivo (la parte sin motor; los tests de fin natural,
//! repeat/shuffle resueltos con el motor simulado y la acumulación de los
//! 30 s reales viven en `services/playback_advance_tests.rs`, que necesita
//! inyectar el backend simulado dentro del crate):
//!
//!   1. La decisión PURA `decide_auto_advance` sobre la instantánea REAL del
//!      agregado (BD `:memory:` migrada, escrita con los `perform_*` del
//!      comando): los pendientes mandan sobre cualquier modo de repeat,
//!      `repeat one` repite la pista actual cuando la cola se agotó,
//!      `repeat all` reconstruye el ciclo (historial + actual, en orden de
//!      reproducción) y `repeat off` con la cola vacía no avanza.
//!   2. La transición del agregado que ejecuta el avance: `perform_rebuild_cycle`
//!      convierte historial + actual en pendientes conservando identidades, y
//!      `perform_set_current` sobre su cabeza deja el agregado exactamente en
//!      el estado post-avance (cabeza consumida, posición a 0).
//!
//! Sin migración nueva: P01 vive en las tablas de 0109 (`player_queue_entries`
//! + `player_state`) y en `playback_listens` (0091).

use std::str::FromStr;

use syncify_tauri_lib::commands::playback_queue::{
    perform_enqueue, perform_get_playback_queue, perform_rebuild_cycle, perform_set_current,
    perform_set_modes,
};
use syncify_tauri_lib::services::playback_advance::{decide_auto_advance, AdvanceDecision};

/// Pool `:memory:` de UNA conexión con FK activa y migraciones completas,
/// igual que la producción (db.rs activa `PRAGMA foreign_keys` por conexión).
async fn memory_pool() -> sqlx::Pool<sqlx::Sqlite> {
    let opts = sqlx::sqlite::SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("cadena en memoria")
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(30));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
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

async fn seed_track(pool: &sqlx::Pool<sqlx::Sqlite>, title: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
        .bind(title)
        .fetch_one(pool)
        .await
        .expect("insertar track de prueba")
}

/// Cita la instantánea a la decisión y devuelta en forma legible para los asserts.
fn head_of(decision: &AdvanceDecision) -> (String, i64) {
    match decision {
        AdvanceDecision::PlayHead { entry_id, track_id }
        | AdvanceDecision::ReplayCurrent { entry_id, track_id } => (entry_id.clone(), *track_id),
        AdvanceDecision::RebuildCycle {
            head_entry_id,
            head_track_id,
            ..
        } => (head_entry_id.clone(), *head_track_id),
        AdvanceDecision::Stop => panic!("se esperaba una decisión de avance, no Stop"),
    }
}

#[tokio::test]
async fn los_pendientes_mandan_sobre_cualquier_modo_de_repeat() {
    let pool = memory_pool().await;
    let a = seed_track(&pool, "A").await;
    let b = seed_track(&pool, "B").await;

    let ids = perform_enqueue(&pool, &[a, b], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("current A");
    // Con repeat one el avance NO repite: primero se consume la cola.
    perform_set_modes(&pool, false, "one").await.expect("modos");

    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    let decision = decide_auto_advance(&snapshot);
    let (entry_id, track_id) = head_of(&decision);
    assert_eq!(track_id, b, "la cabeza de los pendientes es la siguiente");
    assert_eq!(entry_id, ids[1], "conserva la identidad de la entrada");
}

#[tokio::test]
async fn repeat_one_repite_la_pista_actual_cuando_la_cola_se_agoto() {
    let pool = memory_pool().await;
    let a = seed_track(&pool, "A").await;
    let ids = perform_enqueue(&pool, &[a], false).await.expect("encolar");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("current");
    perform_set_modes(&pool, false, "one").await.expect("modos");

    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    assert_eq!(
        decide_auto_advance(&snapshot),
        AdvanceDecision::ReplayCurrent {
            entry_id: ids[0].clone(),
            track_id: a
        }
    );
}

#[tokio::test]
async fn repeat_one_sin_pista_actual_no_avanza() {
    let pool = memory_pool().await;
    perform_set_modes(&pool, false, "one").await.expect("modos");
    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    assert_eq!(decide_auto_advance(&snapshot), AdvanceDecision::Stop);
}

#[tokio::test]
async fn repeat_all_reconstruye_el_ciclo_en_orden_de_reproduccion() {
    let pool = memory_pool().await;
    let a = seed_track(&pool, "A").await;
    let b = seed_track(&pool, "B").await;
    let c = seed_track(&pool, "C").await;

    // Ciclo reproducido: A → B (historial, en orden), C suena ahora.
    let ids = perform_enqueue(&pool, &[a, b, c], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("current A");
    perform_set_current(&pool, &ids[1], b)
        .await
        .expect("current B");
    perform_set_current(&pool, &ids[2], c)
        .await
        .expect("current C");
    perform_set_modes(&pool, false, "all").await.expect("modos");

    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    assert_eq!(
        decide_auto_advance(&snapshot),
        AdvanceDecision::RebuildCycle {
            order: vec![ids[0].clone(), ids[1].clone(), ids[2].clone()],
            head_entry_id: ids[0].clone(),
            head_track_id: a,
        },
        "el ciclo es historial + actual en orden de reproducción, cabeza A"
    );
}

#[tokio::test]
async fn repeat_all_con_shuffle_permuta_el_ciclo_en_el_backend() {
    let pool = memory_pool().await;
    let a = seed_track(&pool, "A").await;
    let b = seed_track(&pool, "B").await;
    let c = seed_track(&pool, "C").await;

    let ids = perform_enqueue(&pool, &[a, b, c], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("current A");
    perform_set_current(&pool, &ids[1], b)
        .await
        .expect("current B");
    perform_set_current(&pool, &ids[2], c)
        .await
        .expect("current C");
    perform_set_modes(&pool, true, "all").await.expect("modos");

    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    let decision = decide_auto_advance(&snapshot);
    let AdvanceDecision::RebuildCycle {
        order,
        head_entry_id,
        head_track_id,
    } = decision
    else {
        panic!("se esperaba RebuildCycle con shuffle");
    };
    let mut sorted = order.clone();
    sorted.sort();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(
        sorted, expected,
        "la permutación conserva TODAS las entradas"
    );
    assert!(order.contains(&head_entry_id));
    assert_eq!(
        order.first(),
        Some(&head_entry_id),
        "la cabeza es el primer elemento del orden permutado"
    );
    let expected_head_track = sqlx::query_scalar::<_, i64>(
        "SELECT track_id FROM player_queue_entries WHERE entry_id = ?",
    )
    .bind(&head_entry_id)
    .fetch_one(&pool)
    .await
    .expect("entrada del ciclo");
    assert_eq!(head_track_id, expected_head_track);
}

#[tokio::test]
async fn repeat_off_con_cola_vacia_no_avanza() {
    let pool = memory_pool().await;
    let a = seed_track(&pool, "A").await;
    let ids = perform_enqueue(&pool, &[a], false).await.expect("encolar");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("current");

    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    assert_eq!(decide_auto_advance(&snapshot), AdvanceDecision::Stop);
}

#[tokio::test]
async fn el_rebuild_convierte_historial_y_actual_en_pendientes_con_identidad() {
    let pool = memory_pool().await;
    let a = seed_track(&pool, "A").await;
    let b = seed_track(&pool, "B").await;
    let c = seed_track(&pool, "C").await;

    let ids = perform_enqueue(&pool, &[a, b, c], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("current A");
    perform_set_current(&pool, &ids[1], b)
        .await
        .expect("current B");
    perform_set_current(&pool, &ids[2], c)
        .await
        .expect("current C");

    // El avance: ciclo [A, B, C] y la cabeza es A.
    let order = vec![ids[0].clone(), ids[1].clone(), ids[2].clone()];
    perform_rebuild_cycle(&pool, &order).await.expect("rebuild");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("cabeza");

    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    assert_eq!(
        snapshot.current.as_ref().expect("hay current").entry_id,
        ids[0],
        "la cabeza del ciclo es la pista actual"
    );
    let pending_ids: Vec<String> = snapshot
        .pending
        .iter()
        .map(|e| e.entry_id.clone())
        .collect();
    assert_eq!(
        pending_ids,
        vec![ids[1].clone(), ids[2].clone()],
        "el resto del ciclo queda pendiente, en orden y con identidad"
    );
    assert_eq!(snapshot.position_sec, 0.0, "la posición del avance es 0");
}

#[tokio::test]
async fn el_rebuild_con_entrada_desconocida_falla_sin_dejar_estado_a_mitad() {
    let pool = memory_pool().await;
    let a = seed_track(&pool, "A").await;
    let b = seed_track(&pool, "B").await;
    let ids = perform_enqueue(&pool, &[a, b], false)
        .await
        .expect("encolar");
    perform_set_current(&pool, &ids[0], a)
        .await
        .expect("current A");

    let order = vec![ids[0].clone(), "entry-fantasma".to_string()];
    let err = perform_rebuild_cycle(&pool, &order)
        .await
        .expect_err("entrada fantasma rechazada");
    assert!(err.contains("entry-fantasma") || err.contains("no está"));

    // Rollback: el agregado queda intacto (A actual, nada pendiente nuevo).
    let snapshot = perform_get_playback_queue(&pool).await.expect("snapshot");
    assert_eq!(
        snapshot.current.as_ref().expect("current intacto").entry_id,
        ids[0]
    );
    assert!(
        snapshot.pending.is_empty(),
        "el rebuild fallido no dejó pendientes a mitad"
    );
}

#[tokio::test]
async fn el_rebuild_con_order_vacio_es_error_definitivo() {
    let pool = memory_pool().await;
    assert!(perform_rebuild_cycle(&pool, &[]).await.is_err());
}
