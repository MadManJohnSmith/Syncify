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
    created_at: Option<String>,
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

    let playlist: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, created_at FROM playlists
         WHERE id = ? AND deleted_at IS NULL AND COALESCE(is_smart, 0) = 0 AND local_uuid IS NULL",
    )
    .bind(playlist_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| format!("Failed to read playlist {playlist_id}: {e}"))?;

    let normalized_name = playlist
        .as_ref()
        .map(|(name, _)| normalize_playlist_name(name))
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
    let canonical: Option<(i64, i64)> = sqlx::query_as(
        "SELECT c.id, c.primary_playlist_id
         FROM playlist_canonical_members m
         JOIN playlist_canonicals c ON c.id = m.canonical_id
         WHERE m.playlist_id = ?",
    )
    .bind(playlist_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to read playlist canonical membership: {e}"))?;

    let Some((canonical_id, primary_playlist_id)) = canonical else {
        return Ok(None);
    };

    let rows: Vec<(
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
        SELECT ps.id,
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
        FROM playlist_canonical_members m
        JOIN playlist_canonicals c ON c.id = m.canonical_id
        JOIN playlist_sources ps ON ps.playlist_id = m.playlist_id
        JOIN playlists p ON p.id = ps.playlist_id
        JOIN accounts a ON a.id = ps.account_id
        JOIN services s ON s.id = ps.service_id
        WHERE m.canonical_id = ?
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
    .bind(canonical_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to list playlist canonical sources: {e}"))?;

    let sources = rows
        .into_iter()
        .map(
            |(
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
    name.trim().to_lowercase()
}

async fn load_candidate(
    conn: &mut SqliteConnection,
    playlist_id: i64,
) -> Result<Candidate, CandidateLoadError> {
    let sources: Vec<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, reported_track_count FROM playlist_sources
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
        .any(|(_, reported)| *reported != Some(occurrences))
    {
        return Err(CandidateLoadError::Skip(CandidateSkip::Incomplete));
    }

    let track_ids: BTreeSet<i64> = track_rows.into_iter().map(|(track_id,)| track_id).collect();
    let mut hasher = Sha256::new();
    hasher.update(b"syncify.playlist.content.v1\0");
    for track_id in &track_ids {
        hasher.update(track_id.to_be_bytes());
    }

    let created_at: Option<(Option<String>,)> =
        sqlx::query_as("SELECT created_at FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| {
                CandidateLoadError::Database(format!("Failed to read playlist creation date: {e}"))
            })?;

    Ok(Candidate {
        playlist_id,
        created_at: created_at.and_then(|(created_at,)| created_at),
        content_hash: format!("{:x}", hasher.finalize()),
        track_count: track_ids.len() as i64,
    })
}

async fn persist_group(
    conn: &mut SqliteConnection,
    normalized_name: &str,
    mut candidates: Vec<Candidate>,
) -> Result<(), String> {
    candidates.sort_by(|left, right| {
        let left_key = (
            left.created_at.is_none(),
            left.created_at.as_deref().unwrap_or_default(),
            left.playlist_id,
        );
        let right_key = (
            right.created_at.is_none(),
            right.created_at.as_deref().unwrap_or_default(),
            right.playlist_id,
        );
        left_key.cmp(&right_key)
    });
    let primary = candidates
        .first()
        .ok_or_else(|| "Cannot persist an empty playlist canonical group".to_string())?;

    let canonical_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO playlist_canonicals
            (normalized_name, content_hash, track_count, primary_playlist_id)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(normalized_name, content_hash) DO UPDATE SET
            primary_playlist_id = excluded.primary_playlist_id,
            track_count = excluded.track_count,
            updated_at = CASE
                WHEN playlist_canonicals.primary_playlist_id != excluded.primary_playlist_id
                  OR playlist_canonicals.track_count != excluded.track_count
                THEN CURRENT_TIMESTAMP
                ELSE playlist_canonicals.updated_at
            END
        RETURNING id
        "#,
    )
    .bind(normalized_name)
    .bind(&primary.content_hash)
    .bind(primary.track_count)
    .bind(primary.playlist_id)
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
}
