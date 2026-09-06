//! TASK-21: Suite for Smart Playlists Rules Evaluation, Dynamic Counts, and Persistence
//!
//! Validates:
//! 1. Dynamic preview count by genre (contains, is, isNot).
//! 2. Dynamic preview count by release year (is, greaterThan, lessThan).
//! 3. Dynamic preview count by audio quality (lossless, hires, lossy).
//! 4. Combinations of multiple rules (e.g. genre + year + quality).
//! 5. Durable persistence and recovery of smart playlists and matching playlist_tracks in SQLite.
//! 6. Handling of empty rules and invalid rule inputs.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{
    create_smart_playlist_core, preview_smart_playlist_count_core, Playlist,
};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Seed services and accounts
    sqlx::query(
        "INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) \
         VALUES (1, 'spotify', 0, 'lossy'), (2, 'tidal', 1, 'hires'), (3, 'qobuz', 1, 'hires')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active) \
         VALUES (1, 1, 'Local Account', 1), (2, 2, 'Tidal Account', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

async fn seed_test_catalog(pool: &SqlitePool) {
    // Artists
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Queen'), (2, 'Daft Punk'), (3, 'Michael Jackson')")
        .execute(pool)
        .await
        .unwrap();

    // Albums
    sqlx::query(
        "INSERT INTO albums (id, title, release_date) VALUES \
         (1, 'A Night at the Opera', '1975-11-21'), \
         (2, 'Discovery', '2001-03-12'), \
         (3, 'Thriller', '1982-11-30')",
    )
    .execute(pool)
    .await
    .unwrap();

    // Tracks
    // Track 1: Queen - Bohemian Rhapsody (1975, Rock, lossless)
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, genre, audio_quality, created_at) \
         VALUES (1, 'Bohemian Rhapsody', 1, 'Rock', 'lossless', '2023-01-01 10:00:00')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (1, 1, 'primary')")
        .execute(pool)
        .await
        .unwrap();

    // Track 2: Queen - Love of My Life (1975, Classic Rock, hires)
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, genre, audio_quality, created_at) \
         VALUES (2, 'Love of My Life', 1, 'Classic Rock', 'hires', '2023-02-01 10:00:00')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (2, 1, 'primary')")
        .execute(pool)
        .await
        .unwrap();

    // Track 3: Daft Punk - One More Time (2001, Electronic, lossy)
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, genre, audio_quality, created_at) \
         VALUES (3, 'One More Time', 2, 'Electronic', 'lossy', '2023-03-01 10:00:00')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (3, 2, 'primary')")
        .execute(pool)
        .await
        .unwrap();

    // Track 4: Michael Jackson - Billie Jean (1982, Pop, lossless)
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, genre, audio_quality, created_at) \
         VALUES (4, 'Billie Jean', 3, 'Pop', 'lossless', '2023-04-01 10:00:00')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (4, 3, 'primary')")
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_preview_smart_playlist_count_by_genre() {
    let pool = create_test_db().await;
    seed_test_catalog(&pool).await;

    // Contains "Rock" -> matches Bohemian Rhapsody and Love of My Life (2 tracks)
    let rules_rock = r#"[{"field":"genre","operator":"contains","value":"Rock"}]"#;
    let count_rock = preview_smart_playlist_count_core(&pool, rules_rock)
        .await
        .expect("Preview count rock");
    assert_eq!(count_rock, 2, "Expected 2 tracks matching Rock genre");

    // Exact "Pop" -> matches Billie Jean (1 track)
    let rules_pop = r#"[{"field":"genre","operator":"is","value":"Pop"}]"#;
    let count_pop = preview_smart_playlist_count_core(&pool, rules_pop)
        .await
        .expect("Preview count pop");
    assert_eq!(count_pop, 1, "Expected 1 track matching exact Pop genre");

    // Operator "isNot" Pop -> matches Bohemian Rhapsody, Love of My Life, One More Time (3 tracks)
    let rules_not_pop = r#"[{"field":"genre","operator":"isNot","value":"Pop"}]"#;
    let count_not_pop = preview_smart_playlist_count_core(&pool, rules_not_pop)
        .await
        .expect("Preview count not pop");
    assert_eq!(count_not_pop, 3, "Expected 3 tracks not matching Pop");
}

#[tokio::test]
async fn test_preview_smart_playlist_count_by_year() {
    let pool = create_test_db().await;
    seed_test_catalog(&pool).await;

    // Year is 1975 -> Bohemian Rhapsody, Love of My Life (2 tracks)
    let rules_1975 = r#"[{"field":"year","operator":"is","value":"1975"}]"#;
    let count_1975 = preview_smart_playlist_count_core(&pool, rules_1975)
        .await
        .expect("Preview count 1975");
    assert_eq!(count_1975, 2, "Expected 2 tracks released in 1975");

    // Year greater than 2000 -> One More Time (2001) (1 track)
    let rules_post_2000 = r#"[{"field":"year","operator":"greaterThan","value":"2000"}]"#;
    let count_post_2000 = preview_smart_playlist_count_core(&pool, rules_post_2000)
        .await
        .expect("Preview count post-2000");
    assert_eq!(count_post_2000, 1, "Expected 1 track released after 2000");

    // Year less than 1980 -> Bohemian Rhapsody, Love of My Life (2 tracks)
    let rules_pre_1980 = r#"[{"field":"year","operator":"lessThan","value":"1980"}]"#;
    let count_pre_1980 = preview_smart_playlist_count_core(&pool, rules_pre_1980)
        .await
        .expect("Preview count pre-1980");
    assert_eq!(count_pre_1980, 2, "Expected 2 tracks released before 1980");
}

