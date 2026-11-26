//! Canonicalización de playlists por contenido (issue #74, ADR-0006 decisión 34).
//!
//! Las playlists de origen conservan su identidad remota y su contenido. La
//! canónica es una agrupación persistente de esas copias, no un borrado: así una
//! fuente que diverge después puede separarse sin perder su propio snapshot.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{SqliteConnection, SqlitePool};

/// Resultado de una pasada completa o incremental. Los contadores se leen de
/// las tablas canónicas al terminar, así que dos pasadas sobre los mismos datos
/// producen el mismo resumen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalizationSummary {
    pub playlists_examined: usize,
    pub canonicals: usize,
    pub playlists_linked: usize,
    pub merged_canonicals: usize,
    pub incomplete_playlists: usize,
    pub empty_playlists: usize,
}

/// Una fuente remota tal como se muestra en la ficha de una canónica.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistCanonicalSource {
    pub source_id: i64,
    pub playlist_id: i64,
    pub account_id: i64,
    pub account_name: Option<String>,
    pub account_email: Option<String>,
    pub service_id: i64,
    pub service_name: String,
    pub service_playlist_id: String,
    pub synced_at: Option<String>,
    pub snapshot_id: Option<String>,
    pub is_primary: bool,
}

/// Pertenencia de una playlist de origen a su canónica, con todas las fuentes
/// del grupo. `is_primary` describe a la playlist pedida, no a la fuente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistCanonicalMembership {
    pub canonical_id: i64,
    pub is_primary: bool,
    pub sources: Vec<PlaylistCanonicalSource>,
}

#[derive(Debug, Clone)]
struct Candidate {
    playlist_id: i64,
    content_hash: String,
    track_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateSkip {
    Empty,
    Incomplete,
}

#[derive(Debug)]
enum CandidateLoadError {
    Skip(CandidateSkip),
    Database(String),
}

/// Marca la expansión de una fuente como completa solo si el total declarado
/// por el proveedor coincide con las ocurrencias locales. Es la costura que
/// debe llamarse DESPUÉS de recorrer todas las páginas; un fallo de expansión
/// usa [`mark_playlist_source_expansion_incomplete`].
pub async fn record_playlist_source_expansion(
    db: &SqlitePool,
    playlist_id: i64,
) -> Result<bool, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start playlist expansion update: {e}"))?;
    let complete = record_playlist_source_expansion_in_conn(&mut tx, playlist_id).await?;
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist expansion update: {e}"))?;
    Ok(complete)
}

/// Variante transaccional de [`record_playlist_source_expansion`].
pub async fn record_playlist_source_expansion_in_conn(
    conn: &mut SqliteConnection,
    playlist_id: i64,
) -> Result<bool, String> {
    let sources: Vec<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, reported_track_count FROM playlist_sources
         WHERE playlist_id = ? ORDER BY id",
    )
    .bind(playlist_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| format!("Failed to read playlist expansion sources: {e}"))?;
    let occurrences: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
            .bind(playlist_id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| format!("Failed to count playlist expansion tracks: {e}"))?;
    let complete = occurrences > 0
        && !sources.is_empty()
        && sources
            .iter()
            .all(|(_, reported)| *reported == Some(occurrences));

    sqlx::query("UPDATE playlist_sources SET expansion_complete = ? WHERE playlist_id = ?")
        .bind(complete)
        .bind(playlist_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to record playlist expansion state: {e}"))?;

    Ok(complete)
}

/// Invalidación explícita para rutas que saben que una página o una playlist
/// falló. Retira la membresía canónica en la misma transacción.
pub async fn mark_playlist_source_expansion_incomplete(
    db: &SqlitePool,
    playlist_id: i64,
) -> Result<(), String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start playlist expansion invalidation: {e}"))?;
    mark_playlist_source_expansion_incomplete_in_conn(&mut tx, playlist_id).await?;
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist expansion invalidation: {e}"))?;
    Ok(())
}

/// Variante transaccional de [`mark_playlist_source_expansion_incomplete`].
pub async fn mark_playlist_source_expansion_incomplete_in_conn(
    conn: &mut SqliteConnection,
    playlist_id: i64,
) -> Result<(), String> {
    sqlx::query("UPDATE playlist_sources SET expansion_complete = 0 WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to invalidate playlist expansion: {e}"))?;
    canonicalize_playlist_in_conn(conn, playlist_id).await?;
    Ok(())
}

/// Recalcula todas las canónicas en una transacción propia.
pub async fn canonicalize_playlists(db: &SqlitePool) -> Result<CanonicalizationSummary, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start playlist canonicalization: {e}"))?;
    let summary = canonicalize_playlists_in_conn(&mut tx).await?;
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist canonicalization: {e}"))?;
    Ok(summary)
}

/// Cierra la expansión de una playlist al terminar su importación (#398).
///
/// El llamador recorre todas las páginas del proveedor, registra cada pista en
/// `playlist_tracks` y recompacta posiciones; aquí se certifica el total
/// declarado y se recalcula el grupo canónico al que pertenece. Un fallo de
/// expansión (`expansion_failed`) marca la fuente como incompleta y deshace su
/// membresía: una copia a medias nunca se fusiona con una completa. Devuelve
/// `true` solo cuando la fuente quedó certificada.
///
/// Un fallo de base de datos no propaga el error: la playlist queda sin
/// membresía (el estado seguro) y el sync continúa con las demás.
pub async fn finish_playlist_expansion(
    db: &SqlitePool,
    playlist_id: i64,
    expansion_failed: bool,
) -> bool {
    let result: Result<bool, String> = async {
        if expansion_failed {
            mark_playlist_source_expansion_incomplete(db, playlist_id).await?;
            return Ok(false);
        }
        let complete = record_playlist_source_expansion(db, playlist_id).await?;
        canonicalize_playlist(db, playlist_id).await?;
        Ok(complete)
    }
    .await;

    match result {
        Ok(complete) => complete,
        Err(error) => {
            tracing::warn!(
                playlist_id,
                %error,
                "playlist expansion could not be closed; left unmerged"
            );
            false
        }
    }
}

