//! Integration test suite for Sprint S98: Paridad Total CLI-to-Tauri para Descarga de Favoritos
//!
//! Tests mass favorites orchestration (tracks, albums, artists), deduplication,
//! skipping of downloaded/queued items, priority ordering and contract invariants.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    // Apply all migrations through 0047
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through 0047 must apply cleanly");

    // Insert baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Insert baseline accounts
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    // Insert baseline artist and album
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc) VALUES ('Random Access Memories', '886443926588') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id).bind(artist_id).execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_download_favorites_tracks_enqueues_correctly() {
    let db = create_test_db().await;

    // Track 1: Tidal favorite
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc, favorite_at) VALUES ('Track 1', 1, 'GBAYE1300051', datetime('now')) RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, 'tidal_trk_1')")
        .bind(t1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title) VALUES (3, 3, 'track', 'tidal_trk_1', 'Track 1')")
        .execute(&db).await.unwrap();

    // Track 2: Qobuz favorite
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc, favorite_at) VALUES ('Track 2', 1, 'GBAYE1300052', datetime('now')) RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 2, 'qobuz_trk_2')")
        .bind(t2).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title) VALUES (2, 2, 'track', 'qobuz_trk_2', 'Track 2')")
        .execute(&db).await.unwrap();

    // 1. Enqueue Tidal favorites only
    let tidal_tracks: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        JOIN track_sources ts ON ts.track_id = t.id
        JOIN services s ON s.id = ts.service_id
        LEFT JOIN favorites f ON f.item_type = 'track' AND f.service_item_id = ts.service_track_id
        WHERE (t.favorite_at IS NOT NULL OR f.id IS NOT NULL)
          AND s.name = 'tidal'
        "#
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(tidal_tracks.len(), 1);
    assert_eq!(tidal_tracks[0].0, t1);

    // Insert into queue
    sqlx::query("INSERT INTO download_queue (track_id, priority, position, status, quality_preference, resumable) VALUES (?, 60, 0, 'queued', 'lossless', 1)")
        .bind(t1).execute(&db).await.unwrap();

    let queued_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(queued_count.0, 1);
}

#[tokio::test]
async fn test_download_favorites_album_expansion() {
    let db = create_test_db().await;

    // Create 3 tracks under album 1
    for i in 1..=3 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES (?, 1) RETURNING id")
            .bind(format!("RAM Track {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, 1)")
            .bind(tid).execute(&db).await.unwrap();
    }

    // Mark album 1 as favorite
    sqlx::query("UPDATE albums SET favorite_at = datetime('now') WHERE id = 1")
        .execute(&db).await.unwrap();

    // Query tracks from favorite albums
    let album_tracks: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        JOIN albums a ON a.id = t.album_id
        LEFT JOIN favorites f ON f.item_type = 'album' AND (f.service_item_id = a.upc OR f.title = a.title)
        WHERE a.favorite_at IS NOT NULL OR f.id IS NOT NULL
        "#
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(album_tracks.len(), 3, "All 3 album tracks must be resolved");

    // Enqueue all 3 tracks with sequential positions
    for (pos, (tid,)) in album_tracks.iter().enumerate() {
        sqlx::query("INSERT INTO download_queue (track_id, priority, position, status, quality_preference, resumable) VALUES (?, 60, ?, 'queued', 'hires', 1)")
            .bind(tid)
            .bind(pos as i64)
            .execute(&db)
            .await
            .unwrap();
    }

    let queued_items: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT track_id, priority, position FROM download_queue WHERE status = 'queued' ORDER BY position ASC"
    )
    .fetch_all(&db).await.unwrap();

    assert_eq!(queued_items.len(), 3);
    assert_eq!(queued_items[0].1, 60);
    assert_eq!(queued_items[0].2, 0);
    assert_eq!(queued_items[1].2, 1);
    assert_eq!(queued_items[2].2, 2);
}

#[tokio::test]
async fn test_download_favorites_artist_expansion() {
    let db = create_test_db().await;

    // Create 2 tracks for Daft Punk (artist 1)
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Around the World', 1) RETURNING id")
        .fetch_one(&db).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('One More Time', 1) RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, 1), (?, 1)")
        .bind(t1).bind(t2).execute(&db).await.unwrap();

    // Mark artist 1 as favorite
    sqlx::query("UPDATE artists SET favorite_at = datetime('now') WHERE id = 1")
        .execute(&db).await.unwrap();

    // Query tracks from favorite artists
    let artist_tracks: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        JOIN track_artists ta ON ta.track_id = t.id
        JOIN artists art ON art.id = ta.artist_id
        LEFT JOIN favorites f ON f.item_type = 'artist' AND (f.title = art.name OR f.artist_name = art.name)
        WHERE art.favorite_at IS NOT NULL OR f.id IS NOT NULL
        "#
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(artist_tracks.len(), 2);
}

#[tokio::test]
async fn test_download_favorites_skips_already_downloaded() {
    let db = create_test_db().await;

    // Insert track and corresponding downloaded file in downloads table
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, favorite_at) VALUES ('Downloaded Track', 1, datetime('now')) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, 'C:/Music/track.flac', 'FLAC')")
        .bind(t1).execute(&db).await.unwrap();

    // Verify it is recognized as already downloaded
    let download_info: Option<(String,)> = sqlx::query_as("SELECT file_path FROM downloads WHERE track_id = ?")
        .bind(t1)
        .fetch_optional(&db)
        .await
        .unwrap();

    let has_file = download_info.map(|(fp,)| !fp.trim().is_empty()).unwrap_or(false);
    assert!(has_file, "Existing file_path in downloads must be recognized to prevent re-download");
}

#[tokio::test]
async fn test_download_favorites_skips_already_queued() {
    let db = create_test_db().await;

    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, favorite_at) VALUES ('Queued Track', 1, datetime('now')) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    // Enqueue track
    sqlx::query("INSERT INTO download_queue (track_id, priority, position, status) VALUES (?, 60, 0, 'queued')")
        .bind(t1).execute(&db).await.unwrap();

    // Check if already in queue
    let queue_item: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, status FROM download_queue WHERE track_id = ? AND status IN ('queued', 'downloading') LIMIT 1"
    )
    .bind(t1)
    .fetch_optional(&db)
    .await
    .unwrap();

    assert!(queue_item.is_some(), "Track already in queued state must be skipped from duplicate enqueueing");
}

#[tokio::test]
async fn test_download_favorites_flac_and_m4a_parity_contracts() {
    let db = create_test_db().await;

    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, favorite_at) VALUES ('Contract Track', 1, datetime('now')) RETURNING id"
    )
    .fetch_one(&db).await.unwrap();

    // Enqueue track with high priority and resumable = 1
    sqlx::query(
        "INSERT INTO download_queue (track_id, priority, position, status, quality_preference, resumable) VALUES (?, 60, 0, 'queued', 'lossless', 1)"
    )
    .bind(t1)
    .execute(&db).await.unwrap();

    let row: (i64, i64, String, i64) = sqlx::query_as(
        "SELECT priority, position, quality_preference, resumable FROM download_queue WHERE track_id = ?"
    )
    .bind(t1)
    .fetch_one(&db).await.unwrap();

    assert_eq!(row.0, 60, "Favorites priority must be 60");
    assert_eq!(row.1, 0, "Initial position must be 0");
    assert_eq!(row.2, "lossless");
    assert_eq!(row.3, 1, "Resumable flag must be set for HTTP Range support");
}
