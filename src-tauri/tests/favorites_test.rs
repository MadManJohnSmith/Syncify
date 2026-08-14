//! Integration Tests for Syncify Favorites Domain and Persistence

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::types::LibraryTrack;

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    // Run core normalized schema
    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            supports_download INTEGER DEFAULT 0,
            max_quality TEXT DEFAULT 'lossless'
        );
        CREATE TABLE artists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE albums (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            release_date TEXT,
            total_tracks INTEGER DEFAULT 0,
            cover_art_url TEXT
        );
        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            album_id INTEGER REFERENCES albums(id),
            duration_ms INTEGER,
            track_number INTEGER,
            disc_number INTEGER DEFAULT 1,
            isrc TEXT UNIQUE,
            genre TEXT,
            bpm REAL,
            musical_key TEXT,
            release_year INTEGER,
            explicit INTEGER DEFAULT 0,
            musicbrainz_id TEXT,
            audio_quality TEXT,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            favorite_at TEXT
        );
        CREATE TABLE track_artists (
            track_id INTEGER REFERENCES tracks(id),
            artist_id INTEGER REFERENCES artists(id),
            role TEXT DEFAULT 'primary',
            PRIMARY KEY (track_id, artist_id)
        );
        CREATE TABLE track_sources (
            id INTEGER PRIMARY KEY,
            track_id INTEGER REFERENCES tracks(id),
            service_id INTEGER REFERENCES services(id),
            service_track_id TEXT NOT NULL,
            format TEXT,
            UNIQUE(track_id, service_id),
            UNIQUE(service_id, service_track_id)
        );
        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            source_service_id INTEGER REFERENCES services(id),
            file_path TEXT NOT NULL,
            file_format TEXT NOT NULL,
            bit_depth INTEGER,
            sample_rate INTEGER,
            file_size_bytes INTEGER,
            status TEXT NOT NULL DEFAULT 'verified'
        );
        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY,
            track_id INTEGER REFERENCES tracks(id),
            status TEXT DEFAULT 'queued',
            priority INTEGER DEFAULT 50,
            created_at TEXT DEFAULT (datetime('now'))
        );
        CREATE TABLE lyrics (
            id INTEGER PRIMARY KEY,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            content TEXT,
            sync_level TEXT DEFAULT 'none'
        );
        CREATE INDEX idx_tracks_favorite ON tracks(is_favorite);
        CREATE INDEX idx_tracks_favorite_at ON tracks(is_favorite, favorite_at DESC);
        "#
    )
    .execute(&pool)
    .await
    .expect("Schema init must succeed");

    pool
}