/// Recalcula una playlist y las copias de su nombre. Es la costura que usan los
/// importadores después de recompactar: no escanea todo el catálogo por cada
/// playlist, pero sí repara el grupo completo al que pertenece.
pub async fn canonicalize_playlist(
    db: &SqlitePool,
    playlist_id: i64,
) -> Result<CanonicalizationSummary, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start playlist canonicalization: {e}"))?;
    let summary = canonicalize_playlist_in_conn(&mut tx, playlist_id).await?;
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist canonicalization: {e}"))?;
    Ok(summary)
}

/// Variante transaccional para que sanitización, renombrado y borrado confirmen
/// la agrupación en la misma transacción que su cambio de contenido.
pub async fn canonicalize_playlist_in_conn(
    conn: &mut SqliteConnection,
    playlist_id: i64,
) -> Result<CanonicalizationSummary, String> {
    let old_normalized_name: Option<String> = sqlx::query_scalar(
        "SELECT c.normalized_name
         FROM playlist_canonical_members m
         JOIN playlist_canonicals c ON c.id = m.canonical_id
         WHERE m.playlist_id = ?",
    )
    .bind(playlist_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| format!("Failed to read previous playlist canonical: {e}"))?;

    let playlist: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM playlists
         WHERE id = ? AND deleted_at IS NULL AND COALESCE(is_smart, 0) = 0 AND local_uuid IS NULL",
    )
    .bind(playlist_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| format!("Failed to read playlist {playlist_id}: {e}"))?;

    let normalized_name = playlist
        .as_ref()
        .map(|(name,)| normalize_playlist_name(name))
        .or(old_normalized_name);

    // Si la playlist cambió de nombre, de contenido o de elegibilidad, su
    // membresía vieja no puede seguir apuntando a la huella anterior.
    sqlx::query("DELETE FROM playlist_canonical_members WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to clear playlist canonical member: {e}"))?;

    let mut incomplete = 0usize;
    let mut empty = 0usize;
    if let Some(normalized_name) = normalized_name.filter(|name| !name.is_empty()) {
        let playlist_ids: Vec<(i64,)> = sqlx::query_as(
            "SELECT p.id
             FROM playlists p
             WHERE p.deleted_at IS NULL
               AND COALESCE(p.is_smart, 0) = 0
               AND p.local_uuid IS NULL
               AND EXISTS (SELECT 1 FROM playlist_sources ps WHERE ps.playlist_id = p.id)
             ORDER BY p.id",
        )
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("Failed to list remote playlists: {e}"))?;

        let mut groups: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
        for (candidate_id,) in playlist_ids {
            let candidate_name: Option<(String,)> =
                sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
                    .bind(candidate_id)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|e| format!("Failed to read playlist name: {e}"))?;
            let Some((candidate_name,)) = candidate_name else {
                continue;
            };
            if normalize_playlist_name(&candidate_name) != normalized_name {
                continue;
            }

            match load_candidate(&mut *conn, candidate_id).await {
                Ok(candidate) => groups
                    .entry(candidate.content_hash.clone())
                    .or_default()
                    .push(candidate),
                Err(CandidateLoadError::Skip(CandidateSkip::Empty)) => {
                    remove_member(&mut *conn, candidate_id).await?;
                    if candidate_id == playlist_id {
                        empty += 1;
                    }
                }
                Err(CandidateLoadError::Skip(CandidateSkip::Incomplete)) => {
                    remove_member(&mut *conn, candidate_id).await?;
                    if candidate_id == playlist_id {
                        incomplete += 1;
                    }
                }
                Err(CandidateLoadError::Database(error)) => return Err(error),
            }
        }

        for candidates in groups.into_values() {
            persist_group(&mut *conn, &normalized_name, candidates).await?;
        }
    }

    prune_empty_canonicals(&mut *conn).await?;
    read_summary(&mut *conn, 1, incomplete, empty).await
}

/// Recalcula el catálogo dentro de una transacción ya abierta.
pub async fn canonicalize_playlists_in_conn(
    conn: &mut SqliteConnection,
) -> Result<CanonicalizationSummary, String> {
    let playlist_ids: Vec<(i64,)> = sqlx::query_as(
        "SELECT p.id
         FROM playlists p
         WHERE p.deleted_at IS NULL
           AND COALESCE(p.is_smart, 0) = 0
           AND p.local_uuid IS NULL
           AND EXISTS (SELECT 1 FROM playlist_sources ps WHERE ps.playlist_id = p.id)
         ORDER BY p.id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| format!("Failed to list remote playlists: {e}"))?;

    let mut incomplete = 0usize;
    let mut empty = 0usize;
    for (playlist_id,) in &playlist_ids {
        let summary = canonicalize_playlist_in_conn(conn, *playlist_id).await?;
        incomplete += summary.incomplete_playlists;
        empty += summary.empty_playlists;
    }
    prune_empty_canonicals(conn).await?;
    read_summary(conn, playlist_ids.len(), incomplete, empty).await
}

/// Lee la canónica de una playlist y todas sus fuentes. Una playlist local,
/// smart, vacía o incompleta no tiene membresía y devuelve `None`.
pub async fn get_playlist_canonical_membership(
    db: &SqlitePool,
    playlist_id: i64,
) -> Result<Option<PlaylistCanonicalMembership>, String> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("Failed to start playlist canonical read: {e}"))?;
    let membership = get_playlist_canonical_membership_in_conn(&mut tx, playlist_id).await?;
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist canonical read: {e}"))?;
    Ok(membership)
}

