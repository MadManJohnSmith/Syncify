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

/// Get detailed playlist information by ID
#[tauri::command]
pub async fn get_playlist(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Option<Playlist>, String> {
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
        WHERE p.id = ?
        "#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("Failed to get playlist: {}", e))?;

    Ok(playlist)
}

/// Update a playlist's name or description
#[tauri::command]
pub async fn update_playlist(
    state: State<'_, AppState>,
    id: i64,
    name: Option<String>,
    description: Option<String>,
    is_public: Option<bool>,
) -> Result<Playlist, String> {
    let mut tx = state.db.begin_with("BEGIN IMMEDIATE").await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    if let Some(new_name) = &name {
        sqlx::query("UPDATE playlists SET name = ? WHERE id = ?")
            .bind(new_name)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to update name: {}", e))?;
    }

    if let Some(new_desc) = &description {
        sqlx::query("UPDATE playlists SET description = ? WHERE id = ?")
            .bind(new_desc)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to update description: {}", e))?;
    }

    let _ = is_public; // Preserved for forward-compatibility

    tx.commit().await
        .map_err(|e| format!("Failed to commit update: {}", e))?;

    let updated = get_playlist(state, id).await?
        .ok_or_else(|| format!("Playlist {} not found after update", id))?;

    Ok(updated)
}

/// Delete a playlist and cascade delete its tracks associations
#[tauri::command]
pub async fn delete_playlist(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let mut tx = state.db.begin_with("BEGIN IMMEDIATE").await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    // Cascade delete playlist_tracks and playlist_sources
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to delete playlist tracks: {}", e))?;

    let _ = sqlx::query("DELETE FROM playlist_sources WHERE playlist_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await;

    let res = sqlx::query("DELETE FROM playlists WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to delete playlist: {}", e))?;

    if res.rows_affected() == 0 {
        return Err(format!("Playlist {} not found", id));
    }

    tx.commit().await
        .map_err(|e| format!("Failed to commit delete: {}", e))?;

    Ok(())
}

/// Remove specific tracks from a playlist and compact positions
#[tauri::command]
pub async fn remove_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_ids: Vec<i64>,
) -> Result<usize, String> {
    if track_ids.is_empty() {
        return Ok(0);
    }

    let mut tx = state.db.begin_with("BEGIN IMMEDIATE").await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    let mut removed = 0usize;
    for tid in track_ids {
        let res = sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?")
            .bind(playlist_id)
            .bind(tid)
            .execute(&mut *tx)
            .await;

        if let Ok(r) = res {
            removed += r.rows_affected() as usize;
        }
    }

    tx.commit().await
        .map_err(|e| format!("Failed to commit track removal: {}", e))?;

    // TASK-79: Recompact positions sequentially (strictly 1-indexed) and update track_count
    recompact_playlist_positions(&state.db, playlist_id).await?;

    Ok(removed)
}

/// TASK-107: Sanitization result statistics for playlists
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSanitizationStats {
    pub duplicate_tracks_purged: usize,
    pub playlists_recompacted: usize,
    pub track_counts_updated: usize,
    pub playlist_names_disambiguated: usize,
}

/// TASK-107: Transactionally sanitizes a single playlist:
/// 1. Purgar pistas duplicadas dentro de la misma playlist conservando la de menor position (primera aparición).
/// 2. Recompactar position secuencialmente 1..N usando técnica segura contra colisiones transitorias UNIQUE (staging negativo).
/// 3. Sincronizar playlists.track_count = (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = playlists.id).
pub async fn sanitize_single_playlist(
    pool: &sqlx::SqlitePool,
    playlist_id: i64,
) -> Result<usize, String> {
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to begin transaction for playlist sanitization: {}", e))?;

    // 1. Purge duplicate tracks within this playlist, keeping the first occurrence (lowest position)
    let purge_res = sqlx::query(
        r#"
        DELETE FROM playlist_tracks
        WHERE id IN (
            SELECT id FROM (
                SELECT id,
                       ROW_NUMBER() OVER (
                           PARTITION BY track_id
                           ORDER BY position ASC, added_at ASC, id ASC
                       ) as rn
                FROM playlist_tracks
                WHERE playlist_id = ?
            ) WHERE rn > 1
        )
        "#,
    )
    .bind(playlist_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to purge duplicate tracks in playlist {}: {}", playlist_id, e))?;

    let purged_count = purge_res.rows_affected() as usize;

    // 2. Fetch remaining tracks ordered by current position ASC, and added_at ASC, id ASC as tie-breakers
    let remaining: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC, added_at ASC, id ASC",
    )
    .bind(playlist_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| format!("Failed to fetch playlist tracks for recompact: {}", e))?;

    // 3. Stage existing positions to unique negative values to avoid UNIQUE(playlist_id, position) collisions
    sqlx::query("UPDATE playlist_tracks SET position = -(id + 1) WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to stage playlist track positions: {}", e))?;

    // 4. Sequentially assign 1-indexed positions (1, 2, 3... N)
    for (idx, (row_id,)) in remaining.into_iter().enumerate() {
        let canonical_pos = (idx + 1) as i64;
        sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?")
            .bind(canonical_pos)
            .bind(row_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to reassign playlist track position: {}", e))?;
    }

    // 5. Atomically update track_count in playlists table to match exact COUNT(*)
    sqlx::query(
        "UPDATE playlists SET track_count = (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?) WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update playlists.track_count: {}", e))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit playlist sanitization: {}", e))?;

    Ok(purged_count)
}

