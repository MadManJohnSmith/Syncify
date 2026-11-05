#[allow(unused_imports)]
use super::*;

// Escucha local de pistas ya descargadas (estadisticas de reproduccion).
//
// La ruta de audio por protocolo `syncify-media://` que vivia en este modulo fue
// retirada: el motor nativo (`player_load_track`) la sustituyo y ya no quedaba
// invocador que poblara sus concesiones en memoria. Ver C-19 en
// `docs/Deuda_Tecnica_y_UX.md`.

// ---------------------------------------------------------------------------
// Fase 1 del player: estadísticas de reproducción local (play_count).
// ---------------------------------------------------------------------------

/// Resultado de una invocación de `record_local_play`.
///
/// `counted` es `true` SOLO cuando esta invocación insertó la sesión de
/// escucha y por tanto incrementó `play_count`; un reintento con el mismo
/// `listen_session_id` (respuesta IPC perdida) devuelve `counted: false` con
/// las estadísticas intactas — nunca un segundo incremento.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlayStats {
    pub track_id: i64,
    pub play_count: i64,
    pub last_played: Option<String>,
    pub counted: bool,
}

/// Normaliza el identificador de sesión de escucha a su forma canónica
/// (minúsculas con guiones) ANTES de tocar la PK de `playback_listens`:
/// `"ABC…"` y `"abc…"` son el mismo token y no deben poder contar dos veces.
/// Acepta las formas que parsea `uuid` (hifenada, simple, braced, urn).
fn normalize_listen_session_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("listen_session_id vacío: se esperaba un UUID".to_string());
    }
    uuid::Uuid::parse_str(trimmed)
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| format!("listen_session_id inválido (se esperaba UUID): {raw:?}"))
}

/// Núcleo testeable de `record_local_play` (patrón `perform_*` del resto de
/// comandos): recibe el pool directamente y no `State`.
///
/// Semántica transaccional — INSERT-primero, nunca SELECT-then-INSERT:
///   1. `BEGIN`.
///   2. `INSERT … ON CONFLICT(listen_session_id) DO NOTHING` — la PK decide
///      el doble conteo en una sola sentencia atómica.
///   3. `rows_affected == 1` → `UPDATE tracks …`; exigir `rows_affected == 1`
///      o ROLLBACK + Err (cinturón además de la FK, por si una regresión
///      futura desactiva `PRAGMA foreign_keys`).
///   4. `rows_affected == 0` → misma pista: devolver estadísticas con
///      `counted: false` SIN UPDATE ni cambio de `last_played`; pista
///      distinta → ROLLBACK + Err (el token pertenece a otra escucha).
///
/// Límite de confianza documentado (doc fase 1 / MPRIS): el backend garantiza
/// atomicidad e idempotencia, no puede certificar por sí solo los 30 s
/// audibles de la fase 1 — eso lo acredita la UI antes de invocar.
pub async fn perform_record_local_play(
    db: &DbPool,
    track_id: i64,
    listen_session_id: &str,
) -> Result<PlayStats, String> {
    if track_id <= 0 {
        return Err(format!("track_id inválido: {track_id}"));
    }
    let session_id = normalize_listen_session_id(listen_session_id)?;

    // (1) BEGIN. Primera sentencia es el INSERT (escritura): una transacción
    // DEFERRED no sufre SQLITE_BUSY_SNAPSHOT aquí — el lock de escritura se
    // toma en el INSERT y busy_timeout (30 s, db.rs) serializa escritores.
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;

    // (2) INSERT-primero: el conflicto de PK decide sin SELECT previo, también
    // bajo llamadas concurrentes con el mismo token. Con FK activa (db.rs
    // activa `PRAGMA foreign_keys = ON` en CADA conexión del pool), una pista
    // inexistente/borrada muere aquí: error definitivo, no transitorio.
    let inserted = sqlx::query(
        "INSERT INTO playback_listens (listen_session_id, track_id) VALUES (?, ?) \
         ON CONFLICT(listen_session_id) DO NOTHING",
    )
    .bind(&session_id)
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();

    if inserted == 1 {
        // (3) Primera vez que esta sesión cuenta.
        let updated = sqlx::query(
            "UPDATE tracks \
             SET play_count = play_count + 1, last_played = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(track_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        if updated != 1 {
            // Cinturón además de la FK: si una regresión futura desactiva
            // FK, la sesión NUNCA se persiste sin su incremento asociado.
            tx.rollback()
                .await
                .map_err(|e| format!("rollback tras UPDATE sin fila: {e}"))?;
            return Err(format!(
                "El track {track_id} no existe; no se cuenta la escucha"
            ));
        }
        let (play_count, last_played): (i64, Option<String>) =
            sqlx::query_as("SELECT play_count, last_played FROM tracks WHERE id = ?")
                .bind(track_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        tracing::info!(
            "record_local_play: track_id={} contado (play_count={play_count})",
            track_id
        );
        return Ok(PlayStats {
            track_id,
            play_count,
            last_played,
            counted: true,
        });
    }

    // (4) Conflicto de PK: la sesión ya se registró antes.
    let existing_track_id: Option<i64> =
        sqlx::query_scalar("SELECT track_id FROM playback_listens WHERE listen_session_id = ?")
            .bind(&session_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

    match existing_track_id {
        Some(existing) if existing == track_id => {
            // Reintento idempotente: SIN UPDATE, last_played intacto.
            let (play_count, last_played): (i64, Option<String>) =
                sqlx::query_as("SELECT play_count, last_played FROM tracks WHERE id = ?")
                    .bind(track_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            tx.commit().await.map_err(|e| e.to_string())?;
            tracing::debug!(
                "record_local_play: track_id={} ya contado con esta sesión (dedupe)",
                track_id
            );
            Ok(PlayStats {
                track_id,
                play_count,
                last_played,
                counted: false,
            })
        }
        Some(existing) => {
            tx.rollback()
                .await
                .map_err(|e| format!("rollback tras conflicto con otra pista: {e}"))?;
            Err(format!(
                "El token de escucha ya pertenece al track {existing}, no al {track_id}"
            ))
        }
        None => {
            // Inalcanzable salvo carrera exótica (ON CONFLICT sin fila
            // visible): fallar de forma explícita sin persistir nada.
            tx.rollback()
                .await
                .map_err(|e| format!("rollback tras conflicto sin fila: {e}"))?;
            Err(format!(
                "Conflicto de sesión de escucha {session_id} sin fila registrada"
            ))
        }
    }
}

/// Cuenta una escucha de una pista local. La UI genera un UUID v4 canónico
/// por reproducción lógica y lo invoca UNA sola vez por sesión al cruzar el
/// umbral de 30 s de reproducción real acreditada; el backend deduplica por
/// PK ante reintentos (respuesta IPC perdida) y rechaza el token reutilizado
/// con otra pista.
#[tauri::command]
pub async fn record_local_play(
    state: State<'_, crate::AppState>,
    track_id: i64,
    listen_session_id: String,
) -> Result<PlayStats, String> {
    tracing::info!("record_local_play: track_id={track_id}");
    perform_record_local_play(&state.db, track_id, &listen_session_id).await
}