#[tokio::test]
async fn test_preview_smart_playlist_count_by_quality() {
    let pool = create_test_db().await;
    seed_test_catalog(&pool).await;

    // Quality is "lossless" -> Bohemian Rhapsody, Billie Jean (2 tracks)
    let rules_lossless = r#"[{"field":"quality","operator":"is","value":"lossless"}]"#;
    let count_lossless = preview_smart_playlist_count_core(&pool, rules_lossless)
        .await
        .expect("Preview count lossless");
    assert_eq!(count_lossless, 2, "Expected 2 lossless tracks");

    // Quality is "hires" -> Love of My Life (1 track)
    let rules_hires = r#"[{"field":"quality","operator":"is","value":"hires"}]"#;
    let count_hires = preview_smart_playlist_count_core(&pool, rules_hires)
        .await
        .expect("Preview count hires");
    assert_eq!(count_hires, 1, "Expected 1 hires track");
}

#[tokio::test]
async fn test_preview_smart_playlist_combined_rules() {
    let pool = create_test_db().await;
    seed_test_catalog(&pool).await;

    // Combined: Genre contains "Rock" AND Quality is "lossless" -> Bohemian Rhapsody (1 track)
    let rules_rock_lossless = r#"[
        {"field":"genre","operator":"contains","value":"Rock"},
        {"field":"quality","operator":"is","value":"lossless"}
    ]"#;
    let count = preview_smart_playlist_count_core(&pool, rules_rock_lossless)
        .await
        .expect("Preview combined count");
    assert_eq!(count, 1, "Expected exactly 1 rock lossless track");

    // Combined: Genre contains "Rock" AND Year < 1980 AND Quality is "hires" -> Love of My Life (1 track)
    let rules_complex = r#"[
        {"field":"genre","operator":"contains","value":"Rock"},
        {"field":"year","operator":"lessThan","value":"1980"},
        {"field":"quality","operator":"is","value":"hires"}
    ]"#;
    let count_complex = preview_smart_playlist_count_core(&pool, rules_complex)
        .await
        .expect("Preview complex count");
    assert_eq!(count_complex, 1, "Expected 1 track for complex query");

    // Combined with no match
    let rules_no_match = r#"[
        {"field":"genre","operator":"contains","value":"Rock"},
        {"field":"year","operator":"greaterThan","value":"2010"}
    ]"#;
    let count_none = preview_smart_playlist_count_core(&pool, rules_no_match)
        .await
        .expect("Preview no match count");
    assert_eq!(count_none, 0, "Expected 0 tracks for non-matching combination");
}

#[tokio::test]
async fn test_create_smart_playlist_persistence_and_tracks_population() {
    let pool = create_test_db().await;
    seed_test_catalog(&pool).await;

    let rules_json = r#"[{"field":"genre","operator":"contains","value":"Rock"}]"#;

    // Create smart playlist
    let created = create_smart_playlist_core(&pool, "Classic Rock Gems", rules_json, Some(1))
        .await
        .expect("Create smart playlist");

    assert!(created.id > 0, "Created playlist must have a valid positive ID");
    assert_eq!(created.name, "Classic Rock Gems");
    assert_eq!(created.track_count, 2, "Must contain 2 tracks");
    assert!(created.is_smart, "is_smart must be true");
    assert_eq!(created.rules_json.as_deref(), Some(rules_json));

    // Verify persistence directly in SQLite
    let (is_smart_db, rules_db, track_count_db): (i64, Option<String>, i64) = sqlx::query_as(
        "SELECT is_smart, rules_json, track_count FROM playlists WHERE id = ?",
    )
    .bind(created.id)
    .fetch_one(&pool)
    .await
    .expect("Query playlists table");

    assert_eq!(is_smart_db, 1, "Database column is_smart must be 1");
    assert_eq!(rules_db.as_deref(), Some(rules_json), "Database rules_json must match");
    assert_eq!(track_count_db, 2, "Database track_count must be 2");

    // Verify playlist_tracks entries
    let playlist_tracks: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT playlist_id, track_id, position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC",
    )
    .bind(created.id)
    .fetch_all(&pool)
    .await
    .expect("Query playlist_tracks");

    assert_eq!(playlist_tracks.len(), 2, "Expected 2 entries in playlist_tracks");
    assert_eq!(playlist_tracks[0], (created.id, 1, 1), "Track 1 at position 1");
    assert_eq!(playlist_tracks[1], (created.id, 2, 2), "Track 2 at position 2");

    // Verify reading back with get_playlist query pattern
    let fetched = sqlx::query_as::<_, Playlist>(
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
    .bind(created.id)
    .fetch_one(&pool)
    .await
    .expect("Fetch playlist by ID");

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.name, "Classic Rock Gems");
    assert!(fetched.is_smart);
    assert_eq!(fetched.rules_json.as_deref(), Some(rules_json));
}

#[tokio::test]
async fn test_empty_rules_and_error_handling() {
    let pool = create_test_db().await;
    seed_test_catalog(&pool).await;

    // Empty list
    let count_empty = preview_smart_playlist_count_core(&pool, "[]")
        .await
        .expect("Empty rules list preview");
    assert_eq!(count_empty, 0, "Empty rules must yield 0 count");

    // Rule with empty value skips condition
    let count_blank_val = preview_smart_playlist_count_core(
        &pool,
        r#"[{"field":"genre","operator":"contains","value":""}]"#,
    )
    .await
    .expect("Blank rule value preview");
    assert_eq!(count_blank_val, 0, "Blank value must yield 0 count");

    // Malformed JSON returns error
    let malformed_res = preview_smart_playlist_count_core(&pool, "{ invalid json }").await;
    assert!(malformed_res.is_err(), "Malformed JSON must return Err");
}
