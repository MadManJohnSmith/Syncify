//! Cola de reproducción persistente en SQLite (#131, P00 — parte de #130).
//!
//! El backend es dueño de la cola, el historial, la pista actual, la posición,
//! los modos y el volumen (`player_queue_entries` + `player_state`, migración
//! 0109). La UI es una vista sobre estos comandos: dejó de escribir el estado
//! de reproducción en localStorage y `syncify.playerState.v1` se descarta sin
//! migración.
//!
//! Convivencia: el transporte nativo sigue en `commands/player.rs` (motor
//! `syncify-player`) y `commands/playback.rs` conserva solo la escucha local
//! (`record_local_play`). Este módulo no toca el motor: persiste el agregado
//! de cola que la UI orquesta sobre él.
//!
//! Entradas efímeras de proveedor (#134): una entrada referencia un track de
//! biblioteca o la identidad `service_id` + `service_track_id` de un ítem del
//! proveedor. Encolar una efímera no crea `Track` ni `TrackSource`; la promoción
//! ocurre sólo tras 30 s reales de escucha.

use super::*;
use tauri::Manager;

/// Límite del historial de reproducción (paridad con el recorte que hacía la
/// UI): al superar 100, se elimina por el extremo más antiguo.
const HISTORY_LIMIT: i64 = 100;

/// Entrada de la cola/historial tal como la devuelve el backend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaybackQueueEntryDto {
    pub entry_id: String,
    /// Track de biblioteca; `0` identifica una entrada que sigue efímera.
    pub track_id: i64,
    /// Identidad del proveedor mientras la entrada sigue efímera.
    pub service_id: Option<i64>,
    pub service_track_id: Option<String>,
    /// `pending` | `history` | `current`.
    pub kind: String,
    pub position: i64,
    /// Motivo persistido cuando la promoción aún no pudo resolver identidad.
    pub promotion_error: Option<String>,
}

/// Evento delta: una entrada cambió de identidad o de motivo de promoción.
pub const PLAYER_QUEUE_ENTRY_CHANGED_EVENT: &str = "player:queue-entry-changed";

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayerQueueEntryChangedPayload {
    pub entry: PlaybackQueueEntryDto,
}

/// Instantánea completa del agregado de cola (`playback_queue_get`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaybackQueueSnapshot {
    pub pending: Vec<PlaybackQueueEntryDto>,
    /// Orden de reproducción (el más reciente al final).
    pub history: Vec<PlaybackQueueEntryDto>,
    pub current: Option<PlaybackQueueEntryDto>,
    pub position_sec: f64,
    pub shuffle: bool,
    /// `off` | `all` | `one`.
    pub repeat: String,
    pub volume: f64,
    pub muted: bool,
}

fn validate_track_ids(track_ids: &[i64]) -> Result<(), String> {
    if track_ids.is_empty() {
        return Err("track_ids vacío: se esperaba al menos un track".to_string());
    }
    if track_ids.iter().any(|id| *id <= 0) {
        return Err(format!("track_id inválido en {track_ids:?}"));
    }
    Ok(())
}

fn validate_repeat(repeat: &str) -> Result<(), String> {
    match repeat {
        "off" | "all" | "one" => Ok(()),
        other => Err(format!("repeat inválido: {other:?} (off|all|one)")),
    }
}

/// Genera la identidad canónica de una entrada nueva (UUID v4 con guiones,
/// minúsculas — el mismo formato que normaliza `record_local_play`).
fn new_entry_id() -> String {
    uuid::Uuid::new_v4().hyphenated().to_string()
}

