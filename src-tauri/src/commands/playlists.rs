#[allow(unused_imports)]
use super::*;

// Playlist Commands - submodule of crate::commands
// Manages playlist CRUD, reordering, and multi-service synchronization

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistTrackPosition {
    pub track_id: i64,
    pub new_position: i64,
}

/// Ficha de playlist con su pertenencia canónica. `flatten` mantiene el
/// contrato histórico de `Playlist`; los campos nuevos son aditivos para la UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistDetail {
    #[serde(flatten)]
    pub playlist: Playlist,
    #[serde(default)]
    pub canonical_id: Option<i64>,
    #[serde(default)]
    pub is_primary: bool,
    #[serde(default)]
    pub sources: Vec<crate::services::playlist_canonical::PlaylistCanonicalSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPlaylistsResult {
    pub playlists_synced: i64,
    pub tracks_linked: i64,
    pub message: String,
    /// S189-F2-5: desglose real por servicio desde la tabla local.
    #[serde(default)]
    pub services: Vec<PlaylistServiceSummary>,
}

/// Agregado de catálogo local para un servicio conectado.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaylistServiceSummary {
    pub service: String,
    pub playlists: i64,
    pub tracks_linked: i64,
    /// MAX(playlists.last_synced) del servicio, si existe.
    pub last_synced: Option<String>,
}

/// Get detailed playlist information by ID, including every canonical source.
#[tauri::command]
pub async fn get_playlist(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Option<PlaylistDetail>, String> {
    get_playlist_detail_core(&state.db, id).await
}

/// Testable read seam for the playlist detail contract.
pub async fn get_playlist_detail_core(
    db: &sqlx::SqlitePool,
    id: i64,
) -> Result<Option<PlaylistDetail>, String> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("Failed to start playlist detail read: {}", e))?;
    let playlist = sqlx::query_as::<_, Playlist>(
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner_name,
            p.track_count,
            p.image_url,
            s.name as service_name
        FROM playlists p
        LEFT JOIN accounts a ON a.id = p.account_id
        LEFT JOIN services s ON s.id = a.service_id
        WHERE p.id = ? AND p.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("Failed to get playlist: {}", e))?;

    let Some(playlist) = playlist else {
        return Ok(None);
    };
    let membership =
        crate::services::playlist_canonical::get_playlist_canonical_membership_in_conn(&mut tx, id)
            .await?;
    let (canonical_id, is_primary, sources) = match membership {
        Some(membership) => (
            Some(membership.canonical_id),
            membership.is_primary,
            membership.sources,
        ),
        None => (None, false, Vec::new()),
    };

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist detail read: {}", e))?;

    Ok(Some(PlaylistDetail {
        playlist,
        canonical_id,
        is_primary,
        sources,
    }))
}

/// Update a playlist's name or description
///
/// PL01 (#154): renombrar o editar la descripción no toca la identidad
/// (`local_uuid`, `service_playlist_id` ni `account_id`). Una playlist con
/// tombstone ya no existe para la UI y tampoco se puede editar.
#[tauri::command]
pub async fn update_playlist(
    state: State<'_, AppState>,
    id: i64,
    name: Option<String>,
    description: Option<String>,
    is_public: Option<bool>,
) -> Result<Playlist, String> {
    update_playlist_core(&state.db, id, name, description, is_public).await
}

/// Núcleo testeable de `update_playlist`: solo toca `name` y `description`
/// (más `updated_at`), nunca columnas de identidad, y rechaza playlists con
/// tombstone (la relectura final ya filtra `deleted_at`).
pub async fn update_playlist_core(
    db: &sqlx::SqlitePool,
    id: i64,
    name: Option<String>,
    description: Option<String>,
    is_public: Option<bool>,
) -> Result<Playlist, String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    if let Some(new_name) = &name {
        sqlx::query(
            "UPDATE playlists SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(new_name)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update name: {}", e))?;
    }

    if let Some(new_desc) = &description {
        sqlx::query(
            "UPDATE playlists SET description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(new_desc)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update description: {}", e))?;
    }

    let _ = is_public; // Preserved for forward-compatibility

    crate::services::playlist_canonical::canonicalize_playlist_in_conn(&mut tx, id).await?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit update: {}", e))?;

    let updated = sqlx::query_as::<_, Playlist>(
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner_name,
            p.track_count,
            p.image_url,
            s.name as service_name
        FROM playlists p
        LEFT JOIN accounts a ON a.id = p.account_id
        LEFT JOIN services s ON s.id = a.service_id
        WHERE p.id = ? AND p.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Failed to get playlist: {}", e))?
    .ok_or_else(|| format!("Playlist {} not found after update", id))?;

    Ok(updated)
}

/// Delete a playlist and cascade delete its tracks associations
///
/// PL01 (#154): una playlist local (sin cuenta o smart) se borra de verdad:
/// es del usuario y no tiene contraparte remota que la resucite. Una
/// importada deja tombstone (`deleted_at`): sus pistas se borran, pero la
/// fila conserva su identidad remota (`account_id`, `service_playlist_id` y
/// su registro en `playlist_sources`) para que el siguiente sync, al
/// reencontrarla, no la recree (`upsert_playlist_and_source`).
#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    delete_playlist_core(&state.db, id).await
}

/// Núcleo testeable de `delete_playlist`: local se DELETE, importada deja
/// tombstone. El borrado local de la fila dispara el ON DELETE CASCADE de sus
/// `playlist_sources` (0002); el DELETE explícito deja el intento legible.
pub async fn delete_playlist_core(db: &sqlx::SqlitePool, id: i64) -> Result<(), String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let target: Option<(Option<String>, Option<i64>, i64)> = sqlx::query_as(
        "SELECT local_uuid, account_id, COALESCE(is_smart, 0) FROM playlists WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("Failed to get playlist: {}", e))?;

    let (local_uuid, account_id, is_smart) =
        target.ok_or_else(|| format!("Playlist {} not found", id))?;
    let is_local = local_uuid.is_some() || account_id.is_none() || is_smart != 0;

    // Las pistas dejan de existir en ambos casos: el contenido se va con la
    // playlist, sea física o tombstone.
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to delete playlist tracks: {}", e))?;

    if is_local {
        sqlx::query("DELETE FROM playlist_sources WHERE playlist_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to delete playlist sources: {}", e))?;

        let res = sqlx::query("DELETE FROM playlists WHERE id = ? AND deleted_at IS NULL")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to delete playlist: {}", e))?;

        if res.rows_affected() == 0 {
            return Err(format!("Playlist {} not found", id));
        }
    } else {
        let res = sqlx::query(
            "UPDATE playlists SET deleted_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, track_count = 0 WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to tombstone playlist: {}", e))?;

        if res.rows_affected() == 0 {
            return Err(format!("Playlist {} not found", id));
        }
    }

    crate::services::playlist_canonical::canonicalize_playlist_in_conn(&mut tx, id).await?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit delete: {}", e))?;

    Ok(())
}

/// Remove specific tracks from a playlist and compact positions
///
/// PL02 (#155): una entrada del payload quita UNA ocurrencia, no todas. Con la
/// misma pista repetida, cada repetición del payload consume la siguiente
/// ocurrencia de menor posición, de izquierda a derecha. Si el payload trae la
/// pista una sola vez, la lista conserva sus demás repeticiones.
#[tauri::command]
pub async fn remove_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<usize, String> {
    remove_from_playlist_core(&state.db, playlist_id, &track_ids).await
}

/// Núcleo testeable de `remove_from_playlist`: quita una ocurrencia por entrada
/// y deja las posiciones contiguas 1..N y `track_count` al día en la MISMA
/// transacción (si la recompactación fallara, el borrado no se confirma).
pub async fn remove_from_playlist_core(
    db: &sqlx::SqlitePool,
    playlist_id: i64,
    track_ids: &[i64],
) -> Result<usize, String> {
    if track_ids.is_empty() {
        return Ok(0);
    }

    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let mut removed = 0usize;
    for tid in track_ids {
        let res = sqlx::query(
            r#"
            DELETE FROM playlist_tracks
            WHERE id = (
                SELECT id FROM playlist_tracks
                WHERE playlist_id = ? AND track_id = ?
                ORDER BY position ASC, id ASC
                LIMIT 1
            )
            "#,
        )
        .bind(playlist_id)
        .bind(tid)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to remove track {}: {}", tid, e))?;

        removed += res.rows_affected() as usize;
    }

    recompact_playlist_tracks_in_conn(&mut tx, playlist_id).await?;
    crate::services::playlist_canonical::mark_playlist_source_expansion_incomplete_in_conn(
        &mut tx,
        playlist_id,
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit track removal: {}", e))?;

    Ok(removed)
}

/// TASK-107: Sanitization result statistics for playlists
///
/// PL00 (#153): las playlists admiten la misma pista repetida, así que la
/// sanitización ya no purga duplicados. `duplicate_tracks_purged` se conserva
/// por compatibilidad del contrato IPC y siempre vale 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSanitizationStats {
    pub duplicate_tracks_purged: usize,
    pub playlists_recompacted: usize,
    pub track_counts_updated: usize,
    pub playlist_names_disambiguated: usize,
}

/// TASK-107 / PL00: Transactionally sanitizes a single playlist:
/// 1. Recompactar position secuencialmente 1..N usando técnica segura contra colisiones transitorias UNIQUE (staging negativo).
/// 2. Sincronizar playlists.track_count = (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = playlists.id).
///
/// PL00 (#153): las playlists admiten la misma pista repetida, así que la
/// sanitización ya NO purga duplicados; conserva todas las ocurrencias y solo
/// recompacta posiciones. Devuelve el número de filas que quedaron tras la
/// recompactación (0 en una playlist vacía).
pub async fn sanitize_single_playlist(
    pool: &sqlx::SqlitePool,
    playlist_id: i64,
) -> Result<usize, String> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.map_err(|e| {
        format!(
            "Failed to begin transaction for playlist sanitization: {}",
            e
        )
    })?;

    let remaining = recompact_playlist_tracks_in_conn(&mut tx, playlist_id).await?;
    crate::services::playlist_canonical::canonicalize_playlist_in_conn(&mut tx, playlist_id)
        .await?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist sanitization: {}", e))?;

    Ok(remaining)
}

