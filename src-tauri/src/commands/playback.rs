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
pub(crate) fn normalize_listen_session_id(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("listen_session_id vacío: se esperaba un UUID".to_string());
    }
    uuid::Uuid::parse_str(trimmed)
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| format!("listen_session_id inválido (se esperaba UUID): {raw:?}"))
}

/// Núcleo transaccional del crédito. El llamador posee la transacción: así la
/// promoción puede convertir la entrada y acreditar la escucha de forma atómica.
///
/// Semántica INSERT-primero, nunca SELECT-then-INSERT:
/// 1. `INSERT … ON CONFLICT(listen_session_id) DO NOTHING` decide el doble conteo.
/// 2. Si insertó, `UPDATE tracks` exige exactamente una fila.
/// 3. Si el token ya existe, sólo acepta la misma pista y no modifica stats.
pub(crate) async fn record_local_play_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    track_id: i64,
    listen_session_id: &str,
) -> Result<PlayStats, String> {
    if track_id <= 0 {
        return Err(format!("track_id inválido: {track_id}"));
    }
    let session_id = normalize_listen_session_id(listen_session_id)?;

    let inserted = sqlx::query(
        "INSERT INTO playback_listens (listen_session_id, track_id) VALUES (?, ?) \
         ON CONFLICT(listen_session_id) DO NOTHING",
    )
    .bind(&session_id)
    .bind(track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();

    if inserted == 1 {
        let updated = sqlx::query(
            "UPDATE tracks \
             SET play_count = play_count + 1, last_played = CURRENT_TIMESTAMP \
             WHERE id = ?",
        )
        .bind(track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
        if updated != 1 {
            return Err(format!(
                "El track {track_id} no existe; no se cuenta la escucha"
            ));
        }
        let (play_count, last_played): (i64, Option<String>) =
            sqlx::query_as("SELECT play_count, last_played FROM tracks WHERE id = ?")
                .bind(track_id)
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| e.to_string())?;
        return Ok(PlayStats {
            track_id,
            play_count,
            last_played,
            counted: true,
        });
    }

    let existing_track_id: Option<i64> =
        sqlx::query_scalar("SELECT track_id FROM playback_listens WHERE listen_session_id = ?")
            .bind(&session_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    match existing_track_id {
        Some(existing) if existing == track_id => {
            let (play_count, last_played): (i64, Option<String>) =
                sqlx::query_as("SELECT play_count, last_played FROM tracks WHERE id = ?")
                    .bind(track_id)
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(|e| e.to_string())?;
            Ok(PlayStats {
                track_id,
                play_count,
                last_played,
                counted: false,
            })
        }
        Some(existing) => Err(format!(
            "El token de escucha ya pertenece al track {existing}, no al {track_id}"
        )),
        None => Err(format!(
            "Conflicto de sesión de escucha {session_id} sin fila registrada"
        )),
    }
}

/// Núcleo testeable de `record_local_play` (patrón `perform_*` del resto de
/// comandos): recibe el pool directamente y no `State`.
pub async fn perform_record_local_play(
    db: &DbPool,
    track_id: i64,
    listen_session_id: &str,
) -> Result<PlayStats, String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let stats = record_local_play_in_tx(&mut tx, track_id, listen_session_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    if stats.counted {
        tracing::info!(
            "record_local_play: track_id={} contado (play_count={})",
            stats.track_id,
            stats.play_count
        );
    } else {
        tracing::debug!(
            "record_local_play: track_id={} ya contado con esta sesión (dedupe)",
            stats.track_id
        );
    }
    Ok(stats)
}

/// Aplica el crédito que produjo el ticker. Una entrada de biblioteca usa el
/// camino directo; una efímera resuelve identidad y promociona antes de contar.
pub async fn perform_apply_listen_credit(
    db: &DbPool,
    credit: &crate::services::playback_session::ListenCredit,
) -> Result<crate::services::playback_promotion::PromotionOutcome, String> {
    use crate::services::playback_promotion::{
        freeze_queue_listen_session, promote_queue_entry, resolve_provider_identity,
    };
    use crate::services::streaming::StreamTarget;

    match &credit.target {
        StreamTarget::Library(track_id) => {
            let stats = perform_record_local_play(db, *track_id, &credit.listen_session_id).await?;
            Ok(
                crate::services::playback_promotion::PromotionOutcome::AlreadyLibrary {
                    track_id: *track_id,
                    counted: stats.counted,
                },
            )
        }
        StreamTarget::Provider(item) => {
            let entry =
                crate::commands::playback_queue::perform_get_queue_entry(db, &item.entry_id)
                    .await?;
            if entry.track_id > 0 {
                let frozen: Option<String> = sqlx::query_scalar(
                    "SELECT listen_session_id FROM player_queue_entries WHERE entry_id = ?",
                )
                .bind(&item.entry_id)
                .fetch_optional(db)
                .await
                .map_err(|e| e.to_string())?;
                if frozen.as_deref() == Some(credit.listen_session_id.as_str()) {
                    promote_queue_entry(
                        db,
                        &item.entry_id,
                        &credit.listen_session_id,
                        Err("la entrada ya fue promocionada".into()),
                    )
                    .await
                } else {
                    let stats =
                        perform_record_local_play(db, entry.track_id, &credit.listen_session_id)
                            .await?;
                    Ok(
                        crate::services::playback_promotion::PromotionOutcome::AlreadyLibrary {
                            track_id: entry.track_id,
                            counted: stats.counted,
                        },
                    )
                }
            } else {
                // El umbral ya ocurrió: congelar precede a la resolución de red
                // para que una promoción concurrente no elija otra sesión.
                freeze_queue_listen_session(db, &item.entry_id, &credit.listen_session_id).await?;
                let resolved =
                    resolve_provider_identity(db, item.service_id, &item.service_track_id).await;
                promote_queue_entry(db, &item.entry_id, &credit.listen_session_id, resolved).await
            }
        }
    }
}

// La acreditación de escucha ya no es un comando IPC (#132, P01): el backend
// acumula los 30 s reales en el ticker (`services::playback_session`) y llama
// a `perform_record_local_play` directamente — la UI no cronometra. La
// semántica transaccional e idempotente no cambia: el umbral lo cruza el
// motor, la deduplicación por PK de `listen_session_id` sigue protegiendo
// frente a reintentos y el token reutilizado con otra pista se rechaza.
