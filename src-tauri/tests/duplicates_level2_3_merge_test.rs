//! TASK-104: Duplicates Level 2/3 Merge and Explicit Discrimination Test Suite
//!
//! Validates:
//! 1. Intra-album duplicate merge (same album, colliding track_numbers) retaining the highest
//!    quality track and renumbering tracks sequentially without numbering collisions.
//! 2. Explicit flag discrimination: Tracks with contradictory explicit flags (explicit=1 vs explicit=0)
//!    are NEVER falsely collapsed/merged.
//! 3. Preservation of all unique track_sources across services with quality upgrade for shared services.
//! 4. Unit-level validation of track_matcher explicit compatibility and fuzzy matching.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{auto_resolve_duplicates_inner, merge_level2_3_duplicates_inner};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::track_matcher::{is_explicit_compatible, is_fuzzy_track_match};

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

#[tokio::test]
async fn test_intra_album_duplicate_merge_and_renumbering() {
    let pool = setup_test_db().await;

    // Create Artist and Album
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('The Wall') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    // Track 1A: 16-bit, lower quality score 50, track_number = 1
    let t1a: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, audio_quality) VALUES ('In the Flesh?', ?, 196000, 1, 1, 'LOSSLESS') RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool).await.unwrap();

    // Track 1B: Duplicate of Track 1, 24-bit 96kHz downloaded, quality score 120, track_number = 1 (numbering collision)
    let t1b: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, audio_quality) VALUES ('In the Flesh?', ?, 196500, 1, 1, 'HI_RES') RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool).await.unwrap();

    // Track 2: "The Thin Ice", track_number = 2
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, audio_quality) VALUES ('The Thin Ice', ?, 148000, 2, 1, 'LOSSLESS') RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool).await.unwrap();

    // Link artists
    for tid in [t1a, t1b, t2] {
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&pool).await.unwrap();
    }

    // Sources: t1a has quality_score 50, t1b has quality_score 120
    let s1: (i64,) = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 't1a_src', 50, 16, 44100)")
        .bind(t1a).bind(s1.0).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 't1b_src', 120, 24, 96000)")
        .bind(t1b).bind(s1.0).execute(&pool).await.unwrap();

    // t1b has a local physical file
    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_size_bytes, bit_depth, sample_rate, file_format) VALUES (?, '/music/Pink Floyd/The Wall/01 In the Flesh.flac', 45000000, 24, 96000, 'FLAC')"
    ).bind(t1b).execute(&pool).await.unwrap();

    // Verify pre-condition: 3 tracks, and collision on (album_id, disc 1, track 1)
    let collision_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT track_number FROM tracks WHERE album_id = ? GROUP BY disc_number, track_number HAVING COUNT(*) > 1)"
    )
    .bind(album_id)
    .fetch_one(&pool).await.unwrap();
    assert_eq!(collision_count, 1, "There must be 1 track number collision pre-merge");

    // Execute merge
    let res = merge_level2_3_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 1);
    assert_eq!(res.tracks_removed, 1);

    // Verify survivor: t1b must survive because it is 24-bit / downloaded / higher quality score
    let remaining_t1: Vec<(i64, Option<String>)> = sqlx::query_as(
        "SELECT id, audio_quality FROM tracks WHERE album_id = ? AND title = 'In the Flesh?'"
    )
    .bind(album_id)
    .fetch_all(&pool).await.unwrap();
    assert_eq!(remaining_t1.len(), 1);
    assert_eq!(remaining_t1[0].0, t1b, "Higher quality downloaded track t1b must survive");

    // Verify album renumbering: track_number must be 1 for In the Flesh? and 2 for The Thin Ice
    let album_tracks: Vec<(i64, String, i32)> = sqlx::query_as(
        "SELECT id, title, track_number FROM tracks WHERE album_id = ? ORDER BY track_number ASC"
    )
    .bind(album_id)
    .fetch_all(&pool).await.unwrap();

    assert_eq!(album_tracks.len(), 2);
    assert_eq!(album_tracks[0].0, t1b);
    assert_eq!(album_tracks[0].2, 1, "First track must be renumbered to 1");
    assert_eq!(album_tracks[1].0, t2);
    assert_eq!(album_tracks[1].2, 2, "Second track must be renumbered to 2");

    // Verify zero collisions post-merge
    let post_collisions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT track_number FROM tracks WHERE album_id = ? GROUP BY disc_number, track_number HAVING COUNT(*) > 1)"
    )
    .bind(album_id)
    .fetch_one(&pool).await.unwrap();
    assert_eq!(post_collisions, 0, "Numbering collisions must be exactly 0 post-merge");
}

#[tokio::test]
async fn test_explicit_flag_discrimination_never_merges() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Eminem') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Track 1: Explicit version (explicit = 1)
    let t1_explicit: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, explicit) VALUES ('Lose Yourself', 326000, 1) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    // Track 2: Clean/Radio edit (explicit = 0) with identical title and near duration
    let t2_clean: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, explicit) VALUES ('Lose Yourself', 325500, 0) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    for tid in [t1_explicit, t2_clean] {
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&pool).await.unwrap();
    }

    // Execute merge
    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 0, "Contradictory explicit tracks must NOT be resolved");
    assert_eq!(res.tracks_removed, 0, "Zero tracks removed when explicit flags contradict");

    // Both tracks must still exist
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE title = 'Lose Yourself'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 2, "Both explicit and clean tracks must remain separate canonical entries");
}

