use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use std::sync::Arc;
use syncify_tauri_lib::commands::{update_track_metadata, UpdateTrackMetadata};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tauri::Manager;

#[tokio::test]
async fn test_update_track_metadata_prevents_sql_injection() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // Insert dummy artist, album, track
    let artist_id: i64 = sqlx::query("INSERT INTO artists (name) VALUES ('Original Artist') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);

    let album_id: i64 = sqlx::query("INSERT INTO albums (title, release_date) VALUES ('Original Album', '2020-01-01') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);

    let track_id: i64 = sqlx::query(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number) VALUES ('Original Title', ?, 200000, 1, 1) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap()
    .get(0);

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'main')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        album_lock: Arc::new(tokio::sync::Mutex::new(())),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);

    let app_state = app.state::<AppState>();

    // Malicious payload with SQL injection vectors
    let malicious_payload = UpdateTrackMetadata {
        title: Some("Test'; DROP TABLE tracks; --".to_string()),
        album_name: Some("Album'; DROP TABLE albums; --".to_string()),
        artist_name: Some("Artist'; DROP TABLE artists; --".to_string()),
        track_number: Some(5),
        disc_number: Some(1),
        isrc: Some("US123'; DELETE FROM tracks; --".to_string()),
        explicit: Some(true),
        genre: Some("Rock'; UPDATE tracks SET bpm = 999; --".to_string()),
        year: Some(2023),
        bpm: Some(128.5),
        musical_key: Some("Am' OR 1=1 --".to_string()),
        mb_track_id: Some("mbid-123' OR 'a'='a".to_string()),
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: Some("Label' UNION SELECT * FROM users --".to_string()),
    };

    let result = update_track_metadata(app_state, track_id, malicious_payload).await;
    assert!(result.is_ok(), "update_track_metadata should succeed cleanly: {:?}", result.err());

    let updated_track = result.unwrap();

    // Verify all fields are stored as literal strings without executing any SQL commands
    assert_eq!(updated_track.title, "Test'; DROP TABLE tracks; --");
    assert_eq!(updated_track.artist_name, Some("Artist'; DROP TABLE artists; --".to_string()));
    assert_eq!(updated_track.album_name, Some("Album'; DROP TABLE albums; --".to_string()));
    assert_eq!(updated_track.isrc, Some("US123'; DELETE FROM tracks; --".to_string()));
    assert_eq!(updated_track.genre, Some("Rock'; UPDATE tracks SET bpm = 999; --".to_string()));
    assert_eq!(updated_track.musical_key, Some("Am' OR 1=1 --".to_string()));
    assert_eq!(updated_track.bpm, Some(128.5));

    // Verify database tables still exist and were not dropped or deleted
    let tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks_count, 1, "tracks table must still exist and contain 1 record");

    let artists_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(artists_count >= 1, "artists table must still exist");

    let albums_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(albums_count >= 1, "albums table must still exist");

    // Verify raw fields in SQLite directly
    let row = sqlx::query("SELECT title, isrc, genre, musical_key, musicbrainz_id, record_label, bpm FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let title: String = row.get("title");
    let isrc: String = row.get("isrc");
    let genre: String = row.get("genre");
    let musical_key: String = row.get("musical_key");
    let mbid: String = row.get("musicbrainz_id");
    let label: String = row.get("record_label");
    let bpm: f64 = row.get("bpm");

    assert_eq!(title, "Test'; DROP TABLE tracks; --");
    assert_eq!(isrc, "US123'; DELETE FROM tracks; --");
    assert_eq!(genre, "Rock'; UPDATE tracks SET bpm = 999; --");
    assert_eq!(musical_key, "Am' OR 1=1 --");
    assert_eq!(mbid, "mbid-123' OR 'a'='a");
    assert_eq!(label, "Label' UNION SELECT * FROM users --");
    assert_eq!(bpm, 128.5);
}