/// Membership read inside the caller's transaction, so playlist fields and
/// canonical sources come from the same SQLite snapshot.
pub async fn get_playlist_canonical_membership_in_conn(
    conn: &mut SqliteConnection,
    playlist_id: i64,
) -> Result<Option<PlaylistCanonicalMembership>, String> {
    // Una sola sentencia: la pertenencia, la primaria derivada y las fuentes
    // salen del mismo snapshot de SQLite, incluso si otra conexión recanonicalza.
    let rows: Vec<(
        i64,
        i64,
        i64,
        i64,
        i64,
        Option<String>,
        Option<String>,
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
        i64,
    )> = sqlx::query_as(
        r#"
        WITH requested AS (
            SELECT canonical_id
            FROM playlist_canonical_members
            WHERE playlist_id = ?
        ),
        canonical AS (
            SELECT c.id,
                   (
                       SELECT primary_member.playlist_id
                       FROM playlist_canonical_members primary_member
                       JOIN playlists primary_playlist
                           ON primary_playlist.id = primary_member.playlist_id
                       WHERE primary_member.canonical_id = c.id
                       ORDER BY COALESCE(primary_playlist.created_at, '9999-12-31 23:59:59'),
                                primary_playlist.id
                       LIMIT 1
                   ) AS primary_playlist_id
            FROM playlist_canonicals c
            JOIN requested r ON r.canonical_id = c.id
        )
        SELECT c.id,
               c.primary_playlist_id,
               ps.id,
               ps.playlist_id,
               ps.account_id,
               COALESCE(NULLIF(TRIM(a.display_name), ''), a.email),
               a.email,
               ps.service_id,
               s.name,
               ps.service_playlist_id,
               ps.synced_at,
               ps.snapshot_id,
               CASE WHEN ps.playlist_id = c.primary_playlist_id
                     AND ps.id = (
                         SELECT MIN(primary_source.id)
                         FROM playlist_sources primary_source
                         WHERE primary_source.playlist_id = c.primary_playlist_id
                     )
                    THEN 1 ELSE 0 END
        FROM canonical c
        JOIN playlist_canonical_members m ON m.canonical_id = c.id
        JOIN playlist_sources ps ON ps.playlist_id = m.playlist_id
        JOIN playlists p ON p.id = ps.playlist_id
        JOIN accounts a ON a.id = ps.account_id
        JOIN services s ON s.id = ps.service_id
        ORDER BY CASE WHEN ps.playlist_id = c.primary_playlist_id
                           AND ps.id = (
                               SELECT MIN(primary_source.id)
                               FROM playlist_sources primary_source
                               WHERE primary_source.playlist_id = c.primary_playlist_id
                           )
                      THEN 0 ELSE 1 END,
                 p.created_at ASC,
                 ps.id ASC
        "#,
    )
    .bind(playlist_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| format!("Failed to list playlist canonical sources: {e}"))?;

    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let canonical_id = first.0;
    let primary_playlist_id = first.1;

    let sources = rows
        .into_iter()
        .map(
            |(
                _canonical_id,
                _primary_playlist_id,
                source_id,
                source_playlist_id,
                account_id,
                account_name,
                account_email,
                service_id,
                service_name,
                service_playlist_id,
                synced_at,
                snapshot_id,
                is_primary,
            )| PlaylistCanonicalSource {
                source_id,
                playlist_id: source_playlist_id,
                account_id,
                account_name,
                account_email,
                service_id,
                service_name,
                service_playlist_id,
                synced_at,
                snapshot_id,
                is_primary: is_primary != 0,
            },
        )
        .collect();

    Ok(Some(PlaylistCanonicalMembership {
        canonical_id,
        is_primary: primary_playlist_id == playlist_id,
        sources,
    }))
}

fn normalize_playlist_name(name: &str) -> String {
    let lowered = name.to_lowercase();
    let folded: String = lowered
        .chars()
        .filter(|character| !is_combining_mark(*character))
        .map(fold_latin_accent)
        .collect();
    let mut normalized = folded.split_whitespace().collect::<Vec<_>>().join(" ");

    loop {
        let without_suffix = COPY_SUFFIXES.iter().find_map(|suffix| {
            normalized
                .strip_suffix(suffix)
                .map(str::trim_end)
                .filter(|base| !base.is_empty())
        });
        match without_suffix {
            Some(base) => normalized = base.to_string(),
            None => return normalized,
        }
    }
}

const COPY_SUFFIXES: &[&str] = &[
    "(copy)",
    "[copy]",
    "- copy",
    "– copy",
    "— copy",
    "(copia)",
    "[copia]",
    " copy",
    " copia",
    "- copia",
    "– copia",
    "— copia",
];