/// TASK-79 & TASK-107: Recompact playlist positions to be strictly 1-indexed, sequential, and gap-free (1, 2, 3... N).
/// Atomically purges duplicate tracks within the playlist, recompacts positions, and reconciles `playlists.track_count`.
pub async fn recompact_playlist_positions(
    pool: &sqlx::SqlitePool,
    playlist_id: i64,
) -> Result<(), String> {
    sanitize_single_playlist(pool, playlist_id).await.map(|_| ())
}

/// TASK-107: Transactionally sanitizes all playlists across the library:
/// 1. Purgar pistas duplicadas dentro de la misma playlist conservando la de menor position (primera aparición).
/// 2. Recompactar position secuencialmente 1..N sin huecos para todas las playlists.
/// 3. Sincronizar playlists.track_count con el conteo real en playlist_tracks.
/// 4. Desambiguar colisiones de nombres de playlists bajo la misma cuenta (account_id, LOWER(TRIM(name))).
pub async fn sanitize_playlists_in_pool(
    pool: &sqlx::SqlitePool,
) -> Result<PlaylistSanitizationStats, String> {
    let mut tx = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to begin transaction for global playlist sanitization: {}", e))?;

    // 1. Purgar pistas duplicadas dentro de cada playlist conservando la primera aparición
    let purge_res = sqlx::query(
        r#"
        DELETE FROM playlist_tracks
        WHERE id IN (
            SELECT id FROM (
                SELECT id,
                       ROW_NUMBER() OVER (
                           PARTITION BY playlist_id, track_id
                           ORDER BY position ASC, added_at ASC, id ASC
                       ) as rn
                FROM playlist_tracks
            ) WHERE rn > 1
        )
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to purge duplicate tracks across playlists: {}", e))?;

    let duplicate_tracks_purged = purge_res.rows_affected() as usize;

    // 2. Recompactar position a secuencia contigua 1..N sin huecos
    // 2a. Crear staging temporal con nueva posición 1-indexed
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

    // 2b. Staging negativo de todas las posiciones para evitar colisiones UNIQUE(playlist_id, position)
    sqlx::query("UPDATE playlist_tracks SET position = -(id + 1)")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to stage playlist positions to negative: {}", e))?;

    // 2c. Aplicar nuevas posiciones 1-indexed desde la tabla staging
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

    // 3. Sincronizar track_count de todas las playlists
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

    // 4. Desambiguar colisiones de nombres de playlists bajo la misma cuenta (account_id, LOWER(TRIM(name)))
    let dup_name_groups: Vec<(i64, String, i64)> = sqlx::query_as(
        r#"
        SELECT account_id, LOWER(TRIM(name)) as norm_name, COUNT(*) as cnt
        FROM playlists
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
        let pls: Vec<(i64, String)> = sqlx::query_as(
            r#"
            SELECT id, name
            FROM playlists
            WHERE account_id = ? AND LOWER(TRIM(name)) = ?
            ORDER BY id ASC
            "#,
        )
        .bind(acc_id)
        .bind(&norm_name)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("Failed to fetch playlists for group '{}': {}", norm_name, e))?;

        let existing_names: Vec<(String,)> = sqlx::query_as(
            "SELECT LOWER(TRIM(name)) FROM playlists WHERE account_id = ?",
        )
        .bind(acc_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("Failed to fetch existing playlist names for account {}: {}", acc_id, e))?;

        let mut existing_set: std::collections::HashSet<String> = existing_names
            .into_iter()
            .map(|(n,)| n)
            .collect();

        // La primera conserva su nombre original (pls[0]). Las siguientes reciben sufijo (2), (3)...
        for (idx, (pid, orig_name)) in pls.into_iter().enumerate().skip(1) {
            let mut cand_idx = (idx + 1) as usize;
            let mut new_name = format!("{} ({})", orig_name.trim(), cand_idx);
            while existing_set.contains(&new_name.trim().to_lowercase()) {
                cand_idx += 1;
                new_name = format!("{} ({})", orig_name.trim(), cand_idx);
            }
            existing_set.insert(new_name.trim().to_lowercase());

            sqlx::query("UPDATE playlists SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(&new_name)
                .bind(pid)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update disambiguated playlist name for id {}: {}", pid, e))?;

            playlist_names_disambiguated += 1;
        }
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit global playlist sanitization: {}", e))?;

    let pls_with_tracks: (i64,) = sqlx::query_as("SELECT COUNT(DISTINCT playlist_id) FROM playlist_tracks")
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
#[tauri::command]
pub async fn reorder_playlist_tracks(
    state: State<'_, AppState>,
    playlist_id: i64,
    positions: Vec<PlaylistTrackPosition>,
) -> Result<(), String> {
    let mut tx = state.db.begin_with("BEGIN IMMEDIATE").await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    // Stage existing positions to negative values to avoid UNIQUE(playlist_id, position) collisions during sequential update
    sqlx::query("UPDATE playlist_tracks SET position = -position - 1 WHERE playlist_id = ?")
        .bind(playlist_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to stage playlist reordering: {}", e))?;

    for item in positions {
        sqlx::query(
            "UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND track_id = ?"
        )
        .bind(item.new_position)
        .bind(playlist_id)
        .bind(item.track_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to update track position: {}", e))?;
    }

    tx.commit().await
        .map_err(|e| format!("Failed to commit reordering: {}", e))?;

    // TASK-79: Recompact after reordering to guarantee 1-indexed continuous sequence and track_count consistency
    recompact_playlist_positions(&state.db, playlist_id).await?;

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
          AND (? = 'all' OR LOWER(s.name) = LOWER(?))
        GROUP BY s.name
        ORDER BY s.name
        "#,
    )
    .bind(if filter_specific { target_service.as_str() } else { "all" })
    .bind(target_service.as_str())
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to aggregate playlists: {}", e))?;

    let services: Vec<PlaylistServiceSummary> = rows
        .into_iter()
        .map(|(name, playlists, tracks, last_synced)| PlaylistServiceSummary {
            service: name,
            playlists,
            tracks_linked: tracks,
            last_synced,
        })
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
            return Err("Acceso denegado: secuencias de escape ('..') detectadas (sandbox violation)".to_string());
        }
    }

    // 3. Reject hidden files
    let file_name = target_path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| "Acceso denegado: nombre de archivo no válido (sandbox violation)".to_string())?;

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
        return Err("Acceso denegado: ruta en directorio protegido del sistema (sandbox violation)".to_string());
    }

    if allowed_bases.is_empty() {
        return Err("Acceso denegado: no se definieron directorios base permitidos (sandbox violation)".to_string());
    }

    // 6. Lexical containment check against allowed bases
    let matches_lexical = allowed_bases.iter().any(|base| target_path.starts_with(base));
    if !matches_lexical {
        return Err(
            "Acceso denegado: la ruta está fuera de los directorios permitidos (sandbox violation)".to_string(),
        );
    }

    // 7. Parent directory resolution and creation
    let parent = target_path
        .parent()
        .ok_or_else(|| "Acceso denegado: ruta sin directorio padre válido (sandbox violation)".to_string())?;

    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear el directorio {}: {}", parent.display(), e))?;
    }

    // 8. Canonicalize parent directory and verify containment
    let canonical_parent = std::fs::canonicalize(parent)
        .map_err(|e| format!("Error al canonicalizar directorio {}: {}", parent.display(), e))?;

    let mut canonical_allowed_bases = Vec::new();
    for b in allowed_bases {
        if let Ok(c) = std::fs::canonicalize(b) {
            canonical_allowed_bases.push(c);
        }
        canonical_allowed_bases.push(b.clone());
    }

    if !canonical_allowed_bases.iter().any(|base| canonical_parent.starts_with(base)) {
        return Err("Acceso denegado: el directorio destino canonicalizado está fuera del sandbox permitido (sandbox violation)".to_string());
    }

    let safe_target = canonical_parent.join(file_name);

    // 9. Prevent symlink overwriting or escaping via existing symlinks
    if safe_target.is_symlink()
        || std::fs::symlink_metadata(&safe_target)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err("Acceso denegado: no se permite sobreescribir enlaces simbólicos (sandbox violation)".to_string());
    }

    if safe_target.exists() {
        let canonical_target = std::fs::canonicalize(&safe_target)
            .map_err(|e| format!("Error al canonicalizar archivo existente: {}", e))?;
        if !canonical_allowed_bases.iter().any(|base| canonical_target.starts_with(base)) {
            return Err("Acceso denegado: el archivo destino existente resuelve fuera del sandbox permitido (sandbox violation)".to_string());
        }
    }

    Ok(safe_target)
}

