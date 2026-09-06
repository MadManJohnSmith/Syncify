//! Integration Test for TASK-124: Schema Alignment of Queries in dashboard.rs and settings.rs
//!
//! Validates that queries across dashboard.rs and settings.rs execute seamlessly against
//! a freshly migrated SQLite database without encountering missing columns (such as legacy
//! `t.album`, `t.artist_id`, `t.artist_name`, `t.year`, `t.file_format`, or `alb.name`).

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::types::LibraryTrack;
use syncify_tauri_lib::models::AlbumDetail;

async fn create_migrated_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly to in-memory SQLite");

    pool
}

async fn seed_test_catalog(pool: &SqlitePool) -> (i64, i64, i64, i64, i64) {
    // 1. Services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(pool).await.unwrap();

    // 2. Artists
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name, musicbrainz_id) VALUES ('Pink Floyd', 'art-mbid-001') RETURNING id"
    )
    .fetch_one(pool).await.unwrap();

    let guest_artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name, musicbrainz_id) VALUES ('David Gilmour', 'art-mbid-002') RETURNING id"
    )
    .fetch_one(pool).await.unwrap();

    // 3. Album
    let album_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO albums (title, release_date, total_tracks, cover_art_url, label) 
        VALUES ('The Dark Side of the Moon', '1973-03-01', 2, 'https://example.com/cover.jpg', 'Harvest Records') 
        RETURNING id
        "#
    )
    .fetch_one(pool).await.unwrap();

    // Link album to artist
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(artist_id)
        .execute(pool).await.unwrap();

    // 4. Tracks
    let track1_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, release_year, genre)
        VALUES ('Speak to Me', ?, 65000, 1, 1, 'GBAYE7300001', 1973, 'Progressive Rock')
        RETURNING id
        "#
    )
    .bind(album_id)
    .fetch_one(pool).await.unwrap();

    let track2_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, release_year, genre)
        VALUES ('Breathe (In the Air)', ?, 169000, 2, 1, 'GBAYE7300002', 1973, 'Progressive Rock')
        RETURNING id
        "#
    )
    .bind(album_id)
    .fetch_one(pool).await.unwrap();

    // Link track_artists
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track1_id)
        .bind(artist_id)
        .execute(pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track2_id)
        .bind(artist_id)
        .execute(pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')")
        .bind(track2_id)
        .bind(guest_artist_id)
        .execute(pool).await.unwrap();

    // 5. Download record for track 1
    sqlx::query(
        r#"
        INSERT INTO downloads (track_id, source_service_id, file_path, file_format, file_size_bytes)
        VALUES (?, 2, '/music/Pink Floyd/The Dark Side of the Moon/01 - Speak to Me.flac', 'FLAC', 15000000)
        "#
    )
    .bind(track1_id)
    .execute(pool).await.unwrap();

    (artist_id, guest_artist_id, album_id, track1_id, track2_id)
}

