//! Superficie de revisión de identidad de pista (#383, continuación de #73).
//!
//! El sync deja en `track_identity_reviews` (0115) los casos que la huella no
//! pudo decidir: el ítem del proveedor se importó como Track propio y la
//! evidencia de ambos lados quedó congelada. Este módulo es el único lugar que
//! lee esa cola para la UI y el único que la cierra.
//!
//! Reglas que fija:
//!
//!   * **Leer no decide.** `list_pending` solo devuelve revisiones `pending`
//!     con la evidencia congelada (`incoming_*` y los candidatos locales).
//!   * **Cerrar es definitivo.** La resolución se escribe en una transacción
//!     `BEGIN IMMEDIATE` junto con `resolved_at`; una revisión ya cerrada no se
//!     reabre ni se vuelve a resolver.
//!   * **Separar y descartar no tocan la biblioteca.** Solo escriben el estado
//!     de la revisión.
//!   * **Fusionar no pierde filas en silencio.** El Track importado se pliega
//!     en el elegido conservando fuentes, pertenencias, favoritos, playlists y
//!     el ledger de ediciones. Si queda una referencia que el merge no sabe
//!     mover (p. ej. una descarga en ambos lados), se aborta la transacción y
//!     se explica qué la bloquea; nunca se borra la pista con datos colgando.
//!
//! La pista importada se identifica por la clave canónica del ADR-0001:
//! `track_sources(service_id, service_track_id)`. Si el ítem está ligado a más
//! de una pista, no se elige una al azar: la revisión queda abierta con un
//! error accionable.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sqlx::{Sqlite, Transaction};
use syncify_core_domain::quality::{classify_audio_tier, AudioTier};

use crate::db::DbPool;
use crate::services::library_membership::refresh_track_library_added_at;

/// Tablas que referencian `tracks(id)` y por tanto deben quedar vacías para
/// poder borrar la pista plegada. La lista es un contrato verificado por test
/// contra `pragma_foreign_key_list`: una migración futura que añada otra FK a
/// `tracks` obliga a decidir aquí qué hacer con sus filas.
///
/// `edition_legacy_collisions` no entra a propósito: no tiene FK a `tracks`,
/// es una foto histórica de las colisiones del ledger legacy y
/// `download/resolver.rs` solo lee `service_id`/`service_track_id`. Repuntarla
/// reescribiría la auditoría sin arreglar ninguna referencia operativa.
pub const TRACK_REFERENCE_TABLES: &[(&str, &str)] = &[
    ("download_entitlements", "track_id"),
    ("download_queue", "track_id"),
    ("downloads", "track_id"),
    ("edition_item_tracks", "track_id"),
    ("enrichment_progress", "track_id"),
    ("library_entries", "track_id"),
    ("library_memberships", "track_id"),
    ("lyrics", "track_id"),
    ("lyrics_deletion_marks", "track_id"),
    ("lyrics_resolution_failures", "track_id"),
    ("lyrics_restored_formats", "track_id"),
    ("playback_listens", "track_id"),
    ("player_queue_entries", "track_id"),
    ("playlist_tracks", "track_id"),
    ("track_artists", "track_id"),
    ("track_credits", "track_id"),
    ("track_identity_review_candidates", "track_id"),
    ("track_identity_reviews", "resolved_track_id"),
    ("track_sources", "track_id"),
];

/// Evidencia congelada de un candidato local en el momento de la detección.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityReviewCandidate {
    pub track_id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub duration_delta_ms: Option<i64>,
}

/// Una revisión `pending` con los dos lados de la comparación.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackIdentityReview {
    pub id: i64,
    pub service_id: i64,
    pub service_name: String,
    pub account_id: Option<i64>,
    pub account_name: Option<String>,
    pub service_track_id: String,
    pub reason: String,
    pub status: String,
    pub incoming_title: Option<String>,
    pub incoming_artist: Option<String>,
    pub incoming_album: Option<String>,
    pub incoming_duration_ms: Option<i64>,
    pub incoming_isrc: Option<String>,
    /// Track al que `track_sources` liga hoy el ítem del proveedor. Es la pista
    /// que se pliega si la persona elige `merged`; `None` si el ítem ya no
    /// está en la biblioteca.
    pub linked_track_id: Option<i64>,
    pub candidates: Vec<IdentityReviewCandidate>,
    pub created_at: String,
    pub updated_at: String,
}

/// Acción humana sobre una revisión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityReviewResolution {
    /// Las dos pistas son la misma grabación; se conserva `resolved_track_id`.
    Merged,
    /// Son grabaciones distintas; no se toca ninguna pista.
    Separate,
    /// La evidencia dejó de aplicar; no se toca ninguna pista.
    Dismissed,
}

impl IdentityReviewResolution {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "merged" => Ok(Self::Merged),
            "separate" => Ok(Self::Separate),
            "dismissed" => Ok(Self::Dismissed),
            other => Err(format!(
                "unknown identity review resolution '{other}'; expected merged, separate or dismissed"
            )),
        }
    }

    fn status(self) -> &'static str {
        match self {
            Self::Merged => "resolved_merged",
            Self::Separate => "resolved_separate",
            Self::Dismissed => "dismissed",
        }
    }
}

/// Resumen de lo que movió una fusión, para que la UI pueda contarlo sin
/// volver a consultar la biblioteca.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct IdentityMergeSummary {
    pub sources_moved: u32,
    pub library_entries_moved: u32,
    pub memberships_moved: u32,
    pub playlist_entries_moved: u32,
    pub edition_links_moved: u32,
}

/// Resultado de resolver una revisión.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityReviewResolutionResult {
    pub review_id: i64,
    pub status: String,
    pub merged_track_id: Option<i64>,
    pub merge: Option<IdentityMergeSummary>,
}

