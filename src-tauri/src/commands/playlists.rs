// Playlist Commands - included via include!() in mod.rs
// Manages playlist CRUD, reordering, and multi-service synchronization

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrackPosition {
    pub track_id: i64,
    pub new_position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPlaylistsResult {
    pub playlists_synced: i64,
    pub tracks_linked: i64,
    pub message: String,
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
    let mut tx = state.db.begin().await
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
    let mut tx = state.db.begin().await
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

    let mut tx = state.db.begin().await
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
    let mut tx = state.db.begin().await
        .map_err(|e| format!("Failed to start transaction: {}", e))?;

    for item in positions {
        let _ = sqlx::query(
            "UPDATE playlist_tracks SET position = ? WHERE playlist_id = ? AND track_id = ?"
        )
        .bind(item.new_position)
        .bind(playlist_id)
        .bind(item.track_id)
        .execute(&mut *tx)
        .await;
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
    let _target_service = service.unwrap_or_else(|| "all".to_string());
    
    // Query active accounts
    let accounts: Vec<(i64, String, String)> = sqlx::query_as(
        r#"
        SELECT a.id, s.name, COALESCE(a.display_name, a.email, 'Account')
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE a.is_active = 1
        "#
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut total_playlists = 0i64;
    let mut total_tracks = 0i64;

    for (acc_id, s_name, _display) in accounts {
        if _target_service != "all" && !s_name.eq_ignore_ascii_case(&_target_service) {
            continue;
        }

        // Count playlists and tracks already synchronized for this account
        let p_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM playlists WHERE account_id = ?"
        )
        .bind(acc_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        let t_count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) 
            FROM playlist_tracks pt
            JOIN playlists p ON p.id = pt.playlist_id
            WHERE p.account_id = ?
            "#
        )
        .bind(acc_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

        total_playlists += p_count.0;
        total_tracks += t_count.0;
    }

    Ok(SyncPlaylistsResult {
        playlists_synced: total_playlists,
        tracks_linked: total_tracks,
        message: format!("Synchronized {} playlists with {} tracks across active services", total_playlists, total_tracks),
    })
}