#[tokio::test]
async fn test_dashboard_get_album_detail_query() {
    let pool = create_migrated_db().await;
    let (_artist_id, _guest_id, album_id, _t1, _t2) = seed_test_catalog(&pool).await;

    let album_name = "The Dark Side of the Moon".to_string();
    let artist_name = "Pink Floyd".to_string();

    let row: (
        i64,
        String,
        String,
        Option<i32>,
        Option<String>,
        Option<String>,
        i64,
        i64,
        Option<String>,
    ) = sqlx::query_as(
        r#"
        SELECT 
            alb.id,
            alb.title,
            COALESCE(
                (SELECT a.name FROM album_artists aa JOIN artists a ON a.id = aa.artist_id WHERE aa.album_id = alb.id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1),
                (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id JOIN tracks tr ON tr.id = ta.track_id WHERE tr.album_id = alb.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
                ?
            ) as artist_name,
            COALESCE(CAST(SUBSTR(alb.release_date, 1, 4) AS INTEGER), MIN(t.release_year)) as release_year,
            MIN(t.genre) as genre,
            alb.label,
            COUNT(t.id) as track_count,
            COALESCE(SUM(t.duration_ms), 0) as total_duration_ms,
            alb.cover_art_url
        FROM albums alb
        LEFT JOIN tracks t ON t.album_id = alb.id
        WHERE alb.title = ? 
          AND (
              EXISTS (
                  SELECT 1 FROM album_artists aa 
                  JOIN artists a ON a.id = aa.artist_id 
                  WHERE aa.album_id = alb.id AND a.name = ?
              )
              OR EXISTS (
                  SELECT 1 FROM track_artists ta 
                  JOIN artists a ON a.id = ta.artist_id 
                  JOIN tracks tr ON tr.id = ta.track_id
                  WHERE tr.album_id = alb.id AND a.name = ?
              )
              OR ? = ''
          )
        GROUP BY alb.id, alb.title, alb.release_date, alb.label, alb.cover_art_url
        "#,
    )
    .bind(&artist_name)
    .bind(&album_name)
    .bind(&artist_name)
    .bind(&artist_name)
    .bind(&artist_name)
    .fetch_one(&pool)
    .await
    .expect("get_album_detail query must succeed on migrated DB without schema error");

    let detail = AlbumDetail {
        id: row.0,
        title: row.1,
        artist_name: row.2,
        release_year: row.3,
        genre: row.4,
        label: row.5,
        track_count: row.6,
        total_duration_ms: row.7,
        artwork_url: row.8,
        quality: None,
        source_service: None,
    };

    assert_eq!(detail.id, album_id);
    assert_eq!(detail.title, "The Dark Side of the Moon");
    assert_eq!(detail.artist_name, "Pink Floyd");
    assert_eq!(detail.release_year, Some(1973));
    assert_eq!(detail.genre, Some("Progressive Rock".to_string()));
    assert_eq!(detail.label, Some("Harvest Records".to_string()));
    assert_eq!(detail.track_count, 2);
    assert_eq!(detail.total_duration_ms, 65000 + 169000);
    assert_eq!(detail.artwork_url, Some("https://example.com/cover.jpg".to_string()));
}

#[tokio::test]
async fn test_dashboard_get_album_tracks_query() {
    let pool = create_migrated_db().await;
    let (_artist_id, _guest_id, album_id, _t1, _t2) = seed_test_catalog(&pool).await;

    let album_name = "The Dark Side of the Moon".to_string();
    let artist_name = "Pink Floyd".to_string();

    let tracks = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT 
            t.id, 
            t.title, 
            COALESCE(
                (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
                (SELECT a.name FROM album_artists aa JOIN artists a ON a.id = aa.artist_id WHERE aa.album_id = alb.id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1)
            ) as artist_name,
            (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1) as artist_id,
            alb.title as album_name, 
            alb.id as album_id,
            t.duration_ms,
            t.isrc,
            d.file_format as quality,
            CASE WHEN d.file_path IS NOT NULL THEN 'downloaded' ELSE 'not_downloaded' END as download_status,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path,
            alb.cover_art_url
        FROM tracks t
        JOIN albums alb ON t.album_id = alb.id
        LEFT JOIN downloads d ON d.track_id = t.id
        WHERE alb.title = ? 
          AND (
              EXISTS (
                  SELECT 1 FROM track_artists ta 
                  JOIN artists a ON a.id = ta.artist_id 
                  WHERE ta.track_id = t.id AND a.name = ?
              )
              OR EXISTS (
                  SELECT 1 FROM album_artists aa 
                  JOIN artists a ON a.id = aa.artist_id 
                  WHERE aa.album_id = alb.id AND a.name = ?
              )
              OR ? = ''
          )
        ORDER BY t.disc_number ASC NULLS LAST, t.track_number ASC NULLS LAST, t.title ASC
        "#
    )
    .bind(&album_name)
    .bind(&artist_name)
    .bind(&artist_name)
    .bind(&artist_name)
    .fetch_all(&pool)
    .await
    .expect("get_album_tracks query must succeed on migrated DB without schema error");

    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].title, "Speak to Me");
    assert_eq!(tracks[0].artist_name, Some("Pink Floyd".to_string()));
    assert_eq!(tracks[0].album_name, Some("The Dark Side of the Moon".to_string()));
    assert_eq!(tracks[0].album_id, Some(album_id));
    assert_eq!(tracks[0].track_number, Some(1));
    assert_eq!(tracks[0].download_status, Some("downloaded".to_string()));
    assert_eq!(tracks[0].quality, Some("FLAC".to_string()));
    assert_eq!(tracks[0].file_path, Some("/music/Pink Floyd/The Dark Side of the Moon/01 - Speak to Me.flac".to_string()));

    assert_eq!(tracks[1].title, "Breathe (In the Air)");
    assert_eq!(tracks[1].artist_name, Some("Pink Floyd".to_string()));
    assert_eq!(tracks[1].album_name, Some("The Dark Side of the Moon".to_string()));
    assert_eq!(tracks[1].album_id, Some(album_id));
    assert_eq!(tracks[1].track_number, Some(2));
    assert_eq!(tracks[1].download_status, Some("not_downloaded".to_string()));
}

