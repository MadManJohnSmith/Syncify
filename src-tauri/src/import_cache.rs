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

/// Ítem 22: same-title album candidates for one edition identity — the album
/// linked to the (normalized) artist, or any compilation when importing under
/// Various Artists. Ordered most-complete first: primary link, imported track
/// count, total duration, oldest id last, so the caller can reuse the row that
/// best describes the edition instead of an arbitrary one.
async fn find_album_candidates(
    db: &SqlitePool,
    title: &str,
    artist_id: i64,
    effective_is_compilation: bool,
) -> Result<Vec<(i64,)>, String> {
    if effective_is_compilation {
        sqlx::query_as(
            "SELECT a.id
             FROM albums a
             JOIN album_artists aa ON aa.album_id = a.id
             WHERE LOWER(a.title) = LOWER(?) AND (aa.artist_id = ? OR a.is_compilation = 1)
             GROUP BY a.id
             ORDER BY MAX(a.is_compilation) DESC,
                      (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = a.id) DESC,
                      (SELECT COALESCE(SUM(duration_ms), 0) FROM tracks WHERE tracks.album_id = a.id) DESC,
                      a.id ASC",
        )
        .bind(title)
        .bind(artist_id)
        .fetch_all(db)
        .await
        .map_err(|e| format!("DB error: {}", e))
    } else {
        sqlx::query_as(
            "SELECT a.id
             FROM albums a
             JOIN album_artists aa ON aa.album_id = a.id
             WHERE LOWER(a.title) = LOWER(?) AND aa.artist_id = ?
             GROUP BY a.id
             ORDER BY MAX(aa.is_primary) DESC,
                      (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = a.id) DESC,
                      (SELECT COALESCE(SUM(duration_ms), 0) FROM tracks WHERE tracks.album_id = a.id) DESC,
                      a.id ASC",
        )
        .bind(title)
        .bind(artist_id)
        .fetch_all(db)
        .await
        .map_err(|e| format!("DB error: {}", e))
    }
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
        self.artists.insert(
            syncify_core_domain::metadata::CANONICAL_VARIOUS_ARTISTS.to_string(),
            id,
        );
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
        } else {
            self.various_artists_id
                .filter(|&known_va| primary_artist_id == known_va)
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
                self.albums
                    .get(&format!("va:{}", clean_name.to_lowercase()))
            } else {
                self.albums.get(album_key)
            }
        }) {
            return Ok(id);
        }

        // Ítem 22: match the album by edition identity (title normalized + artist
        // link), not by title alone. When several candidates share the normalized
        // title, the tie is broken by how complete each row is — imported track
        // count, then total duration — and the tie is registered in the log,
        // keeping every candidate row intact (no data merging here).
        let candidates = find_album_candidates(
            db,
            &clean_name,
            effective_artist_id,
            effective_is_compilation,
        )
        .await?;
        if candidates.len() > 1 {
            tracing::warn!(
                album_title = %clean_name,
                artist_id = effective_artist_id,
                candidates = candidates.len(),
                chosen_album_id = candidates[0].0,
                "Album title collision for the same edition identity: resolved by imported track count and total duration; all candidate rows kept, tie recorded in this log entry"
            );
        }

        let is_comp_flag = if effective_is_compilation { 1 } else { 0 };

        let id = if let Some(&(id,)) = candidates.first() {
            if effective_is_compilation {
                let _ = sqlx::query(
                    "UPDATE albums SET is_compilation = 1 WHERE id = ? AND is_compilation != 1",
                )
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
            // Create the row and read back OUR id via RETURNING: the old
            // "INSERT OR IGNORE + SELECT newest row with this title" could land
            // on another artist's same-titled album under concurrency.
            let mut created = sqlx::query_scalar::<_, i64>(
                "INSERT INTO albums (title, release_date, cover_art_url, is_compilation) VALUES (?, ?, ?, ?) RETURNING id",
            )
            .bind(&clean_name)
            .bind(release_date)
            .bind(image_url)
            .bind(is_comp_flag)
            .fetch_one(db)
            .await;

            if created.is_err() {
                // Concurrent import may have created the row between the
                // candidate lookup and this insert: re-run the match once
                // before duplicating the row.
                match find_album_candidates(
                    db,
                    &clean_name,
                    effective_artist_id,
                    effective_is_compilation,
                )
                .await
                {
                    Ok(retry) if !retry.is_empty() => {
                        created = Ok(retry[0].0);
                    }
                    _ => {
                        created = sqlx::query_scalar::<_, i64>(
                            "INSERT INTO albums (title, release_date, cover_art_url, is_compilation) VALUES (?, ?, ?, ?) RETURNING id",
                        )
                        .bind(&clean_name)
                        .bind(release_date)
                        .bind(image_url)
                        .bind(is_comp_flag)
                        .fetch_one(db)
                        .await;
                    }
                }
            }

            let id =
                created.map_err(|e| format!("Failed to create album '{}': {}", clean_name, e))?;

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
            self.albums
                .insert(format!("va:{}", clean_name.to_lowercase()), id);
            self.albums.insert(
                format!("{}:{}", effective_artist_id, clean_name.to_lowercase()),
                id,
            );
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
}

/// Helper to sanitize track title and purge redundant remaster suffixes when the album title declares a remaster edition.
#[allow(dead_code)] // Cubierta por `tests/redundant_remaster_strip_test.rs`.
pub fn process_track_title(track_title: &str, album_title: Option<&str>) -> String {
    let clean_title = syncify_core_domain::metadata::sanitize_track_title(track_title);
    if let Some(album) = album_title {
        syncify_core_domain::metadata::strip_redundant_remaster(&clean_title, album)
    } else {
        clean_title
    }
}
