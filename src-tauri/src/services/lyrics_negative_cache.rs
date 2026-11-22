//! Caché negativa persistente de resolución de letras (LY03, issue #184).
//!
//! El motor no conoce la base de datos: devuelve, por Fuente de letras
//! sondeada, el motivo del fallo (`LyricsSourceFailure`). Este módulo traduce
//! esa evidencia a la tabla `lyrics_resolution_failures`: lee las Fuentes con
//! un fallo vigente para saltarlas en la siguiente resolución y escribe los
//! fallos nuevos con caducidad. El resultado positivo no se guarda aquí; se
//! persiste como letra/sidecar y limpia la evidencia negativa de la pista.
//!
//! La Marca de borrado (migración 0098) es voluntad del usuario, no un fallo:
//! las rutas que ya la respetan llegan aquí con `deletion_mark_standing` y no
//! escriben ninguna fila.

use std::collections::HashSet;

use sqlx::{Sqlite, SqlitePool, Transaction};
use syncify_lyrics_domain::ResolutionStatus;
use syncify_lyrics_engine::{
    redact_failure_detail, LyricsCascadeReport, LyricsClient, LyricsResolutionReport,
    LyricsSourceFailure,
};

/// Caducidad de fábrica cuando la frecuencia de reintento no define una
/// ventana conocida: la misma semana del ajuste por defecto.
pub const DEFAULT_TTL_HOURS: i64 = 168;

/// Ventana de la caché negativa según la cadencia de reintento configurada
/// (`lyrics_config.retry_frequency`). `always` significa «no suprimir»: la fila
/// puede registrarse para auditoría, pero `expires_at > datetime('now')` no se
/// cumple y la Fuente se vuelve a sondear en el siguiente intento. `never` y
/// los valores desconocidos usan la semana de fábrica: desactivan el barrido
/// automático, no la evidencia.
pub fn ttl_hours(retry_frequency: &str) -> i64 {
    match retry_frequency.trim().to_ascii_lowercase().as_str() {
        "always" => 0,
        "hourly" => 1,
        "daily" => 24,
        "weekly" => DEFAULT_TTL_HOURS,
        _ => DEFAULT_TTL_HOURS,
    }
}

/// Fuentes cuyo último fallo para esta pista todavía no ha caducado.
pub async fn active_failed_sources(
    db: &SqlitePool,
    track_id: i64,
) -> Result<HashSet<String>, String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT provider_id FROM lyrics_resolution_failures \
         WHERE track_id = ? AND expires_at > datetime('now')",
    )
    .bind(track_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("lyrics negative cache read failed: {e}"))?;
    Ok(rows.into_iter().map(|(provider_id,)| provider_id).collect())
}

/// Guarda los fallos observados en la cascada, refrescando la caducidad de las
/// Fuentes que ya tenían una fila.
pub async fn record_failures(
    db: &SqlitePool,
    track_id: i64,
    failures: &[LyricsSourceFailure],
    retry_frequency: &str,
) -> Result<(), String> {
    if failures.is_empty() {
        return Ok(());
    }
    let expiry_modifier = format!("+{} hours", ttl_hours(retry_frequency));
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("lyrics negative cache write failed: {e}"))?;

    for failure in failures {
        // The engine already redacts credentials at the source of the message;
        // redacting again here guarantees the persistence boundary never stores
        // a URL with `usertoken`/`token` even if a future caller builds the
        // failure by hand.
        let detail = redact_failure_detail(&failure.detail);
        // A resolution can arrive with a track id that no longer exists (stale
        // UI row): the cache is derived data, so it is a no-op instead of
        // turning into an FK error on the resolution path.
        sqlx::query(
            "INSERT INTO lyrics_resolution_failures \
                 (track_id, provider_id, reason, detail, expires_at) \
             SELECT ?, ?, ?, ?, datetime('now', ?) \
             WHERE EXISTS (SELECT 1 FROM tracks WHERE id = ?) \
             ON CONFLICT(track_id, provider_id) DO UPDATE SET \
                 reason = excluded.reason, \
                 detail = excluded.detail, \
                 recorded_at = datetime('now'), \
                 expires_at = excluded.expires_at",
        )
        .bind(track_id)
        .bind(&failure.provider_id)
        .bind(failure.kind.as_str())
        .bind(&detail)
        .bind(&expiry_modifier)
        .bind(track_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("lyrics negative cache write failed: {e}"))?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("lyrics negative cache write failed: {e}"))
}

/// Olvida toda la evidencia negativa: la causa de un fallo puede ser la
/// configuración o unas credenciales que acaban de cambiar, así que las filas
/// anteriores dejan de ser válidas. Se llama desde los comandos que cambian
/// esos ajustes y desde el reset completo.
pub async fn clear_all(db: &SqlitePool) -> Result<(), String> {
    clear_all_executor(db).await
}

