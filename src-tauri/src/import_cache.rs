//! Import optimization utilities
//!
//! Provides caching and batch transaction support for efficient imports.

use sqlx::SqlitePool;
use std::collections::HashMap;

pub const CANONICAL_VARIOUS_ARTISTS_ID: i64 = 30698;

/// Resolves or creates the canonical "Various Artists" artist record in the database using a connection.
pub async fn get_or_create_canonical_various_artists_conn(
    conn: &mut sqlx::SqliteConnection,
) -> Result<i64, String> {
    // 1. First check if "Various Artists" already exists (case-insensitive)
    if let Ok(Some((id,))) = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' ORDER BY id ASC LIMIT 1",
    )
    .fetch_optional(&mut *conn)
    .await
    {
        return Ok(id);
    }

    // 2. Try inserting with canonical ID 30698
    let id_opt: Option<i64> = sqlx::query_scalar(
        "INSERT INTO artists (id, name) VALUES (?, 'Various Artists')
         ON CONFLICT(id) DO UPDATE SET name = excluded.name
         RETURNING id",
    )
    .bind(CANONICAL_VARIOUS_ARTISTS_ID)
    .fetch_optional(&mut *conn)
    .await
    .ok()
    .flatten();

    if let Some(id) = id_opt {
        return Ok(id);
    }

    // 3. Fallback standard insert
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Various Artists')
         ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id = id
         RETURNING id",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| format!("Failed to get/create Various Artists: {}", e))?;

    Ok(id)
}

/// Resolves or creates the canonical "Various Artists" artist record in the database pool.
pub async fn get_or_create_canonical_various_artists(db: &SqlitePool) -> Result<i64, String> {
    let mut conn = db.acquire().await.map_err(|e| e.to_string())?;
    get_or_create_canonical_various_artists_conn(&mut conn).await
}

/// Cache for artist and album IDs during import
/// Reduces redundant DB lookups when multiple tracks share artists/albums
#[derive(Default)]
pub struct ImportCache {
    artists: HashMap<String, i64>,
    albums: HashMap<String, i64>,
    service_ids: HashMap<String, i64>,
    various_artists_id: Option<i64>,
}

