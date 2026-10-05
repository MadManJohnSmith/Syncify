//! Regression tests for audit items 20 and 21 (BD-2, BD-5), migration 0088.
//!
//! BD-2 — 0086/0087 mirror a track when its service identity or its artist link
//! is written, but nothing mirrored the EDITS of an already mirrored track:
//! `tracks.title`, `tracks.album_id`, `tracks.duration_ms`,
//! `tracks.audio_quality` and `albums.title` corrections left the mirror with
//! the values the track had at import time, and both migration consumers
//! (preview/start migration and search_destination_track) read the mirror.
//!
//! BD-5 — the 0085 triggers overwrote `albums.total_tracks` with COUNT(*) on
//! every track write, so importing 3 of a 20-track release advertised 3. Now
//! `total_tracks` keeps the declared release total and
//! `albums.local_track_count` carries how many of them are imported here.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

/// Seeds services (1 = spotify) and returns an album id.
async fn seed_album(
    pool: &SqlitePool,
    title: &str,
    total_tracks: Option<i64>,
    is_stub: i64,
) -> i64 {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();

    sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(title)
    .bind(total_tracks)
    .bind(is_stub)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Inserts a track with its artist link and its spotify identity, i.e. a track
/// the 0086/0087 triggers mirror into `library_items`.
async fn seed_mirrored_track(pool: &SqlitePool, title: &str, album_id: Option<i64>) -> i64 {
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
        .bind(format!("{} Artist", title))
        .fetch_one(pool)
        .await
        .unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, audio_quality)
         VALUES (?, ?, 200000, 'lossless') RETURNING id",
    )
    .bind(title)
    .bind(album_id)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score)
         VALUES (?, 1, ?, 1, 100)",
    )
    .bind(track_id)
    .bind(format!("sp-{}", track_id))
    .execute(pool)
    .await
    .unwrap();

    track_id
}