/// Lee la instantánea completa del agregado. Cada lista en su orden propio:
/// pendientes por `position` ascendente, historial en orden de reproducción.
pub async fn perform_get_playback_queue(db: &DbPool) -> Result<PlaybackQueueSnapshot, String> {
    let rows: Vec<(
        String,
        i64,
        Option<i64>,
        Option<String>,
        String,
        i64,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT entry_id, COALESCE(track_id, 0), service_id, service_track_id, kind, position, promotion_error \
         FROM player_queue_entries \
         ORDER BY position ASC, enqueued_at ASC, rowid ASC",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    let entries: Vec<PlaybackQueueEntryDto> = rows
        .into_iter()
        .map(
            |(
                entry_id,
                track_id,
                service_id,
                service_track_id,
                kind,
                position,
                promotion_error,
            )| {
                PlaybackQueueEntryDto {
                    entry_id,
                    track_id,
                    service_id,
                    service_track_id,
                    kind,
                    position,
                    promotion_error,
                }
            },
        )
        .collect();

    let (position_sec, shuffle, repeat, volume, muted): (f64, bool, String, f64, bool) =
        sqlx::query_as(
            "SELECT position_sec, shuffle, repeat, volume, muted FROM player_state WHERE id = 1",
        )
        .fetch_one(db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(PlaybackQueueSnapshot {
        pending: entries
            .iter()
            .filter(|e| e.kind == "pending")
            .cloned()
            .collect(),
        history: entries
            .iter()
            .filter(|e| e.kind == "history")
            .cloned()
            .collect(),
        current: entries.iter().find(|e| e.kind == "current").cloned(),
        position_sec,
        shuffle,
        repeat,
        volume,
        muted,
    })
}

/// Reserva posiciones para inserciones nuevas. Delante desplaza todos los
/// pendientes; detrás empalma tras el máximo actual.
async fn reserve_pending_positions(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    count: usize,
    at_front: bool,
) -> Result<i64, String> {
    let shift = count as i64;
    if at_front {
        sqlx::query(
            "UPDATE player_queue_entries SET position = position + ? WHERE kind = 'pending'",
        )
        .bind(shift)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(0);
    }
    let max: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) FROM player_queue_entries WHERE kind = 'pending'",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(max + 1)
}

/// Encola tracks de biblioteca (devuelve los `entry_id` en el mismo orden).
/// `at_front` inserta delante de los pendientes (regla `playNext`); si no,
/// al final. La FK valida que los tracks existan.
pub async fn perform_enqueue(
    db: &DbPool,
    track_ids: &[i64],
    at_front: bool,
) -> Result<Vec<String>, String> {
    validate_track_ids(track_ids)?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let start = reserve_pending_positions(&mut tx, track_ids.len(), at_front).await?;

    let mut entry_ids = Vec::with_capacity(track_ids.len());
    for (i, track_id) in track_ids.iter().enumerate() {
        let entry_id = new_entry_id();
        let position = start + i as i64;
        let inserted = sqlx::query(
            "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
             VALUES (?, ?, 'pending', ?)",
        )
        .bind(&entry_id)
        .bind(track_id)
        .bind(position)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        if inserted != 1 {
            tx.rollback()
                .await
                .map_err(|e| format!("rollback al encolar {track_id}: {e}"))?;
            return Err(format!("No se pudo encolar el track {track_id} (¿existe?)"));
        }
        entry_ids.push(entry_id);
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(entry_ids)
}

/// Encola un ítem del proveedor sin materializarlo en biblioteca. La única
/// escritura es la entrada efímera: ni `tracks` ni `track_sources` se tocan.
pub async fn perform_enqueue_provider_item(
    db: &DbPool,
    service_id: i64,
    service_track_id: &str,
    at_front: bool,
) -> Result<String, String> {
    if service_id <= 0 {
        return Err(format!("service_id inválido: {service_id}"));
    }
    let service_track_id = service_track_id.trim();
    if service_track_id.is_empty() {
        return Err("service_track_id vacío: se esperaba un id del proveedor".to_string());
    }
    let service_name: String = sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
        .bind(service_id)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("El servicio {service_id} no existe"))?;
    if !matches!(
        service_name.to_ascii_lowercase().as_str(),
        "qobuz" | "tidal"
    ) {
        return Err(format!(
            "Proveedor sin reproducción o promoción de entradas efímeras: {service_name}"
        ));
    }
    let preflight_error =
        crate::services::streaming::eligible_streaming_account_ids(db, &service_name)
            .await?
            .is_empty()
            .then(|| "reconnect: no hay una cuenta habilitada con streaming".to_string());

    let entry_id = new_entry_id();
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let position = reserve_pending_positions(&mut tx, 1, at_front).await?;
    sqlx::query(
        "INSERT INTO player_queue_entries \
         (entry_id, service_id, service_track_id, kind, position, promotion_error) \
         VALUES (?, ?, ?, 'pending', ?, ?)",
    )
    .bind(&entry_id)
    .bind(service_id)
    .bind(service_track_id)
    .bind(position)
    .bind(preflight_error)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(entry_id)
}

/// `previous` sobre cualquier identidad (biblioteca o efímera): devuelve la
/// pista que sonaba al frente de los pendientes y activa la entrada histórica.
pub async fn perform_return_to_pending_and_set_current_entry(
    db: &DbPool,
    entry_id: &str,
) -> Result<PlaybackQueueEntryDto, String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE player_queue_entries SET position = position + 1 WHERE kind = 'pending'")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE player_queue_entries SET kind = 'pending', position = 0 WHERE kind = 'current'",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let updated = sqlx::query(
        "UPDATE player_queue_entries SET kind = 'current', position = 0 \
         WHERE entry_id = ? AND kind = 'history'",
    )
    .bind(entry_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    if updated != 1 {
        tx.rollback()
            .await
            .map_err(|e| format!("rollback tras previous sin fila: {e}"))?;
        return Err(format!(
            "La entrada {entry_id} no está en el historial (o ya no está)"
        ));
    }
    sqlx::query(
        "UPDATE player_state SET position_sec = 0, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    perform_get_queue_entry(db, entry_id).await
}

/// Quita UNA entrada pendiente por su identidad. El historial y la pista
/// actual no se tocan: la UI solo elimina de «a continuación».
pub async fn perform_remove_queue_entry(db: &DbPool, entry_id: &str) -> Result<(), String> {
    let deleted =
        sqlx::query("DELETE FROM player_queue_entries WHERE entry_id = ? AND kind = 'pending'")
            .bind(entry_id)
            .execute(db)
            .await
            .map_err(|e| e.to_string())?
            .rows_affected();
    if deleted != 1 {
        return Err(format!(
            "La entrada {entry_id} no está en la cola pendiente (o ya no está)"
        ));
    }
    Ok(())
}

/// Fija el orden EXACTO de los pendientes (reasignación total de posiciones).
/// Cubre reordenar a mano, aplicar la permutación de shuffle y restaurar el
/// orden base al apagarlo: la UI envía la lista completa en su nuevo orden.
pub async fn perform_reorder_queue(db: &DbPool, entry_ids: &[String]) -> Result<(), String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let pending: Vec<String> = sqlx::query_scalar(
        "SELECT entry_id FROM player_queue_entries WHERE kind = 'pending' ORDER BY position ASC",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    let same_set = pending.len() == entry_ids.len()
        && entry_ids.iter().all(|id| pending.iter().any(|p| p == id));
    if !same_set {
        tx.rollback()
            .await
            .map_err(|e| format!("rollback al reordenar: {e}"))?;
        return Err("La lista de reordenación no coincide con los pendientes actuales".to_string());
    }
    for (position, entry_id) in entry_ids.iter().enumerate() {
        sqlx::query("UPDATE player_queue_entries SET position = ? WHERE entry_id = ?")
            .bind(position as i64)
            .bind(entry_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Reconstruye el ciclo de `repeat all` desde el backend (#132, P01): convierte
/// `history` + `current` (identidades dadas en `order`, ya deduplicadas y
/// permutadas por el llamante) en la cola pendiente nueva. Conserva la
/// identidad de cada entrada (`entry_id`): la UI reconoce las suyas y el
/// crédito de escucha no depende de ello, pero el agregado mantiene la regla
/// P00 de identidades estables. El `current` vigente se convierte a pendiente:
/// `perform_set_current` de la cabeza lo archivará de nuevo al consumirla.
pub async fn perform_rebuild_cycle(db: &DbPool, order: &[String]) -> Result<(), String> {
    if order.is_empty() {
        return Err("order vacío: se esperaba al menos una entrada del ciclo".to_string());
    }
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM player_queue_entries WHERE kind = 'pending'")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for (position, entry_id) in order.iter().enumerate() {
        let updated = sqlx::query(
            "UPDATE player_queue_entries SET kind = 'pending', position = ? \
             WHERE entry_id = ? AND kind IN ('history', 'current')",
        )
        .bind(position as i64)
        .bind(entry_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        if updated != 1 {
            tx.rollback()
                .await
                .map_err(|e| format!("rollback al reconstruir el ciclo: {e}"))?;
            return Err(format!(
                "La entrada {entry_id} no está en el historial ni es la pista actual"
            ));
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Recorta el historial al límite: elimina las más antiguas (posición menor).
async fn trim_history(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<(), String> {
    // Conserva las HISTORY_LIMIT más recientes: elimina las estrictamente más
    // antiguas que la fila en el límite (índice HISTORY_LIMIT-1 por orden DESC).
    sqlx::query(
        "DELETE FROM player_queue_entries WHERE kind = 'history' AND position < \
         (SELECT position FROM player_queue_entries WHERE kind = 'history' \
          ORDER BY position DESC LIMIT 1 OFFSET ?)",
    )
    .bind(HISTORY_LIMIT - 1)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Mueve una entrada a «pista actual» y resetea la posición a 0. La entrada
/// puede venir de los pendientes o del historial (regla `previous`); si no
/// existe aún (play inmediato de un track no encolado) se crea. La pista
/// anterior pasa al historial en orden de reproducción, con recorte al límite.
pub async fn perform_set_current(db: &DbPool, entry_id: &str, track_id: i64) -> Result<(), String> {
    if track_id <= 0 {
        return Err(format!("track_id inválido: {track_id}"));
    }
    if entry_id.trim().is_empty() {
        return Err("entry_id vacío".to_string());
    }
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;

    // La entrada deja su lista de origen (si estaba en una).
    sqlx::query("DELETE FROM player_queue_entries WHERE entry_id = ? AND kind != 'current'")
        .bind(entry_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // La pista actual anterior pasa al historial (al final, orden de
    // reproducción). Su posición puede coincidir con restos previos: se
    // reasigna con el máximo + 1.
    let promoted = sqlx::query(
        "UPDATE player_queue_entries \
         SET kind = 'history', \
             position = (SELECT COALESCE(MAX(position), -1) FROM player_queue_entries \
                         WHERE kind = 'history') + 1 \
         WHERE kind = 'current'",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    if promoted > 1 {
        // Inalcanzable con el índice único parcial; si una regresión lo
        // rompiera, aquí no se confirma el estado ambiguo.
        tx.rollback()
            .await
            .map_err(|e| format!("rollback con current múltiple: {e}"))?;
        return Err("Estado ambiguo: había más de una pista actual".to_string());
    }
    if promoted == 1 {
        trim_history(&mut tx).await?;
    }

    sqlx::query(
        "INSERT INTO player_queue_entries (entry_id, track_id, kind, position) \
         VALUES (?, ?, 'current', 0) \
         ON CONFLICT(entry_id) DO UPDATE SET kind = 'current', position = 0",
    )
    .bind(entry_id)
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        // FK: la pista no existe → error definitivo, no transitorio.
        e.to_string()
    })?;

    sqlx::query(
        "UPDATE player_state SET position_sec = 0, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Lee una entrada concreta por su identidad estable.
pub async fn perform_get_queue_entry(
    db: &DbPool,
    entry_id: &str,
) -> Result<PlaybackQueueEntryDto, String> {
    let row: Option<(
        String,
        i64,
        Option<i64>,
        Option<String>,
        String,
        i64,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT entry_id, COALESCE(track_id, 0), service_id, service_track_id, kind, position, promotion_error \
         FROM player_queue_entries WHERE entry_id = ?",
    )
    .bind(entry_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;
    let (entry_id, track_id, service_id, service_track_id, kind, position, promotion_error) =
        row.ok_or_else(|| format!("La entrada {entry_id} no existe"))?;
    Ok(PlaybackQueueEntryDto {
        entry_id,
        track_id,
        service_id,
        service_track_id,
        kind,
        position,
        promotion_error,
    })
}

/// Persiste el motivo de preflight/promoción sin tocar una entrada ya convertida.
pub async fn perform_set_provider_promotion_error(
    db: &DbPool,
    entry_id: &str,
    reason: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE player_queue_entries SET promotion_error = ? \
         WHERE entry_id = ? AND track_id IS NULL",
    )
    .bind(reason)
    .bind(entry_id)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Activa una entrada por su identidad estable, sea de biblioteca o efímera.
/// La transacción cubre también la carrera con la promoción: el `UPDATE` no
/// depende de cómo era la identidad cuando empezó la llamada.
pub async fn perform_set_current_entry(
    db: &DbPool,
    entry_id: &str,
) -> Result<PlaybackQueueEntryDto, String> {
    if entry_id.trim().is_empty() {
        return Err("entry_id vacío".to_string());
    }
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| e.to_string())?;
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM player_queue_entries WHERE entry_id = ?")
            .bind(entry_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    if exists != 1 {
        tx.rollback()
            .await
            .map_err(|e| format!("rollback tras activar entrada inexistente: {e}"))?;
        return Err(format!("La entrada {entry_id} no existe"));
    }
    let already_current: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM player_queue_entries \
         WHERE entry_id = ? AND kind = 'current'",
    )
    .bind(entry_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if already_current == 0 {
        let promoted = sqlx::query(
            "UPDATE player_queue_entries \
             SET kind = 'history', \
                 position = (SELECT COALESCE(MAX(position), -1) FROM player_queue_entries \
                             WHERE kind = 'history') + 1 \
             WHERE kind = 'current'",
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        if promoted > 1 {
            tx.rollback()
                .await
                .map_err(|e| format!("rollback con current múltiple: {e}"))?;
            return Err("Estado ambiguo: había más de una pista actual".to_string());
        }
        if promoted == 1 {
            trim_history(&mut tx).await?;
        }
        let updated = sqlx::query(
            "UPDATE player_queue_entries SET kind = 'current', position = 0 \
             WHERE entry_id = ? AND kind IN ('pending', 'history')",
        )
        .bind(entry_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        if updated != 1 {
            tx.rollback()
                .await
                .map_err(|e| format!("rollback al activar entrada: {e}"))?;
            return Err(format!(
                "La entrada {entry_id} no está pendiente ni en el historial"
            ));
        }
    }
    sqlx::query(
        "UPDATE player_state SET position_sec = 0, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    perform_get_queue_entry(db, entry_id).await
}

/// Vacía la cola pendiente. La pista actual y el historial no se tocan (la
/// regla de la UI: `clearQueue` nunca corta lo que suena).
pub async fn perform_clear_queue(db: &DbPool) -> Result<(), String> {
    sqlx::query("DELETE FROM player_queue_entries WHERE kind = 'pending'")
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Vacía el historial de reproducción.
pub async fn perform_clear_history(db: &DbPool) -> Result<(), String> {
    sqlx::query("DELETE FROM player_queue_entries WHERE kind = 'history'")
        .execute(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Guarda la posición de reproducción (pausa, seek y ocultar ventana).
pub async fn perform_set_position(db: &DbPool, position_sec: f64) -> Result<(), String> {
    if !position_sec.is_finite() || position_sec < 0.0 {
        return Err(format!("position_sec inválido: {position_sec}"));
    }
    sqlx::query(
        "UPDATE player_state SET position_sec = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
    )
    .bind(position_sec)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Guarda los modos (shuffle y repeat). Persisten entre reinicios.
pub async fn perform_set_modes(db: &DbPool, shuffle: bool, repeat: &str) -> Result<(), String> {
    validate_repeat(repeat)?;
    sqlx::query(
        "UPDATE player_state SET shuffle = ?, repeat = ?, updated_at = CURRENT_TIMESTAMP \
         WHERE id = 1",
    )
    .bind(shuffle)
    .bind(repeat)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Cambia `shuffle` y ORDENA de nuevo la cola pendiente en la misma
/// transacción (#133, P02): al activarlo se permuta el orden vigente; al
/// apagarlo se vuelve al orden de encolado (`enqueued_at`, `rowid`). Devuelve
/// las `entry_id` en su nuevo orden para que la UI y MPRIS proyecten lo mismo.
#[cfg(target_os = "linux")]
pub async fn perform_set_shuffle(db: &DbPool, enabled: bool) -> Result<Vec<String>, String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut pending: Vec<String> = sqlx::query_scalar(
        "SELECT entry_id FROM player_queue_entries WHERE kind = 'pending' \
         ORDER BY position ASC, enqueued_at ASC, rowid ASC",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    if enabled {
        use rand::seq::SliceRandom;
        pending.shuffle(&mut rand::thread_rng());
    } else {
        // Sin una copia separada del orden base (ADR-0010 lo deja para la
        // evolución de la cola), el orden de encolado es el verificable.
        pending = sqlx::query_scalar(
            "SELECT entry_id FROM player_queue_entries WHERE kind = 'pending' \
             ORDER BY enqueued_at ASC, rowid ASC",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    for (position, entry_id) in pending.iter().enumerate() {
        sqlx::query("UPDATE player_queue_entries SET position = ? WHERE entry_id = ?")
            .bind(position as i64)
            .bind(entry_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE player_state SET shuffle = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 1")
        .bind(enabled)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(pending)
}

/// Guarda volumen y silencio (0..1; el silencio lo modela la UI como volumen
/// efectivo 0 hacia el motor, pero su interruptor se persiste aparte).
pub async fn perform_set_volume(db: &DbPool, volume: f64, muted: bool) -> Result<(), String> {
    if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
        return Err(format!("volume inválido: {volume} (0..1)"));
    }
    sqlx::query(
        "UPDATE player_state SET volume = ?, muted = ?, updated_at = CURRENT_TIMESTAMP \
         WHERE id = 1",
    )
    .bind(volume)
    .bind(muted)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Vuelca TODO el agregado (cola, historial, pista actual y posición).
/// `stop()` de la UI lo usa: los modos y el volumen se conservan.
pub async fn perform_reset(db: &DbPool) -> Result<(), String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM player_queue_entries")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE player_state SET position_sec = 0, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Comandos Tauri
// ---------------------------------------------------------------------------

/// Instantánea completa de cola, historial, pista actual, posición y modos.
/// Es la primera invocación de la restauración al arrancar.
#[tauri::command]
pub async fn playback_queue_get(
    state: State<'_, crate::AppState>,
) -> Result<PlaybackQueueSnapshot, String> {
    perform_get_playback_queue(&state.db).await
}

/// Encola tracks de biblioteca y devuelve sus `entry_id` en el mismo orden.
#[tauri::command]
pub async fn playback_queue_enqueue(
    state: State<'_, crate::AppState>,
    track_ids: Vec<i64>,
    at_front: bool,
) -> Result<Vec<String>, String> {
    perform_enqueue(&state.db, &track_ids, at_front).await
}

/// Encola un ítem del proveedor como entrada efímera, sin crear biblioteca.
#[tauri::command]
pub async fn playback_queue_enqueue_provider_item(
    state: State<'_, crate::AppState>,
    service_id: i64,
    service_track_id: String,
    at_front: bool,
) -> Result<String, String> {
    perform_enqueue_provider_item(&state.db, service_id, &service_track_id, at_front).await
}

/// Quita una entrada pendiente de la cola.
#[tauri::command]
pub async fn playback_queue_remove(
    state: State<'_, crate::AppState>,
    entry_id: String,
) -> Result<(), String> {
    perform_remove_queue_entry(&state.db, &entry_id).await
}

/// Fija el orden exacto de la cola pendiente (reordenar, shuffle on/off).
#[tauri::command]
pub async fn playback_queue_reorder(
    state: State<'_, crate::AppState>,
    entry_ids: Vec<String>,
) -> Result<(), String> {
    perform_reorder_queue(&state.db, &entry_ids).await
}

/// Mueve una entrada a «pista actual» (la anterior pasa al historial) y
/// resetea la posición a 0.
#[tauri::command]
pub async fn playback_queue_set_current(
    state: State<'_, crate::AppState>,
    entry_id: String,
    track_id: i64,
) -> Result<(), String> {
    perform_set_current(&state.db, &entry_id, track_id).await
}

/// Activa una entrada de cola por su identidad, sin exigir un Track.
#[tauri::command]
pub async fn playback_queue_set_current_entry(
    state: State<'_, crate::AppState>,
    entry_id: String,
) -> Result<PlaybackQueueEntryDto, String> {
    perform_set_current_entry(&state.db, &entry_id).await
}

/// Vacía la cola pendiente (la pista actual y el historial se conservan).
#[tauri::command]
pub async fn playback_queue_clear(state: State<'_, crate::AppState>) -> Result<(), String> {
    perform_clear_queue(&state.db).await
}

/// Vacía el historial de reproducción.
#[tauri::command]
pub async fn playback_history_clear(state: State<'_, crate::AppState>) -> Result<(), String> {
    perform_clear_history(&state.db).await
}

/// Guarda la posición de reproducción.
#[tauri::command]
pub async fn playback_state_set_position(
    state: State<'_, crate::AppState>,
    position_sec: f64,
) -> Result<(), String> {
    perform_set_position(&state.db, position_sec).await
}

/// Guarda shuffle y repeat. El panel MPRIS refleja el modo persistido: hasta
/// #133 la UI lo empujaba aparte con `player_mpris_update`; ahora la fuente es
/// esta misma fila de `player_state`.
#[tauri::command]
pub async fn playback_state_set_modes(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
    shuffle: bool,
    repeat: String,
) -> Result<(), String> {
    perform_set_modes(&state.db, shuffle, &repeat).await?;
    if let Some(mpris) = app.try_state::<crate::services::mpris::MprisService>() {
        mpris.update_modes(shuffle, &repeat);
    }
    Ok(())
}

/// Guarda volumen y silencio.
#[tauri::command]
pub async fn playback_state_set_volume(
    state: State<'_, crate::AppState>,
    volume: f64,
    muted: bool,
) -> Result<(), String> {
    perform_set_volume(&state.db, volume, muted).await
}

/// Vuelca todo el agregado de cola (cola, historial, current, posición).
#[tauri::command]
pub async fn playback_state_reset(state: State<'_, crate::AppState>) -> Result<(), String> {
    perform_reset(&state.db).await
}