/// Variante para una transacción abierta por el llamador: el reset de ajustes
/// y la invalidación de la caché son una sola unidad.
pub async fn clear_all_in_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<(), String> {
    clear_all_executor(&mut **tx).await
}

async fn clear_all_executor<'e, E>(executor: E) -> Result<(), String>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    sqlx::query("DELETE FROM lyrics_resolution_failures")
        .execute(executor)
        .await
        .map_err(|e| format!("lyrics negative cache clear failed: {e}"))?;
    Ok(())
}

/// Olvida la evidencia negativa de una pista: se llama cuando una resolución
/// positiva la deja obsoleta.
pub async fn clear_for_track(db: &SqlitePool, track_id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM lyrics_resolution_failures WHERE track_id = ?")
        .bind(track_id)
        .execute(db)
        .await
        .map_err(|e| format!("lyrics negative cache clear failed: {e}"))?;
    Ok(())
}

async fn record_outcome(
    db: &SqlitePool,
    track_id: i64,
    deletion_mark_standing: bool,
    resolved: bool,
    failures: &[LyricsSourceFailure],
    retry_frequency: &str,
) -> Result<(), String> {
    if deletion_mark_standing {
        return Ok(());
    }
    if resolved {
        return clear_for_track(db, track_id).await;
    }
    record_failures(db, track_id, failures, retry_frequency).await
}

async fn failing_sources(
    db: &SqlitePool,
    track_id: Option<i64>,
) -> Result<HashSet<String>, String> {
    match track_id {
        Some(track_id) => active_failed_sources(db, track_id).await,
        None => Ok(HashSet::new()),
    }
}

/// Orquestación con caché negativa para la resolución on-demand y los barridos
/// que producen una `LyricsResolution`.
#[allow(clippy::too_many_arguments)]
pub async fn resolve_with_negative_cache(
    db: &SqlitePool,
    client: &LyricsClient,
    track_id: Option<i64>,
    artist: &str,
    title: &str,
    album: Option<&str>,
    duration_sec: f64,
    deletion_mark_standing: bool,
) -> Result<LyricsResolutionReport, String> {
    let skip = match failing_sources(db, track_id).await {
        Ok(skip) => skip,
        Err(error) => {
            tracing::warn!(
                track_id,
                error = %error,
                "no se pudo leer la caché negativa de letras; se resuelve sin saltar Fuentes"
            );
            HashSet::new()
        }
    };
    let report = client
        .orchestrate_resolution_reporting(artist, title, album, duration_sec, &skip)
        .await;
    if let Some(track_id) = track_id {
        let outcome = record_outcome(
            db,
            track_id,
            deletion_mark_standing,
            report.resolution.status == ResolutionStatus::Resolved,
            &report.failures,
            &client.config().retry_frequency,
        )
        .await;
        if let Err(error) = outcome {
            tracing::warn!(
                track_id,
                error = %error,
                "no se pudo actualizar la caché negativa de letras; la resolución sigue"
            );
        }
    }
    Ok(report)
}

/// Lo mismo para la ruta batch, que consume la respuesta cruda del motor.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_with_negative_cache(
    db: &SqlitePool,
    client: &LyricsClient,
    track_id: Option<i64>,
    artist: &str,
    title: &str,
    duration_sec: f64,
    deletion_mark_standing: bool,
) -> Result<LyricsCascadeReport, String> {
    let skip = match failing_sources(db, track_id).await {
        Ok(skip) => skip,
        Err(error) => {
            tracing::warn!(
                track_id,
                error = %error,
                "no se pudo leer la caché negativa de letras; se resuelve sin saltar Fuentes"
            );
            HashSet::new()
        }
    };
    let report = client
        .fetch_all_sources_skipping(artist, title, duration_sec, &skip)
        .await;
    if let Some(track_id) = track_id {
        let outcome = record_outcome(
            db,
            track_id,
            deletion_mark_standing,
            report.response.is_some(),
            &report.failures,
            &client.config().retry_frequency,
        )
        .await;
        if let Err(error) = outcome {
            tracing::warn!(
                track_id,
                error = %error,
                "no se pudo actualizar la caché negativa de letras; la resolución sigue"
            );
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Un resultado positivo deja obsoleta toda la evidencia negativa de la
    /// pista: `record_outcome` es la rama que lo hace y debe limpiar, no
    /// refrescar la fila. La migración real se aplica para que la tabla y la FK
    /// sean las de producción.
    #[tokio::test]
    async fn positive_resolution_clears_the_track_failure_rows() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool en memoria");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migraciones reales");
        sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Song')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO lyrics_resolution_failures \
                 (track_id, provider_id, reason, detail, expires_at) \
             VALUES (1, 'lrclib_synced', 'not_found', 'stale', datetime('now', '+1 day'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        record_outcome(&pool, 1, false, true, &[], "weekly")
            .await
            .expect("la limpieza positiva no falla");

        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM lyrics_resolution_failures WHERE track_id = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            remaining, 0,
            "una resolución positiva limpia la evidencia negativa de la pista"
        );
    }
}
