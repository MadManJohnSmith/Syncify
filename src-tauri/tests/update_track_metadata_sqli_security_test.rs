use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use std::sync::Arc;
use syncify_tauri_lib::commands::{update_track_metadata, UpdateTrackMetadata};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tauri::Manager;

async fn setup_test_context() -> (tauri::App<tauri::test::MockRuntime>, sqlx::SqlitePool, i64, i64) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // Insert 2 initial artists
    let artist_1: i64 = sqlx::query("INSERT INTO artists (name) VALUES ('Artist One') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);

    let artist_2: i64 = sqlx::query("INSERT INTO artists (name) VALUES ('Artist Two') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);

    // Insert 2 initial albums
    let album_1: i64 = sqlx::query("INSERT INTO albums (title, release_date) VALUES ('Album One', '2020-01-01') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);

    let album_2: i64 = sqlx::query("INSERT INTO albums (title, release_date) VALUES ('Album Two', '2021-01-01') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);

    // Insert 2 initial tracks
    let track_1: i64 = sqlx::query(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, bpm) VALUES ('Track One', ?, 180000, 1, 1, 120.0) RETURNING id"
    )
    .bind(album_1)
    .fetch_one(&pool)
    .await
    .unwrap()
    .get(0);

    let track_2: i64 = sqlx::query(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, bpm) VALUES ('Track Two', ?, 210000, 2, 1, 130.0) RETURNING id"
    )
    .bind(album_2)
    .fetch_one(&pool)
    .await
    .unwrap()
    .get(0);

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'main')")
        .bind(track_1)
        .bind(artist_1)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'main')")
        .bind(track_2)
        .bind(artist_2)
        .execute(&pool)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);

    (app, pool, track_1, track_2)
}

#[tokio::test]
async fn test_sqli_payloads_in_all_fields_stored_literally() {
    let (app, pool, track_1, track_2) = setup_test_context().await;
    let app_state = app.state::<AppState>();

    let malicious_payload = UpdateTrackMetadata {
        title: Some("Title'; DROP TABLE tracks; --".to_string()),
        album_name: Some("Album'; DROP TABLE albums; --".to_string()),
        artist_name: Some("Artist'; DELETE FROM artists; --".to_string()),
        track_number: Some(7),
        disc_number: Some(1),
        isrc: Some("US123'; DROP TABLE tracks; --".to_string()),
        explicit: Some(true),
        genre: Some("Rock'; UPDATE tracks SET bpm = 999.0; --".to_string()),
        year: Some(2025),
        bpm: Some(125.0),
        musical_key: Some("Am' OR '1'='1".to_string()),
        mb_track_id: Some("mbid-666' UNION SELECT id, name FROM artists --".to_string()),
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: Some("Label' OR 1=1; DELETE FROM tracks; --".to_string()),
    };

    let result = update_track_metadata(app_state, track_1, malicious_payload).await;
    assert!(result.is_ok(), "update_track_metadata must succeed safely: {:?}", result.err());

    // 1. Ensure tracks table exists and no records were dropped
    let tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tracks_count, 2, "Tracks table must still contain exactly 2 records");

    // 2. Ensure albums and artists tables are intact
    let albums_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(albums_count >= 2, "Albums table must not be dropped or emptied");

    let artists_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(artists_count >= 2, "Artists table must not be dropped or emptied");

    // 3. Ensure track_2 was not affected by any injected statement (e.g. bpm update or deletion)
    let track_2_row = sqlx::query("SELECT title, bpm FROM tracks WHERE id = ?")
        .bind(track_2)
        .fetch_one(&pool)
        .await
        .unwrap();
    let track_2_title: String = track_2_row.get("title");
    let track_2_bpm: f64 = track_2_row.get("bpm");
    assert_eq!(track_2_title, "Track Two");
    assert_eq!(track_2_bpm, 130.0, "Track 2 BPM must remain untouched");

    // 4. Verify fields on track_1 were stored literally without SQL interpretation
    let track_1_row = sqlx::query(
        "SELECT title, isrc, genre, musical_key, musicbrainz_id, record_label, bpm FROM tracks WHERE id = ?"
    )
    .bind(track_1)
    .fetch_one(&pool)
    .await
    .unwrap();

    let title: String = track_1_row.get("title");
    let isrc: String = track_1_row.get("isrc");
    let genre: String = track_1_row.get("genre");
    let musical_key: String = track_1_row.get("musical_key");
    let mbid: String = track_1_row.get("musicbrainz_id");
    let label: String = track_1_row.get("record_label");
    let bpm: f64 = track_1_row.get("bpm");

    assert_eq!(title, "Title'; DROP TABLE tracks; --");
    assert_eq!(isrc, "US123'; DROP TABLE tracks; --");
    // Migration 74 trigger normalizes genre on ';' delimiter to primary genre "Rock'"
    assert_eq!(genre, "Rock'");
    assert_eq!(musical_key, "Am' OR '1'='1");
    assert_eq!(mbid, "mbid-666' UNION SELECT id, name FROM artists --");
    assert_eq!(label, "Label' OR 1=1; DELETE FROM tracks; --");
    assert_eq!(bpm, 125.0);
}