impl ImportCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolves canonical Various Artists ID with caching
    pub async fn get_or_create_various_artists(&mut self, db: &SqlitePool) -> Result<i64, String> {
        if let Some(id) = self.various_artists_id {
            return Ok(id);
        }
        let id = get_or_create_canonical_various_artists(db).await?;
        self.various_artists_id = Some(id);
        self.artists.insert(syncify_core_domain::metadata::CANONICAL_VARIOUS_ARTISTS.to_string(), id);
        self.artists.insert("various artists".to_string(), id);
        Ok(id)
    }

    /// Get or create artist with caching
    pub async fn get_or_create_artist(
        &mut self,
        db: &SqlitePool,
        name: &str,
    ) -> Result<i64, String> {
        let clean_name = syncify_core_domain::metadata::sanitize_artist_name(name);
        if clean_name.is_empty() {
            return Err("Cannot create artist with empty name".to_string());
        }
        // Check cache first
        if let Some(&id) = self.artists.get(&clean_name) {
            return Ok(id);
        }

        // Try to find existing (case-insensitive)
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM artists WHERE LOWER(name) = LOWER(?)")
                .bind(&clean_name)
                .fetch_optional(db)
                .await
                .map_err(|e| format!("DB error: {}", e))?;

        let id = if let Some((id,)) = existing {
            id
        } else {
            // Create new (use INSERT OR IGNORE in case of race condition)
            let _ = sqlx::query("INSERT OR IGNORE INTO artists (name) VALUES (?)")
                .bind(&clean_name)
                .execute(db)
                .await;

            // Always SELECT to get the ID (handles both new insert and race condition)
            let (id,): (i64,) =
                sqlx::query_as("SELECT id FROM artists WHERE LOWER(name) = LOWER(?)")
                    .bind(&clean_name)
                    .fetch_one(db)
                    .await
                    .map_err(|e| format!("Failed to get artist ID for '{}': {}", clean_name, e))?;
            id
        };

        // Cache the result
        self.artists.insert(clean_name, id);
        Ok(id)
    }

    /// Get or create album with caching - delegates to get_or_create_album_with_compilation
    #[allow(dead_code)]
    pub async fn get_or_create_album(
        &mut self,
        db: &SqlitePool,
        album_key: &str,                      // Use "artist_id:album_name" as key
        album_name: &str,
        primary_artist_id: i64,
        release_date: Option<&str>,
        image_url: Option<&str>,
    ) -> Result<i64, String> {
        self.get_or_create_album_with_compilation(
            db,
            album_key,
            album_name,
            primary_artist_id,
            release_date,
            image_url,
            false,
        )
        .await
    }

    /// Get or create album with compilation detection and caching - fully lock-free.
    /// If `is_compilation` is true (or artist is Various Artists), assigns album_artist to
    /// canonical Various Artists (id 30698), marks `albums.is_compilation = 1`, and deduplicates
    /// across all tracks sharing the normalized title under Various Artists.
    pub async fn get_or_create_album_with_compilation(
        &mut self,
        db: &SqlitePool,
        album_key: &str,
        album_name: &str,
        primary_artist_id: i64,
        release_date: Option<&str>,
        image_url: Option<&str>,
        is_compilation: bool,
    ) -> Result<i64, String> {
        let clean_name = syncify_core_domain::metadata::sanitize_album_title(album_name);
        if clean_name.is_empty() {
            return Err("Cannot create album with empty title".to_string());
        }

        let va_id = if is_compilation {
            Some(self.get_or_create_various_artists(db).await?)
        } else if let Some(known_va) = self.various_artists_id {
            if primary_artist_id == known_va {
                Some(known_va)
            } else {
                None
            }
        } else {
            None
        };

        let effective_is_compilation = is_compilation || va_id.is_some();
        let effective_artist_id = va_id.unwrap_or(primary_artist_id);
        let canonical_key = if effective_is_compilation {
            format!("va:{}", clean_name.to_lowercase())
        } else {
            format!("{}:{}", primary_artist_id, clean_name.to_lowercase())
        };

        // Check cache first
        if let Some(&id) = self.albums.get(&canonical_key).or_else(|| {
            if effective_is_compilation {
                self.albums.get(&format!("va:{}", clean_name.to_lowercase()))
            } else {
                self.albums.get(album_key)
            }
        }) {
            return Ok(id);
        }

        // Try to find existing album
        let existing: Option<(i64,)> = if effective_is_compilation {
            sqlx::query_as(
                "SELECT a.id FROM albums a 
                 JOIN album_artists aa ON aa.album_id = a.id 
                 WHERE LOWER(a.title) = LOWER(?) AND (aa.artist_id = ? OR a.is_compilation = 1)
                 ORDER BY a.is_compilation DESC, a.total_tracks DESC, a.id ASC LIMIT 1",
            )
            .bind(&clean_name)
            .bind(effective_artist_id)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        } else {
            sqlx::query_as(
                "SELECT a.id FROM albums a 
                 JOIN album_artists aa ON aa.album_id = a.id 
                 WHERE LOWER(a.title) = LOWER(?) AND aa.artist_id = ? AND aa.is_primary = 1",
            )
            .bind(&clean_name)
            .bind(primary_artist_id)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("DB error: {}", e))?
        };

        let is_comp_flag = if effective_is_compilation { 1 } else { 0 };

        let id = if let Some((id,)) = existing {
            if effective_is_compilation {
                let _ = sqlx::query("UPDATE albums SET is_compilation = 1 WHERE id = ? AND is_compilation != 1")
                    .bind(id)
                    .execute(db)
                    .await;
                let _ = sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
                    .bind(id)
                    .bind(effective_artist_id)
                    .execute(db)
                    .await;
            }
            id
        } else {
            // Try to insert (may fail if another task just created it - that's OK)
            let insert_result = sqlx::query(
                "INSERT OR IGNORE INTO albums (title, release_date, cover_art_url, is_compilation) VALUES (?, ?, ?, ?)"
            )
            .bind(&clean_name)
            .bind(release_date)
            .bind(image_url)
            .bind(is_comp_flag)
            .execute(db)
            .await;

            // Ignore insert errors - we'll SELECT anyway
            if let Err(e) = &insert_result {
                tracing::debug!("Album insert (race condition OK): {}", e);
            }

            // Get the album ID (works whether we just created it or another task did)
            let result: Option<(i64,)> = sqlx::query_as(
                "SELECT id FROM albums WHERE LOWER(title) = LOWER(?) ORDER BY id DESC LIMIT 1",
            )
            .bind(&clean_name)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Failed to get album ID: {}", e))?;

            let id = match result {
                Some((id,)) => id,
                None => {
                    // Very rare: album not found after insert attempt - retry with fresh insert
                    sqlx::query(
                        "INSERT INTO albums (title, release_date, cover_art_url, is_compilation) VALUES (?, ?, ?, ?)",
                    )
                    .bind(&clean_name)
                    .bind(release_date)
                    .bind(image_url)
                    .bind(is_comp_flag)
                    .execute(db)
                    .await
                    .map_err(|e| format!("Failed to create album (retry): {}", e))?;

                    let (id,): (i64,) = sqlx::query_as(
                        "SELECT id FROM albums WHERE LOWER(title) = LOWER(?) ORDER BY id DESC LIMIT 1"
                    )
                    .bind(&clean_name)
                    .fetch_one(db)
                    .await
                    .map_err(|e| format!("Failed to get album ID (retry): {}", e))?;
                    id
                }
            };

            // Link to primary artist (INSERT OR IGNORE handles duplicates)
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)"
            )
            .bind(id)
            .bind(effective_artist_id)
            .execute(db)
            .await;
            // Ignore errors - link might already exist

            id
        };

        // Cache the result under canonical key and VA alias
        self.albums.insert(canonical_key.clone(), id);
        if effective_is_compilation {
            self.albums.insert(format!("va:{}", clean_name.to_lowercase()), id);
            self.albums.insert(format!("{}:{}", effective_artist_id, clean_name.to_lowercase()), id);
        }
        if !album_key.is_empty() {
            self.albums.insert(album_key.to_string(), id);
        }
        Ok(id)
    }

    /// Get service ID with caching
    pub async fn get_service_id(&mut self, db: &SqlitePool, name: &str) -> Result<i64, String> {
        if let Some(&id) = self.service_ids.get(name) {
            return Ok(id);
        }

        let (id,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = ?")
            .bind(name)
            .fetch_one(db)
            .await
            .map_err(|e| format!("Service not found: {}", e))?;

        self.service_ids.insert(name.to_string(), id);
        Ok(id)
    }

    /// Get cache statistics (for logging)
    pub fn stats(&self) -> (usize, usize) {
        (self.artists.len(), self.albums.len())
    }

    /// Sanitizes and strips redundant remaster suffixes from track title if album declares remaster
    #[allow(dead_code)]
    pub fn clean_track_title(&self, track_title: &str, album_title: Option<&str>) -> String {
        process_track_title(track_title, album_title)
    }
}

/// Helper to sanitize track title and purge redundant remaster suffixes when the album title declares a remaster edition.
#[allow(dead_code)]
pub fn process_track_title(track_title: &str, album_title: Option<&str>) -> String {
    let clean_title = syncify_core_domain::metadata::sanitize_track_title(track_title);
    if let Some(album) = album_title {
        syncify_core_domain::metadata::strip_redundant_remaster(&clean_title, album)
    } else {
        clean_title
    }
}