/// Recompacta las pistas de UNA playlist dentro de una conexión/transacción ya
/// abierta: posiciones contiguas 1..N en el orden actual, y
/// `playlists.track_count` = número de OCURRENCIAS.
///
/// PL02 (#155): es el único sitio que asigna posiciones canónicas, para que
/// quitar una ocurrencia y reordenar compartan la misma regla sin abrir
/// transacciones anidadas. El staging negativo evita colisiones transitorias
/// con `UNIQUE(playlist_id, position)` mientras se reasigna.
pub async fn recompact_playlist_tracks_in_conn(
    conn: &mut sqlx::SqliteConnection,
    playlist_id: i64,
) -> Result<usize, String> {
    // 1. Fetch tracks ordered by current position ASC, and added_at ASC, id ASC as tie-breakers
    let remaining: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC, added_at ASC, id ASC",
    )
    .bind(playlist_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| format!("Failed to fetch playlist tracks for recompact: {}", e))?;

    // 2. Stage existing positions to unique negative values to avoid UNIQUE(playlist_id, position) collisions
    sqlx::query("UPDATE playlist_tracks SET position = -(id + 1) WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("Failed to stage playlist track positions: {}", e))?;

    // 3. Sequentially assign 1-indexed positions (1, 2, 3... N)
    for (idx, (row_id,)) in remaining.iter().enumerate() {
        let canonical_pos = (idx + 1) as i64;
        sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?")
            .bind(canonical_pos)
            .bind(row_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Failed to reassign playlist track position: {}", e))?;
    }

    // 4. Atomically update track_count in playlists table to match exact COUNT(*)
    sqlx::query(
        "UPDATE playlists SET track_count = (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?) WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("Failed to update playlists.track_count: {}", e))?;

    Ok(remaining.len())
}

/// Aud23: recalcula `playlists.track_count` para las playlists cuyo contador o
/// cuyas posiciones dejaron de cuadrar, sin tocar las que ya están bien.
///
/// `playlist_tracks.track_id` es `ON DELETE CASCADE` (migración 0064), así que
/// borrar una pista de la biblioteca borra en cascada su fila de
/// `playlist_tracks`… pero SQLite no dispara ningún trigger sobre `playlists`
/// y el contador que la UI lee en `get_playlists` (`p.track_count`) se queda
/// congelado. La cascada además deja huecos en `position` (1, 3, 4…), que es
/// la otra mitad del mismo defecto: por eso delega en `sanitize_single_playlist`,
/// que deduplica, recompacta 1..N y sincroniza el contador en una transacción.
///
/// Devuelve cuántas playlists corrigió.
pub async fn reconcile_playlist_track_counts(pool: &sqlx::SqlitePool) -> Result<usize, String> {
    let stale: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT p.id
        FROM playlists p
        LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
        GROUP BY p.id
        HAVING p.track_count != COUNT(pt.id)
            OR COALESCE(MAX(pt.position), 0) != COUNT(pt.id)
            OR COALESCE(MIN(pt.position), 1) != 1
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to list playlists with a stale track_count: {}", e))?;

    let mut reconciled = 0usize;
    for (playlist_id,) in stale {
        sanitize_single_playlist(pool, playlist_id).await?;
        reconciled += 1;
    }

    Ok(reconciled)
}

/// TASK-79 & TASK-107: Recompact playlist positions to be strictly 1-indexed, sequential, and gap-free (1, 2, 3... N).
/// Atomically purges duplicate tracks within the playlist, recompacts positions, and reconciles `playlists.track_count`.
pub async fn recompact_playlist_positions(
    pool: &sqlx::SqlitePool,
    playlist_id: i64,
) -> Result<(), String> {
    sanitize_single_playlist(pool, playlist_id)
        .await
        .map(|_| ())
}

/// TASK-107: Transactionally sanitizes all playlists across the library:
/// 1. Recompactar position a secuencia contigua 1..N sin huecos para todas las playlists.
/// 2. Sincronizar playlists.track_count con el conteo real en playlist_tracks.
/// 3. Desambiguar colisiones de nombres de playlists bajo la misma cuenta (account_id, LOWER(TRIM(name))).
///
/// PL00: ya no purga duplicados (las repeticiones del mismo track son legales).
pub async fn sanitize_playlists_in_pool(
    pool: &sqlx::SqlitePool,
) -> Result<PlaylistSanitizationStats, String> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.map_err(|e| {
        format!(
            "Failed to begin transaction for global playlist sanitization: {}",
            e
        )
    })?;

    let duplicate_tracks_purged: usize = 0;

    // 1. Recompactar position a secuencia contigua 1..N sin huecos
    // 1a. Crear staging temporal con nueva posición 1-indexed
    sqlx::query("DROP TABLE IF EXISTS _playlist_tracks_recompact")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to drop temp table: {}", e))?;

    sqlx::query(
        r#"
        CREATE TEMP TABLE _playlist_tracks_recompact (
            id INTEGER PRIMARY KEY,
            new_pos INTEGER NOT NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to create temp recompact table: {}", e))?;

    sqlx::query(
        r#"
        INSERT INTO _playlist_tracks_recompact (id, new_pos)
        SELECT
            id,
            ROW_NUMBER() OVER (
                PARTITION BY playlist_id
                ORDER BY position ASC, added_at ASC, id ASC
            )
        FROM playlist_tracks
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to populate temp recompact table: {}", e))?;

    // 1b. Staging negativo de todas las posiciones para evitar colisiones UNIQUE(playlist_id, position)
    sqlx::query("UPDATE playlist_tracks SET position = -(id + 1)")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to stage playlist positions to negative: {}", e))?;

    // 1c. Aplicar nuevas posiciones 1-indexed desde la tabla staging
    sqlx::query(
        r#"
        UPDATE playlist_tracks
        SET position = (
            SELECT r.new_pos
            FROM _playlist_tracks_recompact r
            WHERE r.id = playlist_tracks.id
        )
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update recompacted positions: {}", e))?;

    sqlx::query("DROP TABLE IF EXISTS _playlist_tracks_recompact")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to drop temp recompact table: {}", e))?;

    // 2. Sincronizar track_count de todas las playlists
    let count_res = sqlx::query(
        r#"
        UPDATE playlists
        SET track_count = (
            SELECT COUNT(*)
            FROM playlist_tracks
            WHERE playlist_tracks.playlist_id = playlists.id
        )
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update playlists track_count: {}", e))?;

    let track_counts_updated = count_res.rows_affected() as usize;

    // 3. Recalcular canónicas SIN certificar expansiones: solo una ruta que
    // recorrió todas las páginas del proveedor puede marcar `expansion_complete`.
    // La sanitización local debe conservar NULL/0 y no fusionar datos a medias.
    let canonicalization =
        crate::services::playlist_canonical::canonicalize_playlists_in_conn(&mut tx).await?;
    tracing::info!(
        canonicals = canonicalization.canonicals,
        merged = canonicalization.merged_canonicals,
        incomplete = canonicalization.incomplete_playlists,
        "playlist canonicalization completed"
    );

    // 4. Desambiguar nombres bajo la misma cuenta, pero NO separar dos copias
    // que ya comparten canónica. El contenido distinto sí recibe sufijo.
    // PL01 (#154): los tombstones no participan.
    let dup_name_groups: Vec<(i64, String, i64)> = sqlx::query_as(
        r#"
        SELECT account_id, LOWER(TRIM(name)) as norm_name, COUNT(*) as cnt
        FROM playlists
        WHERE deleted_at IS NULL
        GROUP BY account_id, LOWER(TRIM(name))
        HAVING cnt > 1
        ORDER BY account_id, norm_name
        "#,
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("Failed to query duplicate playlist names: {}", e))?;

    let mut playlist_names_disambiguated = 0usize;

    for (acc_id, norm_name, _) in dup_name_groups {
        let pls: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
            r#"
            SELECT p.id, p.name, m.canonical_id
            FROM playlists p
            LEFT JOIN playlist_canonical_members m ON m.playlist_id = p.id
            WHERE p.account_id = ? AND LOWER(TRIM(p.name)) = ? AND p.deleted_at IS NULL
            ORDER BY p.id ASC
            "#,
        )
        .bind(acc_id)
        .bind(&norm_name)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("Failed to fetch playlists for group '{}': {}", norm_name, e))?;

        let existing_names: Vec<(String,)> = sqlx::query_as(
            "SELECT LOWER(TRIM(name)) FROM playlists WHERE account_id = ? AND deleted_at IS NULL",
        )
        .bind(acc_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| {
            format!(
                "Failed to fetch existing playlist names for account {}: {}",
                acc_id, e
            )
        })?;

        let mut existing_set: std::collections::HashSet<String> =
            existing_names.into_iter().map(|(n,)| n).collect();
        let mut group_indexes: std::collections::HashMap<i64, usize> =
            std::collections::HashMap::new();
        let mut groups: Vec<Vec<(i64, String)>> = Vec::new();
        for (pid, orig_name, canonical_id) in pls {
            // Sin canónica, cada playlist es su propia identidad: conserva el
            // comportamiento histórico de desambiguar por fila.
            let identity = canonical_id.unwrap_or(-pid);
            let group_index = match group_indexes.get(&identity) {
                Some(index) => *index,
                None => {
                    let index = groups.len();
                    groups.push(Vec::new());
                    group_indexes.insert(identity, index);
                    index
                }
            };
            groups[group_index].push((pid, orig_name));
        }

        let mut suffix = 2i64;
        for (group_index, members) in groups.into_iter().enumerate() {
            if group_index == 0 {
                continue;
            }
            let new_name = loop {
                let candidate = format!("{} ({})", members[0].1.trim(), suffix);
                suffix += 1;
                if !existing_set.contains(&candidate.trim().to_lowercase()) {
                    break candidate;
                }
            };
            existing_set.insert(new_name.trim().to_lowercase());

            // Todas las copias exactas reciben el mismo nombre; renombrar solo
            // una las separaría de su grupo canónico al recalcular la huella.
            for (pid, _) in members {
                sqlx::query(
                    "UPDATE playlists SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(&new_name)
                .bind(pid)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to update disambiguated playlist name for id {}: {}",
                        pid, e
                    )
                })?;

                crate::services::playlist_canonical::canonicalize_playlist_in_conn(&mut tx, pid)
                    .await?;
                playlist_names_disambiguated += 1;
            }
        }
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit global playlist sanitization: {}", e))?;

    let pls_with_tracks: (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT playlist_id) FROM playlist_tracks")
            .fetch_one(pool)
            .await
            .unwrap_or((0,));

    Ok(PlaylistSanitizationStats {
        duplicate_tracks_purged,
        playlists_recompacted: pls_with_tracks.0 as usize,
        track_counts_updated,
        playlist_names_disambiguated,
    })
}

/// Tauri command to sanitize all playlists across the library (TASK-107).
#[tauri::command]
pub async fn sanitize_playlists(
    state: State<'_, AppState>,
) -> Result<PlaylistSanitizationStats, String> {
    sanitize_playlists_in_pool(&state.db).await
}

/// Reorder tracks in a playlist given target positions
///
/// PL02 (#155): el payload es el orden COMPLETO deseado (una entrada por
/// ocurrencia, no por pista). Con la misma pista repetida, la k-ésima entrada
/// de esa pista en el payload mueve la k-ésima ocurrencia en el orden actual;
/// así dos repeticiones nunca se pisan ni se pierde el destino pedido.
#[tauri::command]
pub async fn reorder_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: i64,
    positions: Vec<PlaylistTrackPosition>,
) -> Result<(), String> {
    reorder_playlist_tracks_core(&state.db, playlist_id, &positions).await
}

/// Núcleo testeable de `reorder_playlist_tracks`.
///
/// Mapea cada entrada del payload a una fila concreta de `playlist_tracks`
/// (no a un `track_id`, que puede repetirse), escribe las posiciones canónicas
/// 0..n-1 en ese orden —1..N en la base, la secuencia contigua que fija 0110—
/// y sincroniza `track_count` con el número de ocurrencias. Las ocurrencias que
/// el payload no menciona conservan su orden relativo al final, de modo que un
/// payload parcial (mover una pista) sigue siendo válido.
pub async fn reorder_playlist_tracks_core(
    db: &sqlx::SqlitePool,
    playlist_id: i64,
    positions: &[PlaylistTrackPosition],
) -> Result<(), String> {
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    // Fila y pista de cada ocurrencia, en el orden actual de la playlist.
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, track_id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC, id ASC",
    )
    .bind(playlist_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("Failed to read playlist occurrences: {}", e))?;

    // Ocurrencias pendientes por pista, en orden de playlist: pop_front consume
    // la repetición siguiente de esa pista.
    let mut pending: std::collections::HashMap<i64, std::collections::VecDeque<i64>> =
        std::collections::HashMap::new();
    for (row_id, track_id) in &rows {
        pending.entry(*track_id).or_default().push_back(*row_id);
    }

    // `new_position` ordena el payload; el orden de entrada desempata (sort
    // estable), que es como la UI envía el orden final.
    let mut wanted: Vec<&PlaylistTrackPosition> = positions.iter().collect();
    wanted.sort_by_key(|item| item.new_position);

    let mut desired: Vec<i64> = Vec::with_capacity(rows.len());
    let mut moved: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for item in wanted {
        let row_id = pending
            .get_mut(&item.track_id)
            .and_then(|queue| queue.pop_front())
            .ok_or_else(|| {
                format!(
                    "Track {} has no remaining occurrence in playlist {}",
                    item.track_id, playlist_id
                )
            })?;
        moved.insert(row_id);
        desired.push(row_id);
    }

    // Lo que el payload no menciona se queda detrás, en su orden actual.
    for (row_id, _) in &rows {
        if !moved.contains(row_id) {
            desired.push(*row_id);
        }
    }

    // Staging negativo → asignación canónica 1..N en una sola pasada.
    sqlx::query("UPDATE playlist_tracks SET position = -(id + 1) WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to stage playlist reordering: {}", e))?;

    for (index, row_id) in desired.iter().enumerate() {
        sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?")
            .bind(index as i64 + 1)
            .bind(row_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to assign playlist position: {}", e))?;
    }

    sqlx::query(
        "UPDATE playlists SET track_count = (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?), updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update playlist track count: {}", e))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit reordering: {}", e))?;

    Ok(())
}