#[tokio::test]
async fn test_individual_fields_sqli_isolation() {
    let (app, pool, track_1, track_2) = setup_test_context().await;
    let app_state = app.state::<AppState>();

    // Vector A: isrc alone with DROP TABLE
    let payload_isrc = UpdateTrackMetadata {
        title: None,
        album_name: None,
        artist_name: None,
        track_number: None,
        disc_number: None,
        isrc: Some("ISRC'; DROP TABLE tracks; --".to_string()),
        explicit: None,
        genre: None,
        year: None,
        bpm: None,
        musical_key: None,
        mb_track_id: None,
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: None,
    };
    let res = update_track_metadata(app_state.clone(), track_1, payload_isrc).await;
    assert!(res.is_ok());
    let isrc: String = sqlx::query_scalar("SELECT isrc FROM tracks WHERE id = ?")
        .bind(track_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(isrc, "ISRC'; DROP TABLE tracks; --");

    // Vector B: musical_key alone with tautology
    let payload_key = UpdateTrackMetadata {
        title: None,
        album_name: None,
        artist_name: None,
        track_number: None,
        disc_number: None,
        isrc: None,
        explicit: None,
        genre: None,
        year: None,
        bpm: None,
        musical_key: Some("C#m' OR '1'='1".to_string()),
        mb_track_id: None,
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: None,
    };
    let res = update_track_metadata(app_state.clone(), track_1, payload_key).await;
    assert!(res.is_ok());
    let key: String = sqlx::query_scalar("SELECT musical_key FROM tracks WHERE id = ?")
        .bind(track_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(key, "C#m' OR '1'='1");

    // Vector C: mb_track_id alone with DELETE
    let payload_mbid = UpdateTrackMetadata {
        title: None,
        album_name: None,
        artist_name: None,
        track_number: None,
        disc_number: None,
        isrc: None,
        explicit: None,
        genre: None,
        year: None,
        bpm: None,
        musical_key: None,
        mb_track_id: Some("mbid-xyz'; DELETE FROM tracks; --".to_string()),
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: None,
    };
    let res = update_track_metadata(app_state.clone(), track_1, payload_mbid).await;
    assert!(res.is_ok());
    let mbid: String = sqlx::query_scalar("SELECT musicbrainz_id FROM tracks WHERE id = ?")
        .bind(track_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mbid, "mbid-xyz'; DELETE FROM tracks; --");

    // Vector D: label alone with UNION SELECT
    let payload_label = UpdateTrackMetadata {
        title: None,
        album_name: None,
        artist_name: None,
        track_number: None,
        disc_number: None,
        isrc: None,
        explicit: None,
        genre: None,
        year: None,
        bpm: None,
        musical_key: None,
        mb_track_id: None,
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: Some("Label' UNION SELECT null, null, null --".to_string()),
    };
    let res = update_track_metadata(app_state.clone(), track_1, payload_label).await;
    assert!(res.is_ok());
    let label: String = sqlx::query_scalar("SELECT record_label FROM tracks WHERE id = ?")
        .bind(track_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(label, "Label' UNION SELECT null, null, null --");

    // Vector E: genre with tautology and quotes
    let payload_genre = UpdateTrackMetadata {
        title: None,
        album_name: None,
        artist_name: None,
        track_number: None,
        disc_number: None,
        isrc: None,
        explicit: None,
        genre: Some("Electronic' OR 'x'='x".to_string()),
        year: None,
        bpm: None,
        musical_key: None,
        mb_track_id: None,
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: None,
    };
    let res = update_track_metadata(app_state.clone(), track_1, payload_genre).await;
    assert!(res.is_ok());
    let genre: String = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = ?")
        .bind(track_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(genre, "Electronic' OR 'x'='x");

    // Ensure total track count is still 2
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2, "Tracks count must remain 2 throughout single-field injections");

    // Ensure track 2 was never altered
    let t2_bpm: f64 = sqlx::query_scalar("SELECT bpm FROM tracks WHERE id = ?")
        .bind(track_2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t2_bpm, 130.0);
}

#[tokio::test]
async fn test_legitimate_update_persists_correctly() {
    let (app, pool, track_1, _) = setup_test_context().await;
    let app_state = app.state::<AppState>();

    let legit_payload = UpdateTrackMetadata {
        title: Some("Hotel California".to_string()),
        album_name: Some("Hotel California (2013 Remaster)".to_string()),
        artist_name: Some("Eagles".to_string()),
        track_number: Some(1),
        disc_number: Some(1),
        isrc: Some("USPR37603914".to_string()),
        explicit: Some(false),
        genre: Some("Classic Rock".to_string()),
        year: Some(1976),
        bpm: Some(75.0),
        musical_key: Some("Bm".to_string()),
        mb_track_id: Some("7c1b5042-4f32-4d51-8742-c2e8c258e77a".to_string()),
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: Some("Asylum Records".to_string()),
    };

    let result = update_track_metadata(app_state, track_1, legit_payload).await;
    assert!(result.is_ok(), "Legitimate update must succeed: {:?}", result.err());

    let track = result.unwrap();
    assert_eq!(track.title, "Hotel California");
    assert_eq!(track.artist_name, Some("Eagles".to_string()));
    assert_eq!(track.album_name, Some("Hotel California (2013 Remaster)".to_string()));
    assert_eq!(track.track_number, Some(1));
    assert_eq!(track.disc_number, Some(1));
    assert_eq!(track.isrc, Some("USPR37603914".to_string()));
    assert_eq!(track.explicit, Some(false));
    assert_eq!(track.genre, Some("Classic Rock".to_string()));
    assert_eq!(track.release_year, Some(1976));
    assert_eq!(track.bpm, Some(75.0));
    assert_eq!(track.musical_key, Some("Bm".to_string()));

    // Verify directly in database
    let row = sqlx::query(
        "SELECT t.title, t.isrc, t.record_label, t.musicbrainz_id, al.title as album_title, ar.name as artist_name \
         FROM tracks t \
         JOIN albums al ON t.album_id = al.id \
         JOIN track_artists ta ON t.id = ta.track_id AND ta.role = 'main' \
         JOIN artists ar ON ta.artist_id = ar.id \
         WHERE t.id = ?"
    )
    .bind(track_1)
    .fetch_one(&pool)
    .await
    .unwrap();

    let db_title: String = row.get("title");
    let db_isrc: String = row.get("isrc");
    let db_label: String = row.get("record_label");
    let db_mbid: String = row.get("musicbrainz_id");
    let db_album: String = row.get("album_title");
    let db_artist: String = row.get("artist_name");

    assert_eq!(db_title, "Hotel California");
    assert_eq!(db_isrc, "USPR37603914");
    assert_eq!(db_label, "Asylum Records");
    assert_eq!(db_mbid, "7c1b5042-4f32-4d51-8742-c2e8c258e77a");
    assert_eq!(db_album, "Hotel California (2013 Remaster)");
    assert_eq!(db_artist, "Eagles");
}

#[tokio::test]
async fn test_empty_metadata_update_leaves_record_unchanged() {
    let (app, pool, track_1, _) = setup_test_context().await;
    let app_state = app.state::<AppState>();

    let empty_payload = UpdateTrackMetadata {
        title: None,
        album_name: None,
        artist_name: None,
        track_number: None,
        disc_number: None,
        isrc: None,
        explicit: None,
        genre: None,
        year: None,
        bpm: None,
        musical_key: None,
        mb_track_id: None,
        _mb_release_id: None,
        _upc: None,
        _copyright: None,
        _composer: None,
        label: None,
    };

    let result = update_track_metadata(app_state, track_1, empty_payload).await;
    assert!(result.is_ok(), "Empty update must succeed: {:?}", result.err());

    let track = result.unwrap();
    assert_eq!(track.title, "Track One");
    assert_eq!(track.track_number, Some(1));
    assert_eq!(track.bpm, Some(120.0));

    // Verify DB still intact
    let title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
        .bind(track_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, "Track One");
}
