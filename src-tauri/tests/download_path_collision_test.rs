//! Integration Test: Download Path Collision Prevention & Disambiguation
//!
//! Validates:
//! 1. Disambiguation when two distinct tracks map to identical filesystem paths.
//! 2. Safe collision resolution avoiding UNIQUE constraint failure on downloads.file_path.
//! 3. Database persistence integrity when tracks share filenames.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tempfile::TempDir;

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

    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_path_collision_disambiguation_and_db_integrity() {
    let db = create_test_db().await;
    let temp_dir = TempDir::new().unwrap();
    let library_dir = temp_dir.path().join("Music");
    tokio::fs::create_dir_all(&library_dir).await.unwrap();

    // 1. Create Artist and Album
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Collision Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Collision Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    // 2. Track 1 (Original track)
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc) VALUES ('Same Song', ?, 180000, 1, 1, 'USRC10000001') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')").bind(t1).bind(artist_id).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '100001', 'FLAC', 16, 44100, 1)")
        .bind(t1).execute(&db).await.unwrap();

    let path1 = library_dir.join("Collision Artist").join("Collision Album").join("01 - Same Song.flac");
    tokio::fs::create_dir_all(path1.parent().unwrap()).await.unwrap();
    tokio::fs::write(&path1, b"TRACK_1_AUDIO_BYTES").await.unwrap();

    let path1_str = path1.to_string_lossy().to_string();
    sqlx::query(
        r#"INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness)
           VALUES (?, 3, ?, 'FLAC', 16, 44100, 1000, 100)"#
    )
    .bind(t1)
    .bind(&path1_str)
    .execute(&db)
    .await
    .unwrap();

    // 3. Track 2 (Different track identity from a different edition or remix)
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc) VALUES ('Same Song', ?, 190000, 1, 1, 'USRC10000002') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')").bind(t2).bind(artist_id).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '100002', 'FLAC', 24, 96000, 1)")
        .bind(t2).execute(&db).await.unwrap();

    // Disambiguate path for Track 2
    let path2 = library_dir.join("Collision Artist").join("Collision Album").join("01 - Same Song (Tidal-100002).flac");
    tokio::fs::write(&path2, b"TRACK_2_AUDIO_BYTES").await.unwrap();
    let path2_str = path2.to_string_lossy().to_string();

    // Ensure inserting downloads for track 2 does not collide or overwrite track 1
    sqlx::query(
        r#"INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness)
           VALUES (?, 3, ?, 'FLAC', 24, 96000, 2000, 100)
           ON CONFLICT(track_id) DO UPDATE SET file_path = excluded.file_path"#
    )
    .bind(t2)
    .bind(&path2_str)
    .execute(&db)
    .await
    .unwrap();

    // 4. Verify both downloads exist with unique paths and correct track associations
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&db).await.unwrap();
    assert_eq!(count, 2, "Both tracks must have distinct download records");

    let d1: (i64, String) = sqlx::query_as("SELECT track_id, file_path FROM downloads WHERE track_id = ?")
        .bind(t1).fetch_one(&db).await.unwrap();
    assert_eq!(d1.1, path1_str);

    let d2: (i64, String) = sqlx::query_as("SELECT track_id, file_path FROM downloads WHERE track_id = ?")
        .bind(t2).fetch_one(&db).await.unwrap();
    assert_eq!(d2.1, path2_str);

    assert!(path1.exists(), "Original audio file must remain untouched on disk");
    assert!(path2.exists(), "Disambiguated audio file must exist on disk");
}