/// Sync playlists across connected services (Tidal, Qobuz, Spotify) into SQLite
#[tauri::command]
pub async fn sync_playlists(
    state: State<'_, AppState>,
    service: Option<String>,
) -> Result<SyncPlaylistsResult, String> {
    // S189-F2-5: lectura agregada REAL de la tabla playlists multi-servicio
    // (antes era un stub que contaba y presentaba el conteo como «sync»).
    // El alta/actualización contra proveedores vive en perform_sync_service;
    // este comando reporta el catálogo local enlazado, por servicio.
    let target_service = service.unwrap_or_else(|| "all".to_string());
    let filter_specific = !target_service.eq_ignore_ascii_case("all");

    let rows: Vec<(String, i64, i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT s.name,
               COUNT(DISTINCT p.id),
               COUNT(pt.id),
               MAX(p.last_synced)
        FROM playlists p
        JOIN accounts a ON a.id = p.account_id
        JOIN services s ON s.id = a.service_id
        LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
        WHERE a.is_active = 1
          AND p.deleted_at IS NULL
          AND (? = 'all' OR LOWER(s.name) = LOWER(?))
        GROUP BY s.name
        ORDER BY s.name
        "#,
    )
    .bind(if filter_specific {
        target_service.as_str()
    } else {
        "all"
    })
    .bind(target_service.as_str())
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to aggregate playlists: {}", e))?;

    let services: Vec<PlaylistServiceSummary> = rows
        .into_iter()
        .map(
            |(name, playlists, tracks, last_synced)| PlaylistServiceSummary {
                service: name,
                playlists,
                tracks_linked: tracks,
                last_synced,
            },
        )
        .collect();

    let total_playlists: i64 = services.iter().map(|s| s.playlists).sum();
    let total_tracks: i64 = services.iter().map(|s| s.tracks_linked).sum();
    let service_names: Vec<String> = services.iter().map(|s| s.service.clone()).collect();

    let message = format!(
        "Catálogo local: {} playlists con {} pistas enlazadas ({})",
        total_playlists,
        total_tracks,
        if service_names.is_empty() {
            "sin servicios con playlists".to_string()
        } else {
            service_names.join(", ")
        }
    );

    Ok(SyncPlaylistsResult {
        playlists_synced: total_playlists,
        tracks_linked: total_tracks,
        message,
        services,
    })
}

// ==============================================
// S201 - MODO A: EXPORT M3U «SOLO LAS QUE YA TENGO»
// ==============================================

/// Una pista de la playlist con los datos mínimos para el M3U.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct PlaylistM3uEntry {
    pub track_id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub file_path: Option<String>,
}

/// Pista que NO pudo verificarse en disco (para la lista de faltantes en UI).
#[derive(Debug, Clone, serde::Serialize)]
pub struct MissingPlaylistFile {
    pub track_id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    /// `sin_archivo_local` (sin fila en downloads) | `archivo_no_encontrado` (stat falló)
    pub reason: String,
}

/// Resultado honesto del export Modo A: conteos reales + contenido M3U.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistM3uExportResult {
    pub playlist_id: i64,
    pub playlist_name: String,
    /// Pistas totales de la playlist.
    pub total_tracks: usize,
    /// Pistas cuyo archivo local fue verificado con stat() real.
    pub verified_count: usize,
    pub missing_count: usize,
    pub missing_tracks: Vec<MissingPlaylistFile>,
    /// Ruta escrita (None si solo se pidió el contenido).
    pub file_path: Option<String>,
    pub bytes_written: Option<u64>,
    /// Contenido generado (solo pistas verificadas), paridad CLI:
    /// `#EXTM3U` + `#EXTINF:<segundos>,<Artista - Título>` + ruta absoluta.
    pub m3u_content: String,
}