#[tokio::test]
async fn test_dashboard_get_artist_detail_and_discography_queries() {
    let pool = create_migrated_db().await;
    let (artist_id, _guest_id, album_id, _t1, _t2) = seed_test_catalog(&pool).await;

    // 1. get_artist_detail query
    let (id, name): (i64, String) = sqlx::query_as("SELECT id, name FROM artists WHERE id = ?")
        .bind(artist_id)
        .fetch_one(&pool)
        .await
        .expect("Artist lookup must succeed");
    assert_eq!(id, artist_id);
    assert_eq!(name, "Pink Floyd");

    let (album_count,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT alb_id) FROM (
            SELECT album_id AS alb_id FROM album_artists WHERE artist_id = ?
            UNION
            SELECT t.album_id AS alb_id FROM tracks t 
            JOIN track_artists ta ON ta.track_id = t.id 
            WHERE ta.artist_id = ? AND t.album_id IS NOT NULL
        )
        "#,
    )
    .bind(artist_id)
    .bind(artist_id)
    .fetch_one(&pool)
    .await
    .expect("album_count query must succeed");
    assert_eq!(album_count, 1);

    let (track_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT track_id) FROM track_artists WHERE artist_id = ?"
    )
    .bind(artist_id)
    .fetch_one(&pool)
    .await
    .expect("track_count query must succeed");
    assert_eq!(track_count, 2);

    // 2. get_artist_albums query
    let album_rows: Vec<(
        i64,
        String,
        String,
        Option<i32>,
        Option<String>,
        Option<String>,
        i64,
        i64,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT 
            alb.id,
            alb.title,
            COALESCE(art.name, 'Unknown Artist') as artist_name,
            COALESCE(CAST(SUBSTR(alb.release_date, 1, 4) AS INTEGER), MIN(t.release_year)) as release_year,
            MIN(t.genre) as genre,
            alb.label,
            COUNT(t.id) as track_count,
            COALESCE(SUM(t.duration_ms), 0) as total_duration_ms,
            alb.cover_art_url
        FROM albums alb
        JOIN artists art ON art.id = ?
        LEFT JOIN tracks t ON t.album_id = alb.id
        WHERE alb.id IN (
            SELECT album_id FROM album_artists WHERE artist_id = ?
            UNION
            SELECT t2.album_id FROM tracks t2 
            JOIN track_artists ta ON ta.track_id = t2.id 
            WHERE ta.artist_id = ? AND t2.album_id IS NOT NULL
        )
        GROUP BY alb.id, alb.title, art.name, alb.release_date, alb.label, alb.cover_art_url
        ORDER BY release_year DESC NULLS LAST, alb.title ASC
        "#,
    )
    .bind(artist_id)
    .bind(artist_id)
    .bind(artist_id)
    .fetch_all(&pool)
    .await
    .expect("get_artist_albums query must succeed");

    assert_eq!(album_rows.len(), 1);
    assert_eq!(album_rows[0].0, album_id);
    assert_eq!(album_rows[0].1, "The Dark Side of the Moon");
    assert_eq!(album_rows[0].2, "Pink Floyd");
    assert_eq!(album_rows[0].3, Some(1973));
    assert_eq!(album_rows[0].6, 2); // track_count

    // 3. get_artist_tracks query
    let artist_tracks = sqlx::query_as::<_, LibraryTrack>(
        r#"
        SELECT 
            t.id, 
            t.title, 
            a.name as artist_name, 
            a.id as artist_id,
            alb.title as album_name, 
            alb.id as album_id,
            t.duration_ms,
            t.isrc,
            d.file_format as quality,
            CASE WHEN d.file_path IS NOT NULL THEN 'downloaded' ELSE 'not_downloaded' END as download_status,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path,
            alb.cover_art_url
        FROM tracks t
        JOIN track_artists ta ON ta.track_id = t.id AND ta.artist_id = ?
        JOIN artists a ON a.id = ta.artist_id
        LEFT JOIN albums alb ON alb.id = t.album_id
        LEFT JOIN downloads d ON d.track_id = t.id
        ORDER BY alb.title NULLS LAST, t.disc_number ASC NULLS LAST, t.track_number ASC NULLS LAST, t.title ASC
        "#,
    )
    .bind(artist_id)
    .fetch_all(&pool)
    .await
    .expect("get_artist_tracks query must succeed");

    assert_eq!(artist_tracks.len(), 2);
    assert_eq!(artist_tracks[0].title, "Speak to Me");
    assert_eq!(artist_tracks[0].artist_id, Some(artist_id));
    assert_eq!(artist_tracks[0].download_status, Some("downloaded".to_string()));
    assert_eq!(artist_tracks[1].title, "Breathe (In the Air)");
    assert_eq!(artist_tracks[1].artist_id, Some(artist_id));
    assert_eq!(artist_tracks[1].download_status, Some("not_downloaded".to_string()));
}