/// Helper to validate an M3U export path against default allowed directories.
pub fn validate_safe_m3u_write_path(target_path: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let allowed_bases = get_allowed_m3u_directories();
    validate_safe_m3u_write_path_with_bases(target_path, &allowed_bases)
}

/// Escritura atómica-en-un-archivo del M3U con bases permitidas personalizadas.
#[allow(dead_code)]
pub fn write_m3u_to_disk_with_bases(
    path: &str,
    contents: &str,
    allowed_bases: &[std::path::PathBuf],
) -> Result<u64, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Acceso denegado: la ruta no puede estar vacía (sandbox violation)".to_string());
    }
    let target = std::path::Path::new(trimmed);
    let safe_target = validate_safe_m3u_write_path_with_bases(target, allowed_bases)?;

    std::fs::write(&safe_target, contents)
        .map_err(|e| format!("No se pudo escribir {}: {}", safe_target.display(), e))?;
    Ok(contents.as_bytes().len() as u64)
}

/// Escritura de M3U en disco confinado a directorios permitidos (Música, Descargas, Documentos, App Data).
pub fn write_m3u_to_disk(path: &str, contents: &str) -> Result<u64, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Acceso denegado: la ruta no puede estar vacía (sandbox violation)".to_string());
    }
    let target = std::path::Path::new(trimmed);
    let safe_target = validate_safe_m3u_write_path(target)?;

    std::fs::write(&safe_target, contents)
        .map_err(|e| format!("No se pudo escribir {}: {}", safe_target.display(), e))?;
    Ok(contents.as_bytes().len() as u64)
}

