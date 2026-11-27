//! IPC del modo de conflicto y la cola de revisión de playlists (#410).
//!
//! El contrato de persistencia y decisión vive en `services::playlist_canonical`
//! (#403). Este módulo solo expone a la UI la ficha de conflicto y resuelve las
//! filas `needs_review` con una acción explícita del usuario: «conservar local»,
//! «usar proveedor» o descartar. Resolver «proveedor» es el único camino que
//! escribe el valor remoto; ninguna resolución ocurre en silencio.

use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, Transaction};

use crate::services::playlist_canonical::PlaylistConflictMode;

const REVIEW_STATUS_NEEDS_REVIEW: &str = "needs_review";
const REVIEW_STATUS_RESOLVED_LOCAL: &str = "resolved_local";
const REVIEW_STATUS_RESOLVED_PROVIDER: &str = "resolved_provider";
const REVIEW_STATUS_DISMISSED: &str = "dismissed";

/// Ficha de conflicto de una playlist para la UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistConflictState {
    pub playlist_id: i64,
    pub conflict_mode: String,
    pub service_name: Option<String>,
    /// La playlist se importó de un proveedor (tiene al menos una fuente).
    pub is_remote: bool,
    /// Modo de fábrica sin elección del usuario: la próxima edición local debe
    /// preguntar antes de arriesgar el cambio.
    pub requires_mode_choice: bool,
    pub pending_reviews: Vec<PlaylistConflictReviewEntry>,
}

/// Una revisión de conflicto con la evidencia de ambos lados y el nombre de la
/// playlist, para que la cola global sea legible sin consultas adicionales.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistConflictReviewEntry {
    pub id: i64,
    pub playlist_id: i64,
    pub playlist_name: String,
    pub service_name: Option<String>,
    pub conflict_kind: String,
    pub local_value: Option<String>,
    pub provider_value: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    /// Valores decodificados solo para `membership`; `None` en `name`.
    pub local_track_ids: Option<Vec<i64>>,
    pub provider_track_ids: Option<Vec<i64>>,
}

#[derive(Debug, sqlx::FromRow)]
struct ReviewRow {
    id: i64,
    playlist_id: i64,
    playlist_name: String,
    service_name: Option<String>,
    conflict_kind: String,
    local_value: Option<String>,
    provider_value: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
    resolved_at: Option<String>,
}

impl ReviewRow {
    fn into_entry(self) -> PlaylistConflictReviewEntry {
        let is_membership = self.conflict_kind == "membership";
        let local_track_ids = if is_membership {
            decode_track_ids(self.local_value.as_deref())
        } else {
            None
        };
        let provider_track_ids = if is_membership {
            decode_track_ids(self.provider_value.as_deref())
        } else {
            None
        };
        PlaylistConflictReviewEntry {
            id: self.id,
            playlist_id: self.playlist_id,
            playlist_name: self.playlist_name,
            service_name: self.service_name,
            conflict_kind: self.conflict_kind,
            local_value: self.local_value,
            provider_value: self.provider_value,
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
            resolved_at: self.resolved_at,
            local_track_ids,
            provider_track_ids,
        }
    }
}

/// Acción explícita del usuario sobre una revisión pendiente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReviewResolution {
    Local,
    Provider,
    Dismiss,
}

impl ReviewResolution {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "local" => Ok(Self::Local),
            "provider" => Ok(Self::Provider),
            "dismiss" => Ok(Self::Dismiss),
            other => Err(format!(
                "Unknown playlist conflict resolution '{other}': expected local, provider or dismiss"
            )),
        }
    }

    fn status(self) -> &'static str {
        match self {
            Self::Local => REVIEW_STATUS_RESOLVED_LOCAL,
            Self::Provider => REVIEW_STATUS_RESOLVED_PROVIDER,
            Self::Dismiss => REVIEW_STATUS_DISMISSED,
        }
    }
}

fn decode_track_ids(raw: Option<&str>) -> Option<Vec<i64>> {
    raw.and_then(|value| serde_json::from_str::<Vec<i64>>(value).ok())
}

