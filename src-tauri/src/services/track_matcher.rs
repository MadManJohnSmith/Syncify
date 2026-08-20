//! Track matching and deduplication utilities
//!
//! Provides shared logic for finding or creating tracks using ISRC as primary key.
//! These utilities are designed to be used by service importers for consistent track handling.

#![allow(dead_code)] // Public API for service importers - will be used when importers are refactored

use sqlx::SqlitePool;
use syncify_core_domain::metadata::{is_placeholder_title, is_valid_isrc, ProviderTrackIdentity};

/// Result of a track lookup or creation
#[derive(Debug, Clone)]
pub struct TrackMatch {
    pub track_id: i64,
    pub is_new: bool,
}

/// Find an existing track by (service_id, service_track_id) first, then valid ISRC, or create a new one.
pub async fn find_or_create_track_with_identity(
    db: &SqlitePool,
    identity: &ProviderTrackIdentity,
    album_id: Option<i64>,
) -> Result<TrackMatch, String> {
    // Step 1: Check existing source mapping by (service_id, service_track_id)
    if let Ok(Some((existing_id,))) = sqlx::query_as::<_, (i64,)>(
        "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = ? LIMIT 1"
    )
    .bind(identity.service_id)
    .bind(&identity.service_track_id)
    .fetch_optional(db)
    .await {
        if let Some(alb_id) = album_id {
            let _ = sqlx::query("UPDATE tracks SET album_id = COALESCE(album_id, ?) WHERE id = ?")
                .bind(alb_id)
                .bind(existing_id)
                .execute(db)
                .await;
        }
        return Ok(TrackMatch { track_id: existing_id, is_new: false });
    }

    // Step 2: Try to find by validated ISRC (never numeric IDs)
    if let Some(valid_isrc) = identity.sanitized_isrc() {
        if let Ok(Some((existing_id,))) = sqlx::query_as::<_, (i64,)>(
            "SELECT id FROM tracks WHERE isrc = ? LIMIT 1"
        )
        .bind(&valid_isrc)
        .fetch_optional(db)
        .await {
            if let Some(alb_id) = album_id {
                let _ = sqlx::query("UPDATE tracks SET album_id = COALESCE(album_id, ?) WHERE id = ?")
                    .bind(alb_id)
                    .bind(existing_id)
                    .execute(db)
                    .await;
            }
            return Ok(TrackMatch { track_id: existing_id, is_new: false });
        }
    }

    // Step 3: Create new canonical track
    let safe_title = identity.title.as_deref().unwrap_or("Unknown Track");
    let is_placeholder = is_placeholder_title(safe_title);
    let raw_isrc = identity.sanitized_isrc();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc, explicit, track_number, disc_number) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(safe_title)
    .bind(album_id)
    .bind(identity.duration_ms)
    .bind(raw_isrc)
    .bind(identity.explicit.unwrap_or(false))
    .bind(identity.track_number.unwrap_or(1))
    .bind(identity.disc_number.unwrap_or(1))
    .fetch_one(db)
    .await
    .map_err(|e| format!("Failed to insert canonical track: {}", e))?;

    if is_placeholder {
        tracing::warn!("Created track {} with placeholder title '{}' - pending enrichment", track_id, safe_title);
    }

    Ok(TrackMatch {
        track_id,
        is_new: true,
    })
}

/// Legacy wrapper for find_or_create_track preserving backward compatibility while enforcing identity rules
pub async fn find_or_create_track(
    db: &SqlitePool,
    title: &str,
    isrc: Option<&str>,
    album_id: Option<i64>,
    duration_ms: Option<i64>,
    explicit: Option<bool>,
) -> Result<TrackMatch, String> {
    let sanitized_isrc = isrc.and_then(|c| if is_valid_isrc(c) { Some(c.to_string()) } else { None });
    if let Some(ref valid_isrc) = sanitized_isrc {
        if let Ok(Some((existing_id,))) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE isrc = ? LIMIT 1")
            .bind(valid_isrc)
            .fetch_optional(db)
            .await
        {
            if let Some(alb_id) = album_id {
                let _ = sqlx::query("UPDATE tracks SET album_id = COALESCE(album_id, ?) WHERE id = ?")
                    .bind(alb_id)
                    .bind(existing_id)
                    .execute(db)
                    .await;
            }
            return Ok(TrackMatch {
                track_id: existing_id,
                is_new: false,
            });
        }
    }

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc, explicit) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(title)
    .bind(album_id)
    .bind(duration_ms)
    .bind(sanitized_isrc)
    .bind(explicit)
    .fetch_one(db)
    .await
    .map_err(|e| format!("Failed to insert track: {}", e))?;

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
