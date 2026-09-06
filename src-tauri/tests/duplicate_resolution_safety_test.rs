//! TASK-81: Duplicate Resolution Safety and Asymmetric Coverage Test Suite
//!
//! Validates:
//! 1. Homonymous songs ("Intro", "Home") by different artists without ISRC are NEVER merged.
//! 2. Songs by the same artist with identical title and duration within ±2000ms merge correctly.
//! 3. Asymmetric pairs (one with ISRC, one without) resolve in favor of the track with ISRC,
//!    inheriting sources, downloads, and highest audio quality tier.
//! 4. Invalid tracks (duration <= 10s, empty/whitespace title, duration difference > 2000ms) are safely ignored.
//! 5. Distinct non-null ISRCs are never merged even with identical titles and artists.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::auto_resolve_duplicates_inner;
use syncify_tauri_lib::crypto;

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
async fn test_homonymous_tracks_different_artists_never_merge() {
    let pool = setup_test_db().await;

    // Artists: The xx vs Alt-J
    let art1: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('The xx') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let art2: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Alt-J') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Homonymous tracks "Intro" without ISRC, near duration
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Intro', NULL, 127000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Intro', NULL, 128000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t1).bind(art1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t2).bind(art2).execute(&pool).await.unwrap();

    // Artists: Edward Sharpe vs Michael Bublé
    let art3: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Edward Sharpe') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let art4: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Michael Bublé') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Homonymous tracks "Home" without ISRC, near duration
    let t3: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Home', NULL, 215000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let t4: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Home', NULL, 216000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t3).bind(art3).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t4).bind(art4).execute(&pool).await.unwrap();

    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 0, "Different artists must NEVER be resolved as duplicates");
    assert_eq!(res.tracks_removed, 0, "No tracks should be removed");

    // Verify all tracks remain untouched
    let all_tracks: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks ORDER BY id ASC")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(all_tracks, vec![t1, t2, t3, t4]);
}

#[tokio::test]
async fn test_same_artist_same_title_near_duration_merges_successfully() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Adele') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Two tracks by Adele, same title, duration delta = 500ms <= 2000ms
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Rolling in the Deep', NULL, 228000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Rolling in the Deep', NULL, 228500) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t1).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t2).bind(artist_id).execute(&pool).await.unwrap();

    let services: Vec<(i64,)> = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 2")
        .fetch_all(&pool).await.unwrap();
    let s1 = services[0].0;
    let s2 = services[1].0;

    // Track 1 has source on service 1 with quality 50, Track 2 has source on service 2 with quality 100
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score) VALUES (?, ?, 'src_t1', 50)")
        .bind(t1).bind(s1).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score) VALUES (?, ?, 'src_t2', 100)")
        .bind(t2).bind(s2).execute(&pool).await.unwrap();

    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 1);
    assert_eq!(res.tracks_removed, 1);

    // Winner should be t2 (quality 100 > 50)
    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Rolling in the Deep'")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(remaining, vec![t2]);

    // Check that sources were merged into t2
    let sources_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(t2).fetch_one(&pool).await.unwrap();
    assert_eq!(sources_count, 2, "Sources from loser track should be merged into winner");
}