#[tokio::test]
async fn test_insert_favorite() {
    let db = create_test_db().await;
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Heroes', 'USJT11700035') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Toggle to favorite
    let res: (i32,) = sqlx::query_as(
        "UPDATE tracks \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite"
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(res.0, 1, "Track must be marked as favorite (1)");

    let row: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(row.0, 1);
    assert!(row.1.is_some(), "favorite_at timestamp must be populated on insert");
}

#[tokio::test]
async fn test_remove_favorite() {
    let db = create_test_db().await;
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES ('Heroes', 'USJT11700035', 1, datetime('now')) RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Toggle to unfavorite
    let res: (i32,) = sqlx::query_as(
        "UPDATE tracks \
         SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END, \
             favorite_at = CASE WHEN is_favorite = 0 THEN datetime('now') ELSE NULL END \
         WHERE id = ? \
         RETURNING is_favorite"
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(res.0, 0, "Track must be unmarked as favorite (0)");

    let row: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(row.0, 0);
    assert!(row.1.is_none(), "favorite_at timestamp must be cleared on remove");
}

#[tokio::test]
async fn test_idempotence_insert_favorite() {
    let db = create_test_db().await;
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Heroes', 'USJT11700035') RETURNING id")
        .fetch_one(&db).await.unwrap();

    for _ in 0..3 {
        sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = COALESCE(favorite_at, datetime('now')) WHERE id = ?")
            .bind(track_id)
            .execute(&db)
            .await
            .unwrap();
    }

    let is_fav: i32 = sqlx::query_scalar("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(is_fav, 1, "Idempotent set_favorite(true) must remain 1");
}

#[tokio::test]
async fn test_idempotence_remove_favorite() {
    let db = create_test_db().await;
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc, is_favorite) VALUES ('Heroes', 'USJT11700035', 1) RETURNING id")
        .fetch_one(&db).await.unwrap();

    for _ in 0..3 {
        sqlx::query("UPDATE tracks SET is_favorite = 0, favorite_at = NULL WHERE id = ?")
            .bind(track_id)
            .execute(&db)
            .await
            .unwrap();
    }

    let is_fav: i32 = sqlx::query_scalar("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(is_fav, 0, "Idempotent set_favorite(false) must remain 0");
}

#[tokio::test]
async fn test_favorite_downloaded_track() {
    let db = create_test_db().await;
    let service_id: i64 = sqlx::query_scalar("INSERT INTO services (name, supports_download) VALUES ('tidal', 1) RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Heroes', 'USJT11700035') RETURNING id")
        .fetch_one(&db).await.unwrap();

    let dummy_path = "C:/Users/tardis/Music/Syncify/David Bowie/Heroes/03 - Heroes.flac";
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, status) VALUES (?, ?, ?, 'FLAC', 'verified')")
        .bind(track_id).bind(service_id).bind(dummy_path).execute(&db).await.unwrap();

    // Mark as favorite
    sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
        .bind(track_id).execute(&db).await.unwrap();

    // Invariant: downloads row must be completely unaffected
    let dl_row: (String, String) = sqlx::query_as("SELECT file_path, status FROM downloads WHERE track_id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();

    assert_eq!(dl_row.0, dummy_path, "Physical file path must not be modified by favorite toggle");
    assert_eq!(dl_row.1, "verified", "Download verification status must remain verified");
}

#[tokio::test]
async fn test_favorite_non_downloaded_track() {
    let db = create_test_db().await;
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Cigaro', 'USSM10502123') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Verify 0 downloads
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(count, 0, "Track has no physical download record");

    // Mark favorite
    sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
        .bind(track_id).execute(&db).await.unwrap();

    let is_fav: i32 = sqlx::query_scalar("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(is_fav, 1, "Non-downloaded track must support favorite status without downloads record");
}

#[tokio::test]
async fn test_favorite_persistence_across_reconnect() {
    use sqlx::sqlite::SqliteConnectOptions;

    let temp_db_path = std::env::temp_dir().join(format!("syncify_fav_test_{}.db", uuid::Uuid::new_v4()));
    let opts = SqliteConnectOptions::new()
        .filename(&temp_db_path)
        .create_if_missing(true);

    let pool1 = SqlitePoolOptions::new().connect_with(opts.clone()).await.unwrap();
    sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT, is_favorite INTEGER DEFAULT 0, favorite_at TEXT);")
        .execute(&pool1).await.unwrap();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, is_favorite, favorite_at) VALUES ('Reopen Test', 1, '2026-08-15T00:00:00Z') RETURNING id")
        .fetch_one(&pool1).await.unwrap();

    drop(pool1); // Simulate application close

    // Reopen connection pool
    let pool2 = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
    let row: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_id).fetch_one(&pool2).await.unwrap();

    assert_eq!(row.0, 1, "Favorite status must persist across app restart/reconnect");
    assert_eq!(row.1.as_deref(), Some("2026-08-15T00:00:00Z"));

    drop(pool2);
    let _ = tokio::fs::remove_file(&temp_db_path).await;
}

#[tokio::test]
async fn test_deduplication_by_isrc_and_service_track_id() {
    let db = create_test_db().await;
    let service_id: i64 = sqlx::query_scalar("INSERT INTO services (name) VALUES ('tidal') RETURNING id")
        .fetch_one(&db).await.unwrap();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Track A', 'USJT11700035') RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, '80654035')")
        .bind(track_id).bind(service_id).execute(&db).await.unwrap();

    // Duplicate ISRC must fail UNIQUE constraint
    let dup_isrc_res = sqlx::query("INSERT INTO tracks (title, isrc) VALUES ('Track A Duplicate', 'USJT11700035')")
        .execute(&db).await;
    assert!(dup_isrc_res.is_err(), "Duplicate ISRC insertion must be rejected by UNIQUE constraint");

    // Duplicate Service Track ID must fail UNIQUE constraint
    let dup_source_res = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (999, ?, '80654035')")
        .bind(service_id).execute(&db).await;
    assert!(dup_source_res.is_err(), "Duplicate (service_id, service_track_id) must be rejected by UNIQUE constraint");
}

#[tokio::test]
async fn test_sqlite_error_rollback_safety() {
    let db = create_test_db().await;

    // Toggle non-existent track
    let result: Option<(i32,)> = sqlx::query_as(
        "UPDATE tracks SET is_favorite = CASE WHEN is_favorite = 0 THEN 1 ELSE 0 END WHERE id = 999999 RETURNING is_favorite"
    )
    .fetch_optional(&db)
    .await
    .unwrap();

    assert!(result.is_none(), "Updating non-existent track must return None / Err");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&db).await.unwrap();
    assert_eq!(count, 0, "No tracks created or corrupted on invalid favorite toggle");
}

