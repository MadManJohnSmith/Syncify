// Backup & Restore Commands - included via include!() in mod.rs
// Manages JSON/SQLite library backup export and atomic restore across machines

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTrackDto {
    pub isrc: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration_ms: Option<i64>,
    pub explicit: Option<i32>,
    pub favorite_at: Option<String>,
    pub service: Option<String>,
    pub service_track_id: Option<String>,
    pub downloaded_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupAlbumDto {
    pub title: String,
    pub artist: String,
    pub upc: Option<String>,
    pub release_date: Option<String>,
    pub favorite_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupArtistDto {
    pub name: String,
    pub favorite_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPlaylistDto {
    pub name: String,
    pub description: Option<String>,
    pub track_isrcs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryBackupManifest {
    pub version: String,
    pub schema_version: i64,
    pub exported_at: String,
    pub app_version: String,
    pub checksum_sha256: Option<String>,
    pub tracks: Vec<BackupTrackDto>,
    pub albums: Vec<BackupAlbumDto>,
    pub artists: Vec<BackupArtistDto>,
    pub playlists: Vec<BackupPlaylistDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportLibraryResult {
    pub file_path: String,
    pub tracks_count: usize,
    pub albums_count: usize,
    pub artists_count: usize,
    pub playlists_count: usize,
    pub file_size_bytes: u64,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLibraryResult {
    pub tracks_imported: i64,
    pub albums_imported: i64,
    pub artists_imported: i64,
    pub playlists_imported: i64,
    pub favorites_restored: i64,
    pub message: String,
}

/// Calculate SHA-256 hex string of bytes
pub fn compute_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Export the full library into a portable, versioned backup manifest JSON file
#[tauri::command]
pub async fn export_library(
    state: State<'_, AppState>,
    output_path: Option<String>,
) -> Result<ExportLibraryResult, String> {
    // 1. Query all artists
    let artists_rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT name, favorite_at FROM artists ORDER BY name ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to export artists: {}", e))?;

    let backup_artists: Vec<BackupArtistDto> = artists_rows
        .into_iter()
        .map(|(name, favorite_at)| BackupArtistDto { name, favorite_at })
        .collect();

    // 2. Query all albums
    let albums_rows: Vec<(String, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT a.title, art.name, a.upc, a.release_date, a.favorite_at
        FROM albums a
        LEFT JOIN album_artists aa ON aa.album_id = a.id
        LEFT JOIN artists art ON art.id = aa.artist_id
        ORDER BY a.title ASC
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to export albums: {}", e))?;

    let backup_albums: Vec<BackupAlbumDto> = albums_rows
        .into_iter()
        .map(|(title, artist, upc, release_date, favorite_at)| BackupAlbumDto {
            title,
            artist: artist.unwrap_or_else(|| "Unknown Artist".to_string()),
            upc,
            release_date,
            favorite_at,
        })
        .collect();

    // 3. Query all tracks with sources & downloads
    let tracks_rows: Vec<(
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<i32>,
        Option<i64>,
        Option<i32>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT 
            t.isrc,
            t.title,
            COALESCE(art.name, 'Unknown Artist') as artist,
            a.title as album,
            t.track_number,
            t.disc_number,
            t.duration_ms,
            t.explicit,
            t.favorite_at,
            s.name as service,
            ts.service_track_id,
            d.file_format as downloaded_format
        FROM tracks t
        LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        LEFT JOIN artists art ON art.id = ta.artist_id
        LEFT JOIN albums a ON a.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN downloads d ON d.track_id = t.id
        ORDER BY t.title ASC
        "#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to export tracks: {}", e))?;

    let backup_tracks: Vec<BackupTrackDto> = tracks_rows
        .into_iter()
        .map(|(isrc, title, artist, album, track_number, disc_number, duration_ms, explicit, favorite_at, service, service_track_id, downloaded_format)| {
            BackupTrackDto {
                isrc,
                title,
                artist: artist.unwrap_or_else(|| "Unknown Artist".to_string()),
                album,
                track_number,
                disc_number,
                duration_ms,
                explicit,
                favorite_at,
                service,
                service_track_id,
                downloaded_format,
            }
        })
        .collect();

    // 4. Query playlists
    let playlists_rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, description FROM playlists ORDER BY name ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to export playlists: {}", e))?;

    let mut backup_playlists: Vec<BackupPlaylistDto> = Vec::new();
    for (pid, name, description) in playlists_rows {
        let isrcs: Vec<(Option<String>,)> = sqlx::query_as(
            r#"
            SELECT t.isrc
            FROM playlist_tracks pt
            JOIN tracks t ON t.id = pt.track_id
            WHERE pt.playlist_id = ?
            ORDER BY pt.position ASC
            "#
        )
        .bind(pid)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let track_isrcs: Vec<String> = isrcs.into_iter().filter_map(|(i,)| i).collect();
        backup_playlists.push(BackupPlaylistDto {
            name,
            description,
            track_isrcs,
        });
    }

    let tracks_count = backup_tracks.len();
    let albums_count = backup_albums.len();
    let artists_count = backup_artists.len();
    let playlists_count = backup_playlists.len();

    let mut manifest = LibraryBackupManifest {
        version: "1.0.0".to_string(),
        schema_version: 47,
        exported_at: chrono::Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        checksum_sha256: None,
        tracks: backup_tracks,
        albums: backup_albums,
        artists: backup_artists,
        playlists: backup_playlists,
    };

    // Serialize to JSON without checksum to compute hash
    let raw_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Serialization error: {}", e))?;
    let checksum = compute_sha256_hex(raw_json.as_bytes());
    manifest.checksum_sha256 = Some(checksum.clone());

    // Serialize final with checksum
    let final_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Serialization error: {}", e))?;

    let dest_path = match output_path {
        Some(p) if !p.trim().is_empty() => std::path::PathBuf::from(p),
        _ => {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let filename = format!("Syncify_Backup_{}.json", timestamp);
            dirs::download_dir()
                .or_else(dirs::audio_dir)
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(filename)
        }
    };

    if let Some(parent) = dest_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::write(&dest_path, &final_json)
        .map_err(|e| format!("Failed to write backup file to {}: {}", dest_path.display(), e))?;

    let file_size_bytes = std::fs::metadata(&dest_path)
        .map(|m| m.len())
        .unwrap_or(final_json.len() as u64);

    Ok(ExportLibraryResult {
        file_path: dest_path.to_string_lossy().to_string(),
        tracks_count,
        albums_count,
        artists_count,
        playlists_count,
        file_size_bytes,
        checksum,
    })
}

/// Import a backup manifest into the database with validation and atomic rollback
#[tauri::command]
pub async fn import_library(
    state: State<'_, AppState>,
    file_path: String,
    ignore_checksum_error: Option<bool>,
) -> Result<ImportLibraryResult, String> {
    let p = std::path::Path::new(&file_path);
    if !p.exists() {
        return Err(format!("Backup file does not exist: {}", file_path));
    }

    let content = std::fs::read_to_string(p)
        .map_err(|e| format!("Failed to read backup file: {}", e))?;

    let manifest: LibraryBackupManifest = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid backup manifest format: {}", e))?;

    // Checksum verification (if provided)
    if let Some(expected_checksum) = &manifest.checksum_sha256 {
        // Recompute on manifest without checksum field
        let mut cloned = manifest.clone();
        cloned.checksum_sha256 = None;
        if let Ok(raw) = serde_json::to_string_pretty(&cloned) {
            let computed = compute_sha256_hex(raw.as_bytes());
            if computed != *expected_checksum && !ignore_checksum_error.unwrap_or(false) {
                return Err(format!(
                    "Backup file checksum mismatch (expected {}, got {}). File may be corrupted or modified.",
                    expected_checksum, computed
                ));
            }
        }
    }

    // Atomic SQLite Transaction
    let mut tx = state.db.begin().await
        .map_err(|e| format!("Failed to start database transaction: {}", e))?;

    let mut artists_imported = 0i64;
    let mut albums_imported = 0i64;
    let mut tracks_imported = 0i64;
    let mut playlists_imported = 0i64;
    let mut favorites_restored = 0i64;

    // 1. Import Artists
    for artist in &manifest.artists {
        let res = sqlx::query(
            r#"
            INSERT INTO artists (name, favorite_at)
            VALUES (?, ?)
            ON CONFLICT(name) DO UPDATE SET
                favorite_at = COALESCE(excluded.favorite_at, artists.favorite_at)
            "#
        )
        .bind(&artist.name)
        .bind(&artist.favorite_at)
        .execute(&mut *tx)
        .await;

        if res.is_ok() {
            artists_imported += 1;
            if artist.favorite_at.is_some() {
                favorites_restored += 1;
            }
        }
    }

    // 2. Import Albums
    for album in &manifest.albums {
        // Ensure artist exists
        let artist_id: i64 = match sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = ?")
            .bind(&album.artist)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten()
        {
            Some(id) => id,
            None => {
                sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                    .bind(&album.artist)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to create artist {}: {}", album.artist, e))?
            }
        };

        let album_id: i64 = match sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE title = ?")
            .bind(&album.title)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten()
        {
            Some(id) => {
                if let Some(fav) = &album.favorite_at {
                    let _ = sqlx::query("UPDATE albums SET favorite_at = ? WHERE id = ?")
                        .bind(fav)
                        .bind(id)
                        .execute(&mut *tx)
                        .await;
                }
                id
            }
            None => {
                sqlx::query_scalar(
                    "INSERT INTO albums (title, upc, release_date, favorite_at) VALUES (?, ?, ?, ?) RETURNING id"
                )
                .bind(&album.title)
                .bind(&album.upc)
                .bind(&album.release_date)
                .bind(&album.favorite_at)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert album {}: {}", album.title, e))?
            }
        };

        // Link album_artists
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)"
        )
        .bind(album_id)
        .bind(artist_id)
        .execute(&mut *tx)
        .await;

        albums_imported += 1;
        if album.favorite_at.is_some() {
            favorites_restored += 1;
        }
    }

    // 3. Import Tracks
    for track in &manifest.tracks {
        // Resolve artist
        let artist_id: i64 = match sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = ?")
            .bind(&track.artist)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten()
        {
            Some(id) => id,
            None => {
                sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                    .bind(&track.artist)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to create artist {}: {}", track.artist, e))?
            }
        };

        // Resolve album if any
        let album_id: Option<i64> = if let Some(alb_name) = &track.album {
            sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE title = ?")
                .bind(alb_name)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        // Upsert track (deduplicating by ISRC if present, otherwise by title & album_id)
        let track_id: i64 = if let Some(isrc_val) = &track.isrc {
            match sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc_val)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
            {
                Some(existing_id) => {
                    if let Some(fav) = &track.favorite_at {
                        let _ = sqlx::query("UPDATE tracks SET favorite_at = ? WHERE id = ?")
                            .bind(fav)
                            .bind(existing_id)
                            .execute(&mut *tx)
                            .await;
                    }
                    existing_id
                }
                None => {
                    sqlx::query_scalar(
                        r#"
                        INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, explicit, favorite_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                        RETURNING id
                        "#
                    )
                    .bind(&track.title)
                    .bind(album_id)
                    .bind(track.duration_ms)
                    .bind(track.track_number)
                    .bind(track.disc_number.unwrap_or(1))
                    .bind(&track.isrc)
                    .bind(track.explicit.unwrap_or(0))
                    .bind(&track.favorite_at)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert track {}: {}", track.title, e))?
                }
            }
        } else {
            match sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE title = ? AND (album_id = ? OR (album_id IS NULL AND ? IS NULL))")
                .bind(&track.title)
                .bind(album_id)
                .bind(album_id)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
            {
                Some(existing_id) => {
                    if let Some(fav) = &track.favorite_at {
                        let _ = sqlx::query("UPDATE tracks SET favorite_at = ? WHERE id = ?")
                            .bind(fav)
                            .bind(existing_id)
                            .execute(&mut *tx)
                            .await;
                    }
                    existing_id
                }
                None => {
                    sqlx::query_scalar(
                        r#"
                        INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, explicit, favorite_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                        RETURNING id
                        "#
                    )
                    .bind(&track.title)
                    .bind(album_id)
                    .bind(track.duration_ms)
                    .bind(track.track_number)
                    .bind(track.disc_number.unwrap_or(1))
                    .bind(&track.isrc)
                    .bind(track.explicit.unwrap_or(0))
                    .bind(&track.favorite_at)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert track {}: {}", track.title, e))?
                }
            }
        };

        // Link track_artists
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(&mut *tx)
        .await;

        tracks_imported += 1;
        if track.favorite_at.is_some() {
            favorites_restored += 1;
        }
    }

    // 4. Import Playlists
    for pl in &manifest.playlists {
        // Find or create playlist
        let pl_id: i64 = sqlx::query_scalar(
            "INSERT INTO playlists (account_id, name, description) VALUES (1, ?, ?) RETURNING id"
        )
        .bind(&pl.name)
        .bind(&pl.description)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Failed to create playlist {}: {}", pl.name, e))?;

        for (pos, isrc) in pl.track_isrcs.iter().enumerate() {
            if let Some(tid) = sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
            {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
                )
                .bind(pl_id)
                .bind(tid)
                .bind(pos as i64)
                .execute(&mut *tx)
                .await;
            }
        }

        playlists_imported += 1;
    }

    tx.commit().await
        .map_err(|e| format!("Failed to commit import transaction: {}", e))?;

    Ok(ImportLibraryResult {
        tracks_imported,
        albums_imported,
        artists_imported,
        playlists_imported,
        favorites_restored,
        message: format!(
            "Successfully imported {} tracks, {} albums, {} artists, {} playlists with {} favorites restored",
            tracks_imported, albums_imported, artists_imported, playlists_imported, favorites_restored
        ),
    })
}