/// Lee las pistas de la playlist (orden de posición) con su file_path efectivo.
async fn fetch_playlist_m3u_entries(
    db: &sqlx::SqlitePool,
    playlist_id: i64,
) -> Result<Vec<PlaylistM3uEntry>, String> {
    sqlx::query_as::<_, PlaylistM3uEntry>(
        r#"
        SELECT
            t.id as track_id,
            t.title,
            (SELECT a2.name FROM track_artists ta2
             JOIN artists a2 ON a2.id = ta2.artist_id
             WHERE ta2.track_id = t.id AND ta2.role = 'primary'
             LIMIT 1) as artist_name,
            t.duration_ms,
            t.isrc,
            d.file_path
        FROM playlist_tracks pt
        INNER JOIN tracks t ON t.id = pt.track_id
        LEFT JOIN downloads d ON d.track_id = t.id
        WHERE pt.playlist_id = ?
        ORDER BY pt.position ASC, t.id ASC
        "#,
    )
    .bind(playlist_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Formato estándar M3U (paridad con scripts/playlist_bridge.py export --format m3u):
/// `#EXTM3U`, una línea `#EXTINF:<segundos>,<Artista - Título>` por pista
/// seguida de su ruta absoluta. Solo recibe pistas ya verificadas.
pub fn build_m3u_content(tracks: &[PlaylistM3uEntry]) -> String {
    let mut lines: Vec<String> = vec!["#EXTM3U".to_string()];
    for t in tracks {
        let secs = (t.duration_ms.unwrap_or(0)).max(0) / 1000;
        let artist = t
            .artist_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown");
        lines.push(format!("#EXTINF:{},{} - {}", secs, artist, t.title));
        if let Some(path) = &t.file_path {
            lines.push(path.clone());
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Verificación REAL en disco (stat por pista). Bloqueante: llamar desde
/// spawn_blocking. Devuelve (verificadas, faltantes, contenido m3u).
pub fn verify_playlist_files_for_m3u(
    entries: Vec<PlaylistM3uEntry>,
) -> (Vec<PlaylistM3uEntry>, Vec<MissingPlaylistFile>, String) {
    let mut verified: Vec<PlaylistM3uEntry> = Vec::new();
    let mut missing: Vec<MissingPlaylistFile> = Vec::new();

    for e in entries {
        match e.file_path.as_deref() {
            None => missing.push(MissingPlaylistFile {
                track_id: e.track_id,
                title: e.title.clone(),
                artist_name: e.artist_name.clone(),
                reason: "sin_archivo_local".to_string(),
            }),
            Some(path) => {
                let exists = std::fs::metadata(path)
                    .map(|m| m.is_file())
                    .unwrap_or(false);
                if exists {
                    verified.push(e);
                } else {
                    missing.push(MissingPlaylistFile {
                        track_id: e.track_id,
                        title: e.title.clone(),
                        artist_name: e.artist_name.clone(),
                        reason: "archivo_no_encontrado".to_string(),
                    });
                }
            }
        }
    }

    let content = build_m3u_content(&verified);
    (verified, missing, content)
}

/// Allowed extensions for M3U playlist files.
pub const ALLOWED_M3U_EXTENSIONS: &[&str] = &["m3u", "m3u8"];

/// Returns the set of allowed base directories for M3U export persistence.
/// Strictly confined to the user's Music/Audio, Downloads, Documents, and app data directory.
pub fn get_allowed_m3u_directories() -> Vec<std::path::PathBuf> {
    let mut bases = Vec::new();

    if let Some(audio) = dirs::audio_dir() {
        if let Ok(canon) = std::fs::canonicalize(&audio) {
            bases.push(canon);
        }
        bases.push(audio);
    }

    if let Some(home) = dirs::home_dir() {
        let music = home.join("Music");
        if let Ok(canon) = std::fs::canonicalize(&music) {
            bases.push(canon);
        }
        bases.push(music);
    }

    if let Some(download) = dirs::download_dir() {
        if let Ok(canon) = std::fs::canonicalize(&download) {
            bases.push(canon);
        }
        bases.push(download);
    }

    if let Some(doc) = dirs::document_dir() {
        if let Ok(canon) = std::fs::canonicalize(&doc) {
            bases.push(canon);
        }
        bases.push(doc);
    }

    if let Some(data_local) = dirs::data_local_dir() {
        let app_dir = data_local.join("com.syncify.app");
        if let Ok(canon) = std::fs::canonicalize(&app_dir) {
            bases.push(canon);
        }
        bases.push(app_dir);
    }

    if let Some(data) = dirs::data_dir() {
        let app_dir = data.join("com.syncify.app");
        if let Ok(canon) = std::fs::canonicalize(&app_dir) {
            bases.push(canon);
        }
        bases.push(app_dir);
    }

    bases.sort();
    bases.dedup();
    bases
}

/// Validates that an M3U export path conforms to sandbox confinement, path traversal
/// restrictions, and file extension whitelisting (.m3u / .m3u8).
pub fn validate_safe_m3u_write_path_with_bases(
    target_path: &std::path::Path,
    allowed_bases: &[std::path::PathBuf],
) -> Result<std::path::PathBuf, String> {
    // 1. Must be an absolute path
    if !target_path.is_absolute() {
        return Err("Acceso denegado: la ruta debe ser absoluta (sandbox violation)".to_string());
    }

    // 2. Reject path traversal sequences (.. or ParentDir)
    for component in target_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(
                "Acceso denegado: secuencias de escape ('..') detectadas (sandbox violation)"
                    .to_string(),
            );
        }
    }

    // 3. Reject hidden files
    let file_name = target_path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| {
            "Acceso denegado: nombre de archivo no válido (sandbox violation)".to_string()
        })?;

    if file_name.starts_with('.') {
        return Err("Acceso denegado: no se permite escribir archivos ocultos o de configuración (sandbox violation)".to_string());
    }

    // 4. Strict extension check: .m3u or .m3u8 (case-insensitive)
    let ext = target_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let ext_str = match &ext {
        Some(e) => e.as_str(),
        None => {
            return Err("Acceso denegado: el archivo debe tener extensión obligatoria .m3u o .m3u8 (sandbox violation)".to_string());
        }
    };

    if !ALLOWED_M3U_EXTENSIONS.contains(&ext_str) {
        return Err(format!(
            "Acceso denegado: extensión '.{}' no permitida. Solo se permite .m3u o .m3u8 (sandbox violation)",
            ext_str
        ));
    }

    // 5. Defense in depth: reject sensitive system directories
    let path_str = target_path.to_string_lossy();
    if path_str.starts_with("/etc")
        || path_str.starts_with("/proc")
        || path_str.starts_with("/sys")
        || path_str.starts_with("/dev")
        || path_str.starts_with("/var")
        || path_str.contains("/.ssh")
        || path_str.contains("/.gnupg")
        || path_str.contains("/.aws")
    {
        return Err(
            "Acceso denegado: ruta en directorio protegido del sistema (sandbox violation)"
                .to_string(),
        );
    }

    if allowed_bases.is_empty() {
        return Err(
            "Acceso denegado: no se definieron directorios base permitidos (sandbox violation)"
                .to_string(),
        );
    }

    // 6. Lexical containment check against allowed bases
    let matches_lexical = allowed_bases
        .iter()
        .any(|base| target_path.starts_with(base));
    if !matches_lexical {
        return Err(
            "Acceso denegado: la ruta está fuera de los directorios permitidos (sandbox violation)"
                .to_string(),
        );
    }

    // 7. Parent directory resolution and creation
    let parent = target_path.parent().ok_or_else(|| {
        "Acceso denegado: ruta sin directorio padre válido (sandbox violation)".to_string()
    })?;

    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear el directorio {}: {}", parent.display(), e))?;
    }

    // 8. Canonicalize parent directory and verify containment
    let canonical_parent = std::fs::canonicalize(parent).map_err(|e| {
        format!(
            "Error al canonicalizar directorio {}: {}",
            parent.display(),
            e
        )
    })?;

    let mut canonical_allowed_bases = Vec::new();
    for b in allowed_bases {
        if let Ok(c) = std::fs::canonicalize(b) {
            canonical_allowed_bases.push(c);
        }
        canonical_allowed_bases.push(b.clone());
    }

    if !canonical_allowed_bases
        .iter()
        .any(|base| canonical_parent.starts_with(base))
    {
        return Err("Acceso denegado: el directorio destino canonicalizado está fuera del sandbox permitido (sandbox violation)".to_string());
    }

    let safe_target = canonical_parent.join(file_name);

    // 9. Prevent symlink overwriting or escaping via existing symlinks
    if safe_target.is_symlink()
        || std::fs::symlink_metadata(&safe_target)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(
            "Acceso denegado: no se permite sobreescribir enlaces simbólicos (sandbox violation)"
                .to_string(),
        );
    }

    if safe_target.exists() {
        let canonical_target = std::fs::canonicalize(&safe_target)
            .map_err(|e| format!("Error al canonicalizar archivo existente: {}", e))?;
        if !canonical_allowed_bases
            .iter()
            .any(|base| canonical_target.starts_with(base))
        {
            return Err("Acceso denegado: el archivo destino existente resuelve fuera del sandbox permitido (sandbox violation)".to_string());
        }
    }

    Ok(safe_target)
}

/// Helper to validate an M3U export path against default allowed directories.
pub fn validate_safe_m3u_write_path(
    target_path: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let allowed_bases = get_allowed_m3u_directories();
    validate_safe_m3u_write_path_with_bases(target_path, &allowed_bases)
}

/// Escritura de M3U en disco confinado a directorios permitidos (Música, Descargas, Documentos, App Data).
pub fn write_m3u_to_disk(path: &str, contents: &str) -> Result<u64, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(
            "Acceso denegado: la ruta no puede estar vacía (sandbox violation)".to_string(),
        );
    }
    let target = std::path::Path::new(trimmed);
    let safe_target = validate_safe_m3u_write_path(target)?;

    std::fs::write(&safe_target, contents)
        .map_err(|e| format!("No se pudo escribir {}: {}", safe_target.display(), e))?;
    Ok(contents.len() as u64)
}

/// Núcleo testeable del export Modo A: verifica archivos reales y, si se da
/// `file_path`, escribe el .m3u. Toda la IO de disco corre en spawn_blocking.
pub async fn export_playlist_m3u_core(
    db: &sqlx::SqlitePool,
    playlist_id: i64,
    file_path: Option<String>,
) -> Result<PlaylistM3uExportResult, String> {
    let name_row: Option<(String,)> = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    let playlist_name = name_row
        .map(|(n,)| n)
        .ok_or_else(|| format!("Playlist {} not found", playlist_id))?;

    let entries = fetch_playlist_m3u_entries(db, playlist_id).await?;
    let total_tracks = entries.len();

    // stat() de cada archivo + render del contenido: fuera del runtime async.
    let (verified, missing, content) =
        tokio::task::spawn_blocking(move || verify_playlist_files_for_m3u(entries))
            .await
            .map_err(|e| format!("Error verifying local files: {}", e))?;

    let target = file_path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());

    if verified.is_empty() && target.is_some() {
        return Err(
            "Ninguna pista tiene un archivo local verificado: no se escribió el .m3u \
             (usa «Descargar las pistas faltantes» para obtenerlas)"
                .to_string(),
        );
    }

    let mut bytes_written = None;
    let mut written_path = None;
    if let Some(path) = target {
        let path_for_result = path.clone();
        let content_for_write = content.clone();
        let bytes =
            tokio::task::spawn_blocking(move || write_m3u_to_disk(&path, &content_for_write))
                .await
                .map_err(|e| format!("Error writing M3U file: {}", e))??;
        tracing::info!(
            "export_playlist_m3u: {} pistas verificadas -> {} ({} bytes)",
            verified.len(),
            path_for_result,
            bytes
        );
        bytes_written = Some(bytes);
        written_path = Some(path_for_result);
    }

    Ok(PlaylistM3uExportResult {
        playlist_id,
        playlist_name,
        total_tracks,
        verified_count: verified.len(),
        missing_count: missing.len(),
        missing_tracks: missing,
        file_path: written_path,
        bytes_written,
        m3u_content: content,
    })
}

/// S201 Modo A «Solo las que ya tengo»: verifica los archivos locales de las
/// pistas de la playlist (stat real, sin red) y exporta un .m3u con SOLO las
/// verificadas. Devuelve conteos honestos {total, verified, missing} y la
/// lista de faltantes para mostrarlos en UI. Con `file_path = None` devuelve
/// solo el contenido/conteos (dry-run).
#[tauri::command]
pub async fn export_playlist_m3u(
    state: State<'_, AppState>,
    playlist_id: i64,
    file_path: Option<String>,
) -> Result<PlaylistM3uExportResult, String> {
    export_playlist_m3u_core(&state.db, playlist_id, file_path).await
}

// ============================================================================
// FE-6 — IMPORT DE PLAYLISTS DESDE ARCHIVO (.m3u/.m3u8, .csv, .txt)
// ============================================================================

/// Extensiones de audio reconocidas dentro de un M3U (paridad con scripts/scanner_bridge.py AUDIO_EXTENSIONS).
pub const AUDIO_FILE_EXTENSIONS: &[&str] =
    &["mp3", "flac", "m4a", "wav", "ogg", "aac", "wma", "opus"];

/// Entrada parseada del archivo de playlist, antes del matching con la biblioteca.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedPlaylistEntry {
    pub title: String,
    pub artist: Option<String>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub file_path: Option<String>,
}

/// Pista parseada que no pudo enlazarse a ninguna pista existente de la biblioteca.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnmatchedImportedEntry {
    pub title: String,
    pub artist: Option<String>,
}

/// Resultado honesto del import: playlist creada, conteos reales y faltantes.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPlaylistFromFileResult {
    pub playlist_id: i64,
    pub playlist_name: String,
    /// Entradas totales parseadas del archivo.
    pub total_entries: usize,
    /// Entradas enlazadas a pistas existentes (las repeticiones cuentan).
    pub matched_count: usize,
    pub unmatched: Vec<UnmatchedImportedEntry>,
}

/// Divide "Artista - Título" en la PRIMERA ocurrencia de " - " (paridad con
/// scripts/playlist_bridge.py parse_m3u_file). Devuelve (artista, título).
fn split_artist_title(line: &str) -> (Option<String>, String) {
    match line.split_once(" - ") {
        Some((artist, title)) => {
            let artist = artist.trim();
            let title = title.trim();
            if artist.is_empty() || title.is_empty() {
                (None, line.trim().to_string())
            } else {
                (Some(artist.to_string()), title.to_string())
            }
        }
        None => (None, line.trim().to_string()),
    }
}

/// True si la línea parece una ruta/nombre de archivo de audio (por extensión).
fn looks_like_audio_path(line: &str) -> bool {
    let lowered = line.trim().to_lowercase();
    AUDIO_FILE_EXTENSIONS
        .iter()
        .any(|ext| lowered.rsplit('.').next() == Some(*ext) && lowered.contains('.'))
}