#[tokio::test]
async fn test_settings_preview_folder_path_query() {
    let pool = create_migrated_db().await;
    let (_artist_id, _guest_id, _album_id, track1_id, track2_id) = seed_test_catalog(&pool).await;

    // Track 1 has a download row with file_format = 'FLAC'
    let track1_info: (String, String, String, Option<String>, Option<i32>, i64, String) = sqlx::query_as(
        r#"
        SELECT 
            t.title, 
            COALESCE(
                (SELECT art.name FROM track_artists ta JOIN artists art ON art.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
                (SELECT art.name FROM album_artists aa JOIN artists art ON art.id = aa.artist_id WHERE aa.album_id = t.album_id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1),
                'Unknown Artist'
            ) as artist, 
            COALESCE(alb.title, 'Unknown Album') as album, 
            alb.release_date,
            t.disc_number, 
            COALESCE(CAST(t.track_number AS INTEGER), 1) as track_number, 
            COALESCE(LOWER(d.file_format), 'flac') as format
        FROM tracks t
        LEFT JOIN albums alb ON t.album_id = alb.id
        LEFT JOIN downloads d ON d.track_id = t.id
        WHERE t.id = ?
        "#,
    )
    .bind(track1_id)
    .fetch_one(&pool)
    .await
    .expect("preview_folder_path query for track1 must succeed on migrated DB without schema error");

    assert_eq!(track1_info.0, "Speak to Me");
    assert_eq!(track1_info.1, "Pink Floyd");
    assert_eq!(track1_info.2, "The Dark Side of the Moon");
    assert_eq!(track1_info.3, Some("1973-03-01".to_string()));
    assert_eq!(track1_info.4, Some(1));
    assert_eq!(track1_info.5, 1);
    assert_eq!(track1_info.6, "flac");

    // Track 2 has no download row -> fallback format 'flac'
    let track2_info: (String, String, String, Option<String>, Option<i32>, i64, String) = sqlx::query_as(
        r#"
        SELECT 
            t.title, 
            COALESCE(
                (SELECT art.name FROM track_artists ta JOIN artists art ON art.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
                (SELECT art.name FROM album_artists aa JOIN artists art ON art.id = aa.artist_id WHERE aa.album_id = t.album_id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1),
                'Unknown Artist'
            ) as artist, 
            COALESCE(alb.title, 'Unknown Album') as album, 
            alb.release_date,
            t.disc_number, 
            COALESCE(CAST(t.track_number AS INTEGER), 1) as track_number, 
            COALESCE(LOWER(d.file_format), 'flac') as format
        FROM tracks t
        LEFT JOIN albums alb ON t.album_id = alb.id
        LEFT JOIN downloads d ON d.track_id = t.id
        WHERE t.id = ?
        "#,
    )
    .bind(track2_id)
    .fetch_one(&pool)
    .await
    .expect("preview_folder_path query for track2 must succeed on migrated DB without schema error");

    assert_eq!(track2_info.0, "Breathe (In the Air)");
    assert_eq!(track2_info.1, "Pink Floyd");
    assert_eq!(track2_info.2, "The Dark Side of the Moon");
    assert_eq!(track2_info.3, Some("1973-03-01".to_string()));
    assert_eq!(track2_info.4, Some(1));
    assert_eq!(track2_info.5, 2);
    assert_eq!(track2_info.6, "flac");
}

#[tokio::test]
async fn test_dashboard_get_duplicate_stats_query() {
    let pool = create_migrated_db().await;
    let (artist_id, _guest_id, album_id, _t1, _t2) = seed_test_catalog(&pool).await;

    // Add duplicate of track 1
    let dupe_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number)
        VALUES ('Speak to Me', ?, 65000, 1, 1)
        RETURNING id
        "#
    )
    .bind(album_id)
    .fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(dupe_id)
        .bind(artist_id)
        .execute(&pool).await.unwrap();

    let (extra_tracks,): (i64,) = sqlx::query_as(
        r#"
        SELECT IFNULL(SUM(cnt - 1), 0) FROM (
            SELECT t.title, ta.artist_id, COUNT(*) as cnt 
            FROM tracks t 
            JOIN track_artists ta ON t.id = ta.track_id AND ta.role = 'primary' 
            GROUP BY t.title, ta.artist_id 
            HAVING COUNT(*) > 1
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .expect("get_duplicate_stats query must succeed");

    assert_eq!(extra_tracks, 1);
}
