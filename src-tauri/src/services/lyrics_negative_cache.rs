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

use sqlx::SqlitePool;
use syncify_lyrics_domain::ResolutionStatus;
use syncify_lyrics_engine::{
    LyricsCascadeReport, LyricsClient, LyricsResolutionReport, LyricsSourceFailure,
};

/// Caducidad de fábrica cuando la frecuencia de reintento no define una
/// ventana conocida: la misma semana del ajuste por defecto.
pub const DEFAULT_TTL_HOURS: i64 = 168;

/// Ventana de la caché negativa según la cadencia de reintento configurada
/// (`lyrics_config.retry_frequency`). `never` y los valores desconocidos usan
/// la semana de fábrica: desactivan el barrido automático, no la evidencia.
pub fn ttl_hours(retry_frequency: &str) -> i64 {
    match retry_frequency.trim().to_ascii_lowercase().as_str() {
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
        .bind(&failure.detail)
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
    let skip = failing_sources(db, track_id).await?;
    let report = client
        .orchestrate_resolution_reporting(artist, title, album, duration_sec, &skip)
        .await;
    if let Some(track_id) = track_id {
        record_outcome(
            db,
            track_id,
            deletion_mark_standing,
            report.resolution.status == ResolutionStatus::Resolved,
            &report.failures,
            &client.config().retry_frequency,
        )
        .await?;
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
    let skip = failing_sources(db, track_id).await?;
    let report = client
        .fetch_all_sources_skipping(artist, title, duration_sec, &skip)
        .await;
    if let Some(track_id) = track_id {
        record_outcome(
            db,
            track_id,
            deletion_mark_standing,
            report.response.is_some(),
            &report.failures,
            &client.config().retry_frequency,
        )
        .await?;
    }
    Ok(report)
}
