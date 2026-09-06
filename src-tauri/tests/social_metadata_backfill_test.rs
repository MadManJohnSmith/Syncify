//! Tests for Social Metadata Backfill & Canonical Year Alignment (TASK-113)
//!
//! Validates:
//! 1. Genre propagation from album siblings to tracks with genre NULL.
//! 2. Genre propagation from artist dominant genre when no album sibling has genre.
//! 3. Album release_date inference from MIN(tracks.release_year) and track ISRC.
//! 4. Canonical release_year derivation aligning divergent track years to parent album.
//! 5. Enrichment engine persistence behavior honoring canonical album date & fallback genre.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::services::enrichment::{
    backfill_social_metadata, EnrichmentEngine, OriginTrackMetadata,
};

async fn setup_test_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Run migrations");

    pool
}

#[tokio::test]
async fn test_genre_propagation_from_album_and_artist() {
    let pool = setup_test_db().await;

    // Create Artist 1
    let artist_1_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create Album 1
    let album_1_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('The Dark Side of the Moon', '1973-03-01') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_1_id)
        .bind(artist_1_id)
        .execute(&pool)
        .await
        .unwrap();

    // Track 1: Has Genre "Progressive Rock"
    let track_1_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, genre, release_year) VALUES ('Time', ?, 'Progressive Rock', 1973) RETURNING id"
    )
    .bind(album_1_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_1_id)
        .bind(artist_1_id)
        .execute(&pool)
        .await
        .unwrap();

    // Track 2: Same album, Genre is NULL
    let track_2_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, genre, release_year) VALUES ('Money', ?, NULL, 1973) RETURNING id"
    )
    .bind(album_1_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_2_id)
        .bind(artist_1_id)
        .execute(&pool)
        .await
        .unwrap();

    // Create Album 2: Entirely different album with no genres set on any track
    let album_2_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('Meddle', '1971-10-30') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_2_id)
        .bind(artist_1_id)
        .execute(&pool)
        .await
        .unwrap();

    // Track 3: On Album 2, Genre is NULL
    let track_3_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, genre, release_year) VALUES ('Echoes', ?, NULL, 1971) RETURNING id"
    )
    .bind(album_2_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_3_id)
        .bind(artist_1_id)
        .execute(&pool)
        .await
        .unwrap();

    // Execute backfill
    let report = backfill_social_metadata(&pool)
        .await
        .expect("Backfill execution succeeded");

    assert!(report.genres_backfilled >= 2, "Expected at least 2 genres backfilled");

    // Assert Track 2 received genre from Album 1 sibling
    let genre_2: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = ?")
        .bind(track_2_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(genre_2.as_deref(), Some("Progressive Rock"));

    // Assert Track 3 received genre from Artist Pink Floyd dominant genre
    let genre_3: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = ?")
        .bind(track_3_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(genre_3.as_deref(), Some("Progressive Rock"));

    // Assert remaining null genres in this database is 0
    let remaining_null: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE genre IS NULL")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining_null, 0);
}

#[tokio::test]
async fn test_album_release_date_inference_and_track_sync() {
    let pool = setup_test_db().await;

    // Create Album without release_date
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('Definitely Maybe', NULL) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Track 1 with known release_year = 1994
    let _t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, release_year) VALUES ('Rock ''n'' Roll Star', ?, 1994) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Track 2 with release_year NULL
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, release_year) VALUES ('Live Forever', ?, NULL) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Run backfill
    let report = backfill_social_metadata(&pool)
        .await
        .expect("Backfill succeeded");

    assert!(report.albums_dates_inferred >= 1);

    // Verify album received inferred date 1994-01-01
    let album_date: Option<String> = sqlx::query_scalar("SELECT release_date FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(album_date.as_deref(), Some("1994-01-01"));

    // Verify Track 2 received synchronized release_year 1994
    let t2_year: Option<i32> = sqlx::query_scalar("SELECT release_year FROM tracks WHERE id = ?")
        .bind(t2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t2_year, Some(1994));
}