#[tokio::test]
async fn test_track_sources_preservation_and_quality_upgrade() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Radiohead') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('OK Computer') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    // Two duplicate tracks in the same album
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number) VALUES ('Karma Police', ?, 264000, 6) RETURNING id"
    ).bind(album_id).fetch_one(&pool).await.unwrap();

    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number) VALUES ('Karma Police', ?, 264500, 6) RETURNING id"
    ).bind(album_id).fetch_one(&pool).await.unwrap();

    for tid in [t1, t2] {
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&pool).await.unwrap();
    }

    let services: Vec<(i64,)> = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 3")
        .fetch_all(&pool).await.unwrap();
    let s1 = services[0].0;
    let s2 = services[1].0;
    let s3 = services[2].0;

    // t1 sources: service 1 (qs 50), service 2 (qs 80)
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 't1_s1', 50, 16, 44100)")
        .bind(t1).bind(s1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 't1_s2', 80, 16, 44100)")
        .bind(t1).bind(s2).execute(&pool).await.unwrap();

    // t2 sources: service 2 (qs 120 - higher quality!), service 3 (qs 150)
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 't2_s2', 120, 24, 96000)")
        .bind(t2).bind(s2).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 't2_s3', 150, 24, 192000)")
        .bind(t2).bind(s3).execute(&pool).await.unwrap();

    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 1);
    assert_eq!(res.tracks_removed, 1);

    // Remaining track must have 3 sources (s1, s2, s3)
    let remaining_id: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE album_id = ?")
        .bind(album_id).fetch_one(&pool).await.unwrap();

    let sources: Vec<(i64, String, Option<i32>)> = sqlx::query_as(
        "SELECT service_id, service_track_id, quality_score FROM track_sources WHERE track_id = ? ORDER BY service_id ASC"
    )
    .bind(remaining_id)
    .fetch_all(&pool).await.unwrap();

    assert_eq!(sources.len(), 3, "All 3 service sources must be retained");
    assert_eq!(sources[0].0, s1);
    assert_eq!(sources[0].2, Some(50));

    // For service 2, the quality score must be 120 (upgraded from 80)
    assert_eq!(sources[1].0, s2);
    assert_eq!(sources[1].2, Some(120), "Service 2 source must retain the higher quality score 120");

    assert_eq!(sources[2].0, s3);
    assert_eq!(sources[2].2, Some(150));
}

#[tokio::test]
async fn test_intra_album_isrc_divergence_reconciliation() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Random Access Memories') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    // Track 1 with US ISRC, 16-bit
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc, track_number, audio_quality) VALUES ('Get Lucky', ?, 248000, 'USQX91300105', 8, 'LOSSLESS') RETURNING id"
    ).bind(album_id).fetch_one(&pool).await.unwrap();

    // Track 2 with divergent GB ISRC, 24-bit 96kHz (higher quality)
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc, track_number, audio_quality) VALUES ('Get Lucky', ?, 248200, 'GBAYE1300042', 8, 'HI_RES') RETURNING id"
    ).bind(album_id).fetch_one(&pool).await.unwrap();

    for tid in [t1, t2] {
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&pool).await.unwrap();
    }

    let s1: (i64,) = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 1").fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 's1', 50, 16, 44100)")
        .bind(t1).bind(s1.0).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate) VALUES (?, ?, 's2', 120, 24, 96000)")
        .bind(t2).bind(s1.0).execute(&pool).await.unwrap();

    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 1);
    assert_eq!(res.tracks_removed, 1);

    // Remaining track must be t2 (higher quality score 120 vs 50, 24-bit vs 16-bit)
    let remaining: Vec<(i64, Option<String>)> = sqlx::query_as("SELECT id, isrc FROM tracks WHERE album_id = ?")
        .bind(album_id).fetch_all(&pool).await.unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].0, t2);
    assert!(remaining[0].1.is_some(), "ISRC must be retained");
}

#[test]
fn test_track_matcher_explicit_and_fuzzy_rules() {
    // Explicit discrimination compatibility
    assert!(!is_explicit_compatible(Some(true), Some(0)), "Explicit requested on clean DB must fail");
    assert!(!is_explicit_compatible(Some(false), Some(1)), "Clean requested on explicit DB must fail");
    assert!(is_explicit_compatible(Some(true), Some(1)), "Explicit matches explicit");
    assert!(is_explicit_compatible(Some(false), Some(0)), "Clean matches clean");
    assert!(is_explicit_compatible(None, Some(1)), "Unknown requested matches explicit");
    assert!(is_explicit_compatible(Some(true), None), "Explicit requested matches unknown DB");

    // Title normalization and duration tolerance
    assert!(is_fuzzy_track_match("Heroes - Remastered", Some(370000), "Heroes", Some(371000)));
    assert!(is_fuzzy_track_match("Paranoid Android (Live)", Some(387000), "Paranoid Android", Some(388500)));
    assert!(!is_fuzzy_track_match("Song A", Some(100000), "Song B", Some(100000)), "Different titles must not match");
    assert!(!is_fuzzy_track_match("Song A", Some(100000), "Song A", Some(105000)), "Duration diff > 2000ms must not match");
}
