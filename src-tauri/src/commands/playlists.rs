// Playlist Commands - included via include!() in mod.rs
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

    // Recompact positions sequentially
    let remaining: Vec<(i64,)> = sqlx::query_as(
        "SELECT id FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC, id ASC"
    )
    .bind(playlist_id)
    .fetch_all(&mut *tx)
    .await
    .unwrap_or_default();

    for (pos, (row_id,)) in remaining.into_iter().enumerate() {
        let _ = sqlx::query("UPDATE playlist_tracks SET position = ? WHERE id = ?")
            .bind(pos as i64)
            .bind(row_id)
            .execute(&mut *tx)
            .await;
    }

    // Update track_count in playlists table
    let _ = sqlx::query(
        "UPDATE playlists SET track_count = (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?) WHERE id = ?"
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&mut *tx)
    .await;

    tx.commit().await
        .map_err(|e| format!("Failed to commit track removal: {}", e))?;

    Ok(removed)
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

/// Escritura atómica-en-un-archivo del M3U (bloqueante; crear directorios padre).
fn write_m3u_to_disk(path: &str, contents: &str) -> Result<u64, String> {
    let target = std::path::Path::new(path);
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!("No se pudo crear el directorio {}: {}", parent.display(), e)
            })?;
        }
    }
    std::fs::write(target, contents)
        .map_err(|e| format!("No se pudo escribir {}: {}", path, e))?;
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