/// Normaliza un ISRC para comparación (paridad con migration 0064):
/// mayúsculas sin guiones ni espacios.
fn normalize_isrc(isrc: &str) -> String {
    isrc.trim().to_uppercase().replace(['-', ' '], "")
}

/// Parsea el contenido de un .m3u/.m3u8: cabecera `#EXTM3U`, directivas
/// `#EXTINF:<segundos>,<Artista - Título>` (+ `# ISRC:` opcional, paridad CLI)
/// seguidas de la ruta del archivo, o líneas sueltas "Artista - Título"/título.
pub fn parse_m3u_content(content: &str) -> Vec<ParsedPlaylistEntry> {
    let mut entries: Vec<ParsedPlaylistEntry> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_artist: Option<String> = None;
    let mut current_duration_ms: Option<i64> = None;
    let mut current_isrc: Option<String> = None;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(extinf) = line
            .strip_prefix("#EXTINF:")
            .or_else(|| line.strip_prefix("#extinf:"))
        {
            // Formato: #EXTINF:<segundos>,<Artista - Título>
            let (secs_part, label) = match extinf.split_once(',') {
                Some((s, l)) => (s.trim(), l.trim()),
                None => (extinf.trim(), ""),
            };
            current_duration_ms = secs_part
                .parse::<i64>()
                .ok()
                .map(|secs| (secs.max(0)) * 1000);
            let (artist, title) = split_artist_title(label);
            current_artist = artist;
            current_title = if title.is_empty() { None } else { Some(title) };
        } else if let Some(isrc) = line
            .strip_prefix("# ISRC:")
            .or_else(|| line.strip_prefix("#ISRC:"))
        {
            let norm = isrc.trim();
            if !norm.is_empty() {
                current_isrc = Some(norm.to_string());
            }
        } else if line.starts_with('#') {
            // Otra directiva o comentario M3U: ignorar.
            continue;
        } else if looks_like_audio_path(line) {
            // Línea de ruta de archivo: usa los metadatos del #EXTINF previo.
            entries.push(ParsedPlaylistEntry {
                title: current_title.take().unwrap_or_else(|| line.to_string()),
                artist: current_artist.take(),
                duration_ms: current_duration_ms.take(),
                isrc: current_isrc.take(),
                file_path: Some(line.to_string()),
            });
        } else {
            // Línea "Artista - Título" o título suelto (M3U sin rutas).
            // Si había un #EXTINF pendiente sin ruta, se emite como entrada propia.
            if let Some(pending_title) = current_title.take() {
                entries.push(ParsedPlaylistEntry {
                    title: pending_title,
                    artist: current_artist.take(),
                    duration_ms: current_duration_ms.take(),
                    isrc: current_isrc.take(),
                    file_path: None,
                });
            }
            let (artist, title) = split_artist_title(line);
            if !title.is_empty() {
                entries.push(ParsedPlaylistEntry {
                    title,
                    artist,
                    duration_ms: None,
                    isrc: None,
                    file_path: None,
                });
            }
            current_title = None;
            current_artist = None;
        }
    }

    // Flush final: un #EXTINF sin línea de ruta al final del archivo.
    if let Some(pending_title) = current_title.take() {
        entries.push(ParsedPlaylistEntry {
            title: pending_title,
            artist: current_artist.take(),
            duration_ms: current_duration_ms.take(),
            isrc: current_isrc.take(),
            file_path: None,
        });
    }

    entries
}

/// Divide una línea CSV en campos respetando comillas dobles ("a,b" es un campo).
fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(c);
            }
        } else if c == '"' && current.is_empty() {
            in_quotes = true;
        } else if c == ',' {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// Normaliza una cabecera CSV para mapeo de columnas: minúsculas, cualquier
/// secuencia de no-alfanuméricos colapsa a '_' y se recortan los extremos.
/// "Duration (ms)" -> "duration_ms", "Artist Name" -> "artist_name".
fn norm_header(field: &str) -> String {
    let lowered = field.trim().to_lowercase();
    let mut out = String::with_capacity(lowered.len());
    let mut last_underscore = false;
    for c in lowered.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_underscore = false;
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Parsea el contenido de un .csv. Si la primera fila es una cabecera
/// (contiene title/track/song/artist/...) mapea columnas por nombre
/// (title|track|song|name, artist|author, isrc, duration|duration_ms|duration_s,
/// path|file|file_path|location); si no, asume `title,artist` (formato
/// "Title,Artist" de exportaciones habituales).
pub fn parse_csv_content(content: &str) -> Vec<ParsedPlaylistEntry> {
    let rows: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    if rows.is_empty() {
        return Vec::new();
    }

    // Detección de cabecera: nombres conocidos en la primera fila.
    let first_fields = split_csv_line(rows[0]);
    let header_keys: Vec<String> = first_fields.iter().map(|f| norm_header(f)).collect();
    let known = [
        "title",
        "track",
        "track_name",
        "song",
        "name",
        "artist",
        "artist_name",
        "author",
        "isrc",
        "duration",
        "duration_ms",
        "duration_s",
        "path",
        "file",
        "file_path",
        "location",
    ];
    let has_header = header_keys.iter().any(|k| known.contains(&k.as_str()));

    let mut col: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    if has_header {
        for (idx, key) in header_keys.iter().enumerate() {
            // Primera ocurrencia gana (no sobrescribir duplicados).
            col.entry(key.clone()).or_insert(idx);
        }
    }
    // Con cabecera la primera fila son los nombres; sin cabecera, todas son datos.
    let data_rows: &[&str] = if has_header { &rows[1..] } else { &rows[..] };

    let get = |fields: &[String], names: &[&str]| -> Option<String> {
        for n in names {
            if let Some(&idx) = col.get(*n) {
                let v = fields.get(idx).map(|s| s.trim().to_string());
                if let Some(v) = v {
                    if !v.is_empty() {
                        return Some(v);
                    }
                }
            }
        }
        None
    };

    let mut entries = Vec::new();
    for row in data_rows {
        let fields = split_csv_line(row);
        if fields.iter().all(|f| f.is_empty()) {
            continue;
        }

        if has_header {
            let title =
                get(&fields, &["title", "track", "track_name", "song", "name"]).unwrap_or_default();
            if title.is_empty() {
                continue;
            }
            let duration_ms = get(&fields, &["duration_ms"])
                .and_then(|v| v.parse::<i64>().ok())
                .or_else(|| {
                    get(&fields, &["duration", "duration_s"])
                        .and_then(|v| v.parse::<f64>().ok().map(|s| (s.max(0.0) * 1000.0) as i64))
                });
            entries.push(ParsedPlaylistEntry {
                title,
                artist: get(&fields, &["artist", "artist_name", "author"]),
                duration_ms,
                isrc: get(&fields, &["isrc"]),
                file_path: get(&fields, &["path", "file", "file_path", "location"]),
            });
        } else {
            // Sin cabecera: title,artist (formato habitual de exportación).
            let title = fields[0].trim().to_string();
            if title.is_empty() {
                continue;
            }
            entries.push(ParsedPlaylistEntry {
                title,
                artist: fields
                    .get(1)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                duration_ms: None,
                isrc: None,
                file_path: None,
            });
        }
    }

    entries
}

/// Parsea el contenido de un .txt: una pista por línea, en formato
/// "Artista - Título", título suelto, o ruta de archivo de audio.
pub fn parse_txt_content(content: &str) -> Vec<ParsedPlaylistEntry> {
    let mut entries = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if looks_like_audio_path(line) {
            // Ruta de archivo: título = stem del archivo (sin extensión) o
            // "Artista - Título" si el nombre tiene ese formato.
            let base = line
                .rsplit('/')
                .next()
                .unwrap_or(line)
                .rsplit('\\')
                .next()
                .unwrap_or(line);
            let stem = std::path::Path::new(base)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(base);
            let (artist, title) = split_artist_title(stem);
            entries.push(ParsedPlaylistEntry {
                title: if title.is_empty() {
                    base.to_string()
                } else {
                    title
                },
                artist,
                duration_ms: None,
                isrc: None,
                file_path: Some(line.to_string()),
            });
        } else {
            let (artist, title) = split_artist_title(line);
            if !title.is_empty() {
                entries.push(ParsedPlaylistEntry {
                    title,
                    artist,
                    duration_ms: None,
                    isrc: None,
                    file_path: None,
                });
            }
        }
    }
    entries
}

/// Despacha el parseo según la extensión de `file_name`.
/// Soporta .m3u/.m3u8, .csv y .txt (case-insensitive).
pub fn parse_playlist_file(
    file_name: &str,
    content: &str,
) -> Result<Vec<ParsedPlaylistEntry>, String> {
    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .ok_or_else(|| {
            "Archivo sin extensión: no se puede determinar el formato (m3u, m3u8, csv o txt)"
                .to_string()
        })?;

    let entries = match ext.as_str() {
        "m3u" | "m3u8" => parse_m3u_content(content),
        "csv" => parse_csv_content(content),
        "txt" => parse_txt_content(content),
        other => {
            return Err(format!(
                "Formato de playlist no soportado: '.{}'. Solo .m3u, .m3u8, .csv y .txt",
                other
            ))
        }
    };

    if entries.is_empty() {
        return Err(
            "No se encontraron pistas en el archivo: revisa el formato (m3u, m3u8, csv o txt)"
                .to_string(),
        );
    }

    Ok(entries)
}

/// Nombre de playlist por defecto: stem del archivo ("Mi Lista.m3u8" -> "Mi Lista").
fn default_playlist_name(file_name: &str) -> String {
    std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Imported Playlist".to_string())
}

/// Matching con el mecanismo existente de la biblioteca local, por orden de
/// especificidad: (1) ISRC exacto (normalizado, paridad migration 0064),
/// (2) file_path de downloads (misma relación que el export M3U),
/// (3) título + artista primario exactos case-insensitive,
/// (4) título exacto case-insensitive. Devuelve el track_id si hubo match.
async fn match_entry_to_track(
    db: &sqlx::SqlitePool,
    entry: &ParsedPlaylistEntry,
) -> Result<Option<i64>, String> {
    // 1. ISRC exacto (normalizado: mayúsculas sin guiones).
    if let Some(isrc) = &entry.isrc {
        let norm = normalize_isrc(isrc);
        if !norm.is_empty() {
            let id: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM tracks WHERE UPPER(REPLACE(TRIM(isrc), '-', '')) = ? LIMIT 1",
            )
            .bind(&norm)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Database error matching ISRC: {}", e))?;
            if let Some((track_id,)) = id {
                return Ok(Some(track_id));
            }
        }
    }

    // 2. Ruta de archivo local (tabla downloads, misma que usa el export M3U).
    if let Some(path) = &entry.file_path {
        let id: Option<(i64,)> =
            sqlx::query_as("SELECT track_id FROM downloads WHERE file_path = ? LIMIT 1")
                .bind(path)
                .fetch_optional(db)
                .await
                .map_err(|e| format!("Database error matching file path: {}", e))?;
        if let Some((track_id,)) = id {
            return Ok(Some(track_id));
        }
    }

    let title_norm = entry.title.trim().to_lowercase();
    if title_norm.is_empty() {
        return Ok(None);
    }

    // 3. Título + artista primario exactos (case-insensitive).
    if let Some(artist) = &entry.artist {
        let artist_norm = artist.trim().to_lowercase();
        if !artist_norm.is_empty() {
            let id: Option<(i64,)> = sqlx::query_as(
                r#"
                SELECT t.id FROM tracks t
                WHERE LOWER(TRIM(t.title)) = ?
                AND EXISTS (
                    SELECT 1 FROM track_artists ta
                    JOIN artists a ON a.id = ta.artist_id
                    WHERE ta.track_id = t.id AND LOWER(TRIM(a.name)) = ?
                )
                ORDER BY t.id ASC
                LIMIT 1
                "#,
            )
            .bind(&title_norm)
            .bind(&artist_norm)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Database error matching title+artist: {}", e))?;
            if let Some((track_id,)) = id {
                return Ok(Some(track_id));
            }
        }
    }

    // 4. Título exacto (case-insensitive).
    let id: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM tracks WHERE LOWER(TRIM(title)) = ? ORDER BY id ASC LIMIT 1",
    )
    .bind(&title_norm)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database error matching title: {}", e))?;

    Ok(id.map(|(track_id,)| track_id))
}