fn validate_review_status(status: &str) -> Result<(), String> {
    match status {
        REVIEW_STATUS_NEEDS_REVIEW
        | REVIEW_STATUS_RESOLVED_LOCAL
        | REVIEW_STATUS_RESOLVED_PROVIDER
        | REVIEW_STATUS_DISMISSED => Ok(()),
        other => Err(format!("Unknown playlist conflict status '{other}'")),
    }
}

fn entry_select_sql(where_clause: &str) -> String {
    format!(
        "SELECT r.id, r.playlist_id, p.name AS playlist_name, s.name AS service_name,
                r.conflict_kind, r.local_value, r.provider_value, r.status,
                r.created_at, r.updated_at, r.resolved_at
         FROM playlist_conflict_reviews r
         JOIN playlists p ON p.id = r.playlist_id
         LEFT JOIN accounts a ON a.id = p.account_id
         LEFT JOIN services s ON s.id = a.service_id
         WHERE {where_clause}"
    )
}

async fn load_review_row(db: &SqlitePool, review_id: i64) -> Result<Option<ReviewRow>, String> {
    let sql = entry_select_sql("r.id = ?");
    sqlx::query_as::<_, ReviewRow>(&sql)
        .bind(review_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("Failed to read playlist conflict review: {e}"))
}