#[tokio::test]
async fn test_isrc_release_date_inference_for_stub_albums() {
    let pool = setup_test_db().await;

    // Album without release_date and tracks without release_year, but with valid ISRC
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('Cerulean', NULL) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // ISRC US2CT1010021 has '10' -> year 2010
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc, release_year) VALUES ('♥', ?, 'US2CT1010021', NULL) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let report = backfill_social_metadata(&pool).await.unwrap();
    assert!(report.albums_dates_inferred >= 1);

    let album_date: Option<String> = sqlx::query_scalar("SELECT release_date FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(album_date.as_deref(), Some("2010-01-01"));

    let track_year: Option<i32> = sqlx::query_scalar("SELECT release_year FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(track_year, Some(2010));
}

#[tokio::test]
async fn test_divergent_track_years_canonical_reconciliation() {
    let pool = setup_test_db().await;

    // Album Rumours (1977)
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('Rumours', '1977-02-03') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Track 1 has original 1977
    let _t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, release_year) VALUES ('The Chain', ?, 1977) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Track 2 has spurious remastered year 2023 (>2 years divergence)
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, release_year) VALUES ('Dreams', ?, 2023) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Verify pre-condition: divergence > 2 years
    let pre_div: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM (
            SELECT a.id FROM albums a JOIN tracks t ON t.album_id = a.id
            WHERE t.release_year IS NOT NULL
            GROUP BY a.id HAVING (MAX(t.release_year) - MIN(t.release_year)) > 2
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pre_div, 1, "Must have 1 divergent album before reconciliation");

    // Execute backfill
    let report = backfill_social_metadata(&pool).await.unwrap();
    assert_eq!(report.remaining_divergent_albums, 0);

    // Verify Track 2 was reconciled to album's canonical year (1977)
    let t2_year: Option<i32> = sqlx::query_scalar("SELECT release_year FROM tracks WHERE id = ?")
        .bind(t2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t2_year, Some(1977));

    // Verify divergence count is now exactly 0
    let post_div: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM (
            SELECT a.id FROM albums a JOIN tracks t ON t.album_id = a.id
            WHERE t.release_year IS NOT NULL
            GROUP BY a.id HAVING (MAX(t.release_year) - MIN(t.release_year)) > 2
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(post_div, 0, "Divergence must reduce to 0");
}

#[tokio::test]
async fn test_enrichment_engine_canonical_year_and_genre_derivation() {
    let pool = setup_test_db().await;
    let engine = EnrichmentEngine::new();

    // 1. Seed artist and album with genre & release_date
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Radiohead') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('In Rainbows', '2007-10-10') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // Seed sibling track with genre "Art Rock"
    let _s_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, genre, release_year) VALUES ('15 Step', ?, 'Art Rock', 2007) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 2. Persist new track for this album without genre and with a remaster year 2021
    let origin = OriginTrackMetadata {
        title: Some("Bodysnatchers".to_string()),
        artist: Some("Radiohead".to_string()),
        album: Some("In Rainbows".to_string()),
        release_year: Some("2021".to_string()),
        source_name: "qobuz".to_string(),
        genre: None,
        ..Default::default()
    };

    let sync_res = engine
        .enrich_and_persist_sync_track(
            &pool,
            syncify_tauri_lib::services::enrichment::SyncTrackInput {
                service_id: 2,
                service_name: "qobuz".to_string(),
                service_track_id: "qb_radiohead_bodysnatchers".to_string(),
                account_id: 1,
                is_favorite: true,
                is_purchased: false,
                album_provider_track_id: None,
                cover_art_url: None,
                origin_meta: origin.clone(),
                duration_ms: Some(242000),
                format: Some("FLAC".to_string()),
                bit_depth: Some(24),
                sample_rate: Some(96000),
                quality_score: Some(150),
                audio_quality: Some("hires".to_string()),
                album_is_favorite: false,
                query_musicbrainz: false,
            },
        )
        .await
        .expect("Persist track succeeded");

    let track_id = sync_res.track_id;

    // Assert that the track received canonical year 2007 from parent album (not 2021)
    let year: Option<i32> = sqlx::query_scalar("SELECT release_year FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(year, Some(2007));

    // Assert that the track inherited the genre "Art Rock" from the album sibling
    let genre: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(genre.as_deref(), Some("Art Rock"));
}
