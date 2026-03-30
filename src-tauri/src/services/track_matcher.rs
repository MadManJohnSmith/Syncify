//! Track matching and deduplication utilities
//!
//! Provides shared logic for finding or creating tracks using ISRC as primary key.
//! These utilities are designed to be used by service importers for consistent track handling.

#![allow(dead_code)] // Public API for service importers - will be used when importers are refactored

use sqlx::SqlitePool;

/// Result of a track lookup or creation
#[derive(Debug, Clone)]
pub struct TrackMatch {
    pub track_id: i64,
    pub is_new: bool,
}

/// Find an existing track by ISRC or create a new one
///
/// This implements the ISRC-first matching pattern used across all services:
/// 1. If ISRC is provided, search for existing track with that ISRC
/// 2. If found, optionally update missing fields (album_id)
/// 3. If not found, create a new track
///
/// # Arguments
/// * `db` - Database connection pool
/// * `title` - Track title
/// * `isrc` - Optional ISRC code (primary matching key)
/// * `album_id` - Optional album ID to associate
/// * `duration_ms` - Track duration in milliseconds
/// * `explicit` - Whether track has explicit content
pub async fn find_or_create_track(
    db: &SqlitePool,
    title: &str,
    isrc: Option<&str>,
    album_id: Option<i64>,
    duration_ms: Option<i64>,
    explicit: Option<bool>,
) -> Result<TrackMatch, String> {
    // Step 1: Try to find by ISRC (most reliable)
    if let Some(isrc) = isrc {
        if let Ok(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE isrc = ?")
            .bind(isrc)
            .fetch_one(db)
            .await
        {
            // Update album_id if not set and we have one
            if let Some(album_id) = album_id {
                let _ =
                    sqlx::query("UPDATE tracks SET album_id = ? WHERE id = ? AND album_id IS NULL")
                        .bind(album_id)
                        .bind(row.0)
                        .execute(db)
                        .await;
            }

            tracing::debug!("Found existing track by ISRC {}: id={}", isrc, row.0);
            return Ok(TrackMatch {
                track_id: row.0,
                is_new: false,
            });
        }
    }

    // Step 2: Create new track
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc, explicit) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(title)
    .bind(album_id)
    .bind(duration_ms)
    .bind(isrc)
    .bind(explicit)
    .fetch_one(db)
    .await
    .map_err(|e| format!("Failed to insert track: {}", e))?;

    tracing::debug!("Created new track: id={}, title={}", track_id, title);

    Ok(TrackMatch {
        track_id,
        is_new: true,
    })
}

/// Link an artist to a track
pub async fn link_track_artist(
    db: &SqlitePool,
    track_id: i64,
    artist_id: i64,
    role: &str,
) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, ?)")
        .bind(track_id)
        .bind(artist_id)
        .bind(role)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to link artist: {}", e))?;

    Ok(())
}

/// Add a library entry for a track (favorite/liked)
pub async fn add_library_entry(
    db: &SqlitePool,
    account_id: i64,
    track_id: i64,
    is_liked: bool,
) -> Result<bool, String> {
    let result = sqlx::query(
        "INSERT OR IGNORE INTO library_entries (account_id, track_id, is_liked) VALUES (?, ?, ?)",
    )
    .bind(account_id)
    .bind(track_id)
    .bind(is_liked)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to add library entry: {}", e))?;

    Ok(result.rows_affected() > 0)
}

/// Add a track source (service-specific availability info)
pub async fn add_track_source(
    db: &SqlitePool,
    track_id: i64,
    service_id: i64,
    service_track_id: &str,
    format: Option<&str>,
    bit_depth: Option<i32>,
    sample_rate: Option<i32>,
    quality_score: i32,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT OR REPLACE INTO track_sources 
        (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) 
        VALUES (?, ?, ?, ?, ?, ?, ?, 1)
        "#
    )
    .bind(track_id)
    .bind(service_id)
    .bind(service_track_id)
    .bind(format)
    .bind(bit_depth)
    .bind(sample_rate)
    .bind(quality_score)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to add track source: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    // Integration tests would require a test database
    // See migration_tests.rs for examples
}