/// Núcleo testeable del import FE-6: parsea `content` según la extensión de
/// `file_name`, hace matching contra la biblioteca local y crea la playlist
/// con las pistas enlazadas (posiciones 1..N). NO inventa pistas: las entradas
/// sin match se reportan en `unmatched`.
pub async fn import_playlist_from_file_core(
    pool: &sqlx::SqlitePool,
    file_name: &str,
    content: &str,
    name: Option<String>,
    account_id: Option<i64>,
) -> Result<ImportPlaylistFromFileResult, String> {
    let entries = parse_playlist_file(file_name, content)?;

    let playlist_name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| default_playlist_name(file_name));

    // Resolver cuenta destino: la dada (validada), la primera activa, o la primera.
    // playlists.account_id es NOT NULL REFERENCES accounts(id), así que sin
    // cuentas no hay import (error explícito en lugar de FK críptico).
    let resolved_account: Option<(i64,)> = if let Some(aid) = account_id {
        sqlx::query_as("SELECT id FROM accounts WHERE id = ?")
            .bind(aid)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Database error resolving account: {}", e))?
    } else {
        let active: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM accounts WHERE is_active = 1 ORDER BY id LIMIT 1")
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Database error resolving account: {}", e))?;
        match active {
            Some(a) => Some(a),
            None => sqlx::query_as("SELECT id FROM accounts ORDER BY id LIMIT 1")
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("Database error resolving account: {}", e))?,
        }
    };
    let target_account_id = resolved_account.map(|(id,)| id).ok_or_else(|| {
        "No hay ninguna cuenta configurada: conecta un servicio antes de importar una playlist"
            .to_string()
    })?;

    // Matching de cada entrada contra la biblioteca local.
    let mut matched_track_ids: Vec<i64> = Vec::new();
    let mut unmatched: Vec<UnmatchedImportedEntry> = Vec::new();
    for entry in &entries {
        let track_id = match_entry_to_track(pool, entry).await?;
        match track_id {
            Some(id) => {
                // PL00 (#153): las repeticiones se conservan. playlist_tracks
                // ya no tiene UNIQUE(playlist_id, track_id) (migration 0064) y
                // el modelo nuevo permite la misma pista dos veces, incluida
                // dos veces dentro del mismo archivo importado.
                matched_track_ids.push(id);
            }
            None => unmatched.push(UnmatchedImportedEntry {
                title: entry.title.clone(),
                artist: entry.artist.clone(),
            }),
        }
    }

    // Crear playlist + pistas enlazadas en una transacción.
    let service_playlist_id = format!("file_{}", uuid::Uuid::new_v4());
    let track_count = matched_track_ids.len() as i64;

    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let playlist_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO playlists (
            account_id, service_playlist_id, name, description, is_public, track_count, created_at
        )
        VALUES (?, ?, ?, NULL, 0, ?, CURRENT_TIMESTAMP)
        RETURNING id
        "#,
    )
    .bind(target_account_id)
    .bind(&service_playlist_id)
    .bind(&playlist_name)
    .bind(track_count)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Failed to insert imported playlist: {}", e))?;

    for (idx, track_id) in matched_track_ids.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind((idx + 1) as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to add imported track to playlist: {}", e))?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit imported playlist: {}", e))?;

    tracing::info!(
        "import_playlist_from_file: '{}' -> playlist {} ({} entradas, {} matched, {} unmatched)",
        file_name,
        playlist_id,
        entries.len(),
        matched_track_ids.len(),
        unmatched.len()
    );

    Ok(ImportPlaylistFromFileResult {
        playlist_id,
        playlist_name,
        total_entries: entries.len(),
        matched_count: matched_track_ids.len(),
        unmatched,
    })
}

/// FE-6: importa una playlist desde el contenido de un archivo (.m3u/.m3u8,
/// .csv o .txt). El frontend lee el archivo (File API) y envía nombre+contenido;
/// las entradas se enlazan a pistas existentes por ISRC, ruta local o
/// título+artista; las que no se enlacen se reportan (no se inventan).
#[tauri::command]
pub async fn import_playlist_from_file(
    state: State<'_, AppState>,
    file_name: String,
    content: String,
    name: Option<String>,
    account_id: Option<i64>,
) -> Result<ImportPlaylistFromFileResult, String> {
    import_playlist_from_file_core(&state.db, &file_name, &content, name, account_id).await
}