/// Núcleo testeable del export Modo A: verifica archivos reales y, si se da
/// `file_path`, escribe el .m3u. Toda la IO de disco corre en spawn_blocking.
pub async fn export_playlist_m3u_core(
    db: &sqlx::SqlitePool,
    playlist_id: i64,
    file_path: Option<String>,
) -> Result<PlaylistM3uExportResult, String> {
    let name_row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
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
        let bytes = tokio::task::spawn_blocking(move || {
            write_m3u_to_disk(&path, &content_for_write)
        })
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
    Err(format!("Failed to parse smart playlist rules JSON: {}", rules_json))
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
                        builder.push("(CAST(SUBSTR(COALESCE(al.release_date, '0000'), 1, 4) AS INTEGER) > ");
                        builder.push_bind(year_num);
                        builder.push(")");
                    }
                    "lessthan" | "lt" | "<" => {
                        builder.push("(CAST(SUBSTR(COALESCE(al.release_date, '0000'), 1, 4) AS INTEGER) < ");
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
            },
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
                let is_true = val == "true" || val == "1" || val.to_lowercase() == "yes" || val.is_empty();
                if is_true {
                    builder.push("EXISTS (SELECT 1 FROM lyrics l WHERE l.track_id = t.id AND ((l.plain_lyrics IS NOT NULL AND LENGTH(TRIM(l.plain_lyrics)) > 0) OR (l.synced_lyrics IS NOT NULL AND LENGTH(TRIM(l.synced_lyrics)) > 0)))");
                } else {
                    builder.push("NOT EXISTS (SELECT 1 FROM lyrics l WHERE l.track_id = t.id AND ((l.plain_lyrics IS NOT NULL AND LENGTH(TRIM(l.plain_lyrics)) > 0) OR (l.synced_lyrics IS NOT NULL AND LENGTH(TRIM(l.synced_lyrics)) > 0)))");
                }
            },
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