#[tokio::test]
async fn test_asymmetric_pair_isrc_wins_and_inherits_enrichment() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Track 1 (Canonical with ISRC, but lower quality score)
    let t1_isrc: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms, audio_quality) VALUES ('Get Lucky', 'USQX91300105', 248000, 'LOSSLESS') RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    // Track 2 (Imported without ISRC, but has higher quality score and downloaded physical file)
    let t2_no_isrc: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms, audio_quality) VALUES ('Get Lucky', NULL, 247500, 'HI_RES') RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t1_isrc).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t2_no_isrc).bind(artist_id).execute(&pool).await.unwrap();

    let services: Vec<(i64,)> = sqlx::query_as("SELECT id FROM services ORDER BY id LIMIT 2")
        .fetch_all(&pool).await.unwrap();
    let s1 = services[0].0;
    let s2 = services[1].0;

    // Source on t1: 16-bit 44.1kHz (CD quality), quality_score = 50
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate, format) VALUES (?, ?, 't1_src', 50, 16, 44100, 'FLAC')")
        .bind(t1_isrc).bind(s1).execute(&pool).await.unwrap();

    // Source on t2: 24-bit 96kHz (Hi-Res), quality_score = 120
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, quality_score, bit_depth, sample_rate, format) VALUES (?, ?, 't2_src', 120, 24, 96000, 'FLAC')")
        .bind(t2_no_isrc).bind(s2).execute(&pool).await.unwrap();

    // Physical download on t2
    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_size_bytes, bit_depth, sample_rate, file_format) VALUES (?, '/music/Daft Punk/Get Lucky.flac', 55000000, 24, 96000, 'FLAC')"
    ).bind(t2_no_isrc).execute(&pool).await.unwrap();

    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 1);
    assert_eq!(res.tracks_removed, 1);

    // Assert that the track with ISRC (t1_isrc) WON despite t2 having higher quality and download
    let remaining_tracks: Vec<(i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, isrc, audio_quality FROM tracks WHERE title = 'Get Lucky'"
    ).fetch_all(&pool).await.unwrap();

    assert_eq!(remaining_tracks.len(), 1);
    let (survivor_id, survivor_isrc, survivor_quality) = &remaining_tracks[0];
    assert_eq!(*survivor_id, t1_isrc, "The track with ISRC MUST be retained as canonical");
    assert_eq!(survivor_isrc.as_deref(), Some("USQX91300105"), "ISRC must be preserved");

    // The survivor must inherit the highest quality tier (hires)
    assert_eq!(survivor_quality.as_deref(), Some("hires"), "Survivor must inherit highest audio quality");

    // Download record must be transferred to the winner
    let download_track_id: i64 = sqlx::query_scalar("SELECT track_id FROM downloads WHERE file_path = '/music/Daft Punk/Get Lucky.flac'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(download_track_id, t1_isrc, "Download record must be transferred to canonical ISRC winner");

    // Both sources must now belong to the winner
    let sources_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(t1_isrc).fetch_one(&pool).await.unwrap();
    assert_eq!(sources_count, 2, "Both track sources must now be attached to the canonical winner");
}

#[tokio::test]
async fn test_invalid_and_short_entries_ignored() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Short Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // 1. Short tracks (duration <= 10000ms e.g. 5000ms)
    let t1_short: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Short Clip', NULL, 5000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let t2_short: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Short Clip', NULL, 5100) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t1_short).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t2_short).bind(artist_id).execute(&pool).await.unwrap();

    // 2. Empty/whitespace titles
    let t3_blank: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('   ', NULL, 150000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let t4_blank: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('', NULL, 150500) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t3_blank).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t4_blank).bind(artist_id).execute(&pool).await.unwrap();

    // 3. Duration difference > 2000ms (e.g. 180000 vs 185000)
    let t5_diff: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Delta Song', NULL, 180000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let t6_diff: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Delta Song', NULL, 185000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t5_diff).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t6_diff).bind(artist_id).execute(&pool).await.unwrap();

    // 4. Tracks without any artist
    let _t7_no_art: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('No Artist Song', NULL, 180000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let _t8_no_art: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('No Artist Song', NULL, 180000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 0, "No invalid tracks should be resolved");
    assert_eq!(res.tracks_removed, 0, "No invalid tracks should be removed");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 8, "All 8 invalid candidate tracks must remain intact");
}

#[tokio::test]
async fn test_distinct_isrcs_never_merged() {
    let pool = setup_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Taylor Swift') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Same title, same artist, near duration, but DIFFERENT non-null ISRCs (e.g. Original vs Re-recording)
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Love Story', 'USCJY0803301', 235000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Love Story', 'USUG12100345', 235500) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t1).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t2).bind(artist_id).execute(&pool).await.unwrap();

    let res = auto_resolve_duplicates_inner(&pool).await.unwrap();
    assert_eq!(res.groups_resolved, 0, "Tracks with distinct ISRCs must NEVER be merged");
    assert_eq!(res.tracks_removed, 0);

    let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE title = 'Love Story' ORDER BY id ASC")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(remaining, vec![t1, t2], "Both tracks with distinct ISRCs must be preserved");
}