// ============================================================================
// TASK-21: Smart Playlists Rules & Persistence
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartPlaylistRule {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartPlaylistPayload {
    #[serde(default)]
    pub name: Option<String>,
    pub rules: Vec<SmartPlaylistRule>,
    #[serde(default)]
    pub auto_update: Option<bool>,
}

pub fn parse_smart_rules(rules_json: &str) -> Result<Vec<SmartPlaylistRule>, String> {
    let trimmed = rules_json.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(rules) = serde_json::from_str::<Vec<SmartPlaylistRule>>(trimmed) {
        return Ok(rules);
    }
    if let Ok(payload) = serde_json::from_str::<SmartPlaylistPayload>(trimmed) {
        return Ok(payload.rules);
    }
    #[derive(Deserialize)]
    struct LoosePayload {
        rules: Option<Vec<SmartPlaylistRule>>,
    }
    if let Ok(loose) = serde_json::from_str::<LoosePayload>(trimmed) {
        if let Some(r) = loose.rules {
            return Ok(r);
        }
    }
    Err(format!(
        "Failed to parse smart playlist rules JSON: {}",
        rules_json
    ))
}

fn apply_smart_rules<'a>(
    builder: &mut sqlx::QueryBuilder<'a, sqlx::Sqlite>,
    rules: &[SmartPlaylistRule],
) -> bool {
    let mut has_conditions = false;
    for rule in rules {
        let field = rule.field.trim().to_lowercase();
        let op = rule.operator.trim().to_lowercase();
        let val = rule.value.trim().to_string();

        if val.is_empty() && field != "haslyrics" && field != "has_lyrics" {
            continue;
        }

        if !has_conditions {
            builder.push(" WHERE ");
            has_conditions = true;
        } else {
            builder.push(" AND ");
        }

        match field.as_str() {
            "genre" => match op.as_str() {
                "contains" | "like" => {
                    builder.push("(LOWER(COALESCE(t.genre, '')) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
                "is" | "eq" | "equals" | "=" | "==" => {
                    builder.push("(LOWER(COALESCE(t.genre, '')) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                "isnot" | "is_not" | "neq" | "not_equals" | "!=" => {
                    builder.push("(t.genre IS NULL OR LOWER(t.genre) != ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                _ => {
                    builder.push("(LOWER(COALESCE(t.genre, '')) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
            },
            "quality" | "audio_quality" => match op.as_str() {
                "contains" | "like" => {
                    builder.push("(LOWER(COALESCE(t.audio_quality, '')) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
                "is" | "eq" | "equals" | "=" | "==" => {
                    builder.push("(LOWER(COALESCE(t.audio_quality, '')) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                "isnot" | "is_not" | "neq" | "not_equals" | "!=" => {
                    builder.push("(t.audio_quality IS NULL OR LOWER(t.audio_quality) != ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                _ => {
                    builder.push("(LOWER(COALESCE(t.audio_quality, '')) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
            },
            "artist" => match op.as_str() {
                "contains" | "like" => {
                    builder.push("EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND LOWER(a.name) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
                "is" | "eq" | "equals" | "=" | "==" => {
                    builder.push("EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND LOWER(a.name) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                "isnot" | "is_not" | "neq" | "not_equals" | "!=" => {
                    builder.push("NOT EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND LOWER(a.name) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                _ => {
                    builder.push("EXISTS (SELECT 1 FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id AND LOWER(a.name) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
            },
            "year" => {
                let year_num = val.parse::<i64>().unwrap_or(0);
                match op.as_str() {
                    "is" | "eq" | "equals" | "=" | "==" => {
                        builder.push("(SUBSTR(COALESCE(al.release_date, ''), 1, 4) = ");
                        builder.push_bind(val);
                        builder.push(")");
                    }
                    "greaterthan" | "gt" | ">" => {
                        builder.push(
                            "(CAST(SUBSTR(COALESCE(al.release_date, '0000'), 1, 4) AS INTEGER) > ",
                        );
                        builder.push_bind(year_num);
                        builder.push(")");
                    }
                    "lessthan" | "lt" | "<" => {
                        builder.push(
                            "(CAST(SUBSTR(COALESCE(al.release_date, '0000'), 1, 4) AS INTEGER) < ",
                        );
                        builder.push_bind(year_num);
                        builder.push(" AND CAST(SUBSTR(COALESCE(al.release_date, '0000'), 1, 4) AS INTEGER) > 0)");
                    }
                    "contains" | "like" => {
                        builder.push("(COALESCE(al.release_date, '') LIKE ");
                        builder.push_bind(format!("%{}%", val));
                        builder.push(")");
                    }
                    _ => {
                        builder.push("(SUBSTR(COALESCE(al.release_date, ''), 1, 4) = ");
                        builder.push_bind(val);
                        builder.push(")");
                    }
                }
            }
            "service" => match op.as_str() {
                "contains" | "like" => {
                    builder.push("EXISTS (SELECT 1 FROM track_sources ts JOIN services s ON s.id = ts.service_id WHERE ts.track_id = t.id AND LOWER(s.name) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
                "is" | "eq" | "equals" | "=" | "==" => {
                    builder.push("EXISTS (SELECT 1 FROM track_sources ts JOIN services s ON s.id = ts.service_id WHERE ts.track_id = t.id AND LOWER(s.name) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                "isnot" | "is_not" | "neq" | "not_equals" | "!=" => {
                    builder.push("NOT EXISTS (SELECT 1 FROM track_sources ts JOIN services s ON s.id = ts.service_id WHERE ts.track_id = t.id AND LOWER(s.name) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                _ => {
                    builder.push("EXISTS (SELECT 1 FROM track_sources ts JOIN services s ON s.id = ts.service_id WHERE ts.track_id = t.id AND LOWER(s.name) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
            },
            "haslyrics" | "has_lyrics" => {
                let is_true =
                    val == "true" || val == "1" || val.to_lowercase() == "yes" || val.is_empty();
                // La tabla `lyrics` guarda el texto en `content` (plain, lrc o ttml);
                // formatos con fila pero sin texto (instrumentales) no cuentan.
                if is_true {
                    builder.push("EXISTS (SELECT 1 FROM lyrics l WHERE l.track_id = t.id AND l.content IS NOT NULL AND LENGTH(TRIM(l.content)) > 0)");
                } else {
                    builder.push("NOT EXISTS (SELECT 1 FROM lyrics l WHERE l.track_id = t.id AND l.content IS NOT NULL AND LENGTH(TRIM(l.content)) > 0)");
                }
            }
            "addeddate" | "added_date" => match op.as_str() {
                "greaterthan" | "gt" | ">" => {
                    builder.push("(date(t.created_at) > date(");
                    builder.push_bind(val);
                    builder.push("))");
                }
                "lessthan" | "lt" | "<" => {
                    builder.push("(date(t.created_at) < date(");
                    builder.push_bind(val);
                    builder.push("))");
                }
                _ => {
                    builder.push("(date(t.created_at) = date(");
                    builder.push_bind(val);
                    builder.push("))");
                }
            },
            "title" => match op.as_str() {
                "contains" | "like" => {
                    builder.push("(LOWER(t.title) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
                "is" | "eq" => {
                    builder.push("(LOWER(t.title) = ");
                    builder.push_bind(val.to_lowercase());
                    builder.push(")");
                }
                _ => {
                    builder.push("(LOWER(t.title) LIKE ");
                    builder.push_bind(format!("%{}%", val.to_lowercase()));
                    builder.push(")");
                }
            },
            _ => {}
        }
    }
    has_conditions
}

pub async fn preview_smart_playlist_count_core(
    pool: &sqlx::SqlitePool,
    rules_json: &str,
) -> Result<i64, String> {
    let rules = parse_smart_rules(rules_json)?;
    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT COUNT(*) FROM tracks t LEFT JOIN albums al ON al.id = t.album_id",
    );
    let has_cond = apply_smart_rules(&mut qb, &rules);
    if !has_cond {
        return Ok(0);
    }
    let count: (i64,) = qb
        .build_query_as()
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to count tracks matching smart rules: {}", e))?;
    Ok(count.0)
}

pub async fn create_smart_playlist_core(
    pool: &sqlx::SqlitePool,
    name: &str,
    rules_json: &str,
    account_id: Option<i64>,
) -> Result<Playlist, String> {
    let rules = parse_smart_rules(rules_json)?;
    let mut select_qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT t.id FROM tracks t LEFT JOIN albums al ON al.id = t.album_id",
    );
    let has_cond = apply_smart_rules(&mut select_qb, &rules);

    let track_ids: Vec<i64> = if has_cond {
        select_qb.push(" ORDER BY t.id ASC");
        select_qb
            .build_query_scalar::<i64>()
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to evaluate smart rules: {}", e))?
    } else {
        Vec::new()
    };

    let playlist_name = if name.trim().is_empty() {
        "Smart Playlist".to_string()
    } else {
        name.trim().to_string()
    };

    let service_playlist_id = format!("smart_{}", uuid::Uuid::new_v4());
    let target_account_id = account_id.unwrap_or(1);
    let track_count = track_ids.len() as i64;

    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let playlist_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO playlists (
            account_id, service_playlist_id, name, description, is_public, track_count, is_smart, rules_json, created_at
        )
        VALUES (?, ?, ?, NULL, 0, ?, 1, ?, CURRENT_TIMESTAMP)
        RETURNING id
        "#,
    )
    .bind(target_account_id)
    .bind(&service_playlist_id)
    .bind(&playlist_name)
    .bind(track_count)
    .bind(rules_json)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Failed to insert smart playlist: {}", e))?;

    for (idx, track_id) in track_ids.iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind((idx + 1) as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to add track to smart playlist: {}", e))?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit smart playlist: {}", e))?;

    let playlist = sqlx::query_as::<_, Playlist>(
        r#"
        SELECT
            p.id,
            p.name,
            p.description,
            p.owner_name,
            p.track_count,
            p.image_url,
            s.name as service_name,
            p.is_smart,
            p.rules_json
        FROM playlists p
        LEFT JOIN accounts a ON a.id = p.account_id
        LEFT JOIN services s ON s.id = a.service_id
        WHERE p.id = ?
        "#,
    )
    .bind(playlist_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to fetch created smart playlist: {}", e))?;

    Ok(playlist)
}

/// Calculate dynamic count of tracks matching smart playlist rules
#[tauri::command]
pub async fn preview_smart_playlist_count(
    state: State<'_, AppState>,
    rules_json: String,
) -> Result<i64, String> {
    preview_smart_playlist_count_core(&state.db, &rules_json).await
}

/// Create a smart playlist, evaluate its rules against library tracks, and persist it
#[tauri::command]
pub async fn create_smart_playlist(
    state: State<'_, AppState>,
    name: String,
    rules_json: String,
    account_id: Option<i64>,
) -> Result<Playlist, String> {
    create_smart_playlist_core(&state.db, &name, &rules_json, account_id).await
}

#[cfg(test)]
mod playlist_import_parser_tests {
    use super::*;

    #[test]
    fn test_split_artist_title() {
        assert_eq!(
            split_artist_title("Daft Punk - One More Time"),
            (Some("Daft Punk".to_string()), "One More Time".to_string())
        );
        // Solo el primer " - " separa (paridad CLI).
        assert_eq!(
            split_artist_title("AC/DC - TNT - Live"),
            (Some("AC/DC".to_string()), "TNT - Live".to_string())
        );
        // Sin separador: todo es título.
        assert_eq!(
            split_artist_title("Only Title"),
            (None, "Only Title".to_string())
        );
        // Artista vacío: no se separa.
        assert_eq!(
            split_artist_title(" - Title"),
            (None, "- Title".to_string())
        );
    }

    #[test]
    fn test_looks_like_audio_path() {
        assert!(looks_like_audio_path("/music/song.mp3"));
        assert!(looks_like_audio_path("C:\\Music\\Song.FLAC"));
        assert!(looks_like_audio_path("relative/track 01.ogg"));
        assert!(!looks_like_audio_path("Artist - Title"));
        assert!(!looks_like_audio_path("mp3"));
        assert!(!looks_like_audio_path("plain text"));
    }

    #[test]
    fn test_normalize_isrc() {
        assert_eq!(normalize_isrc("us-um7-11-00999"), "USUM71100999");
        assert_eq!(normalize_isrc(" GBAAA1200456 "), "GBAAA1200456");
    }

    #[test]
    fn test_parse_m3u_full() {
        let content = "#EXTM3U\n\
                      #EXTINF:304,Daft Punk - One More Time\n\
                      # ISRC: USUM71100999\n\
                      /music/daft_punk/one_more_time.flac\n\
                      #EXTINF:-12,Bad Duration Artist - Song B\n\
                      /music/song_b.mp3\n\
                      #EXTM3U\n";
        let entries = parse_m3u_content(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "One More Time");
        assert_eq!(entries[0].artist.as_deref(), Some("Daft Punk"));
        assert_eq!(entries[0].duration_ms, Some(304_000));
        assert_eq!(entries[0].isrc.as_deref(), Some("USUM71100999"));
        assert_eq!(
            entries[0].file_path.as_deref(),
            Some("/music/daft_punk/one_more_time.flac")
        );
        // Duración negativa se satura a 0*1000.
        assert_eq!(entries[1].duration_ms, Some(0));
        assert_eq!(entries[1].title, "Song B");
    }

    #[test]
    fn test_parse_m3u_artist_title_lines_only() {
        // M3U sin rutas: EXTINF suelto + líneas "Artista - Título".
        let content = "#EXTM3U\n\
                      Artist A - Song A\n\
                      Artist B - Song B\n\
                      Plain Song\n";
        let entries = parse_m3u_content(content);
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0].artist.as_deref(),
            Some("Artist A"),
            "la primera línea no debe heredar el título del siguiente EXTINF"
        );
        assert_eq!(entries[1].title, "Song B");
        assert!(entries[2].artist.is_none());
        assert_eq!(entries[2].title, "Plain Song");
    }

    #[test]
    fn test_parse_m3u_extinf_without_path_is_flushed() {
        // EXTINF seguido de una línea suelta: el EXTINF pendiente se emite
        // como entrada propia antes de la línea.
        let content = "#EXTM3U\n\
                      #EXTINF:100,Pending Artist - Pending Song\n\
                      Loose Artist - Loose Song\n";
        let entries = parse_m3u_content(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Pending Song");
        assert_eq!(entries[0].duration_ms, Some(100_000));
        assert!(entries[0].file_path.is_none());
        assert_eq!(entries[1].title, "Loose Song");
    }

    #[test]
    fn test_parse_csv_with_header_and_quotes() {
        let content = "Title,Artist,ISRC,Duration (ms),Path\n\
                       \"Song, With Comma\",Daft Punk,USUM71100999,304000,/music/a.flac\n\
                       Second Song,Other Artist,,,\n";
        let entries = parse_csv_content(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Song, With Comma");
        assert_eq!(entries[0].artist.as_deref(), Some("Daft Punk"));
        assert_eq!(entries[0].isrc.as_deref(), Some("USUM71100999"));
        assert_eq!(entries[0].duration_ms, Some(304_000));
        assert_eq!(entries[0].file_path.as_deref(), Some("/music/a.flac"));
        assert_eq!(entries[1].title, "Second Song");
        assert_eq!(entries[1].duration_ms, None, "duracion no numérica -> None");
        assert!(entries[1].file_path.is_none());
    }

    #[test]
    fn test_parse_csv_headerless_assumes_title_artist() {
        let content = "Song One,Artist One\nSong Two,Artist Two\n,Empty\n";
        let entries = parse_csv_content(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Song One");
        assert_eq!(entries[0].artist.as_deref(), Some("Artist One"));
        assert_eq!(entries[1].title, "Song Two");
    }

    #[test]
    fn test_parse_csv_title_first_column_wins() {
        // Cabecera con título y artista intercambiados: el mapeo es por nombre.
        let content = "Artist,Title\nDaft Punk,One More Time\n";
        let entries = parse_csv_content(content);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "One More Time");
        assert_eq!(entries[0].artist.as_deref(), Some("Daft Punk"));
    }

    #[test]
    fn test_parse_txt_one_per_line() {
        let content = "# comentario\n\
                      Artist A - Song A\n\
                      Song B\n\
                      /music/song_c.mp3\n\
                      \n";
        let entries = parse_txt_content(content);
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries[0].artist.as_deref(),
            Some("Artist A"),
            "la primera línea no debe heredar el título de la siguiente"
        );
        assert_eq!(entries[1].title, "Song B");
        assert_eq!(entries[2].file_path.as_deref(), Some("/music/song_c.mp3"));
        assert_eq!(entries[2].title, "song_c");
    }

    #[test]
    fn test_parse_playlist_file_dispatch_and_errors() {
        assert!(parse_playlist_file("list.m3u", "#EXTM3U\nA - B\n").is_ok());
        assert!(parse_playlist_file("list.M3U8", "A - B\n").is_ok());
        assert!(parse_playlist_file("list.csv", "Title,Artist\nA,B\n").is_ok());
        assert!(parse_playlist_file("list.txt", "A - B\n").is_ok());
        // Formato no soportado / vacío / sin pistas.
        assert!(parse_playlist_file("list.json", "[]").is_err());
        assert!(parse_playlist_file("list.txt", "   \n# solo comentarios\n").is_err());
        assert!(parse_playlist_file("noext", "A - B").is_err());
    }

    #[test]
    fn test_default_playlist_name() {
        assert_eq!(default_playlist_name("Mi Lista.m3u8"), "Mi Lista");
        assert_eq!(default_playlist_name("noext"), "noext");
        // Nombre vacío/espacios: fallback.
        assert_eq!(default_playlist_name("   "), "Imported Playlist");
    }
}

#[cfg(test)]
mod playlist_canonical_command_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Pool, Sqlite};

    async fn migrated_pool() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool :memory:");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migraciones reales");
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
        .expect("cuenta")
    }

    async fn seed_remote(
        pool: &Pool<Sqlite>,
        account_id: i64,
        remote_id: &str,
        name: &str,
        track_ids: &[i64],
    ) -> i64 {
        let playlist_id = sqlx::query_scalar(
            "INSERT INTO playlists
                (account_id, service_playlist_id, name, track_count, created_at)
             VALUES (?, ?, ?, ?, datetime('now', '-' || ? || ' seconds')) RETURNING id",
        )
        .bind(account_id)
        .bind(remote_id)
        .bind(name)
        .bind(track_ids.len() as i64)
        .bind(1000 - remote_id.len() as i64)
        .fetch_one(pool)
        .await
        .expect("playlist");
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
        .expect("fuente");
        for (position, track_id) in track_ids.iter().enumerate() {
            sqlx::query(
                "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
            )
            .bind(playlist_id)
            .bind(track_id)
            .bind(position as i64 + 1)
            .execute(pool)
            .await
            .expect("pista");
        }
        playlist_id
    }

    async fn seed_tracks(pool: &Pool<Sqlite>) -> [i64; 3] {
        let mut ids = [0; 3];
        for (index, slot) in ids.iter_mut().enumerate() {
            *slot =
                sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
                    .bind(format!("Command track {index}"))
                    .bind(format!("CMD-{}-{index}", uuid::Uuid::new_v4()))
                    .fetch_one(pool)
                    .await
                    .expect("track");
        }
        ids
    }

    #[tokio::test]
    async fn detail_lists_every_canonical_source_and_local_has_none() {
        let pool = migrated_pool().await;
        let spotify = seed_account(&pool, "spotify", "Detail Spotify").await;
        let qobuz = seed_account(&pool, "qobuz", "Detail Qobuz").await;
        let tracks = seed_tracks(&pool).await;
        let primary = seed_remote(
            &pool,
            spotify,
            "detail-sp",
            "Detail",
            &[tracks[0], tracks[1]],
        )
        .await;
        let copy = seed_remote(&pool, qobuz, "detail-qb", "detail", &tracks[..2]).await;
        for playlist_id in [primary, copy] {
            crate::services::playlist_canonical::record_playlist_source_expansion(
                &pool,
                playlist_id,
            )
            .await
            .expect("expansión completa");
        }
        crate::services::playlist_canonical::canonicalize_playlists(&pool)
            .await
            .expect("canonicalización");

        let detail = get_playlist_detail_core(&pool, copy)
            .await
            .expect("detalle")
            .expect("playlist existe");
        assert_eq!(detail.playlist.id, copy);
        assert_eq!(detail.playlist.name, "detail");
        assert!(!detail.is_primary);
        assert_eq!(detail.sources.len(), 2);
        assert_eq!(detail.sources[0].playlist_id, primary);
        assert!(detail.sources[0].is_primary);
        assert!(detail
            .sources
            .iter()
            .any(|source| source.account_id == spotify && source.service_name == "spotify"));
        assert!(detail
            .sources
            .iter()
            .any(|source| source.account_id == qobuz && source.service_name == "qobuz"));

        let local = create_playlist_core(&pool, "Local detail".into(), None)
            .await
            .expect("local");
        let local_detail = get_playlist_detail_core(&pool, local)
            .await
            .expect("detalle local")
            .expect("local existe");
        assert!(local_detail.canonical_id.is_none());
        assert!(local_detail.sources.is_empty());
        assert!(get_playlist_detail_core(&pool, -1)
            .await
            .expect("inexistente")
            .is_none());
    }

    #[tokio::test]
    async fn explicit_expansion_rename_and_remove_keep_membership_in_sync() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "qobuz", "Mutation").await;
        let tracks = seed_tracks(&pool).await;
        let first = seed_remote(&pool, account, "mutation-a", "Mutation", &tracks[..2]).await;
        let second = seed_remote(&pool, account, "mutation-b", "mutation", &tracks[..2]).await;

        recompact_playlist_positions(&pool, first)
            .await
            .expect("recompactar primera");
        recompact_playlist_positions(&pool, second)
            .await
            .expect("recompactar segunda");
        for playlist_id in [first, second] {
            assert!(
                crate::services::playlist_canonical::record_playlist_source_expansion(
                    &pool,
                    playlist_id,
                )
                .await
                .expect("expansión completa")
            );
            crate::services::playlist_canonical::canonicalize_playlist(&pool, playlist_id)
                .await
                .expect("canonicalizar tras expansión");
        }
        let first_detail = get_playlist_detail_core(&pool, first)
            .await
            .unwrap()
            .unwrap();
        let second_detail = get_playlist_detail_core(&pool, second)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first_detail.canonical_id, second_detail.canonical_id);
        assert_eq!(second_detail.sources.len(), 2);

        update_playlist_core(&pool, second, Some("Different".into()), None, None)
            .await
            .expect("renombrar");
        let renamed = get_playlist_detail_core(&pool, second)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(renamed.canonical_id, first_detail.canonical_id);
        assert_eq!(renamed.sources.len(), 1);

        update_playlist_core(&pool, second, Some("Mutation".into()), None, None)
            .await
            .expect("devolver nombre");
        assert_eq!(
            get_playlist_detail_core(&pool, second)
                .await
                .unwrap()
                .unwrap()
                .canonical_id,
            first_detail.canonical_id
        );

        remove_from_playlist_core(&pool, second, &[tracks[0]])
            .await
            .expect("quitar pista");
        let incomplete = get_playlist_detail_core(&pool, second)
            .await
            .unwrap()
            .unwrap();
        assert!(incomplete.canonical_id.is_none());
        let surviving = get_playlist_detail_core(&pool, first)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(surviving.sources.len(), 1);
        assert!(surviving.is_primary);
    }

    #[tokio::test]
    async fn generic_sanitization_never_certifies_provider_expansion() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "tidal", "No certification").await;
        let tracks = seed_tracks(&pool).await;
        let first = seed_remote(&pool, account, "uncertified-a", "Pending", &tracks[..2]).await;
        let second = seed_remote(&pool, account, "uncertified-b", "pending", &tracks[..2]).await;

        sanitize_playlists_in_pool(&pool)
            .await
            .expect("sanitización sin certificar");

        assert!(get_playlist_detail_core(&pool, first)
            .await
            .unwrap()
            .unwrap()
            .canonical_id
            .is_none());
        assert!(get_playlist_detail_core(&pool, second)
            .await
            .unwrap()
            .unwrap()
            .canonical_id
            .is_none());
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM playlist_sources WHERE expansion_complete IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(pending, 2);
    }

    #[tokio::test]
    async fn sanitization_keeps_exact_copies_together_and_disambiguates_only_the_other_content() {
        let pool = migrated_pool().await;
        let account = seed_account(&pool, "spotify", "Sanitize").await;
        let tracks = seed_tracks(&pool).await;
        let first = seed_remote(&pool, account, "sanitize-a", "Mix", &tracks[..2]).await;
        let different =
            seed_remote(&pool, account, "sanitize-c", "MIX", &[tracks[0], tracks[2]]).await;
        let copy = seed_remote(&pool, account, "sanitize-b", " mix ", &tracks[..2]).await;
        let different_copy =
            seed_remote(&pool, account, "sanitize-d", "mix", &[tracks[0], tracks[2]]).await;
        for playlist_id in [first, different, copy, different_copy] {
            crate::services::playlist_canonical::record_playlist_source_expansion(
                &pool,
                playlist_id,
            )
            .await
            .expect("expansión completa");
        }

        let stats = sanitize_playlists_in_pool(&pool)
            .await
            .expect("sanitización global");

        assert_eq!(stats.playlist_names_disambiguated, 2);
        let first_detail = get_playlist_detail_core(&pool, first)
            .await
            .unwrap()
            .unwrap();
        let copy_detail = get_playlist_detail_core(&pool, copy)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first_detail.canonical_id, copy_detail.canonical_id);
        assert_eq!(copy_detail.sources.len(), 2);
        assert_eq!(copy_detail.playlist.name, " mix ");
        let different_detail = get_playlist_detail_core(&pool, different)
            .await
            .unwrap()
            .unwrap();
        let different_copy_detail = get_playlist_detail_core(&pool, different_copy)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            different_detail.canonical_id,
            different_copy_detail.canonical_id
        );
        assert_eq!(different_detail.sources.len(), 2);
        assert_eq!(different_detail.playlist.name, "MIX (2)");
        assert_eq!(different_copy_detail.playlist.name, "MIX (2)");
    }
}