fn is_combining_mark(character: char) -> bool {
    matches!(
        character,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

/// Pliegue de los acentes latinos precompuestos más comunes. Los nombres de
/// playlist llegan en UTF-8 y no conviene añadir una dependencia de Unicode
/// completa para esta normalización acotada.
fn fold_latin_accent(character: char) -> char {
    match character {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' | 'ǎ' | 'ǟ' | 'ǡ' | 'ǻ' | 'ȁ' | 'ȃ'
        | 'ȧ' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' | 'ȅ' | 'ȇ' | 'ȩ' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' | 'ǵ' => 'g',
        'ĥ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ǐ' | 'ȉ' | 'ȋ' => 'i',
        'ĵ' | 'ǰ' => 'j',
        'ķ' | 'ǩ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ȴ' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ǹ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' | 'ơ' | 'ǒ' | 'ǫ' | 'ǭ' | 'ǿ' | 'ȍ' | 'ȏ'
        | 'ȫ' | 'ȭ' | 'ȯ' | 'ȱ' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => 's',
        'ţ' | 'ť' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' | 'ư' | 'ǔ' | 'ǖ' | 'ǘ' | 'ǚ'
        | 'ǜ' | 'ȕ' | 'ȗ' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        other => other,
    }
}

async fn load_candidate(
    conn: &mut SqliteConnection,
    playlist_id: i64,
) -> Result<Candidate, CandidateLoadError> {
    let sources: Vec<(i64, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT id, reported_track_count, expansion_complete FROM playlist_sources
         WHERE playlist_id = ? ORDER BY id",
    )
    .bind(playlist_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| {
        CandidateLoadError::Database(format!(
            "Failed to read playlist sources for {playlist_id}: {e}"
        ))
    })?;

    if sources.is_empty() {
        return Err(CandidateLoadError::Skip(CandidateSkip::Incomplete));
    }

    let track_rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT track_id FROM playlist_tracks WHERE playlist_id = ? ORDER BY track_id",
    )
    .bind(playlist_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| {
        CandidateLoadError::Database(format!(
            "Failed to read playlist tracks for {playlist_id}: {e}"
        ))
    })?;

    if track_rows.is_empty() {
        return Err(CandidateLoadError::Skip(CandidateSkip::Empty));
    }

    let occurrences = track_rows.len() as i64;
    if sources
        .iter()
        .any(|(_, reported, complete)| *reported != Some(occurrences) || *complete != Some(1))
    {
        return Err(CandidateLoadError::Skip(CandidateSkip::Incomplete));
    }

    let track_ids: BTreeSet<i64> = track_rows.into_iter().map(|(track_id,)| track_id).collect();
    let mut hasher = Sha256::new();
    hasher.update(b"syncify.playlist.content.v1\0");
    for track_id in &track_ids {
        hasher.update(track_id.to_be_bytes());
    }

    Ok(Candidate {
        playlist_id,
        content_hash: format!("{:x}", hasher.finalize()),
        track_count: track_ids.len() as i64,
    })
}

async fn persist_group(
    conn: &mut SqliteConnection,
    normalized_name: &str,
    candidates: Vec<Candidate>,
) -> Result<(), String> {
    let representative = candidates
        .first()
        .ok_or_else(|| "Cannot persist an empty playlist canonical group".to_string())?;

    let canonical_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO playlist_canonicals
            (normalized_name, content_hash, track_count)
        VALUES (?, ?, ?)
        ON CONFLICT(normalized_name, content_hash) DO UPDATE SET
            track_count = excluded.track_count,
            updated_at = CASE
                WHEN playlist_canonicals.track_count != excluded.track_count
                THEN CURRENT_TIMESTAMP
                ELSE playlist_canonicals.updated_at
            END
        RETURNING id
        "#,
    )
    .bind(normalized_name)
    .bind(&representative.content_hash)
    .bind(representative.track_count)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| format!("Failed to upsert playlist canonical: {e}"))?;

    for candidate in candidates {
        sqlx::query(
            "INSERT INTO playlist_canonical_members (canonical_id, playlist_id)
             VALUES (?, ?)
             ON CONFLICT(playlist_id) DO UPDATE SET canonical_id = excluded.canonical_id",
        )
        .bind(canonical_id)
        .bind(candidate.playlist_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to link playlist canonical member: {e}"))?;
    }

    Ok(())
}

async fn remove_member(conn: &mut SqliteConnection, playlist_id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM playlist_canonical_members WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(conn)
        .await
        .map_err(|e| format!("Failed to remove ineligible playlist canonical member: {e}"))?;
    Ok(())
}

async fn prune_empty_canonicals(conn: &mut SqliteConnection) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM playlist_canonicals
         WHERE NOT EXISTS (
             SELECT 1 FROM playlist_canonical_members m
             WHERE m.canonical_id = playlist_canonicals.id
         )",
    )
    .execute(conn)
    .await
    .map_err(|e| format!("Failed to prune empty playlist canonicals: {e}"))?;
    Ok(())
}