#[derive(sqlx::FromRow)]
struct ReviewRow {
    id: i64,
    service_id: i64,
    service_name: String,
    account_id: Option<i64>,
    account_name: Option<String>,
    service_track_id: String,
    reason: String,
    status: String,
    incoming_title: Option<String>,
    incoming_artist: Option<String>,
    incoming_album: Option<String>,
    incoming_duration_ms: Option<i64>,
    incoming_isrc: Option<String>,
    linked_track_id: Option<i64>,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct CandidateRow {
    review_id: i64,
    track_id: i64,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_ms: Option<i64>,
    isrc: Option<String>,
    duration_delta_ms: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct PendingReviewRow {
    service_id: i64,
    service_track_id: String,
    status: String,
}

#[derive(sqlx::FromRow)]
struct SourceRow {
    id: i64,
    service_id: i64,
    service_track_id: String,
    format: Option<String>,
    bit_depth: Option<i32>,
    sample_rate: Option<i32>,
    bitrate: Option<i32>,
    quality_score: Option<i32>,
}

/// Campos del Track importado que el merge puede heredar. Se leen completos
/// antes de liberar sus columnas únicas, para no perder los identificadores
/// que solo él tenía.
#[derive(sqlx::FromRow)]
struct SourceTrackMetadata {
    album_id: Option<i64>,
    duration_ms: Option<i64>,
    track_number: Option<i32>,
    disc_number: Option<i32>,
    isrc: Option<String>,
    musicbrainz_id: Option<String>,
    spotify_id: Option<String>,
    qobuz_id: Option<String>,
    genre: Option<String>,
    subgenre: Option<String>,
    release_year: Option<i32>,
    record_label: Option<String>,
    bpm: Option<f64>,
    musical_key: Option<String>,
    audio_quality: Option<String>,
    explicit: Option<i32>,
    is_favorite: Option<i32>,
    favorite_at: Option<String>,
    play_count: Option<i64>,
    last_played: Option<String>,
}

/// Lista las revisiones `pending`, de la más reciente a la más antigua, con
/// la evidencia de ambos lados ya agrupada.
pub async fn list_pending(db: &DbPool) -> Result<Vec<TrackIdentityReview>, String> {
    let rows: Vec<ReviewRow> = sqlx::query_as(
        r#"
        SELECT
            r.id,
            r.service_id,
            s.name AS service_name,
            r.account_id,
            a.display_name AS account_name,
            r.service_track_id,
            r.reason,
            r.status,
            r.incoming_title,
            r.incoming_artist,
            r.incoming_album,
            r.incoming_duration_ms,
            r.incoming_isrc,
            (
                SELECT ts.track_id
                FROM track_sources ts
                WHERE ts.service_id = r.service_id
                  AND ts.service_track_id = r.service_track_id
                  AND (
                      SELECT COUNT(DISTINCT other.track_id)
                      FROM track_sources other
                      WHERE other.service_id = r.service_id
                        AND other.service_track_id = r.service_track_id
                  ) = 1
                ORDER BY ts.id
                LIMIT 1
            ) AS linked_track_id,
            r.created_at,
            r.updated_at
        FROM track_identity_reviews r
        JOIN services s ON s.id = r.service_id
        LEFT JOIN accounts a ON a.id = r.account_id
        WHERE r.status = 'pending'
        ORDER BY r.created_at DESC, r.id DESC
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("identity review list failed: {e}"))?;

    let candidate_rows: Vec<CandidateRow> = sqlx::query_as(
        r#"
        SELECT
            c.review_id,
            c.track_id,
            c.title,
            c.artist,
            c.album,
            c.duration_ms,
            c.isrc,
            c.duration_delta_ms
        FROM track_identity_review_candidates c
        JOIN track_identity_reviews r ON r.id = c.review_id
        WHERE r.status = 'pending'
        ORDER BY c.review_id, c.duration_delta_ms IS NULL, c.duration_delta_ms, c.track_id
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("identity review evidence lookup failed: {e}"))?;

    let mut by_review: BTreeMap<i64, Vec<IdentityReviewCandidate>> = BTreeMap::new();
    for c in candidate_rows {
        by_review
            .entry(c.review_id)
            .or_default()
            .push(IdentityReviewCandidate {
                track_id: c.track_id,
                title: c.title,
                artist: c.artist,
                album: c.album,
                duration_ms: c.duration_ms,
                isrc: c.isrc,
                duration_delta_ms: c.duration_delta_ms,
            });
    }

    Ok(rows
        .into_iter()
        .map(|row| TrackIdentityReview {
            id: row.id,
            service_id: row.service_id,
            service_name: row.service_name,
            account_id: row.account_id,
            account_name: row.account_name,
            service_track_id: row.service_track_id,
            reason: row.reason,
            status: row.status,
            incoming_title: row.incoming_title,
            incoming_artist: row.incoming_artist,
            incoming_album: row.incoming_album,
            incoming_duration_ms: row.incoming_duration_ms,
            incoming_isrc: row.incoming_isrc,
            linked_track_id: row.linked_track_id,
            candidates: by_review.remove(&row.id).unwrap_or_default(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

/// Resuelve una revisión `pending`. `track_id` es obligatorio (y debe ser un
/// candidato congelado) solo para `merged`.
pub async fn resolve(
    db: &DbPool,
    review_id: i64,
    resolution: IdentityReviewResolution,
    track_id: Option<i64>,
) -> Result<IdentityReviewResolutionResult, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("identity review transaction failed: {e}"))?;

    let review: Option<PendingReviewRow> = sqlx::query_as(
        "SELECT service_id, service_track_id, status
         FROM track_identity_reviews WHERE id = ?",
    )
    .bind(review_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("identity review lookup failed: {e}"))?;

    let Some(review) = review else {
        return Err(format!("identity review {review_id} does not exist"));
    };
    if review.status != "pending" {
        return Err(format!(
            "identity review {review_id} is already {} and is not reopened",
            review.status
        ));
    }

    let merge = if resolution == IdentityReviewResolution::Merged {
        let target_track_id = track_id
            .ok_or_else(|| "merged resolution requires the canonical track_id".to_string())?;
        ensure_candidate(&mut tx, review_id, target_track_id).await?;
        let summary = merge_provider_item_into(
            &mut tx,
            review.service_id,
            &review.service_track_id,
            target_track_id,
        )
        .await?;
        Some(summary)
    } else {
        None
    };

    let updated = sqlx::query(
        "UPDATE track_identity_reviews
         SET status = ?,
             resolved_track_id = CASE WHEN ? = 'resolved_merged' THEN ? ELSE NULL END,
             resolved_at = CURRENT_TIMESTAMP,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ? AND status = 'pending'",
    )
    .bind(resolution.status())
    .bind(resolution.status())
    .bind(if resolution == IdentityReviewResolution::Merged {
        track_id
    } else {
        None
    })
    .bind(review_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("identity review resolution failed: {e}"))?
    .rows_affected();

    if updated != 1 {
        return Err(format!(
            "identity review {review_id} changed while it was being resolved; nothing was written"
        ));
    }

    tx.commit()
        .await
        .map_err(|e| format!("identity review commit failed: {e}"))?;

    Ok(IdentityReviewResolutionResult {
        review_id,
        status: resolution.status().to_string(),
        merged_track_id: if resolution == IdentityReviewResolution::Merged {
            track_id
        } else {
            None
        },
        merge,
    })
}

async fn ensure_candidate(
    tx: &mut Transaction<'_, Sqlite>,
    review_id: i64,
    track_id: i64,
) -> Result<(), String> {
    let is_candidate: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM track_identity_review_candidates
            WHERE review_id = ? AND track_id = ?
        )",
    )
    .bind(review_id)
    .bind(track_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| format!("identity review candidate check failed: {e}"))?;
    if !is_candidate {
        return Err(format!(
            "track {track_id} is not one of the frozen candidates of review {review_id}"
        ));
    }
    Ok(())
}

/// Pista que `track_sources` liga al ítem del proveedor. Una coincidencia
/// múltiple (legado 0097) no se resuelve al azar.
async fn linked_track_for_item(
    tx: &mut Transaction<'_, Sqlite>,
    service_id: i64,
    service_track_id: &str,
) -> Result<Option<i64>, String> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT DISTINCT track_id FROM track_sources
         WHERE service_id = ? AND service_track_id = ?
         ORDER BY track_id",
    )
    .bind(service_id)
    .bind(service_track_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("identity review linked track lookup failed: {e}"))?;
    match rows.as_slice() {
        [] => Ok(None),
        [(track_id,)] => Ok(Some(*track_id)),
        _ => Err(
            "the provider item is linked to several tracks; resolve the duplicate in the library first"
                .to_string(),
        ),
    }
}

/// Pliega la pista que hoy representa al ítem en `target_track_id`. Todo
/// ocurre dentro de la transacción de la revisión: si algo no se puede mover
/// sin perder datos, no se escribe nada.
async fn merge_provider_item_into(
    tx: &mut Transaction<'_, Sqlite>,
    service_id: i64,
    service_track_id: &str,
    target_track_id: i64,
) -> Result<IdentityMergeSummary, String> {
    let source_track_id = linked_track_for_item(tx, service_id, service_track_id)
        .await?
        .ok_or_else(|| {
            "the provider item is no longer linked to a library track; only separate or dismissed can close this review"
                .to_string()
        })?;

    if source_track_id == target_track_id {
        // El sync ya había enlazado el ítem al Track elegido: no hay filas que
        // mover, solo se cierra la revisión.
        return Ok(IdentityMergeSummary::default());
    }

    let target_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?)")
            .bind(target_track_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| format!("identity review target lookup failed: {e}"))?;
    if !target_exists {
        return Err(format!("canonical track {target_track_id} does not exist"));
    }