#[tokio::test]
async fn test_filter_favorites_pagination() {
    let db = create_test_db().await;

    for i in 1..=15 {
        let is_fav = if i % 2 == 1 { 1 } else { 0 };
        let fav_at = if is_fav == 1 { Some(format!("2026-08-15T00:{:02}:00Z", i)) } else { None };
        sqlx::query("INSERT INTO tracks (title, isrc, is_favorite, favorite_at) VALUES (?, ?, ?, ?)")
            .bind(format!("Song {:02}", i))
            .bind(format!("ISRC{:08}", i))
            .bind(is_fav)
            .bind(fav_at)
            .execute(&db)
            .await
            .unwrap();
    }

    // Total favorites = 8 (odd numbers: 1, 3, 5, 7, 9, 11, 13, 15)
    let total_favs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE is_favorite = 1")
        .fetch_one(&db).await.unwrap();
    assert_eq!(total_favs, 8);

    // Page 1: limit 5, offset 0
    let page1_tracks: Vec<LibraryTrack> = sqlx::query_as(
        r#"
        SELECT
            t.id, t.title, NULL as artist_name, NULL as artist_id, NULL as album_name, NULL as album_id,
            t.duration_ms, t.isrc, NULL as services, NULL as quality, 'not_downloaded' as download_status,
            100 as metadata_score, 'none' as lyrics_type, NULL as cover_art_url, NULL as spotify_track_id,
            t.track_number, t.disc_number, t.genre, t.bpm, t.musical_key, t.release_year, t.explicit,
            t.is_favorite, t.favorite_at, NULL as file_path
        FROM tracks t
        WHERE t.is_favorite = 1
        ORDER BY t.favorite_at DESC
        LIMIT 5 OFFSET 0
        "#
    )
    .fetch_all(&db).await.unwrap();

    assert_eq!(page1_tracks.len(), 5);
    assert_eq!(page1_tracks[0].title, "Song 15", "First song in page must be the most recently favorited (Song 15)");
    assert_eq!(page1_tracks[4].title, "Song 07");

    // Page 2: limit 5, offset 5
    let page2_tracks: Vec<LibraryTrack> = sqlx::query_as(
        r#"
        SELECT
            t.id, t.title, NULL as artist_name, NULL as artist_id, NULL as album_name, NULL as album_id,
            t.duration_ms, t.isrc, NULL as services, NULL as quality, 'not_downloaded' as download_status,
            100 as metadata_score, 'none' as lyrics_type, NULL as cover_art_url, NULL as spotify_track_id,
            t.track_number, t.disc_number, t.genre, t.bpm, t.musical_key, t.release_year, t.explicit,
            t.is_favorite, t.favorite_at, NULL as file_path
        FROM tracks t
        WHERE t.is_favorite = 1
        ORDER BY t.favorite_at DESC
        LIMIT 5 OFFSET 5
        "#
    )
    .fetch_all(&db).await.unwrap();

    assert_eq!(page2_tracks.len(), 3, "Page 2 must contain remaining 3 favorites");
    assert_eq!(page2_tracks[0].title, "Song 05");
    assert_eq!(page2_tracks[2].title, "Song 01");
}