async fn read_summary(
    conn: &mut SqliteConnection,
    playlists_examined: usize,
    incomplete_playlists: usize,
    empty_playlists: usize,
) -> Result<CanonicalizationSummary, String> {
    let counts: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM playlist_canonicals c
             WHERE EXISTS (SELECT 1 FROM playlist_canonical_members m WHERE m.canonical_id = c.id)),
            (SELECT COUNT(*) FROM playlist_canonical_members),
            (SELECT COUNT(*) FROM playlist_canonicals c
             WHERE (SELECT COUNT(*) FROM playlist_canonical_members m WHERE m.canonical_id = c.id) > 1)
        "#,
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| format!("Failed to summarize playlist canonicals: {e}"))?;

    Ok(CanonicalizationSummary {
        playlists_examined,
        canonicals: counts.0 as usize,
        playlists_linked: counts.1 as usize,
        merged_canonicals: counts.2 as usize,
        incomplete_playlists,
        empty_playlists,
    })
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Pool, Sqlite};

    use crate::services::playlist_canonical::{
        canonicalize_playlist, canonicalize_playlists, get_playlist_canonical_membership,
        record_playlist_source_expansion,
    };

    async fn migrated_pool() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool :memory:");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("todas las migraciones reales aplican");
        pool
    }

    async fn seed_account(pool: &Pool<Sqlite>, service: &str, label: &str) -> i64 {
        sqlx::query_scalar(
            "INSERT INTO accounts (service_id, display_name, email)
             SELECT id, ?, ? FROM services WHERE name = ? RETURNING id",
        )
        .bind(label)
        .bind(format!("{label}@example.test"))
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("cuenta sembrada")
    }

    async fn seed_tracks(pool: &Pool<Sqlite>, count: i64) -> Vec<i64> {
        let mut ids = Vec::new();
        for index in 0..count {
            let id =
                sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
                    .bind(format!("Track {index}"))
                    .bind(format!("CANON-{}-{index}", uuid::Uuid::new_v4()))
                    .fetch_one(pool)
                    .await
                    .expect("track sembrada");
            ids.push(id);
        }
        ids
    }

    /// Fuente remota con cuenta, servicio y contenido. `declared_count` es la
    /// declaración del proveedor: diferirlo de las filas simula expansión
    /// incompleta incluso cuando el contador local se reconcilia después.
    async fn seed_remote_playlist(
        pool: &Pool<Sqlite>,
        account_id: i64,
        remote_id: &str,
        name: &str,
        declared_count: i64,
        track_ids: &[i64],
        created_at: &str,
    ) -> i64 {
        let playlist_id = sqlx::query_scalar(
            "INSERT INTO playlists
                (account_id, service_playlist_id, name, track_count, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(account_id)
        .bind(remote_id)
        .bind(name)
        .bind(declared_count)
        .bind(created_at)
        .bind(created_at)
        .fetch_one(pool)
        .await
        .expect("playlist remota sembrada");

        sqlx::query(
            "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
             SELECT ?, ?, service_id, ? FROM accounts WHERE id = ?",
        )
        .bind(playlist_id)
        .bind(account_id)
        .bind(remote_id)
        .bind(account_id)
        .execute(pool)
        .await
        .expect("fuente remota sembrada");

        for (position, track_id) in track_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position)
                 VALUES (?, ?, ?)",
            )
            .bind(playlist_id)
            .bind(track_id)
            .bind(position as i64 + 1)
            .execute(pool)
            .await
            .expect("pista enlazada");
        }

        let has_expansion_state: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('playlist_sources')
             WHERE name = 'expansion_complete'",
        )
        .fetch_one(pool)
        .await
        .expect("esquema de expansión consultado");
        if has_expansion_state > 0 {
            record_playlist_source_expansion(pool, playlist_id)
                .await
                .expect("estado de expansión registrado");
        }

        playlist_id
    }

    #[tokio::test]
    async fn exact_copies_cross_accounts_and_services_share_the_oldest_source() {
        let pool = migrated_pool().await;
        let spotify_a = seed_account(&pool, "spotify", "Spotify A").await;
        let spotify_b = seed_account(&pool, "spotify", "Spotify B").await;
        let qobuz = seed_account(&pool, "qobuz", "Qobuz").await;
        let tracks = seed_tracks(&pool, 2).await;

        let oldest = seed_remote_playlist(
            &pool,
            spotify_a,
            "sp-road",
            "  Road Trip  ",
            2,
            &[tracks[0], tracks[1]],
            "2024-01-01 00:00:00",
        )
        .await;
        let same_account_copy = seed_remote_playlist(
            &pool,
            spotify_a,
            "sp-road-copy",
            "road trip",
            2,
            &[tracks[1], tracks[0]],
            "2024-02-01 00:00:00",
        )
        .await;
        let other_service = seed_remote_playlist(
            &pool,
            qobuz,
            "qb-road",
            "ROAD TRIP",
            3,
            &[tracks[0], tracks[1], tracks[0]],
            "2024-03-01 00:00:00",
        )
        .await;
        let _other_account = seed_remote_playlist(
            &pool,
            spotify_b,
            "sp-road-b",
            "Road Trip",
            2,
            &[tracks[1], tracks[0]],
            "2024-04-01 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool)
            .await
            .expect("canonicalización de catálogo");

        assert_eq!(summary.canonicals, 1);
        assert_eq!(summary.merged_canonicals, 1);
        assert_eq!(summary.playlists_linked, 4);

        for playlist_id in [oldest, same_account_copy, other_service] {
            let membership = get_playlist_canonical_membership(&pool, playlist_id)
                .await
                .expect("ficha canónica")
                .expect("playlist participa");
            assert_eq!(membership.sources.len(), 4);
            assert_eq!(membership.is_primary, playlist_id == oldest);
            assert_eq!(
                membership
                    .sources
                    .iter()
                    .filter(|source| source.is_primary)
                    .count(),
                1
            );
            assert_eq!(
                membership
                    .sources
                    .iter()
                    .find(|source| source.is_primary)
                    .unwrap()
                    .playlist_id,
                oldest
            );
            assert!(membership
                .sources
                .iter()
                .any(|source| source.account_id == spotify_a));
            assert!(membership
                .sources
                .iter()
                .any(|source| source.account_id == spotify_b));
            assert!(membership
                .sources
                .iter()
                .any(|source| source.service_name == "qobuz"));
        }
    }

    #[tokio::test]
    async fn same_name_with_different_content_stays_separate() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Spotify").await;
        let tracks = seed_tracks(&pool, 3).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "sp-first",
            "Mix",
            2,
            &tracks[..2],
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "sp-second",
            " MIX ",
            2,
            &[tracks[0], tracks[2]],
            "2024-01-02 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool)
            .await
            .expect("canonicalización");

        assert_eq!(summary.canonicals, 2);
        assert_eq!(summary.merged_canonicals, 0);
        let first_membership = get_playlist_canonical_membership(&pool, first)
            .await
            .expect("primera ficha")
            .expect("primera participa");
        let second_membership = get_playlist_canonical_membership(&pool, second)
            .await
            .expect("segunda ficha")
            .expect("segunda participa");
        assert_ne!(
            first_membership.canonical_id,
            second_membership.canonical_id
        );
        assert_eq!(first_membership.sources.len(), 1);
        assert_eq!(second_membership.sources.len(), 1);
    }

    #[tokio::test]
    async fn empty_and_incomplete_expansions_do_not_merge() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "tidal", "Tidal").await;
        let tracks = seed_tracks(&pool, 2).await;
        let empty = seed_remote_playlist(
            &pool,
            account,
            "td-empty",
            "Empty",
            0,
            &[],
            "2024-01-01 00:00:00",
        )
        .await;
        let incomplete = seed_remote_playlist(
            &pool,
            account,
            "td-incomplete",
            "Incomplete",
            3,
            &tracks,
            "2024-01-02 00:00:00",
        )
        .await;
        let complete_a = seed_remote_playlist(
            &pool,
            account,
            "td-complete-a",
            "Complete",
            2,
            &tracks,
            "2024-01-03 00:00:00",
        )
        .await;
        let complete_b = seed_remote_playlist(
            &pool,
            account,
            "td-complete-b",
            "complete",
            2,
            &[tracks[1], tracks[0]],
            "2024-01-04 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool)
            .await
            .expect("canonicalización");

        assert_eq!(summary.empty_playlists, 1);
        assert_eq!(summary.incomplete_playlists, 1);
        assert_eq!(summary.canonicals, 1);
        assert_eq!(summary.playlists_linked, 2);
        assert!(get_playlist_canonical_membership(&pool, empty)
            .await
            .expect("vacía leída")
            .is_none());
        assert!(get_playlist_canonical_membership(&pool, incomplete)
            .await
            .expect("incompleta leída")
            .is_none());
        let membership = get_playlist_canonical_membership(&pool, complete_a)
            .await
            .expect("completa leída")
            .expect("completa participa");
        assert_eq!(membership.sources.len(), 2);
        assert!(membership.is_primary);
        assert_eq!(
            get_playlist_canonical_membership(&pool, complete_b)
                .await
                .expect("copia leída")
                .unwrap()
                .canonical_id,
            membership.canonical_id
        );
    }

    #[tokio::test]
    async fn measured_64_name_groups_merge_only_the_16_exact_sets() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Measured").await;
        let tracks = seed_tracks(&pool, 3).await;

        for group in 0..64 {
            let first_tracks = if group % 2 == 0 {
                vec![tracks[0], tracks[1]]
            } else {
                vec![tracks[0], tracks[2]]
            };
            let second_tracks = if group < 16 {
                vec![first_tracks[1], first_tracks[0]]
            } else {
                vec![tracks[1], tracks[2]]
            };
            seed_remote_playlist(
                &pool,
                account,
                &format!("measured-{group}-a"),
                &format!(" Measured Group {group:02} "),
                2,
                &first_tracks,
                "2024-01-01 00:00:00",
            )
            .await;
            seed_remote_playlist(
                &pool,
                account,
                &format!("measured-{group}-b"),
                &format!("measured group {group:02}"),
                2,
                &second_tracks,
                "2024-01-02 00:00:00",
            )
            .await;
        }

        let summary = canonicalize_playlists(&pool).await.expect("64 grupos");

        assert_eq!(summary.playlists_examined, 128);
        assert_eq!(summary.playlists_linked, 128);
        assert_eq!(summary.merged_canonicals, 16);
        assert_eq!(summary.canonicals, 16 + 48 * 2);
        assert_eq!(summary.incomplete_playlists, 0);
        assert_eq!(summary.empty_playlists, 0);
    }

    #[tokio::test]
    async fn changed_content_splits_from_its_copy_and_reruns_are_idempotent() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "qobuz", "Qobuz").await;
        let tracks = seed_tracks(&pool, 3).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "qb-first",
            "Split",
            2,
            &[tracks[0], tracks[1]],
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "qb-second",
            "split",
            2,
            &[tracks[1], tracks[0]],
            "2024-01-02 00:00:00",
        )
        .await;

        canonicalize_playlists(&pool).await.expect("fusión inicial");
        let before = get_playlist_canonical_membership(&pool, second)
            .await
            .expect("antes del cambio")
            .unwrap();
        assert_eq!(before.sources.len(), 2);

        sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
            .bind(second)
            .execute(&pool)
            .await
            .expect("contenido anterior retirado");
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position)
             VALUES (?, ?, 1), (?, ?, 2)",
        )
        .bind(second)
        .bind(tracks[0])
        .bind(second)
        .bind(tracks[2])
        .execute(&pool)
        .await
        .expect("nuevo contenido");
        sqlx::query("UPDATE playlists SET track_count = 2 WHERE id = ?")
            .bind(second)
            .execute(&pool)
            .await
            .expect("declaración actualizada");
        sqlx::query(
            "UPDATE playlist_sources SET synced_at = CURRENT_TIMESTAMP WHERE playlist_id = ?",
        )
        .bind(second)
        .execute(&pool)
        .await
        .expect("fuente reexpandida");
        record_playlist_source_expansion(&pool, second)
            .await
            .expect("expansión reexpandida completa");

        canonicalize_playlist(&pool, second)
            .await
            .expect("separación incremental");

        let first_after = get_playlist_canonical_membership(&pool, first)
            .await
            .expect("primera tras el cambio")
            .unwrap();
        let second_after = get_playlist_canonical_membership(&pool, second)
            .await
            .expect("segunda tras el cambio")
            .unwrap();
        assert_ne!(first_after.canonical_id, second_after.canonical_id);
        assert_eq!(first_after.sources.len(), 1);
        assert_eq!(second_after.sources.len(), 1);

        let once = canonicalize_playlists(&pool)
            .await
            .expect("primera repetición");
        let twice = canonicalize_playlists(&pool)
            .await
            .expect("segunda repetición");
        assert_eq!(once.canonicals, twice.canonicals);
        assert_eq!(once.playlists_linked, twice.playlists_linked);
        assert_eq!(once.merged_canonicals, twice.merged_canonicals);
    }

    #[tokio::test]
    async fn normalization_folds_accents_whitespace_and_explicit_copy_suffixes() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Normalization").await;
        let tracks = seed_tracks(&pool, 2).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "norm-a",
            "  Café   Road (Copy) ",
            2,
            &tracks,
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "norm-b",
            "CAFE ROAD Copia",
            2,
            &[tracks[1], tracks[0]],
            "2024-01-02 00:00:00",
        )
        .await;
        let numbered = seed_remote_playlist(
            &pool,
            account,
            "norm-numbered",
            "Cafe Road (2)",
            2,
            &tracks,
            "2024-01-03 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool).await.expect("normalización");

        assert_eq!(summary.canonicals, 2);
        assert_eq!(summary.merged_canonicals, 1);
        assert_eq!(
            get_playlist_canonical_membership(&pool, first)
                .await
                .unwrap()
                .unwrap()
                .canonical_id,
            get_playlist_canonical_membership(&pool, second)
                .await
                .unwrap()
                .unwrap()
                .canonical_id
        );
        assert_ne!(
            get_playlist_canonical_membership(&pool, first)
                .await
                .unwrap()
                .unwrap()
                .canonical_id,
            get_playlist_canonical_membership(&pool, numbered)
                .await
                .unwrap()
                .unwrap()
                .canonical_id
        );
    }

    #[tokio::test]
    async fn distinct_latin_letters_are_not_folded_as_accents() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Distinct letters").await;
        let tracks = seed_tracks(&pool, 2).await;
        seed_remote_playlist(
            &pool,
            account,
            "letter-i",
            "I",
            2,
            &tracks,
            "2024-01-01 00:00:00",
        )
        .await;
        seed_remote_playlist(
            &pool,
            account,
            "letter-dotless-i",
            "ı",
            2,
            &tracks,
            "2024-01-02 00:00:00",
        )
        .await;

        let summary = canonicalize_playlists(&pool)
            .await
            .expect("letras distintas");

        assert_eq!(summary.canonicals, 2);
        assert_eq!(summary.merged_canonicals, 0);
    }

    #[tokio::test]
    async fn renaming_primary_keeps_the_surviving_group_consistent() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "qobuz", "Rename").await;
        let tracks = seed_tracks(&pool, 2).await;
        let primary = seed_remote_playlist(
            &pool,
            account,
            "rename-primary",
            "Original",
            2,
            &tracks,
            "2024-01-01 00:00:00",
        )
        .await;
        let copy = seed_remote_playlist(
            &pool,
            account,
            "rename-copy",
            "original",
            2,
            &tracks,
            "2024-01-02 00:00:00",
        )
        .await;
        canonicalize_playlists(&pool).await.expect("fusión inicial");
        let original_id = get_playlist_canonical_membership(&pool, copy)
            .await
            .unwrap()
            .unwrap()
            .canonical_id;

        sqlx::query("UPDATE playlists SET name = 'Renamed' WHERE id = ?")
            .bind(primary)
            .execute(&pool)
            .await
            .unwrap();
        canonicalize_playlist(&pool, primary)
            .await
            .expect("rename recanonicalizado");

        let surviving = get_playlist_canonical_membership(&pool, copy)
            .await
            .expect("copia superviviente")
            .expect("la copia sigue participando");
        assert_eq!(surviving.canonical_id, original_id);
        assert!(surviving.is_primary);
        assert_eq!(surviving.sources.len(), 1);
        assert!(surviving.sources[0].is_primary);
        assert_ne!(
            get_playlist_canonical_membership(&pool, primary)
                .await
                .unwrap()
                .unwrap()
                .canonical_id,
            original_id
        );
    }

    #[tokio::test]
    async fn deleting_primary_keeps_copies_from_other_accounts() {
        let pool = migrated_pool().await;
        let spotify = seed_account(&pool, "spotify", "Delete Spotify").await;
        let qobuz = seed_account(&pool, "qobuz", "Delete Qobuz").await;
        let tracks = seed_tracks(&pool, 2).await;
        let primary = seed_remote_playlist(
            &pool,
            spotify,
            "delete-primary",
            "Survivors",
            2,
            &tracks,
            "2024-01-01 00:00:00",
        )
        .await;
        let copy = seed_remote_playlist(
            &pool,
            qobuz,
            "delete-copy",
            "survivors",
            2,
            &tracks,
            "2024-01-02 00:00:00",
        )
        .await;
        canonicalize_playlists(&pool).await.expect("fusión inicial");
        let canonical_id = get_playlist_canonical_membership(&pool, copy)
            .await
            .unwrap()
            .unwrap()
            .canonical_id;

        sqlx::query("DELETE FROM playlists WHERE id = ?")
            .bind(primary)
            .execute(&pool)
            .await
            .expect("primaria borrada");

        let surviving = get_playlist_canonical_membership(&pool, copy)
            .await
            .expect("copia superviviente")
            .expect("el grupo no desaparece");
        assert_eq!(surviving.canonical_id, canonical_id);
        assert!(surviving.is_primary);
        assert_eq!(surviving.sources.len(), 1);
        assert_eq!(surviving.sources[0].account_id, qobuz);
    }

    #[tokio::test]
    async fn reencounter_and_content_mutation_invalidate_membership_until_completed() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Invalidation").await;
        let tracks = seed_tracks(&pool, 3).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "invalidate-a",
            "Invalidate",
            2,
            &tracks[..2],
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "invalidate-b",
            "invalidate",
            2,
            &tracks[..2],
            "2024-01-02 00:00:00",
        )
        .await;
        canonicalize_playlists(&pool).await.expect("fusión inicial");

        sqlx::query(
            "UPDATE playlist_sources SET synced_at = CURRENT_TIMESTAMP WHERE playlist_id = ?",
        )
        .bind(second)
        .execute(&pool)
        .await
        .expect("fuente reencontrada");
        assert!(get_playlist_canonical_membership(&pool, second)
            .await
            .unwrap()
            .is_none());
        let states: Vec<(i64, Option<i64>)> = sqlx::query_as(
            "SELECT ps.playlist_id, ps.expansion_complete
             FROM playlist_sources ps ORDER BY ps.playlist_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(states, vec![(first, Some(1)), (second, None)]);
        let during_expansion = canonicalize_playlists(&pool)
            .await
            .expect("durante la expansión");
        assert_eq!(during_expansion.incomplete_playlists, 1);
        assert!(get_playlist_canonical_membership(&pool, second)
            .await
            .unwrap()
            .is_none());

        assert!(record_playlist_source_expansion(&pool, second)
            .await
            .expect("recorrido completo"));
        canonicalize_playlist(&pool, second)
            .await
            .expect("recuperar fusión");
        assert_eq!(
            get_playlist_canonical_membership(&pool, second)
                .await
                .unwrap()
                .unwrap()
                .sources
                .len(),
            2
        );

        sqlx::query(
            "UPDATE playlist_tracks SET track_id = ? WHERE playlist_id = ? AND position = 1",
        )
        .bind(tracks[2])
        .bind(second)
        .execute(&pool)
        .await
        .expect("reidentificación de pista");
        assert!(get_playlist_canonical_membership(&pool, second)
            .await
            .unwrap()
            .is_none());

        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position)
             VALUES (?, ?, 3)",
        )
        .bind(second)
        .bind(tracks[0])
        .execute(&pool)
        .await
        .expect("mutación local");
        assert!(get_playlist_canonical_membership(&pool, second)
            .await
            .unwrap()
            .is_none());
        assert!(get_playlist_canonical_membership(&pool, first)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn unknown_historical_totals_are_not_treated_as_complete() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "tidal", "Unknown totals").await;
        let tracks = seed_tracks(&pool, 2).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "unknown-a",
            "Unknown",
            2,
            &tracks,
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "unknown-b",
            "unknown",
            2,
            &tracks,
            "2024-01-02 00:00:00",
        )
        .await;
        sqlx::query(
            "UPDATE playlist_sources SET reported_track_count = NULL, expansion_complete = 0",
        )
        .execute(&pool)
        .await
        .expect("total histórico desconocido");

        let summary = canonicalize_playlists(&pool).await.expect("sin evidencia");

        assert_eq!(summary.incomplete_playlists, 2);
        assert_eq!(summary.canonicals, 0);
        assert!(get_playlist_canonical_membership(&pool, first)
            .await
            .unwrap()
            .is_none());
        assert!(get_playlist_canonical_membership(&pool, second)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn migrations_0117_0118_drop_fabricated_evidence_and_recover_after_reexpansion() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool :memory:");
        let migrator = sqlx::migrate!("./migrations");
        let through_0116 = sqlx::migrate::Migrator {
            migrations: std::borrow::Cow::Owned(
                migrator
                    .iter()
                    .filter(|migration| migration.version <= 116)
                    .map(|migration| (*migration).clone())
                    .collect(),
            ),
            ignore_missing: false,
            locking: true,
            no_tx: false,
        };
        through_0116.run(&pool).await.expect("migración hasta 0116");

        let account = seed_account(&pool, "spotify", "Upgrade").await;
        let tracks = seed_tracks(&pool, 2).await;
        let first = seed_remote_playlist(
            &pool,
            account,
            "upgrade-a",
            "Upgrade",
            2,
            &tracks,
            "2024-01-01 00:00:00",
        )
        .await;
        let second = seed_remote_playlist(
            &pool,
            account,
            "upgrade-b",
            "upgrade",
            2,
            &tracks,
            "2024-01-02 00:00:00",
        )
        .await;
        sqlx::query(
            "INSERT INTO playlist_canonicals
                (normalized_name, content_hash, track_count, primary_playlist_id)
             VALUES ('upgrade', ?, 2, ?)",
        )
        .bind("0".repeat(64))
        .bind(first)
        .execute(&pool)
        .await
        .expect("canónica sembrada bajo 0116");
        let canonical_id: i64 = sqlx::query_scalar(
            "INSERT INTO playlist_canonical_members (canonical_id, playlist_id)
             SELECT id, ? FROM playlist_canonicals
             RETURNING canonical_id",
        )
        .bind(first)
        .fetch_one(&pool)
        .await
        .expect("primera membresía");
        sqlx::query(
            "INSERT INTO playlist_canonical_members (canonical_id, playlist_id) VALUES (?, ?)",
        )
        .bind(canonical_id)
        .bind(second)
        .execute(&pool)
        .await
        .expect("segunda membresía");
        let before = get_playlist_canonical_membership(&pool, first)
            .await
            .unwrap()
            .unwrap();

        let migration_0117 = migrator
            .iter()
            .find(|migration| migration.version == 117)
            .expect("0117 embebida");
        sqlx::raw_sql(&migration_0117.sql)
            .execute(&pool)
            .await
            .expect("0117 aplica");

        let after = get_playlist_canonical_membership(&pool, first)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.canonical_id, before.canonical_id);
        assert_eq!(after.sources.len(), 2);
        let unknown_totals: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM playlist_sources WHERE reported_track_count IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(unknown_totals, 2);

        let migration_0118 = migrator
            .iter()
            .find(|migration| migration.version == 118)
            .expect("0118 embebida");
        sqlx::raw_sql(&migration_0118.sql)
            .execute(&pool)
            .await
            .expect("0118 aplica");
        assert!(get_playlist_canonical_membership(&pool, first)
            .await
            .unwrap()
            .is_none());
        let incomplete_states: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM playlist_sources WHERE expansion_complete = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(incomplete_states, 2);

        // Una re-expansión real vuelve a capturar la declaración del proveedor.
        sqlx::query("UPDATE playlist_sources SET synced_at = CURRENT_TIMESTAMP")
            .execute(&pool)
            .await
            .expect("fuentes reexpandidas");
        for playlist_id in [first, second] {
            assert!(record_playlist_source_expansion(&pool, playlist_id)
                .await
                .expect("expansión completa"));
        }
        let recovered = canonicalize_playlists(&pool).await.expect("recuperación");
        assert_eq!(recovered.merged_canonicals, 1);
        assert_eq!(
            get_playlist_canonical_membership(&pool, second)
                .await
                .unwrap()
                .unwrap()
                .sources
                .len(),
            2
        );
    }
}