/// Estado de conflicto de una playlist: modo persistido, si es remota y sus
/// revisiones pendientes. `requires_mode_choice` es verdadero mientras siga en
/// el modo de fábrica «gana el proveedor», que perdería una edición local.
pub async fn get_playlist_conflict_state_core(
    db: &SqlitePool,
    playlist_id: i64,
) -> Result<PlaylistConflictState, String> {
    let row: Option<(String, Option<String>, i64)> = sqlx::query_as(
        "SELECT p.conflict_mode, s.name,
                (SELECT COUNT(*) FROM playlist_sources ps WHERE ps.playlist_id = p.id)
         FROM playlists p
         LEFT JOIN accounts a ON a.id = p.account_id
         LEFT JOIN services s ON s.id = a.service_id
         WHERE p.id = ? AND p.deleted_at IS NULL",
    )
    .bind(playlist_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to read playlist conflict state: {e}"))?;

    let Some((conflict_mode, service_name, source_count)) = row else {
        return Err(format!("Playlist {playlist_id} not found"));
    };
    let is_remote = source_count > 0;
    let pending_reviews = list_playlist_conflict_reviews_core(
        db,
        Some(playlist_id),
        Some(REVIEW_STATUS_NEEDS_REVIEW.to_string()),
    )
    .await?;

    Ok(PlaylistConflictState {
        playlist_id,
        requires_mode_choice: is_remote
            && conflict_mode == PlaylistConflictMode::ProviderWins.as_str(),
        conflict_mode,
        service_name,
        is_remote,
        pending_reviews,
    })
}

/// Persiste la elección de la ficha. Acepta los tres modos: la UI de primera
/// edición ofrece `editable` y `gana_local`, pero la ficha puede volver a
/// «gana el proveedor» sin una migración.
pub async fn set_playlist_conflict_mode_core(
    db: &SqlitePool,
    playlist_id: i64,
    mode: &str,
) -> Result<PlaylistConflictState, String> {
    let parsed = PlaylistConflictMode::parse(mode)
        .ok_or_else(|| format!("Unknown playlist conflict mode '{mode}'"))?;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM playlists WHERE id = ? AND deleted_at IS NULL")
            .bind(playlist_id)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Failed to read playlist: {e}"))?;
    if exists.is_none() {
        return Err(format!("Playlist {playlist_id} not found"));
    }

    let updated =
        crate::services::playlist_canonical::set_playlist_conflict_mode(db, playlist_id, parsed)
            .await?;
    if !updated {
        return Err(format!("Playlist {playlist_id} not found"));
    }
    get_playlist_conflict_state_core(db, playlist_id).await
}

/// Lista las revisiones de una playlist o de toda la biblioteca, filtradas por
/// estado. El orden deja primero las más recientes.
pub async fn list_playlist_conflict_reviews_core(
    db: &SqlitePool,
    playlist_id: Option<i64>,
    status: Option<String>,
) -> Result<Vec<PlaylistConflictReviewEntry>, String> {
    let status = status.unwrap_or_else(|| REVIEW_STATUS_NEEDS_REVIEW.to_string());
    validate_review_status(&status)?;
    let sql = format!(
        "{} AND (? IS NULL OR r.playlist_id = ?) ORDER BY r.created_at DESC, r.id DESC",
        entry_select_sql("r.status = ?")
    );
    let rows: Vec<ReviewRow> = sqlx::query_as::<_, ReviewRow>(&sql)
        .bind(&status)
        .bind(playlist_id)
        .bind(playlist_id)
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to list playlist conflict reviews: {e}"))?;
    Ok(rows.into_iter().map(ReviewRow::into_entry).collect())
}

/// Resuelve una revisión pendiente. Solo `provider` escribe el valor remoto;
/// `local` y `dismiss` conservan lo que ya está en la playlist. La fila no se
/// borra: queda como evidencia con su estado y `resolved_at`.
pub async fn resolve_playlist_conflict_review_core(
    db: &SqlitePool,
    review_id: i64,
    resolution: &str,
) -> Result<PlaylistConflictReviewEntry, String> {
    let resolution = ReviewResolution::parse(resolution)?;
    let review = load_review_row(db, review_id)
        .await?
        .ok_or_else(|| format!("Playlist conflict review {review_id} not found"))?;
    if review.status != REVIEW_STATUS_NEEDS_REVIEW {
        return Err(format!(
            "Playlist conflict review {review_id} is already {}",
            review.status
        ));
    }

    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start playlist conflict resolution: {e}"))?;

    let mut removed_tracks: Vec<i64> = Vec::new();
    if resolution == ReviewResolution::Provider {
        match review.conflict_kind.as_str() {
            "name" => {
                let provider_name = review.provider_value.as_deref().ok_or_else(|| {
                    format!("Playlist conflict review {review_id} has no provider name")
                })?;
                let result = sqlx::query(
                    "UPDATE playlists
                     SET name = ?, updated_at = CURRENT_TIMESTAMP
                     WHERE id = ? AND deleted_at IS NULL",
                )
                .bind(provider_name)
                .bind(review.playlist_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to apply provider playlist name: {e}"))?;
                if result.rows_affected() == 0 {
                    return Err(format!("Playlist {} not found", review.playlist_id));
                }
            }
            "membership" => {
                let provider_tracks = decode_track_ids(review.provider_value.as_deref())
                    .ok_or_else(|| {
                        format!(
                            "Playlist conflict review {review_id} has no valid provider membership"
                        )
                    })?;
                removed_tracks =
                    replace_playlist_tracks_in_tx(&mut tx, review.playlist_id, &provider_tracks)
                        .await?;
            }
            other => {
                return Err(format!(
                    "Playlist conflict review {review_id} has unknown kind '{other}'"
                ))
            }
        }
    }

    let result = sqlx::query(
        "UPDATE playlist_conflict_reviews
         SET status = ?, resolved_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP
         WHERE id = ? AND status = 'needs_review'",
    )
    .bind(resolution.status())
    .bind(review_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to resolve playlist conflict review: {e}"))?;
    if result.rows_affected() == 0 {
        return Err(format!(
            "Playlist conflict review {review_id} was already resolved"
        ));
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist conflict resolution: {e}"))?;

    for track_id in removed_tracks {
        if let Err(error) =
            crate::services::library_membership::refresh_track_library_added_at(db, track_id).await
        {
            tracing::warn!(
                track_id,
                %error,
                "resolved playlist membership could not refresh library_added_at"
            );
        }
    }

    load_review_row(db, review_id)
        .await?
        .map(ReviewRow::into_entry)
        .ok_or_else(|| format!("Playlist conflict review {review_id} not found"))
}

/// Sustituye la pertenencia por el valor del proveedor dentro de la
/// transacción de resolución. Devuelve las pistas que dejaron de pertenecer
/// para refrescar `library_added_at` después del commit.
///
/// Es el mismo efecto que `playlist_canonical::replace_playlist_tracks`, que es
/// privado del servicio: la resolución de #410 escribe desde la transacción que
/// también cierra la revisión, para que no queden estados intermedios.
async fn replace_playlist_tracks_in_tx(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    playlist_id: i64,
    track_ids: &[i64],
) -> Result<Vec<i64>, String> {
    let existing: Vec<(i64, Option<String>)> = sqlx::query_as(
        "SELECT track_id, added_at FROM playlist_tracks
         WHERE playlist_id = ?
         ORDER BY position ASC, id ASC",
    )
    .bind(playlist_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| format!("Failed to read current playlist membership: {e}"))?;

    let previous_track_ids: Vec<i64> = existing.iter().map(|(track_id, _)| *track_id).collect();
    let mut added_at_by_track: std::collections::HashMap<
        i64,
        std::collections::VecDeque<Option<String>>,
    > = std::collections::HashMap::new();
    for (track_id, added_at) in existing {
        added_at_by_track
            .entry(track_id)
            .or_default()
            .push_back(added_at);
    }

    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("Failed to clear playlist membership: {e}"))?;

    for (index, track_id) in track_ids.iter().enumerate() {
        let added_at = added_at_by_track
            .get_mut(track_id)
            .and_then(std::collections::VecDeque::pop_front)
            .flatten();
        sqlx::query(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind((index + 1) as i64)
        .bind(added_at)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("Failed to apply provider playlist membership: {e}"))?;
    }

    sqlx::query(
        "UPDATE playlists
         SET track_count = (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?),
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("Failed to reconcile playlist track count: {e}"))?;

    sqlx::query(
        "DELETE FROM library_memberships
         WHERE collection_type = 'playlist' AND collection_id = ?
           AND track_id NOT IN (
                SELECT track_id FROM playlist_tracks WHERE playlist_id = ?
           )",
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| format!("Failed to reconcile playlist library memberships: {e}"))?;

    let restored: std::collections::HashSet<i64> = track_ids.iter().copied().collect();
    Ok(previous_track_ids
        .into_iter()
        .filter(|track_id| !restored.contains(track_id))
        .collect())
}

/// Estado de conflicto de la ficha de playlist.
#[tauri::command]
pub async fn get_playlist_conflict_state(
    state: tauri::State<'_, crate::AppState>,
    playlist_id: i64,
) -> Result<PlaylistConflictState, String> {
    get_playlist_conflict_state_core(&state.db, playlist_id).await
}

/// Persiste el modo elegido en la ficha (`editable`, `gana_proveedor` o
/// `gana_local`) y devuelve el estado resultante.
#[tauri::command]
pub async fn set_playlist_conflict_mode(
    state: tauri::State<'_, crate::AppState>,
    playlist_id: i64,
    mode: String,
) -> Result<PlaylistConflictState, String> {
    set_playlist_conflict_mode_core(&state.db, playlist_id, &mode).await
}

/// Revisiones de conflicto; por defecto solo las pendientes (`needs_review`).
#[tauri::command]
pub async fn list_playlist_conflict_reviews(
    state: tauri::State<'_, crate::AppState>,
    playlist_id: Option<i64>,
    status: Option<String>,
) -> Result<Vec<PlaylistConflictReviewEntry>, String> {
    list_playlist_conflict_reviews_core(&state.db, playlist_id, status).await
}

/// Resuelve una revisión pendiente con una acción explícita.
#[tauri::command]
pub async fn resolve_playlist_conflict_review(
    state: tauri::State<'_, crate::AppState>,
    review_id: i64,
    resolution: String,
) -> Result<PlaylistConflictReviewEntry, String> {
    resolve_playlist_conflict_review_core(&state.db, review_id, &resolution).await
}