    let mut summary = IdentityMergeSummary::default();

    backfill_target_metadata(tx, source_track_id, target_track_id).await?;
    merge_sources(
        tx,
        source_track_id,
        target_track_id,
        service_id,
        service_track_id,
        &mut summary,
    )
    .await?;
    merge_edition_links(tx, source_track_id, target_track_id, &mut summary).await?;
    merge_library_entries(tx, source_track_id, target_track_id, &mut summary).await?;
    merge_memberships(tx, source_track_id, target_track_id, &mut summary).await?;
    merge_playlist_entries(tx, source_track_id, target_track_id, &mut summary).await?;
    merge_artists_and_credits(tx, source_track_id, target_track_id).await?;
    merge_supporting_rows(tx, source_track_id, target_track_id).await?;

    refresh_track_library_added_at(&mut **tx, target_track_id)
        .await
        .map_err(|e| format!("identity review library date refresh failed: {e}"))?;

    ensure_no_references_left(tx, source_track_id).await?;

    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review source track delete failed: {e}"))?;

    Ok(summary)
}

/// Rellena huecos del Track elegido con lo que el importado sí tenía. Nunca
/// pisa un valor existente: la identidad elegida manda.
///
/// Los identificadores únicos se leen ANTES de liberar las columnas del
/// importado: si se limpiaran primero, la copia leería NULL y el Track
/// importado se borraría con sus ISRC/Spotify/Qobuz/MusicBrainz perdidos.
/// Entre leer y escribir solo media la transacción; si el UPDATE del elegido
/// falla, el ROLLBACK devuelve las columnas al importado.
async fn backfill_target_metadata(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
) -> Result<(), String> {
    let source: SourceTrackMetadata = sqlx::query_as(
        "SELECT album_id, duration_ms, track_number, disc_number,
                isrc, musicbrainz_id, spotify_id, qobuz_id,
                genre, subgenre, release_year, record_label, bpm, musical_key,
                audio_quality, explicit, is_favorite, favorite_at, play_count, last_played
         FROM tracks WHERE id = ?",
    )
    .bind(source_track_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| format!("identity review source metadata lookup failed: {e}"))?;

    // `audio_quality` sigue la regla de la deduplicación: el tier más alto de
    // los dos Tracks y de sus fuentes, nunca el peor. Un valor del elegido que
    // no se puede interpretar no se pisa.
    let target_audio_quality: Option<String> =
        sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
            .bind(target_track_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| format!("identity review target quality lookup failed: {e}"))?;
    let target_tier = target_audio_quality
        .as_deref()
        .map(|raw| raw.parse::<AudioTier>().ok());
    let mut best_tier = [
        source.audio_quality.as_deref(),
        target_audio_quality.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter_map(|raw| raw.parse::<AudioTier>().ok())
    .max();
    let source_slots: Vec<(Option<i32>, Option<i32>, Option<i32>, Option<String>)> =
        sqlx::query_as(
            "SELECT bit_depth, sample_rate, bitrate, format FROM track_sources
             WHERE track_id IN (?, ?)",
        )
        .bind(source_track_id)
        .bind(target_track_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| format!("identity review quality slots lookup failed: {e}"))?;
    for (bit_depth, sample_rate, bitrate, format) in source_slots {
        let tier = classify_audio_tier(bit_depth, sample_rate, bitrate, format.as_deref());
        best_tier = Some(best_tier.map_or(tier, |current| current.max(tier)));
    }
    let upgrade_quality = match target_tier {
        None => best_tier,
        Some(Some(current)) => best_tier.filter(|best| *best > current),
        // Valor no canónico del elegido: se respeta tal cual.
        Some(None) => None,
    };

    // El importado se va a borrar: liberar sus columnas únicas permite que el
    // elegido las herede sin chocar contra `tracks.isrc` (UNIQUE).
    sqlx::query(
        "UPDATE tracks SET isrc = NULL, spotify_id = NULL, qobuz_id = NULL, musicbrainz_id = NULL
         WHERE id = ?",
    )
    .bind(source_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review source unique column release failed: {e}"))?;

    sqlx::query(
        r#"
        UPDATE tracks
        SET
            album_id = COALESCE(album_id, ?1),
            duration_ms = COALESCE(duration_ms, ?2),
            track_number = COALESCE(track_number, ?3),
            disc_number = COALESCE(disc_number, ?4),
            isrc = COALESCE(isrc, ?5),
            musicbrainz_id = COALESCE(musicbrainz_id, ?6),
            spotify_id = COALESCE(spotify_id, ?7),
            qobuz_id = COALESCE(qobuz_id, ?8),
            genre = COALESCE(genre, ?9),
            subgenre = COALESCE(subgenre, ?10),
            release_year = COALESCE(release_year, ?11),
            record_label = COALESCE(record_label, ?12),
            bpm = COALESCE(bpm, ?13),
            musical_key = COALESCE(musical_key, ?14),
            audio_quality = COALESCE(audio_quality, ?15),
            explicit = MAX(COALESCE(explicit, 0), COALESCE(?16, 0)),
            is_favorite = MAX(COALESCE(is_favorite, 0), COALESCE(?17, 0)),
            favorite_at = COALESCE(favorite_at, ?18),
            play_count = COALESCE(play_count, 0) + COALESCE(?19, 0),
            last_played = COALESCE(last_played, ?20)
        WHERE id = ?21
        "#,
    )
    .bind(source.album_id)
    .bind(source.duration_ms)
    .bind(source.track_number)
    .bind(source.disc_number)
    .bind(&source.isrc)
    .bind(&source.musicbrainz_id)
    .bind(&source.spotify_id)
    .bind(&source.qobuz_id)
    .bind(&source.genre)
    .bind(&source.subgenre)
    .bind(source.release_year)
    .bind(&source.record_label)
    .bind(source.bpm)
    .bind(&source.musical_key)
    .bind(&source.audio_quality)
    .bind(source.explicit)
    .bind(source.is_favorite)
    .bind(&source.favorite_at)
    .bind(source.play_count)
    .bind(&source.last_played)
    .bind(target_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review metadata backfill failed: {e}"))?;

    if let Some(tier) = upgrade_quality {
        sqlx::query("UPDATE tracks SET audio_quality = ? WHERE id = ?")
            .bind(tier.as_str())
            .bind(target_track_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("identity review quality upgrade failed: {e}"))?;
    }
    Ok(())
}

/// Mueve `track_sources`. Si el Track elegido ya ocupa la ranura del servicio,
/// el ítem extra se liga por el ledger de ediciones (la misma ruta 2 del
/// motor): la ranura legacy no se pisa y ningún ítem desaparece. El ítem que
/// abrió la revisión siempre queda ligado al Track elegido por el ledger.
async fn merge_sources(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
    provenance_service_id: i64,
    provenance_service_track_id: &str,
    summary: &mut IdentityMergeSummary,
) -> Result<(), String> {
    let sources: Vec<SourceRow> = sqlx::query_as(
        "SELECT id, service_id, service_track_id, format, bit_depth, sample_rate, bitrate, quality_score
         FROM track_sources WHERE track_id = ? ORDER BY id",
    )
    .bind(source_track_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("identity review source lookup failed: {e}"))?;

    for source in sources {
        let target_source: Option<(i64, Option<i32>)> = sqlx::query_as(
            "SELECT id, quality_score FROM track_sources
             WHERE track_id = ? AND service_id = ?",
        )
        .bind(target_track_id)
        .bind(source.service_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| format!("identity review target source lookup failed: {e}"))?;

        match target_source {
            None => {
                sqlx::query("UPDATE track_sources SET track_id = ? WHERE id = ?")
                    .bind(target_track_id)
                    .bind(source.id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| format!("identity review source move failed: {e}"))?;
            }
            Some((target_source_id, target_quality)) => {
                // La ranura legacy del Track elegido se conserva; el ítem que
                // llega se registra como edición del mismo Track.
                ensure_edition_link(
                    tx,
                    source.service_id,
                    &source.service_track_id,
                    target_track_id,
                )
                .await?;
                if source.quality_score.unwrap_or(0) > target_quality.unwrap_or(0) {
                    sqlx::query(
                        "UPDATE track_sources
                         SET format = COALESCE(?, format),
                             bit_depth = COALESCE(?, bit_depth),
                             sample_rate = COALESCE(?, sample_rate),
                             bitrate = COALESCE(?, bitrate),
                             quality_score = ?
                         WHERE id = ?",
                    )
                    .bind(source.format.as_deref())
                    .bind(source.bit_depth)
                    .bind(source.sample_rate)
                    .bind(source.bitrate)
                    .bind(source.quality_score)
                    .bind(target_source_id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| format!("identity review source quality merge failed: {e}"))?;
                }
                sqlx::query("DELETE FROM track_sources WHERE id = ?")
                    .bind(source.id)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| format!("identity review duplicate source delete failed: {e}"))?;
            }
        }
        summary.sources_moved += 1;
    }

    // El ítem del proveedor que abrió la revisión tiene que quedar ligado al
    // Track elegido aunque su ranura se haya liberado arriba por otra vía.
    // Es idempotente con el enlace que ya se haya escrito dentro del bucle.
    ensure_edition_link(
        tx,
        provenance_service_id,
        provenance_service_track_id,
        target_track_id,
    )
    .await?;
    Ok(())
}

async fn ensure_edition_link(
    tx: &mut Transaction<'_, Sqlite>,
    service_id: i64,
    service_track_id: &str,
    target_track_id: i64,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO edition_provider_items(service_id, service_track_id, evidence_status, provenance)
         VALUES (?, ?, 'observed', 'track_identity_review_merge')
         ON CONFLICT(service_id, service_track_id) DO NOTHING",
    )
    .bind(service_id)
    .bind(service_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review edition item failed: {e}"))?;

    let item_id: i64 = sqlx::query_scalar(
        "SELECT id FROM edition_provider_items
         WHERE service_id = ? AND service_track_id = ?",
    )
    .bind(service_id)
    .bind(service_track_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| format!("identity review edition item lookup failed: {e}"))?;

    sqlx::query(
        "INSERT INTO edition_item_tracks(item_id, track_id, validation_status, provenance)
         VALUES (?, ?, 'verified', 'track_identity_review_merge')
         ON CONFLICT(item_id, track_id) DO UPDATE SET
             validation_status = CASE
                 WHEN edition_item_tracks.validation_status = 'verified' THEN 'verified'
                 ELSE excluded.validation_status
             END",
    )
    .bind(item_id)
    .bind(target_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review edition link failed: {e}"))?;
    Ok(())
}

async fn merge_edition_links(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
    summary: &mut IdentityMergeSummary,
) -> Result<(), String> {
    summary.edition_links_moved += sqlx::query(
        "INSERT INTO edition_item_tracks(item_id, track_id, validation_status, provenance)
         SELECT item_id, ?, validation_status, provenance
         FROM edition_item_tracks WHERE track_id = ?
         ON CONFLICT(item_id, track_id) DO UPDATE SET
             validation_status = CASE
                 WHEN excluded.validation_status = 'verified' THEN 'verified'
                 ELSE edition_item_tracks.validation_status
             END",
    )
    .bind(target_track_id)
    .bind(source_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review edition links merge failed: {e}"))?
    .rows_affected() as u32;

    sqlx::query("DELETE FROM edition_item_tracks WHERE track_id = ?")
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review edition links cleanup failed: {e}"))?;
    Ok(())
}

/// Mueve las entradas de biblioteca/favorito. Cuando ambos lados tienen fila
/// para la misma cuenta, se conserva lo favorable de cada una: `is_liked` e
/// `is_purchased` se OR-ean y `added_at` conserva el del elegido; solo se
/// rellena con el del importado cuando falta.
async fn merge_library_entries(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
    summary: &mut IdentityMergeSummary,
) -> Result<(), String> {
    let source_entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE track_id = ?")
            .bind(source_track_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| format!("identity review library entry count failed: {e}"))?;

    sqlx::query(
        r#"
        UPDATE library_entries AS target
        SET
            is_liked = MAX(
                COALESCE(target.is_liked, 0),
                COALESCE((
                    SELECT MAX(src.is_liked)
                    FROM library_entries src
                    WHERE src.track_id = ?1 AND src.account_id = target.account_id
                ), 0)
            ),
            is_purchased = MAX(
                COALESCE(target.is_purchased, 0),
                COALESCE((
                    SELECT MAX(src.is_purchased)
                    FROM library_entries src
                    WHERE src.track_id = ?1 AND src.account_id = target.account_id
                ), 0)
            ),
            added_at = COALESCE(
                target.added_at,
                (
                    SELECT MIN(src.added_at)
                    FROM library_entries src
                    WHERE src.track_id = ?1 AND src.account_id = target.account_id
                      AND src.added_at IS NOT NULL
                )
            )
        WHERE target.track_id = ?2
          AND EXISTS (
              SELECT 1 FROM library_entries src
              WHERE src.track_id = ?1 AND src.account_id = target.account_id
          )
        "#,
    )
    .bind(source_track_id)
    .bind(target_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review library entry merge failed: {e}"))?;

    sqlx::query(
        "INSERT OR IGNORE INTO library_entries(account_id, track_id, added_at, is_liked, is_purchased)
         SELECT account_id, ?, added_at, is_liked, is_purchased
         FROM library_entries WHERE track_id = ?",
    )
    .bind(target_track_id)
    .bind(source_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review library entry move failed: {e}"))?;

    sqlx::query("DELETE FROM library_entries WHERE track_id = ?")
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review library entry cleanup failed: {e}"))?;
    summary.library_entries_moved += source_entries as u32;
    Ok(())
}

/// Mueve las pertenencias por cuenta/colección (0112) sin duplicar la clave
/// única; si ambos lados tienen la misma pertenencia, se conserva el
/// `provider_item` y la fecha más antigua.
async fn merge_memberships(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
    summary: &mut IdentityMergeSummary,
) -> Result<(), String> {
    let source_memberships: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_memberships WHERE track_id = ?")
            .bind(source_track_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| format!("identity review membership count failed: {e}"))?;

    sqlx::query(
        r#"
        UPDATE library_memberships AS target
        SET
            provider_item = COALESCE(
                target.provider_item,
                (
                    SELECT src.provider_item
                    FROM library_memberships src
                    WHERE src.track_id = ?1
                      AND src.account_id = target.account_id
                      AND src.collection_type = target.collection_type
                      AND ifnull(src.collection_id, -1) = ifnull(target.collection_id, -1)
                    LIMIT 1
                )
            ),
            added_at = CASE
                WHEN target.added_at IS NULL THEN (
                    SELECT MIN(src.added_at)
                    FROM library_memberships src
                    WHERE src.track_id = ?1
                      AND src.account_id = target.account_id
                      AND src.collection_type = target.collection_type
                      AND ifnull(src.collection_id, -1) = ifnull(target.collection_id, -1)
                      AND src.added_at IS NOT NULL
                )
                WHEN target.added_at LIKE '1970-01-01%' THEN (
                    SELECT MIN(src.added_at)
                    FROM library_memberships src
                    WHERE src.track_id = ?1
                      AND src.account_id = target.account_id
                      AND src.collection_type = target.collection_type
                      AND ifnull(src.collection_id, -1) = ifnull(target.collection_id, -1)
                      AND src.added_at IS NOT NULL
                      AND src.added_at NOT LIKE '1970-01-01%'
                )
                ELSE target.added_at
            END
        WHERE target.track_id = ?2
          AND EXISTS (
              SELECT 1 FROM library_memberships src
              WHERE src.track_id = ?1
                AND src.account_id = target.account_id
                AND src.collection_type = target.collection_type
                AND ifnull(src.collection_id, -1) = ifnull(target.collection_id, -1)
          )
        "#,
    )
    .bind(source_track_id)
    .bind(target_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review membership merge failed: {e}"))?;

    sqlx::query(
        "INSERT OR IGNORE INTO library_memberships
             (account_id, track_id, collection_type, collection_id, provider_item, added_at)
         SELECT account_id, ?, collection_type, collection_id, provider_item, added_at
         FROM library_memberships WHERE track_id = ?",
    )
    .bind(target_track_id)
    .bind(source_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review membership move failed: {e}"))?;

    sqlx::query("DELETE FROM library_memberships WHERE track_id = ?")
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review membership cleanup failed: {e}"))?;
    summary.memberships_moved += source_memberships as u32;
    Ok(())
}

/// `playlist_tracks` admite la misma pista más de una vez (0110) y su única
/// restricción es por posición: basta con repuntar, sin fusionar filas.
async fn merge_playlist_entries(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
    summary: &mut IdentityMergeSummary,
) -> Result<(), String> {
    summary.playlist_entries_moved +=
        sqlx::query("UPDATE playlist_tracks SET track_id = ? WHERE track_id = ?")
            .bind(target_track_id)
            .bind(source_track_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("identity review playlist move failed: {e}"))?
            .rows_affected() as u32;
    Ok(())
}

async fn merge_artists_and_credits(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO track_artists(track_id, artist_id, role)
         SELECT ?, artist_id, role FROM track_artists WHERE track_id = ?",
    )
    .bind(target_track_id)
    .bind(source_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review track artists merge failed: {e}"))?;
    sqlx::query("DELETE FROM track_artists WHERE track_id = ?")
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review track artists cleanup failed: {e}"))?;

    sqlx::query(
        "INSERT OR IGNORE INTO track_credits(track_id, artist_id, role)
         SELECT ?, artist_id, role FROM track_credits WHERE track_id = ?",
    )
    .bind(target_track_id)
    .bind(source_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review track credits merge failed: {e}"))?;
    sqlx::query("DELETE FROM track_credits WHERE track_id = ?")
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review track credits cleanup failed: {e}"))?;
    Ok(())
}

/// Filas dependientes que no son pertenencia pero quedarían huérfanas o
/// desaparecerían por CASCADE si se borra el Track. Se repuntan o se fusionan
/// por su clave única; las descargas se mueven solo si el Track elegido no
/// tiene ya una (dos archivos para una pista se resuelven a mano).
async fn merge_supporting_rows(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
) -> Result<(), String> {
    let source_downloads: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
            .bind(source_track_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| format!("identity review download lookup failed: {e}"))?;
    let target_downloads: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
            .bind(target_track_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| format!("identity review target download lookup failed: {e}"))?;
    if source_downloads > 0 && target_downloads == 0 {
        sqlx::query("UPDATE downloads SET track_id = ? WHERE track_id = ?")
            .bind(target_track_id)
            .bind(source_track_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("identity review download move failed: {e}"))?;
    }

    for table in [
        "download_queue",
        "player_queue_entries",
        "playback_listens",
        "operation_journal",
    ] {
        sqlx::query(&format!(
            "UPDATE {table} SET track_id = ? WHERE track_id = ?"
        ))
        .bind(target_track_id)
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review {table} move failed: {e}"))?;
    }

    merge_unique_rows(
        tx,
        source_track_id,
        target_track_id,
        "download_entitlements",
        "track_id, service_name, account_id, state, quality, source,
         last_delivered_at, checked_at, expires_at, error_class, created_at",
    )
    .await?;
    merge_unique_rows(
        tx,
        source_track_id,
        target_track_id,
        "enrichment_progress",
        "track_id, service, status, retry_count, last_error, last_attempt, completed_at, created_at",
    )
    .await?;
    merge_unique_rows(
        tx,
        source_track_id,
        target_track_id,
        "lyrics",
        "track_id, format, sync_level, source, content, language, embedded_in_file, created_at",
    )
    .await?;
    merge_unique_rows(
        tx,
        source_track_id,
        target_track_id,
        "lyrics_deletion_marks",
        "track_id, format",
    )
    .await?;
    merge_unique_rows(
        tx,
        source_track_id,
        target_track_id,
        "lyrics_restored_formats",
        "track_id, format",
    )
    .await?;
    merge_unique_rows(
        tx,
        source_track_id,
        target_track_id,
        "lyrics_resolution_failures",
        "track_id, provider_id, reason, detail, recorded_at, expires_at",
    )
    .await?;

    Ok(())
}

/// `INSERT OR IGNORE` de las filas del Track importado sobre el elegido,
/// seguido del borrado de las del importado. En conflicto gana la fila del
/// Track elegido: es la identidad que la persona confirmó.
async fn merge_unique_rows(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
    target_track_id: i64,
    table: &str,
    columns: &str,
) -> Result<(), String> {
    let select_columns = columns.replacen("track_id", "?", 1);
    sqlx::query(&format!(
        "INSERT OR IGNORE INTO {table} ({columns})
         SELECT {select_columns} FROM {table} WHERE track_id = ?"
    ))
    .bind(target_track_id)
    .bind(source_track_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("identity review {table} merge failed: {e}"))?;
    sqlx::query(&format!("DELETE FROM {table} WHERE track_id = ?"))
        .bind(source_track_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("identity review {table} cleanup failed: {e}"))?;
    Ok(())
}

/// Antes de borrar el Track importado, comprueba tabla por tabla que no queda
/// ninguna fila que apunte a él. Es la garantía de «nada se pierde en
/// silencio»: si una fila no se movió (porque el merge no sabe fusionarla), la
/// transacción entera se revierte con un mensaje accionable.
async fn ensure_no_references_left(
    tx: &mut Transaction<'_, Sqlite>,
    source_track_id: i64,
) -> Result<(), String> {
    let mut blocking = BTreeSet::new();
    for (table, column) in TRACK_REFERENCE_TABLES {
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?"))
                .bind(source_track_id)
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| format!("identity review reference check {table} failed: {e}"))?;
        if count > 0 {
            blocking.insert(format!("{table}.{column} ({count})"));
        }
    }
    if !blocking.is_empty() {
        return Err(format!(
            "cannot merge: the imported track still has data this merge will not move: {}",
            blocking.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::track_identity::REASON_FINGERPRINT_MULTIPLE_CANDIDATES;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

    async fn pool() -> SqlitePool {
        // Una sola conexión: `:memory:` es una BD por conexión y el merge debe
        // ver las mismas filas y las mismas FKs en cada paso.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn account(pool: &SqlitePool, service: &str) -> (i64, i64) {
        let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
            .bind(service)
            .fetch_one(pool)
            .await
            .unwrap();
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (service_id, display_name) VALUES (?, ?) RETURNING id",
        )
        .bind(service_id)
        .bind(format!("{service} account"))
        .fetch_one(pool)
        .await
        .unwrap();
        (service_id, account_id)
    }

    async fn track(
        pool: &SqlitePool,
        artist: &str,
        title: &str,
        duration_ms: i64,
        isrc: Option<&str>,
    ) -> i64 {
        let artist_id: i64 = sqlx::query_scalar(
            "INSERT INTO artists (name) VALUES (?) ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id = id RETURNING id",
        )
        .bind(artist)
        .fetch_one(pool)
        .await
        .unwrap();
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, duration_ms, isrc) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(title)
        .bind(duration_ms)
        .bind(isrc)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(pool)
        .await
        .unwrap();
        track_id
    }

    async fn source(
        pool: &SqlitePool,
        track_id: i64,
        service_id: i64,
        item: &str,
        quality: Option<i32>,
    ) {
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score)
             VALUES (?, ?, ?, ?)",
        )
        .bind(track_id)
        .bind(service_id)
        .bind(item)
        .bind(quality)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn review(
        pool: &SqlitePool,
        service_id: i64,
        account_id: i64,
        item: &str,
        candidates: &[i64],
    ) -> i64 {
        let review_id: i64 = sqlx::query_scalar(
            "INSERT INTO track_identity_reviews (
                service_id, service_track_id, account_id, reason,
                incoming_title, incoming_artist, incoming_album,
                incoming_duration_ms, incoming_isrc
             ) VALUES (?, ?, ?, ?, 'Same Song', 'Twice', 'Album', 200400, NULL)
             RETURNING id",
        )
        .bind(service_id)
        .bind(item)
        .bind(account_id)
        .bind(REASON_FINGERPRINT_MULTIPLE_CANDIDATES)
        .fetch_one(pool)
        .await
        .unwrap();
        for candidate in candidates {
            sqlx::query(
                "INSERT INTO track_identity_review_candidates (
                    review_id, track_id, title, artist, album, duration_ms, isrc, duration_delta_ms
                 ) VALUES (?, ?, 'Same Song', 'Twice', NULL, 200000, NULL, 400)",
            )
            .bind(review_id)
            .bind(candidate)
            .execute(pool)
            .await
            .unwrap();
        }
        review_id
    }

    async fn favorite(pool: &SqlitePool, account_id: i64, track_id: i64, liked: bool) {
        sqlx::query(
            "INSERT INTO library_entries (account_id, track_id, is_liked, added_at)
             VALUES (?, ?, ?, '2026-01-01')",
        )
        .bind(account_id)
        .bind(track_id)
        .bind(liked as i32)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO library_memberships
                (account_id, track_id, collection_type, collection_id, provider_item, added_at)
             VALUES (?, ?, 'favorite', NULL, ?, '2026-01-01')",
        )
        .bind(account_id)
        .bind(track_id)
        .bind(format!("item-{track_id}"))
        .execute(pool)
        .await
        .unwrap();
    }

    async fn playlist(pool: &SqlitePool, account_id: i64) -> i64 {
        sqlx::query_scalar(
            "INSERT INTO playlists (account_id, name, local_uuid) VALUES (?, 'Mix', ?) RETURNING id",
        )
        .bind(account_id)
        .bind(format!("uuid-{account_id}"))
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[test]
    fn la_resolucion_desconocida_no_se_acepta() {
        assert_eq!(
            IdentityReviewResolution::parse("separate").unwrap(),
            IdentityReviewResolution::Separate
        );
        assert!(IdentityReviewResolution::parse("merge").is_err());
        assert!(IdentityReviewResolution::parse("resolved_merged").is_err());
    }

    #[tokio::test]
    async fn la_lista_de_referencias_cubre_todas_las_fk_a_tracks() {
        let pool = pool().await;
        let rows: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT m.name, fk."from"
            FROM sqlite_master m
            JOIN pragma_foreign_key_list(m.name) fk
            WHERE m.type = 'table' AND fk."table" = 'tracks'
            ORDER BY m.name
            "#,
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let schema: BTreeSet<(String, String)> = rows.into_iter().collect();
        let contract: BTreeSet<(String, String)> = TRACK_REFERENCE_TABLES
            .iter()
            .map(|(table, column)| (table.to_string(), column.to_string()))
            .collect();
        assert_eq!(
            contract, schema,
            "una FK nueva a tracks obliga a decidir qué hace el merge con sus filas"
        );
    }

    #[tokio::test]
    async fn listar_devuelve_la_evidencia_de_los_dos_lados() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let a = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let b = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let review_id = review(&pool, service_id, account_id, "item-1", &[a, b]).await;

        let pending = list_pending(&pool).await.unwrap();
        assert_eq!(pending.len(), 1);
        let item = &pending[0];
        assert_eq!(item.id, review_id);
        assert_eq!(item.service_name, "qobuz");
        assert_eq!(item.account_id, Some(account_id));
        assert_eq!(item.account_name.as_deref(), Some("qobuz account"));
        assert_eq!(item.incoming_title.as_deref(), Some("Same Song"));
        assert_eq!(item.incoming_artist.as_deref(), Some("Twice"));
        assert_eq!(item.incoming_duration_ms, Some(200_400));
        assert_eq!(
            item.candidates
                .iter()
                .map(|c| c.track_id)
                .collect::<Vec<_>>(),
            vec![a, b]
        );
        assert_eq!(item.candidates[0].duration_delta_ms, Some(400));
    }

    #[tokio::test]
    async fn separar_y_descartar_cierran_sin_tocar_pistas() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let a = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let b = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let source_track = track(&pool, "Twice", "Different", 210_000, None).await;
        source(&pool, source_track, service_id, "item-1", None).await;
        let review_id = review(&pool, service_id, account_id, "item-1", &[a, b]).await;

        let result = resolve(&pool, review_id, IdentityReviewResolution::Separate, None)
            .await
            .unwrap();
        assert_eq!(result.status, "resolved_separate");
        assert_eq!(result.merged_track_id, None);

        let status: (String, bool) = sqlx::query_as(
            "SELECT status, resolved_at IS NOT NULL FROM track_identity_reviews WHERE id = ?",
        )
        .bind(review_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, ("resolved_separate".to_string(), true));

        // Ninguna pista ni fuente se toca: la revisión solo cambia de estado.
        let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(tracks, 3);
        let source_link: (i64, String) = sqlx::query_as(
            "SELECT track_id, service_track_id FROM track_sources WHERE service_track_id = 'item-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source_link, (source_track, "item-1".to_string()));

        let dismissed = resolve(
            &pool,
            review(&pool, service_id, account_id, "item-2", &[a, b]).await,
            IdentityReviewResolution::Dismissed,
            None,
        )
        .await
        .unwrap();
        assert_eq!(dismissed.status, "dismissed");
        assert_eq!(dismissed.merged_track_id, None);
        let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(tracks, 3);
    }

    #[tokio::test]
    async fn una_revision_cerrada_no_se_reabre_ni_se_resuelve_dos_veces() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let a = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let b = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let source_track = track(&pool, "Twice", "Different", 210_000, None).await;
        source(&pool, source_track, service_id, "item-1", None).await;
        let review_id = review(&pool, service_id, account_id, "item-1", &[a, b]).await;

        resolve(&pool, review_id, IdentityReviewResolution::Dismissed, None)
            .await
            .unwrap();

        let second = resolve(&pool, review_id, IdentityReviewResolution::Separate, None).await;
        assert!(
            second.is_err(),
            "una revisión cerrada no se vuelve a resolver"
        );
        assert!(second.unwrap_err().contains("already dismissed"));
        let status: String =
            sqlx::query_scalar("SELECT status FROM track_identity_reviews WHERE id = ?")
                .bind(review_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "dismissed");
        assert!(
            list_pending(&pool).await.unwrap().is_empty(),
            "la cola solo devuelve pending"
        );
    }

    #[tokio::test]
    async fn cerrar_permite_abrir_otra_revision_pending_para_el_mismo_item() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let a = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let b = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let source_track = track(&pool, "Twice", "Different", 210_000, None).await;
        source(&pool, source_track, service_id, "item-1", None).await;
        let first = review(&pool, service_id, account_id, "item-1", &[a, b]).await;
        resolve(&pool, first, IdentityReviewResolution::Dismissed, None)
            .await
            .unwrap();

        let second = review(&pool, service_id, account_id, "item-1", &[a, b]).await;
        assert_ne!(first, second);
        let open: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM track_identity_reviews WHERE status = 'pending'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(open, 1);
    }

    #[tokio::test]
    async fn fusionar_conserva_fuentes_pertenencias_favoritos_y_playlists() {
        let pool = pool().await;
        let (qobuz_id, account_id) = account(&pool, "qobuz").await;
        let target = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let other = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let imported = track(&pool, "Twice", "Same Song", 200_400, None).await;
        source(&pool, target, qobuz_id, "target-qobuz-item", Some(3)).await;
        source(&pool, imported, qobuz_id, "review-qobuz-item", Some(4)).await;
        let playlist_id = playlist(&pool, account_id).await;
        for (track_id, position) in [(target, 1), (imported, 2)] {
            sqlx::query(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
            )
            .bind(playlist_id)
            .bind(track_id)
            .bind(position)
            .execute(&pool)
            .await
            .unwrap();
        }
        favorite(&pool, account_id, target, false).await;
        favorite(&pool, account_id, imported, true).await;
        let review_id = review(
            &pool,
            qobuz_id,
            account_id,
            "review-qobuz-item",
            &[target, other],
        )
        .await;

        // Caracterización del estado previo: cada pista tiene su propia fuente,
        // su entrada de biblioteca y su pertenencia; la playlist tiene las dos.
        let before_sources: Vec<(i64, String)> = sqlx::query_as(
            "SELECT track_id, service_track_id FROM track_sources ORDER BY track_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            before_sources,
            vec![
                (target, "target-qobuz-item".to_string()),
                (imported, "review-qobuz-item".to_string()),
            ]
        );
        let before_entries: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE track_id IN (?, ?)")
                .bind(target)
                .bind(imported)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before_entries, 2);
        let before_memberships: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM library_memberships WHERE track_id IN (?, ?)")
                .bind(target)
                .bind(imported)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before_memberships, 2);
        let before_playlist: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
                .bind(playlist_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(before_playlist, 2);

        let result = resolve(
            &pool,
            review_id,
            IdentityReviewResolution::Merged,
            Some(target),
        )
        .await
        .unwrap();

        assert_eq!(result.status, "resolved_merged");
        assert_eq!(result.merged_track_id, Some(target));
        let summary = result.merge.unwrap();
        assert_eq!(summary.sources_moved, 1);
        assert_eq!(summary.playlist_entries_moved, 1);
        assert_eq!(summary.library_entries_moved, 1);

        let status: (String, Option<i64>, bool) = sqlx::query_as(
            "SELECT status, resolved_track_id, resolved_at IS NOT NULL
             FROM track_identity_reviews WHERE id = ?",
        )
        .bind(review_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, ("resolved_merged".to_string(), Some(target), true));

        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?)")
            .bind(imported)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(!exists, "la pista importada se pliega en la elegida");

        // La ranura legacy del Track elegido se conserva; el ítem de la
        // revisión queda ligado al mismo Track por el ledger de ediciones.
        let target_sources: Vec<String> =
            sqlx::query_scalar("SELECT service_track_id FROM track_sources WHERE track_id = ?")
                .bind(target)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(target_sources, vec!["target-qobuz-item".to_string()]);
        let edition_links: Vec<(String, String)> = sqlx::query_as(
            "SELECT i.service_track_id, link.validation_status
             FROM edition_item_tracks link
             JOIN edition_provider_items i ON i.id = link.item_id
             WHERE link.track_id = ? ORDER BY i.service_track_id",
        )
        .bind(target)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(edition_links.contains(&("review-qobuz-item".to_string(), "verified".to_string())));

        // Favorito conservado: el OR de is_liked y la fecha.
        let (liked, added): (i32, String) = sqlx::query_as(
            "SELECT is_liked, added_at FROM library_entries WHERE account_id = ? AND track_id = ?",
        )
        .bind(account_id)
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(liked, 1, "el favorito del Track importado no se pierde");
        assert_eq!(added, "2026-01-01");
        let memberships: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT collection_type, provider_item FROM library_memberships
             WHERE account_id = ? AND track_id = ? ORDER BY collection_type",
        )
        .bind(account_id)
        .bind(target)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(memberships.len(), 1);
        assert_eq!(memberships[0].0, "favorite");
        assert!(
            memberships[0].1.is_some(),
            "la pertenencia conserva su ítem"
        );

        // Playlist: las dos posiciones siguen ahí, ahora sobre el Track elegido.
        let playlist_tracks: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position",
        )
        .bind(playlist_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(playlist_tracks, vec![(target, 1), (target, 2)]);

        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM track_identity_review_candidates WHERE review_id = ?",
        )
        .bind(review_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(remaining, 2, "la evidencia congelada no se reescribe");
    }

    #[tokio::test]
    async fn fusionar_hereda_identificadores_y_metadatos_del_importado() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let target = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let other = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let imported = track(&pool, "Twice", "Same Song", 200_400, None).await;
        sqlx::query(
            "UPDATE tracks SET
                isrc = 'USAAA0000001',
                spotify_id = 'spotify-source',
                qobuz_id = 'qobuz-source',
                musicbrainz_id = 'mbid-source',
                genre = 'source-genre',
                subgenre = 'source-subgenre',
                release_year = 1999,
                record_label = 'source-label',
                bpm = 123.5,
                musical_key = '8A',
                audio_quality = 'hires',
                explicit = 1
             WHERE id = ?",
        )
        .bind(imported)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE tracks SET genre = 'target-genre', audio_quality = 'lossy', explicit = 0
             WHERE id = ?",
        )
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
        source(&pool, imported, service_id, "item-1", None).await;
        let review_id = review(&pool, service_id, account_id, "item-1", &[target, other]).await;

        resolve(
            &pool,
            review_id,
            IdentityReviewResolution::Merged,
            Some(target),
        )
        .await
        .unwrap();

        let inherited: (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i32>,
            Option<String>,
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<i32>,
        ) = sqlx::query_as(
            "SELECT isrc, spotify_id, qobuz_id, musicbrainz_id,
                    genre, subgenre, release_year, record_label, bpm, musical_key,
                    audio_quality, explicit
             FROM tracks WHERE id = ?",
        )
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(inherited.0.as_deref(), Some("USAAA0000001"));
        assert_eq!(inherited.1.as_deref(), Some("spotify-source"));
        assert_eq!(inherited.2.as_deref(), Some("qobuz-source"));
        assert_eq!(inherited.3.as_deref(), Some("mbid-source"));
        assert_eq!(
            inherited.4.as_deref(),
            Some("target-genre"),
            "un campo que el elegido ya tenía no retrocede"
        );
        assert_eq!(inherited.5.as_deref(), Some("source-subgenre"));
        assert_eq!(inherited.6, Some(1999));
        assert_eq!(inherited.7.as_deref(), Some("source-label"));
        assert_eq!(inherited.8, Some(123.5));
        assert_eq!(inherited.9.as_deref(), Some("8A"));
        assert_eq!(inherited.10.as_deref(), Some("hires"));
        assert_eq!(inherited.11, Some(1), "explicit tampoco retrocede");

        let target_duration: Option<i64> =
            sqlx::query_scalar("SELECT duration_ms FROM tracks WHERE id = ?")
                .bind(target)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            target_duration,
            Some(200_000),
            "la duración existente del elegido se conserva"
        );
    }

    #[tokio::test]
    async fn fusionar_no_pisa_los_identificadores_del_elegido() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let target = track(&pool, "Twice", "Same Song", 200_000, Some("USAAA0000002")).await;
        let other = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let imported = track(&pool, "Twice", "Same Song", 200_400, Some("USAAA0000001")).await;
        sqlx::query(
            "UPDATE tracks SET
                spotify_id = 'target-spotify',
                qobuz_id = 'target-qobuz',
                musicbrainz_id = 'target-mbid'
             WHERE id = ?",
        )
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE tracks SET
                spotify_id = 'source-spotify',
                qobuz_id = 'source-qobuz',
                musicbrainz_id = 'source-mbid'
             WHERE id = ?",
        )
        .bind(imported)
        .execute(&pool)
        .await
        .unwrap();
        source(&pool, imported, service_id, "item-1", None).await;
        let review_id = review(&pool, service_id, account_id, "item-1", &[target, other]).await;

        resolve(
            &pool,
            review_id,
            IdentityReviewResolution::Merged,
            Some(target),
        )
        .await
        .unwrap();

        let kept: (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT isrc, spotify_id, qobuz_id, musicbrainz_id FROM tracks WHERE id = ?",
        )
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(kept.0.as_deref(), Some("USAAA0000002"));
        assert_eq!(kept.1.as_deref(), Some("target-spotify"));
        assert_eq!(kept.2.as_deref(), Some("target-qobuz"));
        assert_eq!(kept.3.as_deref(), Some("target-mbid"));
    }

    #[tokio::test]
    async fn fusionar_mueve_los_enlaces_de_edicion_preexistentes() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let target = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let other = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let imported = track(&pool, "Twice", "Same Song", 200_400, None).await;
        source(&pool, imported, service_id, "item-1", None).await;
        let source_item: i64 = sqlx::query_scalar(
            "INSERT INTO edition_provider_items(service_id, service_track_id, evidence_status, provenance)
             VALUES (?, 'edition-source', 'observed', 'sync') RETURNING id",
        )
        .bind(service_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let target_item: i64 = sqlx::query_scalar(
            "INSERT INTO edition_provider_items(service_id, service_track_id, evidence_status, provenance)
             VALUES (?, 'edition-target', 'observed', 'sync') RETURNING id",
        )
        .bind(service_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO edition_item_tracks(item_id, track_id, validation_status, provenance)
             VALUES (?, ?, 'observed', 'sync'), (?, ?, 'verified', 'sync')",
        )
        .bind(source_item)
        .bind(imported)
        .bind(target_item)
        .bind(target)
        .execute(&pool)
        .await
        .unwrap();
        let review_id = review(&pool, service_id, account_id, "item-1", &[target, other]).await;

        let result = resolve(
            &pool,
            review_id,
            IdentityReviewResolution::Merged,
            Some(target),
        )
        .await
        .unwrap();

        assert_eq!(result.merge.unwrap().edition_links_moved, 1);
        let moved: Vec<(i64, String)> = sqlx::query_as(
            "SELECT item_id, validation_status FROM edition_item_tracks
             WHERE track_id = ? AND item_id IN (?, ?) ORDER BY item_id",
        )
        .bind(target)
        .bind(source_item)
        .bind(target_item)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            moved,
            vec![
                (source_item, "observed".to_string()),
                (target_item, "verified".to_string()),
            ],
            "los dos enlaces preexistentes acaban en el elegido con su estado"
        );
        let source_links: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM edition_item_tracks WHERE track_id = ?")
                .bind(imported)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            source_links, 0,
            "el importado se borra sin enlaces colgando"
        );
    }

    #[tokio::test]
    async fn fusionar_exige_un_candidato_congelado() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let a = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let b = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let outsider = track(&pool, "Other", "Other Song", 100_000, None).await;
        let imported = track(&pool, "Twice", "Same Song", 200_400, None).await;
        source(&pool, imported, service_id, "item-1", None).await;
        let review_id = review(&pool, service_id, account_id, "item-1", &[a, b]).await;

        let error = resolve(
            &pool,
            review_id,
            IdentityReviewResolution::Merged,
            Some(outsider),
        )
        .await
        .unwrap_err();
        assert!(error.contains("not one of the frozen candidates"));
        let status: String =
            sqlx::query_scalar("SELECT status FROM track_identity_reviews WHERE id = ?")
                .bind(review_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "pending", "un error no deja la revisión a medias");
    }

    #[tokio::test]
    async fn fusionar_rechaza_un_item_que_ya_no_esta_en_la_biblioteca() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let a = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let b = track(&pool, "Twice", "Same Song", 200_900, None).await;
        let review_id = review(&pool, service_id, account_id, "item-missing", &[a, b]).await;

        let error = resolve(&pool, review_id, IdentityReviewResolution::Merged, Some(a))
            .await
            .unwrap_err();
        assert!(error.contains("no longer linked"));
        let status: String =
            sqlx::query_scalar("SELECT status FROM track_identity_reviews WHERE id = ?")
                .bind(review_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "pending");
    }

    #[tokio::test]
    async fn fusionar_aborta_si_el_track_importado_tiene_descarga_y_el_elegido_tambien() {
        let pool = pool().await;
        let (service_id, account_id) = account(&pool, "qobuz").await;
        let target = track(&pool, "Twice", "Same Song", 200_000, None).await;
        let other = track(&pool, "Twice", "Same Song", 200_900, None).await;
        // Artista distinto para poder demostrar que el movimiento de
        // `track_artists` también se revierte.
        let imported = track(&pool, "Rollback Artist", "Same Song", 200_400, None).await;
        source(&pool, imported, service_id, "item-1", None).await;
        for (track_id, path) in [
            (target, "/tmp/target.flac"),
            (imported, "/tmp/imported.flac"),
        ] {
            sqlx::query(
                "INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'FLAC')",
            )
            .bind(track_id)
            .bind(path)
            .execute(&pool)
            .await
            .unwrap();
        }
        let review_id = review(&pool, service_id, account_id, "item-1", &[target, other]).await;

        let error = resolve(
            &pool,
            review_id,
            IdentityReviewResolution::Merged,
            Some(target),
        )
        .await
        .unwrap_err();
        assert!(
            error.contains("downloads.track_id (1)"),
            "el error debe decir qué bloquea el merge: {error}"
        );
        let status: String =
            sqlx::query_scalar("SELECT status FROM track_identity_reviews WHERE id = ?")
                .bind(review_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "pending", "la transacción se revierte entera");
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tracks WHERE id = ?)")
            .bind(imported)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(exists, "la pista importada sigue ahí");

        // Los movimientos anteriores al fallo también se revierten: la fuente
        // del ítem sigue en el importado y el artista importado no se coló en
        // el elegido.
        let source_owner: i64 = sqlx::query_scalar(
            "SELECT track_id FROM track_sources WHERE service_track_id = 'item-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source_owner, imported);
        let target_has_item: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM track_sources WHERE track_id = ? AND service_track_id = 'item-1')",
        )
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!target_has_item);
        let target_artists: Vec<String> = sqlx::query_scalar(
            "SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
             WHERE ta.track_id = ? ORDER BY a.name",
        )
        .bind(target)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(target_artists, vec!["Twice".to_string()]);
        let imported_artists: Vec<String> = sqlx::query_scalar(
            "SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id
             WHERE ta.track_id = ? ORDER BY a.name",
        )
        .bind(imported)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(imported_artists, vec!["Rollback Artist".to_string()]);
        let edition_links_to_target: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM edition_item_tracks link
             JOIN edition_provider_items item ON item.id = link.item_id
             WHERE link.track_id = ? AND item.service_track_id = 'item-1'",
        )
        .bind(target)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            edition_links_to_target, 0,
            "el enlace de edición creado antes del fallo también se revierte"
        );
    }
}