#[tokio::test]
async fn test_migration_0044_clean_db_and_reapply_idempotence() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    // First run on clean DB: all migrations 0001 -> 0044
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Clean migration up to 0044 must succeed");

    // Verify favorite_at column and default is_favorite = 0
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Migration Test Track') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    let row: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.0, 0, "Default is_favorite must be 0");
    assert_eq!(row.1, None, "Default favorite_at must be NULL");

    // Second run (simulate app startup / reboot): must be 100% idempotent without errors
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Reapplying migrations on app restart must be idempotent");
}

#[tokio::test]
async fn test_migration_0044_from_prior_favorites_state() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    // Setup base table with 0020_favorites schema (is_favorite present, favorite_at missing)
    sqlx::query(
        r#"
        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            is_favorite INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_tracks_favorite ON tracks(is_favorite);
        "#
    )
    .execute(&pool).await.unwrap();

    // Insert existing favorite and non-favorite tracks before migration 0044
    let fav_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, is_favorite) VALUES ('Pre-existing Favorite', 1) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let non_fav_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, is_favorite) VALUES ('Pre-existing Normal', 0) RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Execute migration 0044 manually
    sqlx::query(
        r#"
        ALTER TABLE tracks ADD COLUMN favorite_at TEXT;
        CREATE INDEX IF NOT EXISTS idx_tracks_favorite_at ON tracks(is_favorite, favorite_at DESC);
        "#
    )
    .execute(&pool).await.unwrap();

    // Verify existing favorite state is preserved
    let fav_row: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(fav_id).fetch_one(&pool).await.unwrap();
    assert_eq!(fav_row.0, 1, "Pre-existing favorite flag must NOT be lost");
    assert_eq!(fav_row.1, None, "Pre-existing favorite initially has NULL favorite_at until updated");

    let non_fav_row: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(non_fav_id).fetch_one(&pool).await.unwrap();
    assert_eq!(non_fav_row.0, 0);
    assert_eq!(non_fav_row.1, None);

    // Toggle pre-existing favorite to update timestamp
    sqlx::query("UPDATE tracks SET favorite_at = datetime('now') WHERE id = ?")
        .bind(fav_id).execute(&pool).await.unwrap();
    let updated_fav: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(fav_id).fetch_one(&pool).await.unwrap();
    assert_eq!(updated_fav.0, 1);
    assert!(updated_fav.1.is_some());
}

#[tokio::test]
async fn test_migration_0044_incompatible_schema_fails_explicitly() {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .unwrap();

    // Empty database without 'tracks' table - running 0044 SQL must fail explicitly
    let res = sqlx::query(
        r#"
        ALTER TABLE tracks ADD COLUMN favorite_at TEXT;
        CREATE INDEX IF NOT EXISTS idx_tracks_favorite_at ON tracks(is_favorite, favorite_at DESC);
        "#
    )
    .execute(&pool).await;

    assert!(res.is_err(), "Migration 0044 must return an explicit Err if the target schema is incompatible");
}

#[tokio::test]
async fn test_favorites_distinct_masters_editions_no_auto_merge() {
    let db = create_test_db().await;
    let service_id: i64 = sqlx::query_scalar("INSERT INTO services (name) VALUES ('tidal') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Two distinct versions of "Heroes": Standard Release (1977) vs Remastered (2017)
    // Both share the same logical title and artist, but have distinct service_track_ids and releases
    let track_std_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, release_year) VALUES ('Heroes (Original)', 1977) RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_rem_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, release_year) VALUES ('Heroes (2017 Remaster)', 2017) RETURNING id")
        .fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, '12345678')")
        .bind(track_std_id).bind(service_id).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, '80654035')")
        .bind(track_rem_id).bind(service_id).execute(&db).await.unwrap();

    // Mark ONLY the Remaster as favorite
    sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = datetime('now') WHERE id = ?")
        .bind(track_rem_id).execute(&db).await.unwrap();

    let std_fav: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_std_id).fetch_one(&db).await.unwrap();
    let rem_fav: (i32, Option<String>) = sqlx::query_as("SELECT is_favorite, favorite_at FROM tracks WHERE id = ?")
        .bind(track_rem_id).fetch_one(&db).await.unwrap();

    assert_eq!(std_fav.0, 0, "Standard release must remain not favorite");
    assert_eq!(rem_fav.0, 1, "Remaster release must be favorite");
    assert_ne!(track_std_id, track_rem_id, "Different editions/masters must not be merged automatically");
}
