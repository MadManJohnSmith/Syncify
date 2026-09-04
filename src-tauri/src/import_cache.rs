//! Import optimization utilities
//!
//! Provides caching and batch transaction support for efficient imports.

use sqlx::SqlitePool;
use std::collections::HashMap;

/// Cache for artist and album IDs during import
/// Reduces redundant DB lookups when multiple tracks share artists/albums
#[derive(Default)]
pub struct ImportCache {
    artists: HashMap<String, i64>,
    albums: HashMap<String, i64>,
    service_ids: HashMap<String, i64>,
}

impl ImportCache {
    pub fn new() -> Self {
        Self::default()
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

    /// Get or create album with caching - fully lock-free
    /// Uses INSERT OR IGNORE + SELECT pattern to handle race conditions via database
    pub async fn get_or_create_album(
        &mut self,
        db: &SqlitePool,
        _album_lock: &tokio::sync::Mutex<()>, // Kept for API compatibility but not used
        album_key: &str,                      // Use "artist_id:album_name" as key
        album_name: &str,
        primary_artist_id: i64,
        release_date: Option<&str>,
        image_url: Option<&str>,
    ) -> Result<i64, String> {
        // Check cache first
        if let Some(&id) = self.albums.get(album_key) {
            return Ok(id);
        }

        // Try to find existing album with this title by this artist
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT a.id FROM albums a 
             JOIN album_artists aa ON aa.album_id = a.id 
             WHERE LOWER(a.title) = LOWER(?) AND aa.artist_id = ? AND aa.is_primary = 1",
        )
        .bind(album_name)
        .bind(primary_artist_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        let id = if let Some((id,)) = existing {
            id
        } else {
            // Try to insert (may fail if another task just created it - that's OK)
            let insert_result = sqlx::query(
                "INSERT OR IGNORE INTO albums (title, release_date, cover_art_url) VALUES (?, ?, ?)"
            )
            .bind(album_name)
            .bind(release_date)
            .bind(image_url)
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
            .bind(album_name)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Failed to get album ID: {}", e))?;

            let id = match result {
                Some((id,)) => id,
                None => {
                    // Very rare: album not found after insert attempt - retry with fresh insert
                    sqlx::query(
                        "INSERT INTO albums (title, release_date, cover_art_url) VALUES (?, ?, ?)",
                    )
                    .bind(album_name)
                    .bind(release_date)
                    .bind(image_url)
                    .execute(db)
                    .await
                    .map_err(|e| format!("Failed to create album (retry): {}", e))?;

                    let (id,): (i64,) = sqlx::query_as(
                        "SELECT id FROM albums WHERE LOWER(title) = LOWER(?) ORDER BY id DESC LIMIT 1"
                    )
                    .bind(album_name)
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
            .bind(primary_artist_id)
            .execute(db)
            .await;
            // Ignore errors - link might already exist

            id
        };

        // Cache the result
        self.albums.insert(album_key.to_string(), id);
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
}