async fn mirrored_row(
    pool: &SqlitePool,
    track_id: i64,
) -> Option<(String, String, Option<String>, i64, Option<String>)> {
    sqlx::query_as(
        "SELECT title, artist, album, duration_ms, quality FROM library_items WHERE id = ?",
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn album_totals(pool: &SqlitePool, album_id: i64) -> (Option<i64>, Option<i64>) {
    sqlx::query_as("SELECT total_tracks, local_track_count FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ============================================================================
// BD-2: catalog edits reach the mirror
// ============================================================================

#[tokio::test]
async fn test_track_title_correction_refreshes_the_mirror() {
    let pool = setup_test_db().await;
    let album = seed_album(&pool, "Mirror Album", None, 0).await;
    let track = seed_mirrored_track(&pool, "Old Title", Some(album)).await;

    assert_eq!(
        mirrored_row(&pool, track).await.unwrap().0,
        "Old Title",
        "fixture setup: the mirror starts with the imported title"
    );

    sqlx::query("UPDATE tracks SET title = 'Corrected Title' WHERE id = ?")
        .bind(track)
        .execute(&pool)
        .await
        .unwrap();

    let row = mirrored_row(&pool, track).await.unwrap();
    assert_eq!(
        row.0, "Corrected Title",
        "BD-2: a corrected title must reach library_items"
    );
    assert_eq!(
        row.1, "Old Title Artist",
        "BD-2: the artist link is untouched"
    );
    assert_eq!(row.2.as_deref(), Some("Mirror Album"));
    assert_eq!(row.3, 200000);
    assert_eq!(row.4.as_deref(), Some("lossless"));
}

#[tokio::test]
async fn test_duration_and_quality_corrections_refresh_the_mirror() {
    let pool = setup_test_db().await;
    let track = seed_mirrored_track(&pool, "Corrected Duration", None).await;

    sqlx::query("UPDATE tracks SET duration_ms = 999000, audio_quality = 'hires' WHERE id = ?")
        .bind(track)
        .execute(&pool)
        .await
        .unwrap();

    let row = mirrored_row(&pool, track).await.unwrap();
    assert_eq!(row.3, 999000, "BD-2: duration_ms must reach the mirror");
    assert_eq!(
        row.4.as_deref(),
        Some("hires"),
        "BD-2: audio_quality must reach the mirror"
    );
}

/// 0072 already normalizes `tracks.audio_quality` on the way in, and its own
/// UPDATE trigger runs on the same statement the 0088 mirror listens to: the
/// mirror must carry the canonical value whichever of the two fires first.
#[tokio::test]
async fn test_a_non_canonical_quality_edit_mirrors_the_normalized_value() {
    let pool = setup_test_db().await;
    let track = seed_mirrored_track(&pool, "Quality Order", None).await;

    sqlx::query("UPDATE tracks SET audio_quality = 'HI_RES_LOSSLESS' WHERE id = ?")
        .bind(track)
        .execute(&pool)
        .await
        .unwrap();

    let stored: String = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = ?")
        .bind(track)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "hires", "fixture setup: 0072 normalizes the column");

    assert_eq!(
        mirrored_row(&pool, track).await.unwrap().4.as_deref(),
        Some("hires"),
        "BD-2: the mirror must carry the normalized quality"
    );
}

#[tokio::test]
async fn test_album_rename_and_track_move_refresh_the_mirrored_album() {
    let pool = setup_test_db().await;
    let first = seed_album(&pool, "First Album", None, 0).await;
    let second = seed_album(&pool, "Second Album", None, 0).await;
    let track = seed_mirrored_track(&pool, "Movable", Some(first)).await;

    sqlx::query("UPDATE albums SET title = 'First Album (Remaster)' WHERE id = ?")
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        mirrored_row(&pool, track).await.unwrap().2.as_deref(),
        Some("First Album (Remaster)"),
        "BD-2: an album rename must reach the mirrored album of its tracks"
    );

    sqlx::query("UPDATE tracks SET album_id = ? WHERE id = ?")
        .bind(second)
        .bind(track)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        mirrored_row(&pool, track).await.unwrap().2.as_deref(),
        Some("Second Album"),
        "BD-2: moving a track to another album must republish its album"
    );
}

#[tokio::test]
async fn test_a_track_that_stops_qualifying_loses_its_mirror_row() {
    let pool = setup_test_db().await;
    let track = seed_mirrored_track(&pool, "Temporary", None).await;
    assert!(mirrored_row(&pool, track).await.is_some());

    sqlx::query("UPDATE tracks SET title = '   ' WHERE id = ?")
        .bind(track)
        .execute(&pool)
        .await
        .unwrap();

    assert!(
        mirrored_row(&pool, track).await.is_none(),
        "BD-2: a track without a usable title must not stay mirrored"
    );
}

// ============================================================================
// BD-5: declared release total vs imported track count
// ============================================================================

#[tokio::test]
async fn test_partial_import_keeps_the_declared_release_total() {
    let pool = setup_test_db().await;
    let album = seed_album(&pool, "Twenty Track Album", Some(20), 0).await;

    for n in 1..=3 {
        seed_mirrored_track(&pool, &format!("Track {}", n), Some(album)).await;
    }

    let (total, local) = album_totals(&pool, album).await;
    assert_eq!(
        total,
        Some(20),
        "BD-5: importing 3 of a 20-track release must keep the release total"
    );
    assert_eq!(local, Some(3), "BD-5: the imported count is tracked apart");
}

#[tokio::test]
async fn test_album_without_declared_total_still_derives_it() {
    let pool = setup_test_db().await;
    let album = seed_album(&pool, "Undeclared Album", None, 0).await;

    for n in 1..=3 {
        seed_mirrored_track(&pool, &format!("Undeclared {}", n), Some(album)).await;
    }

    let (total, local) = album_totals(&pool, album).await;
    assert_eq!(
        total,
        Some(3),
        "BD-5: an album that never declared a total still derives it from its tracks"
    );
    assert_eq!(local, Some(3));
}

#[tokio::test]
async fn test_album_with_zero_total_still_derives_it() {
    let pool = setup_test_db().await;
    let album = seed_album(&pool, "Zero Album", Some(0), 0).await;

    for n in 1..=3 {
        seed_mirrored_track(&pool, &format!("Zero {}", n), Some(album)).await;
    }

    let (total, local) = album_totals(&pool, album).await;
    assert_eq!(
        total,
        Some(3),
        "BD-5: zero means 'not declared', the derived total must follow every insert"
    );
    assert_eq!(local, Some(3));
}

#[tokio::test]
async fn test_deleting_a_track_keeps_the_declared_total_and_lowers_the_local_one() {
    let pool = setup_test_db().await;
    let album = seed_album(&pool, "Full Album", Some(2), 0).await;
    let first = seed_mirrored_track(&pool, "Kept", Some(album)).await;
    let second = seed_mirrored_track(&pool, "Removed", Some(album)).await;

    assert_eq!(album_totals(&pool, album).await, (Some(2), Some(2)));

    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(second)
        .execute(&pool)
        .await
        .unwrap();

    let (total, local) = album_totals(&pool, album).await;
    assert_eq!(
        total,
        Some(2),
        "BD-5: deleting an imported track must not shrink the declared release total"
    );
    assert_eq!(
        local,
        Some(1),
        "BD-5: the imported count follows the deletion"
    );

    // Moving the surviving track to another album refreshes both albums.
    let other = seed_album(&pool, "Other Album", None, 0).await;
    sqlx::query("UPDATE tracks SET album_id = ? WHERE id = ?")
        .bind(other)
        .bind(first)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(album_totals(&pool, album).await.1, Some(0));
    assert_eq!(album_totals(&pool, other).await.1, Some(1));
}

#[tokio::test]
async fn test_stub_album_keeps_its_declared_total() {
    let pool = setup_test_db().await;
    let album = seed_album(&pool, "Stub Album", Some(12), 1).await;

    let (total, local) = album_totals(&pool, album).await;
    assert_eq!(total, Some(12), "TASK-138: a stub keeps its declared count");
    assert_eq!(
        local,
        Some(0),
        "BD-5: the stub's imported count is truthful"
    );

    sqlx::query("UPDATE albums SET is_stub = 0 WHERE id = ?")
        .bind(album)
        .execute(&pool)
        .await
        .unwrap();
    let track = seed_mirrored_track(&pool, "Hydrated", Some(album)).await;
    assert_eq!(mirrored_row(&pool, track).await.is_some(), true);

    // 12 declared, 1 imported: still a declared total, not a derived one.
    assert_eq!(album_totals(&pool, album).await, (Some(12), Some(1)));
}
